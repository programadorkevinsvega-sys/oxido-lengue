//! Compilación Cargo de workspaces importados desde manifiestos Oxido.

use super::modules::resolve_project_modules;
use crate::{config::ProjectConfig, generate_ox};
use oxido_cargo::{CargoOutput, run};
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
    pub stderr: String,
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
    let mut package_roots = Vec::with_capacity(member_paths.len());
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

    let mut generated_sources = BTreeMap::<PathBuf, (String, PathBuf)>::new();
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

    for (relative, (contents, _)) in &generated_sources {
        let destination = staged_root.join(relative);
        let parent = destination.parent().ok_or_else(|| {
            format!(
                "el source generado no tiene directorio padre: {}",
                relative.display()
            )
        })?;
        fs::create_dir_all(parent)
            .map_err(|error| format!("no se pudo crear el directorio generado: {error}"))?;
        fs::write(&destination, contents)
            .map_err(|error| format!("no se pudo escribir el Rust generado: {error}"))?;
    }

    let mut arguments = vec![
        OsString::from("check"),
        OsString::from("--workspace"),
        OsString::from("--locked"),
        OsString::from("--offline"),
        OsString::from("--message-format=json"),
    ];
    if release {
        arguments.push(OsString::from("--release"));
    }
    let cargo = run(&staged_root, &arguments)
        .map_err(|error| format!("no se pudo iniciar Cargo para el workspace temporal: {error}"))?;
    let diagnostics = render_workspace_messages(&cargo, &staging.0, &generated_sources);
    Ok(ImportedWorkspaceCheck {
        success: cargo.success,
        exit_code: cargo.exit_code,
        diagnostics,
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
    let mut result = BTreeSet::new();
    for member in members {
        let member = member
            .as_str()
            .ok_or_else(|| "cada miembro del workspace debe ser una ruta de texto".to_owned())?;
        let relative = PathBuf::from(member);
        if relative.is_absolute()
            || relative
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
            || member.contains('*')
        {
            return Err(format!(
                "patrón o ruta de miembro no admitida en este importador: {member}"
            ));
        }
        let canonical = fs::canonicalize(root.join(&relative))
            .map_err(|error| format!("no se pudo resolver el miembro {member}: {error}"))?;
        if !canonical.starts_with(root) || !canonical.is_dir() {
            return Err(format!(
                "el miembro {member} sale del workspace o no es directorio"
            ));
        }
        result.insert(relative);
    }
    if result.is_empty() {
        return Err("el workspace importado no tiene miembros".into());
    }
    Ok(result.into_iter().collect())
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
    } else if let Some(source) = source_for("src/lib.rs")? {
        sources.insert(source);
    }
    if let Some(binaries) = manifest.get("bin").and_then(toml::Value::as_array) {
        for binary in binaries {
            if let Some(path) = binary.get("path").and_then(toml::Value::as_str)
                && let Some(source) = source_for(path)?
            {
                sources.insert(source);
            }
        }
    } else {
        if let Some(source) = source_for("src/main.rs")? {
            sources.insert(source);
        }
        collect_target_files(package_root.join("src/bin"), &mut sources)?;
    }
    match manifest.get("build") {
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

fn collect_target_files(directory: PathBuf, sources: &mut BTreeSet<PathBuf>) -> Result<(), String> {
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
            let main = path.join("main.ox");
            if main.is_file() {
                let relative = main
                    .strip_prefix(directory.parent().unwrap_or(&directory))
                    .map_err(|_| "target fuera de src/bin")?
                    .to_owned();
                sources.insert(relative);
            }
        } else if metadata.is_file()
            && matches!(
                path.extension().and_then(|ext| ext.to_str()),
                Some("ox" | "rs")
            )
        {
            let base = directory.parent().unwrap_or(&directory);
            let relative = path
                .strip_prefix(base)
                .map_err(|_| "target fuera de src/bin")?
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
    generated: &mut BTreeMap<PathBuf, (String, PathBuf)>,
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
    insert_generated(
        generated,
        root_destination.clone(),
        resolved.root_rust,
        source_path,
    )?;
    let source_parent = source_path
        .parent()
        .and_then(|parent| parent.strip_prefix(project_root).ok())
        .ok_or_else(|| "el target no pertenece al workspace".to_owned())?;
    for module in resolved.modules {
        let output = source_parent.join(module.path);
        insert_generated(generated, output, module.rust, source_path)?;
    }
    Ok(())
}

fn insert_generated(
    generated: &mut BTreeMap<PathBuf, (String, PathBuf)>,
    relative: PathBuf,
    contents: String,
    source_path: &Path,
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
    if let Some((existing, _)) = generated.get(&relative) {
        if existing != &contents {
            return Err(format!(
                "dos targets generan contenido distinto para {}",
                relative.display()
            ));
        }
        return Ok(());
    }
    generated.insert(relative, (contents, source_path.to_owned()));
    Ok(())
}

fn render_workspace_messages(
    cargo: &CargoOutput,
    staging_root: &Path,
    generated: &BTreeMap<PathBuf, (String, PathBuf)>,
) -> Vec<String> {
    let mut replacements = generated
        .iter()
        .map(|(relative, (_, source))| (staging_root.join(relative), source.clone()))
        .collect::<Vec<_>>();
    replacements.sort_by_key(|(generated, _)| std::cmp::Reverse(generated.as_os_str().len()));
    let mut rendered = Vec::new();
    for event in &cargo.messages {
        if event.get("reason").and_then(Value::as_str) != Some("compiler-message") {
            continue;
        }
        let Some(message) = event.get("message") else {
            continue;
        };
        if let Some(text) = message.get("rendered").and_then(Value::as_str) {
            let mut text = text.to_owned();
            for (generated_path, source_path) in &replacements {
                text = text.replace(
                    &generated_path.to_string_lossy().to_string(),
                    &source_path.to_string_lossy(),
                );
            }
            rendered.push(text);
        }
    }
    rendered
}
