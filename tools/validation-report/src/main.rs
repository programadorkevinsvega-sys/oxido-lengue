#![forbid(unsafe_code)]

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const FUZZ_TARGETS: [&str; 5] = ["lexer", "parser", "config", "backend", "lsp"];
const FUZZ_TOOLCHAIN: &str = "nightly-2026-10-16";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CheckState {
    Passed,
    Failed,
    NotEvaluated,
}

#[derive(Debug)]
struct CheckResult {
    name: String,
    command: String,
    state: CheckState,
    elapsed: Duration,
    exit_code: Option<i32>,
    tests_passed: usize,
    tests_failed: usize,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("no se pudo generar el informe de validación: {error}");
        std::process::exit(2);
    }
}

fn run() -> Result<(), String> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace = manifest_dir
        .join("../..")
        .canonicalize()
        .map_err(|error| format!("no se pudo resolver la raíz del workspace: {error}"))?;
    let mut results = Vec::new();
    let fuzz_toolchain_argument = format!("+{FUZZ_TOOLCHAIN}");

    results.push(run_check(
        &workspace,
        "Formato del recolector",
        "cargo",
        &[
            "fmt",
            "--manifest-path",
            "tools/validation-report/Cargo.toml",
            "--",
            "--check",
        ],
        false,
    ));
    results.push(run_check(
        &workspace,
        "Lint del recolector",
        "cargo",
        &[
            "clippy",
            "--manifest-path",
            "tools/validation-report/Cargo.toml",
            "--all-targets",
            "--locked",
            "--",
            "-D",
            "warnings",
        ],
        false,
    ));
    results.push(run_check(
        &workspace,
        "Pruebas del recolector",
        "cargo",
        &[
            "test",
            "--manifest-path",
            "tools/validation-report/Cargo.toml",
            "--locked",
        ],
        false,
    ));
    results.push(run_check(
        &workspace,
        "Formato Rust",
        "cargo",
        &["fmt", "--all", "--", "--check"],
        false,
    ));
    results.push(run_check(
        &workspace,
        "Lint workspace",
        "cargo",
        &[
            "clippy",
            "--workspace",
            "--all-targets",
            "--locked",
            "--",
            "-D",
            "warnings",
        ],
        false,
    ));
    results.push(run_check(
        &workspace,
        "Pruebas workspace",
        "cargo",
        &["test", "--workspace", "--locked"],
        false,
    ));
    results.push(run_check(
        &workspace,
        "Auditoría de dependencias",
        "cargo",
        &["deny", "check"],
        true,
    ));
    results.push(run_check(
        &workspace,
        "Formato fuzz",
        "cargo",
        &[
            fuzz_toolchain_argument.as_str(),
            "fmt",
            "--manifest-path",
            "fuzz/Cargo.toml",
            "--all",
            "--",
            "--check",
        ],
        true,
    ));
    results.push(run_check(
        &workspace,
        "Compilación de harnesses fuzz",
        "cargo",
        &[
            fuzz_toolchain_argument.as_str(),
            "check",
            "--manifest-path",
            "fuzz/Cargo.toml",
            "--bins",
            "--locked",
        ],
        true,
    ));
    for target in FUZZ_TARGETS {
        let name = format!("Fuzz {target} (1000 iteraciones)");
        let arguments = [
            fuzz_toolchain_argument.as_str(),
            "fuzz",
            "run",
            target,
            "--",
            "-runs=1000",
            "-max_len=8192",
        ];
        results.push(run_check(&workspace, &name, "cargo", &arguments, true));
    }
    results.push(run_check(
        &workspace,
        "Build del portal documental",
        "npm",
        &["run", "build"],
        true,
    ));

    let report = render_report(&workspace, &results);
    let report_path = workspace.join("target/validation-report.md");
    fs::create_dir_all(
        report_path
            .parent()
            .ok_or_else(|| "la ruta del informe no tiene directorio padre".to_owned())?,
    )
    .map_err(|error| format!("no se pudo crear el directorio del informe: {error}"))?;
    fs::write(&report_path, report)
        .map_err(|error| format!("no se pudo guardar el informe: {error}"))?;

    for result in &results {
        println!(
            "{}: {} ({:.1}s)",
            result.name,
            state_label(result.state),
            result.elapsed.as_secs_f64()
        );
    }
    println!("Informe guardado en target/validation-report.md");

    if results
        .iter()
        .any(|result| result.state == CheckState::Failed)
    {
        return Err(
            "una o más verificaciones fallaron; consulte el estado en el informe".to_owned(),
        );
    }
    Ok(())
}

fn run_check(
    workspace: &Path,
    name: &str,
    program: &str,
    arguments: &[&str],
    optional: bool,
) -> CheckResult {
    let started = Instant::now();
    let command = format!("{program} {}", arguments.join(" "));
    let output = Command::new(program)
        .args(arguments)
        .current_dir(if program == "npm" {
            workspace.join("website")
        } else {
            workspace.to_path_buf()
        })
        .output();
    let elapsed = started.elapsed();

    match output {
        Ok(output) => result_from_output(name, command, elapsed, output, optional),
        Err(error) if optional && error.kind() == std::io::ErrorKind::NotFound => CheckResult {
            name: name.to_owned(),
            command,
            state: CheckState::NotEvaluated,
            elapsed,
            exit_code: None,
            tests_passed: 0,
            tests_failed: 0,
        },
        Err(_) => CheckResult {
            name: name.to_owned(),
            command,
            state: CheckState::Failed,
            elapsed,
            exit_code: None,
            tests_passed: 0,
            tests_failed: 0,
        },
    }
}

fn result_from_output(
    name: &str,
    command: String,
    elapsed: Duration,
    output: Output,
    optional: bool,
) -> CheckResult {
    let (tests_passed, tests_failed) = parse_test_counts(&output.stdout);
    let unavailable_optional = optional
        && !output.status.success()
        && (output
            .stderr
            .windows(b"no such command".len())
            .any(|window| window == b"no such command")
            || output
                .stderr
                .windows(b"is not installed".len())
                .any(|window| window == b"is not installed"));
    CheckResult {
        name: name.to_owned(),
        command,
        state: if unavailable_optional {
            CheckState::NotEvaluated
        } else if output.status.success() {
            CheckState::Passed
        } else {
            CheckState::Failed
        },
        elapsed,
        exit_code: output.status.code(),
        tests_passed,
        tests_failed,
    }
}

fn parse_test_counts(output: &[u8]) -> (usize, usize) {
    let text = String::from_utf8_lossy(output);
    let mut passed = 0;
    let mut failed = 0;
    for line in text.lines() {
        let Some(summary) = line.split("test result:").nth(1) else {
            continue;
        };
        for entry in summary.split(';') {
            let mut count = None;
            for part in entry.split_whitespace() {
                if let Ok(value) = part.parse::<usize>() {
                    count = Some(value);
                    continue;
                }
                match (part, count.take()) {
                    ("passed", Some(value)) => passed += value,
                    ("failed", Some(value)) => failed += value,
                    _ => {}
                }
            }
        }
    }
    (passed, failed)
}

fn render_report(workspace: &Path, results: &[CheckResult]) -> String {
    let commit = git_value(workspace, &["rev-parse", "HEAD"]);
    let branch = git_value(workspace, &["branch", "--show-current"]);
    let dirty = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(workspace)
        .output()
        .map(|output| !output.stdout.is_empty())
        .unwrap_or(true);
    let rustc = version_line("rustc", &["--version"]);
    let cargo = version_line("cargo", &["--version"]);
    let fuzz_toolchain_argument = format!("+{FUZZ_TOOLCHAIN}");
    let nightly = version_line("rustc", &[fuzz_toolchain_argument.as_str(), "--version"]);
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|_| "no disponible".to_owned());
    let mut report = String::new();
    append_line(&mut report, "# Informe de validación de Óxido");
    append_line(&mut report, "");
    append_line(&mut report, "| Dato | Valor |");
    append_line(&mut report, "|---|---|");
    append_line(
        &mut report,
        &format!("| Commit | `{}` |", escape_cell(&commit)),
    );
    append_line(
        &mut report,
        &format!("| Rama | `{}` |", escape_cell(&branch)),
    );
    append_line(&mut report, &format!("| Árbol modificado | `{dirty}` |"));
    append_line(
        &mut report,
        &format!("| Momento UTC (epoch) | `{timestamp}` |"),
    );
    append_line(
        &mut report,
        &format!("| Sistema | `{} {}` |", env::consts::OS, env::consts::ARCH),
    );
    append_line(
        &mut report,
        &format!(
            "| Toolchain estable | `{}` / `{}` |",
            escape_cell(&rustc),
            escape_cell(&cargo)
        ),
    );
    append_line(
        &mut report,
        &format!("| Toolchain fuzz | `{}` |", escape_cell(&nightly)),
    );
    append_line(
        &mut report,
        "| Métricas de rendimiento | No medidas por este informe |",
    );
    append_line(
        &mut report,
        "| CI remota | No consultada; revisar Actions para este commit |",
    );
    append_line(&mut report, "");
    append_line(&mut report, "## Verificaciones");
    append_line(&mut report, "");
    append_line(
        &mut report,
        "| Suite | Estado | Duración | Exit code | Tests aprobados | Tests fallidos | Comando |",
    );
    append_line(&mut report, "|---|---|---:|---:|---:|---:|---|");
    for result in results {
        let exit_code = result
            .exit_code
            .map_or_else(|| "—".to_owned(), |code| code.to_string());
        append_line(
            &mut report,
            &format!(
                "| {} | {} | {:.1}s | {} | {} | {} | `{}` |",
                escape_cell(&result.name),
                state_label(result.state),
                result.elapsed.as_secs_f64(),
                exit_code,
                result.tests_passed,
                result.tests_failed,
                escape_cell(&result.command)
            ),
        );
    }
    let not_evaluated = results
        .iter()
        .filter(|result| result.state == CheckState::NotEvaluated)
        .map(|result| result.name.as_str())
        .collect::<Vec<_>>();
    append_line(&mut report, "");
    append_line(
        &mut report,
        "No se capturan logs crudos de procesos, para evitar filtrar rutas absolutas. Las suites fallidas se identifican por estado y código de salida; consulte el log de CI para el detalle.",
    );
    append_line(&mut report, "");
    append_line(
        &mut report,
        &format!(
            "No evaluadas: {}.",
            if not_evaluated.is_empty() {
                "ninguna".to_owned()
            } else {
                not_evaluated.join(", ")
            }
        ),
    );
    report
}

fn append_line(report: &mut String, line: &str) {
    report.push_str(line);
    report.push('\n');
}

fn git_value(workspace: &Path, arguments: &[&str]) -> String {
    Command::new("git")
        .args(arguments)
        .current_dir(workspace)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "no disponible".to_owned())
}

fn version_line(program: &str, arguments: &[&str]) -> String {
    Command::new(program)
        .args(arguments)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "no disponible".to_owned())
}

fn state_label(state: CheckState) -> &'static str {
    match state {
        CheckState::Passed => "Aprobada",
        CheckState::Failed => "Fallida",
        CheckState::NotEvaluated => "No evaluada",
    }
}

fn escape_cell(value: &str) -> String {
    value
        .replace('|', "\\|")
        .replace('`', "'")
        .replace(['\n', '\r'], " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_cargo_test_totals_across_multiple_test_binaries() {
        let output = b"test result: ok. 4 passed; 0 failed; 0 ignored;\ntest result: FAILED. 2 passed; 1 failed; 0 ignored;\n";
        assert_eq!(parse_test_counts(output), (6, 1));
    }

    #[test]
    fn escapes_report_cells_and_labels_all_states() {
        assert_eq!(escape_cell("a|b`c\nd"), "a\\|b'c d");
        assert_eq!(state_label(CheckState::Passed), "Aprobada");
        assert_eq!(state_label(CheckState::Failed), "Fallida");
        assert_eq!(state_label(CheckState::NotEvaluated), "No evaluada");
    }

    #[test]
    fn report_metadata_does_not_include_the_absolute_workspace_path() {
        let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let absolute_workspace = workspace
            .canonicalize()
            .expect("workspace exists")
            .to_string_lossy()
            .into_owned();
        let report = render_report(&workspace, &[]);
        assert!(!report.contains(&absolute_workspace));
        assert!(report.contains("Métricas de rendimiento | No medidas"));
        assert!(report.contains("CI remota | No consultada"));
    }
}
