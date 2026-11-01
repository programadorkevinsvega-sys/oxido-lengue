use std::{
    io::{self, Read, Write},
    net::TcpStream,
    time::Duration,
};

pub const MAX_BODY_BYTES: usize = 96 * 1024;
const MAX_HEADER_BYTES: usize = 16 * 1024;
const SOCKET_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug)]
pub struct Request {
    pub method: String,
    pub path: String,
    pub content_type: Option<String>,
    pub body: Vec<u8>,
}

#[derive(Debug)]
pub enum HttpError {
    Io(io::Error),
    TooLarge,
    Invalid(String),
}

impl std::fmt::Display for HttpError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "error de red: {error}"),
            Self::TooLarge => write!(formatter, "la solicitud excede el tamaño permitido"),
            Self::Invalid(message) => write!(formatter, "solicitud inválida: {message}"),
        }
    }
}

impl std::error::Error for HttpError {}

impl From<io::Error> for HttpError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

pub fn configure(stream: &TcpStream) -> io::Result<()> {
    stream.set_read_timeout(Some(SOCKET_TIMEOUT))?;
    stream.set_write_timeout(Some(SOCKET_TIMEOUT))?;
    stream.set_nodelay(true)?;
    Ok(())
}

pub fn read_request(stream: &mut TcpStream) -> Result<Request, HttpError> {
    let mut header = Vec::with_capacity(1024);
    let mut byte = [0_u8; 1];
    loop {
        let read = stream.read(&mut byte)?;
        if read == 0 {
            return Err(HttpError::Invalid(
                "conexión cerrada antes de las cabeceras".into(),
            ));
        }
        header.push(byte[0]);
        if header.len() > MAX_HEADER_BYTES {
            return Err(HttpError::TooLarge);
        }
        if header.ends_with(b"\r\n\r\n") {
            break;
        }
    }

    let text = std::str::from_utf8(&header)
        .map_err(|_| HttpError::Invalid("las cabeceras no son UTF-8".into()))?;
    let mut lines = text.split("\r\n");
    let request_line = lines
        .next()
        .ok_or_else(|| HttpError::Invalid("falta la línea de solicitud".into()))?;
    let mut parts = request_line.split_whitespace();
    let method = parts
        .next()
        .ok_or_else(|| HttpError::Invalid("falta el método".into()))?;
    let target = parts
        .next()
        .ok_or_else(|| HttpError::Invalid("falta la ruta".into()))?;
    let version = parts
        .next()
        .ok_or_else(|| HttpError::Invalid("falta la versión HTTP".into()))?;
    if parts.next().is_some() {
        return Err(HttpError::Invalid(
            "la línea de solicitud tiene campos extra".into(),
        ));
    }
    if version != "HTTP/1.1" && version != "HTTP/1.0" {
        return Err(HttpError::Invalid(
            "solo se admite HTTP/1.0 o HTTP/1.1".into(),
        ));
    }

    let mut content_length = 0_usize;
    let mut content_type = None;
    for line in lines {
        if line.is_empty() {
            continue;
        }
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| HttpError::Invalid("cabecera sin dos puntos".into()))?;
        let name = name.trim().to_ascii_lowercase();
        let value = value.trim();
        match name.as_str() {
            "content-length" => {
                content_length = value
                    .parse::<usize>()
                    .map_err(|_| HttpError::Invalid("content-length no es un entero".into()))?;
            }
            "content-type" => {
                content_type = Some(value.to_ascii_lowercase());
            }
            "transfer-encoding" => {
                return Err(HttpError::Invalid(
                    "transfer-encoding no está soportado".into(),
                ));
            }
            "expect" => {
                return Err(HttpError::Invalid("expect no está soportado".into()));
            }
            _ => {}
        }
    }

    if content_length > MAX_BODY_BYTES {
        return Err(HttpError::TooLarge);
    }
    let mut body = vec![0_u8; content_length];
    if content_length > 0 {
        stream.read_exact(&mut body)?;
    }

    Ok(Request {
        method: method.to_owned(),
        path: target.split('?').next().unwrap_or(target).to_owned(),
        content_type,
        body,
    })
}

pub fn respond(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &[u8],
    extra_headers: &[(&str, &str)],
) -> io::Result<()> {
    let mut response = Vec::with_capacity(body.len() + 256);
    response.extend_from_slice(
        format!(
            "HTTP/1.1 {status} {}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\n",
            reason(status),
            body.len()
        )
        .as_bytes(),
    );
    for (name, value) in extra_headers {
        response.extend_from_slice(format!("{name}: {value}\r\n").as_bytes());
    }
    response.extend_from_slice(b"\r\n");
    response.extend_from_slice(body);
    stream.write_all(&response)?;
    stream.flush()
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        413 => "Payload Too Large",
        415 => "Unsupported Media Type",
        429 => "Too Many Requests",
        503 => "Service Unavailable",
        _ => "Error",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn reads_a_bounded_json_request() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("puerto efímero");
        let address = listener.local_addr().expect("dirección local");
        let client = std::thread::spawn(move || {
            let mut stream = TcpStream::connect(address).expect("conexión");
            stream
                .write_all(
                    b"POST /api/ejecutar HTTP/1.1\r\nHost: local\r\nContent-Type: application/json\r\nContent-Length: 14\r\n\r\n{\"fuente\":\"x\"}",
                )
                .expect("escritura");
            stream
                .shutdown(std::net::Shutdown::Write)
                .expect("cierre de escritura");
        });
        let (mut stream, _) = listener.accept().expect("aceptar");
        configure(&stream).expect("timeouts");
        let request = read_request(&mut stream).expect("solicitud válida");
        assert_eq!(request.method, "POST");
        assert_eq!(request.path, "/api/ejecutar");
        assert_eq!(request.content_type.as_deref(), Some("application/json"));
        assert_eq!(request.body, b"{\"fuente\":\"x\"}");
        drop(stream);
        client.join().expect("cliente");
    }

    #[test]
    fn rejects_transfer_encoding() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("puerto efímero");
        let address = listener.local_addr().expect("dirección local");
        std::thread::spawn(move || {
            let mut stream = TcpStream::connect(address).expect("conexión");
            stream
                .write_all(b"POST / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\n")
                .expect("escritura");
        });
        let (mut stream, _) = listener.accept().expect("aceptar");
        let error = read_request(&mut stream).expect_err("debe rechazar chunked");
        assert!(matches!(error, HttpError::Invalid(_)));
    }

    #[test]
    fn rejects_bodies_over_the_limit() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("puerto efímero");
        let address = listener.local_addr().expect("dirección local");
        std::thread::spawn(move || {
            let mut stream = TcpStream::connect(address).expect("conexión");
            let header = format!(
                "POST / HTTP/1.1\r\nContent-Length: {}\r\n\r\n",
                MAX_BODY_BYTES + 1
            );
            stream.write_all(header.as_bytes()).expect("escritura");
        });
        let (mut stream, _) = listener.accept().expect("aceptar");
        let error = read_request(&mut stream).expect_err("debe rechazar el tamaño");
        assert!(matches!(error, HttpError::TooLarge));
    }
}
