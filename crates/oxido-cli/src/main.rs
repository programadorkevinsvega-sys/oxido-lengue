#![forbid(unsafe_code)]

use std::{
    collections::VecDeque,
    env,
    ffi::{OsStr, OsString},
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::ExitCode,
};

use oxido_compiler::{application::cargo::CargoAction, generate_ox};
use oxido_language::{
    SourceFile,
    domain::{
        diagnostics::Diagnostic,
        syntax::{LexOptions, SyntaxMode},
    },
};

const MAX_SOURCE_BYTES: u64 = 4 * 1024 * 1024;

fn main() -> ExitCode {
    match run(env::args_os().skip(1).collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("oxido: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: Vec<OsString>) -> Result<(), String> {
    let Some(command) = args.first().and_then(|value| value.to_str()) else {
        return Err(usage().into());
    };
    match command {
        "analizar-rust" => analyze_rust(&args[1..]),
        "aliases" => print_aliases(&args[1..]),
        "auditar-unsafe" => audit_unsafe(&args[1..]),
        "auditar-proyecto" => audit_project(&args[1..]),
        "exportar" => export_ox(&args[1..]),
        "importar" => import_rust(&args[1..]),
        "importar-proyecto" => import_project(&args[1..]),
        "compilar" => cargo_ox(&args[1..], CargoAction::Check),
        "ejecutar" => cargo_ox(&args[1..], CargoAction::Run),
        "probar" => cargo_ox(&args[1..], CargoAction::Test),
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
    let [root] = args else {
        return Err("uso: oxido auditar-proyecto <directorio>".into());
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
            let lexed = oxido_language::domain::syntax::lex(
                &source,
                LexOptions {
                    project_mode: Some(SyntaxMode::Braces),
                },
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
    let lexed = oxido_language::domain::syntax::lex(
        &source,
        LexOptions {
            project_mode: Some(SyntaxMode::Braces),
        },
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
    let [input, rest @ ..] = args else {
        return Err("uso: oxido formato <archivo.ox> [--comprobar]".into());
    };
    let check = match rest {
        [] => false,
        [flag] if flag == "--comprobar" => true,
        _ => return Err("uso: oxido formato <archivo.ox> [--comprobar]".into()),
    };
    let (source, path) = load_ox(input)?;
    let settings = project_settings(&path)?;
    let generated = generate_ox(&source, settings.options)
        .map_err(|diagnostics| format_diagnostics(&source, &path, &diagnostics))?;
    let formatted = oxido_compiler::format_rust(&SourceFile::new(generated.rust.clone()))
        .map_err(|error| format!("no se pudo formatear Rust generado: {error}"))?;
    if check {
        if formatted != generated.rust {
            return Err(
                "el archivo no está formateado; ejecuta `oxido formato <archivo.ox>`".into(),
            );
        }
        println!("{} está formateado.", path.display());
    } else {
        // El formateador conserva el modo y la sintaxis Óxido mediante la
        // importación reversible del Rust formateado.
        let imported = oxido_compiler::import_rust(&SourceFile::new(formatted))?;
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
        println!("Rust exportado a {}", path.display());
    }
    Ok(())
}

fn cleanup_export_paths(files: &[PathBuf], directories: &[PathBuf]) -> Result<(), String> {
    let mut failures = Vec::new();
    for path in files.iter().rev() {
        if let Err(error) = fs::remove_file(path) {
            if error.kind() != std::io::ErrorKind::NotFound {
                failures.push(format!("{}: {error}", path.display()));
            }
        }
    }
    for path in directories.iter().rev() {
        if let Err(error) = fs::remove_dir(path) {
            if error.kind() != std::io::ErrorKind::NotFound {
                failures.push(format!("{}: {error}", path.display()));
            }
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

fn import_rust(args: &[OsString]) -> Result<(), String> {
    let [input, flag, output] = args else {
        return Err("uso: oxido importar <archivo.rs> --salida <archivo.ox>".into());
    };
    if flag != "--salida" {
        return Err("se esperaba --salida <archivo.ox>".into());
    }
    let input = PathBuf::from(input);
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
    let output = PathBuf::from(output);
    if output.extension() != Some(OsStr::new("ox")) {
        return Err("la ruta de salida debe terminar en .ox".into());
    }
    validate_output_parent(&output)?;
    if fs::symlink_metadata(&output).is_ok() {
        return Err("el archivo .ox de salida ya existe; elige otra ruta".into());
    }
    let imported = oxido_compiler::import_rust(&source)?;
    write_new(&output, imported.as_bytes())?;
    println!("Óxido importado a {}", output.display());
    Ok(())
}

fn import_project(args: &[OsString]) -> Result<(), String> {
    let [input, flag, output] = args else {
        return Err(
            "uso: oxido importar-proyecto <directorio-rust> --salida <directorio-oxido>".into(),
        );
    };
    if flag != "--salida" {
        return Err("se esperaba --salida <directorio-oxido>".into());
    }
    let input = fs::canonicalize(input)
        .map_err(|error| format!("no se pudo resolver el árbol Rust: {error}"))?;
    if !input.is_dir() {
        return Err("la entrada debe ser un directorio Rust".into());
    }
    let output = PathBuf::from(output);
    if output.exists() {
        return Err("el directorio de salida ya existe; elige otra ruta".into());
    }
    let mut files = Vec::new();
    collect_rust_files(&input, &mut files)?;
    if files.is_empty() {
        return Err("el árbol no contiene archivos .rs".into());
    }
    for file in files {
        let relative = file
            .strip_prefix(&input)
            .map_err(|_| "ruta de módulo inválida")?;
        let destination = output.join(relative).with_extension("ox");
        let text = fs::read_to_string(&file)
            .map_err(|error| format!("no se pudo leer {}: {error}", file.display()))?;
        let imported = oxido_compiler::import_rust(&SourceFile::with_path(&file, text))?;
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("no se pudo crear {}: {error}", parent.display()))?;
        }
        fs::write(&destination, imported)
            .map_err(|error| format!("no se pudo escribir {}: {error}", destination.display()))?;
        println!("Óxido importado a {}", destination.display());
    }
    Ok(())
}

fn collect_rust_files(root: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in fs::read_dir(root)
        .map_err(|error| format!("no se pudo leer {}: {error}", root.display()))?
    {
        let entry = entry.map_err(|error| format!("no se pudo leer una entrada: {error}"))?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| format!("no se pudo inspeccionar {}: {error}", path.display()))?;
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            if path.file_name().is_some_and(|name| name == "target") {
                continue;
            }
            collect_rust_files(&path, files)?;
        } else if metadata.is_file() && path.extension() == Some(OsStr::new("rs")) {
            files.push(path);
        }
    }
    files.sort();
    Ok(())
}

fn cargo_ox(args: &[OsString], action: CargoAction) -> Result<(), String> {
    let Some(input) = args.first() else {
        let command = if action == CargoAction::Check {
            "compilar"
        } else if action == CargoAction::Test {
            "probar"
        } else {
            "ejecutar"
        };
        return Err(format!(
            "uso: oxido {command} <archivo.ox> [--release] [--confiar-dependencias]"
        ));
    };
    let mut trusted = false;
    let mut release = false;
    for argument in &args[1..] {
        match argument.to_str() {
            Some("--confiar-dependencias") if !trusted => trusted = true,
            Some("--release") if !release => release = true,
            _ => {
                return Err("uso: oxido compilar|ejecutar <archivo.ox> [--release] [--confiar-dependencias]".into());
            }
        }
    }
    let (source, display_path) = load_ox(input)?;
    let settings = project_settings(&display_path)?;
    let action = match (action, release, trusted) {
        (CargoAction::Check, false, false) => CargoAction::Check,
        (CargoAction::Check, false, true) => CargoAction::CheckTrusted,
        (CargoAction::Check, true, false) => CargoAction::CheckRelease,
        (CargoAction::Check, true, true) => CargoAction::CheckReleaseTrusted,
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
    let result = oxido_compiler::build_ox_project_in_project(
        &source,
        settings.options,
        &settings.dependencies,
        Some(&lockfile),
        action,
        &settings.root,
    )
    .map_err(|diagnostics| format_diagnostics(&source, &display_path, &diagnostics))?;
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
    if matches!(
        action,
        CargoAction::Run
            | CargoAction::RunTrusted
            | CargoAction::RunRelease
            | CargoAction::RunReleaseTrusted
            | CargoAction::Test
            | CargoAction::TestTrusted
    ) {
        for line in result.program_output {
            println!("{line}");
        }
    } else {
        println!("El programa .ox compila correctamente.");
    }
    Ok(())
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
    "uso: oxido <aliases|analizar-rust|auditar-proyecto|auditar-unsafe|descargar-dependencias|nuevo|formato|exportar|importar|importar-proyecto|compilar|probar|ejecutar> ..."
}
