#![forbid(unsafe_code)]

use oxido_lsp::{ServerState, handle_json, read_frame};
use std::io::{self, Write};

fn main() -> io::Result<()> {
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let stdout = io::stdout();
    let mut output = stdout.lock();
    let mut state = ServerState::default();
    loop {
        let Some(body) = read_frame(&mut input)? else {
            return Ok(());
        };
        for response in handle_json(&body, &mut state).map_err(io::Error::other)? {
            let encoded = serde_json::to_vec(&response).map_err(io::Error::other)?;
            write!(output, "Content-Length: {}\r\n\r\n", encoded.len())?;
            output.write_all(&encoded)?;
            output.flush()?;
        }
    }
}
