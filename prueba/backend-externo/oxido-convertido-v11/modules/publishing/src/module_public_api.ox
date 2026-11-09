sintaxis llaves
use xovyra_shared_kernel::BoundedContext;

pub const SCHEMA_NAME: &str = "publishing";
pub const OWNER_ROLE: &str = "xovyra_publishing_owner";
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

#[must_use]
pub const fn bounded_context() -> BoundedContext {
    BoundedContext {
        name: "publishing",
        schema: SCHEMA_NAME,
    }
}
