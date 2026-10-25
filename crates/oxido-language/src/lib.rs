#![forbid(unsafe_code)]

//! Dominio del lenguaje: fuentes, spans, diagnósticos y tokens.

pub mod domain {
    pub mod ast;
    pub mod diagnostics;
    pub mod source;
    pub mod syntax;
}

pub use domain::source::{SourceFile, SourcePosition, Span};
