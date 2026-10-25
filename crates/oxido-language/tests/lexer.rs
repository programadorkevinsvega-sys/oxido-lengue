use oxido_language::{
    SourceFile,
    domain::syntax::{LexOptions, SyntaxMode, TokenKind, lex},
};

fn texts<'a>(
    source: &'a SourceFile,
    tokens: &[oxido_language::domain::syntax::Token],
) -> Vec<&'a str> {
    tokens
        .iter()
        .map(|token| source.slice(token.span))
        .collect()
}

#[test]
fn tracks_utf8_spans_and_line_columns() {
    let source = SourceFile::new("sintaxis llaves\nfuncion café() {}\r\n");
    let output = lex(&source, LexOptions::default());
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let word = output
        .tokens
        .iter()
        .find(|token| source.slice(token.span) == "café")
        .unwrap();
    assert_eq!(source.slice(word.span), "café");
    assert_eq!(source.position(word.span.start()).unwrap().line, 2);
    assert_eq!(source.position(word.span.start()).unwrap().column, 9);
    assert!(
        source
            .span(word.span.start() + 4, word.span.end())
            .is_none()
    );
}

#[test]
fn repeated_lexing_of_identical_input_is_deterministic() {
    let source = SourceFile::new(
        "sintaxis indentacion\nfuncion sumar(a: i32, b: i32) -> i32:\n    dejar resultado = a + b\n    resultado\n",
    );
    let options = LexOptions::default();
    let first = lex(&source, options);
    let second = lex(&source, options);
    assert_eq!(first.mode, second.mode);
    assert_eq!(first.tokens, second.tokens);
    assert_eq!(first.diagnostics, second.diagnostics);
}

#[test]
fn indentation_mode_continues_after_operators_without_changing_block_depth() {
    let source = SourceFile::new(
        "sintaxis indentacion\nfuncion sumar() -> i32:\n    dejar resultado = 1 +\n                2\n    resultado\n",
    );
    let output = lex(&source, LexOptions::default());
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let tokens = texts(&source, &output.tokens);
    let plus = tokens.iter().position(|text| *text == "+").unwrap();
    assert_eq!(tokens.get(plus + 1), Some(&"2"));
    assert_eq!(
        output
            .tokens
            .iter()
            .filter(|token| matches!(token.kind, TokenKind::Indent))
            .count(),
        1
    );
}

#[test]
fn indentation_mode_continues_when_the_next_line_starts_with_member_access() {
    let source = SourceFile::new(
        "sintaxis indentacion\nfuncion leer() -> i32:\n    objeto\n        .campo\n",
    );
    let output = lex(&source, LexOptions::default());
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let tokens = texts(&source, &output.tokens);
    let objeto = tokens.iter().position(|text| *text == "objeto").unwrap();
    assert_eq!(tokens.get(objeto + 1), Some(&"."));
}

#[test]
fn recognizes_aliases_raw_identifiers_and_indentation() {
    let source = SourceFile::new(
        "// cabecera\nsintaxis indentacion\nfuncion calcular() -> i32:\n    dejar r#tipo: i32 = 7\n    r#tipo\n",
    );
    let output = lex(&source, LexOptions::default());
    assert_eq!(output.mode, Some(SyntaxMode::Indentation));
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    assert!(
        output
            .tokens
            .iter()
            .any(|token| matches!(token.kind, TokenKind::Indent))
    );
    assert!(
        output
            .tokens
            .iter()
            .any(|token| matches!(token.kind, TokenKind::Dedent))
    );
    assert!(output.tokens.iter().any(|token| {
        token.kind
            == (TokenKind::Keyword {
                rust_keyword: "fn",
                reserved: false,
            })
            && source.slice(token.span) == "funcion"
    }));
    assert!(output.tokens.iter().any(|token| {
        token.kind == (TokenKind::Identifier { raw: true }) && source.slice(token.span) == "r#tipo"
    }));
}

#[test]
fn preserves_literals_and_nested_comments_as_single_tokens() {
    let source = SourceFile::new(
        "sintaxis llaves\nlet texto = r#\"si tipo\"#; /* a /* b */ c */\nlet ch = 'ñ';\n",
    );
    let output = lex(&source, LexOptions::default());
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let token_text = texts(&source, &output.tokens);
    assert!(token_text.contains(&"r#\"si tipo\"#"));
    assert!(token_text.contains(&"/* a /* b */ c */"));
    assert!(token_text.contains(&"'ñ'"));
}

#[test]
fn reports_mode_conflicts_tabs_bidi_and_confusables() {
    let source = SourceFile::new(
        "sintaxis llaves\nfuncion f():\n\tdejar x = \"\u{202e}\"\n    let pаypal = 1\n",
    );
    let output = lex(
        &source,
        LexOptions {
            project_mode: Some(SyntaxMode::Indentation),
        },
    );
    let codes: Vec<_> = output.diagnostics.iter().map(|diag| diag.code).collect();
    assert!(codes.contains(&"OX0002"));
    assert!(codes.contains(&"OX2001"));
    assert!(codes.contains(&"OX1002"));
    assert!(codes.contains(&"OX1003"));
}

#[test]
fn malformed_input_and_missing_mode_return_diagnostics_without_panicking() {
    let source = SourceFile::new("funcion f(\n  /* sin cerrar");
    let output = lex(&source, LexOptions::default());
    let codes: Vec<_> = output.diagnostics.iter().map(|diag| diag.code).collect();
    assert!(codes.contains(&"OX0003"));
    assert!(codes.contains(&"OX1001"));
}

#[test]
fn rejects_inconsistent_dedentation() {
    let source = SourceFile::new("sintaxis indentacion\nfuncion f():\n    dejar x = 1\n  x\n");
    let output = lex(&source, LexOptions::default());
    assert!(output.diagnostics.iter().any(|diag| diag.code == "OX2001"));
}

#[test]
fn recognizes_character_literals_lifetimes_and_operators() {
    let source = SourceFile::new(
        "sintaxis llaves\nlet x = 'ñ'; let r = r#\"hola\"#; let a: &'static str = \"x\"; x..=2 && true\n",
    );
    let output = lex(&source, LexOptions::default());
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    assert!(
        output
            .tokens
            .iter()
            .any(|token| token.kind == TokenKind::CharacterLiteral)
    );
    assert!(
        output
            .tokens
            .iter()
            .any(|token| token.kind == TokenKind::Lifetime)
    );
    let token_text = texts(&source, &output.tokens);
    assert!(token_text.contains(&"..="), "{token_text:?}");
    assert!(token_text.contains(&"&&"));
}

#[test]
fn enforces_source_and_group_limits_with_diagnostics() {
    let too_large = SourceFile::new("x".repeat(4 * 1024 * 1024 + 1));
    let oversized = lex(
        &too_large,
        LexOptions {
            project_mode: Some(SyntaxMode::Braces),
        },
    );
    assert!(
        oversized
            .diagnostics
            .iter()
            .any(|diag| diag.code == "OX6001")
    );

    let nested = SourceFile::new(format!(
        "sintaxis llaves\n{}x{}",
        "(".repeat(300),
        ")".repeat(300)
    ));
    let output = lex(&nested, LexOptions::default());
    assert!(output.diagnostics.iter().any(|diag| diag.code == "OX6001"));

    let maximum_indentation = SourceFile::new(format!(
        "sintaxis indentacion\nx\n{}",
        (1..=128)
            .map(|level| format!("{}x\n", " ".repeat(level * 4)))
            .collect::<String>()
    ));
    let within_limit = lex(&maximum_indentation, LexOptions::default());
    assert!(
        !within_limit
            .diagnostics
            .iter()
            .any(|diag| diag.code == "OX6001")
    );

    let excessive_indentation = SourceFile::new(format!(
        "sintaxis indentacion\nx\n{}",
        (1..=129)
            .map(|level| format!("{}x\n", " ".repeat(level * 4)))
            .collect::<String>()
    ));
    let over_limit = lex(&excessive_indentation, LexOptions::default());
    assert!(
        over_limit
            .diagnostics
            .iter()
            .any(|diag| diag.code == "OX6001")
    );
}

#[test]
fn enforces_the_token_limit_without_rejecting_the_exact_boundary() {
    let at_limit = SourceFile::new("x ".repeat(1_000_000));
    let accepted = lex(
        &at_limit,
        LexOptions {
            project_mode: Some(SyntaxMode::Braces),
        },
    );
    assert_eq!(accepted.tokens.len(), 1_000_000);
    assert!(
        !accepted
            .diagnostics
            .iter()
            .any(|diag| diag.code == "OX6001")
    );

    let over_limit = SourceFile::new("x ".repeat(1_000_001));
    let rejected = lex(
        &over_limit,
        LexOptions {
            project_mode: Some(SyntaxMode::Braces),
        },
    );
    assert!(
        rejected
            .diagnostics
            .iter()
            .any(|diag| diag.code == "OX6001")
    );
}

#[test]
fn skips_nested_leading_comments_when_finding_the_mode_header() {
    let source =
        SourceFile::new("/* exterior\n   /* interior */\n*/\nsintaxis llaves\nfn f() {}\n");
    let output = lex(&source, LexOptions::default());
    assert_eq!(output.mode, Some(SyntaxMode::Braces));
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    assert!(
        output
            .tokens
            .iter()
            .any(|token| matches!(token.kind, TokenKind::ModeHeader(SyntaxMode::Braces)))
    );
}

#[test]
fn project_mode_works_without_a_file_header_and_braces_ignore_indent() {
    let source = SourceFile::new("fn f() {\n        let x = 1;\n    }\n");
    let output = lex(
        &source,
        LexOptions {
            project_mode: Some(SyntaxMode::Braces),
        },
    );
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    assert!(
        !output
            .tokens
            .iter()
            .any(|token| matches!(token.kind, TokenKind::Indent | TokenKind::Dedent))
    );
}

#[test]
fn handles_underscore_and_raw_identifier_restrictions() {
    let source = SourceFile::new("sintaxis llaves\nlet _ = r#_foo; let bad = r#_;\n");
    let output = lex(&source, LexOptions::default());
    assert!(
        output
            .tokens
            .iter()
            .any(|token| token.kind == TokenKind::Underscore)
    );
    assert!(output.tokens.iter().any(|token| {
        token.kind == (TokenKind::Identifier { raw: true }) && source.slice(token.span) == "r#_foo"
    }));
    assert!(output.diagnostics.iter().any(|diag| diag.code == "OX1001"));
}

#[test]
fn maps_positions_across_sparse_line_index_checkpoints() {
    let source = SourceFile::new(format!("{}fin", "a\n".repeat(300)));
    let offset = source.text().find("fin").unwrap();
    assert_eq!(source.position(offset).unwrap().line, 301);
    assert_eq!(source.position(offset).unwrap().column, 1);
}

#[test]
fn marks_macro_arguments_and_attribute_trees_opaque() {
    let source = SourceFile::new(
        "sintaxis llaves\n#[doc = \"tipo si\"]\nmacro_rules! ejemplo { () => { si }; }\nfn f() { let x = matches!(si, Some(_)); }\n",
    );
    let output = lex(&source, LexOptions::default());
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let alias_inside_macro = output
        .tokens
        .iter()
        .find(|token| source.slice(token.span) == "si")
        .unwrap();
    assert!(alias_inside_macro.opaque);
    let attribute_word = output
        .tokens
        .iter()
        .find(|token| source.slice(token.span) == "doc")
        .unwrap();
    assert!(attribute_word.opaque);
    let macro_body_keyword = output
        .tokens
        .iter()
        .find(|token| {
            source.slice(token.span) == "si"
                && token.span.start() < source.text().find("fn f").unwrap()
        })
        .unwrap();
    assert!(macro_body_keyword.opaque);
    let function_name = output
        .tokens
        .iter()
        .find(|token| source.slice(token.span) == "f")
        .unwrap();
    assert!(!function_name.opaque);
}
