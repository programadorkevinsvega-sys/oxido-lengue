use oxido_language::{SourceFile, domain::diagnostics::Diagnostic};

#[test]
fn diagnostics_model_severity_spans_labels_suggestions_and_rust_context() {
    use oxido_language::domain::diagnostics::{
        DiagnosticLabel, DiagnosticSuggestion, RelatedRustDiagnostic, Severity,
    };

    let source = SourceFile::new("tipo");
    let span = source.span(0, 4).unwrap();
    let mut diagnostic = Diagnostic::warning("OX1003", "identificador confundible", span);
    diagnostic.labels.push(DiagnosticLabel {
        span,
        message: "rango relacionado".into(),
    });
    diagnostic.notes.push("contexto adicional".into());
    diagnostic.suggestions.push(DiagnosticSuggestion {
        span,
        replacement: "nombre".into(),
        message: "cambia el identificador".into(),
    });
    diagnostic.related_rust = Some(RelatedRustDiagnostic {
        code: Some("E0308".into()),
        message: "mismatched types".into(),
    });

    assert_eq!(diagnostic.severity, Severity::Warning);
    assert_eq!(diagnostic.primary_span, span);
    assert_eq!(diagnostic.labels.len(), 1);
    assert_eq!(diagnostic.notes.len(), 1);
    assert_eq!(diagnostic.suggestions.len(), 1);
    assert_eq!(
        diagnostic.related_rust.unwrap().code.as_deref(),
        Some("E0308")
    );
    assert_eq!(
        Diagnostic::info("OX0004", "info", span).severity,
        Severity::Info
    );
    assert_eq!(
        Diagnostic::hint("OX0005", "hint", span).severity,
        Severity::Hint
    );
}
