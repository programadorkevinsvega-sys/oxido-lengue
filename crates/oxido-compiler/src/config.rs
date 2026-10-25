//! Carga y validación de la configuración declarativa del proyecto.

use serde::Deserialize;

const MAX_CONFIG_BYTES: usize = 64 * 1024;

/// Configuración admitida durante el arranque del proyecto.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectConfig {
    pub language: LanguageConfig,
}

/// Opciones de lenguaje soportadas por Óxido 0.1.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LanguageConfig {
    pub syntax: Option<SyntaxMode>,
    pub edition: String,
}

/// Modo estructural seleccionado por el proyecto.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyntaxMode {
    Indentation,
    Braces,
}

impl ProjectConfig {
    /// Parsea TOML declarativo y rechaza claves o ediciones no admitidas.
    pub fn parse(input: &str) -> Result<Self, String> {
        if input.len() > MAX_CONFIG_BYTES {
            return Err("oxido.toml supera el límite de 64 KiB".into());
        }
        let config: Self = toml::from_str(input).map_err(|error| error.to_string())?;
        if config.language.edition != "2024" {
            return Err(format!(
                "edición Rust no soportada: {}; la edición inicial es 2024",
                config.language.edition
            ));
        }
        Ok(config)
    }
}
