#![forbid(unsafe_code)]

use std::{env, fs, process::ExitCode};

use oxido_language::SourceFile;

fn main() -> ExitCode {
    match run(env::args_os().skip(1).collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("oxido: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: Vec<std::ffi::OsString>) -> Result<(), String> {
    let [command, path] = args.as_slice() else {
        return Err("uso: oxido analizar-rust <archivo.rs>".into());
    };
    if command != "analizar-rust" {
        return Err("comando inicial disponible: analizar-rust".into());
    }
    let path = std::path::PathBuf::from(path);
    let text = fs::read_to_string(&path)
        .map_err(|error| format!("no se pudo leer {}: {error}", path.display()))?;
    let source = SourceFile::with_path(path, text);
    let result = oxido_compiler::analyze_rust(&source)?;
    println!("Sintaxis Rust válida ({} items).", result.item_count);
    Ok(())
}
