use oxido_compiler::config::{ProjectConfig, SyntaxMode};

#[test]
fn parses_only_declared_language_data() {
    let config = ProjectConfig::parse("[language]\nsyntax = 'indentation'\nedition = '2024'\n")
        .expect("configuración válida");
    assert_eq!(config.language.syntax, Some(SyntaxMode::Indentation));
    assert!(ProjectConfig::parse("[language]\nedition = '2024'\n").is_ok());
}

#[test]
fn rejects_unknown_fields_and_unsupported_editions() {
    assert!(
        ProjectConfig::parse(
            "[language]\nsyntax = 'braces'\nedition = '2024'\ncommand = 'rm -rf'\n"
        )
        .is_err()
    );
    assert!(ProjectConfig::parse("[language]\nsyntax = 'braces'\nedition = '2021'\n").is_err());
}
