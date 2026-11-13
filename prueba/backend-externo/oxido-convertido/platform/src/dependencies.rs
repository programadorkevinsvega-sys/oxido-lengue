use std::time::Duration;

use async_nats::jetstream;
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use thiserror::Error;
use tokio::time::timeout;
use xovyra_shared_kernel::BoundedContext;

const CONNECTION_TIMEOUT: Duration = Duration::from_secs(5);
const DATABASE_POOL_LIMIT: u32 = 12;

#[derive(Debug, Error)]
pub enum DependencyError {
    #[error("PostgreSQL connection failed")]
    DatabaseConnection(#[source] sqlx::Error),
    #[error("PostgreSQL connection timed out")]
    DatabaseTimeout,
    #[error("PostgreSQL readiness query failed")]
    DatabaseReadiness(#[source] sqlx::Error),
    #[error("identity database connection failed")]
    IdentityDatabaseConnection(#[source] sqlx::Error),
    #[error("identity database connection timed out")]
    IdentityDatabaseTimeout,
    #[error("identity database readiness query failed")]
    IdentityDatabaseReadiness(#[source] sqlx::Error),
    #[error("platform control database connection failed")]
    PlatformControlDatabaseConnection(#[source] sqlx::Error),
    #[error("platform control database connection timed out")]
    PlatformControlDatabaseTimeout,
    #[error("platform control database readiness query failed")]
    PlatformControlDatabaseReadiness(#[source] sqlx::Error),
    #[error("tenancy database connection failed")]
    TenancyDatabaseConnection(#[source] sqlx::Error),
    #[error("tenancy database connection timed out")]
    TenancyDatabaseTimeout,
    #[error("tenancy database readiness query failed")]
    TenancyDatabaseReadiness(#[source] sqlx::Error),
    #[error("commercial billing database connection failed")]
    CommercialBillingDatabaseConnection(#[source] sqlx::Error),
    #[error("commercial billing database readiness query failed")]
    CommercialBillingDatabaseReadiness(#[source] sqlx::Error),
    #[error("notifications database connection failed")]
    NotificationsDatabaseConnection(#[source] sqlx::Error),
    #[error("notifications database readiness query failed")]
    NotificationsDatabaseReadiness(#[source] sqlx::Error),
    #[error("realtime database connection failed")]
    RealtimeDatabaseConnection(#[source] sqlx::Error),
    #[error("realtime database readiness query failed")]
    RealtimeDatabaseReadiness(#[source] sqlx::Error),
    #[error("asset storage database connection failed")]
    AssetStorageDatabaseConnection(#[source] sqlx::Error),
    #[error("asset storage database readiness query failed")]
    AssetStorageDatabaseReadiness(#[source] sqlx::Error),
    #[error("jobs database connection failed")]
    JobsDatabaseConnection(#[source] sqlx::Error),
    #[error("jobs database readiness query failed")]
    JobsDatabaseReadiness(#[source] sqlx::Error),
    #[error("NATS connection failed")]
    NatsConnection(#[source] async_nats::ConnectError),
    #[error("NATS connection timed out")]
    NatsTimeout,
    #[error("Redis connection failed")]
    RedisConnection(#[source] redis::RedisError),
    #[error("Redis connection timed out")]
    RedisTimeout,
    #[error("Redis readiness command failed")]
    RedisReadiness(#[source] redis::RedisError),
    #[error("Redis returned an invalid readiness response")]
    RedisInvalidReadiness,
    #[error("JetStream stream provisioning failed")]
    StreamProvisioning(#[source] async_nats::jetstream::context::CreateStreamError),
}

pub struct PlatformDependencies {
    pub database: PgPool,
    pub identity_database: PgPool,
    pub platform_control_database: PgPool,
    pub tenancy_access_database: PgPool,
    pub commercial_billing_database: PgPool,
    pub notifications_database: PgPool,
    pub realtime_database: PgPool,
    pub asset_storage_database: PgPool,
    pub jobs_database: PgPool,
    pub cache: redis::aio::ConnectionManager,
    pub redis_client: redis::Client,
    pub messaging: async_nats::Client,
    pub jetstream: jetstream::Context,
}

pub struct DependencyConfiguration<'settings> {
    pub database_url: &'settings str,
    pub identity_database_url: &'settings str,
    pub platform_control_database_url: &'settings str,
    pub tenancy_access_database_url: &'settings str,
    pub commercial_billing_database_url: &'settings str,
    pub notifications_database_url: &'settings str,
    pub realtime_database_url: &'settings str,
    pub asset_storage_database_url: &'settings str,
    pub jobs_database_url: &'settings str,
    pub redis_url: &'settings str,
    pub nats_url: &'settings str,
    pub nats_username: &'settings str,
    pub nats_password: &'settings str,
    pub contexts: &'settings [BoundedContext],
}

impl PlatformDependencies {
    pub async fn connect(
        configuration: DependencyConfiguration<'_>,
    ) -> Result<Self, DependencyError> {
        let database = timeout(
            CONNECTION_TIMEOUT,
            PgPoolOptions::new()
                .max_connections(DATABASE_POOL_LIMIT)
                .acquire_timeout(CONNECTION_TIMEOUT)
                .connect(configuration.database_url),
        )
        .await
        .map_err(|_| DependencyError::DatabaseTimeout)?
        .map_err(DependencyError::DatabaseConnection)?;
        sqlx::query_scalar::<_, i32>("SELECT 1")
            .fetch_one(&database)
            .await
            .map_err(DependencyError::DatabaseReadiness)?;

        let identity_database = timeout(
            CONNECTION_TIMEOUT,
            PgPoolOptions::new()
                .max_connections(5)
                .acquire_timeout(CONNECTION_TIMEOUT)
                .connect(configuration.identity_database_url),
        )
        .await
        .map_err(|_| DependencyError::IdentityDatabaseTimeout)?
        .map_err(DependencyError::IdentityDatabaseConnection)?;
        sqlx::query_scalar::<_, i32>("SELECT 1")
            .fetch_one(&identity_database)
            .await
            .map_err(DependencyError::IdentityDatabaseReadiness)?;

        let platform_control_database = timeout(
            CONNECTION_TIMEOUT,
            PgPoolOptions::new()
                .max_connections(5)
                .acquire_timeout(CONNECTION_TIMEOUT)
                .connect(configuration.platform_control_database_url),
        )
        .await
        .map_err(|_| DependencyError::PlatformControlDatabaseTimeout)?
        .map_err(DependencyError::PlatformControlDatabaseConnection)?;
        sqlx::query_scalar::<_, i32>("SELECT 1")
            .fetch_one(&platform_control_database)
            .await
            .map_err(DependencyError::PlatformControlDatabaseReadiness)?;

        let tenancy_access_database = timeout(
            CONNECTION_TIMEOUT,
            PgPoolOptions::new()
                .max_connections(5)
                .acquire_timeout(CONNECTION_TIMEOUT)
                .connect(configuration.tenancy_access_database_url),
        )
        .await
        .map_err(|_| DependencyError::TenancyDatabaseTimeout)?
        .map_err(DependencyError::TenancyDatabaseConnection)?;
        sqlx::query_scalar::<_, i32>("SELECT 1")
            .fetch_one(&tenancy_access_database)
            .await
            .map_err(DependencyError::TenancyDatabaseReadiness)?;

        let commercial_billing_database = timeout(
            CONNECTION_TIMEOUT,
            PgPoolOptions::new()
                .max_connections(5)
                .acquire_timeout(CONNECTION_TIMEOUT)
                .connect(configuration.commercial_billing_database_url),
        )
        .await
        .map_err(|_| DependencyError::DatabaseTimeout)?
        .map_err(DependencyError::CommercialBillingDatabaseConnection)?;
        sqlx::query_scalar::<_, i32>("SELECT 1")
            .fetch_one(&commercial_billing_database)
            .await
            .map_err(DependencyError::CommercialBillingDatabaseReadiness)?;

        let notifications_database = timeout(
            CONNECTION_TIMEOUT,
            PgPoolOptions::new()
                .max_connections(5)
                .acquire_timeout(CONNECTION_TIMEOUT)
                .connect(configuration.notifications_database_url),
        )
        .await
        .map_err(|_| DependencyError::DatabaseTimeout)?
        .map_err(DependencyError::NotificationsDatabaseConnection)?;
        sqlx::query_scalar::<_, i32>("SELECT 1")
            .fetch_one(&notifications_database)
            .await
            .map_err(DependencyError::NotificationsDatabaseReadiness)?;

        let realtime_database = timeout(
            CONNECTION_TIMEOUT,
            PgPoolOptions::new()
                .max_connections(10)
                .acquire_timeout(CONNECTION_TIMEOUT)
                .connect(configuration.realtime_database_url),
        )
        .await
        .map_err(|_| DependencyError::DatabaseTimeout)?
        .map_err(DependencyError::RealtimeDatabaseConnection)?;
        sqlx::query_scalar::<_, i32>("SELECT 1")
            .fetch_one(&realtime_database)
            .await
            .map_err(DependencyError::RealtimeDatabaseReadiness)?;

        let asset_storage_database = timeout(
            CONNECTION_TIMEOUT,
            PgPoolOptions::new()
                .max_connections(5)
                .acquire_timeout(CONNECTION_TIMEOUT)
                .connect(configuration.asset_storage_database_url),
        )
        .await
        .map_err(|_| DependencyError::DatabaseTimeout)?
        .map_err(DependencyError::AssetStorageDatabaseConnection)?;
        sqlx::query_scalar::<_, i32>("SELECT 1")
            .fetch_one(&asset_storage_database)
            .await
            .map_err(DependencyError::AssetStorageDatabaseReadiness)?;

        let jobs_database = timeout(
            CONNECTION_TIMEOUT,
            PgPoolOptions::new()
                .max_connections(5)
                .acquire_timeout(CONNECTION_TIMEOUT)
                .connect(configuration.jobs_database_url),
        )
        .await
        .map_err(|_| DependencyError::DatabaseTimeout)?
        .map_err(DependencyError::JobsDatabaseConnection)?;
        sqlx::query_scalar::<_, i32>("SELECT 1")
            .fetch_one(&jobs_database)
            .await
            .map_err(DependencyError::JobsDatabaseReadiness)?;

        let redis_client = redis::Client::open(configuration.redis_url)
            .map_err(DependencyError::RedisConnection)?;
        let cache_configuration = redis::aio::ConnectionManagerConfig::new()
            .set_min_delay(Duration::from_millis(100))
            .set_max_delay(Duration::from_secs(30))
            .set_number_of_retries(1_000);
        let mut cache = timeout(
            CONNECTION_TIMEOUT,
            redis_client.get_connection_manager_with_config(cache_configuration),
        )
        .await
        .map_err(|_| DependencyError::RedisTimeout)?
        .map_err(DependencyError::RedisConnection)?;
        let cache_response: String = timeout(
            CONNECTION_TIMEOUT,
            redis::cmd("PING").query_async(&mut cache),
        )
        .await
        .map_err(|_| DependencyError::RedisTimeout)?
        .map_err(DependencyError::RedisReadiness)?;
        if cache_response != "PONG" {
            return Err(DependencyError::RedisInvalidReadiness);
        }

        let nats_options = async_nats::ConnectOptions::with_user_and_password(
            configuration.nats_username.to_owned(),
            configuration.nats_password.to_owned(),
        );
        let messaging = timeout(
            CONNECTION_TIMEOUT,
            nats_options.connect(configuration.nats_url),
        )
        .await
        .map_err(|_| DependencyError::NatsTimeout)?
        .map_err(DependencyError::NatsConnection)?;
        let jetstream = jetstream::new(messaging.clone());
        for context in configuration.contexts {
            let stream_name = format!("{}_EVENTS", context.schema.to_ascii_uppercase());
            let subject = format!("{}.>", context.name);
            jetstream
                .get_or_create_stream(jetstream::stream::Config {
                    name: stream_name,
                    subjects: vec![subject],
                    ..Default::default()
                })
                .await
                .map_err(DependencyError::StreamProvisioning)?;
        }

        Ok(Self {
            database,
            identity_database,
            platform_control_database,
            tenancy_access_database,
            commercial_billing_database,
            notifications_database,
            realtime_database,
            asset_storage_database,
            jobs_database,
            cache,
            redis_client,
            messaging,
            jetstream,
        })
    }
}
