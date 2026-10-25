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
        "sintaxis llaves\n#[repr(C)] pub struct Dato { valor: i32 }\nenum Color { Rojo, Azul }\nimpl Dato { fn nuevo() -> Self { Self { valor: 0 } } }\ntrait Leer { fn leer(&self) -> i32; }\nmod interno { pub fn valor() -> i32 { 1 } }\nuse std::fmt;\ntype Alias = i32;\nconst LIMITE: i32 = 10;\nstatic GLOBAL: i32 = 0;\nunion Crudo { valor: i32 }\nmacro_rules! ejemplo { () => {} }\nextern \"C\" { fn externo(); }\n",
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
