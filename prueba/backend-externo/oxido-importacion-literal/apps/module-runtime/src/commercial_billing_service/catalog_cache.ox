sintaxis llaves
use sqlx::PgPool;
use xovyra_commercial_billing::catalog::{CatalogError, PublishedPlan};

pub async fn list_published(
    database: &PgPool,
    cache: &mut redis::aio::ConnectionManager,
    locale: &str,
    currency: &str,
) -> Result<Vec<PublishedPlan>, CatalogError> {
    let key = format!("xovyra:billing:catalog:v1:{locale}:{currency}");
    match redis::cmd("GET")
        .arg(&key)
        .query_async::<Option<String>>(cache)
        .await
    {
        Ok(Some(value)) => match serde_json::from_str::<Vec<PublishedPlan>>(&value) {
            Ok(plans) => return Ok(plans),
            Err(error) => {
                tracing::warn!(event="billing.catalog.cache_invalid", component="commercial_billing", operation="decode_catalog_cache", dependency="redis", error_class="invalid_cached_value", error=%error)
            }
        },
        Ok(None) => {}
        Err(error) => {
            tracing::warn!(event="billing.catalog.cache_unavailable", component="commercial_billing", operation="read_catalog_cache", dependency="redis", error_class="cache_read_failed", error=%error)
        }
    }
    let plans =
        xovyra_commercial_billing::catalog::list_published(database, locale, currency).await?;
    match serde_json::to_string(&plans) {
        Ok(value) => {
            if let Err(error) = redis::cmd("SETEX")
                .arg(&key)
                .arg(60)
                .arg(value)
                .query_async::<String>(cache)
                .await
            {
                tracing::warn!(event="billing.catalog.cache_write_failed", component="commercial_billing", operation="write_catalog_cache", dependency="redis", error_class="cache_write_failed", error=%error);
            }
        }
        Err(error) => {
            tracing::error!(event="billing.catalog.cache_encode_failed", component="commercial_billing", operation="encode_catalog_cache", error_class="serialization_failed", error=%error)
        }
    }
    Ok(plans)
}
