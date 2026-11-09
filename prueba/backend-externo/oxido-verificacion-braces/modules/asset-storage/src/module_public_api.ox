sintaxis llaves
use xovyra_shared_kernel::BoundedContext;

pub const SCHEMA_NAME: &str = "asset_storage";
pub const OWNER_ROLE: &str = "xovyra_asset_storage_owner";
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

#[must_use]
pub const fn bounded_context() -> BoundedContext {
    BoundedContext {
        name: "asset_storage",
        schema: SCHEMA_NAME,
    }
}
