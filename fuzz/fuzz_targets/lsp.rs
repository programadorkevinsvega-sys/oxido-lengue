#![no_main]

use libfuzzer_sys::fuzz_target;
use oxido_lsp::{ServerState, handle_json, read_frame};
use std::io::Cursor;

fuzz_target!(|data: &[u8]| {
    let mut wire = Cursor::new(data);
    if let Ok(Some(body)) = read_frame(&mut wire) {
        let mut state = ServerState::default();
        if let Ok(responses) = handle_json(&body, &mut state) {
            assert!(responses.len() <= 2);
        }
    }
});
