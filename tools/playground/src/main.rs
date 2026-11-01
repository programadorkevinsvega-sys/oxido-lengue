#![forbid(unsafe_code)]

mod http;
mod sandbox;
mod web;

use std::{
    net::{SocketAddr, TcpListener, TcpStream},
    path::PathBuf,
    process,
    str::FromStr,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
};

use serde::Deserialize;
use serde_json::json;

use crate::sandbox::{Sandbox, SandboxError};

const DEFAULT_BIND: &str = "127.0.0.1:7878";
const MAX_CONNECTIONS: usize = 16;

#[derive(Debug)]
struct Options {
    bind: SocketAddr,
    oxido: Option<PathBuf>,
    check: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RunRequest {
    fuente: String,
}

struct Playground {
    sandbox: Sandbox,
    run_lock: Mutex<()>,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        process::exit(2);
    }
}

fn run() -> Result<(), String> {
    let options = parse_options()?;
    if options.check {
        sandbox::check_environment().map_err(|error| describe_startup(&error))?;
        println!("Sandbox disponible: bwrap, prlimit y el aislamiento responden.");
        return Ok(());
    }
    let sandbox = Sandbox::discover(options.oxido).map_err(|error| describe_startup(&error))?;
    sandbox.probe().map_err(|error| describe_startup(&error))?;

    let listener = TcpListener::bind(options.bind)
        .map_err(|error| format!("no se pudo escuchar en {}: {error}", options.bind))?;
    println!(
        "Playground de Óxido en http://{} (sin red, límites activos)",
        options.bind
    );
    if !options.bind.ip().is_loopback() {
        eprintln!("aviso: la interfaz no es local; expone el sandbox a la red");
    }

    let playground = Arc::new(Playground {
        sandbox,
        run_lock: Mutex::new(()),
    });
    let active = Arc::new(AtomicUsize::new(0));

    for stream in listener.incoming() {
        let stream = match stream {
            Ok(stream) => stream,
            Err(error) => {
                eprintln!("conexión rechazada: {error}");
                continue;
            }
        };
        if active.fetch_add(1, Ordering::SeqCst) >= MAX_CONNECTIONS {
            active.fetch_sub(1, Ordering::SeqCst);
            let mut stream = stream;
            if let Err(error) = http::respond(
                &mut stream,
                503,
                "application/json; charset=utf-8",
                br#"{"error":"demasiadas conexiones"}"#,
                &[],
            ) {
                eprintln!("no se pudo rechazar la conexión: {error}");
            }
            continue;
        }
        let playground = Arc::clone(&playground);
        let thread_active = Arc::clone(&active);
        let spawn = thread::Builder::new()
            .name("oxido-playground".to_owned())
            .spawn(move || {
                let mut stream = stream;
                if let Err(error) = handle_connection(&playground, &mut stream) {
                    eprintln!("solicitud no completada: {error}");
                }
                thread_active.fetch_sub(1, Ordering::SeqCst);
            });
        if let Err(error) = spawn {
            active.fetch_sub(1, Ordering::SeqCst);
            eprintln!("no se pudo atender la conexión: {error}");
        }
    }

    Ok(())
}

fn handle_connection(playground: &Playground, stream: &mut TcpStream) -> Result<(), String> {
    http::configure(stream).map_err(|error| error.to_string())?;
    let request = match http::read_request(stream) {
        Ok(request) => request,
        Err(error) => {
            let status = match error {
                http::HttpError::TooLarge => 413,
                _ => 400,
            };
            respond_error(stream, status, &error.to_string())?;
            return Ok(());
        }
    };

    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/") => {
            http::respond(
                stream,
                200,
                "text/html; charset=utf-8",
                web::PAGE.as_bytes(),
                &[(
                    "Content-Security-Policy",
                    "default-src 'none'; style-src 'unsafe-inline'; script-src 'unsafe-inline'; connect-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'",
                )],
            )
            .map_err(|error| error.to_string())
        }
        ("GET", "/salud") => {
            let body = json!({"estado": "ok", "aislamiento": "bwrap"}).to_string();
            http::respond(
                stream,
                200,
                "application/json; charset=utf-8",
                body.as_bytes(),
                &[],
            )
            .map_err(|error| error.to_string())
        }
        ("POST", "/api/ejecutar") => handle_run(
            playground,
            stream,
            request.content_type.as_deref(),
            &request.body,
        ),
        ("GET", "/api/ejecutar") | ("POST", "/") => {
            respond_error(stream, 405, "método no permitido")
        }
        _ => respond_error(stream, 404, "ruta no encontrada"),
    }
}

fn handle_run(
    playground: &Playground,
    stream: &mut TcpStream,
    content_type: Option<&str>,
    body: &[u8],
) -> Result<(), String> {
    if !content_type.is_some_and(|value| value.starts_with("application/json")) {
        return respond_error(stream, 415, "content-type debe ser application/json");
    }
    let request = match serde_json::from_slice::<RunRequest>(body) {
        Ok(request) => request,
        Err(_) => {
            return respond_error(
                stream,
                400,
                "cuerpo JSON inválido; se espera {\"fuente\":\"...\"}",
            );
        }
    };

    let guard = match playground.run_lock.try_lock() {
        Ok(guard) => guard,
        Err(std::sync::TryLockError::WouldBlock) => {
            return respond_error(
                stream,
                429,
                "ya hay una ejecución en curso; inténtalo de nuevo",
            );
        }
        Err(std::sync::TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
    };
    let outcome = playground.sandbox.run(&request.fuente);
    drop(guard);

    match outcome {
        Ok(outcome) => {
            let estado = if outcome.timed_out {
                "tiempo_agotado"
            } else if outcome.exit_code == Some(0) {
                "ok"
            } else {
                "error"
            };
            let body = json!({
                "estado": estado,
                "codigo": outcome.exit_code,
                "salida": outcome.stdout,
                "errores": outcome.stderr,
                "duracion_ms": outcome.duration_ms,
                "truncado": outcome.truncated,
                "tiempo_agotado": outcome.timed_out,
            })
            .to_string();
            http::respond(
                stream,
                200,
                "application/json; charset=utf-8",
                body.as_bytes(),
                &[],
            )
            .map_err(|error| error.to_string())
        }
        Err(SandboxError::InvalidSource(message)) => respond_error(stream, 400, &message),
        Err(error) => {
            eprintln!("fallo del sandbox: {}", describe_startup(&error));
            respond_error(stream, 500, "no se pudo ejecutar el sandbox")
        }
    }
}

fn respond_error(stream: &mut TcpStream, status: u16, message: &str) -> Result<(), String> {
    let body = json!({"error": message}).to_string();
    http::respond(
        stream,
        status,
        "application/json; charset=utf-8",
        body.as_bytes(),
        &[],
    )
    .map_err(|error| error.to_string())
}

fn parse_options() -> Result<Options, String> {
    let mut bind = SocketAddr::from_str(DEFAULT_BIND)
        .map_err(|_| "la dirección por defecto no es válida".to_owned())?;
    let mut oxido = None;
    let mut check = false;
    let mut arguments = std::env::args_os().skip(1);

    while let Some(argument) = arguments.next() {
        let argument = argument
            .to_str()
            .ok_or_else(|| "los argumentos deben ser UTF-8".to_owned())?;
        match argument {
            "--bind" => {
                let value = arguments
                    .next()
                    .ok_or_else(|| "--bind requiere una dirección".to_owned())?;
                let value = value
                    .to_str()
                    .ok_or_else(|| "la dirección debe ser UTF-8".to_owned())?;
                bind = value
                    .parse()
                    .map_err(|_| format!("dirección inválida: {value}"))?;
            }
            "--oxido" => {
                let value = arguments
                    .next()
                    .ok_or_else(|| "--oxido requiere una ruta".to_owned())?;
                oxido = Some(PathBuf::from(value));
            }
            "--check" => check = true,
            "--ayuda" | "-h" => {
                print_help();
                process::exit(0);
            }
            other => return Err(format!("opción desconocida: {other}\n{HELP}")),
        }
    }

    Ok(Options { bind, oxido, check })
}

fn describe_startup(error: &SandboxError) -> String {
    match error {
        SandboxError::Unavailable(message) => message.clone(),
        SandboxError::InvalidSource(message) => message.clone(),
        SandboxError::Timeout => "el aislamiento no respondió a tiempo".to_owned(),
        SandboxError::Io(error) => format!("error de E/S del sandbox ({:?})", error.kind()),
    }
}

const HELP: &str = "uso: oxido-playground [--bind 127.0.0.1:7878] [--oxido RUTA] [--check]";

fn print_help() {
    println!("{HELP}");
    println!("  --bind   dirección local de escucha (por defecto {DEFAULT_BIND})");
    println!("  --oxido  binario oxido que se ejecutará dentro del sandbox");
    println!("  --check  comprueba bwrap, prlimit y el aislamiento, y termina");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_loopback() {
        let address = SocketAddr::from_str(DEFAULT_BIND).expect("dirección válida");
        assert!(address.ip().is_loopback());
    }

    #[test]
    fn rejects_unknown_json_fields() {
        let error = serde_json::from_str::<RunRequest>(r#"{"fuente":"x","extra":1}"#)
            .expect_err("debe rechazar campos desconocidos");
        assert!(error.to_string().contains("unknown field"));
    }

    #[test]
    fn parses_the_run_request() {
        let request = serde_json::from_str::<RunRequest>(r#"{"fuente":"funcion main():"}"#)
            .expect("solicitud válida");
        assert_eq!(request.fuente, "funcion main():");
    }

    #[test]
    fn page_is_self_contained_and_uses_the_local_api() {
        assert!(web::PAGE.contains("/api/ejecutar"));
        assert!(web::PAGE.contains("64 KiB"));
        assert!(web::PAGE.contains("2 GiB"));
        assert!(!web::PAGE.contains("http://"));
        assert!(!web::PAGE.contains("https://"));
    }
}
