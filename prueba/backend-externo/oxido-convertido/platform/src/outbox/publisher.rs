use std::future::Future;
use std::pin::Pin;

use async_nats::HeaderMap;
use async_nats::jetstream;

#[derive(Debug, thiserror::Error)]
#[error("integration event publication was not confirmed")]
pub struct EventPublishError;

pub trait EventPublisher: Send + Sync {
    fn publish<'a>(
        &'a self,
        subject: &'a str,
        headers: HeaderMap,
        payload: Vec<u8>,
    ) -> Pin<Box<dyn Future<Output = Result<(), EventPublishError>> + Send + 'a>>;
}

pub struct JetStreamEventPublisher {
    jetstream: jetstream::Context,
}

impl JetStreamEventPublisher {
    pub fn new(jetstream: jetstream::Context) -> Self {
        Self { jetstream }
    }
}

impl EventPublisher for JetStreamEventPublisher {
    fn publish<'a>(
        &'a self,
        subject: &'a str,
        headers: HeaderMap,
        payload: Vec<u8>,
    ) -> Pin<Box<dyn Future<Output = Result<(), EventPublishError>> + Send + 'a>> {
        Box::pin(async move {
            let acknowledgement = self
                .jetstream
                .publish_with_headers(subject.to_owned(), headers, payload.into())
                .await
                .map_err(|error| {
                    tracing::warn!(error_type = %std::any::type_name_of_val(&error), "JetStream rejected event publication");
                    EventPublishError
                })?;
            acknowledgement.await.map_err(|error| {
                tracing::warn!(error_type = %std::any::type_name_of_val(&error), "JetStream did not confirm event persistence");
                EventPublishError
            })?;
            Ok(())
        })
    }
}
