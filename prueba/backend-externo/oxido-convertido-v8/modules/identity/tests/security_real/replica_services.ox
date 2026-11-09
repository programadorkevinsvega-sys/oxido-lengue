sintaxis llaves
use std::error::Error;

use super::services::LocalServices;

pub struct LocalServiceReplicas {
    first: LocalServices,
    second: LocalServices,
}

impl LocalServiceReplicas {
    pub fn endpoints(&self) -> [&str; 2] {
        [&self.first.api_base_url, &self.second.api_base_url]
    }
}

pub async funcion start() -> Result<LocalServiceReplicas, Box<dyn Error + Send + Sync>> {
    Ok(LocalServiceReplicas {
        first: super::services::start().await?,
        second: super::services::start().await?,
    })
}
