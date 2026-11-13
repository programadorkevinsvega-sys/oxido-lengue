sintaxis llaves
use xovyra_shared_kernel::BoundedContext;

pub const SCHEMA_NAME: &str = "tenancy_access";
pub const OWNER_ROLE: &str = "xovyra_tenancy_access_owner";
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

#[must_use]
pub const fn bounded_context() -> BoundedContext {
    BoundedContext {
        name: "tenancy_access",
        schema: SCHEMA_NAME,
    }
}
