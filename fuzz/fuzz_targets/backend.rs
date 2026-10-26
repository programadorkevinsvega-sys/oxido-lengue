#![no_main]

use libfuzzer_sys::fuzz_target;
use oxido_compiler::{analyze_rust, generate_ox};
use oxido_language::{
    SourceFile,
    domain::syntax::{LexOptions, SyntaxMode},
};

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let source = SourceFile::new(text);
    for mode in [SyntaxMode::Braces, SyntaxMode::Indentation] {
        let options = LexOptions {
            project_mode: Some(mode),
        };
        if let Ok(generated) = generate_ox(&source, options) {
            for mapping in &generated.source_map.mappings {
                assert!(mapping.source_start <= mapping.source_end);
                assert!(mapping.source_end <= source.text().len());
                assert!(source.text().is_char_boundary(mapping.source_start));
                assert!(source.text().is_char_boundary(mapping.source_end));
            }
            assert!(
                analyze_rust(&SourceFile::new(&generated.rust)).is_ok(),
                "backend emitted Rust that Syn cannot parse"
            );
        }
    }
});
