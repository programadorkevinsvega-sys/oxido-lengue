use xovyra_shared_kernel::BoundedContext;

pub const SCHEMA_NAME: &str = "analytics";
pub const OWNER_ROLE: &str = "xovyra_analytics_owner";
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

#[must_use]
pub const fn bounded_context() -> BoundedContext {
    BoundedContext {
        name: "analytics",
        schema: SCHEMA_NAME,
    }
}
