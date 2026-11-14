#![forbid(unsafe_code)]

mod version;

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    env,
    ffi::{OsStr, OsString},
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::ExitCode,
    time::{SystemTime, UNIX_EPOCH},
};

use oxido_compiler::{application::cargo::CargoAction, generate_ox};
use oxido_language::{
    SourceFile,
    domain::{
        diagnostics::Diagnostic,
        syntax::{LexOptions, SyntaxMode},
    },
};
use sha2::{Digest, Sha256};

mod project_input;

const MAX_SOURCE_BYTES: u64 = 4 * 1024 * 1024;
const MAX_IMPORT_FILES: usize = 100_000;
const MAX_IMPORT_TOTAL_BYTES: u64 = 1024 * 1024 * 1024;
const STRUCTURED_ERROR_PREFIX: &str = "\u{1e}oxido-json:";

fn main() -> ExitCode {
    let args = env::args_os().skip(1).collect::<Vec<_>>();
    let json_requested = args.iter().any(|argument| argument == "--json");
    match run(args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            if json_requested {
                if let Some(json) = message.strip_prefix(STRUCTURED_ERROR_PREFIX) {
                    println!("{json}");
                } else {
                    println!(
                        "{}",
                        serde_json::json!({
                            "schemaVersion": 1,
                            "success": false,
                            "diagnostics": [{
                                "code": "OX0001",
                                "severity": "error",
                                "message": sanitize_report_text(&message, None),
                                "file": null,
                                "line": null,
                                "column": null
                            }],
                            "output": []
                        })
                    );
                }
            } else {
                eprintln!("oxido: {message}");
            }
            ExitCode::FAILURE
        }
    }
}

fn run(args: Vec<OsString>) -> Result<(), String> {
    let Some(command) = args.first().and_then(|value| value.to_str()) else {
        return Err(usage().into());
    };
    if args.len() == 1 && matches!(command, "--version" | "-V") {
        println!("{}", version::version_line());
        return Ok(());
    }
    match command {
        "analizar-rust" => analyze_rust(&args[1..]),
        "aliases" => print_aliases(&args[1..]),
        "auditar-unsafe" => audit_unsafe(&args[1..]),
        "auditar-proyecto" => audit_project(&args[1..]),
        "exportar" => export_ox(&args[1..]),
        "importar" => import_rust(&args[1..]),
        "importar-proyecto" => import_project(&args[1..]),
        "check" | "verificar" => cargo_ox(&args[1..], CargoAction::Check),
        "build" | "compilar" => cargo_ox(&args[1..], CargoAction::Build),
        "run" | "ejecutar" => cargo_ox(&args[1..], CargoAction::Run),
        "test" | "probar" => cargo_ox(&args[1..], CargoAction::Test),
        "nuevo" => new_project(&args[1..]),
        "formato" => format_ox(&args[1..]),
        "descargar-dependencias" => fetch_dependencies(&args[1..]),
        _ => Err(usage().into()),
    }
}

fn print_aliases(args: &[OsString]) -> Result<(), String> {
    if args != [OsStr::new("--markdown")] {
        return Err("uso: oxido aliases --markdown".into());
    }
    print!(
        "{}",
        oxido_language::domain::syntax::render_alias_markdown()
    );
    Ok(())
}

fn audit_project(args: &[OsString]) -> Result<(), String> {
    let (root, reject) = match args {
        [root] => (root, false),
        [root, flag] if flag == "--rechazar" => (root, true),
        _ => return Err("uso: oxido auditar-proyecto <directorio> [--rechazar]".into()),
    };
    let root = fs::canonicalize(PathBuf::from(root))
        .map_err(|error| format!("no se pudo resolver el proyecto: {error}"))?;
    if !root.is_dir() {
        return Err("la ruta del proyecto debe ser un directorio".into());
    }
    let mut pending = VecDeque::from([root.clone()]);
    let mut files_seen = 0usize;
    let mut unsafe_count = 0usize;
    let mut build_scripts = Vec::new();
    let mut proc_macro_crates = Vec::new();
    let mut external_calls = Vec::new();
    let mut ffi_blocks = Vec::new();
    while let Some(directory) = pending.pop_front() {
        for entry in fs::read_dir(&directory)
            .map_err(|error| format!("no se pudo leer el proyecto: {error}"))?
        {
            files_seen = files_seen.saturating_add(1);
            if files_seen > 100_000 {
                return Err("la auditoría supera el límite de 100000 entradas".into());
            }
            let entry = entry.map_err(|error| format!("no se pudo leer una entrada: {error}"))?;
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path)
                .map_err(|error| format!("no se pudo inspeccionar una entrada: {error}"))?;
            if metadata.file_type().is_symlink() {
                continue;
            }
            if metadata.is_dir() {
                if matches!(
                    path.file_name().and_then(OsStr::to_str),
                    Some(".git" | "target" | "node_modules" | "dist" | ".astro")
                ) {
                    continue;
                }
                pending.push_back(path);
                continue;
            }
            if !metadata.is_file() {
                continue;
            }
            if path.file_name().is_some_and(|name| name == "build.rs") {
                build_scripts.push(relative_display(&root, &path));
            }
            if path.file_name().is_some_and(|name| name == "Cargo.toml") {
                if metadata.len() > 1024 * 1024 {
                    return Err("un Cargo.toml supera el límite de 1 MiB".into());
                }
                let manifest = fs::read_to_string(&path)
                    .map_err(|error| format!("no se pudo leer un Cargo.toml: {error}"))?;
                let value = toml::from_str::<toml::Value>(&manifest).map_err(|error| {
                    format!(
                        "Cargo.toml inválido en {}: {error}",
                        relative_display(&root, &path)
                    )
                })?;
                if value
                    .get("lib")
                    .and_then(|lib| lib.get("proc-macro"))
                    .and_then(toml::Value::as_bool)
                    == Some(true)
                {
                    proc_macro_crates.push(relative_display(&root, &path));
                }
            }
            let is_source = matches!(path.extension().and_then(OsStr::to_str), Some("rs" | "ox"));
            if !is_source {
                continue;
            }
            if metadata.len() > MAX_SOURCE_BYTES {
                return Err(format!(
                    "{} supera el límite de 4 MiB",
                    relative_display(&root, &path)
                ));
            }
            let text = fs::read_to_string(&path).map_err(|error| {
                format!(
                    "no se pudo leer {}: {error}",
                    relative_display(&root, &path)
                )
            })?;
            let source = SourceFile::with_path(&path, text);
            let lexed = oxido_language::domain::syntax::lex_with_deadline(
                &source,
                LexOptions {
                    project_mode: Some(SyntaxMode::Braces),
                },
                oxido_language::domain::syntax::analysis_deadline(),
            );
            if lexed.has_errors() {
                return Err(format_diagnostics(&source, &path, &lexed.diagnostics));
            }
            let tokens = lexed.tokens;
            for (index, token) in tokens.iter().enumerate() {
                if matches!(
                    token.kind,
                    oxido_language::domain::syntax::TokenKind::Keyword {
                        rust_keyword: "unsafe",
                        ..
                    }
                ) {
                    unsafe_count = unsafe_count.saturating_add(1);
                }
                if text_at(&source, token) == Some("extern")
                    && tokens.get(index + 1).is_some_and(|next| {
                        matches!(
                            next.kind,
                            oxido_language::domain::syntax::TokenKind::StringLiteral
                        )
                    })
                    && tokens
                        .get(index + 2)
                        .is_some_and(|next| text_at(&source, next) == Some("{"))
                {
                    ffi_blocks.push(relative_display(&root, &path));
                }
                if text_at(&source, token) == Some("Command")
                    && (tokens
                        .get(index + 1)
                        .is_some_and(|next| text_at(&source, next) == Some("::"))
                        && tokens
                            .get(index + 2)
                            .is_some_and(|next| text_at(&source, next) == Some("new")))
                {
                    external_calls.push(relative_display(&root, &path));
                }
            }
        }
    }
    println!(
        "Proyecto: {}",
        root.file_name()
            .and_then(OsStr::to_str)
            .unwrap_or("proyecto")
    );
    println!("Tokens unsafe/inseguro: {unsafe_count}");
    print_findings("build.rs", &build_scripts);
    print_findings(
        "Crates con proc-macro=true en sus manifiestos",
        &proc_macro_crates,
    );
    print_findings("Bloques FFI detectados", &ffi_blocks);
    print_findings("Llamadas Command::new detectadas", &external_calls);
    println!(
        "Alcance: análisis estático; no descubre expansiones macro ni prueba ausencia de riesgos."
    );
    if reject && unsafe_count > 0 {
        return Err(format!(
            "la política --rechazar encontró {unsafe_count} uso(s) de unsafe"
        ));
    }
    Ok(())
}

fn text_at<'a>(
    source: &'a SourceFile,
    token: &oxido_language::domain::syntax::Token,
) -> Option<&'a str> {
    source.text().get(token.span.start()..token.span.end())
}

fn relative_display(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}

fn print_findings(label: &str, paths: &[String]) {
    println!("{label}: {}", paths.len());
    for path in paths {
        println!("  {path}");
    }
}

fn fetch_dependencies(args: &[OsString]) -> Result<(), String> {
    let Some(input) = args.first() else {
        return Err("uso: oxido descargar-dependencias <archivo.ox> [--actualizar] [--confirmar] [--permitir-rutas-locales]".into());
    };
    let mut update = false;
    let mut confirm = false;
    let mut allow_paths = false;
    for argument in &args[1..] {
        match argument.to_str() {
            Some("--actualizar") if !update => update = true,
            Some("--confirmar") if !confirm => confirm = true,
            Some("--permitir-rutas-locales") if !allow_paths => allow_paths = true,
            _ => {
                return Err("uso: oxido descargar-dependencias <archivo.ox> [--actualizar] [--confirmar] [--permitir-rutas-locales]".into());
            }
        }
    }
    let (source, display_path) = load_ox(input)?;
    let settings = project_settings(&display_path)?;
    if !allow_paths
        && settings
            .dependencies
            .iter()
            .any(|dependency| dependency.canonical_path.is_some())
    {
        return Err("descargar-dependencias acepta por defecto solo crates de crates.io; para incluir dependencias path revisa el proyecto y pasa --permitir-rutas-locales".into());
    }
    let lockfile = settings.root.join("Cargo.lock");
    let plan_action = if update {
        CargoAction::FetchUpdatePlan
    } else {
        CargoAction::FetchPlan
    };
    let plan = oxido_compiler::build_ox_in_project(
        &source,
        settings.options,
        &settings.dependencies,
        Some(&lockfile),
        plan_action,
    )
    .map_err(|diagnostics| format_diagnostics(&source, &display_path, &diagnostics))?;
    print_fetch_result(&plan)?;
    if !confirm {
        println!(
            "Vista previa solamente. Revisa el plan; repite el comando con --confirmar para descargar y guardar Cargo.lock."
        );
        return Ok(());
    }
    let action = match update {
        true => CargoAction::FetchUpdate,
        false => CargoAction::Fetch,
    };
    let result = oxido_compiler::build_ox_project_in_project(
        &source,
        settings.options,
        &settings.dependencies,
        Some(&lockfile),
        action,
        &settings.root,
    )
    .map_err(|diagnostics| format_diagnostics(&source, &display_path, &diagnostics))?;
    if !result.stderr.trim().is_empty() {
        eprint!("{}", result.stderr);
    }
    if !result.success {
        if result.timed_out {
            return Err("Cargo superó el límite de ejecución de 120 segundos".into());
        }
        return Err(format!(
            "Cargo terminó con estado {}",
            result
                .exit_code
                .map_or_else(|| "sin código de salida".into(), |code| code.to_string())
        ));
    }
    println!("Dependencias descargadas; Cargo.lock guardado en el proyecto.");
    Ok(())
}

fn imported_workspace_root(source: &Path) -> Result<Option<PathBuf>, String> {
    let source = fs::canonicalize(source)
        .map_err(|error| format!("no se pudo resolver {}: {error}", source.display()))?;
    let start = if source.is_dir() {
        source.as_path()
    } else {
        source.parent().unwrap_or(source.as_path())
    };
    for ancestor in start.ancestors() {
        let marker = ancestor.join(".oxido/import-manifest.json");
        let metadata = match fs::symlink_metadata(&marker) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(format!(
                    "no se pudo inspeccionar el manifiesto importado: {error}"
                ));
            }
        };
        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || metadata.len() > 16 * 1024 * 1024
        {
            return Err("el manifiesto de importación no es un archivo regular válido".into());
        }
        let bytes = fs::read(&marker)
            .map_err(|error| format!("no se pudo leer el manifiesto de importación: {error}"))?;
        let manifest: serde_json::Value = serde_json::from_slice(&bytes)
            .map_err(|error| format!("manifiesto de importación inválido: {error}"))?;
        if manifest["kind"] == "oxido-lossless-workspace-import" {
            return Ok(Some(ancestor.to_path_buf()));
        }
    }
    Ok(None)
}

fn print_fetch_result(
    result: &oxido_compiler::application::cargo::CargoBuildResult,
) -> Result<(), String> {
    if let Some(diagnostic) = result.diagnostics.first() {
        return Err(format!(
            "[{}] {}",
            diagnostic.code.as_deref().unwrap_or("OX0001"),
            diagnostic.message
        ));
    }
    if !result.stderr.trim().is_empty() {
        eprint!("{}", result.stderr);
    }
    if !result.success {
        if result.timed_out {
            return Err("Cargo superó el límite de ejecución de 120 segundos".into());
        }
        return Err(format!(
            "Cargo terminó con estado {}",
            result
                .exit_code
                .map_or_else(|| "sin código de salida".into(), |code| code.to_string())
        ));
    }
    for line in &result.program_output {
        println!("{line}");
    }
    Ok(())
}

fn audit_unsafe(args: &[OsString]) -> Result<(), String> {
    let (path, reject) = match args {
        [path] => (PathBuf::from(path), false),
        [path, flag] if flag == "--rechazar" => (PathBuf::from(path), true),
        _ => return Err("uso: oxido auditar-unsafe <archivo.ox|archivo.rs> [--rechazar]".into()),
    };
    if !matches!(path.extension(), Some(extension) if extension == "ox" || extension == "rs") {
        return Err("el archivo debe terminar en .ox o .rs".into());
    }
    let canonical = fs::canonicalize(&path)
        .map_err(|error| format!("no se pudo resolver {}: {error}", path.display()))?;
    let metadata = fs::metadata(&canonical)
        .map_err(|error| format!("no se pudo inspeccionar {}: {error}", path.display()))?;
    if metadata.len() > MAX_SOURCE_BYTES {
        return Err("la fuente supera el límite de 4 MiB".into());
    }
    let text = fs::read_to_string(&canonical)
        .map_err(|error| format!("no se pudo leer {}: {error}", path.display()))?;
    let source = SourceFile::with_path(path.clone(), text);
    let lexed = oxido_language::domain::syntax::lex_with_deadline(
        &source,
        LexOptions {
            project_mode: Some(SyntaxMode::Braces),
        },
        oxido_language::domain::syntax::analysis_deadline(),
    );
    if lexed.has_errors() {
        return Err(format_diagnostics(&source, &path, &lexed.diagnostics));
    }
    let findings = lexed
        .tokens
        .iter()
        .filter(|token| {
            matches!(
                token.kind,
                oxido_language::domain::syntax::TokenKind::Keyword {
                    rust_keyword: "unsafe",
                    ..
                }
            )
        })
        .filter_map(|token| {
            source
                .position(token.span.start())
                .map(|position| (position.line, position.column))
        })
        .collect::<Vec<_>>();
    let display_name = path.file_name().and_then(OsStr::to_str).unwrap_or("fuente");
    if findings.is_empty() {
        println!("No se encontraron tokens `unsafe` en {display_name}.");
        return Ok(());
    }
    for (line, column) in &findings {
        println!("{display_name}:{line}:{column}: token `unsafe`");
    }
    if reject {
        return Err(format!(
            "la política --rechazar encontró {} uso(s) de unsafe",
            findings.len()
        ));
    }
    println!(
        "Informe estático: {} token(s); no demuestra ausencia de riesgo.",
        findings.len()
    );
    Ok(())
}

fn analyze_rust(args: &[OsString]) -> Result<(), String> {
    let [path] = args else {
        return Err("uso: oxido analizar-rust <archivo.rs>".into());
    };
    let path = PathBuf::from(path);
    let text = fs::read_to_string(&path)
        .map_err(|error| format!("no se pudo leer {}: {error}", path.display()))?;
    let source = SourceFile::with_path(path, text);
    let result = oxido_compiler::analyze_rust(&source)?;
    println!("Sintaxis Rust válida ({} items).", result.item_count);
    Ok(())
}

fn new_project(args: &[OsString]) -> Result<(), String> {
    let [directory] = args else {
        return Err("uso: oxido nuevo <directorio>".into());
    };
    let root = PathBuf::from(directory);
    if root.exists() {
        return Err(format!("el directorio ya existe: {}", root.display()));
    }
    fs::create_dir_all(root.join("src"))
        .map_err(|error| format!("no se pudo crear el proyecto: {error}"))?;
    fs::write(
        root.join("oxido.toml"),
        "[language]\nsyntax = \"indentation\"\nedition = \"2024\"\n",
    )
    .map_err(|error| format!("no se pudo crear oxido.toml: {error}"))?;
    fs::write(
        root.join("src/main.ox"),
        "sintaxis indentacion\nfuncion principal():\n    imprimir!(\"Hola, Óxido.\")\n",
    )
    .map_err(|error| format!("no se pudo crear src/main.ox: {error}"))?;
    println!("Proyecto Óxido creado en {}", root.display());
    Ok(())
}

fn format_ox(args: &[OsString]) -> Result<(), String> {
    let mut input = None;
    let mut check = false;
    for argument in args {
        if argument == "--comprobar" && !check {
            check = true;
        } else if argument.to_string_lossy().starts_with('-') || input.is_some() {
            return Err("uso: oxido formato [<archivo.ox|directorio>] [--comprobar]".into());
        } else {
            input = Some(argument.as_os_str());
        }
    }
    let input = project_input::resolve(input)?;
    let (source, path) = load_ox(input.as_os_str())?;
    let settings = project_settings(&path)?;
    let lexed = oxido_language::domain::syntax::lex_with_deadline(
        &source,
        settings.options,
        oxido_language::domain::syntax::analysis_deadline(),
    );
    let imported = match lexed.mode {
        Some(SyntaxMode::Indentation) => {
            let formatted =
                oxido_compiler::application::format::format_indented_source(&source, &lexed);
            let formatted_source = SourceFile::with_path(&path, &formatted);
            generate_ox(&formatted_source, settings.options).map_err(|diagnostics| {
                format_diagnostics(&formatted_source, &path, &diagnostics)
            })?;
            formatted
        }
        Some(SyntaxMode::Braces) => {
            let generated = generate_ox(&source, settings.options)
                .map_err(|diagnostics| format_diagnostics(&source, &path, &diagnostics))?;
            oxido_compiler::import_rust(&SourceFile::new(generated.rust))?
        }
        None => return Err("no se pudo determinar el modo de sintaxis del archivo".into()),
    };
    if check {
        if imported != source.text() {
            return Err(
                "el archivo no está formateado; ejecuta `oxido formato <archivo.ox>`".into(),
            );
        }
        println!("{} está formateado.", path.display());
    } else {
        fs::write(&path, imported)
            .map_err(|error| format!("no se pudo escribir {}: {error}", path.display()))?;
        println!("Óxido formateado: {}", path.display());
    }
    Ok(())
}

fn export_ox(args: &[OsString]) -> Result<(), String> {
    let [input, flag, output] = args else {
        return Err("uso: oxido exportar <archivo.ox> --salida <archivo.rs>".into());
    };
    if flag != "--salida" {
        return Err("se esperaba --salida <archivo.rs>".into());
    }
    let (source, display_path) = load_ox(input)?;
    let settings = project_settings(&display_path)?;
    let output_path = PathBuf::from(output);
    if output_path.extension() != Some(OsStr::new("rs")) {
        return Err("la ruta de salida debe terminar en .rs".into());
    }
    validate_output_parent(&output_path)?;
    let generated = generate_ox(&source, settings.options)
        .map_err(|diagnostics| format_diagnostics(&source, &display_path, &diagnostics))?;
    if !generated.diagnostics.is_empty() {
        eprintln!(
            "{}",
            format_diagnostics(&source, &display_path, &generated.diagnostics)
        );
    }
    let resolved = oxido_compiler::application::modules::resolve_project_modules(
        &source,
        &generated.rust,
        settings.options,
        &settings.root,
    )?;
    let output_root = output_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let output_root = fs::canonicalize(output_root)
        .map_err(|error| format!("no se pudo resolver el directorio de salida: {error}"))?;
    let root_name = output_path
        .file_name()
        .ok_or_else(|| "el archivo de salida no tiene nombre".to_owned())?;
    let root_destination = output_root.join(root_name);
    let root_map_path = root_destination.with_extension("oxido-map.json");
    let mut outputs = Vec::with_capacity(2 + resolved.modules.len() * 2);
    let mut root_source_map = generated.source_map;
    let root_source_relative = fs::canonicalize(&display_path)
        .ok()
        .and_then(|path| {
            path.strip_prefix(&settings.root)
                .ok()
                .map(Path::to_path_buf)
        })
        .map(|path| path.to_string_lossy().into_owned());
    if let Some(relative) = root_source_relative {
        root_source_map.set_source_relative_path(relative);
    }
    root_source_map.set_generated_relative_path(root_name.to_string_lossy().into_owned());
    outputs.push((root_destination.clone(), resolved.root_rust.into_bytes()));
    outputs.push((
        root_map_path.clone(),
        serde_json::to_vec_pretty(&root_source_map)
            .map_err(|error| format!("no se pudo serializar el mapa: {error}"))?,
    ));
    for module in resolved.modules {
        validate_module_output_path(&module.path)?;
        let destination = output_root.join(&module.path);
        let map_path = destination.with_extension("oxido-map.json");
        outputs.push((destination, module.rust.into_bytes()));
        if let Some(mut source_map) = module.source_map {
            source_map.set_generated_relative_path(module.path.to_string_lossy().into_owned());
            outputs.push((
                map_path,
                serde_json::to_vec_pretty(&source_map)
                    .map_err(|error| format!("no se pudo serializar el mapa: {error}"))?,
            ));
        }
    }
    preflight_export_outputs(&output_root, &outputs)?;
    let mut created_directories = Vec::new();
    let mut created_files = Vec::new();
    for (path, contents) in &outputs {
        let relative_parent = path
            .parent()
            .and_then(|parent| parent.strip_prefix(&output_root).ok())
            .ok_or_else(|| "una salida recursiva escapa del directorio de destino".to_owned())?;
        if let Err(error) =
            create_output_directories(&output_root, relative_parent, &mut created_directories)
        {
            cleanup_export_paths(&created_files, &created_directories)?;
            return Err(error);
        }
        if let Err(error) = write_new(path, contents) {
            cleanup_export_paths(&created_files, &created_directories)?;
            return Err(error);
        }
        created_files.push(path.clone());
    }
    for path in created_files {
        let display_path = path.strip_prefix(&output_root).unwrap_or(&path);
        println!("Rust exportado: {}", display_path.display());
    }
    Ok(())
}

fn cleanup_export_paths(files: &[PathBuf], directories: &[PathBuf]) -> Result<(), String> {
    let mut failures = Vec::new();
    for path in files.iter().rev() {
        if let Err(error) = fs::remove_file(path)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            failures.push(format!("{}: {error}", path.display()));
        }
    }
    for path in directories.iter().rev() {
        if let Err(error) = fs::remove_dir(path)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            failures.push(format!("{}: {error}", path.display()));
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "no se pudieron limpiar salidas temporales: {}",
            failures.join("; ")
        ))
    }
}

struct ImportArguments {
    input: PathBuf,
    output: PathBuf,
    spanish: bool,
}

fn parse_import_arguments(
    args: &[OsString],
    usage: &str,
    output_description: &str,
) -> Result<ImportArguments, String> {
    let mut input = None;
    let mut output = None;
    let mut spanish = false;
    let mut index = 0;
    while index < args.len() {
        let argument = &args[index];
        if argument == "--salida" {
            index += 1;
            let value = args
                .get(index)
                .ok_or_else(|| format!("falta la ruta después de --salida\n{usage}"))?;
            if output.replace(value).is_some() {
                return Err(format!("--salida se indicó más de una vez\n{usage}"));
            }
        } else if argument == "--espanol" {
            spanish = true;
        } else if argument.to_string_lossy().starts_with("--") {
            return Err(format!(
                "opción desconocida: {}\n{usage}",
                argument.to_string_lossy()
            ));
        } else if input.replace(argument).is_some() {
            return Err(usage.to_owned());
        }
        index += 1;
    }
    let input = input.ok_or_else(|| usage.to_owned())?;
    let output = output.ok_or_else(|| format!("falta --salida <{output_description}>\n{usage}"))?;
    Ok(ImportArguments {
        input: PathBuf::from(input),
        output: PathBuf::from(output),
        spanish,
    })
}

fn import_rust(args: &[OsString]) -> Result<(), String> {
    let ImportArguments {
        input,
        output,
        spanish,
    } = parse_import_arguments(
        args,
        "uso: oxido importar <archivo.rs> --salida <archivo.ox> [--espanol]",
        "archivo.ox",
    )?;
    if input.extension() != Some(OsStr::new("rs")) {
        return Err("el archivo de entrada debe terminar en .rs".into());
    }
    let metadata = fs::metadata(&input)
        .map_err(|error| format!("no se pudo leer {}: {error}", input.display()))?;
    if metadata.len() > MAX_SOURCE_BYTES {
        return Err("el archivo Rust supera el límite de 4 MiB".into());
    }
    let text = fs::read_to_string(&input)
        .map_err(|error| format!("no se pudo leer {}: {error}", input.display()))?;
    let source = SourceFile::with_path(input, text);
    if output.extension() != Some(OsStr::new("ox")) {
        return Err("la ruta de salida debe terminar en .ox".into());
    }
    validate_output_parent(&output)?;
    if fs::symlink_metadata(&output).is_ok() {
        return Err("el archivo .ox de salida ya existe; elige otra ruta".into());
    }
    let imported = if spanish {
        oxido_compiler::import_rust(&source)?
    } else {
        oxido_compiler::import_literal(&source)
    };
    write_new(&output, imported.as_bytes())?;
    println!("Óxido importado a {}", output.display());
    Ok(())
}

fn import_project(args: &[OsString]) -> Result<(), String> {
    let ImportArguments {
        input,
        output,
        spanish,
    } = parse_import_arguments(
        args,
        "uso: oxido importar-proyecto <directorio-rust> --salida <directorio-oxido> [--espanol]",
        "directorio-oxido",
    )?;
    let input = fs::canonicalize(&input)
        .map_err(|error| format!("no se pudo resolver el árbol Rust: {error}"))?;
    if !input.is_dir() {
        return Err("la entrada debe ser un directorio Rust".into());
    }
    validate_output_parent(&output)?;
    let output_parent = output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let canonical_output_parent = fs::canonicalize(output_parent)
        .map_err(|error| format!("no se pudo resolver el directorio de salida: {error}"))?;
    let output_name = output
        .file_name()
        .ok_or_else(|| "el directorio de salida no tiene nombre".to_owned())?;
    if canonical_output_parent
        .join(output_name)
        .starts_with(&input)
    {
        return Err("la salida de importación no puede estar dentro del árbol de entrada".into());
    }
    match fs::symlink_metadata(&output) {
        Ok(_) => return Err("el directorio de salida ya existe; elige otra ruta".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(format!("no se pudo inspeccionar la salida: {error}"));
        }
    }
    let mut files = Vec::new();
    let mut directories = Vec::new();
    let mut total_bytes = 0_u64;
    collect_project_files(
        &input,
        &input,
        &mut files,
        &mut directories,
        &mut total_bytes,
    )?;
    if files.is_empty() {
        return Err("el árbol no contiene archivos regulares importables".into());
    }
    let rust_sources = files
        .iter()
        .filter(|file| file.kind == ProjectSourceKind::Rust)
        .map(|file| file.relative.clone())
        .collect::<BTreeSet<_>>();
    let ox_sources = files
        .iter()
        .filter(|file| file.kind == ProjectSourceKind::Oxido)
        .map(|file| file.relative.clone())
        .collect::<BTreeSet<_>>();
    let original_paths = files
        .iter()
        .map(|file| file.relative.clone())
        .collect::<BTreeSet<_>>();
    // Las rutas `#[path]` se validan antes de crear la salida. Un atributo que
    // escapa de la raíz es un error fatal, no un fallback silencioso.
    let mut rust_imports = BTreeMap::new();
    for file in files
        .iter()
        .filter(|file| file.kind == ProjectSourceKind::Rust)
    {
        rust_imports.insert(
            file.relative.clone(),
            import_workspace_rust_file(file, &rust_sources, &ox_sources, spanish)?,
        );
    }
    let staging = import_staging_path(&canonical_output_parent, output_name)?;
    fs::create_dir(&staging)
        .map_err(|error| format!("no se pudo crear el staging de importación: {error}"))?;
    let mut created_directories = Vec::new();
    let mut created_files = Vec::new();
    for directory in &directories {
        if let Err(error) = create_output_directories(&staging, directory, &mut created_directories)
        {
            cleanup_import_project(&staging, &created_files, &created_directories)?;
            return Err(error);
        }
    }
    let mut manifest_files = Vec::with_capacity(files.len());
    let mut literal_rust = 0_usize;
    let mut translated_rust = 0_usize;
    let mut passthrough = 0_usize;
    for file in &files {
        let contents = match fs::read(&file.source) {
            Ok(contents) => contents,
            Err(error) => {
                cleanup_import_project(&staging, &created_files, &created_directories)?;
                return Err(format!(
                    "no se pudo leer {}: {error}",
                    file.relative.display()
                ));
            }
        };
        if file.relative.file_name() == Some(OsStr::new("Cargo.toml")) {
            let cargo_text = match std::str::from_utf8(&contents) {
                Ok(text) => text,
                Err(error) => {
                    cleanup_import_project(&staging, &created_files, &created_directories)?;
                    return Err(format!(
                        "{} no es UTF-8 válido: {error}",
                        file.relative.display()
                    ));
                }
            };
            let converted = match convert_cargo_manifest_to_oxido(cargo_text, &file.relative) {
                Ok(converted) => converted,
                Err(error) => {
                    cleanup_import_project(&staging, &created_files, &created_directories)?;
                    return Err(error);
                }
            };
            let oxido_manifest_path = file.relative.with_file_name("oxido.toml");
            if original_paths.contains(&oxido_manifest_path) {
                cleanup_import_project(&staging, &created_files, &created_directories)?;
                return Err(format!(
                    "no se puede convertir {}: ya existe {} en el árbol de entrada",
                    file.relative.display(),
                    oxido_manifest_path.display()
                ));
            }
            if let Err(error) = write_import_file(
                &staging,
                &oxido_manifest_path,
                converted.as_bytes(),
                &mut created_directories,
                &mut created_files,
            ) {
                cleanup_import_project(&staging, &created_files, &created_directories)?;
                return Err(error);
            }
            manifest_files.push(serde_json::json!({
                "source": &file.relative,
                "preservedPath": &oxido_manifest_path,
                "sha256": sha256_hex(&contents),
                "strategy": "cargo-manifest-to-oxido-toml",
                "oxidoPath": null,
                "diagnostic": null,
            }));
            continue;
        }
        let oxido_relative = file.relative.with_extension("ox");
        let conversion = rust_imports.get(&file.relative);
        // Si se genera un `.ox`, el `.rs` original se conserva fuera de la
        // ruta convencional de módulos para que el resolvedor no vea dos
        // fuentes para el mismo módulo.
        let has_ox = matches!(conversion, Some(RustImport::Imported { .. }))
            && !original_paths.contains(&oxido_relative);
        let preserved_relative = if has_ox {
            Path::new(".oxido").join("original").join(&file.relative)
        } else {
            file.relative.clone()
        };
        if let Err(error) = write_import_file(
            &staging,
            &preserved_relative,
            &contents,
            &mut created_directories,
            &mut created_files,
        ) {
            cleanup_import_project(&staging, &created_files, &created_directories)?;
            return Err(error);
        }
        let mut entry = serde_json::json!({
            "source": &file.relative,
            "preservedPath": &preserved_relative,
            "sha256": sha256_hex(&contents),
            "strategy": file.kind.manifest_strategy(),
            "oxidoPath": null,
            "diagnostic": null,
        });
        if let Some(RustImport::Imported { text, mode }) = conversion {
            if original_paths.contains(&oxido_relative) {
                passthrough = passthrough.saturating_add(1);
                entry["strategy"] = serde_json::json!("passthrough-rust");
                entry["diagnostic"] = serde_json::json!(format!(
                    "no se creó el equivalente .ox porque ya existe {}",
                    oxido_relative.display()
                ));
            } else {
                if let Err(error) = write_import_file(
                    &staging,
                    &oxido_relative,
                    text.as_bytes(),
                    &mut created_directories,
                    &mut created_files,
                ) {
                    cleanup_import_project(&staging, &created_files, &created_directories)?;
                    return Err(error);
                }
                entry["oxidoPath"] = serde_json::json!(&oxido_relative);
                match mode {
                    ImportMode::Literal => {
                        literal_rust = literal_rust.saturating_add(1);
                        entry["strategy"] = serde_json::json!("verbatim-rust");
                    }
                    ImportMode::Spanish => {
                        if imported_contains_structured_function(text) {
                            translated_rust = translated_rust.saturating_add(1);
                            entry["strategy"] = serde_json::json!("translated-oxido");
                        } else {
                            passthrough = passthrough.saturating_add(1);
                            entry["strategy"] = serde_json::json!("passthrough-rust");
                            entry["diagnostic"] = serde_json::json!(format!(
                                "se conservó como Rust passthrough dentro de {}",
                                oxido_relative.display()
                            ));
                        }
                    }
                }
            }
        } else if let Some(RustImport::Passthrough(diagnostic)) = conversion {
            passthrough = passthrough.saturating_add(1);
            entry["strategy"] = serde_json::json!("passthrough-rust");
            entry["diagnostic"] = serde_json::json!(diagnostic);
        }
        manifest_files.push(entry);
    }
    // Los archivos producidos por la importación Rust usan `sintaxis llaves`.
    // Fijar el modo a nivel de proyecto evita que los comandos posteriores
    // elijan el valor predeterminado indentado y rechacen esos `.ox`.
    if !original_paths.contains(Path::new("oxido.toml"))
        && !original_paths.contains(Path::new("Cargo.toml"))
        && let Err(error) = write_import_file(
            &staging,
            Path::new("oxido.toml"),
            b"[language]\nsyntax = \"braces\"\nedition = \"2024\"\n",
            &mut created_directories,
            &mut created_files,
        )
    {
        cleanup_import_project(&staging, &created_files, &created_directories)?;
        return Err(error);
    }
    let manifest = serde_json::json!({
        "schemaVersion": 1,
        "kind": "oxido-lossless-workspace-import",
        "sourceFileCount": files.len(),
        "sourceDirectoryCount": directories.len(),
        "sourceBytes": total_bytes,
        "literalRustFiles": literal_rust,
        "translatedRustFiles": translated_rust,
        "passthroughRustFiles": passthrough,
        "files": manifest_files,
    });
    let contents = serde_json::to_vec_pretty(&manifest)
        .map_err(|error| format!("no se pudo serializar el manifiesto de importación: {error}"))?;
    if let Err(error) = write_import_file(
        &staging,
        &Path::new(".oxido").join("import-manifest.json"),
        &contents,
        &mut created_directories,
        &mut created_files,
    ) {
        cleanup_import_project(&staging, &created_files, &created_directories)?;
        return Err(error);
    }
    if let Err(error) = fs::rename(&staging, &output) {
        cleanup_import_project(&staging, &created_files, &created_directories)?;
        return Err(format!(
            "no se pudo publicar la importación sin pérdida: {error}"
        ));
    }
    if spanish {
        println!(
            "Importación sin pérdida creada en {}: {} archivos preservados, {} Rust traducidos a .ox, {} Rust conservados como passthrough.",
            output.display(),
            files.len(),
            translated_rust,
            passthrough
        );
    } else {
        println!(
            "Importación sin pérdida creada en {}: {} archivos preservados, {} Rust importados tal cual, {} Rust conservados como passthrough.",
            output.display(),
            files.len(),
            literal_rust,
            passthrough
        );
    }
    Ok(())
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum ImportMode {
    /// Copia el archivo Rust sin modificar su código.
    Literal,
    /// Traduce `fn` a `funcion` para el subconjunto reversible.
    Spanish,
}

enum RustImport {
    /// El archivo se importó y tiene un `.ox` equivalente.
    Imported { text: String, mode: ImportMode },
    /// El archivo se conserva como Rust; el motivo queda en el manifiesto.
    Passthrough(String),
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum ProjectSourceKind {
    Rust,
    Oxido,
    Other,
}

impl ProjectSourceKind {
    fn manifest_strategy(self) -> &'static str {
        match self {
            Self::Rust => "pending-rust-conversion",
            Self::Oxido => "preserved-oxido",
            Self::Other => "preserved-file",
        }
    }
}

struct ProjectSourceFile {
    source: PathBuf,
    relative: PathBuf,
    kind: ProjectSourceKind,
}

fn collect_project_files(
    root: &Path,
    project_root: &Path,
    files: &mut Vec<ProjectSourceFile>,
    directories: &mut Vec<PathBuf>,
    total_bytes: &mut u64,
) -> Result<(), String> {
    for entry in fs::read_dir(root)
        .map_err(|error| format!("no se pudo leer {}: {error}", root.display()))?
    {
        let entry = entry.map_err(|error| format!("no se pudo leer una entrada: {error}"))?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| format!("no se pudo inspeccionar {}: {error}", path.display()))?;
        if metadata.file_type().is_symlink() {
            return Err(format!(
                "la importación sin pérdida no admite enlaces simbólicos: {}",
                path.display()
            ));
        }
        if metadata.is_dir() {
            if path.file_name().is_some_and(|name| name == ".git") {
                continue;
            }
            let relative = path
                .strip_prefix(project_root)
                .map_err(|_| "la ruta de un directorio escapa del árbol de entrada")?
                .to_path_buf();
            if !relative.as_os_str().is_empty() {
                directories.push(relative);
            }
            collect_project_files(&path, project_root, files, directories, total_bytes)?;
        } else if metadata.is_file() {
            if files.len() >= MAX_IMPORT_FILES {
                return Err(format!(
                    "la importación supera el límite de {MAX_IMPORT_FILES} archivos"
                ));
            }
            *total_bytes = total_bytes.saturating_add(metadata.len());
            if *total_bytes > MAX_IMPORT_TOTAL_BYTES {
                return Err("la importación supera el límite total de 1 GiB".into());
            }
            let kind = match path.extension() {
                Some(extension) if extension == "rs" => ProjectSourceKind::Rust,
                Some(extension) if extension == "ox" => ProjectSourceKind::Oxido,
                _ => ProjectSourceKind::Other,
            };
            let relative = path
                .strip_prefix(project_root)
                .map_err(|_| "la ruta de una fuente escapa del árbol de entrada")?
                .to_path_buf();
            files.push(ProjectSourceFile {
                source: path,
                relative,
                kind,
            });
        }
    }
    files.sort_by(|left, right| left.source.cmp(&right.source));
    directories.sort();
    directories.dedup();
    Ok(())
}

fn import_workspace_rust_file(
    file: &ProjectSourceFile,
    rust_sources: &BTreeSet<PathBuf>,
    ox_sources: &BTreeSet<PathBuf>,
    spanish: bool,
) -> Result<RustImport, String> {
    let metadata = match fs::metadata(&file.source) {
        Ok(metadata) => metadata,
        Err(error) => {
            return Ok(RustImport::Passthrough(format!(
                "no se pudo inspeccionar {}: {error}",
                file.relative.display()
            )));
        }
    };
    if metadata.len() > MAX_SOURCE_BYTES {
        return Ok(RustImport::Passthrough(format!(
            "el archivo Rust supera el límite de 4 MiB para conversión: {}",
            file.relative.display()
        )));
    }
    let text = match fs::read_to_string(&file.source) {
        Ok(text) => text,
        Err(error) => {
            return Ok(RustImport::Passthrough(format!(
                "no se pudo leer como UTF-8 {}: {error}",
                file.relative.display()
            )));
        }
    };
    let source = SourceFile::with_path(&file.source, text);
    let source_parent = file.relative.parent().unwrap_or_else(|| Path::new(""));
    let rewritten = match oxido_compiler::rewrite_rust_module_paths(&source, |path| {
        rewrite_project_module_path(source_parent, path, rust_sources, ox_sources)
    }) {
        Ok(rewritten) => rewritten,
        Err(error) => {
            // Un archivo Rust válido con una ruta rechazada aborta la
            // importación; un archivo que ni siquiera es Rust válido se
            // conserva tal cual y se registra en el manifiesto.
            if oxido_compiler::analyze_rust(&source).is_ok() {
                return Err(error);
            }
            if spanish {
                return Ok(RustImport::Passthrough(error));
            }
            return Ok(RustImport::Imported {
                text: oxido_compiler::import_literal(&source),
                mode: ImportMode::Literal,
            });
        }
    };
    let rewritten_source = SourceFile::with_path(&file.source, rewritten);
    if spanish {
        match oxido_compiler::import_rust(&rewritten_source) {
            Ok(imported) => Ok(RustImport::Imported {
                text: imported,
                mode: ImportMode::Spanish,
            }),
            Err(error) => Ok(RustImport::Passthrough(error)),
        }
    } else {
        Ok(RustImport::Imported {
            text: oxido_compiler::import_literal(&rewritten_source),
            mode: ImportMode::Literal,
        })
    }
}

fn imported_contains_structured_function(text: &str) -> bool {
    let source = SourceFile::new(text);
    let parsed = oxido_compiler::parse_ox(
        &source,
        LexOptions {
            project_mode: Some(SyntaxMode::Braces),
        },
    );
    parsed.file.is_some_and(|file| {
        file.items
            .iter()
            .any(|item| matches!(item, oxido_language::domain::ast::Item::Function(_)))
    })
}

fn write_import_file(
    output_root: &Path,
    relative: &Path,
    contents: &[u8],
    directories: &mut Vec<PathBuf>,
    files: &mut Vec<PathBuf>,
) -> Result<(), String> {
    let parent = relative
        .parent()
        .ok_or_else(|| "una salida no tiene directorio padre".to_owned())?;
    create_output_directories(output_root, parent, directories)?;
    let destination = output_root.join(relative);
    write_new(&destination, contents)?;
    files.push(destination);
    Ok(())
}

fn import_staging_path(parent: &Path, output_name: &OsStr) -> Result<PathBuf, String> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("el reloj del sistema no es válido para importar: {error}"))?
        .as_nanos();
    Ok(parent.join(format!(
        ".{}.oxido-staging-{timestamp}",
        output_name.to_string_lossy()
    )))
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn rewrite_project_module_path(
    source_parent: &Path,
    path: &str,
    rust_sources: &std::collections::BTreeSet<PathBuf>,
    ox_sources: &std::collections::BTreeSet<PathBuf>,
) -> Result<String, String> {
    let path = Path::new(path);
    let relative = normalize_project_relative_path(source_parent, path)?;
    let target = match relative.extension() {
        Some(extension) if extension == "rs" => {
            if !rust_sources.contains(&relative) {
                return Err(format!(
                    "el atributo `#[path]` apunta a una fuente Rust que no se importará: {}",
                    relative.display()
                ));
            }
            relative.with_extension("ox")
        }
        Some(extension) if extension == "ox" => {
            if !ox_sources.contains(&relative) {
                return Err(format!(
                    "el atributo `#[path]` apunta a una fuente Óxido que no existe: {}",
                    relative.display()
                ));
            }
            relative
        }
        _ => {
            return Err(format!(
                "el atributo `#[path]` no apunta a una fuente .rs o .ox del árbol: {}",
                relative.display()
            ));
        }
    };
    // `#[path]` se resuelve desde el directorio del archivo que declara el
    // módulo, no desde la raíz del workspace. Devolver la ruta normalizada
    // relativa al workspace rompía cualquier archivo dentro de un
    // subdirectorio.
    Ok(relative_path_from(source_parent, &target)
        .to_string_lossy()
        .into_owned())
}

/// Construye una ruta relativa desde `base` hasta `target`, ambos normalizados
/// y relativos al workspace. Incluye `..` cuando el destino queda fuera del
/// directorio de origen.
fn relative_path_from(base: &Path, target: &Path) -> PathBuf {
    fn components(path: &Path) -> Vec<&OsStr> {
        path.components()
            .filter_map(|component| match component {
                std::path::Component::Normal(value) => Some(value),
                _ => None,
            })
            .collect()
    }
    let base = components(base);
    let target = components(target);
    let common = base
        .iter()
        .zip(&target)
        .take_while(|(left, right)| left == right)
        .count();
    let mut relative = PathBuf::new();
    for _ in common..base.len() {
        relative.push("..");
    }
    for component in &target[common..] {
        relative.push(component);
    }
    relative
}

fn normalize_project_relative_path(base: &Path, path: &Path) -> Result<PathBuf, String> {
    if path.is_absolute() {
        return Err("el atributo `#[path]` no puede salir del árbol importado".into());
    }
    let mut components = Vec::new();
    for component in base.components().chain(path.components()) {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::Normal(value) => components.push(value.to_os_string()),
            std::path::Component::ParentDir if components.pop().is_some() => {}
            std::path::Component::ParentDir => {
                return Err("el atributo `#[path]` sale de la raíz del árbol importado".into());
            }
            std::path::Component::RootDir | std::path::Component::Prefix(_) => {
                return Err("el atributo `#[path]` no puede salir del árbol importado".into());
            }
        }
    }
    Ok(components.into_iter().collect())
}

fn cleanup_import_project(
    output: &Path,
    files: &[PathBuf],
    directories: &[PathBuf],
) -> Result<(), String> {
    let cleanup_result = cleanup_export_paths(files, directories);
    let root_cleanup = fs::remove_dir(output);
    match (cleanup_result, root_cleanup) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) => Err(error),
        (Ok(()), Err(error)) => Err(format!(
            "no se pudo retirar la salida parcial {}: {error}",
            output.display()
        )),
        (Err(cleanup_error), Err(error)) => Err(format!(
            "{cleanup_error}; tampoco se pudo retirar la raíz de salida {}: {error}",
            output.display()
        )),
    }
}

fn convert_cargo_manifest_to_oxido(contents: &str, source: &Path) -> Result<String, String> {
    let cargo_manifest = toml::from_str::<toml::Value>(contents)
        .map_err(|error| format!("Cargo.toml inválido en {}: {error}", source.display()))?;
    let manifest_table = cargo_manifest
        .as_table()
        .ok_or_else(|| format!("Cargo.toml debe ser una tabla: {}", source.display()))?;
    let package_edition = manifest_table
        .get("package")
        .and_then(|package| package.get("edition"))
        .and_then(toml::Value::as_str);
    let workspace_edition = manifest_table
        .get("workspace")
        .and_then(|workspace| workspace.get("package"))
        .and_then(|package| package.get("edition"))
        .and_then(toml::Value::as_str);
    let edition = package_edition.or(workspace_edition).unwrap_or("2024");

    let mut language = toml::map::Map::new();
    language.insert("syntax".into(), toml::Value::String("braces".into()));
    language.insert("edition".into(), toml::Value::String(edition.to_owned()));
    let mut cargo = toml::map::Map::new();
    cargo.insert("manifest".into(), cargo_manifest);
    let mut oxido_manifest = toml::map::Map::new();
    oxido_manifest.insert("language".into(), toml::Value::Table(language));
    oxido_manifest.insert("cargo".into(), toml::Value::Table(cargo));
    toml::to_string_pretty(&toml::Value::Table(oxido_manifest)).map_err(|error| {
        format!(
            "no se pudo convertir {} a oxido.toml: {error}",
            source.display()
        )
    })
}

fn cargo_ox(args: &[OsString], action: CargoAction) -> Result<(), String> {
    if args.len() == 1 && (args[0] == OsStr::new("--help") || args[0] == OsStr::new("-h")) {
        println!("{}", cargo_usage(action));
        return Ok(());
    }
    let mut input = None;
    let mut trusted = false;
    let mut release = false;
    let mut json_output = false;
    let mut package = None;
    let mut binary = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].to_str() {
            Some("--confiar-dependencias") if !trusted => trusted = true,
            Some("--release") if !release => release = true,
            Some("--json") if !json_output => json_output = true,
            Some("-p" | "--package") => {
                if package.is_some() {
                    return Err(format!("uso: {}", cargo_usage(action)));
                }
                package = Some(selection_value(args, &mut index, "-p <paquete>")?);
            }
            Some("--bin") => {
                if binary.is_some() {
                    return Err(format!("uso: {}", cargo_usage(action)));
                }
                binary = Some(selection_value(args, &mut index, "--bin <nombre>")?);
            }
            _ => {
                let argument = &args[index];
                if argument.to_string_lossy().starts_with('-') || input.is_some() {
                    return Err(format!("uso: {}", cargo_usage(action)));
                }
                input = Some(argument.as_os_str());
            }
        }
        index += 1;
    }
    if action != CargoAction::Run && (package.is_some() || binary.is_some()) {
        return Err(format!(
            "`-p`/`--package` y `--bin` solo se admiten con `oxido ejecutar`; uso: {}",
            cargo_usage(action)
        ));
    }
    let input = project_input::resolve(input)?;
    if release && action == CargoAction::Test {
        return Err("probar no admite --release todavía".into());
    }
    if matches!(
        action,
        CargoAction::Check | CargoAction::Build | CargoAction::Run | CargoAction::Test
    ) && let Some(workspace_root) = imported_workspace_root(&input)?
        && oxido_compiler::application::workspace::is_imported_cargo_workspace(&workspace_root)?
    {
        let selection =
            oxido_compiler::application::workspace::ImportedWorkspaceSelection { package, binary };
        if input.is_file() && (selection.package.is_some() || selection.binary.is_some()) {
            return Err(
                "`-p`/`--package` y `--bin` no se combinan con un archivo `.ox` explícito".into(),
            );
        }
        let workspace_action = match action {
            CargoAction::Check => {
                oxido_compiler::application::workspace::ImportedWorkspaceAction::Check
            }
            CargoAction::Build => {
                oxido_compiler::application::workspace::ImportedWorkspaceAction::Build
            }
            CargoAction::Run => {
                oxido_compiler::application::workspace::ImportedWorkspaceAction::Run
            }
            CargoAction::Test => {
                oxido_compiler::application::workspace::ImportedWorkspaceAction::Test
            }
            _ => return Err("acción de workspace importado no válida".into()),
        };
        let result = oxido_compiler::application::workspace::run_imported_workspace(
            &workspace_root,
            &input,
            trusted,
            release,
            workspace_action,
            &selection,
        )?;
        if json_output {
            let mut diagnostics = result
                .mapped_diagnostics
                .iter()
                .map(|diagnostic| {
                    let file = diagnostic
                        .source_file
                        .as_deref()
                        .and_then(|path| structured_report_path(path, &workspace_root));
                    serde_json::json!({
                        "code": diagnostic.code.as_deref().unwrap_or("OX5001"),
                        "severity": diagnostic.level,
                        "message": sanitize_report_text(&diagnostic.message, Some(&workspace_root)),
                        "file": file,
                        "line": diagnostic.source.map(|location| location.line),
                        "column": diagnostic.source.map(|location| location.column)
                    })
                })
                .collect::<Vec<_>>();
            if !result.stderr.trim().is_empty() {
                diagnostics.push(serde_json::json!({
                    "code": "OX5001",
                    "severity": "error",
                    "message": sanitize_report_text(&result.stderr, Some(&workspace_root)),
                    "file": null,
                    "line": null,
                    "column": null
                }));
            }
            if !result.success && diagnostics.is_empty() {
                diagnostics.push(serde_json::json!({
                    "code": "OX5001",
                    "severity": "error",
                    "message": format!(
                        "Cargo terminó con estado {}",
                        result.exit_code.map_or_else(
                            || "sin código de salida".to_owned(),
                            |code| code.to_string()
                        )
                    ),
                    "file": null,
                    "line": null,
                    "column": null
                }));
            }
            let output = structured_program_output(&result.program_output, &workspace_root);
            let mut report = serde_json::json!({
                "schemaVersion": 1,
                "success": result.success,
                "diagnostics": diagnostics,
                "output": output
            });
            if action == CargoAction::Build {
                report["artifactDirectory"] = serde_json::Value::String(format!(
                    "target/oxido/{}",
                    if release { "release" } else { "debug" }
                ));
            }
            if result.success {
                println!("{report}");
                return Ok(());
            }
            return Err(format!("{STRUCTURED_ERROR_PREFIX}{report}"));
        }
        for diagnostic in &result.diagnostics {
            eprint!("{diagnostic}");
        }
        if !result.stderr.trim().is_empty() {
            eprint!(
                "{}",
                sanitize_report_text(&result.stderr, Some(&workspace_root))
            );
        }
        if result.success {
            for line in &result.program_output {
                println!("{line}");
            }
            if action == CargoAction::Check {
                println!("Workspace Óxido verificado correctamente.");
            }
            if action == CargoAction::Build {
                let profile = if release { "release" } else { "debug" };
                println!("Binarios generados en target/oxido/{profile}.");
            }
            return Ok(());
        }
        return Err(format!(
            "Cargo terminó con estado {}; el workspace importado no compiló.",
            result
                .exit_code
                .map_or_else(|| "desconocido".to_owned(), |code| code.to_string())
        ));
    }
    if package.is_some() || binary.is_some() {
        return Err("`-p`/`--package` y `--bin` solo se admiten en workspaces importados".into());
    }
    let (source, display_path) = load_ox(input.as_os_str())?;
    let settings = project_settings(&display_path)?;
    let action = match (action, release, trusted) {
        (CargoAction::Check, false, false) => CargoAction::Check,
        (CargoAction::Check, false, true) => CargoAction::CheckTrusted,
        (CargoAction::Check, true, false) => CargoAction::CheckRelease,
        (CargoAction::Check, true, true) => CargoAction::CheckReleaseTrusted,
        (CargoAction::Build, false, false) => CargoAction::Build,
        (CargoAction::Build, false, true) => CargoAction::BuildTrusted,
        (CargoAction::Build, true, false) => CargoAction::BuildRelease,
        (CargoAction::Build, true, true) => CargoAction::BuildReleaseTrusted,
        (CargoAction::Run, false, false) => CargoAction::Run,
        (CargoAction::Run, false, true) => CargoAction::RunTrusted,
        (CargoAction::Run, true, false) => CargoAction::RunRelease,
        (CargoAction::Run, true, true) => CargoAction::RunReleaseTrusted,
        (CargoAction::Test, false, false) => CargoAction::Test,
        (CargoAction::Test, false, true) => CargoAction::TestTrusted,
        (CargoAction::Test, true, false) => return Err("probar no admite --release todavía".into()),
        (CargoAction::Test, true, true) => return Err("probar no admite --release todavía".into()),
        _ => return Err("acción Cargo no válida para este comando".into()),
    };
    let lockfile = settings.root.join("Cargo.lock");
    let result = oxido_compiler::build_ox_project_in_project_with_options(
        &source,
        settings.options,
        oxido_compiler::ProjectBuildOptions {
            dependencies: &settings.dependencies,
            lockfile: Some(&lockfile),
            action,
            project_root: &settings.root,
            cancellation: None,
            package: settings.package.as_ref(),
        },
    );
    let result = match result {
        Ok(result) => result,
        Err(diagnostics) if json_output => {
            let diagnostics = diagnostics
                .iter()
                .map(|diagnostic| {
                    structured_source_diagnostic(&source, &display_path, &settings.root, diagnostic)
                })
                .collect();
            return Err(structured_failure(diagnostics, Vec::new()));
        }
        Err(diagnostics) => {
            return Err(format_diagnostics(&source, &display_path, &diagnostics));
        }
    };
    if json_output {
        let mut diagnostics = result
            .diagnostics
            .iter()
            .map(|diagnostic| {
                let file = diagnostic
                    .source_file
                    .as_deref()
                    .and_then(|path| structured_report_path(path, &settings.root));
                serde_json::json!({
                    "code": diagnostic.code.as_deref().unwrap_or("OX5001"),
                    "severity": diagnostic.level,
                    "message": sanitize_report_text(&diagnostic.message, Some(&settings.root)),
                    "file": file,
                    "line": diagnostic.source.map(|location| location.line),
                    "column": diagnostic.source.map(|location| location.column)
                })
            })
            .collect::<Vec<_>>();
        let output = structured_program_output(&result.program_output, &settings.root);
        if !result.success && diagnostics.is_empty() {
            let message = if result.timed_out {
                "Cargo superó el límite de ejecución de 120 segundos".to_owned()
            } else {
                format!(
                    "Cargo terminó con estado {}",
                    result
                        .exit_code
                        .map_or_else(|| "sin código de salida".into(), |code| code.to_string())
                )
            };
            diagnostics.push(serde_json::json!({
                "code": "OX5001",
                "severity": "error",
                "message": message,
                "file": null,
                "line": null,
                "column": null
            }));
        }
        let report = serde_json::json!({
            "schemaVersion": 1,
            "success": result.success,
            "diagnostics": diagnostics,
            "output": output,
            "artifactDirectory": build_artifact_directory(action, release)
        });
        if result.success {
            println!("{report}");
            return Ok(());
        }
        return Err(format!("{STRUCTURED_ERROR_PREFIX}{report}"));
    }
    for diagnostic in &result.diagnostics {
        let code = diagnostic
            .code
            .as_ref()
            .map(|code| format!("[{code}]"))
            .unwrap_or_default();
        if let Some(location) = diagnostic.source {
            let diagnostic_path = diagnostic
                .source_file
                .as_deref()
                .map(|source_file| settings.root.join(source_file))
                .unwrap_or_else(|| display_path.clone());
            let diagnostic_path = std::env::current_dir()
                .ok()
                .and_then(|current| {
                    diagnostic_path
                        .strip_prefix(current)
                        .ok()
                        .map(PathBuf::from)
                })
                .unwrap_or(diagnostic_path);
            eprintln!(
                "{}{}: {}\n --> {}:{}:{}",
                diagnostic.level,
                code,
                diagnostic.message,
                diagnostic_path.display(),
                location.line,
                location.column
            );
        } else {
            eprintln!("{}{}: {}", diagnostic.level, code, diagnostic.message);
        }
    }
    if !result.stderr.trim().is_empty() {
        eprint!("{}", result.stderr);
    }
    let prints_program_output = matches!(
        action,
        CargoAction::Run
            | CargoAction::RunTrusted
            | CargoAction::RunRelease
            | CargoAction::RunReleaseTrusted
            | CargoAction::Test
            | CargoAction::TestTrusted
    );
    if prints_program_output {
        for line in &result.program_output {
            println!("{line}");
        }
    }
    if !result.success {
        if result.timed_out {
            return Err("Cargo superó el límite de ejecución de 120 segundos".into());
        }
        return Err(format!(
            "Cargo terminó con estado {}",
            result
                .exit_code
                .map_or_else(|| "sin código de salida".into(), |code| code.to_string())
        ));
    }
    if !prints_program_output {
        match action {
            CargoAction::Check
            | CargoAction::CheckTrusted
            | CargoAction::CheckRelease
            | CargoAction::CheckReleaseTrusted => {
                println!("El programa .ox pasó la verificación de Cargo.");
            }
            CargoAction::Build
            | CargoAction::BuildTrusted
            | CargoAction::BuildRelease
            | CargoAction::BuildReleaseTrusted => {
                let profile = if release { "release" } else { "debug" };
                println!("Binario generado en target/oxido/target/{profile}.");
                println!("Proyecto Cargo generado en target/oxido/.");
            }
            _ => {}
        }
    }
    Ok(())
}

fn cargo_usage(action: CargoAction) -> &'static str {
    match action {
        CargoAction::Check => {
            "oxido verificar [<archivo.ox|directorio>] [--confiar-dependencias] [--json]"
        }
        CargoAction::Build => {
            "oxido compilar [<archivo.ox|directorio>] [--release] [--confiar-dependencias] [--json]"
        }
        CargoAction::Run => {
            "oxido ejecutar [<archivo.ox|directorio>] [-p <paquete>] [--bin <nombre>] [--release] [--confiar-dependencias] [--json] (alias: run)"
        }
        CargoAction::Test => {
            "oxido probar [<archivo.ox|directorio>] [--confiar-dependencias] [--json]"
        }
        _ => {
            "oxido ejecutar [<archivo.ox|directorio>] [-p <paquete>] [--bin <nombre>] [--release] [--confiar-dependencias] [--json] (alias: run)"
        }
    }
}

fn selection_value(args: &[OsString], index: &mut usize, option: &str) -> Result<String, String> {
    let value = args
        .get(*index + 1)
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty() && !value.starts_with('-'))
        .ok_or_else(|| format!("falta el valor de {option}"))?;
    *index += 1;
    Ok(value.to_owned())
}

fn build_artifact_directory(action: CargoAction, release: bool) -> Option<String> {
    matches!(
        action,
        CargoAction::Build
            | CargoAction::BuildTrusted
            | CargoAction::BuildRelease
            | CargoAction::BuildReleaseTrusted
    )
    .then(|| {
        format!(
            "target/oxido/target/{}",
            if release { "release" } else { "debug" }
        )
    })
}

fn structured_failure(diagnostics: Vec<serde_json::Value>, output: Vec<String>) -> String {
    let report = serde_json::json!({
        "schemaVersion": 1,
        "success": false,
        "diagnostics": diagnostics,
        "output": output
    });
    format!("{STRUCTURED_ERROR_PREFIX}{report}")
}

fn structured_source_diagnostic(
    source: &SourceFile,
    display_path: &Path,
    project_root: &Path,
    diagnostic: &Diagnostic,
) -> serde_json::Value {
    let location = source.position(diagnostic.primary_span.start());
    let file = fs::canonicalize(display_path)
        .ok()
        .and_then(|path| path.strip_prefix(project_root).ok().map(Path::to_path_buf))
        .and_then(|path| structured_report_path(&path.to_string_lossy(), project_root));
    serde_json::json!({
        "code": diagnostic.code,
        "severity": match diagnostic.severity {
            oxido_language::domain::diagnostics::Severity::Error => "error",
            oxido_language::domain::diagnostics::Severity::Warning => "warning",
            oxido_language::domain::diagnostics::Severity::Info => "information",
            oxido_language::domain::diagnostics::Severity::Hint => "hint",
        },
        "message": sanitize_report_text(&diagnostic.message, Some(project_root)),
        "file": file,
        "line": location.map(|position| position.line),
        "column": location.map(|position| position.column)
    })
}

fn structured_report_path(path: &str, project_root: &Path) -> Option<String> {
    let path = Path::new(path);
    let relative = if path.is_absolute() {
        path.strip_prefix(project_root).ok()?
    } else {
        path
    };
    if relative.as_os_str().is_empty()
        || relative
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return None;
    }
    Some(relative.to_string_lossy().replace('\\', "/"))
}

fn sanitize_report_text(value: &str, project_root: Option<&Path>) -> String {
    let mut sanitized = value.to_owned();
    if let Some(project_root) = project_root {
        sanitized = sanitized.replace(&project_root.to_string_lossy().to_string(), "<proyecto>");
    }
    for root in [
        env::current_dir().ok(),
        fs::canonicalize(env::temp_dir()).ok(),
    ]
    .into_iter()
    .flatten()
    {
        sanitized = sanitized.replace(&root.to_string_lossy().to_string(), "<ruta>");
    }
    sanitized
        .split_whitespace()
        .map(|word| {
            let prefix_len = word
                .char_indices()
                .take_while(|(_, character)| matches!(character, '(' | '[' | '{' | '\'' | '"'))
                .map(|(index, character)| index + character.len_utf8())
                .last()
                .unwrap_or(0);
            let (prefix, candidate) = word.split_at(prefix_len);
            if candidate.starts_with('/') {
                format!("{prefix}<ruta>")
            } else {
                word.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn structured_program_output(output: &[String], project_root: &Path) -> Vec<String> {
    output
        .iter()
        .map(|line| sanitize_report_text(line, Some(project_root)))
        .collect()
}

fn load_ox(path: &OsStr) -> Result<(SourceFile, PathBuf), String> {
    let display_path = PathBuf::from(path);
    if display_path.extension() != Some(OsStr::new("ox")) {
        return Err("el archivo de entrada debe terminar en .ox".into());
    }
    let canonical = fs::canonicalize(&display_path)
        .map_err(|error| format!("no se pudo resolver {}: {error}", display_path.display()))?;
    let metadata = fs::metadata(&canonical)
        .map_err(|error| format!("no se pudo leer {}: {error}", display_path.display()))?;
    if metadata.len() > MAX_SOURCE_BYTES {
        return Err("el archivo .ox supera el límite de 4 MiB".into());
    }
    let text = fs::read_to_string(&canonical)
        .map_err(|error| format!("no se pudo leer {}: {error}", display_path.display()))?;
    Ok((
        SourceFile::with_path(display_path.clone(), text),
        display_path,
    ))
}

struct ProjectSettings {
    options: LexOptions,
    dependencies: Vec<oxido_compiler::config::ResolvedDependency>,
    root: PathBuf,
    package: Option<oxido_compiler::application::cargo::GeneratedPackage>,
}

fn project_settings(source_path: &Path) -> Result<ProjectSettings, String> {
    let canonical_source = fs::canonicalize(source_path)
        .map_err(|error| format!("no se pudo resolver el proyecto: {error}"))?;
    let parent = canonical_source
        .parent()
        .ok_or_else(|| "el archivo .ox no tiene directorio de proyecto".to_owned())?;
    for directory in parent.ancestors() {
        let config_path = directory.join("oxido.toml");
        match fs::symlink_metadata(&config_path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err("oxido.toml no puede ser un enlace simbólico".into());
            }
            Ok(metadata) if metadata.is_file() => {
                if metadata.len() > 64 * 1024 {
                    return Err("oxido.toml supera el límite de 64 KiB".into());
                }
                let content = fs::read_to_string(&config_path)
                    .map_err(|error| format!("no se pudo leer oxido.toml: {error}"))?;
                let config = oxido_compiler::config::ProjectConfig::parse(&content)
                    .map_err(|error| format!("oxido.toml inválido: {error}"))?;
                let dependencies = config.resolve_dependencies(directory)?;
                let canonical_root = fs::canonicalize(directory).map_err(|error| {
                    format!("no se pudo resolver la raíz del proyecto: {error}")
                })?;
                let canonical_source = fs::canonicalize(source_path)
                    .map_err(|error| format!("no se pudo resolver el archivo fuente: {error}"))?;
                let mut binary_name = None;
                for binary in &config.bins {
                    let binary_path = canonical_root.join(&binary.path);
                    let canonical_binary = fs::canonicalize(&binary_path).map_err(|error| {
                        format!("no se pudo resolver la ruta de [[bin]]: {error}")
                    })?;
                    if !canonical_binary.starts_with(&canonical_root) || !canonical_binary.is_file()
                    {
                        return Err(
                            "la ruta de [[bin]] debe ser un archivo regular dentro del proyecto"
                                .into(),
                        );
                    }
                    if canonical_binary == canonical_source {
                        binary_name = binary.name.clone();
                    }
                }
                let package = config
                    .package
                    .as_ref()
                    .map(
                        |package| oxido_compiler::application::cargo::GeneratedPackage {
                            name: Some(package.name.clone()),
                            version: Some(package.version.clone()),
                            binary_name: binary_name.clone(),
                        },
                    )
                    .or_else(|| {
                        binary_name.map(|name| {
                            oxido_compiler::application::cargo::GeneratedPackage {
                                name: None,
                                version: None,
                                binary_name: Some(name),
                            }
                        })
                    });
                let options = LexOptions {
                    project_mode: config.language.syntax.map(|mode| match mode {
                        oxido_compiler::config::SyntaxMode::Indentation => SyntaxMode::Indentation,
                        oxido_compiler::config::SyntaxMode::Braces => SyntaxMode::Braces,
                    }),
                };
                return Ok(ProjectSettings {
                    options,
                    dependencies,
                    root: directory.to_path_buf(),
                    package,
                });
            }
            Ok(_) => return Err("oxido.toml debe ser un archivo regular".into()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("no se pudo inspeccionar oxido.toml: {error}")),
        }
        if directory.join(".git").exists() {
            break;
        }
    }
    Ok(ProjectSettings {
        options: LexOptions::default(),
        dependencies: Vec::new(),
        root: parent.to_path_buf(),
        package: None,
    })
}

fn validate_output_parent(path: &Path) -> Result<(), String> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let canonical_parent = fs::canonicalize(parent)
        .map_err(|error| format!("no se pudo resolver el directorio de salida: {error}"))?;
    let workspace = env::current_dir().map_err(|error| error.to_string())?;
    let canonical_workspace = fs::canonicalize(workspace).map_err(|error| error.to_string())?;
    if !canonical_parent.starts_with(&canonical_workspace) {
        return Err("la salida debe quedar dentro del directorio actual del proyecto".into());
    }
    Ok(())
}

fn validate_module_output_path(path: &Path) -> Result<(), String> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path.extension() != Some(OsStr::new("rs"))
        || path
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err("la ruta Rust de un módulo generado no es válida".into());
    }
    Ok(())
}

fn preflight_export_outputs(
    output_root: &Path,
    outputs: &[(PathBuf, Vec<u8>)],
) -> Result<(), String> {
    let mut seen = std::collections::BTreeSet::new();
    for (path, _) in outputs {
        if !path.starts_with(output_root) || !seen.insert(path.clone()) {
            return Err("la exportación produjo rutas duplicadas o fuera del destino".into());
        }
        match fs::symlink_metadata(path) {
            Ok(_) => {
                return Err(format!(
                    "la salida ya existe; no se sobrescribe: {}",
                    path.display()
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "no se pudo inspeccionar {}: {error}",
                    path.display()
                ));
            }
        }
        let parent = path
            .parent()
            .ok_or_else(|| "una ruta de salida no tiene directorio padre".to_owned())?;
        let relative_parent = parent
            .strip_prefix(output_root)
            .map_err(|_| "una salida recursiva escapa del directorio de destino".to_owned())?;
        let mut current = output_root.to_path_buf();
        for component in relative_parent.components() {
            let std::path::Component::Normal(component) = component else {
                return Err("la ruta de salida contiene un componente no permitido".into());
            };
            current.push(component);
            match fs::symlink_metadata(&current) {
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    return Err(format!(
                        "no se exporta a través del enlace simbólico {}",
                        current.display()
                    ));
                }
                Ok(metadata) if !metadata.is_dir() => {
                    return Err(format!(
                        "el directorio de salida no es un directorio: {}",
                        current.display()
                    ));
                }
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
                Err(error) => {
                    return Err(format!(
                        "no se pudo inspeccionar {}: {error}",
                        current.display()
                    ));
                }
            }
        }
    }
    Ok(())
}

fn create_output_directories(
    output_root: &Path,
    relative_parent: &Path,
    created: &mut Vec<PathBuf>,
) -> Result<(), String> {
    let mut current = output_root.to_path_buf();
    for component in relative_parent.components() {
        let std::path::Component::Normal(component) = component else {
            return Err("la ruta de salida contiene un componente no permitido".into());
        };
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(format!(
                    "no se exporta a través del enlace simbólico {}",
                    current.display()
                ));
            }
            Ok(metadata) if !metadata.is_dir() => {
                return Err(format!(
                    "el directorio de salida no es un directorio: {}",
                    current.display()
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir(&current)
                    .map_err(|error| format!("no se pudo crear {}: {error}", current.display()))?;
                created.push(current.clone());
            }
            Err(error) => {
                return Err(format!(
                    "no se pudo inspeccionar {}: {error}",
                    current.display()
                ));
            }
        }
    }
    Ok(())
}

fn write_new(path: &Path, contents: &[u8]) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| format!("no se pudo crear {}: {error}", path.display()))?;
    if let Err(error) = file.write_all(contents) {
        drop(file);
        let cleanup = fs::remove_file(path);
        return match cleanup {
            Ok(()) => Err(format!("no se pudo escribir {}: {error}", path.display())),
            Err(cleanup_error) => Err(format!(
                "no se pudo escribir {}: {error}; tampoco se pudo retirar la salida parcial: {cleanup_error}",
                path.display()
            )),
        };
    }
    Ok(())
}

fn format_diagnostics(source: &SourceFile, path: &Path, diagnostics: &[Diagnostic]) -> String {
    diagnostics
        .iter()
        .map(|diagnostic| {
            let location = source.position(diagnostic.primary_span.start());
            match location {
                Some(location) => format!(
                    "{} {}: {} ({}:{}:{})",
                    diagnostic.code,
                    format!("{:?}", diagnostic.severity).to_lowercase(),
                    diagnostic.message,
                    path.display(),
                    location.line,
                    location.column
                ),
                None => format!("{}: {}", diagnostic.code, diagnostic.message),
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn usage() -> &'static str {
    "uso: oxido [--version|-V] <aliases|analizar-rust|auditar-proyecto|auditar-unsafe|descargar-dependencias|nuevo|formato|exportar|importar|importar-proyecto|probar|verificar|compilar|ejecutar> ...; ejecutar admite -p <paquete> y --bin <nombre> en workspaces importados; run es alias de ejecutar"
}
