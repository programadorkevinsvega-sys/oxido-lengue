sintaxis llaves
use xovyra_shared_kernel::BoundedContext;

pub const SCHEMA_NAME: &str = "usage_ledger";
pub const OWNER_ROLE: &str = "xovyra_usage_ledger_owner";
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

#[must_use]
pub const fn bounded_context() -> BoundedContext {
    BoundedContext {
        name: "usage_ledger",
        schema: SCHEMA_NAME,
    }
}
