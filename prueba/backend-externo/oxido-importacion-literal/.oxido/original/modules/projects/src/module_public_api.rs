use xovyra_shared_kernel::BoundedContext;

pub const SCHEMA_NAME: &str = "projects";
pub const OWNER_ROLE: &str = "xovyra_projects_owner";
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

#[must_use]
pub const fn bounded_context() -> BoundedContext {
    BoundedContext {
        name: "projects",
        schema: SCHEMA_NAME,
    }
}
