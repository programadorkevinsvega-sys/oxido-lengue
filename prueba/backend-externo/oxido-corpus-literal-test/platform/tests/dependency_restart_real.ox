sintaxis llaves
use std::{error::Error, time::Duration};

use async_nats::jetstream::{self, consumer::pull::Config, stream::Config as StreamConfig};
use futures_util::StreamExt;
use tokio::process::Command;
use uuid::Uuid;

const RESTART_TIMEOUT: Duration = Duration::from_secs(90);

#[tokio::test]
async fn redis_and_jetstream_clients_recover_after_restarting_configured_dev_services()
-> Result<(), Box<dyn Error + Send + Sync>> {
    xovyra_platform::configuration::load_environment_file()?;
    let redis_url = required_environment("REDIS_URL")?;
    let nats_url = required_environment("NATS_URL")?;
    let nats_username = required_environment("NATS_USERNAME")?;
    let nats_password = required_environment("NATS_PASSWORD")?;
    let redis_container = required_environment("REDIS_CONTAINER")?;
    let nats_container = required_environment("NATS_CONTAINER")?;
    validate_container_name(&redis_container)?;
    validate_container_name(&nats_container)?;

    let redis_client = redis::Client::open(redis_url)?;
    let redis_configuration = redis::aio::ConnectionManagerConfig::new()
        .set_min_delay(Duration::from_millis(50))
        .set_max_delay(Duration::from_secs(1))
        .set_number_of_retries(120);
    let mut cache = redis_client
        .get_connection_manager_with_config(redis_configuration)
        .await?;
    let unique_id = Uuid::new_v4().simple().to_string();
    let redis_key = format!("xovyra:integration:restart:{unique_id}");
    set_expiring_value(&mut cache, &redis_key, "before-restart").await?;
    restart_container(&redis_container).await?;
    wait_for_redis(&mut cache).await?;
    set_expiring_value(&mut cache, &redis_key, "after-restart").await?;
    let redis_value: String = redis::cmd("GET")
        .arg(&redis_key)
        .query_async(&mut cache)
        .await?;
    if redis_value != "after-restart" {
        return Err(
            "Redis did not preserve the active client's ability to write after restart".into(),
        );
    }
    let _: i64 = redis::cmd("DEL")
        .arg(&redis_key)
        .query_async(&mut cache)
        .await?;

    let messaging =
        async_nats::ConnectOptions::with_user_and_password(nats_username, nats_password)
            .max_reconnects(None)
            .connect(nats_url)
            .await?;
    let jetstream = jetstream::new(messaging.clone());
    let stream_name = format!("XOVYRA_RESTART_{unique_id}");
    let subject = format!("xovyra.integration.restart.{unique_id}");
    let durable_name = format!("xovyra_restart_{unique_id}");
    let event_id = Uuid::new_v4().to_string();
    let payload = format!("{{\"event_id\":\"{event_id}\"}}");
    let stream = jetstream
        .create_stream(StreamConfig {
            name: stream_name.clone(),
            subjects: vec![subject.clone()],
            storage: jetstream::stream::StorageType::File,
            max_age: Duration::from_secs(3600),
            ..Default::default()
        })
        .await?;
    let verification = async {
        stream
            .create_consumer(Config {
                durable_name: Some(durable_name.clone()),
                ack_policy: jetstream::consumer::AckPolicy::Explicit,
                ack_wait: Duration::from_secs(3),
                filter_subject: subject.clone(),
                ..Default::default()
            })
            .await?;
        jetstream
            .publish(subject.clone(), payload.clone().into())
            .await?
            .await?;

        restart_container(&nats_container).await?;
        wait_for_nats(&messaging).await?;

        let recovered_stream = jetstream.get_stream(&stream_name).await?;
        let recovered_consumer = recovered_stream
            .get_consumer::<jetstream::consumer::pull::Config>(&durable_name)
            .await?;
        let mut messages = recovered_consumer.messages().await?;
        let recovered_message = tokio::time::timeout(RESTART_TIMEOUT, messages.next())
            .await?
            .ok_or("JetStream did not redeliver the durable event after restart")??;
        if recovered_message.payload.as_ref() != payload.as_bytes() {
            return Err("JetStream returned a different event after restart".into());
        }
        recovered_message.ack().await?;

        let next_event_id = Uuid::new_v4().to_string();
        let next_payload = format!("{{\"event_id\":\"{next_event_id}\"}}");
        jetstream
            .publish(subject.clone(), next_payload.clone().into())
            .await?
            .await?;
        let next_message = tokio::time::timeout(RESTART_TIMEOUT, messages.next())
            .await?
            .ok_or("JetStream stopped delivering after reconnect")??;
        if next_message.payload.as_ref() != next_payload.as_bytes() {
            return Err("JetStream delivered an unexpected post-restart event".into());
        }
        next_message.ack().await?;
        Ok::<(), Box<dyn Error + Send + Sync>>(())
    }
    .await;
    let cleanup = jetstream.delete_stream(&stream_name).await;
    verification?;
    cleanup?;
    messaging.flush().await?;
    Ok(())
}

async fn set_expiring_value(
    connection: &mut redis::aio::ConnectionManager,
    key: &str,
    value: &str,
) -> Result<(), redis::RedisError> {
    let _: String = redis::cmd("SET")
        .arg(key)
        .arg(value)
        .arg("EX")
        .arg(300)
        .query_async(connection)
        .await?;
    Ok(())
}

async fn wait_for_redis(
    connection: &mut redis::aio::ConnectionManager,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    tokio::time::timeout(RESTART_TIMEOUT, async {
        loop {
            let ping: redis::RedisResult<String> = redis::cmd("PING").query_async(connection).await;
            if ping.is_ok_and(|response| response == "PONG") {
                return;
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    })
    .await?;
    Ok(())
}

async fn wait_for_nats(messaging: &async_nats::Client) -> Result<(), Box<dyn Error + Send + Sync>> {
    tokio::time::timeout(RESTART_TIMEOUT, async {
        loop {
            if messaging.flush().await.is_ok() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    })
    .await?;
    Ok(())
}

async fn restart_container(container_name: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
    let output = Command::new("docker")
        .arg("restart")
        .arg("--time")
        .arg("15")
        .arg(container_name)
        .output()
        .await?;
    if !output.status.success() {
        return Err(format!(
            "Docker could not restart the configured development service {container_name}"
        )
        .into());
    }
    Ok(())
}

fn required_environment(key: &'static str) -> Result<String, Box<dyn Error + Send + Sync>> {
    let value = std::env::var(key)?;
    if value.trim().is_empty() {
        return Err(format!("{key} must be configured for the real restart test").into());
    }
    Ok(value)
}

fn validate_container_name(value: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
    if value
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.'))
    {
        return Ok(());
    }
    Err("Configured Docker container name contains unsupported characters".into())
}
