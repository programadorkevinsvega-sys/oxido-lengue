use oxido_language::SourceFile;

#[test]
fn parses_formats_and_reparses_a_rust_file() {
    let source = SourceFile::new("fn suma(a:i32,b:i32)->i32{a+b}");
    let analysis = oxido_compiler::analyze_rust(&source).expect("pipeline Rust válido");
    assert_eq!(analysis.item_count, 1);
}

#[test]
fn generates_a_rust_program_with_a_serializable_source_map() {
    use oxido_language::domain::syntax::LexOptions;

    let source =
        SourceFile::new("sintaxis indentacion\nfuncion main():\n    println!(\"Hola, mundo.\");\n");
    let generated = oxido_compiler::generate_ox(&source, LexOptions::default())
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
    assert!(generated.rust.contains("fn main()"));
    assert!(generated.rust.contains("println!(\"Hola, mundo.\")"));
    oxido_compiler::analyze_rust(&SourceFile::new(&generated.rust)).unwrap();

    let mapped = generated
        .source_map
        .map_generated(2, 5)
        .expect("el call site de la macro debe volver al .ox");
    let original = source.span(mapped.start, mapped.end).unwrap();
    assert!(source.slice(original).contains("println!"));
    let serialized = serde_json::to_string(&generated.source_map).unwrap();
    assert!(serialized.contains("generatedStartLine"));
}

#[test]
fn preserves_a_final_control_expression_as_the_rust_block_value() {
    use oxido_language::domain::syntax::LexOptions;

    let source = SourceFile::new(
        "sintaxis llaves\nfn elegir(condicion: bool) -> i32 { if condicion { 1 } else { 2 } }\n",
    );
    let generated = oxido_compiler::generate_ox(&source, LexOptions::default())
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
    assert!(generated.rust.contains("else"));
    assert!(!generated.rust.contains("};"));
    oxido_compiler::analyze_rust(&SourceFile::new(&generated.rust)).unwrap();
}

#[test]
fn compiles_and_runs_a_minimal_ox_program_through_cargo() {
    use oxido_compiler::application::cargo::CargoAction;
    use oxido_language::domain::syntax::LexOptions;

    let source =
        SourceFile::new("sintaxis indentacion\nfuncion main():\n    println!(\"Hola, mundo.\");\n");
    let checked = oxido_compiler::build_ox(&source, LexOptions::default(), CargoAction::Check)
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
    assert!(
        checked.success,
        "{} {:?}",
        checked.stderr, checked.diagnostics
    );

    let run = oxido_compiler::build_ox(&source, LexOptions::default(), CargoAction::Run)
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
    assert!(run.success, "{} {:?}", run.stderr, run.diagnostics);
    assert_eq!(run.program_output, ["Hola, mundo."]);
}

#[test]
fn bilingual_block_modes_compile_to_the_same_behavior() {
    use oxido_compiler::application::cargo::CargoAction;
    use oxido_language::domain::syntax::LexOptions;

    let sources = [
        "sintaxis indentacion\nfuncion main():\n    si verdadero:\n        println!(\"equivalente\");\n    sino:\n        println!(\"distinto\");\n",
        "sintaxis llaves\nfuncion main() { si verdadero { println!(\"equivalente\"); } sino { println!(\"distinto\"); } }\n",
    ];
    for text in sources {
        let source = SourceFile::new(text);
        let generated = oxido_compiler::generate_ox(&source, LexOptions::default())
            .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
        assert!(generated.rust.contains("if true"), "{}", generated.rust);
        assert!(
            generated.rust.contains("println!(\"equivalente\")"),
            "{}",
            generated.rust
        );
        let result = oxido_compiler::build_ox(&source, LexOptions::default(), CargoAction::Run)
            .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
        assert!(result.success, "{} {:?}", result.stderr, result.diagnostics);
        assert_eq!(result.program_output, ["equivalente"]);
    }
}

#[test]
fn normalizes_aliases_in_visibility_signatures_and_passthrough_items() {
    use oxido_language::domain::syntax::LexOptions;

    let source = SourceFile::new(
        "sintaxis llaves\npublico usar std::fmt;\npublico funcion inspeccionar(valor: &dinamico std::fmt::Debug) { let _ = valor; }\n",
    );
    let generated = oxido_compiler::generate_ox(&source, LexOptions::default())
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
    assert!(
        generated.rust.contains("pub use std::fmt;"),
        "{}",
        generated.rust
    );
    assert!(
        generated
            .rust
            .contains("pub fn inspeccionar(valor: &dyn std::fmt::Debug)"),
        "{}",
        generated.rust
    );
}

#[test]
fn maps_rustc_type_errors_back_to_the_ox_expression() {
    use oxido_compiler::application::cargo::CargoAction;
    use oxido_language::domain::syntax::LexOptions;

    let source =
        SourceFile::new("sintaxis indentacion\nfuncion main():\n    dejar valor: bool = 1\n");
    let result = oxido_compiler::build_ox(&source, LexOptions::default(), CargoAction::Check)
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
    assert!(!result.success);
    let diagnostic = result
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code.as_deref() == Some("E0308"))
        .expect("se conserva el diagnostic JSON del compilador Rust");
    let mapped = diagnostic.source.expect("el span debe volver al .ox");
    let span = source.span(mapped.start, mapped.end).unwrap();
    assert!(source.slice(span).contains('1'));
}

#[test]
fn maps_rustc_control_header_errors_back_to_the_ox_header() {
    use oxido_compiler::application::cargo::CargoAction;
    use oxido_language::domain::syntax::LexOptions;

    let source = SourceFile::new("sintaxis llaves\nfn main() { if 1 { println!(\"nunca\"); } }\n");
    let result = oxido_compiler::build_ox(&source, LexOptions::default(), CargoAction::Check)
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
    assert!(!result.success);
    let diagnostic = result
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code.as_deref() == Some("E0308"))
        .expect("se conserva el error de condición Rust");
    let mapped = diagnostic
        .source
        .expect("el span de condición debe mapearse");
    let span = source.span(mapped.start, mapped.end).unwrap();
    assert_eq!(source.slice(span), "1");
}

#[test]
fn rejects_non_reversible_rust_items_during_import() {
    let source = SourceFile::new("struct Dato { valor: i32 }\n");
    let error = oxido_compiler::import_rust(&source).unwrap_err();
    assert!(error.contains("solo admite funciones libres"), "{error}");
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

    let continued =
        SourceFile::new("sintaxis indentacion\nfuncion rota():\n    dejar x = 1 +\n    x\n");
    let parsed = oxido_compiler::parse_ox(&continued, LexOptions::default());
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);

    let malformed = SourceFile::new("sintaxis indentacion\nfuncion rota():\n    dejar x = +\n");
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

#[test]
fn validates_passthrough_items_and_all_initial_control_statements_with_syn() {
    use oxido_language::domain::syntax::LexOptions;

    let source = SourceFile::new(
        "sintaxis llaves\n#[derive(Clone)]\npub struct Dato { valor: i32 }\npub(crate) use std::fmt;\ntest_support::assertion!();\n#[inline]\npub async fn cargar() {}\nfn recorrer(xs: &[i32]) { for x in xs { while *x > 0 { if *x == 1 { break; } else { continue; } } } loop { break; } }\n",
    );
    let parsed = oxido_compiler::parse_ox(&source, LexOptions::default());
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
    assert_eq!(parsed.file.unwrap().items.len(), 5);
}

#[test]
fn validates_each_supported_top_level_item_kind_in_braces_mode() {
    use oxido_language::domain::syntax::LexOptions;

    let source = SourceFile::new(
        "sintaxis llaves\n#[repr(C)] pub struct Dato { valor: i32 }\nenum Color { Rojo, Azul }\nimpl Dato { fn nuevo() -> Self { Self { valor: 0 } } }\ntrait Leer { fn leer(&self) -> i32; }\nmod interno { pub fn valor() -> i32 { 1 } }\nuse std::fmt;\ntype Alias = i32;\nconst LIMITE: i32 = 10;\nstatic GLOBAL: i32 = 0;\nunion Crudo { valor: i32 }\nmacro_rules! ejemplo { () => {} }\nextern \"C\" { fn llamada_externa(); }\n",
    );
    let parsed = oxido_compiler::parse_ox(&source, LexOptions::default());
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
    assert_eq!(parsed.file.unwrap().items.len(), 12);
}

#[test]
fn reports_malformed_function_attributes() {
    use oxido_language::domain::syntax::LexOptions;

    let source = SourceFile::new("sintaxis llaves\n#[doc = ]\nfn invalida() {}\n");
    let parsed = oxido_compiler::parse_ox(&source, LexOptions::default());
    assert!(parsed.has_errors());
    assert!(
        parsed
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "OX4001")
    );
}

#[test]
fn validates_the_same_control_subset_in_both_block_modes() {
    use oxido_language::domain::syntax::LexOptions;

    let sources = [
        "sintaxis indentacion\nfn recorrer(xs: &[i32]) -> i32:\n    for x in xs:\n        while *x > 0:\n            if *x == 1:\n                break\n            else:\n                continue\n    loop:\n        return 0\n",
        "sintaxis llaves\nfn recorrer(xs: &[i32]) -> i32 { for x in xs { while *x > 0 { if *x == 1 { break; } else { continue; } } } loop { return 0; } }\n",
    ];
    for source in sources {
        let parsed = oxido_compiler::parse_ox(&SourceFile::new(source), LexOptions::default());
        assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
        assert_eq!(parsed.file.unwrap().items.len(), 1);
    }
}

#[test]
fn reports_invalid_for_and_jump_fragments() {
    use oxido_language::domain::syntax::LexOptions;

    let source = SourceFile::new("sintaxis llaves\nfn invalida() { for x xs { break 1 2; } }\n");
    let parsed = oxido_compiler::parse_ox(&source, LexOptions::default());
    assert!(parsed.has_errors());
    assert!(parsed.file.is_none());
    assert_eq!(
        parsed
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == "OX4001")
            .count(),
        2
    );
}

#[test]
fn validates_function_signatures_patterns_types_and_expressions() {
    use oxido_language::domain::syntax::LexOptions;

    for source in [
        "sintaxis llaves\nfn rota(x:) {}\n",
        "sintaxis llaves\nfn rota() { let x @ = 1; }\n",
        "sintaxis llaves\nfn rota() { let x: Vec< = Vec::new(); }\n",
        "sintaxis llaves\nfn rota() { let x = 1 +; }\n",
    ] {
        let parsed = oxido_compiler::parse_ox(&SourceFile::new(source), LexOptions::default());
        assert!(parsed.has_errors(), "{source}: {:?}", parsed.diagnostics);
        assert!(parsed.file.is_none(), "{source}");
    }
}
