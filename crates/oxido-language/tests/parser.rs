use oxido_language::{
    SourceFile,
    domain::{
        ast::{BlockStyle, Item, Statement},
        syntax::{LexOptions, SyntaxMode, parse, parse_with_deadline},
    },
};
use std::time::{Duration, Instant};

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
fn both_block_styles_keep_the_same_tail_and_semicolon_semantics() {
    for (source, expected_tail) in [
        ("sintaxis indentacion\nfn valor():\n    42\n", true),
        ("sintaxis indentacion\nfn valor():\n    42;\n", false),
        ("sintaxis llaves\nfn valor() { 42 }\n", true),
        ("sintaxis llaves\nfn valor() { 42; }\n", false),
    ] {
        let parsed = parse(&SourceFile::new(source), LexOptions::default());
        assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
        let Item::Function(function) = &parsed.file.unwrap().items[0] else {
            panic!("se esperaba función")
        };
        assert_eq!(function.body.tail.is_some(), expected_tail, "{source}");
        assert_eq!(function.body.statements.len(), usize::from(!expected_tail));
    }
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
    assert!(!control.had_semicolon);
}

#[test]
fn retains_the_semicolon_of_a_braced_control_statement() {
    let source = SourceFile::new(
        "sintaxis llaves\nfn ejecutar() { if true { println!(\"sí\"); }; println!(\"fin\"); }\n",
    );
    let parsed = parse(&source, LexOptions::default());
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
    let Item::Function(function) = &parsed.file.expect("archivo").items[0] else {
        panic!("se esperaba función")
    };
    let Statement::Control(control) = &function.body.statements[0] else {
        panic!("se esperaba control")
    };
    assert!(control.had_semicolon);
}

#[test]
fn parses_visibility_attributes_async_and_rust_passthrough_items() {
    let source = SourceFile::new(
        "sintaxis llaves\n#[inline]\npub async fn cargar() {}\n#[derive(Clone)]\npub struct Dato { valor: i32 }\npub(crate) use crate::{modulo, otro_modulo};\n",
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
    assert_eq!(
        function.span.start(),
        source.text().find("#[inline]").unwrap()
    );
    let Item::RustFragment(structure) = &file.items[1] else {
        panic!("se esperaba struct passthrough")
    };
    assert!(structure.text.starts_with("#[derive(Clone)]"));
    assert!(structure.text.contains("pub struct Dato"));
    let Item::RustFragment(import) = &file.items[2] else {
        panic!("se esperaba use passthrough")
    };
    assert!(import.text.starts_with("pub(crate) use"));
    assert!(import.text.ends_with("};"));
}

#[test]
fn preserves_consecutive_grouped_use_items_in_source_order() {
    let source = SourceFile::new(
        "sintaxis llaves\nuse axum::http::{HeaderMap, StatusCode};\nuse axum::response::Response;\nfn entrada() {}\n",
    );
    let parsed = parse(&source, LexOptions::default());
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
    let file = parsed.file.expect("el archivo se analiza");
    assert_eq!(file.items.len(), 3);
    let Item::RustFragment(first) = &file.items[0] else {
        panic!("se esperaba el primer use")
    };
    let Item::RustFragment(second) = &file.items[1] else {
        panic!("se esperaba el segundo use")
    };
    assert!(first.text.contains("axum::http::{HeaderMap, StatusCode};"));
    assert!(second.text.contains("axum::response::Response;"));
}

#[test]
fn preserves_all_grouped_uses_before_a_restricted_function() {
    let source = SourceFile::new(
        "sintaxis llaves\nuse axum::http::{HeaderMap, StatusCode};\nuse axum::response::Response;\nuse tonic::Request;\nuse tonic::metadata::MetadataValue;\n\nuse crate::authentication::{cookie_value, error_response, inject_trace_context};\n\npub(crate) fn session_credentials() {}\n",
    );
    let parsed = parse(&source, LexOptions::default());
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
    let file = parsed.file.expect("el archivo se analiza");
    let fragments = file
        .items
        .iter()
        .filter_map(|item| match item {
            Item::RustFragment(fragment) => Some(fragment.text.as_str()),
            Item::Function(_) => None,
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(fragments.len(), 5, "{fragments:?}");
    assert!(fragments[0].contains("axum::http"), "{fragments:?}");
    assert!(fragments[1].contains("axum::response"), "{fragments:?}");
    assert!(fragments[2].contains("tonic::Request"), "{fragments:?}");
    assert!(fragments[3].contains("tonic::metadata"), "{fragments:?}");
    assert!(
        fragments[4].contains("crate::authentication"),
        "{fragments:?}"
    );
}

#[test]
fn parses_rust_let_else_as_a_complete_passthrough_statement() {
    let source = SourceFile::new(
        "sintaxis llaves\nfn buscar(valor: Option<i32>) -> i32 { let Some(encontrado) = valor else { return 0; }; encontrado }\n",
    );
    let parsed = parse(&source, LexOptions::default());
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
    let file = parsed.file.expect("el archivo se analiza");
    let Item::Function(function) = &file.items[0] else {
        panic!("se esperaba función")
    };
    let Some(Statement::RustItem(statement)) = function.body.statements.first() else {
        panic!("se esperaba let else preservado")
    };
    assert!(statement.text.contains("let Some(encontrado) = valor else"));
    assert!(statement.text.ends_with(";"));
}

#[test]
fn preserves_slice_let_else_functions_as_opaque_rust_fragments() {
    let source = SourceFile::new(
        "sintaxis llaves\nfn ejecutar(argumentos: &[String]) { let [accion, correo] = argumentos else { return; }; println!(\"{} {}\", accion, correo); }\n",
    );
    let parsed = parse(&source, LexOptions::default());
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
    let file = parsed.file.expect("el archivo se analiza");
    let Item::RustFragment(fragment) = &file.items[0] else {
        panic!("el let con patrón slice debe conservarse como Rust opaco")
    };
    assert!(fragment.text.contains("let [accion, correo]"));
}

#[test]
fn parses_let_type_annotations_touching_the_equals_sign() {
    // Rust acepta `Vec<String>=origen`; el lexer emite `>=` como un solo
    // token y el parser debe separar el cierre genérico del inicializador.
    let source = SourceFile::new(
        "sintaxis llaves\nfn crear() -> Vec<String> { let valores: Vec<String>=vec![]; valores }\n",
    );
    let parsed = parse(&source, LexOptions::default());
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
    let file = parsed.file.expect("el archivo se analiza");
    let Item::Function(function) = &file.items[0] else {
        panic!("se esperaba función")
    };
    let Statement::Let(statement) = &function.body.statements[0] else {
        panic!("se esperaba let")
    };
    let annotation = statement.type_annotation.as_ref().expect("tipo anotado");
    assert_eq!(annotation.text, "Vec<String>");
    assert_eq!(statement.initializer.text, "vec![]");
}

#[test]
fn preserves_block_items_that_are_not_expressions() {
    let source = SourceFile::new(
        "sintaxis llaves\nfn ejecutar() { use std::fmt::Write; const LIMITE: usize = 7; static CONTADOR: usize = 1; let mut texto = String::new(); write!(&mut texto, \"{}:{}\", LIMITE, CONTADOR).unwrap(); }\n",
    );
    let parsed = parse(&source, LexOptions::default());
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
    let file = parsed.file.expect("el archivo se analiza");
    let Item::Function(function) = &file.items[0] else {
        panic!("se esperaba función")
    };
    assert!(matches!(
        function.body.statements[0],
        Statement::RustItem(_)
    ));
    assert!(matches!(
        function.body.statements[1],
        Statement::RustItem(_)
    ));
    assert!(matches!(
        function.body.statements[2],
        Statement::RustItem(_)
    ));
}

#[test]
fn preserves_unsafe_rust_functions_as_validated_items() {
    let source =
        SourceFile::new("sintaxis llaves\nunsafe fn identidad(valor: u32) -> u32 { valor }\n");
    let parsed = parse(&source, LexOptions::default());
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
    let file = parsed.file.expect("el archivo se analiza");
    let Item::RustFragment(function) = &file.items[0] else {
        panic!("la función Rust debe preservarse completa")
    };
    assert!(function.text.contains("unsafe fn identidad"));
}

#[test]
fn falls_back_to_raw_rust_function_for_macro_dsls_that_the_structural_ast_cannot_parse() {
    let source = SourceFile::new(
        "sintaxis llaves\nfn registrar(valor: &str) { tracing::info!(evento = \"registro\", valor, \"mensaje\"); }\n",
    );
    let parsed = parse(&source, LexOptions::default());
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
    let Item::RustFragment(function) = &parsed.file.expect("archivo").items[0] else {
        panic!("la función con DSL debe preservarse")
    };
    assert!(function.text.contains("tracing::info!"));
}

#[test]
fn preserves_attributed_async_rust_tests_with_result_signatures_and_macros() {
    let source = SourceFile::new(
        "sintaxis llaves\n#[tokio::test]\nasync fn integracion() -> Result<(), Box<dyn std::error::Error + Send + Sync>> { println!(\"ok\"); Ok(()) }\n",
    );
    let parsed = parse(&source, LexOptions::default());
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
    let Item::RustFragment(function) = &parsed.file.expect("archivo").items[0] else {
        panic!("la prueba Rust completa debe preservarse")
    };
    assert!(function.text.contains("#[tokio::test]"));
    assert!(function.text.contains("async fn integracion"));
}

#[test]
fn preserves_multiline_trait_impls_with_associated_types() {
    let source = SourceFile::new(
        "sintaxis llaves\nimpl<S, RequestBody> Servicio<RequestBody> for Adaptador<S>\nwhere\n    S: Send + 'static,\n    RequestBody: Send + 'static,\n{\n    type Respuesta = S;\n    fn atender(&self) -> Self::Respuesta { self.interno }\n}\n",
    );
    let parsed = parse(&source, LexOptions::default());
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
}

#[test]
fn preserves_top_level_type_aliases_with_nested_generic_bounds() {
    let source = SourceFile::new(
        "sintaxis llaves\ntype EventStream = Pin<Box<dyn Stream<Item = Result<Mensaje, Estado>> + Send>>;\n",
    );
    let parsed = parse(&source, LexOptions::default());
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
    let file = parsed.file.expect("el archivo se analiza");
    assert!(matches!(file.items[0], Item::RustFragment(_)));
}

#[test]
fn preserves_multiline_type_aliases_until_their_semicolon() {
    let source = SourceFile::new(
        "sintaxis llaves\ntype EventStream =\n    Pin<Box<dyn Stream<Item = Result<Mensaje, Estado>> + Send>>;\n",
    );
    let parsed = parse(&source, LexOptions::default());
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
    let file = parsed.file.expect("el archivo se analiza");
    let Item::RustFragment(alias) = &file.items[0] else {
        panic!("se esperaba alias de tipo Rust")
    };
    assert!(alias.text.ends_with(';'));
}

#[test]
fn parses_multiline_braced_macro_items_as_one_opaque_rust_fragment() {
    let source = SourceFile::new(
        "sintaxis indentacion\npaste::paste! {\n    pub fn [<macro_ si>]() -> &'static str {\n        stringify!({ identificador: [tipo, verdadero], texto: \"si tipo verdadero\" })\n    }\n}\nfuncion principal():\n    imprimir!(\"{}\", macro_si())\n",
    );
    let parsed = parse(&source, LexOptions::default());
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
    let file = parsed.file.expect("AST presente");
    let Item::RustFragment(macro_item) = &file.items[0] else {
        panic!("la macro debe conservarse como fragmento Rust")
    };
    assert!(macro_item.text.starts_with("paste::paste!"));
    assert!(macro_item.text.contains("[<macro_ si>]"));
    assert!(macro_item.text.contains("[tipo, verdadero]"));
    assert!(macro_item.text.ends_with('}'));
    assert!(matches!(file.items[1], Item::Function(_)));
}

#[test]
fn parses_every_initial_control_form_in_both_block_styles() {
    for source in [
        "sintaxis indentacion\nfuncion recorrer(xs: &[i32]):\n    para x en xs:\n        mientras *x > 0:\n            si *x == 1:\n                romper\n            sino:\n                continuar\n    bucle:\n        romper\n",
        "sintaxis llaves\nfn recorrer(xs: &[i32]) { for x in xs { while *x > 0 { if *x == 1 { break; } else { continue; } } } loop { break; } }\n",
    ] {
        let parsed = parse(&SourceFile::new(source), LexOptions::default());
        assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
        let Item::Function(function) = &parsed.file.unwrap().items[0] else {
            panic!("se esperaba función")
        };
        assert!(function.body.statements.iter().any(|statement| matches!(
            statement,
            Statement::Control(control) if control.kind == oxido_language::domain::ast::ControlKind::For
        )));
    }
}

#[test]
fn parser_reports_incomplete_headers_and_recovers_to_next_function() {
    let source = SourceFile::new(
        "sintaxis indentacion\nfuncion rota(\nfuncion bien():\n    1\nfuncion otra(\n",
    );
    let parsed = parse(&source, LexOptions::default());
    assert!(parsed.has_errors());
    assert!(
        parsed
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "OX2003")
    );
    assert!(parsed.diagnostics.len() >= 2, "{:?}", parsed.diagnostics);
    assert!(parsed.file.is_none());
}

#[test]
fn rejects_function_bodies_with_delimiters_from_the_other_mode() {
    for source in [
        "sintaxis llaves\nfn llaves() :\n    1\n",
        "sintaxis indentacion\nfn indentado() { 1 }\n",
    ] {
        let parsed = parse(&SourceFile::new(source), LexOptions::default());
        assert!(parsed.has_errors());
        assert!(parsed.file.is_none());
        assert!(
            parsed
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "OX2002")
        );
    }
}

#[test]
fn diagnoses_reserved_aliases_used_as_function_and_binding_names() {
    for source in [
        "sintaxis indentacion\nfuncion tipo():\n    1\n",
        "sintaxis indentacion\nfuncion valida():\n    dejar tipo = 1\n",
    ] {
        let parsed = parse(&SourceFile::new(source), LexOptions::default());
        assert!(parsed.has_errors(), "{source}");
        let reserved: Vec<_> = parsed
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == "OX3001")
            .collect();
        assert_eq!(reserved.len(), 1, "{:?}", parsed.diagnostics);
        assert!(
            reserved[0]
                .suggestions
                .iter()
                .any(|suggestion| suggestion.replacement == "r#tipo")
        );
    }
}

#[test]
fn future_reserved_aliases_are_rejected_as_names_with_raw_escape_suggestions() {
    for alias in [
        "abstracto",
        "convertirse",
        "caja",
        "hacer",
        "final",
        "generar",
        "macro",
        "sobrescribir",
        "privado",
        "intentar",
        "tipo_de",
        "sin_tamano",
        "virtual",
        "ceder",
    ] {
        let source = SourceFile::new(format!(
            "sintaxis indentacion\nfuncion {alias}():\n    retornar\n"
        ));
        let parsed = parse(&source, LexOptions::default());
        let diagnostic = parsed
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == "OX3001")
            .unwrap_or_else(|| {
                panic!(
                    "{alias} debe permanecer reservado: {:?}",
                    parsed.diagnostics
                )
            });
        assert!(
            diagnostic
                .suggestions
                .iter()
                .any(|suggestion| suggestion.replacement == format!("r#{alias}")),
            "{alias}: {:?}",
            diagnostic.suggestions
        );
    }
}

#[test]
fn diagnoses_prelude_alias_collisions_and_accepts_raw_escape() {
    let collision =
        SourceFile::new("sintaxis indentacion\nfuncion principal():\n    dejar Cadena = 1\n");
    let parsed = oxido_language::domain::syntax::parse(&collision, LexOptions::default());
    assert!(parsed.has_errors());
    assert!(parsed.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "OX3001" && diagnostic.message.contains("r#Cadena")
    }));

    let escaped = SourceFile::new(
        "sintaxis indentacion\nfuncion principal():\n    dejar r#Cadena = 1\n    imprimir!(\"{}\", r#Cadena)\n",
    );
    let parsed = oxido_language::domain::syntax::parse(&escaped, LexOptions::default());
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
}

#[test]
fn diagnoses_reserved_aliases_in_parameters_nested_patterns_and_for_bindings() {
    for source in [
        "sintaxis indentacion\nfuncion valida(tipo: i32):\n    retornar\n",
        "sintaxis indentacion\nfuncion valida():\n    dejar Some(tipo) = Some(1)\n",
        "sintaxis indentacion\nfuncion valida(valores: &[i32]):\n    para tipo en valores:\n        continuar\n",
    ] {
        let parsed = parse(&SourceFile::new(source), LexOptions::default());
        assert!(parsed.has_errors(), "{source}: {:?}", parsed.diagnostics);
        let diagnostic = parsed
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == "OX3001")
            .expect("colisión con alias reservado diagnosticada");
        assert!(
            diagnostic.message.contains("r#tipo"),
            "{}",
            diagnostic.message
        );
    }
}

#[test]
fn diagnoses_reserved_aliases_in_rust_passthrough_member_names() {
    for source in [
        "sintaxis llaves\nstruct Dato { tipo: i32 }\n",
        "sintaxis llaves\nextern \"C\" { fn externo(); }\n",
    ] {
        let parsed = parse(&SourceFile::new(source), LexOptions::default());
        let diagnostic = parsed
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == "OX3001")
            .expect("colisión en item Rust passthrough diagnosticada");
        assert!(diagnostic.message.contains("r#"), "{}", diagnostic.message);
    }
}

#[test]
fn parses_indented_struct_enum_and_use_items_as_valid_rust_fragments() {
    let source = SourceFile::new(
        "sintaxis indentacion\nestructura Punto: // campos\n    x: i32\n    y: i32\nenumeracion Estado:\n    Activo\n    Pausado(i32)\nusar std::fmt\n",
    );
    let parsed = parse(&source, LexOptions::default());
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
    let file = parsed.file.unwrap();
    assert_eq!(file.items.len(), 3);
    let Item::IndentedAggregate(structure) = &file.items[0] else {
        panic!("struct se representa en el AST estructural")
    };
    assert_eq!(
        structure.kind,
        oxido_language::domain::ast::AggregateKind::Struct
    );
    assert_eq!(structure.members.len(), 2);
    assert_eq!(structure.members[0].value.text, "x: i32");
    let Item::IndentedAggregate(enumeration) = &file.items[1] else {
        panic!("enum se representa en el AST estructural")
    };
    assert_eq!(
        enumeration.kind,
        oxido_language::domain::ast::AggregateKind::Enum
    );
    assert_eq!(enumeration.members[1].value.text, "Pausado(i32)");
    let Item::IndentedUse(import) = &file.items[2] else {
        panic!("use se representa como importación estructural")
    };
    assert_eq!(import.tree.text, "std::fmt");
}

#[test]
fn diagnoses_missing_function_names_and_malformed_declarations() {
    for source in [
        "sintaxis llaves\nfn () {}\n",
        "sintaxis llaves\nfn invalida() { let = 1; }\n",
        "sintaxis llaves\nfn invalida() { let x: = 1; }\n",
        "sintaxis llaves\nfn invalida() { if true {} else }\n",
        "sintaxis llaves\nfn invalida() { ; }\n",
    ] {
        let parsed = parse(&SourceFile::new(source), LexOptions::default());
        assert!(parsed.has_errors(), "{source}: {:?}", parsed.diagnostics);
        assert!(parsed.file.is_none(), "{source}");
    }
}

#[test]
fn parser_limits_recursive_block_depth_with_a_diagnostic() {
    let source = SourceFile::new(format!(
        "sintaxis llaves\nfn profunda() {{ {}0; {} }}\n",
        "if true { ".repeat(140),
        "}".repeat(140)
    ));
    let parsed = parse(&source, LexOptions::default());
    assert!(parsed.has_errors());
    assert!(
        parsed
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "OX6001")
    );
    assert!(parsed.file.is_none());
}

#[test]
fn parser_limits_recursive_else_if_depth_with_a_diagnostic() {
    let source = SourceFile::new(format!(
        "sintaxis llaves\nfn profunda() {{ {} }}\n",
        "if true {} else ".repeat(140) + "if true {}"
    ));
    let parsed = parse(&source, LexOptions::default());
    assert!(parsed.has_errors());
    assert!(
        parsed
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "OX6001")
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

#[test]
fn stops_parsing_when_the_deadline_expires_without_panicking() {
    let source = SourceFile::new("funcion main():\n    pass\n");
    let options = LexOptions {
        project_mode: Some(SyntaxMode::Indentation),
    };
    let expired = Instant::now().checked_sub(Duration::from_secs(1));
    let interrupted = parse_with_deadline(&source, options, expired);
    assert!(interrupted.file.is_none());
    assert_eq!(
        interrupted
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == "OX6001")
            .count(),
        1
    );
    assert!(
        interrupted
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("límite de tiempo"))
    );

    // Fuente vacía: el lexer no tiene pasos que comprobar, así que el aviso
    // procede del propio parser y confirma su comprobación independiente.
    let empty = SourceFile::new("");
    let parser_interrupted = parse_with_deadline(
        &empty,
        LexOptions {
            project_mode: Some(SyntaxMode::Braces),
        },
        expired,
    );
    assert!(parser_interrupted.file.is_none());
    assert!(
        parser_interrupted
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("límite de tiempo"))
    );

    let complete = parse_with_deadline(
        &source,
        options,
        Instant::now().checked_add(Duration::from_secs(60)),
    );
    assert!(!complete.has_errors(), "{:?}", complete.diagnostics);
    assert!(complete.file.is_some());
    assert_eq!(
        complete.file.unwrap().items.len(),
        parse(&source, options).file.unwrap().items.len()
    );
}

#[test]
fn caps_parser_diagnostics_and_reports_the_omitted_count() {
    let source = SourceFile::new(format!(
        "sintaxis llaves\nfn f() {{ {} }}",
        "let = 1;\n".repeat(2_000)
    ));
    let parsed = parse(
        &source,
        LexOptions {
            project_mode: Some(SyntaxMode::Braces),
        },
    );
    assert!(parsed.file.is_none());
    assert_eq!(parsed.diagnostics.len(), 1_001);
    let summary = parsed.diagnostics.last().expect("resumen final");
    assert_eq!(summary.code, "OX6001");
    assert!(summary.message.contains("omitieron"), "{}", summary.message);
}
