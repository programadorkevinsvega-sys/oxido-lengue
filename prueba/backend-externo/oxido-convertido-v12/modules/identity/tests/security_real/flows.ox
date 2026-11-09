sintaxis llaves
use sqlx::PgPool;
use std::error::Error;
use uuid::Uuid;
use xovyra_identity::authentication::{
    AccountSessionContext, Audience, IdentityService, PendingVerification, ProfileUpdate,
    RegistrationResult,
};

#[path = "modules/identity/tests/security_real/bff_contract.ox"]
mod bff_contract;
#[path = "modules/identity/tests/security_real/delivery.ox"]
pub(super) mod delivery;

use bff_contract::verify_bff_grpc_profile;
#[path = "modules/identity/tests/security_real/account_sessions.ox"]
mod account_sessions;
#[path = "modules/identity/tests/security_real/operator.ox"]
mod operator;
#[path = "modules/identity/tests/security_real/platform_console.ox"]
mod platform_console;

pub async fn exercise(
    identity: &IdentityService,
    database: &PgPool,
    api_base_url: &str,
) -> Result<Uuid, Box<dyn Error + Send + Sync>> {
    let unique_id = Uuid::new_v4();
    let email = format!("phase3-{unique_id}@integration.invalid");
    let password = format!("Xovyra-Account-{unique_id}-Aa9!");
    let source_address = std::net::Ipv6Addr::from(*Uuid::new_v4().as_bytes()).to_string();
    let correlation_id = Uuid::new_v4().to_string();
    let registration = identity
        .register(
            &email,
            &password,
            "Integration Account",
            true,
            &source_address,
            &correlation_id,
        )
        .await?;
    let PendingVerification { user_id, token, .. } = match registration {
        RegistrationResult::Created(pending) => pending,
        RegistrationResult::AlreadyExists => {
            return Err("Generated integration account already exists".into());
        }
    };
    let result = account_flow(AccountFlow {
        identity,
        database,
        user_id,
        email: &email,
        password: &password,
        verification_token: &token,
        correlation_id: &correlation_id,
        api_base_url,
        source_address: &source_address,
    })
    .await;
    let cleanup = sqlx::query("DELETE FROM identity.users WHERE user_id = $1")
        .bind(user_id)
        .execute(database)
        .await;
    result?;
    cleanup?;
    Ok(user_id)
}

struct AccountFlow<'a> {
    identity: &'a IdentityService,
    database: &'a PgPool,
    user_id: Uuid,
    email: &'a str,
    password: &'a str,
    verification_token: &'a str,
    correlation_id: &'a str,
    api_base_url: &'a str,
    source_address: &'a str,
}

async fn account_flow(flow: AccountFlow<'_>) -> Result<(), Box<dyn Error + Send + Sync>> {
    let AccountFlow {
        identity,
        database,
        user_id,
        email,
        password,
        verification_token,
        correlation_id,
        api_base_url,
        source_address,
    } = flow;
    let activated = identity
        .verify_email(verification_token, "", "", "127.0.0.1", correlation_id)
        .await?;
    tracing::info!("Real identity flow verified email");
    if activated.is_none()
        || identity
            .verify_email(verification_token, "", "", "127.0.0.1", correlation_id)
            .await?
            .is_some()
    {
        return Err("Email verification was not single-use".into());
    }
    let login = identity
        .login(
            email,
            password,
            Audience::Tenant,
            source_address,
            correlation_id,
        )
        .await?;
    tracing::info!("Real identity flow created tenant session");
    let tenant_context = AccountSessionContext {
        session_secret: &login.session_secret,
        csrf_secret: &login.csrf_secret,
        audience: Audience::Tenant,
        correlation_id,
    };
    verify_bff_grpc_profile(
        api_base_url,
        &login.session_secret,
        &login.csrf_secret,
        email,
    )
    .await?;
    let profile = identity.account_profile(tenant_context).await?;
    if profile.user_id != user_id || !profile.email_verified {
        return Err("Profile read escaped the authenticated account".into());
    }
    update_profile(identity, tenant_context, correlation_id).await?;
    tracing::info!("Real identity flow updated profile");
    let updated_password = account_sessions::verify_sessions_and_password(
        identity,
        email,
        password,
        tenant_context,
        correlation_id,
        source_address,
    )
    .await?;
    tracing::info!("Real identity flow verified session and password security");
    operator::verify_operator_separation(
        identity,
        database,
        email,
        &updated_password,
        user_id,
        correlation_id,
        source_address,
    )
    .await?;
    tracing::info!("Real identity flow verified platform separation");
    platform_console::exercise(
        identity,
        database,
        api_base_url,
        email,
        &updated_password,
        user_id,
        source_address,
    )
    .await?;
    tracing::info!("Real BFF and gRPC platform console acceptance passed");
    Ok(())
}

async fn update_profile(
    identity: &IdentityService,
    context: AccountSessionContext<'_>,
    correlation_id: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let updated = identity
        .update_account_profile(
            context,
            ProfileUpdate {
                display_name: "Integration Account Updated",
                locale: "es-CO",
                timezone: "America/Bogota",
                correlation_id,
            },
        )
        .await?;
    if updated.display_name != "Integration Account Updated" {
        return Err("Profile update was not persisted".into());
    }
    Ok(())
}
