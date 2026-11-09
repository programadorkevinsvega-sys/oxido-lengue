sintaxis llaves
#[path = "security_real/replica_services.ox"]
mod replica_services;
#[path = "security_real/services.ox"]
mod services;

use std::{error::Error, net::Ipv6Addr, time::Duration};

use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;
use xovyra_identity::authentication::{
    Audience, IdentityService, PendingVerification, RegistrationResult,
};
use xovyra_tenancy_access::workspaces::{self, WorkspaceCreation};

struct TestAccount {
    user_id: Uuid,
    email: String,
    session_secret: String,
    csrf_secret: String,
    session_id: Uuid,
}

#[tokio::test]
async fn authenticated_tenant_requests_are_isolated_across_two_bff_and_runtime_replicas()
-> Result<(), Box<dyn Error + Send + Sync>> {
    xovyra_platform::configuration::load_environment_file()?;
    let identity_database_url = std::env::var("IDENTITY_DATABASE_URL")?;
    let tenancy_database_url = std::env::var("TENANCY_ACCESS_DATABASE_URL")?;
    let asset_database_url = std::env::var("ASSET_STORAGE_DATABASE_URL")?;
    let cleanup_database_url = std::env::var("MIGRATOR_DATABASE_URL")?;
    let redis_url = std::env::var("REDIS_URL")?;
    let identity_database = PgPoolOptions::new()
        .max_connections(5)
        .connect(&identity_database_url)
        .await?;
    let tenancy_database = PgPoolOptions::new()
        .max_connections(5)
        .connect(&tenancy_database_url)
        .await?;
    let asset_database = PgPoolOptions::new()
        .max_connections(5)
        .connect(&asset_database_url)
        .await?;
    let cleanup_database = PgPoolOptions::new()
        .max_connections(3)
        .connect(&cleanup_database_url)
        .await?;
    let cache = redis::Client::open(redis_url)?
        .get_connection_manager()
        .await?;
    let identity = IdentityService::new(
        identity_database.clone(),
        cache,
        Uuid::new_v4().to_string(),
        Duration::from_secs(1_800),
        Duration::from_secs(86_400),
    )?;
    let mut user_ids = Vec::new();
    let mut workspace_ids = Vec::new();
    let mut asset_ids = Vec::new();
    let verification = async {
        let first_account = create_verified_account(&identity).await?;
        user_ids.push(first_account.user_id);
        let second_account = create_verified_account(&identity).await?;
        user_ids.push(second_account.user_id);
        let first_workspace = create_workspace(
            &tenancy_database,
            &first_account,
            "Replica isolation workspace one",
        )
        .await?;
        workspace_ids.push(first_workspace);
        let second_workspace = create_workspace(
            &tenancy_database,
            &second_account,
            "Replica isolation workspace two",
        )
        .await?;
        workspace_ids.push(second_workspace);
        let first_asset =
            create_pending_asset(&asset_database, first_workspace, &first_account).await?;
        asset_ids.push(first_asset);
        let replicas = replica_services::start().await?;
        let endpoints = replicas.endpoints();
        for endpoint in endpoints {
            verify_workspace_scope(endpoint, &first_account, first_workspace, second_workspace)
                .await?;
            verify_workspace_scope(endpoint, &second_account, second_workspace, first_workspace)
                .await?;
        }
        let denied_delete = reqwest::Client::new()
            .delete(format!("{}/api/v1/assets/{first_asset}", endpoints[1]))
            .header(reqwest::header::ORIGIN, "http://127.0.0.1:5173")
            .header(reqwest::header::COOKIE, tenant_cookie(&second_account))
            .header("x-csrf-token", &second_account.csrf_secret)
            .send()
            .await?;
        if denied_delete.status() != reqwest::StatusCode::NOT_FOUND {
            return Err(format!(
                "a second tenant received status {} when deleting another tenant's asset",
                denied_delete.status()
            )
            .into());
        }
        Ok::<(), Box<dyn Error + Send + Sync>>(())
    }
    .await;
    let cleanup =
        cleanup_transient_records(&cleanup_database, &user_ids, &workspace_ids, &asset_ids).await;
    drop(identity);
    identity_database.close().await;
    tenancy_database.close().await;
    asset_database.close().await;
    cleanup_database.close().await;
    verification?;
    cleanup?;
    Ok(())
}

async fn create_verified_account(
    identity: &IdentityService,
) -> Result<TestAccount, Box<dyn Error + Send + Sync>> {
    let unique_id = Uuid::new_v4();
    let email = format!("phase44-{unique_id}@integration.invalid");
    let password = format!("Xovyra-Phase44-{unique_id}-Aa9!");
    let source_address = Ipv6Addr::from(*Uuid::new_v4().as_bytes()).to_string();
    let correlation_id = Uuid::new_v4().to_string();
    let registration = identity
        .register(
            &email,
            &password,
            "Replica integration account",
            false,
            &source_address,
            &correlation_id,
        )
        .await?;
    let PendingVerification { user_id, token, .. } = match registration {
        RegistrationResult::Created(pending) => pending,
        RegistrationResult::AlreadyExists => {
            return Err("a newly generated integration account already exists".into());
        }
    };
    let verified = identity
        .verify_email(&token, "", "", &source_address, &correlation_id)
        .await?;
    if verified.is_none() {
        return Err("the application did not verify its generated integration account".into());
    }
    let session = identity
        .login(
            &email,
            &password,
            Audience::Tenant,
            &source_address,
            &correlation_id,
        )
        .await?;
    Ok(TestAccount {
        user_id,
        email,
        session_secret: session.session_secret,
        csrf_secret: session.csrf_secret,
        session_id: session.session_id,
    })
}

async fn create_workspace(
    database: &sqlx::PgPool,
    account: &TestAccount,
    name: &str,
) -> Result<Uuid, Box<dyn Error + Send + Sync>> {
    let workspace = workspaces::create_workspace(
        database,
        WorkspaceCreation {
            user_id: account.user_id,
            session_id: account.session_id,
            name,
            workspace_type: "team",
            idempotency_key: &Uuid::new_v4().to_string(),
            correlation_id: &Uuid::new_v4().to_string(),
            owner_email: &account.email,
        },
    )
    .await?;
    Ok(workspace.workspace_id)
}

async fn create_pending_asset(
    database: &sqlx::PgPool,
    workspace_id: Uuid,
    account: &TestAccount,
) -> Result<Uuid, Box<dyn Error + Send + Sync>> {
    let asset = xovyra_asset_storage::assets::create_pending_asset(
        database,
        xovyra_asset_storage::assets::PendingAsset {
            workspace_id,
            owner_user_id: account.user_id,
            display_name: "replica-isolation-upload.png",
            content_type: "image/png",
            size_bytes: 128,
            idempotency_key: &Uuid::new_v4().to_string(),
            correlation_id: &Uuid::new_v4().to_string(),
        },
    )
    .await?;
    Ok(asset.asset_id)
}

async fn verify_workspace_scope(
    endpoint: &str,
    account: &TestAccount,
    owned_workspace_id: Uuid,
    foreign_workspace_id: Uuid,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let response = reqwest::Client::new()
        .get(format!("{endpoint}/api/v1/workspaces"))
        .header(reqwest::header::ORIGIN, "http://127.0.0.1:5173")
        .header(reqwest::header::COOKIE, tenant_cookie(account))
        .send()
        .await?;
    if !response.status().is_success() {
        return Err(format!("workspace list returned {}", response.status()).into());
    }
    let body: serde_json::Value = response.json().await?;
    let workspaces = body
        .get("workspaces")
        .and_then(serde_json::Value::as_array)
        .ok_or("workspace list did not return its typed collection")?;
    let owns_workspace = workspaces.iter().any(|workspace| {
        workspace
            .get("workspace_id")
            .and_then(serde_json::Value::as_str)
            == Some(owned_workspace_id.to_string().as_str())
    });
    let sees_foreign_workspace = workspaces.iter().any(|workspace| {
        workspace
            .get("workspace_id")
            .and_then(serde_json::Value::as_str)
            == Some(foreign_workspace_id.to_string().as_str())
    });
    if !owns_workspace || sees_foreign_workspace {
        return Err("BFF replica returned a workspace outside the account's membership".into());
    }
    Ok(())
}

fn tenant_cookie(account: &TestAccount) -> String {
    format!(
        "xovyra_tenant_session={}; xovyra_tenant_csrf={}",
        account.session_secret, account.csrf_secret
    )
}

async fn cleanup_transient_records(
    database: &sqlx::PgPool,
    user_ids: &[Uuid],
    workspace_ids: &[Uuid],
    asset_ids: &[Uuid],
) -> Result<(), sqlx::Error> {
    if !workspace_ids.is_empty() {
        let workspace_text = workspace_ids
            .iter()
            .map(Uuid::to_string)
            .collect::<Vec<_>>();
        sqlx::query(
            "DELETE FROM asset_storage.outbox_events WHERE payload->>'workspace_id'=ANY($1)",
        )
        .bind(&workspace_text)
        .execute(database)
        .await?;
        sqlx::query("DELETE FROM asset_storage.assets WHERE asset_id=ANY($1)")
            .bind(asset_ids)
            .execute(database)
            .await?;
        sqlx::query(
            "DELETE FROM tenancy_access.outbox_events WHERE payload->>'workspace_id'=ANY($1)",
        )
        .bind(&workspace_text)
        .execute(database)
        .await?;
        sqlx::query("DELETE FROM tenancy_access.workspace_audit_events WHERE workspace_id=ANY($1)")
            .bind(workspace_ids)
            .execute(database)
            .await?;
        sqlx::query(
            "DELETE FROM tenancy_access.workspace_creation_idempotency WHERE user_id=ANY($1)",
        )
        .bind(user_ids)
        .execute(database)
        .await?;
        sqlx::query("DELETE FROM tenancy_access.workspaces WHERE workspace_id=ANY($1)")
            .bind(workspace_ids)
            .execute(database)
            .await?;
    }
    if !user_ids.is_empty() {
        sqlx::query("DELETE FROM identity.users WHERE user_id=ANY($1)")
            .bind(user_ids)
            .execute(database)
            .await?;
    }
    Ok(())
}
