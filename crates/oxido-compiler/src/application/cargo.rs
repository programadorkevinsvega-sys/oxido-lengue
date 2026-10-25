use super::source_map::{SourceLocation, SourceMap};
use serde_json::Value;
use std::{
    collections::BTreeSet,
    ffi::OsString,
    fs, io,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_WORKSPACE: AtomicU64 = AtomicU64::new(0);
static NEXT_LOCKFILE: AtomicU64 = AtomicU64::new(0);
const MAX_LOCKFILE_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CargoAction {
    /// Compile without user dependencies.
    Check,
    /// Compile after the operator explicitly trusts declared dependencies.
    CheckTrusted,
    /// Run without user dependencies.
    Run,
    /// Run after the operator explicitly trusts declared dependencies.
    RunTrusted,
    /// Resolve and display a lockfile/download plan without fetching crates.
    FetchPlan,
    /// Fetch crates for an already reviewed plan and persist the lockfile.
    Fetch,
    /// Resolve and display an updated lockfile plan without fetching crates.
    FetchUpdatePlan,
    /// Apply an explicitly confirmed updated plan and persist its lockfile.
    FetchUpdate,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MappedCargoDiagnostic {
    pub level: String,
    pub code: Option<String>,
    pub message: String,
    pub source: Option<SourceLocation>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CargoBuildResult {
    pub success: bool,
    pub timed_out: bool,
    pub exit_code: Option<i32>,
    pub diagnostics: Vec<MappedCargoDiagnostic>,
    pub program_output: Vec<String>,
    pub stderr: String,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct LockPackage {
    name: String,
    version: String,
    source: Option<String>,
    checksum: Option<String>,
    dependencies: Vec<String>,
}

fn lock_packages(contents: &str) -> io::Result<BTreeSet<LockPackage>> {
    let lock: toml::Value = toml::from_str(contents)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let packages = lock
        .get("package")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Cargo.lock no tiene package"))?;
    let mut result = BTreeSet::new();
    for package in packages {
        let Some(name) = package.get("name").and_then(toml::Value::as_str) else {
            continue;
        };
        let Some(version) = package.get("version").and_then(toml::Value::as_str) else {
            continue;
        };
        let source = package
            .get("source")
            .and_then(toml::Value::as_str)
            .map(str::to_owned);
        let checksum = package
            .get("checksum")
            .and_then(toml::Value::as_str)
            .map(str::to_owned);
        let mut dependencies = package
            .get("dependencies")
            .and_then(toml::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(toml::Value::as_str)
            .map(str::to_owned)
            .collect::<Vec<_>>();
        dependencies.sort();
        result.insert(LockPackage {
            name: name.to_owned(),
            version: version.to_owned(),
            source,
            checksum,
            dependencies,
        });
    }
    Ok(result)
}

fn render_lock_plan(previous: Option<&str>, generated: &str) -> io::Result<String> {
    let next = lock_packages(generated)?;
    let previous_contents = previous;
    let previous_packages = previous.map(lock_packages).transpose()?.unwrap_or_default();
    let next_lock: toml::Value = toml::from_str(generated)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let next_version = next_lock
        .get("version")
        .and_then(toml::Value::as_integer)
        .unwrap_or_default();
    let previous_version = previous_contents
        .map(toml::from_str::<toml::Value>)
        .transpose()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?
        .and_then(|lock| lock.get("version").and_then(toml::Value::as_integer));
    let mut output =
        vec!["Plan de dependencias Cargo (no se ejecutará código de crates):".to_owned()];
    let previous_version =
        previous_version.map_or_else(|| "sin lockfile".into(), |version| version.to_string());
    output.push(format!(
        "  Formato Cargo.lock: {previous_version} -> {next_version}"
    ));
    let removed = previous_packages.difference(&next).collect::<Vec<_>>();
    let added = next.difference(&previous_packages).collect::<Vec<_>>();
    if removed.is_empty() && added.is_empty() {
        output.push("  Cargo.lock: sin cambios".into());
    } else {
        output.push("  Cambios en Cargo.lock:".into());
        for package in removed {
            output.push(format!("  - {}", display_lock_package(package)));
        }
        for package in added {
            output.push(format!("  + {}", display_lock_package(package)));
        }
    }
    output.push("  Crates de crates.io fijadas (Cargo descargará las que falten):".into());
    let mut registry_packages = next
        .iter()
        .filter(|package| {
            package.source.as_deref().is_some_and(|value| {
                value.starts_with("registry+https://github.com/rust-lang/crates.io-index")
            })
        })
        .map(|package| format!("{}@{}", package.name, package.version))
        .collect::<Vec<_>>();
    registry_packages.sort();
    if registry_packages.is_empty() {
        output.push("    (ninguna)".into());
    } else {
        output.extend(
            registry_packages
                .into_iter()
                .map(|package| format!("    {package}")),
        );
    }
    Ok(output.join("\n"))
}

fn display_lock_package(package: &LockPackage) -> String {
    let checksum = package
        .checksum
        .as_deref()
        .map(|checksum| format!(" checksum={checksum}"))
        .unwrap_or_default();
    let dependencies = if package.dependencies.is_empty() {
        String::new()
    } else {
        format!(" deps=[{}]", package.dependencies.join(", "))
    };
    format!(
        "{}@{} [{}]{checksum}{dependencies}",
        package.name,
        package.version,
        lock_source(package.source.as_deref())
    )
}

fn lock_source(source: Option<&str>) -> &'static str {
    match source {
        Some(value) if value.starts_with("registry+") => "registry",
        Some(value) if value.starts_with("git+") => "git",
        Some(_) => "otra fuente",
        None => "local/workspace",
    }
}

pub fn build_generated(
    rust: &str,
    source_map: &SourceMap,
    action: CargoAction,
) -> io::Result<CargoBuildResult> {
    build_generated_with_dependencies(rust, source_map, &[], None, action)
}

pub fn build_generated_with_dependencies(
    rust: &str,
    source_map: &SourceMap,
    dependencies: &[crate::config::ResolvedDependency],
    persistent_lockfile: Option<&Path>,
    action: CargoAction,
) -> io::Result<CargoBuildResult> {
    if !dependencies.is_empty() && matches!(action, CargoAction::Check | CargoAction::Run) {
        return Ok(CargoBuildResult {
            success: false,
            timed_out: false,
            exit_code: None,
            diagnostics: vec![MappedCargoDiagnostic {
                level: "error".into(),
                code: Some("OX7002".into()),
                message: "la compilación puede ejecutar build.rs y macros procedurales; revisa las dependencias y usa explícitamente `--confiar-dependencias`".into(),
                source: None,
            }],
            program_output: Vec::new(),
            stderr: String::new(),
        });
    }
    let update_lock = matches!(
        action,
        CargoAction::FetchUpdatePlan | CargoAction::FetchUpdate
    );
    let workspace = create_workspace(
        rust,
        source_map,
        dependencies,
        persistent_lockfile,
        !update_lock,
    )?;
    let lock_path = workspace.path().join("Cargo.lock");
    if matches!(
        action,
        CargoAction::FetchPlan
            | CargoAction::Fetch
            | CargoAction::FetchUpdatePlan
            | CargoAction::FetchUpdate
    ) {
        if !lock_path.is_file() || update_lock {
            let output =
                oxido_cargo::run(workspace.path(), &[OsString::from("generate-lockfile")])?;
            if !output.success {
                return Ok(CargoBuildResult {
                    success: false,
                    timed_out: output.timed_out,
                    exit_code: output.exit_code,
                    diagnostics: Vec::new(),
                    program_output: output.non_json_stdout,
                    stderr: output.stderr,
                });
            }
        }
        let generated_lock = fs::read_to_string(&lock_path)?;
        let previous_lock = persistent_lockfile
            .filter(|path| path.is_file())
            .map(fs::read_to_string)
            .transpose()?;
        let plan = render_lock_plan(previous_lock.as_deref(), &generated_lock)?;
        if matches!(
            action,
            CargoAction::FetchPlan | CargoAction::FetchUpdatePlan
        ) {
            return Ok(CargoBuildResult {
                success: true,
                timed_out: false,
                exit_code: Some(0),
                diagnostics: Vec::new(),
                program_output: plan.lines().map(str::to_owned).collect(),
                stderr: String::new(),
            });
        }

        let output = oxido_cargo::run(
            workspace.path(),
            &[OsString::from("fetch"), OsString::from("--locked")],
        )?;
        if output.success
            && let Some(persistent_lockfile) = persistent_lockfile
            && (update_lock || !persistent_lockfile.exists())
        {
            persist_lockfile(&lock_path, persistent_lockfile, update_lock)?;
        }
        let mut program_output = plan.lines().map(str::to_owned).collect::<Vec<_>>();
        program_output.extend(output.non_json_stdout);
        return Ok(CargoBuildResult {
            success: output.success,
            timed_out: output.timed_out,
            exit_code: output.exit_code,
            diagnostics: Vec::new(),
            program_output,
            stderr: output.stderr,
        });
    }
    if !lock_path.is_file() {
        if persistent_lockfile.is_some() && !dependencies.is_empty() {
            return Ok(CargoBuildResult {
                success: false,
                timed_out: false,
                exit_code: None,
                diagnostics: vec![MappedCargoDiagnostic {
                    level: "error".into(),
                    code: Some("OX7001".into()),
                    message: "falta Cargo.lock para las dependencias del proyecto; ejecuta `oxido descargar-dependencias <archivo.ox>`, revisa el plan y luego usa `--confirmar`".into(),
                    source: None,
                }],
                program_output: Vec::new(),
                stderr: String::new(),
            });
        }
        let lock = oxido_cargo::run(
            workspace.path(),
            &[
                OsString::from("generate-lockfile"),
                OsString::from("--offline"),
            ],
        )?;
        if !lock.success {
            return Ok(CargoBuildResult {
                success: false,
                timed_out: lock.timed_out,
                exit_code: lock.exit_code,
                diagnostics: Vec::new(),
                program_output: lock.non_json_stdout,
                stderr: lock.stderr,
            });
        }
    }

    let args = match action {
        CargoAction::Check | CargoAction::CheckTrusted => {
            vec!["check", "--locked", "--offline", "--message-format=json"]
        }
        CargoAction::Run | CargoAction::RunTrusted => vec![
            "run",
            "--locked",
            "--offline",
            "--message-format=json",
            "--quiet",
        ],
        CargoAction::FetchPlan
        | CargoAction::Fetch
        | CargoAction::FetchUpdatePlan
        | CargoAction::FetchUpdate => {
            return Err(io::Error::other(
                "la acción fetch debió terminar antes de compilar",
            ));
        }
    }
    .into_iter()
    .map(OsString::from)
    .collect::<Vec<_>>();
    let output = oxido_cargo::run(workspace.path(), &args)?;
    Ok(CargoBuildResult {
        success: output.success,
        timed_out: output.timed_out,
        exit_code: output.exit_code,
        diagnostics: parse_cargo_diagnostics(&output.messages, source_map),
        program_output: output.non_json_stdout,
        stderr: output.stderr,
    })
}

fn persist_lockfile(generated: &Path, destination: &Path, replace: bool) -> io::Result<()> {
    let generated_metadata = fs::metadata(generated)?;
    if generated_metadata.len() > MAX_LOCKFILE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Cargo.lock supera el límite de 16 MiB",
        ));
    }
    let sequence = NEXT_LOCKFILE.fetch_add(1, Ordering::Relaxed);
    let parent = destination.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "Cargo.lock no tiene directorio padre",
        )
    })?;
    let staged = parent.join(format!(
        ".Cargo.lock.oxido-{}-{sequence}.tmp",
        std::process::id()
    ));
    let mut input = fs::File::open(generated)?;
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&staged)?;
    let write_result = io::copy(&mut input, &mut output)
        .map(|_| ())
        .and_then(|()| output.sync_all());
    if let Err(error) = write_result {
        return match fs::remove_file(&staged) {
            Ok(()) => Err(error),
            Err(cleanup_error) => Err(io::Error::other(format!(
                "no se pudo preparar Cargo.lock: {error}; tampoco se pudo retirar el archivo parcial: {cleanup_error}"
            ))),
        };
    }
    if !replace {
        fs::hard_link(&staged, destination)?;
        return fs::remove_file(staged);
    }
    let backup = parent.join(format!(
        ".Cargo.lock.oxido-{}-{sequence}.bak",
        std::process::id()
    ));
    match fs::rename(&staged, destination) {
        Ok(()) => Ok(()),
        Err(rename_error) if destination.exists() => {
            fs::rename(destination, &backup)?;
            match fs::rename(&staged, destination) {
                Ok(()) => fs::remove_file(backup),
                Err(replace_error) => match fs::rename(&backup, destination) {
                    Ok(()) => Err(io::Error::other(format!(
                        "no se pudo reemplazar Cargo.lock (primer intento: {rename_error}; segundo intento: {replace_error}); se restauró el lockfile anterior"
                    ))),
                    Err(restore_error) => Err(io::Error::other(format!(
                        "no se pudo reemplazar Cargo.lock: {replace_error}; tampoco se pudo restaurar el anterior: {restore_error}"
                    ))),
                },
            }
        }
        Err(error) => match fs::remove_file(&staged) {
            Ok(()) => Err(error),
            Err(cleanup_error) => Err(io::Error::other(format!(
                "no se pudo instalar Cargo.lock: {error}; tampoco se pudo retirar el temporal: {cleanup_error}"
            ))),
        },
    }
}

struct Workspace(PathBuf);

impl Workspace {
    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            eprintln!(
                "no se pudo retirar el workspace temporal `{}`: {error}",
                self.0.display()
            );
        }
    }
}

fn create_workspace(
    rust: &str,
    source_map: &SourceMap,
    dependencies: &[crate::config::ResolvedDependency],
    persistent_lockfile: Option<&Path>,
    read_lockfile: bool,
) -> io::Result<Workspace> {
    let temp = fs::canonicalize(std::env::temp_dir())?;
    let root = temp.join("oxido-builds");
    if let Ok(metadata) = fs::symlink_metadata(&root)
        && metadata.file_type().is_symlink()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "el directorio temporal de Óxido no puede ser un enlace simbólico",
        ));
    }
    fs::create_dir_all(&root)?;
    let canonical_root = fs::canonicalize(&root)?;
    if !canonical_root.starts_with(&temp) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "el directorio temporal de Óxido sale de la raíz temporal",
        ));
    }
    let workspace = loop {
        let sequence = NEXT_WORKSPACE.fetch_add(1, Ordering::Relaxed);
        let candidate = canonical_root.join(format!("build-{}-{sequence}", std::process::id()));
        match fs::create_dir(&candidate) {
            Ok(()) => break Workspace(candidate),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    };
    let src = workspace.path().join("src");
    fs::create_dir(&src)?;
    fs::write(
        workspace.path().join("Cargo.toml"),
        generated_manifest(dependencies)?,
    )?;
    if let Some(lockfile) = persistent_lockfile {
        match fs::symlink_metadata(lockfile) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Cargo.lock debe ser un archivo regular, no un enlace simbólico",
                ));
            }
            Ok(metadata) if metadata.len() > MAX_LOCKFILE_BYTES => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Cargo.lock supera el límite de 16 MiB",
                ));
            }
            Ok(_) if read_lockfile => {
                fs::copy(lockfile, workspace.path().join("Cargo.lock"))?;
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    fs::write(src.join("main.rs"), rust)?;
    let mut source_map = source_map.clone();
    source_map.set_generated_file(Some(Path::new("main.rs")));
    let map = serde_json::to_vec_pretty(&source_map)
        .map_err(|error| io::Error::other(error.to_string()))?;
    fs::write(workspace.path().join("main.oxido-map.json"), map)?;
    Ok(workspace)
}

fn generated_manifest(dependencies: &[crate::config::ResolvedDependency]) -> io::Result<String> {
    let mut package = toml::map::Map::new();
    package.insert("name".into(), toml::Value::String("oxido_generated".into()));
    package.insert("version".into(), toml::Value::String("0.1.0".into()));
    package.insert("edition".into(), toml::Value::String("2024".into()));

    let mut dependency_table = toml::map::Map::new();
    for dependency in dependencies {
        let mut declaration = toml::map::Map::new();
        if let Some(path) = &dependency.canonical_path {
            let path = path.to_str().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "la ruta de `{}` no es UTF-8 y no puede escribirse en Cargo.toml",
                        dependency.name
                    ),
                )
            })?;
            declaration.insert("path".into(), toml::Value::String(path.to_owned()));
        }
        if let Some(version) = &dependency.version {
            declaration.insert("version".into(), toml::Value::String(version.clone()));
        }
        if !dependency.features.is_empty() {
            declaration.insert(
                "features".into(),
                toml::Value::Array(
                    dependency
                        .features
                        .iter()
                        .cloned()
                        .map(toml::Value::String)
                        .collect(),
                ),
            );
        }
        if !dependency.default_features {
            declaration.insert("default-features".into(), toml::Value::Boolean(false));
        }
        dependency_table.insert(dependency.name.clone(), toml::Value::Table(declaration));
    }

    let mut manifest = toml::map::Map::new();
    manifest.insert("package".into(), toml::Value::Table(package));
    manifest.insert("dependencies".into(), toml::Value::Table(dependency_table));
    manifest.insert(
        "workspace".into(),
        toml::Value::Table(toml::map::Map::new()),
    );
    toml::to_string(&toml::Value::Table(manifest))
        .map_err(|error| io::Error::other(format!("no se pudo serializar Cargo.toml: {error}")))
}

fn parse_cargo_diagnostics(
    messages: &[Value],
    source_map: &SourceMap,
) -> Vec<MappedCargoDiagnostic> {
    messages
        .iter()
        .filter(|event| event["reason"] == "compiler-message")
        .filter_map(|event| event.get("message"))
        .filter_map(|message| {
            let level = message.get("level")?.as_str()?.to_owned();
            let text = message.get("message")?.as_str()?;
            let code = message
                .get("code")
                .and_then(|value| value.get("code"))
                .and_then(Value::as_str)
                .map(str::to_owned);
            let source = message
                .get("spans")
                .and_then(Value::as_array)
                .and_then(|spans| {
                    spans
                        .iter()
                        .find(|span| span["is_primary"].as_bool() == Some(true))
                        .or_else(|| spans.first())
                })
                .and_then(|span| {
                    let line = span.get("line_start")?.as_u64()? as usize;
                    let column = span.get("column_start")?.as_u64()? as usize;
                    source_map.map_generated(line, column)
                });
            let translated_message = translate_rustc_message(code.as_deref(), text);
            Some(MappedCargoDiagnostic {
                level,
                code,
                message: translated_message,
                source,
            })
        })
        .collect()
}

fn translate_rustc_message(code: Option<&str>, message: &str) -> String {
    let translated = match code {
        Some("E0308") => "tipos incompatibles",
        Some("E0425") => "no se encuentra el nombre",
        Some("E0599") => "no existe ese método o elemento para este tipo",
        Some("E0382") => "uso de un valor que ya se movió",
        Some("E0502") => "conflicto entre préstamos mutables e inmutables",
        Some("E0507") => "no se puede mover un valor prestado",
        Some("E0277") => "no se cumple un requisito de trait",
        Some("E0433") => "no se encuentra el módulo o crate",
        Some("E0061") => "cantidad incorrecta de argumentos",
        Some("E0063") => "faltan campos obligatorios",
        Some("E0004") => "la coincidencia de patrones no cubre todos los casos",
        Some("E0106") => "falta una duración de vida en esta referencia",
        _ => return message.to_owned(),
    };
    let details = message
        .split_once('\n')
        .map(|(_, details)| format!("\n{details}"))
        .unwrap_or_default();
    format!("{translated}{details}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxido_rust_backend::SpanMapping;

    #[test]
    fn maps_cargo_json_spans_and_preserves_unmapped_diagnostics() {
        let map = SourceMap::new(vec![SpanMapping {
            source_start: 30,
            source_end: 40,
            source_start_line: 3,
            source_start_column: 5,
            source_end_line: 3,
            source_end_column: 15,
            generated_start_line: 2,
            generated_start_column: 4,
            generated_end_line: 2,
            generated_end_column: 14,
        }]);
        let event = serde_json::json!({
            "reason": "compiler-message",
            "message": {
                "level": "error",
                "message": "mismatched types",
                "code": {"code": "E0308"},
                "spans": [{
                    "is_primary": true,
                    "line_start": 2,
                    "column_start": 5
                }]
            }
        });
        let diagnostics = parse_cargo_diagnostics(&[event], &map);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code.as_deref(), Some("E0308"));
        assert_eq!(diagnostics[0].source.unwrap().start, 30);
        assert_eq!(diagnostics[0].message, "tipos incompatibles");
    }

    #[test]
    fn translates_known_rustc_codes_and_preserves_unknown_messages() {
        assert_eq!(
            translate_rustc_message(Some("E0599"), "no method named `x`"),
            "no existe ese método o elemento para este tipo"
        );
        assert_eq!(
            translate_rustc_message(Some("E9999"), "future diagnostic"),
            "future diagnostic"
        );
    }
}
