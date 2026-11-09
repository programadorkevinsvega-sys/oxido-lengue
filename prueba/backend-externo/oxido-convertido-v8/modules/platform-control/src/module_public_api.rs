use xovyra_shared_kernel::BoundedContext;

pub const SCHEMA_NAME: &str = "platform_control";
pub const OWNER_ROLE: &str = "xovyra_platform_control_owner";
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

#[must_use]
pub const fn bounded_context() -> BoundedContext {
    BoundedContext {
        name: "platform_control",
        schema: SCHEMA_NAME,
    }
}
