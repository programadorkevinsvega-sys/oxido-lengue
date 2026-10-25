#![forbid(unsafe_code)]

//! Casos de uso del compilador, sin transporte ni acceso directo a procesos.

use oxido_language::SourceFile;

pub mod config;

/// Resultado del análisis sintáctico Rust inicial.
#[derive(Debug, Eq, PartialEq)]
pub struct Analysis {
    /// Número de items top-level reconocidos por el parser Rust.
    pub item_count: usize,
}

/// Analiza la sintaxis Rust de una fuente y devuelve un resumen estable.
pub fn analyze_rust(source: &SourceFile) -> Result<Analysis, String> {
    let file = oxido_rust_backend::parse(source).map_err(|error| error.to_string())?;
    let formatted = oxido_rust_backend::format(&file);
    let reparsed = oxido_rust_backend::parse(&SourceFile::new(formatted))
        .map_err(|error| format!("Rust formateado no válido: {error}"))?;
    Ok(Analysis {
        item_count: reparsed.item_count(),
    })
}

/// Parse a source file and validate preserved Rust fragments through the Rust backend.
pub fn parse_ox(
    source: &SourceFile,
    options: oxido_language::domain::syntax::LexOptions,
) -> oxido_language::domain::syntax::ParseOutput {
    use oxido_language::domain::{
        ast::{Block, ControlAlternative, ControlExpression, ControlKind, Item, Statement},
        diagnostics::Diagnostic,
    };

    fn validate_block(block: &Block, diagnostics: &mut Vec<Diagnostic>) {
        for statement in &block.statements {
            match statement {
                Statement::Let(statement) => {
                    if let Err(error) =
                        oxido_rust_backend::validate_pattern(&statement.pattern.text)
                    {
                        diagnostics.push(Diagnostic::error(
                            "OX4001",
                            format!("patrón Rust inválido: {error}"),
                            statement.pattern.span,
                        ));
                    }
                    if let Some(ty) = &statement.type_annotation {
                        if let Err(error) = oxido_rust_backend::validate_type(&ty.text) {
                            diagnostics.push(Diagnostic::error(
                                "OX4001",
                                format!("tipo Rust inválido: {error}"),
                                ty.span,
                            ));
                        }
                    }
                    if let Err(error) =
                        oxido_rust_backend::validate_expression(&statement.initializer.text)
                    {
                        diagnostics.push(Diagnostic::error(
                            "OX4001",
                            format!("expresión Rust inválida: {error}"),
                            statement.initializer.span,
                        ));
                    }
                }
                Statement::Return(statement) => {
                    if let Some(value) = &statement.value {
                        if let Err(error) = oxido_rust_backend::validate_expression(&value.text) {
                            diagnostics.push(Diagnostic::error(
                                "OX4001",
                                format!("expresión de retorno inválida: {error}"),
                                value.span,
                            ));
                        }
                    }
                }
                Statement::Break(_) | Statement::Continue(_) | Statement::RustItem(_) => {}
                Statement::Expression(expression) => {
                    if let Err(error) =
                        oxido_rust_backend::validate_expression(&expression.value.text)
                    {
                        diagnostics.push(Diagnostic::error(
                            "OX4001",
                            format!("expresión Rust inválida: {error}"),
                            expression.value.span,
                        ));
                    }
                }
                Statement::Control(control) => {
                    validate_control(control, diagnostics);
                }
            }
        }
        if let Some(tail) = &block.tail {
            if let Err(error) = oxido_rust_backend::validate_expression(&tail.value.text) {
                diagnostics.push(Diagnostic::error(
                    "OX4001",
                    format!("expresión final Rust inválida: {error}"),
                    tail.value.span,
                ));
            }
        }
    }

    fn validate_control(control: &ControlExpression, diagnostics: &mut Vec<Diagnostic>) {
        let validation = match control.kind {
            ControlKind::If | ControlKind::While => {
                oxido_rust_backend::validate_expression(&control.header.text)
            }
            ControlKind::For => oxido_rust_backend::validate_for_header(&control.header.text),
            ControlKind::Loop => Ok(()),
        };
        if let Err(error) = validation {
            diagnostics.push(Diagnostic::error(
                "OX4001",
                format!("cabecera Rust inválida: {error}"),
                control.header.span,
            ));
        }
        validate_block(&control.body, diagnostics);
        if let Some(alternative) = &control.alternative {
            match alternative {
                ControlAlternative::Block(block) => validate_block(block, diagnostics),
                ControlAlternative::ElseIf(nested) => validate_control(nested, diagnostics),
            }
        }
    }

    let mut output = oxido_language::domain::syntax::parse(source, options);
    if let Some(file) = &output.file {
        let mut rust_diagnostics = Vec::new();
        for item in &file.items {
            match item {
                Item::Function(function) => {
                    if let Err(error) =
                        oxido_rust_backend::validate_signature(&function.signature.text)
                    {
                        rust_diagnostics.push(Diagnostic::error(
                            "OX4001",
                            format!("firma Rust inválida: {error}"),
                            function.signature.span,
                        ));
                    }
                    validate_block(&function.body, &mut rust_diagnostics);
                }
                Item::RustFragment(fragment) => {
                    if let Err(error) = oxido_rust_backend::validate_item(&fragment.text) {
                        rust_diagnostics.push(Diagnostic::error(
                            "OX4001",
                            format!("item Rust inválido: {error}"),
                            fragment.span,
                        ));
                    }
                }
            }
        }
        output.diagnostics.extend(rust_diagnostics);
        if output.has_errors() {
            output.file = None;
        }
    }
    output
}
