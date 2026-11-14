use oxido_compiler::config::ProjectConfig;
use std::{
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
};

/// Resuelve un archivo `.ox`, un directorio de proyecto o el proyecto del cwd.
pub fn resolve(argument: Option<&OsStr>) -> Result<PathBuf, String> {
    let cwd = std::env::current_dir()
        .map_err(|error| format!("no se pudo leer el directorio actual: {error}"))?;
    let candidate = argument.map_or_else(|| cwd.clone(), |value| cwd.join(value));
    let metadata = fs::metadata(&candidate)
        .map_err(|error| format!("no se pudo inspeccionar la ruta solicitada: {error}"))?;
    if metadata.is_file() {
        if candidate.extension() != Some(OsStr::new("ox")) {
            return Err("el archivo de entrada debe terminar en .ox".into());
        }
        return Ok(candidate);
    }
    if !metadata.is_dir() {
        return Err("la entrada debe ser un archivo .ox o un directorio de proyecto".into());
    }
    let root = find_project_root(&candidate)?;
    select_default_binary(&root)
}

fn find_project_root(start: &Path) -> Result<PathBuf, String> {
    let start = fs::canonicalize(start)
        .map_err(|error| format!("no se pudo resolver el directorio: {error}"))?;
    for directory in start.ancestors() {
        let manifest = directory.join("oxido.toml");
        match fs::symlink_metadata(&manifest) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err("oxido.toml no puede ser un enlace simbólico".into());
            }
            Ok(metadata) if metadata.is_file() => {
                if metadata.len() > 64 * 1024 {
                    return Err("oxido.toml supera el límite de 64 KiB".into());
                }
                let content = fs::read_to_string(&manifest)
                    .map_err(|error| format!("no se pudo leer oxido.toml: {error}"))?;
                ProjectConfig::parse(&content)
                    .map_err(|error| format!("oxido.toml inválido: {error}"))?;
                return Ok(directory.to_path_buf());
            }
            Ok(_) => return Err("oxido.toml debe ser un archivo regular".into()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("no se pudo inspeccionar oxido.toml: {error}")),
        }
        if directory.join(".git").exists() {
            break;
        }
    }
    Err("no se encontró oxido.toml en este directorio ni en sus directorios padres".into())
}

fn select_default_binary(root: &Path) -> Result<PathBuf, String> {
    let config_path = root.join("oxido.toml");
    let content = fs::read_to_string(&config_path)
        .map_err(|error| format!("no se pudo leer oxido.toml: {error}"))?;
    let config =
        ProjectConfig::parse(&content).map_err(|error| format!("oxido.toml inválido: {error}"))?;
    let main = root.join("src/main.ox");
    let selected = if main.is_file() {
        main
    } else if config.bins.len() == 1 {
        root.join(&config.bins[0].path)
    } else if config.bins.is_empty() {
        return Err("el proyecto no contiene src/main.ox ni un [[bin]] único".into());
    } else {
        return Err(
            "el proyecto declara varios [[bin]] y no tiene src/main.ox; indica el archivo .ox"
                .into(),
        );
    };
    let canonical_root = fs::canonicalize(root)
        .map_err(|error| format!("no se pudo resolver la raíz del proyecto: {error}"))?;
    let canonical_source = fs::canonicalize(&selected)
        .map_err(|error| format!("no se pudo resolver el archivo .ox seleccionado: {error}"))?;
    if !canonical_source.starts_with(&canonical_root) || !canonical_source.is_file() {
        return Err(
            "el archivo principal debe ser regular y permanecer dentro del proyecto".into(),
        );
    }
    Ok(canonical_source)
}
