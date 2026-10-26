#![forbid(unsafe_code)]

use std::{
    env, fs,
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const CRUD_ADDRESS: &str = "127.0.0.1:38211";
const CRUD_PORT: u16 = 38211;
const START_TIMEOUT: Duration = Duration::from_secs(90);
const STOP_TIMEOUT: Duration = Duration::from_secs(10);

fn main() {
    if let Err(error) = run() {
        eprintln!("falló el laboratorio de proyectos de referencia: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let workspace = env::current_dir()
        .map_err(|error| format!("no se pudo resolver el directorio actual: {error}"))?;
    ensure_port_is_free()?;

    run_cli(
        &workspace,
        &["ejecutar", "prueba/interoperabilidad_std/main.ox"],
        &[],
        "interoperabilidad con std",
        false,
    )?;
    run_terminal_reference(&workspace)?;

    run_crud_case(&workspace, false)?;
    run_crud_case(&workspace, true)?;

    println!("Proyectos de referencia funcionales aprobados; rendimiento no medido.");
    Ok(())
}

fn run_terminal_reference(workspace: &Path) -> Result<(), String> {
    let oxido_binary = workspace.join("target/terminal-cli-reference/terminal-cli");
    let rust_binary = workspace.join("prueba/terminal-cli-rust/target/debug/terminal-cli");
    if !oxido_binary.is_file() || !rust_binary.is_file() {
        return Err("faltan los binarios de las referencias de terminal".to_owned());
    }

    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("no se pudo leer el reloj para la prueba CLI: {error}"))?
        .as_nanos();
    let test_directory = env::temp_dir().join(format!(
        "oxido-terminal-reference-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir(&test_directory)
        .map_err(|error| format!("no se pudo crear el directorio temporal CLI: {error}"))?;
    let fixture = (|| {
        fs::create_dir(test_directory.join("subdirectorio"))
            .map_err(|error| format!("no se pudo crear el subdirectorio CLI: {error}"))?;
        fs::write(
            test_directory.join("archivo.txt"),
            "contenido desde fixture\n",
        )
        .map_err(|error| format!("no se pudo crear el archivo temporal CLI: {error}"))?;

        let input = concat!(
            "help\n",
            "echo \"modo interactivo\"\n",
            "comando-invalido\n",
            "pwd\n",
            "ls\n",
            "cat archivo.txt\n",
            "cd subdirectorio\n",
            "pwd\n",
            "exit\n"
        );
        let oxido = run_interactive_binary(&oxido_binary, &test_directory, input, "Óxido")?;
        let rust = run_interactive_binary(&rust_binary, &test_directory, input, "Rust")?;
        if !oxido.status.success() || !rust.status.success() {
            return Err("una referencia CLI interactiva terminó con error".to_owned());
        }
        if oxido.stdout != rust.stdout || oxido.stderr != rust.stderr {
            return Err("la salida interactiva de Óxido difiere de la línea base Rust".to_owned());
        }

        let stdout = String::from_utf8_lossy(&oxido.stdout);
        let stderr = String::from_utf8_lossy(&oxido.stderr);
        for expected in [
            "Comandos disponibles:",
            "modo interactivo\n",
            "archivo.txt\nsubdirectorio/\n",
            "contenido desde fixture\n",
        ] {
            if !stdout.contains(expected) {
                return Err("la sesión CLI no produjo una salida funcional esperada".to_owned());
            }
        }
        if !stderr.contains("Comando desconocido: comando-invalido") {
            return Err("la sesión CLI no informó el comando desconocido".to_owned());
        }
        println!("CLI interactiva Óxido/Rust: salida idéntica y casos funcionales aprobados.");
        Ok(())
    })();
    let cleanup = fs::remove_dir_all(&test_directory)
        .map_err(|error| format!("no se pudo limpiar el directorio temporal CLI: {error}"));

    match (fixture, cleanup) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(fixture_error), Ok(())) => Err(fixture_error),
        (Ok(()), Err(cleanup_error)) => Err(cleanup_error),
        (Err(fixture_error), Err(cleanup_error)) => {
            Err(format!("{fixture_error}; {cleanup_error}"))
        }
    }
}

fn run_interactive_binary(
    binary: &Path,
    working_directory: &Path,
    input: &str,
    label: &str,
) -> Result<std::process::Output, String> {
    let mut child = Command::new(binary)
        .current_dir(working_directory)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("no se pudo iniciar la CLI {label}: {error}"))?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| format!("la CLI {label} no expuso stdin"))?;
    if let Err(error) = stdin.write_all(input.as_bytes()) {
        drop(stdin);
        let kill_result = child
            .kill()
            .map_err(|kill_error| format!("no se pudo detener la CLI {label}: {kill_error}"));
        let wait_result = child
            .wait()
            .map_err(|wait_error| format!("no se pudo recoger la CLI {label}: {wait_error}"));
        return match (kill_result, wait_result) {
            (Ok(()), Ok(_)) => Err(format!(
                "no se pudo enviar la sesión a la CLI {label}: {error}"
            )),
            (Err(kill_error), Ok(_)) => Err(format!(
                "no se pudo enviar la sesión a la CLI {label}: {error}; {kill_error}"
            )),
            (Ok(()), Err(wait_error)) => Err(format!(
                "no se pudo enviar la sesión a la CLI {label}: {error}; {wait_error}"
            )),
            (Err(kill_error), Err(wait_error)) => Err(format!(
                "no se pudo enviar la sesión a la CLI {label}: {error}; {kill_error}; {wait_error}"
            )),
        };
    }
    drop(stdin);
    child
        .wait_with_output()
        .map_err(|error| format!("no se pudo recoger la salida de la CLI {label}: {error}"))
}

fn run_crud_case(workspace: &Path, overflow: bool) -> Result<(), String> {
    let label = if overflow {
        "CRUD con desbordamiento de ID"
    } else {
        "CRUD concurrente"
    };
    let initial_id = if overflow { "4294967295" } else { "1" };
    println!("Iniciando {label} en un proceso aislado.");
    let mut server = RunningServer::start(workspace, initial_id)?;

    let mode = if overflow { "overflow" } else { "normal" };
    let client_result = run_cli(
        workspace,
        &[
            "ejecutar",
            "prueba/verificar_api_concurrencia.ox",
            "--confiar-dependencias",
        ],
        &[("CRUD_TEST_ADDR", CRUD_ADDRESS), ("CRUD_TEST_MODE", mode)],
        label,
        true,
    );
    let shutdown_result = server.stop();
    client_result?;
    shutdown_result?;
    ensure_port_is_free()?;
    println!("{label}: aprobado; servidor detenido y puerto libre.");
    Ok(())
}

fn run_cli(
    workspace: &Path,
    arguments: &[&str],
    environment: &[(&str, &str)],
    label: &str,
    show_output: bool,
) -> Result<(), String> {
    println!("Ejecutando referencia: {label}.");
    let output = Command::new("cargo")
        .args(["run", "--locked", "-p", "oxido-cli", "--"])
        .args(arguments)
        .envs(environment.iter().copied())
        .current_dir(workspace)
        .output()
        .map_err(|error| format!("no se pudo ejecutar Cargo para {label}: {error}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let workspace_path = workspace.to_string_lossy();
        let safe_stderr = stderr.replace(workspace_path.as_ref(), ".");
        return Err(format!(
            "{label} terminó con {}: {}",
            status_label(output.status),
            safe_stderr.trim()
        ));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    if show_output && !stdout.trim().is_empty() {
        println!("{}", stdout.trim());
    }
    Ok(())
}

struct RunningServer {
    child: Child,
    process_group: u32,
    server_group: u32,
}

impl RunningServer {
    fn start(workspace: &Path, initial_id: &str) -> Result<Self, String> {
        let target_directory = env::var_os("CARGO_TARGET_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| workspace.join("target"));
        let oxido_cli = target_directory.join("debug/oxido");
        if !oxido_cli.is_file() {
            return Err(format!(
                "no existe el CLI compilado en {}; ejecute el supervisor con `cargo run` y su mismo CARGO_TARGET_DIR",
                display_relative(&oxido_cli, workspace)
            ));
        }
        let source = workspace.join("prueba/crud_api_concurrencia.ox");
        let mut command = Command::new("setsid");
        command
            .arg(&oxido_cli)
            .arg("ejecutar")
            .arg(source)
            .arg("--confiar-dependencias")
            .current_dir(workspace)
            .env("CRUD_ADDR", CRUD_ADDRESS)
            .env("CRUD_ID_INICIAL", initial_id)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit());
        let child = command
            .spawn()
            .map_err(|error| format!("no se pudo iniciar el servidor de referencia: {error}"))?;
        let process_group = child.id();
        let mut server = Self {
            child,
            process_group,
            server_group: 0,
        };
        if let Err(error) = server.wait_until_ready() {
            let stop_result = server.stop();
            return match stop_result {
                Ok(()) => Err(error),
                Err(stop_error) => Err(format!("{error}; además falló la detención: {stop_error}")),
            };
        }
        Ok(server)
    }

    fn wait_until_ready(&mut self) -> Result<(), String> {
        let deadline = Instant::now() + START_TIMEOUT;
        while Instant::now() < deadline {
            if let Some(status) = self
                .child
                .try_wait()
                .map_err(|error| format!("no se pudo consultar el proceso servidor: {error}"))?
            {
                return Err(format!(
                    "el CLI del servidor terminó antes de abrir el puerto ({})",
                    status_label(status)
                ));
            }
            if let Ok((status, body)) = http_response(CRUD_PORT, "/__test/instance")
                && status == 200
            {
                let server_pid = body.trim().parse::<u32>().map_err(|error| {
                    format!("el endpoint de identidad devolvió un PID inválido: {error}")
                })?;
                self.server_group = process_group_for_session(server_pid, self.process_group)?;
                return Ok(());
            }
            thread::sleep(Duration::from_millis(100));
        }
        Err(format!(
            "el servidor no respondió en {CRUD_ADDRESS} en {} segundos",
            START_TIMEOUT.as_secs()
        ))
    }

    fn stop(&mut self) -> Result<(), String> {
        if self
            .child
            .try_wait()
            .map_err(|error| format!("no se pudo consultar el proceso al detenerlo: {error}"))?
            .is_some()
        {
            self.child
                .wait()
                .map_err(|error| format!("no se pudo recoger el proceso terminado: {error}"))?;
            return Ok(());
        }

        let process_group = if self.server_group == 0 {
            self.process_group
        } else {
            self.server_group
        };
        let server_target = format!("-{process_group}");
        let signal = Command::new("kill")
            .args(["-TERM", "--", server_target.as_str()])
            .status()
            .map_err(|error| format!("no se pudo enviar SIGTERM al grupo servidor: {error}"))?;
        if !signal.success() {
            return Err(format!(
                "kill devolvió {} para el grupo servidor",
                status_label(signal)
            ));
        }

        let deadline = Instant::now() + STOP_TIMEOUT;
        loop {
            if self
                .child
                .try_wait()
                .map_err(|error| format!("no se pudo consultar el proceso al detenerlo: {error}"))?
                .is_some()
            {
                self.child
                    .wait()
                    .map_err(|error| format!("no se pudo recoger el servidor detenido: {error}"))?;
                return Ok(());
            }
            if Instant::now() >= deadline {
                let kill = Command::new("kill")
                    .args(["-KILL", "--", server_target.as_str()])
                    .status()
                    .map_err(|error| {
                        format!("no se pudo forzar la detención del servidor: {error}")
                    })?;
                if !kill.success() {
                    return Err(format!(
                        "kill -KILL devolvió {} para el grupo servidor",
                        status_label(kill)
                    ));
                }
                self.child.wait().map_err(|error| {
                    format!("no se pudo recoger el servidor tras SIGKILL: {error}")
                })?;
                return Ok(());
            }
            thread::sleep(Duration::from_millis(50));
        }
    }
}

fn process_group_for_session(pid: u32, expected_session: u32) -> Result<u32, String> {
    let stat_path = format!("/proc/{pid}/stat");
    let stat = fs::read_to_string(&stat_path)
        .map_err(|error| format!("no se pudo validar la identidad del servidor: {error}"))?;
    let close = stat
        .rfind(')')
        .ok_or_else(|| "la identidad de proceso no tiene formato /proc válido".to_owned())?;
    let fields = stat[close + 1..].split_whitespace().collect::<Vec<_>>();
    let process_group = fields
        .get(2)
        .and_then(|value| value.parse::<u32>().ok())
        .ok_or_else(|| "no se pudo leer el grupo del proceso servidor".to_owned())?;
    let session = fields
        .get(3)
        .and_then(|value| value.parse::<u32>().ok())
        .ok_or_else(|| "no se pudo leer la sesión del proceso servidor".to_owned())?;
    if session != expected_session || process_group == 0 {
        return Err("el PID HTTP no pertenece a la sesión aislada de la referencia".to_owned());
    }
    Ok(process_group)
}

fn http_response(port: u16, path: &str) -> Result<(u16, String), String> {
    let address = SocketAddr::from(([127, 0, 0, 1], port));
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_millis(250))
        .map_err(|error| format!("no hay respuesta HTTP todavía: {error}"))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|error| format!("no se pudo limitar la lectura HTTP: {error}"))?;
    stream
        .write_all(
            format!("GET {path} HTTP/1.1\r\nHost: {CRUD_ADDRESS}\r\nConnection: close\r\n\r\n")
                .as_bytes(),
        )
        .map_err(|error| format!("no se pudo enviar la solicitud de salud: {error}"))?;
    let mut response = Vec::new();
    stream
        .take(4096)
        .read_to_end(&mut response)
        .map_err(|error| format!("no se pudo leer la respuesta de salud: {error}"))?;
    let response = String::from_utf8(response)
        .map_err(|error| format!("la respuesta de salud no es UTF-8: {error}"))?;
    let status = response
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|status| status.parse::<u16>().ok())
        .ok_or_else(|| "la respuesta HTTP no contiene un código de estado".to_owned())?;
    let body = response
        .split_once("\r\n\r\n")
        .map(|(_, body)| body.to_owned())
        .ok_or_else(|| "la respuesta HTTP no tiene cuerpo separado".to_owned())?;
    Ok((status, body))
}

fn ensure_port_is_free() -> Result<(), String> {
    match TcpStream::connect_timeout(
        &SocketAddr::from(([127, 0, 0, 1], CRUD_PORT)),
        Duration::from_millis(200),
    ) {
        Ok(_) => Err(format!(
            "el puerto {CRUD_ADDRESS} ya está ocupado; no se iniciará una referencia"
        )),
        Err(error) if error.kind() == std::io::ErrorKind::ConnectionRefused => Ok(()),
        Err(error) => Err(format!(
            "no se pudo comprobar el puerto {CRUD_ADDRESS}: {error}"
        )),
    }
}

fn status_label(status: ExitStatus) -> String {
    status.code().map_or_else(
        || "terminación por señal".to_owned(),
        |code| format!("código {code}"),
    )
}

fn display_relative(path: &Path, workspace: &Path) -> String {
    path.strip_prefix(workspace).map_or_else(
        |_| "target/debug/oxido".to_owned(),
        |relative| relative.display().to_string(),
    )
}
