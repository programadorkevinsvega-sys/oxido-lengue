use std::{
    fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("el reloj debe avanzar")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("oxido-cargo-process-{nonce}"));
        fs::create_dir(&path).expect("el directorio temporal debe crearse");
        Self(path)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            eprintln!("no se pudo limpiar el directorio de prueba: {error}");
        }
    }
}

struct Workspace {
    root: PathBuf,
    fake_cargo: PathBuf,
    finished_file: PathBuf,
    pid_file: PathBuf,
}

fn setup(directory: &Path) -> Workspace {
    let root = directory.join("workspace");
    let member = root.join("member");
    let source_directory = member.join("src");
    fs::create_dir_all(&source_directory).expect("el miembro debe crearse");
    fs::create_dir_all(root.join(".oxido")).expect("el marcador debe crearse");
    fs::write(
        root.join("oxido.toml"),
        "[language]\nsyntax = \"braces\"\nedition = \"2024\"\n\n[cargo.manifest.workspace]\nmembers = [\"member\"]\nresolver = \"3\"\n",
    )
    .expect("la configuración del workspace debe escribirse");
    fs::write(
        member.join("oxido.toml"),
        "[language]\nsyntax = \"braces\"\nedition = \"2024\"\n\n[cargo.manifest.package]\nname = \"fixture_package\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("la configuración del paquete debe escribirse");
    fs::write(
        member.join("src/main.rs"),
        "fn main() { println!(\"fixture\"); }\n",
    )
    .expect("la fuente debe escribirse");
    fs::write(
        root.join(".oxido/import-manifest.json"),
        r#"{"kind":"oxido-lossless-workspace-import"}"#,
    )
    .expect("el marcador de importación debe escribirse");

    let fake_cargo_source =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/fake_cargo.rs");
    let fake_cargo = directory.join(format!("fake-cargo{}", std::env::consts::EXE_SUFFIX));
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let compiled = Command::new(rustc)
        .arg(&fake_cargo_source)
        .args(["--edition=2024", "-o"])
        .arg(&fake_cargo)
        .output()
        .expect("rustc debe compilar el Cargo controlado");
    assert!(
        compiled.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compiled.stdout),
        String::from_utf8_lossy(&compiled.stderr)
    );

    Workspace {
        root,
        fake_cargo,
        finished_file: directory.join("cargo-finished"),
        pid_file: directory.join("cargo-pid"),
    }
}

fn cli(workspace: &Workspace, args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_oxido"));
    command
        .current_dir(&workspace.root)
        .args(args)
        .env("OXIDO_CARGO_BINARY", &workspace.fake_cargo)
        .env("OXIDO_FAKE_CARGO_FINISHED_FILE", &workspace.finished_file)
        .env("OXIDO_FAKE_CARGO_PID_FILE", &workspace.pid_file);
    command
}

fn read_stderr_lines(
    child: &mut Child,
) -> (mpsc::Receiver<String>, thread::JoinHandle<Vec<String>>) {
    let stderr = child.stderr.take().expect("stderr debe estar capturado");
    let (sender, receiver) = mpsc::channel();
    let reader = thread::spawn(move || {
        let mut lines = Vec::new();
        for line in BufReader::new(stderr).lines() {
            match line {
                Ok(line) => {
                    if sender.send(line.clone()).is_err() {
                        break;
                    }
                    lines.push(line);
                }
                Err(error) => {
                    lines.push(format!("error al leer stderr: {error}"));
                    break;
                }
            }
        }
        lines
    });
    (receiver, reader)
}

fn contains_line(lines: &[String], expected: &str) -> bool {
    lines.iter().any(|line| line.contains(expected))
}

#[test]
fn streams_cargo_progress_before_finish_and_keeps_json_stdout_valid() {
    let temp = TestDirectory::new();
    let workspace = setup(&temp.0);
    let mut command = cli(
        &workspace,
        &["verificar", "--confiar-dependencias", "--json"],
    );
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn().expect("oxido debe iniciarse");
    let (receiver, stderr_reader) = read_stderr_lines(&mut child);

    let progress = receiver
        .recv_timeout(Duration::from_secs(10))
        .expect("Checking debe mostrarse antes de terminar Cargo");
    assert!(progress.contains("Checking fixture_crate"), "{progress}");
    assert!(
        !workspace.finished_file.exists(),
        "el progreso se mostró después de finalizar el proceso"
    );

    let output = child.wait_with_output().expect("oxido debe terminar");
    let stderr = stderr_reader
        .join()
        .expect("el lector stderr debe terminar");
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        stderr.join("\n")
    );
    assert!(workspace.finished_file.is_file());
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout debe ser JSON válido");
    assert_eq!(report["schemaVersion"], 1);
    assert_eq!(report["success"], true);
    assert!(contains_line(
        &stderr,
        "Finished Cargo workspace successfully"
    ));
    assert!(
        !stderr
            .iter()
            .any(|line| line.contains(r#"{"reason":"compiler-artifact""#)),
        "el JSON crudo de Cargo no debe aparecer en stderr"
    );

    let built = cli(
        &workspace,
        &["compilar", "--confiar-dependencias", "--json"],
    )
    .output()
    .expect("oxido compilar debe iniciarse");
    assert!(
        built.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&built.stdout),
        String::from_utf8_lossy(&built.stderr)
    );
    let build_report: serde_json::Value =
        serde_json::from_slice(&built.stdout).expect("stdout de compilar debe ser JSON válido");
    assert_eq!(build_report["success"], true);
    assert!(String::from_utf8_lossy(&built.stderr).contains("Compiling fixture_crate"));
    assert!(!String::from_utf8_lossy(&built.stderr).contains(r#"{"reason":"compiler-artifact""#));
}

#[test]
fn reports_the_real_cargo_exit_code() {
    let temp = TestDirectory::new();
    let workspace = setup(&temp.0);
    let output = cli(&workspace, &["verificar", "--confiar-dependencias"])
        .env("OXIDO_FAKE_CARGO_EXIT", "37")
        .output()
        .expect("oxido debe iniciarse");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Cargo terminó con estado 37"), "{stderr}");
    assert!(
        stderr.contains("Finished Cargo workspace with errors"),
        "{stderr}"
    );
}

#[cfg(unix)]
#[test]
fn ctrl_c_reports_cancellation_and_terminates_the_cargo_process() {
    let temp = TestDirectory::new();
    let workspace = setup(&temp.0);
    let mut command = cli(
        &workspace,
        &["verificar", "--confiar-dependencias", "--json"],
    );
    command
        .env("OXIDO_FAKE_CARGO_DELAY_MS", "60000")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().expect("oxido debe iniciarse");
    let child_id = child.id();
    let (receiver, stderr_reader) = read_stderr_lines(&mut child);
    let progress = receiver
        .recv_timeout(Duration::from_secs(10))
        .expect("Cargo debe iniciar y emitir progreso");
    assert!(progress.contains("Checking fixture_crate"), "{progress}");
    let cargo_pid = fs::read_to_string(&workspace.pid_file)
        .expect("el proceso Cargo controlado debe guardar su PID");

    let signal = Command::new("kill")
        .args(["-INT", &child_id.to_string()])
        .status()
        .expect("debe poder enviar SIGINT a oxido");
    assert!(signal.success(), "kill -INT debe terminar correctamente");

    let output = child.wait_with_output().expect("oxido debe terminar");
    let stderr = stderr_reader
        .join()
        .expect("el lector stderr debe terminar");
    assert!(!output.status.success());
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("la cancelación debe conservar JSON válido");
    assert_eq!(report["success"], false);
    assert!(
        report["diagnostics"]
            .to_string()
            .contains("cancelado por el usuario"),
        "{}",
        report["diagnostics"]
    );
    assert!(!workspace.finished_file.exists());
    assert!(
        !contains_line(&stderr, "Finished Cargo workspace successfully"),
        "{}",
        stderr.join("\n")
    );

    let process_gone = Command::new("kill")
        .args(["-0", cargo_pid.trim()])
        .stderr(Stdio::null())
        .status()
        .expect("debe poder comprobar el PID");
    assert!(
        !process_gone.success(),
        "el proceso Cargo {} siguió vivo tras Ctrl+C",
        cargo_pid.trim()
    );
}
