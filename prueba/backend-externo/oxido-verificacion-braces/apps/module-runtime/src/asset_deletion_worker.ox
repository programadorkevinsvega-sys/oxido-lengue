sintaxis llaves
use std::{sync::Arc, time::Duration};

use async_nats::jetstream::{self, consumer::pull::Config, message::AckKind};
use futures_util::StreamExt;
use serde_json::Value;
use tokio::sync::watch;
use tonic::Request;
use xovyra_jobs::jobs::{self, EnqueueAssetDeletion};
use xovyra_jobs::scratch::ScratchSpace;
use xovyra_platform::{
    dependencies::PlatformDependencies,
    grpc::{
        CompleteAssetDeletionRequest, GetAssetDeletionTargetRequest,
        asset_storage_client::AssetStorageClient,
    },
};

use crate::r2_storage::R2Storage;

pub async fn run(
    dependencies: Arc<PlatformDependencies>,
    storage: Option<Arc<R2Storage>>,
    scratch: Arc<ScratchSpace>,
    runtime_endpoint: String,
    worker_auth_secret: String,
    mut shutdown: watch::Receiver<bool>,
) {
    let worker_id = format!("runtime-{}", uuid::Uuid::new_v4());
    let queue_dependencies = Arc::clone(&dependencies);
    let queue_shutdown = shutdown.clone();
    let queue_task = tokio::spawn(async move {
        consume_asset_deletion_events(queue_dependencies, queue_shutdown).await;
    });
    let Some(storage) = storage else {
        tracing::warn!("R2 credentials are not configured; asset deletion jobs will remain queued");
        tokio::select! {
            result = queue_task => {
                if let Err(error) = result {
                    tracing::error!(error_type=%std::any::type_name_of_val(&error), "Asset deletion event consumer stopped");
                }
            }
            shutdown_result = shutdown.changed() => {
                if shutdown_result.is_err() || *shutdown.borrow() { return; }
            }
        }
        return;
    };
    let mut polling = tokio::time::interval(Duration::from_millis(500));
    loop {
        tokio::select! {
            _ = polling.tick() => {
                match jobs::claim_next_job(&dependencies.jobs_database, &worker_id, 30).await {
                    Ok(Some(job)) => {
                        let job_id = job.job_id;
                        if let Err(error) = execute_asset_deletion(
                            &dependencies,
                            storage.as_ref(),
                            scratch.as_ref(),
                            &runtime_endpoint,
                            &worker_auth_secret,
                            &worker_id,
                            job,
                        ).await {
                            tracing::error!(error_type=%std::any::type_name_of_val(&error), "Asset deletion job failed and will retry");
                            let failure_code = match &error {
                                WorkerError::Job(_) => "job_state_conflict",
                                WorkerError::Transport(_) => "asset_runtime_unavailable",
                                WorkerError::Status(_) => "asset_state_unavailable",
                                WorkerError::Storage(_) => "r2_delete_failed",
                                WorkerError::Scratch(_) => "scratch_unavailable",
                                WorkerError::InternalAuthorization(_) => "worker_authorization_failed",
                                WorkerError::UnsupportedJob => "unsupported_job_type",
                            };
                            if let Err(failure) = jobs::fail_job(
                                &dependencies.jobs_database,
                                job_id,
                                &worker_id,
                                failure_code,
                                &job_id.to_string(),
                            ).await {
                                tracing::error!(error_type=%std::any::type_name_of_val(&failure), job_id=%job_id, "Failed asset deletion could not be scheduled for retry");
                            }
                        }
                    }
                    Ok(None) => {}
                    Err(error) => tracing::error!(error_type=%std::any::type_name_of_val(&error), "Tenant job queue is unavailable; polling will retry"),
                }
            }
            shutdown_result = shutdown.changed() => {
                if shutdown_result.is_err() || *shutdown.borrow() {
                    queue_task.abort();
                    let _ = queue_task.await;
                    return;
                }
            }
        }
    }
}

async fn consume_asset_deletion_events(
    dependencies: Arc<PlatformDependencies>,
    mut shutdown: watch::Receiver<bool>,
) {
    let stream = loop {
        match dependencies
            .jetstream
            .get_stream("ASSET_STORAGE_EVENTS")
            .await
        {
            Ok(stream) => break stream,
            Err(error) => {
                tracing::error!(error_type=%std::any::type_name_of_val(&error), "Asset deletion worker cannot find its JetStream stream");
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(3)) => {}
                    result = shutdown.changed() => if result.is_err() || *shutdown.borrow() { return; }
                }
            }
        }
    };
    let consumer = loop {
        match stream
            .get_or_create_consumer(
                "xovyra-asset-deletion-jobs",
                Config {
                    durable_name: Some("xovyra-asset-deletion-jobs".to_owned()),
                    ack_policy: jetstream::consumer::AckPolicy::Explicit,
                    filter_subject: "asset_storage.asset.deletion_requested.v1".to_owned(),
                    max_deliver: 10,
                    ..Default::default()
                },
            )
            .await
        {
            Ok(consumer) => break consumer,
            Err(error) => {
                tracing::error!(error_type=%std::any::type_name_of_val(&error), "Asset deletion worker could not create its durable consumer");
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(3)) => {}
                    result = shutdown.changed() => if result.is_err() || *shutdown.borrow() { return; }
                }
            }
        }
    };
    let mut messages = match consumer.messages().await {
        Ok(messages) => messages,
        Err(error) => {
            tracing::error!(error_type=%std::any::type_name_of_val(&error), "Asset deletion worker could not read durable messages");
            return;
        }
    };
    loop {
        tokio::select! {
            item = messages.next() => match item {
                Some(Ok(message)) => {
                    let event: Result<Value, _> = serde_json::from_slice(&message.payload);
                    let handled = match event {
                        Ok(event) => enqueue_asset_event(&dependencies, &event).await,
                        Err(error) => {
                            tracing::error!(error_type=%std::any::type_name_of_val(&error), "Asset deletion event is malformed and will be acknowledged as terminal");
                            Ok(())
                        }
                    };
                    match handled {
                        Ok(()) => {
                            if let Err(error) = message.ack().await {
                                tracing::warn!(error_type=%std::any::type_name_of_val(&error), "Asset deletion event acknowledgement failed");
                            }
                        }
                        Err(error) => {
                            tracing::error!(error_type=%std::any::type_name_of_val(&error), "Asset deletion event could not be queued");
                            if let Err(ack_error) = message.ack_with(AckKind::Nak(Some(Duration::from_secs(5)))).await {
                                tracing::warn!(error_type=%std::any::type_name_of_val(&ack_error), "Asset deletion event retry request failed");
                            }
                        }
                    }
                }
                Some(Err(error)) => tracing::warn!(error_type=%std::any::type_name_of_val(&error), "Asset deletion event stream failed"),
                None => tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(2)) => {}
                    result = shutdown.changed() => if result.is_err() || *shutdown.borrow() { return; }
                }
            },
            result = shutdown.changed() => if result.is_err() || *shutdown.borrow() { return; }
        }
    }
}

async fn enqueue_asset_event(
    dependencies: &PlatformDependencies,
    event: &Value,
) -> Result<(), xovyra_jobs::jobs::JobError> {
    let event_id = event
        .get("event_id")
        .and_then(Value::as_str)
        .and_then(|value| uuid::Uuid::parse_str(value).ok())
        .ok_or(xovyra_jobs::jobs::JobError::InvalidInput)?;
    let workspace_id = event
        .pointer("/payload/workspace_id")
        .and_then(Value::as_str)
        .and_then(|value| uuid::Uuid::parse_str(value).ok())
        .ok_or(xovyra_jobs::jobs::JobError::InvalidInput)?;
    let owner_user_id = event
        .pointer("/payload/owner_user_id")
        .and_then(Value::as_str)
        .and_then(|value| uuid::Uuid::parse_str(value).ok())
        .ok_or(xovyra_jobs::jobs::JobError::InvalidInput)?;
    let asset_id = event
        .pointer("/payload/asset_id")
        .and_then(Value::as_str)
        .and_then(|value| uuid::Uuid::parse_str(value).ok())
        .ok_or(xovyra_jobs::jobs::JobError::InvalidInput)?;
    let correlation_id = event
        .pointer("/payload/correlation_id")
        .and_then(Value::as_str)
        .ok_or(xovyra_jobs::jobs::JobError::InvalidInput)?;
    jobs::enqueue_asset_deletion(
        &dependencies.jobs_database,
        EnqueueAssetDeletion {
            workspace_id,
            owner_user_id,
            asset_id,
            event_id,
            correlation_id,
        },
    )
    .await?;
    Ok(())
}

async fn execute_asset_deletion(
    dependencies: &PlatformDependencies,
    storage: &R2Storage,
    scratch: &ScratchSpace,
    runtime_endpoint: &str,
    worker_auth_secret: &str,
    worker_id: &str,
    job: jobs::Job,
) -> Result<(), WorkerError> {
    scratch
        .create_job_directory(job.job_id)
        .map_err(WorkerError::Scratch)?;
    if job.job_type != "asset.delete" {
        return Err(WorkerError::UnsupportedJob);
    }
    let mut asset_client = AssetStorageClient::connect(runtime_endpoint.to_owned())
        .await
        .map_err(WorkerError::Transport)?;
    let mut target_request = Request::new(GetAssetDeletionTargetRequest {
        workspace_id: job.workspace_id.to_string(),
        asset_id: job.resource_id.to_string(),
    });
    crate::worker_auth::attach(&mut target_request, worker_auth_secret)
        .map_err(WorkerError::InternalAuthorization)?;
    let target = asset_client
        .get_asset_deletion_target(target_request)
        .await
        .map_err(WorkerError::Status)?
        .into_inner();
    if target.lifecycle_state == "delete_pending" {
        storage
            .delete_object(&target.object_key)
            .await
            .map_err(WorkerError::Storage)?;
        storage
            .delete_object(&target.upload_object_key)
            .await
            .map_err(WorkerError::Storage)?;
        let mut completion_request = Request::new(CompleteAssetDeletionRequest {
            workspace_id: job.workspace_id.to_string(),
            asset_id: job.resource_id.to_string(),
            correlation_id: job.job_id.to_string(),
        });
        crate::worker_auth::attach(&mut completion_request, worker_auth_secret)
            .map_err(WorkerError::InternalAuthorization)?;
        asset_client
            .complete_asset_deletion(completion_request)
            .await
            .map_err(WorkerError::Status)?;
    }
    jobs::complete_job(
        &dependencies.jobs_database,
        job.job_id,
        worker_id,
        &job.job_id.to_string(),
    )
    .await
    .map_err(WorkerError::Job)?;
    if let Err(error) = scratch.remove_job_directory(job.job_id) {
        tracing::warn!(error_type=%std::any::type_name_of_val(&error), job_id=%job.job_id, "Completed job scratch will be removed by the retention worker");
    }
    Ok(())
}

#[derive(Debug, thiserror::Error)]
enum WorkerError {
    #[error("job queue operation failed")]
    Job(#[source] xovyra_jobs::jobs::JobError),
    #[error("internal asset storage endpoint is unavailable")]
    Transport(#[source] tonic::transport::Error),
    #[error("asset storage rejected an internal deletion request")]
    Status(#[source] tonic::Status),
    #[error("Cloudflare R2 deletion failed")]
    Storage(String),
    #[error("tenant job type is unsupported")]
    UnsupportedJob,
    #[error("job scratch area is unavailable")]
    Scratch(#[source] xovyra_jobs::scratch::ScratchError),
    #[error("internal worker authorization could not be constructed")]
    InternalAuthorization(#[source] tonic::Status),
}
