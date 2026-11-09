sintaxis llaves
use xovyra_shared_kernel::BoundedContext;

pub const SCHEMA_NAME: &str = "media_pipeline";
pub const OWNER_ROLE: &str = "xovyra_media_pipeline_owner";
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

#[must_use]
pub const fn bounded_context() -> BoundedContext {
    BoundedContext {
        name: "media_pipeline",
        schema: SCHEMA_NAME,
    }
}
