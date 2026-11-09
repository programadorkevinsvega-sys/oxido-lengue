sintaxis llaves
use std::error::Error;

use reqwest::{StatusCode, header};
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;
use xovyra_asset_storage::assets::{self, PendingAsset, UploadConfirmation};
use xovyra_identity::authentication::platform_access::PlatformOperatorProvisioner;
use xovyra_identity::authentication::{Audience, IdentityService};
use xovyra_tenancy_access::workspaces::{self, WorkspaceCreation};

const TENANT_ORIGIN: &str = "http://127.0.0.1:5173";
const PLATFORM_ORIGIN: &str = "http://127.0.0.1:5174";

pub async fn exercise(
    identity: &IdentityService,
    identity_database: &sqlx::PgPool,
    api_base_url: &str,
    email: &str,
    password: &str,
    user_id: Uuid,
    source_address: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let correlation_id = Uuid::new_v4().to_string();
    let tenant_login = identity
        .login(
            email,
            password,
            Audience::Tenant,
            source_address,
            &correlation_id,
        )
        .await?;
    let tenancy_database = PgPoolOptions::new()
        .max_connections(2)
        .connect(&std::env::var("TENANCY_ACCESS_DATABASE_URL")?)
        .await?;
    let asset_database = PgPoolOptions::new()
        .max_connections(2)
        .connect(&std::env::var("ASSET_STORAGE_DATABASE_URL")?)
        .await?;

    let workspace_name = format!("Phase 4.5 {user_id}");
    let workspace = workspaces::create_workspace(
        &tenancy_database,
        WorkspaceCreation {
            user_id,
            session_id: tenant_login.session_id,
            name: &workspace_name,
            workspace_type: "team",
            idempotency_key: &Uuid::new_v4().to_string(),
            correlation_id: &correlation_id,
            owner_email: email,
        },
    )
    .await?;
    let asset = assets::create_pending_asset(
        &asset_database,
        PendingAsset {
            workspace_id: workspace.workspace_id,
            owner_user_id: user_id,
            display_name: "phase-4-5-integration.png",
            content_type: "image/png",
            size_bytes: 16,
            idempotency_key: &Uuid::new_v4().to_string(),
            correlation_id: &correlation_id,
        },
    )
    .await?;
    assets::activate_uploaded_asset(
        &asset_database,
        UploadConfirmation {
            workspace_id: workspace.workspace_id,
            owner_user_id: user_id,
            asset_id: asset.asset_id,
            actual_size_bytes: 16,
            actual_content_type: "image/png",
            etag: "phase-4-5-integration-etag",
            correlation_id: &correlation_id,
        },
    )
    .await?;

    let provisioner = PlatformOperatorProvisioner::new(identity_database.clone());
    let reason = format!("Automated Phase 4.5 acceptance {correlation_id}");
    if !provisioner
        .grant_role(email, "platform_admin", &reason, &correlation_id)
        .await?
        .completed
    {
        return Err("Verified integration account could not receive the test operator role".into());
    }

    let acceptance: Result<(), Box<dyn Error + Send + Sync>> = async {
        let client = reqwest::Client::builder().build()?;
        let tenant_login_response = client
            .post(format!("{api_base_url}/api/v1/auth/login"))
            .header(header::ORIGIN, TENANT_ORIGIN)
            .json(&serde_json::json!({"email": email, "password": password}))
            .send()
            .await?;
        ensure_status(
            tenant_login_response.status(),
            StatusCode::OK,
            "tenant login",
        )?;
        let tenant_cookie = session_cookie(&tenant_login_response, "xovyra_tenant_session")?;
        let tenant_csrf = session_cookie(&tenant_login_response, "xovyra_tenant_csrf")?;

        let tenant_cross_audience = client
            .get(format!("{api_base_url}/platform/workspaces"))
            .header(header::ORIGIN, PLATFORM_ORIGIN)
            .header(header::COOKIE, format!("{tenant_cookie}; {tenant_csrf}"))
            .send()
            .await?;
        ensure_status(
            tenant_cross_audience.status(),
            StatusCode::UNAUTHORIZED,
            "tenant cookie rejected by platform routes",
        )?;

        let unauthenticated = client
            .get(format!("{api_base_url}/platform/workspaces"))
            .header(header::ORIGIN, PLATFORM_ORIGIN)
            .send()
            .await?;
        ensure_status(
            unauthenticated.status(),
            StatusCode::UNAUTHORIZED,
            "platform directory requires authentication",
        )?;

        let platform_login_response = client
            .post(format!("{api_base_url}/platform/auth/login"))
            .header(header::ORIGIN, PLATFORM_ORIGIN)
            .json(&serde_json::json!({"email": email, "password": password}))
            .send()
            .await?;
        ensure_status(
            platform_login_response.status(),
            StatusCode::OK,
            "platform login",
        )?;
        let platform_cookie = session_cookie(&platform_login_response, "xovyra_platform_session")?;
        let platform_csrf = session_cookie(&platform_login_response, "xovyra_platform_csrf")?;
        let cookies = format!("{platform_cookie}; {platform_csrf}");

        let session = client
            .get(format!("{api_base_url}/platform/auth/session"))
            .header(header::ORIGIN, PLATFORM_ORIGIN)
            .header(header::COOKIE, &cookies)
            .send()
            .await?;
        ensure_status(session.status(), StatusCode::OK, "platform session")?;
        let permissions = client
            .get(format!("{api_base_url}/platform/me/permissions"))
            .header(header::ORIGIN, PLATFORM_ORIGIN)
            .header(header::COOKIE, &cookies)
            .send()
            .await?;
        ensure_status(
            permissions.status(),
            StatusCode::OK,
            "platform operator permissions",
        )?;
        let permission_body: serde_json::Value = permissions.json().await?;
        if !permission_body
            .get("permissions")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|permissions| {
                [
                    "platform.organizations.read",
                    "platform.support.grants.manage",
                    "platform.support.content.read",
                ]
                .iter()
                .all(|required| permissions.iter().any(|permission| permission == required))
            })
        {
            return Err(
                "Platform operator permission response omitted a required permission".into(),
            );
        }

        let directory = client
            .get(format!(
                "{api_base_url}/platform/workspaces?search={user_id}"
            ))
            .header(header::ORIGIN, PLATFORM_ORIGIN)
            .header(header::COOKIE, &cookies)
            .send()
            .await?;
        let directory =
            ensure_response(directory, StatusCode::OK, "platform workspace directory").await?;
        let directory_body: serde_json::Value = directory.json().await?;
        if !directory_body
            .get("workspaces")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|items| {
                items.iter().any(|item| {
                    item.get("workspace_id").and_then(serde_json::Value::as_str)
                        == Some(workspace.workspace_id.to_string().as_str())
                })
            })
        {
            return Err("Platform directory did not return the created workspace".into());
        }

        for (path, collection, expected_id) in [
            (
                format!("/platform/workspaces/{}/members", workspace.workspace_id),
                "members",
                user_id,
            ),
            (
                format!("/platform/workspaces/{}/assets", workspace.workspace_id),
                "assets",
                asset.asset_id,
            ),
        ] {
            let response = client
                .get(format!("{api_base_url}{path}"))
                .header(header::ORIGIN, PLATFORM_ORIGIN)
                .header(header::COOKIE, &cookies)
                .send()
                .await?;
            ensure_status(response.status(), StatusCode::OK, collection)?;
            let body: serde_json::Value = response.json().await?;
            if !body
                .get(collection)
                .and_then(serde_json::Value::as_array)
                .is_some_and(|items| {
                    items.iter().any(|item| {
                        let key = if collection == "members" {
                            "user_id"
                        } else {
                            "asset_id"
                        };
                        item.get(key).and_then(serde_json::Value::as_str)
                            == Some(expected_id.to_string().as_str())
                    })
                })
            {
                return Err(
                    format!("Platform {collection} response omitted the created record").into(),
                );
            }
        }

        let no_csrf = client
            .post(format!("{api_base_url}/platform/support-grants"))
            .header(header::ORIGIN, PLATFORM_ORIGIN)
            .header(header::COOKIE, &cookies)
            .json(&grant_body(workspace.workspace_id, asset.asset_id))
            .send()
            .await?;
        ensure_status(
            no_csrf.status(),
            StatusCode::FORBIDDEN,
            "grant CSRF enforcement",
        )?;

        let create_grant = client
            .post(format!("{api_base_url}/platform/support-grants"))
            .header(header::ORIGIN, PLATFORM_ORIGIN)
            .header(header::COOKIE, &cookies)
            .header("x-csrf-token", platform_csrf_value(&platform_csrf)?)
            .json(&grant_body(workspace.workspace_id, asset.asset_id))
            .send()
            .await?;
        ensure_status(
            create_grant.status(),
            StatusCode::OK,
            "create support grant",
        )?;
        let grant_body: serde_json::Value = create_grant.json().await?;
        let grant_id = grant_body
            .get("grant")
            .and_then(|grant| grant.get("grant_id"))
            .or_else(|| grant_body.get("grant_id"))
            .and_then(serde_json::Value::as_str)
            .ok_or("Create grant response omitted grant_id")?
            .to_owned();

        let grants_response = client
            .get(format!("{api_base_url}/platform/support-grants"))
            .header(header::ORIGIN, PLATFORM_ORIGIN)
            .header(header::COOKIE, &cookies)
            .send()
            .await?;
        ensure_status(
            grants_response.status(),
            StatusCode::OK,
            "list support grants",
        )?;
        let grants_body: serde_json::Value = grants_response.json().await?;
        if !grants_body
            .get("grants")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|items| items.iter().any(|item| item["grant_id"] == grant_id))
        {
            return Err("Support grant list did not return the created grant".into());
        }

        let download_url = format!(
            "{api_base_url}/platform/support-grants/{grant_id}/workspaces/{}/assets/{}/download",
            workspace.workspace_id, asset.asset_id
        );
        let download = client
            .post(&download_url)
            .header(header::ORIGIN, PLATFORM_ORIGIN)
            .header(header::COOKIE, &cookies)
            .header("x-csrf-token", platform_csrf_value(&platform_csrf)?)
            .send()
            .await?;
        ensure_status(
            download.status(),
            StatusCode::OK,
            "authorized support download",
        )?;
        if download
            .headers()
            .get(header::CACHE_CONTROL)
            .and_then(|value| value.to_str().ok())
            != Some("no-store")
        {
            return Err("Support download response is missing no-store cache controls".into());
        }
        let download_body: serde_json::Value = download.json().await?;
        if !download_body
            .get("download_url")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|url| url.starts_with("https://"))
        {
            return Err("Authorized support download did not return a temporary HTTPS URL".into());
        }

        let wrong_scope = client
        .post(format!(
            "{api_base_url}/platform/support-grants/{grant_id}/workspaces/{}/assets/{}/download",
            workspace.workspace_id,
            Uuid::new_v4()
        ))
        .header(header::ORIGIN, PLATFORM_ORIGIN)
        .header(header::COOKIE, &cookies)
        .header("x-csrf-token", platform_csrf_value(&platform_csrf)?)
        .send()
        .await?;
        ensure_status(
            wrong_scope.status(),
            StatusCode::NOT_FOUND,
            "support grant scope",
        )?;

        let revoke = client
            .post(format!(
                "{api_base_url}/platform/support-grants/{grant_id}/revoke"
            ))
            .header(header::ORIGIN, PLATFORM_ORIGIN)
            .header(header::COOKIE, &cookies)
            .header("x-csrf-token", platform_csrf_value(&platform_csrf)?)
            .json(&serde_json::json!({"reason":"Automated access review completed"}))
            .send()
            .await?;
        ensure_status(revoke.status(), StatusCode::OK, "revoke support grant")?;
        let denied_download = client
            .post(download_url)
            .header(header::ORIGIN, PLATFORM_ORIGIN)
            .header(header::COOKIE, &cookies)
            .header("x-csrf-token", platform_csrf_value(&platform_csrf)?)
            .send()
            .await?;
        if ![StatusCode::NOT_FOUND, StatusCode::GONE].contains(&denied_download.status()) {
            return Err("Revoked support grant still authorized an asset download".into());
        }

        let audit = client
            .get(format!(
                "{api_base_url}/platform/support-audit?page_size=200"
            ))
            .header(header::ORIGIN, PLATFORM_ORIGIN)
            .header(header::COOKIE, &cookies)
            .send()
            .await?;
        ensure_status(audit.status(), StatusCode::OK, "support audit")?;
        let audit_body: serde_json::Value = audit.json().await?;
        if !audit_body
            .get("events")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|events| {
                events.iter().any(|event| {
                    event.get("grant_id").and_then(serde_json::Value::as_str)
                        == Some(grant_id.as_str())
                })
            })
        {
            return Err("Support audit did not include the grant lifecycle".into());
        }

        let platform_on_tenant = client
            .get(format!("{api_base_url}/api/v1/auth/session"))
            .header(header::ORIGIN, TENANT_ORIGIN)
            .header(
                header::COOKIE,
                format!(
                    "xovyra_tenant_session={}; xovyra_tenant_csrf={}",
                    platform_cookie_value(&platform_cookie)?,
                    platform_csrf_value(&platform_csrf)?
                ),
            )
            .send()
            .await?;
        ensure_status(
            platform_on_tenant.status(),
            StatusCode::UNAUTHORIZED,
            "platform cookie rejected by tenant routes",
        )?;

        provisioner
            .revoke_role(email, "platform_admin", &reason, &correlation_id)
            .await?;
        if identity
            .authenticate_session(
                platform_cookie_value(&platform_cookie)?,
                platform_csrf_value(&platform_csrf)?,
                Audience::Platform,
            )
            .await
            .is_ok()
        {
            return Err("Revoked operator role left its platform session authorized".into());
        }

        Ok(())
    }
    .await;
    let role_cleanup = if acceptance.is_err() {
        Some(
            provisioner
                .revoke_role(email, "platform_admin", &reason, &correlation_id)
                .await,
        )
    } else {
        None
    };
    let fixture_cleanup = cleanup_fixture(&workspace, asset.asset_id, user_id).await;
    tenancy_database.close().await;
    asset_database.close().await;
    acceptance?;
    if let Some(role_cleanup) = role_cleanup {
        role_cleanup?;
    }
    fixture_cleanup?;
    Ok(())
}

fn grant_body(workspace_id: Uuid, asset_id: Uuid) -> serde_json::Value {
    serde_json::json!({
        "workspace_id": workspace_id,
        "asset_id": asset_id,
        "reason": "Automated Phase 4.5 access review",
        "expires_at_unix": unix_now() + 600,
        "idempotency_key": Uuid::new_v4().to_string(),
    })
}

fn session_cookie(
    response: &reqwest::Response,
    name: &str,
) -> Result<String, Box<dyn Error + Send + Sync>> {
    response
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .find_map(|cookie| {
            cookie
                .split(';')
                .next()
                .filter(|pair| pair.starts_with(&format!("{name}=")))
                .map(str::to_owned)
        })
        .ok_or_else(|| format!("BFF login omitted the {name} cookie").into())
}

fn platform_cookie_value(cookie: &str) -> Result<&str, Box<dyn Error + Send + Sync>> {
    cookie
        .strip_prefix("xovyra_platform_session=")
        .ok_or_else(|| "Platform session cookie has an unexpected name".into())
}

fn platform_csrf_value(cookie: &str) -> Result<&str, Box<dyn Error + Send + Sync>> {
    cookie
        .strip_prefix("xovyra_platform_csrf=")
        .ok_or_else(|| "Platform CSRF cookie has an unexpected name".into())
}

fn ensure_status(
    actual: StatusCode,
    expected: StatusCode,
    operation: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    if actual != expected {
        return Err(format!("{operation} returned HTTP {actual}, expected {expected}").into());
    }
    Ok(())
}

async fn ensure_response(
    response: reqwest::Response,
    expected: StatusCode,
    operation: &str,
) -> Result<reqwest::Response, Box<dyn Error + Send + Sync>> {
    let status = response.status();
    if status != expected {
        return Err(format!("{operation} returned HTTP {status}, expected HTTP {expected}").into());
    }
    Ok(response)
}

async fn cleanup_fixture(
    workspace: &workspaces::Workspace,
    asset_id: Uuid,
    user_id: Uuid,
) -> Result<(), sqlx::Error> {
    let cleanup_url = std::env::var("MIGRATOR_DATABASE_URL").map_err(|_| {
        sqlx::Error::Configuration("MIGRATOR_DATABASE_URL is required for cleanup".into())
    })?;
    let cleanup = PgPoolOptions::new()
        .max_connections(1)
        .connect(&cleanup_url)
        .await?;
    for statement in [
        "DELETE FROM asset_storage.outbox_events WHERE payload->>'asset_id' = $1",
        "DELETE FROM asset_storage.assets WHERE asset_id = $1::uuid",
        "DELETE FROM tenancy_access.outbox_events WHERE payload->>'workspace_id' = $1",
        "DELETE FROM tenancy_access.workspace_audit_events WHERE workspace_id = $1::uuid",
        "DELETE FROM tenancy_access.workspace_memberships WHERE workspace_id = $1::uuid",
        "DELETE FROM tenancy_access.workspaces WHERE workspace_id = $1::uuid AND created_by_user_id = $2::uuid",
        "DELETE FROM tenancy_access.workspace_creation_idempotency WHERE user_id = $2::uuid AND workspace_id = $1::uuid",
    ] {
        let mut query = sqlx::query(statement).bind(workspace.workspace_id.to_string());
        if statement.contains("$2") {
            query = query.bind(user_id.to_string());
        }
        if statement.contains("asset_id") && !statement.contains("workspace_id") {
            query = sqlx::query(statement).bind(asset_id.to_string());
        }
        query.execute(&cleanup).await?;
    }
    cleanup.close().await;
    Ok(())
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| i64::try_from(duration.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or_default()
}
