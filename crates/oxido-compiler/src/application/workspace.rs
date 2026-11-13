//! Compilación Cargo de workspaces importados desde manifiestos Oxido.

use super::{
    cargo::{MappedCargoDiagnostic, parse_cargo_diagnostics_with_maps},
    modules::resolve_project_modules,
    source_map::{SourceMap, SourceMapSet},
};
use crate::{config::ProjectConfig, generate_ox};
use oxido_cargo::run;
use oxido_language::{
    SourceFile,
    domain::syntax::{LexOptions, SyntaxMode as LexerSyntaxMode},
};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::{OsStr, OsString},
    fs, io,
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_WORKSPACE: AtomicU64 = AtomicU64::new(0);
const MAX_WORKSPACE_FILES: usize = 100_000;
const MAX_SOURCE_BYTES: u64 = 4 * 1024 * 1024;
const MAX_TOTAL_SOURCE_BYTES: u64 = 1024 * 1024 * 1024;

/// Salida verificable de `cargo check` para un workspace importado.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportedWorkspaceCheck {
    pub success: bool,
    pub exit_code: Option<i32>,
    pub diagnostics: Vec<String>,
    pub program_output: Vec<String>,
    pub stderr: String,
}

pub fn is_imported_cargo_workspace(project_root: &Path) -> Result<bool, String> {
    let root = fs::canonicalize(project_root)
        .map_err(|error| format!("no se pudo resolver la raíz del workspace: {error}"))?;
    let config = read_project_config(&root)?;
    let manifest = config
        .cargo
        .as_ref()
        .and_then(|cargo| cargo.get("manifest"));
    Ok(manifest
        .and_then(|manifest| manifest.get("workspace"))
        .and_then(|workspace| workspace.get("members"))
        .and_then(toml::Value::as_array)
        .is_some())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImportedWorkspaceAction {
    Check,
    Build,
    Run,
    Test,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct GeneratedSource {
    contents: String,
    source_path: PathBuf,
    source_map: Option<SourceMap>,
}

struct StagingWorkspace(PathBuf);

impl Drop for StagingWorkspace {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            eprintln!("no se pudo retirar el workspace temporal de Óxido: {error}");
        }
    }
}

/// Exporta los targets `.ox` de un workspace importado dentro de un árbol temporal
/// y ejecuta `cargo check` usando los manifiestos reconstruidos desde `oxido.toml`.
/// La ejecución requiere confianza explícita porque Cargo puede ejecutar build scripts
/// y macros procedurales.
pub fn check_imported_workspace(
    project_root: &Path,
    trusted: bool,
    release: bool,
) -> Result<ImportedWorkspaceCheck, String> {
    run_imported_workspace(
        project_root,
        project_root,
        trusted,
        release,
        ImportedWorkspaceAction::Check,
    )
}

/// Exporta el workspace importado en staging y ejecuta la acción Cargo solicitada.
pub fn run_imported_workspace(
    project_root: &Path,
    selected_source: &Path,
    trusted: bool,
    release: bool,
    action: ImportedWorkspaceAction,
) -> Result<ImportedWorkspaceCheck, String> {
    if !trusted {
        return Err("compilar el workspace puede ejecutar build.rs y macros procedurales; revisa las dependencias y usa explícitamente `--confiar-dependencias`".into());
    }
    let project_root = fs::canonicalize(project_root)
        .map_err(|error| format!("no se pudo resolver la raíz del workspace: {error}"))?;
    if !project_root.is_dir() {
        return Err("la raíz del workspace debe ser un directorio".into());
    }
    let root_config = read_project_config(&project_root)?;
    let root_manifest = cargo_manifest(&root_config, &project_root)?;
    if root_manifest
        .get("workspace")
        .and_then(|workspace| workspace.get("members"))
        .is_none()
    {
        return Err(
            "oxido.toml no declara un workspace importado en cargo.manifest.workspace.members"
                .into(),
        );
    }

    let staging = create_staging_workspace()?;
    let mut file_count = 0usize;
    let mut total_bytes = 0u64;
    copy_project_tree(
        &project_root,
        &staging.0,
        &project_root,
        &mut file_count,
        &mut total_bytes,
    )?;

    let staged_root = staging.0.clone();
    write_cargo_manifest(&staged_root, &root_manifest)?;
    let member_paths = workspace_members(&root_manifest, &project_root)?;
    let mut package_roots = Vec::with_capacity(member_paths.len() + 1);
    if root_manifest.get("package").is_some() {
        package_roots.push((PathBuf::new(), project_root.clone(), root_manifest.clone()));
    }
    for relative in member_paths {
        let package_root = project_root.join(&relative);
        let package_config = read_project_config(&package_root)?;
        let package_manifest = cargo_manifest(&package_config, &package_root)?;
        if package_manifest.get("package").is_none() {
            return Err(format!(
                "el miembro {} no declara [cargo.manifest.package]",
                relative.display()
            ));
        }
        write_cargo_manifest(&staged_root.join(&relative), &package_manifest)?;
        package_roots.push((relative, package_root, package_manifest));
    }

    let mut generated_sources = BTreeMap::<PathBuf, GeneratedSource>::new();
    for (relative_package, package_root, manifest) in &package_roots {
        for source_relative in cargo_target_sources(package_root, manifest)? {
            let source_path = package_root.join(&source_relative);
            let output_relative = relative_package.join(&source_relative).with_extension("rs");
            generate_workspace_source(
                &source_path,
                &project_root,
                &output_relative,
                &mut generated_sources,
            )?;
        }
    }

    for (relative, source) in &generated_sources {
        let destination = staged_root.join(relative);
        let parent = destination.parent().ok_or_else(|| {
            format!(
                "el source generado no tiene directorio padre: {}",
                relative.display()
            )
        })?;
        fs::create_dir_all(parent)
            .map_err(|error| format!("no se pudo crear el directorio generado: {error}"))?;
        fs::write(&destination, &source.contents)
            .map_err(|error| format!("no se pudo escribir el Rust generado: {error}"))?;
    }

    let mut arguments = match action {
        ImportedWorkspaceAction::Check => vec![
            OsString::from("check"),
            OsString::from("--workspace"),
            OsString::from("--all-targets"),
        ],
        ImportedWorkspaceAction::Build => {
            vec![OsString::from("build"), OsString::from("--workspace")]
        }
        ImportedWorkspaceAction::Test => vec![
            OsString::from("test"),
            OsString::from("--workspace"),
            OsString::from("--all-targets"),
        ],
        ImportedWorkspaceAction::Run => {
            let (package, kind, target) = selected_cargo_target(&package_roots, selected_source)?;
            let mut arguments = vec![
                OsString::from("run"),
                OsString::from("--package"),
                package.into(),
            ];
            if kind == "bin" {
                arguments.extend([OsString::from("--bin"), target.into()]);
            } else if kind == "example" {
                arguments.extend([OsString::from("--example"), target.into()]);
            } else {
                return Err(
                    "`oxido ejecutar` necesita seleccionar un binario o ejemplo Cargo".into(),
                );
            }
            arguments
        }
    };
    arguments.extend([
        OsString::from("--locked"),
        OsString::from("--offline"),
        OsString::from("--message-format=json"),
    ]);
    if action == ImportedWorkspaceAction::Build {
        let target_dir = super::cargo::oxido_target_directory(&project_root)
            .map_err(|error| format!("no se pudo preparar la salida de binarios: {error}"))?;
        arguments.extend([OsString::from("--target-dir"), target_dir.into_os_string()]);
    }
    if release && action != ImportedWorkspaceAction::Test {
        arguments.push(OsString::from("--release"));
    }
    let cargo = run(&staged_root, &arguments)
        .map_err(|error| format!("no se pudo iniciar Cargo para el workspace temporal: {error}"))?;
    let diagnostics = render_workspace_messages(
        &cargo.messages,
        &staging.0,
        &project_root,
        &generated_sources,
    );
    Ok(ImportedWorkspaceCheck {
        success: cargo.success,
        exit_code: cargo.exit_code,
        diagnostics,
        program_output: cargo.non_json_stdout,
        stderr: cargo.stderr,
    })
}

fn create_staging_workspace() -> Result<StagingWorkspace, String> {
    let temp = fs::canonicalize(std::env::temp_dir())
        .map_err(|error| format!("no se pudo resolver el directorio temporal: {error}"))?;
    let root = temp.join("oxido-workspaces");
    if let Ok(metadata) = fs::symlink_metadata(&root)
        && metadata.file_type().is_symlink()
    {
        return Err("la raíz temporal de workspaces no puede ser un enlace simbólico".into());
    }
    fs::create_dir_all(&root)
        .map_err(|error| format!("no se pudo crear el directorio temporal: {error}"))?;
    let canonical_root = fs::canonicalize(&root)
        .map_err(|error| format!("no se pudo canonicalizar el directorio temporal: {error}"))?;
    if !canonical_root.starts_with(&temp) {
        return Err("la raíz temporal de workspaces sale del directorio temporal".into());
    }
    loop {
        let sequence = NEXT_WORKSPACE.fetch_add(1, Ordering::Relaxed);
        let candidate = canonical_root.join(format!("build-{}-{sequence}", std::process::id()));
        match fs::create_dir(&candidate) {
            Ok(()) => return Ok(StagingWorkspace(candidate)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(format!("no se pudo crear el staging temporal: {error}")),
        }
    }
}

fn copy_project_tree(
    source: &Path,
    destination: &Path,
    project_root: &Path,
    file_count: &mut usize,
    total_bytes: &mut u64,
) -> Result<(), String> {
    let mut entries = fs::read_dir(source)
        .map_err(|error| format!("no se pudo leer el árbol importado: {error}"))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("no se pudo leer una entrada importada: {error}"))?;
    entries.sort();
    for entry in entries {
        let relative = entry
            .strip_prefix(project_root)
            .map_err(|_| "una ruta de entrada escapa de la raíz importada")?;
        if relative.components().any(
            |component| matches!(component, Component::Normal(name) if name == OsStr::new(".git")),
        ) || relative == Path::new("target")
            || relative.starts_with(Path::new("target"))
            || relative == Path::new(".oxido/original")
            || relative.starts_with(Path::new(".oxido/original"))
        {
            continue;
        }
        let metadata = fs::symlink_metadata(&entry)
            .map_err(|error| format!("no se pudo inspeccionar una ruta importada: {error}"))?;
        if metadata.file_type().is_symlink() {
            return Err(format!(
                "el workspace importado contiene un enlace simbólico: {}",
                relative.display()
            ));
        }
        let target = destination.join(relative);
        if metadata.is_dir() {
            fs::create_dir_all(&target)
                .map_err(|error| format!("no se pudo crear un directorio temporal: {error}"))?;
            copy_project_tree(&entry, destination, project_root, file_count, total_bytes)?;
        } else if metadata.is_file() {
            *file_count = file_count.saturating_add(1);
            *total_bytes = total_bytes.saturating_add(metadata.len());
            if *file_count > MAX_WORKSPACE_FILES || *total_bytes > MAX_TOTAL_SOURCE_BYTES {
                return Err(
                    "el workspace importado supera los límites de archivos o tamaño".into(),
                );
            }
            if metadata.len() > MAX_SOURCE_BYTES && entry.extension().is_some_and(|ext| ext == "ox")
            {
                return Err(format!("{} supera el límite de 4 MiB", relative.display()));
            }
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)
                    .map_err(|error| format!("no se pudo crear un directorio temporal: {error}"))?;
            }
            fs::copy(&entry, &target)
                .map_err(|error| format!("no se pudo copiar una entrada al staging: {error}"))?;
        }
    }
    Ok(())
}

fn read_project_config(directory: &Path) -> Result<ProjectConfig, String> {
    let path = directory.join("oxido.toml");
    let metadata = fs::symlink_metadata(&path)
        .map_err(|error| format!("no se pudo inspeccionar {}: {error}", path.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > 64 * 1024 {
        return Err(format!(
            "{} debe ser un oxido.toml regular de hasta 64 KiB",
            path.display()
        ));
    }
    let text = fs::read_to_string(&path)
        .map_err(|error| format!("no se pudo leer {}: {error}", path.display()))?;
    ProjectConfig::parse(&text).map_err(|error| format!("{} inválido: {error}", path.display()))
}

fn cargo_manifest(config: &ProjectConfig, directory: &Path) -> Result<toml::Value, String> {
    let manifest = config
        .cargo
        .as_ref()
        .and_then(|cargo| cargo.get("manifest"))
        .cloned()
        .ok_or_else(|| {
            format!(
                "{} no contiene cargo.manifest",
                directory.join("oxido.toml").display()
            )
        })?;
    if !manifest.is_table() {
        return Err(format!(
            "cargo.manifest debe ser una tabla en {}",
            directory.join("oxido.toml").display()
        ));
    }
    Ok(manifest)
}

fn write_cargo_manifest(directory: &Path, manifest: &toml::Value) -> Result<(), String> {
    fs::create_dir_all(directory)
        .map_err(|error| format!("no se pudo crear la carpeta del paquete: {error}"))?;
    let contents = toml::to_string_pretty(manifest)
        .map_err(|error| format!("no se pudo serializar el manifiesto temporal: {error}"))?;
    fs::write(directory.join("Cargo.toml"), contents)
        .map_err(|error| format!("no se pudo escribir el manifiesto Cargo temporal: {error}"))
}

fn workspace_members(manifest: &toml::Value, root: &Path) -> Result<Vec<PathBuf>, String> {
    let members = manifest
        .get("workspace")
        .and_then(|workspace| workspace.get("members"))
        .and_then(toml::Value::as_array)
        .ok_or_else(|| "cargo.manifest.workspace.members debe ser una lista".to_owned())?;
    let mut patterns = Vec::new();
    for member in members {
        let member = member
            .as_str()
            .ok_or_else(|| "cada miembro del workspace debe ser una ruta de texto".to_owned())?;
        let relative = PathBuf::from(member);
        if relative.is_absolute()
            || relative
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(format!("ruta de miembro de workspace no segura: {member}"));
        }
        patterns.push(relative);
    }
    let exclusions = manifest
        .get("workspace")
        .and_then(|workspace| workspace.get("exclude"))
        .and_then(toml::Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(toml::Value::as_str)
                .map(PathBuf::from)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let mut directories = Vec::new();
    collect_workspace_directories(root, &mut directories)?;
    let mut result = BTreeSet::new();
    for directory in directories {
        let relative = directory
            .strip_prefix(root)
            .map_err(|_| "directorio de miembro fuera del workspace")?;
        if patterns
            .iter()
            .any(|pattern| glob_path_matches(pattern, relative))
            && !exclusions
                .iter()
                .any(|pattern| glob_path_matches(pattern, relative))
            && directory.join("oxido.toml").is_file()
        {
            result.insert(relative.to_owned());
        }
    }
    for pattern in patterns.iter().filter(|pattern| {
        !pattern
            .to_string_lossy()
            .chars()
            .any(|c| matches!(c, '*' | '?'))
    }) {
        if !result.contains(pattern) {
            return Err(format!(
                "el miembro {} no existe, está excluido o no contiene oxido.toml",
                pattern.display()
            ));
        }
    }
    if result.is_empty() {
        return Err("el workspace importado no tiene miembros".into());
    }
    Ok(result.into_iter().collect())
}

fn selected_cargo_target(
    packages: &[(PathBuf, PathBuf, toml::Value)],
    selected_source: &Path,
) -> Result<(String, String, String), String> {
    let selected = fs::canonicalize(selected_source)
        .map_err(|error| format!("no se pudo resolver el target seleccionado: {error}"))?;
    for (_, package_root, manifest) in packages {
        let Some(package_name) = manifest
            .get("package")
            .and_then(|package| package.get("name"))
            .and_then(toml::Value::as_str)
        else {
            continue;
        };
        for source in cargo_target_sources(package_root, manifest)? {
            let default_main = source.parent() == Some(Path::new("src"))
                && source.file_stem().is_some_and(|stem| stem == "main");
            let kind = if source.starts_with("examples/") {
                "example"
            } else if default_main
                || source.starts_with("src/bin/")
                || manifest.get("bin").is_some()
            {
                "bin"
            } else {
                continue;
            };
            let source_name = source
                .file_stem()
                .and_then(OsStr::to_str)
                .unwrap_or_default();
            let target_name = manifest
                .get(if kind == "bin" { "bin" } else { "example" })
                .and_then(toml::Value::as_array)
                .and_then(|items| {
                    items.iter().find(|item| {
                        let default_path = if kind == "bin" {
                            format!("src/bin/{source_name}.rs")
                        } else {
                            format!("examples/{source_name}.rs")
                        };
                        let declared = item
                            .get("path")
                            .and_then(toml::Value::as_str)
                            .unwrap_or(&default_path);
                        let declared_path = Path::new(declared);
                        let ox_path = declared_path.with_extension("ox");
                        declared_path == source
                            || (package_root.join(&ox_path).is_file() && ox_path == source)
                    })
                })
                .and_then(|item| item.get("name").and_then(toml::Value::as_str))
                .unwrap_or(if kind == "bin" && default_main {
                    package_name
                } else {
                    source_name
                });
            if fs::canonicalize(package_root.join(&source)).ok().as_deref()
                == Some(selected.as_path())
            {
                return Ok((
                    package_name.to_owned(),
                    kind.to_owned(),
                    target_name.to_owned(),
                ));
            }
        }
    }
    Err("el archivo seleccionado no es un target binario o ejemplo del workspace Cargo".into())
}

fn collect_workspace_directories(
    directory: &Path,
    output: &mut Vec<PathBuf>,
) -> Result<(), String> {
    let mut entries = fs::read_dir(directory)
        .map_err(|error| format!("no se pudo explorar miembros Cargo: {error}"))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("no se pudo leer directorio de miembro: {error}"))?;
    entries.sort();
    for path in entries {
        let name = path.file_name();
        if name.is_some_and(|name| matches!(name.to_str(), Some(".git" | "target" | ".oxido"))) {
            continue;
        }
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| format!("no se pudo inspeccionar miembro: {error}"))?;
        if metadata.file_type().is_symlink() {
            return Err(format!(
                "el workspace importado contiene enlace simbólico: {}",
                path.display()
            ));
        }
        if metadata.is_dir() {
            if !path.join("Cargo.toml").exists() && !path.join("oxido.toml").exists() {
                output.push(path.clone());
                collect_workspace_directories(&path, output)?;
            } else {
                output.push(path);
            }
        }
    }
    Ok(())
}

fn glob_path_matches(pattern: &Path, path: &Path) -> bool {
    let patterns = pattern
        .components()
        .filter_map(|part| match part {
            Component::Normal(value) => value.to_str(),
            _ => None,
        })
        .collect::<Vec<_>>();
    let parts = path
        .components()
        .filter_map(|part| match part {
            Component::Normal(value) => value.to_str(),
            _ => None,
        })
        .collect::<Vec<_>>();
    glob_segments_match(&patterns, &parts)
}

fn glob_segments_match(patterns: &[&str], parts: &[&str]) -> bool {
    match patterns.split_first() {
        None => parts.is_empty(),
        Some((&"**", rest)) => {
            (0..=parts.len()).any(|count| glob_segments_match(rest, &parts[count..]))
        }
        Some((pattern, rest)) => {
            parts
                .first()
                .is_some_and(|part| wildcard_match(pattern, part))
                && glob_segments_match(rest, &parts[1..])
        }
    }
}

fn wildcard_match(pattern: &str, value: &str) -> bool {
    let pattern = pattern.chars().collect::<Vec<_>>();
    let value = value.chars().collect::<Vec<_>>();
    let mut reachable = vec![false; value.len() + 1];
    reachable[0] = true;
    for token in pattern {
        let mut next = vec![false; value.len() + 1];
        if token == '*' {
            let mut seen = false;
            for index in 0..=value.len() {
                seen |= reachable[index];
                next[index] = seen;
            }
        } else {
            for index in 0..value.len() {
                if reachable[index] && (token == '?' || token == value[index]) {
                    next[index + 1] = true;
                }
            }
        }
        reachable = next;
    }
    reachable[value.len()]
}

fn cargo_target_sources(
    package_root: &Path,
    manifest: &toml::Value,
) -> Result<Vec<PathBuf>, String> {
    let mut sources = BTreeSet::new();
    let source_for = |path: &str| -> Result<Option<PathBuf>, String> {
        let declared = Path::new(path);
        if declared.is_absolute()
            || declared
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(format!("ruta de target no válida en Cargo: {path}"));
        }
        let mut ox = declared.with_extension("ox");
        if package_root.join(&ox).is_file() {
            return Ok(Some(ox));
        }
        if package_root.join(declared).is_file() {
            ox = declared.to_owned();
            return Ok(Some(ox));
        }
        Ok(None)
    };
    if let Some(lib) = manifest.get("lib") {
        if let Some(path) = lib.get("path").and_then(toml::Value::as_str) {
            if let Some(source) = source_for(path)? {
                sources.insert(source);
            }
        } else if let Some(source) = source_for("src/lib.rs")? {
            sources.insert(source);
        }
    } else if manifest
        .get("package")
        .and_then(|package| package.get("autolib"))
        .and_then(toml::Value::as_bool)
        != Some(false)
        && let Some(source) = source_for("src/lib.rs")?
    {
        sources.insert(source);
    }
    let autobins = manifest
        .get("package")
        .and_then(|package| package.get("autobins"))
        .and_then(toml::Value::as_bool)
        != Some(false);
    if let Some(binaries) = manifest.get("bin").and_then(toml::Value::as_array) {
        insert_declared_target_sources("bin", binaries, manifest, &source_for, &mut sources)?;
    }
    if autobins {
        if let Some(source) = source_for("src/main.rs")? {
            sources.insert(source);
        }
        collect_auto_targets(package_root, Path::new("src/bin"), &mut sources)?;
    }
    for (key, directory, auto_key) in [
        ("example", "examples", "autoexamples"),
        ("test", "tests", "autotests"),
        ("bench", "benches", "autobenches"),
    ] {
        let automatic = manifest
            .get("package")
            .and_then(|package| package.get(auto_key))
            .and_then(toml::Value::as_bool)
            != Some(false);
        if let Some(declared) = manifest.get(key).and_then(toml::Value::as_array) {
            insert_declared_target_sources(key, declared, manifest, &source_for, &mut sources)?;
        }
        if automatic {
            collect_auto_targets(package_root, Path::new(directory), &mut sources)?;
        }
    }
    match manifest
        .get("package")
        .and_then(|package| package.get("build"))
    {
        Some(toml::Value::Boolean(false)) => {}
        Some(toml::Value::String(path)) => {
            if let Some(source) = source_for(path)? {
                sources.insert(source);
            }
        }
        _ => {
            if let Some(source) = source_for("build.rs")? {
                sources.insert(source);
            }
        }
    }
    if sources.is_empty() {
        return Err(format!(
            "no se encontraron targets .ox/.rs en {}",
            package_root.display()
        ));
    }
    Ok(sources.into_iter().collect())
}

fn insert_declared_target_sources(
    kind: &str,
    targets: &[toml::Value],
    manifest: &toml::Value,
    source_for: &impl Fn(&str) -> Result<Option<PathBuf>, String>,
    sources: &mut BTreeSet<PathBuf>,
) -> Result<(), String> {
    for target in targets {
        let package_name = manifest
            .get("package")
            .and_then(|package| package.get("name"))
            .and_then(toml::Value::as_str)
            .unwrap_or_default();
        let name = target
            .get("name")
            .and_then(toml::Value::as_str)
            .unwrap_or(package_name);
        let default_path = match kind {
            "bin" if name == package_name => "src/main.rs".to_owned(),
            "bin" => format!("src/bin/{name}.rs"),
            "example" => format!("examples/{name}.rs"),
            "test" => format!("tests/{name}.rs"),
            "bench" => format!("benches/{name}.rs"),
            _ => continue,
        };
        let path = target
            .get("path")
            .and_then(toml::Value::as_str)
            .unwrap_or(&default_path);
        if let Some(source) = source_for(path)? {
            sources.insert(source);
        }
    }
    Ok(())
}

fn collect_auto_targets(
    package_root: &Path,
    relative_directory: &Path,
    sources: &mut BTreeSet<PathBuf>,
) -> Result<(), String> {
    let directory = package_root.join(relative_directory);
    if !directory.exists() {
        return Ok(());
    }
    let mut entries = fs::read_dir(&directory)
        .map_err(|error| format!("no se pudo leer {}: {error}", directory.display()))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("no se pudo leer un target: {error}"))?;
    entries.sort();
    for path in entries {
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| format!("no se pudo inspeccionar un target: {error}"))?;
        if metadata.file_type().is_symlink() {
            return Err(format!("un target es enlace simbólico: {}", path.display()));
        }
        if metadata.is_dir() {
            let main = path.join("main.rs");
            if main.is_file() {
                sources.insert(
                    main.strip_prefix(package_root)
                        .map_err(|_| "target fuera del paquete")?
                        .to_owned(),
                );
            }
        } else if metadata.is_file()
            && matches!(path.extension().and_then(|ext| ext.to_str()), Some("rs"))
        {
            let relative = path
                .strip_prefix(package_root)
                .map_err(|_| "target fuera del paquete")?
                .to_owned();
            sources.insert(relative);
        }
    }
    Ok(())
}

fn generate_workspace_source(
    source_path: &Path,
    project_root: &Path,
    output_relative: &Path,
    generated: &mut BTreeMap<PathBuf, GeneratedSource>,
) -> Result<(), String> {
    let metadata = fs::symlink_metadata(source_path)
        .map_err(|error| format!("no se pudo inspeccionar {}: {error}", source_path.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > MAX_SOURCE_BYTES
    {
        return Err(format!(
            "target de fuente no válido: {}",
            source_path.display()
        ));
    }
    let text = fs::read_to_string(source_path)
        .map_err(|error| format!("no se pudo leer {}: {error}", source_path.display()))?;
    let source = SourceFile::with_path(source_path.to_owned(), text);
    let config_dir = source_path
        .parent()
        .ok_or_else(|| "el target no tiene directorio padre".to_owned())?;
    let mut ancestor = config_dir;
    let config = loop {
        let path = ancestor.join("oxido.toml");
        if path.is_file() {
            break read_project_config(ancestor)?;
        }
        ancestor = ancestor
            .parent()
            .ok_or_else(|| format!("no se encontró oxido.toml para {}", source_path.display()))?;
        if !ancestor.starts_with(project_root) {
            return Err("oxido.toml del target sale del workspace".into());
        }
    };
    let options = LexOptions {
        project_mode: config.language.syntax.map(|syntax| match syntax {
            crate::config::SyntaxMode::Indentation => LexerSyntaxMode::Indentation,
            crate::config::SyntaxMode::Braces => LexerSyntaxMode::Braces,
        }),
    };
    let generated_program = generate_ox(&source, options).map_err(|diagnostics| {
        diagnostics
            .into_iter()
            .map(|diagnostic| format!("{}: {}", source_path.display(), diagnostic.message))
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    let resolved =
        resolve_project_modules(&source, &generated_program.rust, options, project_root)?;
    let root_destination = output_relative.to_owned();
    let source_relative = source_path
        .strip_prefix(project_root)
        .map_err(|_| "el target no pertenece al workspace")?
        .to_string_lossy()
        .replace('\\', "/");
    let mut root_map = generated_program.source_map;
    root_map.set_source_relative_path(source_relative);
    root_map.set_generated_relative_path(output_relative.to_string_lossy().replace('\\', "/"));
    insert_generated(
        generated,
        root_destination.clone(),
        resolved.root_rust,
        source_path,
        Some(root_map),
    )?;
    let source_parent = source_path
        .parent()
        .and_then(|parent| parent.strip_prefix(project_root).ok())
        .ok_or_else(|| "el target no pertenece al workspace".to_owned())?;
    for module in resolved.modules {
        let output = source_parent.join(module.path);
        let mut source_map = module.source_map;
        if let Some(source_map) = &mut source_map {
            source_map.set_generated_relative_path(output.to_string_lossy().replace('\\', "/"));
        }
        insert_generated(generated, output, module.rust, source_path, source_map)?;
    }
    Ok(())
}

fn insert_generated(
    generated: &mut BTreeMap<PathBuf, GeneratedSource>,
    relative: PathBuf,
    contents: String,
    source_path: &Path,
    source_map: Option<SourceMap>,
) -> Result<(), String> {
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
        || relative
            .extension()
            .and_then(|extension| extension.to_str())
            != Some("rs")
    {
        return Err(format!(
            "ruta Rust generada no segura: {}",
            relative.display()
        ));
    }
    if let Some(existing) = generated.get(&relative) {
        if existing.contents != contents {
            return Err(format!(
                "dos targets generan contenido distinto para {}",
                relative.display()
            ));
        }
        return Ok(());
    }
    generated.insert(
        relative,
        GeneratedSource {
            contents,
            source_path: source_path.to_owned(),
            source_map,
        },
    );
    Ok(())
}

fn render_workspace_messages(
    messages: &[Value],
    staging_root: &Path,
    project_root: &Path,
    generated: &BTreeMap<PathBuf, GeneratedSource>,
) -> Vec<String> {
    let maps = generated
        .iter()
        .filter_map(|(relative, source)| {
            source.source_map.clone().map(|mut map| {
                map.set_generated_relative_path(relative.to_string_lossy().replace('\\', "/"));
                (relative.clone(), map)
            })
        })
        .map(|(_, map)| map)
        .collect::<Vec<_>>();
    let mapped = parse_cargo_diagnostics_with_maps(messages, &SourceMapSet::new(maps));
    let mut rendered = mapped
        .into_iter()
        .map(|diagnostic| render_mapped_diagnostic(&diagnostic))
        .collect::<Vec<_>>();
    for event in messages
        .iter()
        .filter(|event| event["reason"] == "compiler-message")
    {
        let Some(message) = event.get("message") else {
            continue;
        };
        let primary_path = message
            .get("spans")
            .and_then(Value::as_array)
            .and_then(|spans| spans.iter().find(|span| span["is_primary"] == true))
            .and_then(|span| span["file_name"].as_str());
        if primary_path.is_some_and(|path| {
            generated
                .keys()
                .any(|relative| path.ends_with(&relative.to_string_lossy().replace('\\', "/")))
        }) {
            continue;
        }
        if let Some(text) = message.get("rendered").and_then(Value::as_str) {
            let mut text = text.to_owned();
            for (relative, source) in generated {
                let generated_path = staging_root.join(relative);
                let source_relative = source
                    .source_path
                    .strip_prefix(project_root)
                    .unwrap_or(&source.source_path);
                text = text.replace(
                    &generated_path.to_string_lossy().to_string(),
                    &source_relative.to_string_lossy(),
                );
            }
            rendered.push(text);
        }
    }
    rendered
}

fn render_mapped_diagnostic(diagnostic: &MappedCargoDiagnostic) -> String {
    let code = diagnostic
        .code
        .as_deref()
        .map(|code| format!("[{code}] "))
        .unwrap_or_default();
    let location = match (&diagnostic.source_file, &diagnostic.source) {
        (Some(path), Some(source)) => format!(" --> {path}:{}:{}", source.line, source.column),
        _ => String::new(),
    };
    format!(
        "{}: {code}{}{location}\n",
        diagnostic.level, diagnostic.message
    )
}
