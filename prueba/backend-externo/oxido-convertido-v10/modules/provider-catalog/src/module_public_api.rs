use xovyra_shared_kernel::BoundedContext;

pub const SCHEMA_NAME: &str = "provider_catalog";
pub const OWNER_ROLE: &str = "xovyra_provider_catalog_owner";
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

#[must_use]
pub const fn bounded_context() -> BoundedContext {
    BoundedContext {
        name: "provider_catalog",
        schema: SCHEMA_NAME,
    }
}
