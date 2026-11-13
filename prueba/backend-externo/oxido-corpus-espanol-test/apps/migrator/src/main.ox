sintaxis llaves
mod migration_coordinator;
mod migration_target;

use std::process::ExitCode;

use xovyra_platform::configuration::{MigrationConfiguration, load_environment_file};
use xovyra_platform::observability;

funcion requested_context() -> Result<Option<String>, &'static str> {
    let mut arguments = std::env::args().skip(1);
    match (arguments.next(), arguments.next(), arguments.next()) {
        (None, None, None) => Ok(None),
        (Some(flag), Some(context), None) if flag == "--context" && !context.is_empty() => {
            Ok(Some(context))
        }
        _ => Err(
            "usage: xovyra-migrator [--context <bounded_context>]; omit --context to apply all contexts",
        ),
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    let environment_result = load_environment_file();
    if let Err(error) = observability::initialize("info") {
        tracing::error!(error = %error, "Cannot initialize structured logging");
        return ExitCode::FAILURE;
    }
    if let Err(error) = environment_result {
        tracing::error!(error = %error, "Cannot load local environment configuration");
        return ExitCode::FAILURE;
    }
    let context = match requested_context() {
        Ok(context) => context,
        Err(message) => {
            tracing::error!(message, "Invalid migration command");
            return ExitCode::FAILURE;
        }
    };
    let configuration = match MigrationConfiguration::from_environment() {
        Ok(configuration) => configuration,
        Err(error) => {
            tracing::error!(error = %error, "Invalid migration configuration");
            return ExitCode::FAILURE;
        }
    };
    let migration_result = match context.as_deref() {
        Some(context_name) => {
            migration_coordinator::apply_context(&configuration.database_url, context_name).await
        }
        None => migration_coordinator::apply_all(&configuration.database_url).await,
    };
    match migration_result {
        Ok(()) => {
            match context {
                Some(context_name) => tracing::info!(
                    context = context_name,
                    "Bounded-context migrations completed"
                ),
                None => tracing::info!("All bounded-context migrations completed"),
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            tracing::error!(error = %error, details = ?error, "Bounded-context migrations failed");
            ExitCode::FAILURE
        }
    }
}
