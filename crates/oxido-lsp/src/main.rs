#![forbid(unsafe_code)]

use oxido_lsp::{ServerState, handle_json, validate_frame_length};
use std::io::{self, BufRead, Read, Write};

fn main() -> io::Result<()> {
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let stdout = io::stdout();
    let mut output = stdout.lock();
    let mut state = ServerState::default();
    loop {
        let mut length = None;
        let mut line = String::new();
        loop {
            line.clear();
            if input.read_line(&mut line)? == 0 {
                return Ok(());
            }
            if line == "\r\n" || line == "\n" {
                break;
            }
            if let Some(value) = line.strip_prefix("Content-Length:") {
                length = value.trim().parse::<usize>().ok();
            }
        }
        let length = length
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "falta Content-Length"))?;
        validate_frame_length(length)?;
        let mut body = vec![0_u8; length];
        input.read_exact(&mut body)?;
        for response in handle_json(&body, &mut state).map_err(io::Error::other)? {
            let encoded = serde_json::to_vec(&response).map_err(io::Error::other)?;
            write!(output, "Content-Length: {}\r\n\r\n", encoded.len())?;
            output.write_all(&encoded)?;
            output.flush()?;
        }
    }
}
