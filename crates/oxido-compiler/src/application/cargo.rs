use super::source_map::{SourceLocation, SourceMap};
use serde_json::Value;
use std::{
    ffi::OsString,
    fs, io,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_WORKSPACE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CargoAction {
    Check,
    Run,
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

pub fn build_generated(
    rust: &str,
    source_map: &SourceMap,
    action: CargoAction,
) -> io::Result<CargoBuildResult> {
    build_generated_with_dependencies(rust, source_map, &[], action)
}

pub fn build_generated_with_dependencies(
    rust: &str,
    source_map: &SourceMap,
    dependencies: &[crate::config::ResolvedDependency],
    action: CargoAction,
) -> io::Result<CargoBuildResult> {
    let workspace = create_workspace(rust, source_map, dependencies)?;
    let cargo_home = workspace.path().join("cargo-home");
    fs::create_dir(&cargo_home)?;
    let lock = oxido_cargo::run_with_cargo_home(
        workspace.path(),
        &[
            OsString::from("generate-lockfile"),
            OsString::from("--offline"),
        ],
        Some(&cargo_home),
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

    let args = match action {
        CargoAction::Check => vec!["check", "--locked", "--offline", "--message-format=json"],
        CargoAction::Run => vec![
            "run",
            "--locked",
            "--offline",
            "--message-format=json",
            "--quiet",
        ],
    }
    .into_iter()
    .map(OsString::from)
    .collect::<Vec<_>>();
    let output = oxido_cargo::run_with_cargo_home(workspace.path(), &args, Some(&cargo_home))?;
    Ok(CargoBuildResult {
        success: output.success,
        timed_out: output.timed_out,
        exit_code: output.exit_code,
        diagnostics: parse_cargo_diagnostics(&output.messages, source_map),
        program_output: output.non_json_stdout,
        stderr: output.stderr,
    })
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
        let path = dependency.canonical_path.to_str().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "la ruta de `{}` no es UTF-8 y no puede escribirse en Cargo.toml",
                    dependency.name
                ),
            )
        })?;
        let mut declaration = toml::map::Map::new();
        declaration.insert("path".into(), toml::Value::String(path.to_owned()));
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
