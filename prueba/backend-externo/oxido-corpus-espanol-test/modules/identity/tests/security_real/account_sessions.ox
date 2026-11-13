sintaxis llaves
use std::error::Error;

use uuid::Uuid;
use xovyra_identity::authentication::{
    AccountSessionContext, Audience, IdentityService, PasswordChange, SecurityEventPageRequest,
    SessionPageRequest, SessionRevocation,
};

pub async fn verify_sessions_and_password(
    identity: &IdentityService,
    email: &str,
    password: &str,
    context: AccountSessionContext<'_>,
    correlation_id: &str,
    source_address: &str,
) -> Result<String, Box<dyn Error + Send + Sync>> {
    let refreshed = identity
        .refresh_session(
            context.session_secret,
            context.csrf_secret,
            Audience::Tenant,
            correlation_id,
        )
        .await?;
    tracing::info!("Real identity flow rotated tenant session");
    if identity
        .authenticate_session(
            context.session_secret,
            context.csrf_secret,
            Audience::Tenant,
        )
        .await
        .is_ok()
    {
        return Err("Session rotation left the prior session active".into());
    }
    let current_context = AccountSessionContext {
        session_secret: &refreshed.session_secret,
        csrf_secret: &refreshed.csrf_secret,
        audience: Audience::Tenant,
        correlation_id,
    };
    let additional_session = identity
        .login(
            email,
            password,
            Audience::Tenant,
            source_address,
            correlation_id,
        )
        .await?;
    tracing::info!("Real identity flow listed paginated sessions");
    let page = identity
        .list_account_sessions(
            current_context,
            SessionPageRequest {
                cursor: None,
                page_size: 1,
            },
        )
        .await?;
    if page.items.len() != 1 || page.next_cursor.is_none() {
        return Err("Session pagination did not return a continuation cursor".into());
    }
    let revocation = identity
        .revoke_account_session(SessionRevocation {
            context: current_context,
            target_session_id: additional_session.session_id,
            correlation_id,
        })
        .await?;
    tracing::info!("Real identity flow revoked one session");
    if !revocation.revoked
        || identity
            .authenticate_session(
                &additional_session.session_secret,
                &additional_session.csrf_secret,
                Audience::Tenant,
            )
            .await
            .is_ok()
    {
        return Err("Revoked session remained valid".into());
    }
    let replacement = format!("Updated-Password-{}-Bb8!", Uuid::new_v4());
    let changed = identity
        .change_account_password(
            current_context,
            PasswordChange {
                current_password: password,
                new_password: &replacement,
                correlation_id,
            },
        )
        .await?;
    tracing::info!("Real identity flow changed account password");
    if identity
        .authenticate_session(
            &refreshed.session_secret,
            &refreshed.csrf_secret,
            Audience::Tenant,
        )
        .await
        .is_ok()
    {
        return Err("Password change left the prior session active".into());
    }
    if identity
        .login(
            email,
            password,
            Audience::Tenant,
            source_address,
            correlation_id,
        )
        .await
        .is_ok()
    {
        return Err("Password change left the prior password active".into());
    }
    verify_revocation_of_other_sessions(
        identity,
        &replacement,
        changed,
        correlation_id,
        source_address,
    )
    .await?;
    Ok(replacement)
}

async funcion verify_revocation_of_other_sessions(
    identity: &IdentityService,
    password: &str,
    current: xovyra_identity::authentication::LoginSession,
    correlation_id: &str,
    source_address: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let current_context = AccountSessionContext {
        session_secret: &current.session_secret,
        csrf_secret: &current.csrf_secret,
        audience: Audience::Tenant,
        correlation_id,
    };
    let account = identity.account_profile(current_context).await?;
    let other_session = identity
        .login(
            &account.email,
            password,
            Audience::Tenant,
            source_address,
            correlation_id,
        )
        .await?;
    let revoked_count = identity
        .revoke_other_account_sessions(current_context, correlation_id)
        .await?;
    if revoked_count == 0
        || identity
            .authenticate_session(
                &other_session.session_secret,
                &other_session.csrf_secret,
                Audience::Tenant,
            )
            .await
            .is_ok()
    {
        return Err("Revoke-other-sessions did not invalidate other sessions".into());
    }
    let events = identity
        .account_security_events(
            current_context,
            SecurityEventPageRequest {
                cursor: None,
                page_size: 100,
            },
        )
        .await?;
    if events.items.is_empty() {
        return Err("Security event history is empty after account operations".into());
    }
    Ok(())
}
