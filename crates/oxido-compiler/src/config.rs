//! Carga y validación de la configuración declarativa del proyecto.

use serde::Deserialize;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

const MAX_CONFIG_BYTES: usize = 64 * 1024;

/// Configuración admitida durante el arranque del proyecto.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectConfig {
    pub language: LanguageConfig,
    #[serde(default)]
    pub package: Option<PackageConfig>,
    #[serde(default, rename = "bin")]
    pub bins: Vec<BinaryConfig>,
    #[serde(default)]
    pub dependencies: BTreeMap<String, PathDependency>,
    /// Manifiesto Cargo convertido por `oxido importar-proyecto`.
    /// Se mantiene como datos TOML; la configuración nunca admite comandos.
    #[serde(default)]
    pub cargo: Option<toml::Value>,
}

/// Metadatos opcionales del paquete que acompañan a los manifiestos Óxido.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageConfig {
    pub name: String,
    pub version: String,
}

/// Binario de proyecto seleccionable por su ruta fuente `.ox`.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BinaryConfig {
    #[serde(default)]
    pub name: Option<String>,
    pub path: PathBuf,
}

/// Dependencia Cargo local declarada sin comandos ni opciones ejecutables.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PathDependency {
    pub path: Option<PathBuf>,
    pub version: Option<String>,
    #[serde(default)]
    pub features: Vec<String>,
    #[serde(rename = "default-features", default = "default_true")]
    pub default_features: bool,
}

/// Dependencia ya resuelta y validada dentro de la raíz del proyecto.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedDependency {
    pub name: String,
    pub canonical_path: Option<PathBuf>,
    pub version: Option<String>,
    pub features: Vec<String>,
    pub default_features: bool,
}

fn default_true() -> bool {
    true
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
        if config.cargo.as_ref().is_some_and(|value| !value.is_table()) {
            return Err("la sección `cargo` de oxido.toml debe ser una tabla de datos".into());
        }
        if config.language.edition != "2024" {
            return Err(format!(
                "edición Rust no soportada: {}; la edición inicial es 2024",
                config.language.edition
            ));
        }
        if let Some(package) = &config.package {
            if !valid_cargo_name(&package.name) {
                return Err(format!("nombre de paquete no válido: `{}`", package.name));
            }
            semver::Version::parse(&package.version)
                .map_err(|error| format!("versión de paquete no válida: {error}"))?;
        }
        if config.bins.len() > 128 {
            return Err("oxido.toml supera el límite de 128 binarios".into());
        }
        let mut bin_names = std::collections::BTreeSet::new();
        let mut bin_paths = std::collections::BTreeSet::new();
        for bin in &config.bins {
            if bin.path.is_absolute()
                || bin.path.components().any(|component| {
                    matches!(
                        component,
                        std::path::Component::ParentDir | std::path::Component::Prefix(_)
                    )
                })
                || bin.path.extension() != Some(std::ffi::OsStr::new("ox"))
            {
                return Err(
                    "la ruta de [[bin]] debe ser relativa, confinada al proyecto y terminar en .ox"
                        .into(),
                );
            }
            if let Some(name) = &bin.name
                && (!valid_cargo_name(name) || !bin_names.insert(name))
            {
                return Err(format!("nombre de [[bin]] no válido o duplicado: `{name}`"));
            }
            if !bin_paths.insert(&bin.path) {
                return Err(format!(
                    "ruta de [[bin]] duplicada: `{}`",
                    bin.path.display()
                ));
            }
        }
        for (name, dependency) in &config.dependencies {
            validate_dependency(name, dependency)?;
        }
        Ok(config)
    }

    /// Canonicaliza crates locales y rechaza rutas que escapen de la raíz del proyecto.
    pub fn resolve_dependencies(
        &self,
        project_root: &Path,
    ) -> Result<Vec<ResolvedDependency>, String> {
        if self.dependencies.len() > 128 {
            return Err("oxido.toml supera el límite de 128 dependencias locales".into());
        }
        let root = fs::canonicalize(project_root)
            .map_err(|error| format!("no se pudo resolver la raíz del proyecto: {error}"))?;
        let mut resolved = Vec::with_capacity(self.dependencies.len());
        for (name, dependency) in &self.dependencies {
            validate_dependency(name, dependency)?;
            let canonical_path = if let Some(path) = &dependency.path {
                if path.is_absolute() {
                    return Err(format!(
                        "la dependencia `{name}` debe usar una ruta relativa al proyecto"
                    ));
                }
                let candidate = root.join(path);
                let canonical_path = fs::canonicalize(&candidate).map_err(|error| {
                    format!("no se pudo resolver la ruta de la dependencia `{name}`: {error}")
                })?;
                if !canonical_path.starts_with(&root) {
                    return Err(format!(
                        "la dependencia `{name}` sale de la raíz del proyecto"
                    ));
                }
                let metadata = fs::symlink_metadata(&canonical_path).map_err(|error| {
                    format!("no se pudo inspeccionar la dependencia `{name}`: {error}")
                })?;
                if !metadata.is_dir() {
                    return Err(format!(
                        "la ruta de la dependencia `{name}` debe ser un directorio"
                    ));
                }
                let manifest = canonical_path.join("Cargo.toml");
                let metadata = fs::symlink_metadata(&manifest).map_err(|error| {
                    format!("no se pudo inspeccionar Cargo.toml de `{name}`: {error}")
                })?;
                if metadata.file_type().is_symlink() || !metadata.is_file() {
                    return Err(format!(
                        "Cargo.toml de `{name}` debe ser un archivo regular"
                    ));
                }
                Some(canonical_path)
            } else {
                None
            };
            resolved.push(ResolvedDependency {
                name: name.clone(),
                canonical_path,
                version: dependency.version.clone(),
                features: dependency.features.clone(),
                default_features: dependency.default_features,
            });
        }
        Ok(resolved)
    }
}

fn validate_dependency(name: &str, dependency: &PathDependency) -> Result<(), String> {
    if !valid_cargo_name(name) {
        return Err(format!("nombre de dependencia Cargo no válido: `{name}`"));
    }
    if dependency.path.is_some() == dependency.version.is_some() {
        return Err(format!(
            "la dependencia `{name}` debe declarar exactamente uno de `path` o `version`"
        ));
    }
    if dependency.features.len() > 128
        || dependency
            .features
            .iter()
            .any(|feature| feature.is_empty() || feature.len() > 256 || feature.contains('\0'))
    {
        return Err(format!("features no válidas en la dependencia `{name}`"));
    }
    if let Some(version) = &dependency.version {
        semver::VersionReq::parse(version)
            .map_err(|error| format!("requisito de versión no válido para `{name}`: {error}"))?;
    }
    Ok(())
}

fn valid_cargo_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
        && name.as_bytes()[0].is_ascii_alphanumeric()
}
