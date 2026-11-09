use std::error::Error;

use sqlx::PgPool;
use uuid::Uuid;
use xovyra_identity::authentication::platform_access::PlatformOperatorProvisioner;
use xovyra_identity::authentication::{AccountSessionContext, Audience, IdentityService};

pub async fn verify_operator_separation(
    identity: &IdentityService,
    database: &PgPool,
    email: &str,
    password: &str,
    user_id: Uuid,
    correlation_id: &str,
    source_address: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let provisioner = PlatformOperatorProvisioner::new(database.clone());
    let reason = format!("Phase 3 live integration validation {correlation_id}");
    let grant = provisioner
        .grant_role(email, "platform_admin", &reason, correlation_id)
        .await?;
    if !grant.completed {
        return Err("Verified account did not receive its explicit operator grant".into());
    }
    let tenant_session = identity
        .login(
            email,
            password,
            Audience::Tenant,
            source_address,
            correlation_id,
        )
        .await?;
    let platform_session = identity
        .login(
            email,
            password,
            Audience::Platform,
            source_address,
            correlation_id,
        )
        .await?;
    if identity
        .authenticate_session(
            &tenant_session.session_secret,
            &tenant_session.csrf_secret,
            Audience::Platform,
        )
        .await
        .is_ok()
    {
        return Err("Tenant session authenticated as platform operator".into());
    }
    let operator_context = AccountSessionContext {
        session_secret: &platform_session.session_secret,
        csrf_secret: &platform_session.csrf_secret,
        audience: Audience::Platform,
        correlation_id,
    };
    let permissions = identity.platform_permissions(operator_context).await?;
    if permissions.user_id != user_id
        || !permissions
            .permissions
            .iter()
            .any(|permission| permission == "platform.auth.read")
    {
        return Err("Operator permissions did not match the granted role".into());
    }
    if identity.account_profile(operator_context).await.is_ok() {
        return Err("Platform session entered the tenant account surface".into());
    }
    let revoke = provisioner
        .revoke_role(email, "platform_admin", &reason, correlation_id)
        .await?;
    if !revoke.completed
        || identity
            .authenticate_session(
                &platform_session.session_secret,
                &platform_session.csrf_secret,
                Audience::Platform,
            )
            .await
            .is_ok()
    {
        return Err("Revoked operator grant left platform access active".into());
    }
    Ok(())
}
