#![forbid(unsafe_code)]

//! Adaptador de proceso para Cargo. No interpola argumentos ni utiliza un shell.

use serde_json::Value;
use std::{
    ffi::OsString,
    io::{self, BufRead, BufReader, Read},
    path::Path,
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, RecvTimeoutError},
    },
    thread,
    time::{Duration, Instant},
};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CargoOutput {
    pub success: bool,
    pub timed_out: bool,
    pub canceled: bool,
    pub exit_code: Option<i32>,
    pub messages: Vec<Value>,
    pub non_json_stdout: Vec<String>,
    pub stderr: String,
}

enum CargoEventHandler<'a> {
    CollectOnly,
    Notify(&'a mut dyn FnMut(&Value)),
}

impl CargoEventHandler<'_> {
    fn notify(&mut self, event: &Value) {
        if let Self::Notify(callback) = self {
            callback(event);
        }
    }
}

/// Ejecuta Cargo con argv separado, stdin cerrado y directorio de trabajo explícito.
pub fn run(cwd: &Path, args: &[OsString]) -> io::Result<CargoOutput> {
    run_with_timeout(cwd, args, None, DEFAULT_TIMEOUT)
}

/// Ejecuta Cargo y permite abortar el proceso cuando la bandera se active.
pub fn run_cancellable(
    cwd: &Path,
    args: &[OsString],
    cancellation: Option<&AtomicBool>,
) -> io::Result<CargoOutput> {
    run_with_timeout_cancellable(cwd, args, None, DEFAULT_TIMEOUT, cancellation)
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
    run_with_timeout_cancellable(cwd, args, cargo_home, timeout, None)
}

pub fn run_with_timeout_cancellable(
    cwd: &Path,
    args: &[OsString],
    cargo_home: Option<&Path>,
    timeout: Duration,
    cancellation: Option<&AtomicBool>,
) -> io::Result<CargoOutput> {
    let program = std::env::var_os("OXIDO_CARGO_BINARY").unwrap_or_else(|| OsString::from("cargo"));
    run_process(
        program,
        cwd,
        args,
        cargo_home,
        Some(timeout),
        cancellation,
        CargoEventHandler::CollectOnly,
    )
}

/// Ejecuta Cargo sin timeout fijo y entrega cada evento JSON conforme se recibe.
pub fn run_without_timeout_cancellable_with_progress(
    cwd: &Path,
    args: &[OsString],
    cargo_home: Option<&Path>,
    cancellation: Option<&AtomicBool>,
    mut on_event: impl FnMut(&Value),
) -> io::Result<CargoOutput> {
    let program = std::env::var_os("OXIDO_CARGO_BINARY").unwrap_or_else(|| OsString::from("cargo"));
    run_process(
        program,
        cwd,
        args,
        cargo_home,
        None,
        cancellation,
        CargoEventHandler::Notify(&mut on_event),
    )
}

fn run_process(
    program: OsString,
    cwd: &Path,
    args: &[OsString],
    cargo_home: Option<&Path>,
    timeout: Option<Duration>,
    cancellation: Option<&AtomicBool>,
    mut on_event: CargoEventHandler<'_>,
) -> io::Result<CargoOutput> {
    if cancellation.is_some_and(|flag| flag.load(Ordering::Relaxed)) {
        return Ok(canceled_output());
    }
    let mut command = Command::new(program);
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
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("no se pudo capturar stdout de Cargo"))?;
    let mut stderr = child
        .stderr
        .take()
        .ok_or_else(|| io::Error::other("no se pudo capturar stderr de Cargo"))?;
    let (stdout_sender, stdout_receiver) = mpsc::channel();
    let stdout_reader = thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => break Ok(()),
                Ok(_) => {
                    if stdout_sender.send(line).is_err() {
                        break Ok(());
                    }
                }
                Err(error) => break Err(error),
            }
        }
    });
    let stderr_reader = thread::spawn(move || {
        let mut output = Vec::new();
        stderr.read_to_end(&mut output)?;
        Ok::<Vec<u8>, io::Error>(output)
    });
    let started = Instant::now();
    let mut timed_out = false;
    let mut canceled = false;
    let mut status = None;
    let mut stdout_open = true;
    let mut messages = Vec::new();
    let mut non_json_stdout = Vec::new();
    while status.is_none() || stdout_open {
        if status.is_none() {
            if cancellation.is_some_and(|flag| flag.load(Ordering::Relaxed)) {
                canceled = true;
                kill_process_tree(&mut child)?;
                status = Some(child.wait()?);
            } else if timeout.is_some_and(|limit| started.elapsed() >= limit) {
                timed_out = true;
                kill_process_tree(&mut child)?;
                status = Some(child.wait()?);
            } else if let Some(child_status) = child.try_wait()? {
                status = Some(child_status);
            }
        }
        if stdout_open {
            match stdout_receiver.recv_timeout(Duration::from_millis(20)) {
                Ok(line) => {
                    parse_stdout_line(&line, &mut messages, &mut non_json_stdout, &mut on_event)
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => stdout_open = false,
            }
        }
    }
    stdout_reader.join().map_err(|error| {
        io::Error::other(format!("falló el lector de stdout de Cargo: {error:?}"))
    })??;
    let stderr = stderr_reader.join().map_err(|error| {
        io::Error::other(format!("falló el lector de stderr de Cargo: {error:?}"))
    })??;
    let status = status.ok_or_else(|| io::Error::other("Cargo terminó sin estado de salida"))?;
    Ok(CargoOutput {
        success: status.success() && !timed_out && !canceled,
        timed_out,
        canceled,
        exit_code: status.code(),
        messages,
        non_json_stdout,
        stderr: String::from_utf8_lossy(&stderr).into_owned(),
    })
}

fn canceled_output() -> CargoOutput {
    CargoOutput {
        success: false,
        timed_out: false,
        canceled: true,
        exit_code: None,
        messages: Vec::new(),
        non_json_stdout: Vec::new(),
        stderr: String::new(),
    }
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

fn parse_stdout_line(
    line: &str,
    messages: &mut Vec<Value>,
    non_json_stdout: &mut Vec<String>,
    on_event: &mut CargoEventHandler<'_>,
) {
    match serde_json::from_str::<Value>(line) {
        Ok(message) if message.get("reason").and_then(Value::as_str).is_some() => {
            on_event.notify(&message);
            messages.push(message);
        }
        _ if !line.trim().is_empty() => non_json_stdout.push(line.trim_end().to_owned()),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn parses_cargo_json_diagnostics_and_preserves_external_output() {
        let source = concat!(
            "{\"reason\":\"compiler-message\",\"message\":{\"message\":\"mismatch\"}}\n",
            "Hola\n",
            "{\"program\":\"json output\"}\n"
        );
        let mut messages = Vec::new();
        let mut output = Vec::new();
        let mut handler = CargoEventHandler::CollectOnly;
        for line in source.lines() {
            parse_stdout_line(line, &mut messages, &mut output, &mut handler);
        }
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

    #[test]
    fn applies_an_injected_timeout_without_waiting_for_the_normal_limit() {
        let result = run_process(
            std::env::current_exe()
                .expect("ejecutable de prueba")
                .into_os_string(),
            Path::new(env!("CARGO_MANIFEST_DIR")),
            &fake_cargo_arguments(),
            None,
            Some(Duration::from_millis(20)),
            None,
            CargoEventHandler::CollectOnly,
        )
        .expect("debe poder iniciar y detener el Cargo de prueba");
        assert!(result.timed_out);
        assert!(!result.success);
    }

    #[test]
    fn streams_json_events_without_timeout_or_raw_json_output() {
        let mut events = Vec::new();
        let started = Instant::now();
        let result = run_process(
            std::env::current_exe()
                .expect("ejecutable de prueba")
                .into_os_string(),
            Path::new(env!("CARGO_MANIFEST_DIR")),
            &fake_cargo_arguments(),
            None,
            None,
            None,
            CargoEventHandler::Notify(&mut |event| {
                events.push((
                    event["reason"].as_str().unwrap_or_default().to_owned(),
                    started.elapsed(),
                ));
            }),
        )
        .expect("el Cargo de prueba debe finalizar sin timeout");

        assert!(result.success);
        assert!(!result.timed_out);
        assert!(!result.canceled);
        assert!(
            result
                .messages
                .iter()
                .any(|message| message["reason"] == "compiler-artifact")
        );
        assert!(
            result
                .messages
                .iter()
                .any(|message| message["reason"] == "build-finished")
        );
        assert!(
            !result
                .non_json_stdout
                .iter()
                .any(|line| line.starts_with("{\"reason\""))
        );
        assert_eq!(events[0].0, "compiler-artifact");
        assert!(events[0].1 < Duration::from_millis(500));
        assert_eq!(events[1].0, "build-finished");
    }

    #[test]
    fn cancels_and_waits_for_the_child_process() {
        let cancellation = std::sync::Arc::new(AtomicBool::new(false));
        let signal = std::sync::Arc::clone(&cancellation);
        let cancel_thread = thread::spawn(move || {
            thread::sleep(Duration::from_millis(40));
            signal.store(true, Ordering::Relaxed);
        });
        let result = run_process(
            std::env::current_exe()
                .expect("ejecutable de prueba")
                .into_os_string(),
            Path::new(env!("CARGO_MANIFEST_DIR")),
            &fake_cargo_arguments(),
            None,
            None,
            Some(&cancellation),
            CargoEventHandler::CollectOnly,
        )
        .expect("el proceso debe cancelarse");
        cancel_thread
            .join()
            .expect("la señal de cancelación debe terminar");
        assert!(result.canceled);
        assert!(!result.timed_out);
        assert!(!result.success);
    }

    #[test]
    fn honors_a_cancellation_flag_before_spawning_cargo() {
        let cancellation = AtomicBool::new(true);
        let result = run_with_timeout_cancellable(
            Path::new(env!("CARGO_MANIFEST_DIR")),
            &["--version".into()],
            None,
            DEFAULT_TIMEOUT,
            Some(&cancellation),
        )
        .expect("la cancelación no debe fallar al no iniciar Cargo");
        assert!(!result.success);
        assert!(!result.timed_out);
        assert!(result.canceled);
        assert_eq!(result.exit_code, None);
        assert!(result.messages.is_empty());
        assert!(result.non_json_stdout.is_empty());
        assert!(result.stderr.is_empty());
    }

    fn fake_cargo_arguments() -> Vec<OsString> {
        vec![
            OsString::from("--exact"),
            OsString::from("tests::fake_cargo_child_emits_events_and_waits"),
            OsString::from("--nocapture"),
        ]
    }

    #[test]
    fn fake_cargo_child_emits_events_and_waits() {
        if !std::env::args().any(|argument| argument == "--exact") {
            return;
        }
        io::stdout()
            .write_all(
                b"{\"reason\":\"compiler-artifact\",\"target\":{\"name\":\"fixture_crate\"}}\n",
            )
            .expect("el evento inicial debe escribirse");
        io::stdout()
            .flush()
            .expect("el evento inicial debe vaciarse");
        thread::sleep(Duration::from_millis(600));
        io::stdout()
            .write_all(b"{\"reason\":\"build-finished\",\"success\":true}\n")
            .expect("el evento final debe escribirse");
    }
}
