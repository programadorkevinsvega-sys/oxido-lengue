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
