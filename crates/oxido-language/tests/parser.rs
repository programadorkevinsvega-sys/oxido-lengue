use oxido_language::{
    SourceFile,
    domain::{
        ast::{BlockStyle, Item, Statement},
        syntax::{LexOptions, parse},
    },
};

#[test]
fn parses_indented_function_and_preserves_tail_semicolon_semantics() {
    let source = SourceFile::new(
        "sintaxis indentacion\nfuncion sumar(a: i32, b: i32) -> i32:\n    dejar resultado: i32 = a + b\n    resultado\n",
    );
    let parsed = parse(&source, LexOptions::default());
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
    let file = parsed.file.expect("AST presente");
    let Item::Function(function) = &file.items[0] else {
        panic!("se esperaba función")
    };
    assert_eq!(function.name, "sumar");
    assert_eq!(function.body.style, BlockStyle::Indentation);
    assert_eq!(function.body.statements.len(), 1);
    assert!(function.body.tail.is_some());
    let Statement::Let(let_statement) = &function.body.statements[0] else {
        panic!("se esperaba let")
    };
    assert_eq!(let_statement.pattern.text, "resultado");
    assert_eq!(let_statement.type_annotation.as_ref().unwrap().text, "i32");
    assert_eq!(let_statement.initializer.text, "a + b");
}

#[test]
fn semicolon_turns_the_final_expression_into_a_statement() {
    let source = SourceFile::new("sintaxis indentacion\nfuncion unidad():\n    42;\n");
    let parsed = parse(&source, LexOptions::default());
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
    let file = parsed.file.unwrap();
    let Item::Function(function) = &file.items[0] else {
        panic!("se esperaba función")
    };
    assert!(function.body.tail.is_none());
    let Statement::Expression(expression) = &function.body.statements[0] else {
        panic!("se esperaba expresión")
    };
    assert!(expression.had_semicolon);
    assert_eq!(expression.value.text, "42");
}

#[test]
fn parses_braced_function_and_nested_if_else_with_shared_ast() {
    let source =
        SourceFile::new("sintaxis llaves\nfn elegir(x: bool) -> i32 { if x { 1 } else { 2 } }\n");
    let parsed = parse(&source, LexOptions::default());
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
    let file = parsed.file.unwrap();
    let Item::Function(function) = &file.items[0] else {
        panic!("se esperaba función")
    };
    assert_eq!(function.body.style, BlockStyle::Braces);
    let Statement::Control(control) = &function.body.statements[0] else {
        panic!("se esperaba if como statement interior")
    };
    assert_eq!(control.header.text, "x");
    assert!(control.alternative.is_some());
}

#[test]
fn parses_visibility_attributes_async_and_rust_passthrough_items() {
    let source = SourceFile::new(
        "sintaxis llaves\n#[inline]\npub async fn cargar() {}\nstruct Dato { valor: i32 }\nuse crate::modulo;\n",
    );
    let parsed = parse(&source, LexOptions::default());
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
    let file = parsed.file.unwrap();
    assert_eq!(file.items.len(), 3);
    let Item::Function(function) = &file.items[0] else {
        panic!("se esperaba función")
    };
    assert!(function.asynchronous);
    assert_eq!(function.attributes.len(), 1);
    assert_eq!(function.visibility.as_ref().unwrap().text, "pub");
    assert!(matches!(file.items[1], Item::RustFragment(_)));
    assert!(matches!(file.items[2], Item::RustFragment(_)));
}

#[test]
fn parser_reports_incomplete_headers_and_recovers_to_next_function() {
    let source = SourceFile::new("sintaxis indentacion\nfuncion rota(\nfuncion bien():\n    1\n");
    let parsed = parse(&source, LexOptions::default());
    assert!(parsed.has_errors());
    assert!(
        parsed
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "OX2003")
    );
    assert!(parsed.file.is_none());
}

#[test]
fn parser_keeps_macro_tokens_and_unicode_spans() {
    let source = SourceFile::new(
        "sintaxis llaves\nfn prueba() { let café = matches!(si, Some(_)); café }\n",
    );
    let parsed = parse(&source, LexOptions::default());
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
    let Item::Function(function) = &parsed.file.unwrap().items[0] else {
        panic!("se esperaba función")
    };
    let Statement::Let(let_statement) = &function.body.statements[0] else {
        panic!("se esperaba let")
    };
    assert_eq!(let_statement.initializer.text, "matches!(si, Some(_))");
    assert!(let_statement.span.end() > let_statement.span.start());
}
