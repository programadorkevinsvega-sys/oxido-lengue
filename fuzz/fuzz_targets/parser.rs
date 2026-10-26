#![no_main]

use libfuzzer_sys::fuzz_target;
use oxido_language::{
    SourceFile,
    domain::syntax::{LexOptions, SyntaxMode, parse},
};

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let source = SourceFile::new(text);
    for mode in [SyntaxMode::Braces, SyntaxMode::Indentation] {
        let output = parse(
            &source,
            LexOptions {
                project_mode: Some(mode),
            },
        );
        for diagnostic in output.diagnostics {
            assert_valid_span(&source, diagnostic.primary_span);
            for label in diagnostic.labels {
                assert_valid_span(&source, label.span);
            }
            for suggestion in diagnostic.suggestions {
                assert_valid_span(&source, suggestion.span);
            }
        }
    }
});

fn assert_valid_span(source: &SourceFile, span: oxido_language::Span) {
    assert!(span.start() <= span.end());
    assert!(span.end() <= source.text().len());
    assert!(source.text().is_char_boundary(span.start()));
    assert!(source.text().is_char_boundary(span.end()));
}
