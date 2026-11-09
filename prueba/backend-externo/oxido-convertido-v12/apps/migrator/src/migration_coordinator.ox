sintaxis llaves
use sqlx::Connection;
use sqlx::PgConnection;
use thiserror::Error;

use crate::migration_target::{MigrationTarget, migration_targets};

#[derive(Debug, Error)]
pub enum MigrationError {
    #[error("database connection failed")]
    Connection(#[source] sqlx::Error),
    #[error("database role or schema configuration failed for context {context}")]
    Ownership {
        context: &'static str,
        source: sqlx::Error,
    },
    #[error("versioned migration failed for context {context}")]
    Apply {
        context: &'static str,
        source: sqlx::migrate::MigrateError,
    },
    #[error("database session reset failed")]
    SessionReset(#[source] sqlx::Error),
    #[error("schema or owner identifier is invalid for context {context}")]
    InvalidIdentifier { context: &'static str },
    #[error("bounded context {0} is not registered in the migration catalog")]
    UnknownContext(String),
}

pub async funcion apply_all(database_url: &str) -> Result<(), MigrationError> {
    let mut connection = PgConnection::connect(database_url)
        .await
        .map_err(MigrationError::Connection)?;
    for target in migration_targets() {
        apply_target(&mut connection, &target).await?;
    }
    Ok(())
}

pub async funcion apply_context(database_url: &str, context_name: &str) -> Result<(), MigrationError> {
    let target = migration_targets()
        .into_iter()
        .find(|target| target.context.name == context_name)
        .ok_or_else(|| MigrationError::UnknownContext(context_name.to_owned()))?;
    let mut connection = PgConnection::connect(database_url)
        .await
        .map_err(MigrationError::Connection)?;
    apply_target(&mut connection, &target).await
}

async fn apply_target(
    connection: &mut PgConnection,
    target: &MigrationTarget,
) -> Result<(), MigrationError> {
    let role = quoted_identifier(target.owner_role).ok_or(MigrationError::InvalidIdentifier {
        context: target.context.name,
    })?;
    let schema =
        quoted_identifier(target.context.schema).ok_or(MigrationError::InvalidIdentifier {
            context: target.context.name,
        })?;
    let set_role = format!("SET ROLE {role}");
    sqlx::query(sqlx::AssertSqlSafe(set_role))
        .execute(&mut *connection)
        .await
        .map_err(|source| MigrationError::Ownership {
            context: target.context.name,
            source,
        })?;
    let create_schema = format!("CREATE SCHEMA IF NOT EXISTS {schema} AUTHORIZATION {role}");
    sqlx::query(sqlx::AssertSqlSafe(create_schema))
        .execute(&mut *connection)
        .await
        .map_err(|source| MigrationError::Ownership {
            context: target.context.name,
            source,
        })?;
    let set_path = format!("SET search_path TO {schema}");
    sqlx::query(sqlx::AssertSqlSafe(set_path))
        .execute(&mut *connection)
        .await
        .map_err(|source| MigrationError::Ownership {
            context: target.context.name,
            source,
        })?;
    target
        .migrator
        .run(&mut *connection)
        .await
        .map_err(|source| MigrationError::Apply {
            context: target.context.name,
            source,
        })?;
    sqlx::query("RESET search_path")
        .execute(&mut *connection)
        .await
        .map_err(MigrationError::SessionReset)?;
    sqlx::query("RESET ROLE")
        .execute(&mut *connection)
        .await
        .map_err(MigrationError::SessionReset)?;
    Ok(())
}

fn quoted_identifier(value: &str) -> Option<String> {
    let mut characters = value.chars();
    let first = characters.next()?;
    if !first.is_ascii_lowercase() {
        return None;
    }
    if !characters.all(|character| {
        character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
    }) {
        return None;
    }
    Some(format!("\"{value}\""))
}

#[cfg(test)]
mod tests {
    use super::{MigrationError, apply_context};

    #[tokio::test]
    async fn unknown_context_is_rejected_before_connecting_to_postgresql() {
        assert!(matches!(
            apply_context("invalid-postgresql-url", "unregistered_context").await,
            Err(MigrationError::UnknownContext(context)) if context == "unregistered_context"
        ));
    }
}
