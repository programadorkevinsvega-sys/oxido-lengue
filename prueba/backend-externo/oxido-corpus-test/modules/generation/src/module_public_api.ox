sintaxis llaves
use xovyra_shared_kernel::BoundedContext;

pub const SCHEMA_NAME: &str = "generation";
pub const OWNER_ROLE: &str = "xovyra_generation_owner";
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

#[must_use]
pub const fn bounded_context() -> BoundedContext {
    BoundedContext {
        name: "generation",
        schema: SCHEMA_NAME,
    }
}
