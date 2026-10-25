use crate::domain::source::Span;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Severity {
    Error,
    Warning,
    Info,
    Hint,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Diagnostic {
    pub code: &'static str,
    pub severity: Severity,
    pub message: String,
    pub primary_span: Span,
    pub labels: Vec<DiagnosticLabel>,
    pub notes: Vec<String>,
    pub suggestions: Vec<DiagnosticSuggestion>,
    pub related_rust: Option<RelatedRustDiagnostic>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticLabel {
    pub span: Span,
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticSuggestion {
    pub span: Span,
    pub replacement: String,
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelatedRustDiagnostic {
    pub code: Option<String>,
    pub message: String,
}

impl Diagnostic {
    #[must_use]
    pub fn error(code: &'static str, message: impl Into<String>, primary_span: Span) -> Self {
        Self {
            code,
            severity: Severity::Error,
            message: message.into(),
            primary_span,
            labels: Vec::new(),
            notes: Vec::new(),
            suggestions: Vec::new(),
            related_rust: None,
        }
    }
    #[must_use]
    pub fn warning(code: &'static str, message: impl Into<String>, primary_span: Span) -> Self {
        Self {
            code,
            severity: Severity::Warning,
            message: message.into(),
            primary_span,
            labels: Vec::new(),
            notes: Vec::new(),
            suggestions: Vec::new(),
            related_rust: None,
        }
    }

    #[must_use]
    pub fn info(code: &'static str, message: impl Into<String>, primary_span: Span) -> Self {
        Self::with_severity(code, Severity::Info, message, primary_span)
    }

    #[must_use]
    pub fn hint(code: &'static str, message: impl Into<String>, primary_span: Span) -> Self {
        Self::with_severity(code, Severity::Hint, message, primary_span)
    }

    fn with_severity(
        code: &'static str,
        severity: Severity,
        message: impl Into<String>,
        primary_span: Span,
    ) -> Self {
        Self {
            code,
            severity,
            message: message.into(),
            primary_span,
            labels: Vec::new(),
            notes: Vec::new(),
            suggestions: Vec::new(),
            related_rust: None,
        }
    }
}
