#![forbid(unsafe_code)]

//! Casos de uso del compilador, sin transporte ni acceso directo a procesos.

use oxido_language::SourceFile;
use std::{fs, path::Path, sync::atomic::AtomicBool};

pub mod config;
pub mod application {
    pub mod cargo;
    pub mod format;
    pub mod modules;
    pub mod source_map;
}

use application::source_map::SourceMap;
use oxido_language::domain::diagnostics::{Diagnostic, Severity};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedProgram {
    pub rust: String,
    pub source_map: SourceMap,
    pub diagnostics: Vec<Diagnostic>,
}

/// Parse and emit Rust for the currently supported `.ox` subset.
pub fn generate_ox(
    source: &SourceFile,
    options: oxido_language::domain::syntax::LexOptions,
) -> Result<GeneratedProgram, Vec<Diagnostic>> {
    let parsed = parse_ox(source, options);
    if parsed.has_errors() {
        return Err(parsed.diagnostics);
    }
    let Some(file) = parsed.file else {
        return Err(parsed.diagnostics);
    };
    let diagnostics = parsed.diagnostics;
    let generated = oxido_rust_backend::generate(&file, source).map_err(|error| {
        let span = source
            .span(0, source.text().len())
            .unwrap_or_else(oxido_language::domain::source::Span::empty);
        vec![Diagnostic::error(
            "OX4001",
            format!("no se pudo generar Rust: {error}"),
            span,
        )]
    })?;
    let mut source_map = SourceMap::new(generated.mappings);
    source_map.set_source_file(source.path());
    Ok(GeneratedProgram {
        rust: generated.code,
        source_map,
        diagnostics,
    })
}

/// Compila o ejecuta un programa generado en un workspace temporal aislado.
pub fn build_ox(
    source: &SourceFile,
    options: oxido_language::domain::syntax::LexOptions,
    action: application::cargo::CargoAction,
) -> Result<application::cargo::CargoBuildResult, Vec<Diagnostic>> {
    build_ox_in_project(source, options, &[], None, action)
}

/// Genera, compila o ejecuta `.ox` con dependencias Cargo locales validadas del proyecto.
pub fn build_ox_with_dependencies(
    source: &SourceFile,
    options: oxido_language::domain::syntax::LexOptions,
    dependencies: &[config::ResolvedDependency],
    action: application::cargo::CargoAction,
) -> Result<application::cargo::CargoBuildResult, Vec<Diagnostic>> {
    build_ox_in_project(source, options, dependencies, None, action)
}

/// Genera y ejecuta `.ox` con dependencias y lockfile persistente del proyecto.
pub fn build_ox_in_project(
    source: &SourceFile,
    options: oxido_language::domain::syntax::LexOptions,
    dependencies: &[config::ResolvedDependency],
    lockfile: Option<&Path>,
    action: application::cargo::CargoAction,
) -> Result<application::cargo::CargoBuildResult, Vec<Diagnostic>> {
    build_ox_in_project_cancellable(source, options, dependencies, lockfile, action, None)
}

/// Igual que [`build_ox_in_project`], pero aborta Cargo cuando la bandera se active.
pub fn build_ox_in_project_cancellable(
    source: &SourceFile,
    options: oxido_language::domain::syntax::LexOptions,
    dependencies: &[config::ResolvedDependency],
    lockfile: Option<&Path>,
    action: application::cargo::CargoAction,
    cancellation: Option<&AtomicBool>,
) -> Result<application::cargo::CargoBuildResult, Vec<Diagnostic>> {
    let generated = generate_ox(source, options)?;
    let mut result = application::cargo::build_generated_with_dependencies_cancellable(
        &generated.rust,
        &generated.source_map,
        dependencies,
        lockfile,
        action,
        cancellation,
    )
    .map_err(|error| {
        let span = source
            .span(0, source.text().len())
            .unwrap_or_else(oxido_language::domain::source::Span::empty);
        vec![Diagnostic::error(
            "OX5001",
            format!("no se pudo iniciar Cargo: {error}"),
            span,
        )]
    })?;
    append_warnings(&mut result, &generated.diagnostics, source);
    Ok(result)
}

fn append_warnings(
    result: &mut application::cargo::CargoBuildResult,
    diagnostics: &[Diagnostic],
    source: &SourceFile,
) {
    for diagnostic in diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Warning)
    {
        let location = source
            .position(diagnostic.primary_span.start())
            .map(|position| application::source_map::SourceLocation {
                line: position.line,
                column: position.column,
                start: diagnostic.primary_span.start(),
                end: diagnostic.primary_span.end(),
            });
        result
            .diagnostics
            .push(application::cargo::MappedCargoDiagnostic {
                level: "warning".to_owned(),
                code: Some(diagnostic.code.to_owned()),
                message: diagnostic.message.clone(),
                source_file: None,
                source: location,
            });
    }
}

/// Resuelve los módulos externos del proyecto, genera el árbol Rust y lo compila como unidad.
pub fn build_ox_project_in_project(
    source: &SourceFile,
    options: oxido_language::domain::syntax::LexOptions,
    dependencies: &[config::ResolvedDependency],
    lockfile: Option<&Path>,
    action: application::cargo::CargoAction,
    project_root: &Path,
) -> Result<application::cargo::CargoBuildResult, Vec<Diagnostic>> {
    build_ox_project_in_project_cancellable(
        source,
        options,
        dependencies,
        lockfile,
        action,
        project_root,
        None,
    )
}

/// Igual que [`build_ox_project_in_project`], pero aborta Cargo cuando la bandera se active.
pub fn build_ox_project_in_project_cancellable(
    source: &SourceFile,
    options: oxido_language::domain::syntax::LexOptions,
    dependencies: &[config::ResolvedDependency],
    lockfile: Option<&Path>,
    action: application::cargo::CargoAction,
    project_root: &Path,
    cancellation: Option<&AtomicBool>,
) -> Result<application::cargo::CargoBuildResult, Vec<Diagnostic>> {
    let generated = generate_ox(source, options)?;
    let resolved = application::modules::resolve_project_modules(
        source,
        &generated.rust,
        options,
        project_root,
    )
    .map_err(|message| {
        let span = source
            .span(0, 0)
            .unwrap_or_else(oxido_language::domain::source::Span::empty);
        vec![Diagnostic::error("OX8001", message, span)]
    })?;
    let generated = GeneratedProgram {
        rust: resolved.root_rust,
        ..generated
    };
    let mut source_map = generated.source_map.clone();
    source_map.set_generated_relative_path("src/main.rs".to_owned());
    let relative_source = source
        .path()
        .and_then(|path| fs::canonicalize(path).ok())
        .and_then(|path| path.strip_prefix(project_root).ok().map(Path::to_path_buf))
        .map(|path| path.to_string_lossy().into_owned());
    if let Some(relative_source) = relative_source {
        source_map.set_source_relative_path(relative_source);
    }
    let mut result = application::cargo::build_generated_project_with_dependencies_cancellable(
        &generated.rust,
        &source_map,
        &resolved.modules,
        dependencies,
        lockfile,
        action,
        cancellation,
    )
    .map_err(|error| {
        let span = source
            .span(0, source.text().len())
            .unwrap_or_else(oxido_language::domain::source::Span::empty);
        vec![Diagnostic::error(
            "OX5001",
            format!("no se pudo iniciar Cargo: {error}"),
            span,
        )]
    })?;
    append_warnings(&mut result, &generated.diagnostics, source);
    Ok(result)
}

/// Importa Rust al subconjunto reversible de Óxido y verifica que regenerarlo conserve Rust.
pub fn import_rust(source: &SourceFile) -> Result<String, String> {
    let imported = oxido_rust_backend::import_reversible(source)
        .map_err(|error| format!("Rust fuera del subconjunto importable: {error}"))?;
    let ox_source = SourceFile::new(imported.clone());
    let options = oxido_language::domain::syntax::LexOptions {
        project_mode: Some(oxido_language::domain::syntax::SyntaxMode::Braces),
    };
    let parsed = parse_ox(&ox_source, options);
    if parsed.has_errors() {
        return Err(format!(
            "la estructura importada no es reversible: {:?}",
            parsed.diagnostics
        ));
    }
    let regenerated = generate_ox(&ox_source, options)
        .map_err(|diagnostics| format!("falló la validación de la importación: {diagnostics:?}"))?;
    let original = oxido_rust_backend::format(
        &oxido_rust_backend::parse(source)
            .map_err(|error| format!("Rust de entrada inválido: {error}"))?,
    );
    let regenerated_semantics = oxido_rust_backend::format(
        &oxido_rust_backend::parse(&SourceFile::new(&regenerated.rust))
            .map_err(|error| format!("Rust regenerado inválido: {error}"))?,
    );
    if regenerated_semantics != original {
        return Err("la importación cambia la estructura Rust al regenerarse".into());
    }
    Ok(imported)
}

/// Reescribe valores de `#[path]` de forma estructural sin alterar trivia del archivo.
pub fn rewrite_rust_module_paths(
    source: &SourceFile,
    rewrite: impl FnMut(&str) -> Result<String, String>,
) -> Result<String, String> {
    oxido_rust_backend::rewrite_module_path_attributes(source, rewrite)
}

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

/// Formatea una fuente Rust ya validada con el backend compartido.
pub fn format_rust(source: &SourceFile) -> Result<String, String> {
    let file = oxido_rust_backend::parse(source).map_err(|error| error.to_string())?;
    Ok(oxido_rust_backend::format(&file))
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
                    if let Some(ty) = &statement.type_annotation
                        && let Err(error) = oxido_rust_backend::validate_type(&ty.text)
                    {
                        diagnostics.push(Diagnostic::error(
                            "OX4001",
                            format!("tipo Rust inválido: {error}"),
                            ty.span,
                        ));
                    }
                    if let Some(control) = &statement.match_initializer {
                        validate_control(control, diagnostics);
                    } else if let Err(error) =
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
                    if let Some(value) = &statement.value
                        && let Err(error) = oxido_rust_backend::validate_expression(&value.text)
                    {
                        diagnostics.push(Diagnostic::error(
                            "OX4001",
                            format!("expresión de retorno inválida: {error}"),
                            value.span,
                        ));
                    }
                }
                Statement::Break(statement) => {
                    if let Err(error) = oxido_rust_backend::validate_jump(
                        "break",
                        statement.value.as_ref().map(|value| value.text.as_str()),
                    ) {
                        diagnostics.push(Diagnostic::error(
                            "OX4001",
                            format!("sentencia `break` inválida: {error}"),
                            statement.span,
                        ));
                    }
                }
                Statement::Continue(statement) => {
                    if let Err(error) = oxido_rust_backend::validate_jump(
                        "continue",
                        statement.value.as_ref().map(|value| value.text.as_str()),
                    ) {
                        diagnostics.push(Diagnostic::error(
                            "OX4001",
                            format!("sentencia `continue` inválida: {error}"),
                            statement.span,
                        ));
                    }
                }
                Statement::RustItem(_) => {}
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
        if let Some(tail) = &block.tail
            && let Err(error) = oxido_rust_backend::validate_expression(&tail.value.text)
        {
            diagnostics.push(Diagnostic::error(
                "OX4001",
                format!("expresión final Rust inválida: {error}"),
                tail.value.span,
            ));
        }
    }

    fn validate_control(control: &ControlExpression, diagnostics: &mut Vec<Diagnostic>) {
        let validation = match control.kind {
            ControlKind::If | ControlKind::While => {
                oxido_rust_backend::validate_expression(&control.header.text)
            }
            ControlKind::For => oxido_rust_backend::validate_for_header(&control.header.text),
            ControlKind::Loop => Ok(()),
            ControlKind::Match => oxido_rust_backend::validate_expression(&control.header.text),
        };
        if let Err(error) = validation {
            diagnostics.push(Diagnostic::error(
                "OX4001",
                format!("cabecera Rust inválida: {error}"),
                control.header.span,
            ));
        }
        if control.kind == ControlKind::Match {
            for arm in &control.match_arms {
                if let Err(error) = oxido_rust_backend::validate_pattern(&arm.pattern.text) {
                    diagnostics.push(Diagnostic::error(
                        "OX4001",
                        format!("patrón de `coincidir` inválido: {error}"),
                        arm.pattern.span,
                    ));
                }
                if let Some(expression) = &arm.expression
                    && let Err(error) = oxido_rust_backend::validate_expression(&expression.text)
                {
                    diagnostics.push(Diagnostic::error(
                        "OX4001",
                        format!("expresión de brazo `coincidir` inválida: {error}"),
                        expression.span,
                    ));
                }
                if let Some(block) = &arm.body {
                    validate_block(block, diagnostics);
                }
            }
        } else {
            validate_block(&control.body, diagnostics);
        }
        if let Some(alternative) = &control.alternative {
            match alternative {
                ControlAlternative::Block(block) => validate_block(block, diagnostics),
                ControlAlternative::ElseIf(nested) => validate_control(nested, diagnostics),
            }
        }
    }

    let mut output = oxido_language::domain::syntax::parse_with_deadline(
        source,
        options,
        oxido_language::domain::syntax::analysis_deadline(),
    );
    if let Some(file) = &output.file {
        let mut rust_diagnostics = Vec::new();
        for item in &file.items {
            match item {
                Item::Function(function) => {
                    for attribute in &function.attributes {
                        if let Err(error) = oxido_rust_backend::validate_attribute(&attribute.text)
                        {
                            rust_diagnostics.push(Diagnostic::error(
                                "OX4001",
                                format!("atributo Rust inválido: {error}"),
                                attribute.span,
                            ));
                        }
                    }
                    if let Some(visibility) = &function.visibility
                        && let Err(error) =
                            oxido_rust_backend::validate_visibility(&visibility.text)
                    {
                        rust_diagnostics.push(Diagnostic::error(
                            "OX4001",
                            format!("visibilidad Rust inválida: {error}"),
                            visibility.span,
                        ));
                    }
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
                Item::IndentedAggregate(aggregate) => {
                    if let Err(error) = oxido_rust_backend::validate_indented_aggregate(aggregate) {
                        rust_diagnostics.push(Diagnostic::error(
                            "OX4001",
                            format!("item estructural Rust inválido: {error}"),
                            aggregate.header.span,
                        ));
                    }
                }
                Item::IndentedUse(item) => {
                    if let Err(error) = oxido_rust_backend::validate_indented_use(item) {
                        rust_diagnostics.push(Diagnostic::error(
                            "OX4001",
                            format!("importación Rust inválida: {error}"),
                            item.tree.span,
                        ));
                    }
                }
                Item::IndentedContainer(container) => {
                    if let Err(error) = oxido_rust_backend::validate_indented_container(container) {
                        rust_diagnostics.push(Diagnostic::error(
                            "OX4001",
                            format!("contenedor Rust inválido: {error}"),
                            container.header.span,
                        ));
                    }
                }
            }
        }
        cap_rust_diagnostics(&mut rust_diagnostics);
        output.diagnostics.extend(rust_diagnostics);
        if output.has_errors() {
            output.file = None;
        }
    }
    output
}

fn cap_rust_diagnostics(diagnostics: &mut Vec<Diagnostic>) {
    let max = oxido_language::domain::syntax::MAX_DIAGNOSTICS;
    if diagnostics.len() > max {
        let omitted = diagnostics.len() - max;
        diagnostics.truncate(max);
        diagnostics.push(Diagnostic::error(
            "OX6001",
            format!("se omitieron {omitted} diagnósticos adicionales"),
            oxido_language::domain::source::Span::empty(),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::cap_rust_diagnostics;
    use oxido_language::domain::diagnostics::Diagnostic;
    use oxido_language::domain::source::Span;
    use oxido_language::domain::syntax::MAX_DIAGNOSTICS;

    #[test]
    fn caps_rust_diagnostics_and_reports_the_omitted_count() {
        let mut diagnostics = (0..MAX_DIAGNOSTICS + 500)
            .map(|_| Diagnostic::error("OX4001", "fragmento inválido", Span::empty()))
            .collect::<Vec<_>>();
        cap_rust_diagnostics(&mut diagnostics);
        assert_eq!(diagnostics.len(), MAX_DIAGNOSTICS + 1);
        let summary = diagnostics.last().expect("resumen final");
        assert_eq!(summary.code, "OX6001");
        assert!(summary.message.contains("500"), "{}", summary.message);
    }

    #[test]
    fn leaves_a_list_within_the_limit_unchanged() {
        let mut diagnostics = vec![Diagnostic::error(
            "OX4001",
            "fragmento inválido",
            Span::empty(),
        )];
        cap_rust_diagnostics(&mut diagnostics);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "OX4001");
    }
}
