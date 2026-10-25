#![forbid(unsafe_code)]

//! Adaptador de proceso para Cargo. No interpola argumentos ni utiliza un shell.

use serde_json::Value;
use std::{
    ffi::OsString,
    io::{self, Read},
    path::Path,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CargoOutput {
    pub success: bool,
    pub timed_out: bool,
    pub exit_code: Option<i32>,
    pub messages: Vec<Value>,
    pub non_json_stdout: Vec<String>,
    pub stderr: String,
}

/// Ejecuta Cargo con argv separado, stdin cerrado y directorio de trabajo explícito.
pub fn run(cwd: &Path, args: &[OsString]) -> io::Result<CargoOutput> {
    run_with_timeout(cwd, args, None, DEFAULT_TIMEOUT)
}

/// Ejecuta Cargo usando un `CARGO_HOME` aislado del proyecto anfitrión cuando se indique.
pub fn run_with_cargo_home(
    cwd: &Path,
    args: &[OsString],
    cargo_home: Option<&Path>,
) -> io::Result<CargoOutput> {
    run_with_timeout(cwd, args, cargo_home, DEFAULT_TIMEOUT)
}

pub fn run_with_timeout(
    cwd: &Path,
    args: &[OsString],
    cargo_home: Option<&Path>,
    timeout: Duration,
) -> io::Result<CargoOutput> {
    let mut command = Command::new("cargo");
    command
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(cargo_home) = cargo_home {
        command.env("CARGO_HOME", cargo_home);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0000_0200);
    }
    let mut child = command.spawn()?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("no se pudo capturar stdout de Cargo"))?;
    let mut stderr = child
        .stderr
        .take()
        .ok_or_else(|| io::Error::other("no se pudo capturar stderr de Cargo"))?;
    let stdout_reader = thread::spawn(move || {
        let mut output = Vec::new();
        stdout.read_to_end(&mut output).map(|_| output)
    });
    let stderr_reader = thread::spawn(move || {
        let mut output = Vec::new();
        stderr.read_to_end(&mut output).map(|_| output)
    });
    let started = Instant::now();
    let mut timed_out = false;
    let status = loop {
        if started.elapsed() >= timeout {
            timed_out = true;
            kill_process_tree(&mut child)?;
            break child.wait()?;
        }
        if let Some(status) = child.try_wait()? {
            break status;
        }
        thread::sleep(Duration::from_millis(20));
    };
    let stdout = stdout_reader
        .join()
        .map_err(|_| io::Error::other("falló el lector de stdout de Cargo"))??;
    let stderr = stderr_reader
        .join()
        .map_err(|_| io::Error::other("falló el lector de stderr de Cargo"))??;
    let (messages, non_json_stdout) = parse_stdout(&String::from_utf8_lossy(&stdout));
    Ok(CargoOutput {
        success: status.success() && !timed_out,
        timed_out,
        exit_code: status.code(),
        messages,
        non_json_stdout,
        stderr: String::from_utf8_lossy(&stderr).into_owned(),
    })
}

fn kill_process_tree(child: &mut std::process::Child) -> io::Result<()> {
    let process_id = child.id().to_string();
    #[cfg(unix)]
    let killed = Command::new("kill")
        .args(["-KILL", "--"])
        .arg(format!("-{process_id}"))
        .status()
        .is_ok_and(|status| status.success());
    #[cfg(windows)]
    let killed = Command::new("taskkill")
        .args(["/PID", &process_id, "/T", "/F"])
        .status()
        .is_ok_and(|status| status.success());
    #[cfg(not(any(unix, windows)))]
    let killed = false;
    if !killed {
        child.kill()?;
    }
    Ok(())
}

fn parse_stdout(stdout: &str) -> (Vec<Value>, Vec<String>) {
    let mut messages = Vec::new();
    let mut non_json_stdout = Vec::new();
    for line in stdout.lines() {
        match serde_json::from_str::<Value>(line) {
            Ok(message) if message.get("reason").and_then(Value::as_str).is_some() => {
                messages.push(message);
            }
            _ if !line.trim().is_empty() => non_json_stdout.push(line.to_owned()),
            _ => {}
        }
    }
    (messages, non_json_stdout)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_cargo_json_diagnostics_and_preserves_external_output() {
        let source = concat!(
            "{\"reason\":\"compiler-message\",\"message\":{\"message\":\"mismatch\"}}\n",
            "Hola\n",
            "{\"program\":\"json output\"}\n"
        );
        let (messages, output) = parse_stdout(source);
        assert_eq!(messages[0]["reason"], "compiler-message");
        assert_eq!(messages[0]["message"]["message"], "mismatch");
        assert_eq!(output, ["Hola", "{\"program\":\"json output\"}"]);
    }

    #[test]
    fn invokes_cargo_without_a_shell() {
        let result = run(Path::new(env!("CARGO_MANIFEST_DIR")), &["--version".into()])
            .expect("cargo disponible en el entorno de pruebas");
        assert!(result.success, "{}", result.stderr);
        assert!(!result.non_json_stdout.is_empty());
    }

    #[test]
    fn enforces_the_process_timeout() {
        let result = run_with_timeout(
            Path::new(env!("CARGO_MANIFEST_DIR")),
            &["--version".into()],
            None,
            Duration::ZERO,
        )
        .expect("debe poder iniciar y detener Cargo");
        assert!(result.timed_out);
        assert!(!result.success);
    }
}
