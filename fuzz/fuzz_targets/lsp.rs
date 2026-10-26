#![no_main]

use libfuzzer_sys::fuzz_target;
use oxido_lsp::{ServerState, handle_json, validate_frame_length};

fuzz_target!(|data: &[u8]| {
    if validate_frame_length(data.len()).is_err() {
        return;
    }
    let mut state = ServerState::default();
    if let Ok(responses) = handle_json(data, &mut state) {
        assert!(responses.len() <= 2);
    }
});
