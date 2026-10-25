use oxido_language::SourceFile;

#[test]
fn parses_formats_and_reparses_a_rust_file() {
    let source = SourceFile::new("fn suma(a:i32,b:i32)->i32{a+b}");
    let analysis = oxido_compiler::analyze_rust(&source).expect("pipeline Rust válido");
    assert_eq!(analysis.item_count, 1);
}

#[test]
fn returns_a_parser_error_for_invalid_rust() {
    let source = SourceFile::new("fn { ");
    assert!(oxido_compiler::analyze_rust(&source).is_err());
}

#[test]
fn parses_ox_structure_and_validates_rust_fragments_with_syn() {
    use oxido_language::domain::syntax::LexOptions;

    let source =
        SourceFile::new("sintaxis indentacion\nfuncion sumar(a: i32, b: i32) -> i32:\n    a + b\n");
    let parsed = oxido_compiler::parse_ox(&source, LexOptions::default());
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
    assert_eq!(parsed.file.unwrap().items.len(), 1);

    let malformed =
        SourceFile::new("sintaxis indentacion\nfuncion rota():\n    dejar x = 1 +\n    x\n");
    let parsed = oxido_compiler::parse_ox(&malformed, LexOptions::default());
    assert!(parsed.has_errors());
    assert!(parsed.file.is_none());
    assert!(
        parsed
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "OX4001")
    );
}
