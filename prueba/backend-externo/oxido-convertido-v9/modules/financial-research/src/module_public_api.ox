sintaxis llaves
use xovyra_shared_kernel::BoundedContext;

pub const SCHEMA_NAME: &str = "financial_research";
pub const OWNER_ROLE: &str = "xovyra_financial_research_owner";
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

#[must_use]
pub const fn bounded_context() -> BoundedContext {
    BoundedContext {
        name: "financial_research",
        schema: SCHEMA_NAME,
    }
}
