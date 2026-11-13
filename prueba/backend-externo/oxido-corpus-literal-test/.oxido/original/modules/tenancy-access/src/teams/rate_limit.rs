use std::time::Duration;

use redis::aio::ConnectionManager;
use uuid::Uuid;

use super::models::TeamError;
use super::token::InvitationTokenSigner;

const ACTOR_LIMIT: i64 = 5;
const ACTOR_WINDOW_SECONDS: i64 = 900;
const WORKSPACE_LIMIT: i64 = 20;
const WORKSPACE_WINDOW_SECONDS: i64 = 86_400;
const RECIPIENT_LIMIT: i64 = 3;
const RECIPIENT_WINDOW_SECONDS: i64 = 86_400;

pub async fn enforce_invitation_rate(
    cache: &mut ConnectionManager,
    signer: &InvitationTokenSigner,
    workspace_id: Uuid,
    actor_user_id: Uuid,
    normalized_email: &str,
) -> Result<(), TeamError> {
    let recipient_bucket = signer
        .email_bucket(normalized_email)
        .map_err(|_| TeamError::InvalidInput)?;
    let actor_key = format!("xovyra:tenancy:invite:actor:{workspace_id}:{actor_user_id}");
    let workspace_key = format!("xovyra:tenancy:invite:workspace:{workspace_id}");
    let recipient_key = format!("xovyra:tenancy:invite:recipient:{recipient_bucket}");
    let (actor_count, workspace_count, recipient_count, actor_ttl, workspace_ttl, recipient_ttl): (
        i64,
        i64,
        i64,
        i64,
        i64,
        i64,
    ) = tokio::time::timeout(
        Duration::from_secs(2),
        redis::Script::new("local a=redis.call('INCR',KEYS[1]); if a==1 then redis.call('EXPIRE',KEYS[1],ARGV[1]); end; local w=redis.call('INCR',KEYS[2]); if w==1 then redis.call('EXPIRE',KEYS[2],ARGV[2]); end; local r=redis.call('INCR',KEYS[3]); if r==1 then redis.call('EXPIRE',KEYS[3],ARGV[3]); end; return {a,w,r,redis.call('TTL',KEYS[1]),redis.call('TTL',KEYS[2]),redis.call('TTL',KEYS[3])}")
            .key(actor_key)
            .key(workspace_key)
            .key(recipient_key)
            .arg(ACTOR_WINDOW_SECONDS)
            .arg(WORKSPACE_WINDOW_SECONDS)
            .arg(RECIPIENT_WINDOW_SECONDS)
            .invoke_async(cache),
    )
    .await
    .map_err(|_| TeamError::RateLimitUnavailable(redis::RedisError::from(
        (redis::ErrorKind::InvalidClientConfig, "invitation rate limit timed out"),
    )))?
    .map_err(TeamError::RateLimitUnavailable)?;
    let retry_after_seconds = [
        (actor_count > ACTOR_LIMIT, actor_ttl),
        (workspace_count > WORKSPACE_LIMIT, workspace_ttl),
        (recipient_count > RECIPIENT_LIMIT, recipient_ttl),
    ]
    .into_iter()
    .filter_map(|(exceeded, ttl)| exceeded.then_some(ttl.max(1) as u64))
    .max();
    if let Some(retry_after_seconds) = retry_after_seconds {
        return Err(TeamError::RateLimited {
            retry_after_seconds,
        });
    }
    Ok(())
}
