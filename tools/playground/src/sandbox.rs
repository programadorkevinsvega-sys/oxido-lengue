use std::{
    ffi::OsString,
    fmt, fs, io,
    io::Read,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub const MAX_SOURCE_BYTES: usize = 64 * 1024;
const MAX_OUTPUT_BYTES: u64 = 64 * 1024;
const WALL_TIMEOUT: Duration = Duration::from_secs(12);
const PROBE_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug)]
pub enum SandboxError {
    Unavailable(String),
    InvalidSource(String),
    Io(io::Error),
    Timeout,
}

impl fmt::Display for SandboxError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable(message) => write!(formatter, "sandbox no disponible: {message}"),
            Self::InvalidSource(message) => write!(formatter, "fuente rechazada: {message}"),
            Self::Io(error) => write!(formatter, "error de sandbox: {error}"),
            Self::Timeout => write!(formatter, "el sandbox agotó el tiempo de pared"),
        }
    }
}

impl std::error::Error for SandboxError {}

impl From<io::Error> for SandboxError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

#[derive(Debug)]
pub struct RunOutcome {
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u128,
    pub truncated: bool,
}

#[derive(Debug)]
pub struct Sandbox {
    oxido: PathBuf,
    rustup_home: Option<PathBuf>,
    cargo_bin: Option<PathBuf>,
}

impl Sandbox {
    pub fn discover(oxido: Option<PathBuf>) -> Result<Self, SandboxError> {
        check_platform()?;
        let oxido = match oxido {
            Some(path) => path,
            None => discover_oxido()?,
        };
        if !oxido.is_file() {
            return Err(SandboxError::Unavailable(
                "no se encontró el binario oxido; use --oxido".to_owned(),
            ));
        }
        require_tool("bwrap")?;
        require_tool("prlimit")?;

        let rustup_home = env_path("RUSTUP_HOME").or_else(|| home_path(".rustup"));
        let cargo_bin = env_path("CARGO_HOME")
            .map(|home| home.join("bin"))
            .or_else(|| home_path(".cargo/bin"));

        Ok(Self {
            oxido,
            rustup_home: rustup_home.filter(|path| path.is_dir()),
            cargo_bin: cargo_bin.filter(|path| path.is_dir()),
        })
    }

    /// Comprueba que el aislamiento real funcione en esta máquina.
    pub fn probe(&self) -> Result<(), SandboxError> {
        probe_isolation()
    }

    pub fn run(&self, source: &str) -> Result<RunOutcome, SandboxError> {
        if source.len() > MAX_SOURCE_BYTES {
            return Err(SandboxError::InvalidSource(format!(
                "el límite es {MAX_SOURCE_BYTES} bytes y llegaron {}",
                source.len()
            )));
        }
        if source.contains('\0') {
            return Err(SandboxError::InvalidSource(
                "la fuente contiene un byte NUL".to_owned(),
            ));
        }

        let workspace = create_workspace()?;
        let source_path = workspace.join("programa.ox");
        let outcome = self.run_in_workspace(&workspace, &source_path, source);
        let cleanup = fs::remove_dir_all(&workspace);
        match (outcome, cleanup) {
            (Ok(mut outcome), Ok(())) => {
                outcome.stderr = sanitize(&outcome.stderr, &workspace);
                outcome.stdout = sanitize(&outcome.stdout, &workspace);
                Ok(outcome)
            }
            (Ok(mut outcome), Err(error)) => {
                outcome.stderr.push_str(&format!(
                    "\nno se pudo limpiar el directorio temporal del sandbox: {error}"
                ));
                outcome.stderr = sanitize(&outcome.stderr, &workspace);
                outcome.stdout = sanitize(&outcome.stdout, &workspace);
                Ok(outcome)
            }
            (Err(error), _) => Err(error),
        }
    }

    fn run_in_workspace(
        &self,
        workspace: &Path,
        source_path: &Path,
        source: &str,
    ) -> Result<RunOutcome, SandboxError> {
        fs::write(source_path, source)?;
        let mut command = self.command(workspace, source_path)?;
        let started = Instant::now();
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command.spawn()?;
        let stdout_pipe = child
            .stdout
            .take()
            .ok_or_else(|| io::Error::other("no se pudo capturar stdout del sandbox"))?;
        let stderr_pipe = child
            .stderr
            .take()
            .ok_or_else(|| io::Error::other("no se pudo capturar stderr del sandbox"))?;
        let stdout_reader = thread::spawn(move || read_capped(stdout_pipe));
        let stderr_reader = thread::spawn(move || read_capped(stderr_pipe));

        let mut timed_out = false;
        let status = loop {
            if let Some(status) = child.try_wait()? {
                break status;
            }
            if started.elapsed() >= WALL_TIMEOUT {
                timed_out = true;
                terminate(&mut child)?;
                break child.wait()?;
            }
            thread::sleep(Duration::from_millis(25));
        };

        let (stdout, stdout_truncated) = join_reader(stdout_reader)?;
        let (stderr, stderr_truncated) = join_reader(stderr_reader)?;

        Ok(RunOutcome {
            exit_code: status.code(),
            timed_out,
            stdout: String::from_utf8_lossy(&stdout).into_owned(),
            stderr: String::from_utf8_lossy(&stderr).into_owned(),
            duration_ms: started.elapsed().as_millis(),
            truncated: stdout_truncated || stderr_truncated,
        })
    }

    fn command(&self, workspace: &Path, source_path: &Path) -> Result<Command, SandboxError> {
        let mut command = Command::new("prlimit");
        command.args(limit_arguments()).arg("--").arg("bwrap");
        command.args(bind_arguments(
            &self.oxido,
            self.rustup_home.as_deref(),
            self.cargo_bin.as_deref(),
            source_path,
        ));
        command
            .args(["--clearenv"])
            .args(["--setenv", "PATH", sandbox_path(self.cargo_bin.as_deref())])
            .args(["--setenv", "HOME", "/work/home"])
            .args(["--setenv", "TMPDIR", "/tmp"])
            .args(["--setenv", "CARGO_HOME", "/work/.cargo"])
            .args(["--setenv", "CARGO_NET_OFFLINE", "true"])
            .args(["--setenv", "LANG", "C.UTF-8"])
            .args(["--setenv", "LC_ALL", "C.UTF-8"])
            .args(["--setenv", "TERM", "dumb"])
            .args(["--setenv", "NO_COLOR", "1"])
            .args(["--setenv", "RUST_BACKTRACE", "0"]);
        if self.rustup_home.is_some() {
            command.args(["--setenv", "RUSTUP_HOME", "/rustup"]);
        }
        if let Ok(toolchain) = std::env::var("RUSTUP_TOOLCHAIN") {
            command.args(["--setenv", "RUSTUP_TOOLCHAIN", &toolchain]);
        }
        command
            .arg("--chdir")
            .arg("/work")
            .arg("--")
            .arg("/oxido")
            .arg("ejecutar")
            .arg("programa.ox")
            .current_dir(workspace);
        Ok(command)
    }
}

/// Comprueba las herramientas y el aislamiento sin requerir el binario `oxido`.
pub fn check_environment() -> Result<(), SandboxError> {
    check_platform()?;
    require_tool("bwrap")?;
    require_tool("prlimit")?;
    probe_isolation()
}

fn check_platform() -> Result<(), SandboxError> {
    if !cfg!(target_os = "linux") {
        return Err(SandboxError::Unavailable(
            "el sandbox solo está implementado para Linux".to_owned(),
        ));
    }
    Ok(())
}

fn probe_isolation() -> Result<(), SandboxError> {
    let mut command = Command::new("prlimit");
    command
        .args(limit_arguments())
        .arg("--")
        .arg("bwrap")
        .args(probe_arguments())
        .arg("/bin/true")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let output = run_with_timeout(command, PROBE_TIMEOUT)?;
    if output.status.success() {
        Ok(())
    } else {
        let detail = String::from_utf8_lossy(&output.stderr);
        Err(SandboxError::Unavailable(format!(
            "bubblewrap no pudo crear el aislamiento: {}",
            one_line(&detail)
        )))
    }
}

fn limit_arguments() -> [&'static str; 6] {
    [
        "--as=1073741824",
        "--cpu=10",
        "--nproc=64",
        "--nofile=256",
        "--fsize=67108864",
        "--core=0",
    ]
}

fn probe_arguments() -> [&'static str; 9] {
    [
        "--unshare-all",
        "--die-with-parent",
        "--new-session",
        "--cap-drop",
        "ALL",
        "--ro-bind",
        "/",
        "/",
        "--",
    ]
}

fn bind_arguments(
    oxido: &Path,
    rustup_home: Option<&Path>,
    cargo_bin: Option<&Path>,
    source_path: &Path,
) -> Vec<OsString> {
    let mut arguments = Vec::new();
    for program in [
        "bwrap",
        "--unshare-all",
        "--die-with-parent",
        "--new-session",
        "--cap-drop",
        "ALL",
        "--ro-bind-try",
        "/usr",
        "/usr",
        "--ro-bind-try",
        "/bin",
        "/bin",
        "--ro-bind-try",
        "/lib",
        "/lib",
        "--ro-bind-try",
        "/lib64",
        "/lib64",
        "--ro-bind-try",
        "/etc/ld.so.cache",
        "/etc/ld.so.cache",
        "--ro-bind-try",
        "/etc/alternatives",
        "/etc/alternatives",
        "--ro-bind-try",
        "/etc/nsswitch.conf",
        "/etc/nsswitch.conf",
    ] {
        arguments.push(OsString::from(program));
    }
    arguments.push(OsString::from("--ro-bind"));
    arguments.push(oxido.as_os_str().to_owned());
    arguments.push(OsString::from("/oxido"));
    if let Some(cargo_bin) = cargo_bin {
        arguments.push(OsString::from("--ro-bind"));
        arguments.push(cargo_bin.as_os_str().to_owned());
        arguments.push(OsString::from("/cargo-bin"));
    }
    if let Some(rustup_home) = rustup_home {
        arguments.push(OsString::from("--ro-bind"));
        arguments.push(rustup_home.as_os_str().to_owned());
        arguments.push(OsString::from("/rustup"));
    }
    arguments.push(OsString::from("--size"));
    arguments.push(OsString::from("536870912"));
    arguments.push(OsString::from("--tmpfs"));
    arguments.push(OsString::from("/tmp"));
    arguments.push(OsString::from("--size"));
    arguments.push(OsString::from("268435456"));
    arguments.push(OsString::from("--tmpfs"));
    arguments.push(OsString::from("/work"));
    arguments.push(OsString::from("--dir"));
    arguments.push(OsString::from("/work/home"));
    arguments.push(OsString::from("--dir"));
    arguments.push(OsString::from("/work/.cargo"));
    arguments.push(OsString::from("--ro-bind"));
    arguments.push(source_path.as_os_str().to_owned());
    arguments.push(OsString::from("/work/programa.ox"));
    arguments.push(OsString::from("--proc"));
    arguments.push(OsString::from("/proc"));
    arguments.push(OsString::from("--dev"));
    arguments.push(OsString::from("/dev"));
    arguments
}

fn sandbox_path(cargo_bin: Option<&Path>) -> &'static str {
    if cargo_bin.is_some() {
        "/cargo-bin:/usr/local/bin:/usr/bin:/bin"
    } else {
        "/usr/local/bin:/usr/bin:/bin"
    }
}

fn require_tool(tool: &str) -> Result<(), SandboxError> {
    let output = Command::new(tool)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output();
    match output {
        Ok(output) if output.status.success() => Ok(()),
        Ok(_) => Err(SandboxError::Unavailable(format!(
            "{tool} no respondió correctamente"
        ))),
        Err(_) => Err(SandboxError::Unavailable(format!(
            "{tool} no está instalado"
        ))),
    }
}

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name).map(PathBuf::from)
}

fn home_path(relative: &str) -> Option<PathBuf> {
    env_path("HOME").map(|home| home.join(relative))
}

fn discover_oxido() -> Result<PathBuf, SandboxError> {
    if let Some(path) = env_path("OXIDO_BIN") {
        return Ok(path);
    }
    for candidate in [
        PathBuf::from("target/release/oxido"),
        PathBuf::from("target/debug/oxido"),
    ] {
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    let path = std::env::var_os("PATH").ok_or_else(|| {
        SandboxError::Unavailable("no hay PATH para localizar el binario oxido".to_owned())
    })?;
    for directory in std::env::split_paths(&path) {
        let candidate = directory.join("oxido");
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    Err(SandboxError::Unavailable(
        "no se encontró oxido en target/ ni en PATH; use --oxido".to_owned(),
    ))
}

fn create_workspace() -> Result<PathBuf, SandboxError> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let workspace =
        std::env::temp_dir().join(format!("oxido-playground-{}-{stamp}", std::process::id()));
    fs::create_dir(&workspace)?;
    restrict_workspace(&workspace)?;
    Ok(workspace)
}

#[cfg(unix)]
fn restrict_workspace(path: &Path) -> Result<(), SandboxError> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

#[cfg(not(unix))]
fn restrict_workspace(_path: &Path) -> Result<(), SandboxError> {
    Ok(())
}

fn read_capped(pipe: impl io::Read) -> io::Result<(Vec<u8>, bool)> {
    let mut output = Vec::new();
    pipe.take(MAX_OUTPUT_BYTES + 1).read_to_end(&mut output)?;
    let truncated = output.len() as u64 > MAX_OUTPUT_BYTES;
    if truncated {
        output.truncate(MAX_OUTPUT_BYTES as usize);
    }
    Ok((output, truncated))
}

fn join_reader(
    handle: thread::JoinHandle<io::Result<(Vec<u8>, bool)>>,
) -> Result<(Vec<u8>, bool), SandboxError> {
    handle
        .join()
        .map_err(|_| io::Error::other("falló el lector del sandbox"))?
        .map_err(SandboxError::from)
}

fn terminate(child: &mut Child) -> Result<(), SandboxError> {
    match child.kill() {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(SandboxError::Io(error)),
    }
}

fn run_with_timeout(
    mut command: Command,
    timeout: Duration,
) -> Result<std::process::Output, SandboxError> {
    let mut child = command.spawn()?;
    let started = Instant::now();
    loop {
        if child.try_wait()?.is_some() {
            return Ok(child.wait_with_output()?);
        }
        if started.elapsed() >= timeout {
            terminate(&mut child)?;
            child.wait()?;
            return Err(SandboxError::Timeout);
        }
        thread::sleep(Duration::from_millis(25));
    }
}

fn one_line(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("; ")
}

fn sanitize(text: &str, workspace: &Path) -> String {
    let workspace = workspace.to_string_lossy();
    text.replace(workspace.as_ref(), "/trabajo")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sandbox_for_tests() -> Sandbox {
        Sandbox {
            oxido: PathBuf::from("/usr/local/bin/oxido"),
            rustup_home: None,
            cargo_bin: None,
        }
    }

    #[test]
    fn rejects_sources_over_the_limit() {
        let sandbox = sandbox_for_tests();
        let source = "a".repeat(MAX_SOURCE_BYTES + 1);
        let error = sandbox.run(&source).expect_err("debe rechazar la fuente");
        assert!(matches!(error, SandboxError::InvalidSource(_)));
    }

    #[test]
    fn rejects_nul_bytes_before_spawning() {
        let sandbox = sandbox_for_tests();
        let error = sandbox
            .run("fn main() {}\0")
            .expect_err("debe rechazar NUL");
        assert!(matches!(error, SandboxError::InvalidSource(_)));
    }

    #[test]
    fn isolation_flags_disable_network_and_share_no_host_paths() {
        let sandbox = sandbox_for_tests();
        let command = sandbox
            .command(
                Path::new("/tmp/trabajo"),
                Path::new("/tmp/trabajo/programa.ox"),
            )
            .expect("debe construir el comando");
        let arguments = command
            .get_args()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert!(arguments.iter().any(|argument| argument == "--unshare-all"));
        assert!(!arguments.iter().any(|argument| argument == "--share-net"));
        assert!(
            arguments
                .iter()
                .any(|argument| argument == "--die-with-parent")
        );
        assert!(arguments.iter().any(|argument| argument == "--cap-drop"));
        assert!(arguments.iter().any(|argument| argument == "--clearenv"));
        assert!(
            arguments
                .windows(2)
                .any(|pair| pair == ["--setenv", "CARGO_NET_OFFLINE"])
        );
        assert!(
            arguments
                .iter()
                .any(|argument| argument == "CARGO_NET_OFFLINE")
        );
        assert!(arguments.iter().any(|argument| argument == "true"));
        assert!(arguments.windows(2).any(|pair| pair == ["--", "/oxido"]));
        assert!(!arguments.iter().any(|argument| argument == "/home"));
    }

    #[test]
    fn limits_include_memory_cpu_processes_and_file_size() {
        let arguments = limit_arguments();
        assert!(arguments.contains(&"--as=1073741824"));
        assert!(arguments.contains(&"--cpu=10"));
        assert!(arguments.contains(&"--nproc=64"));
        assert!(arguments.contains(&"--fsize=67108864"));
        assert!(arguments.contains(&"--core=0"));
    }

    #[test]
    fn sanitizes_workspace_paths_from_output() {
        let output = sanitize(
            "error en /tmp/oxido-playground-1-2/programa.ox",
            Path::new("/tmp/oxido-playground-1-2"),
        );
        assert_eq!(output, "error en /trabajo/programa.ox");
    }

    #[test]
    fn caps_output_and_marks_truncation() {
        let input = vec![b'a'; MAX_OUTPUT_BYTES as usize + 10];
        let (output, truncated) = read_capped(input.as_slice()).expect("lectura correcta");
        assert_eq!(output.len(), MAX_OUTPUT_BYTES as usize);
        assert!(truncated);
    }
}
