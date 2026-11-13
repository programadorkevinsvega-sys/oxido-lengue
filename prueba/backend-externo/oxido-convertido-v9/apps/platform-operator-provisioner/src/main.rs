use std::process::ExitCode;

use sqlx::postgres::PgPoolOptions;
use xovyra_identity::authentication::platform_access::PlatformOperatorProvisioner;

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), &'static str> {
    match dotenvy::dotenv() {
        Ok(environment_path) => {
            if !environment_path.is_file() {
                return Err("Local environment file could not be validated");
            }
        }
        Err(error) if error.not_found() => {}
        Err(_) => return Err("Local environment configuration could not be loaded"),
    }
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let [
        action,
        email_flag,
        email,
        role_flag,
        role,
        reason_flag,
        reason,
    ] = arguments.as_slice()
    else {
        return Err(
            "Usage: xovyra-platform-operator-provisioner <grant|revoke> --email ADDRESS --role <platform_admin|platform_support_readonly> --reason 'audited reason'",
        );
    };
    if email_flag != "--email" || role_flag != "--role" || reason_flag != "--reason" {
        return Err("Arguments must include --email, --role, and --reason in that order");
    }
    let database_url = std::env::var("IDENTITY_DATABASE_URL")
        .map_err(|_| "IDENTITY_DATABASE_URL must be configured in backend/.env")?;
    let database = PgPoolOptions::new()
        .max_connections(2)
        .connect(&database_url)
        .await
        .map_err(|_| "Identity database connection failed")?;
    let provisioner = PlatformOperatorProvisioner::new(database.clone());
    let correlation_id = uuid::Uuid::new_v4().to_string();
    let result = match action.as_str() {
        "grant" => {
            provisioner
                .grant_role(email, role, reason, &correlation_id)
                .await
        }
        "revoke" => {
            provisioner
                .revoke_role(email, role, reason, &correlation_id)
                .await
        }
        _ => return Err("Action must be grant or revoke"),
    }
    .map_err(|_| "Operator role provisioning failed; inspect protected server logs")?;
    database.close().await;
    if !result.completed {
        return Err("No verified active account/role was changed; the attempt was audited");
    }
    println!("Platform operator role action completed and audited");
    Ok(())
}
