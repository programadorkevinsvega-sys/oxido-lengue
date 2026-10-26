#![no_main]

use libfuzzer_sys::fuzz_target;
use oxido_compiler::config::ProjectConfig;

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    match ProjectConfig::parse(text) {
        Ok(config) => assert_eq!(config.language.edition, "2024"),
        Err(message) => assert!(!message.is_empty()),
    }
});
