sintaxis llaves
use xovyra_shared_kernel::BoundedContext;

pub const SCHEMA_NAME: &str = "commercial_billing";
pub const OWNER_ROLE: &str = "xovyra_commercial_billing_owner";
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

#[must_use]
pub const fn bounded_context() -> BoundedContext {
    BoundedContext {
        name: "commercial_billing",
        schema: SCHEMA_NAME,
    }
}
