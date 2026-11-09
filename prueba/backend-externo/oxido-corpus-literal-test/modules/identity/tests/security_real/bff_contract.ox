sintaxis llaves
use std::error::Error;

pub async fn verify_bff_grpc_profile(
    api_base_url: &str,
    session_secret: &str,
    csrf_secret: &str,
    expected_email: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let response = reqwest::Client::new()
        .get(format!("{api_base_url}/api/v1/me"))
        .header("Origin", "http://127.0.0.1:5173")
        .header(
            reqwest::header::COOKIE,
            format!("xovyra_tenant_session={session_secret}; xovyra_tenant_csrf={csrf_secret}"),
        )
        .send()
        .await?;
    if !response.status().is_success() {
        return Err(format!(
            "BFF profile request failed with status {}",
            response.status()
        )
        .into());
    }
    let profile: serde_json::Value = response.json().await?;
    if profile.get("email").and_then(serde_json::Value::as_str) != Some(expected_email) {
        return Err("BFF gRPC profile response did not match the authenticated account".into());
    }
    let rejected_origin = reqwest::Client::new()
        .get(format!("{api_base_url}/api/v1/me"))
        .header("Origin", "http://127.0.0.1:5174")
        .header(
            reqwest::header::COOKIE,
            format!("xovyra_tenant_session={session_secret}; xovyra_tenant_csrf={csrf_secret}"),
        )
        .send()
        .await?;
    if rejected_origin.status() != reqwest::StatusCode::FORBIDDEN {
        return Err("Tenant BFF accepted the platform frontend origin".into());
    }
    let csrf_rejected = reqwest::Client::new()
        .patch(format!("{api_base_url}/api/v1/me"))
        .header("Origin", "http://127.0.0.1:5173")
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .header(
            reqwest::header::COOKIE,
            format!("xovyra_tenant_session={session_secret}; xovyra_tenant_csrf={csrf_secret}"),
        )
        .body(r#"{"display_name":"Rejected Update","locale":"es-CO","timezone":"America/Bogota"}"#)
        .send()
        .await?;
    if csrf_rejected.status() != reqwest::StatusCode::FORBIDDEN {
        return Err("Tenant BFF accepted a profile mutation without its CSRF header".into());
    }
    let cross_audience = reqwest::Client::new()
        .get(format!("{api_base_url}/platform/auth/session"))
        .header("Origin", "http://127.0.0.1:5174")
        .header(
            reqwest::header::COOKIE,
            format!("xovyra_tenant_session={session_secret}; xovyra_tenant_csrf={csrf_secret}"),
        )
        .send()
        .await?;
    if cross_audience.status() != reqwest::StatusCode::UNAUTHORIZED {
        return Err("Tenant session authenticated on the platform BFF surface".into());
    }
    Ok(())
}
