sintaxis llaves
use std::error::Error;
use std::net::{SocketAddr, TcpListener};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

pub struct LocalServices {
    runtime: Child,
    bff: Child,
    pub api_base_url: String,
}

impl Drop for LocalServices {
    fn drop(&mut self) {
        if let Err(error) = self.bff.kill() {
            tracing::warn!(error = %error, "BFF integration process did not stop cleanly");
        }
        if let Err(error) = self.runtime.kill() {
            tracing::warn!(error = %error, "Runtime integration process did not stop cleanly");
        }
        if let Err(error) = self.bff.wait() {
            tracing::warn!(error = %error, "BFF integration process could not be reaped");
        }
        if let Err(error) = self.runtime.wait() {
            tracing::warn!(error = %error, "Runtime integration process could not be reaped");
        }
    }
}

pub async fn start() -> Result<LocalServices, Box<dyn Error + Send + Sync>> {
    let runtime_address = free_address()?;
    let bff_address = free_address()?;
    let runtime_endpoint = format!("http://{runtime_address}");
    let executable_directory = workspace_root()?.join("target/debug");
    let runtime = Command::new(executable_directory.join("xovyra-module-runtime"))
        .env("MODULE_RUNTIME_BIND", runtime_address.to_string())
        .env("MODULE_RUNTIME_INTERNAL_ENDPOINT", &runtime_endpoint)
        .env("RESEND_API_KEY", "")
        .env("RESEND_FROM_EMAIL", "")
        .env("RESEND_FROM_MARKETING", "")
        .env("RESEND_WEBHOOK_SECRET", "")
        .env("PUBLIC_APP_ORIGIN", "http://127.0.0.1:5173")
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?;
    let api_base_url = format!("http://{bff_address}");
    let bff = Command::new(executable_directory.join("xovyra-bff"))
        .env("BFF_BIND", bff_address.to_string())
        .env("MODULE_RUNTIME_ENDPOINT", &runtime_endpoint)
        .env("BFF_TENANT_ALLOWED_ORIGINS", "http://127.0.0.1:5173")
        .env("BFF_PLATFORM_ALLOWED_ORIGINS", "http://127.0.0.1:5174")
        .env("BFF_SECURE_COOKIES", "false")
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?;
    let services = LocalServices {
        runtime,
        bff,
        api_base_url,
    };
    wait_for_readiness(&services.api_base_url).await?;
    verify_readiness_failure_diagnostics().await?;
    Ok(services)
}

pub async fn verify_readiness_failure_diagnostics() -> Result<(), Box<dyn Error + Send + Sync>> {
    let bff_address = free_address()?;
    let unavailable_runtime_address = free_address()?;
    let executable_directory = workspace_root()?.join("target/debug");
    let mut bff = Command::new(executable_directory.join("xovyra-bff"))
        .env("BFF_BIND", bff_address.to_string())
        .env(
            "MODULE_RUNTIME_ENDPOINT",
            format!("http://{unavailable_runtime_address}"),
        )
        .env("BFF_TENANT_ALLOWED_ORIGINS", "http://127.0.0.1:5173")
        .env("BFF_PLATFORM_ALLOWED_ORIGINS", "http://127.0.0.1:5174")
        .env("BFF_SECURE_COOKIES", "false")
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?;

    let verification: Result<(), Box<dyn Error + Send + Sync>> = async {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()?;
        let endpoint = format!("http://{bff_address}/api/v1/system/health");
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        loop {
            if tokio::time::Instant::now() >= deadline {
                return Err("BFF readiness diagnostic probe did not answer before timeout".into());
            }
            match client.get(&endpoint).send().await {
                Ok(response) => {
                    if response.status() != reqwest::StatusCode::SERVICE_UNAVAILABLE {
                        return Err(
                            "BFF readiness diagnostic probe returned an unexpected status".into(),
                        );
                    }
                    let correlation_id = response
                        .headers()
                        .get("x-correlation-id")
                        .and_then(|value| value.to_str().ok())
                        .ok_or("BFF readiness error omitted its correlation ID")?
                        .to_owned();
                    let body: serde_json::Value = response.json().await?;
                    if body.get("code").and_then(serde_json::Value::as_str)
                        != Some("runtime_readiness_dependency_unavailable")
                        || body
                            .pointer("/details/request_id")
                            .and_then(serde_json::Value::as_str)
                            != Some(correlation_id.as_str())
                    {
                        return Err(
                            "BFF readiness error did not preserve its safe request diagnostic"
                                .into(),
                        );
                    }
                    return Ok(());
                }
                Err(_) => tokio::time::sleep(Duration::from_millis(50)).await,
            }
        }
    }
    .await;
    if let Err(error) = bff.kill() {
        tracing::warn!(error = %error, "BFF readiness diagnostic probe did not stop cleanly");
    }
    if let Err(error) = bff.wait() {
        tracing::warn!(error = %error, "BFF readiness diagnostic probe could not be reaped");
    }
    verification
}

async fn wait_for_readiness(api_base_url: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()?;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    loop {
        if tokio::time::Instant::now() >= deadline {
            return Err("BFF and gRPC runtime did not become ready".into());
        }
        match client
            .get(format!("{api_base_url}/api/v1/system/health"))
            .send()
            .await
        {
            Ok(response) if response.status().is_success() => return Ok(()),
            Ok(response) => {
                tracing::debug!(status = %response.status(), "BFF is waiting for gRPC runtime readiness");
            }
            Err(error) => {
                tracing::debug!(error = %error, "BFF integration listener is starting");
            }
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

funcion free_address() -> Result<SocketAddr, Box<dyn Error + Send + Sync>> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    Ok(listener.local_addr()?)
}

fn workspace_root() -> Result<PathBuf, Box<dyn Error + Send + Sync>> {
    let manifest_directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_directory
        .ancestors()
        .nth(2)
        .map(PathBuf::from)
        .ok_or_else(|| "Identity manifest is not nested in the backend workspace".into())
}
