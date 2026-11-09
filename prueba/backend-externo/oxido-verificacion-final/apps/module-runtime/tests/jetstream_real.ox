sintaxis llaves
use std::error::Error;
use std::time::Duration;

use async_nats::jetstream;
use async_nats::jetstream::consumer::pull::Config;
use async_nats::jetstream::stream::Config as StreamConfig;
use futures_util::StreamExt;
use uuid::Uuid;

#[tokio::test]
async fn jetstream_persists_delivers_acknowledges_and_replays_real_events()
-> Result<(), Box<dyn Error + Send + Sync>> {
    xovyra_platform::configuration::load_environment_file()?;
    let nats_url = std::env::var("NATS_URL")?;
    let nats_username = std::env::var("NATS_USERNAME")?;
    let nats_password = std::env::var("NATS_PASSWORD")?;
    let messaging =
        async_nats::ConnectOptions::with_user_and_password(nats_username, nats_password)
            .connect(nats_url)
            .await
            .map_err(|error| verification_error("connecting client", error))?;
    let jetstream = jetstream::new(messaging.clone());
    let unique_id = Uuid::new_v4().simple().to_string();
    let stream_name = format!("XOVYRA_VERIFY_{unique_id}");
    let subject = format!("xovyra.verification.{unique_id}");
    let payload = format!("{{\"event_id\":\"{unique_id}\",\"version\":1}}");
    let stream = jetstream
        .create_stream(StreamConfig {
            name: stream_name.clone(),
            subjects: vec![subject.clone()],
            ..Default::default()
        })
        .await
        .map_err(|error| verification_error("creating stream", error))?;

    let verification =
        verify_delivery_and_replay(&jetstream, &stream, &subject, payload.as_bytes()).await;
    let cleanup = jetstream.delete_stream(&stream_name).await;
    verification?;
    cleanup?;
    messaging.flush().await?;
    Ok(())
}

async fn verify_delivery_and_replay(
    jetstream: &jetstream::Context,
    stream: &jetstream::stream::Stream,
    subject: &str,
    payload: &[u8],
) -> Result<(), Box<dyn Error + Send + Sync>> {
    jetstream
        .publish(subject.to_owned(), payload.to_vec().into())
        .await
        .map_err(|error| verification_error("publishing event", error))?
        .await
        .map_err(|error| verification_error("persisting event", error))?;
    let first_consumer = stream
        .create_consumer(Config {
            ack_policy: jetstream::consumer::AckPolicy::Explicit,
            ..Default::default()
        })
        .await
        .map_err(|error| verification_error("creating delivery consumer", error))?;
    let mut first_messages = first_consumer
        .messages()
        .await
        .map_err(|error| verification_error("opening delivery stream", error))?;
    let first_message = tokio::time::timeout(Duration::from_secs(5), first_messages.next())
        .await?
        .ok_or("JetStream did not deliver the persisted event")??;
    if first_message.payload.as_ref() != payload {
        return Err("JetStream delivered an unexpected event payload".into());
    }
    first_message
        .ack()
        .await
        .map_err(|error| verification_error("acknowledging event", error))?;
    drop(first_messages);

    let replay_consumer = stream
        .create_consumer(Config {
            ack_policy: jetstream::consumer::AckPolicy::Explicit,
            ..Default::default()
        })
        .await
        .map_err(|error| verification_error("creating replay consumer", error))?;
    let mut replay_messages = replay_consumer
        .messages()
        .await
        .map_err(|error| verification_error("opening replay stream", error))?;
    let replayed_message = tokio::time::timeout(Duration::from_secs(5), replay_messages.next())
        .await?
        .ok_or("JetStream did not replay the persisted event")??;
    if replayed_message.payload.as_ref() != payload {
        return Err("JetStream replayed an unexpected event payload".into());
    }
    replayed_message
        .ack()
        .await
        .map_err(|error| verification_error("acknowledging replay", error))?;
    Ok(())
}

fn verification_error(stage: &str, error: impl std::fmt::Display) -> std::io::Error {
    std::io::Error::other(format!("JetStream {stage} failed: {error}"))
}
