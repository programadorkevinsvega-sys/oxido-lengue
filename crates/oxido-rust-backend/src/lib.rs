#![forbid(unsafe_code)]

//! Adaptador que valida y formatea archivos Rust mediante AST.

use oxido_language::{
    SourceFile,
    domain::{
        ast::{
            Block, ControlAlternative, ControlExpression, ControlKind, File, Function, Item,
            RustFragment, Statement,
        },
        source::Span,
    },
};
use proc_macro2::{Delimiter, LineColumn, Span as RustSpan, TokenStream, TokenTree};
use quote::quote;
use serde::Serialize;
use std::str::FromStr;
use syn::parse::{Error as SynError, Parser};
use syn::spanned::Spanned;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpanMapping {
    pub source_start: usize,
    pub source_end: usize,
    pub source_start_line: usize,
    pub source_start_column: usize,
    pub source_end_line: usize,
    pub source_end_column: usize,
    pub generated_start_line: usize,
    pub generated_start_column: usize,
    pub generated_end_line: usize,
    pub generated_end_column: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedRust {
    pub code: String,
    pub mappings: Vec<SpanMapping>,
}

struct PendingMapping {
    source: Span,
    tokens: Vec<String>,
}

/// Emite Rust desde el AST Óxido, conserva correspondencias de tokens y formatea el resultado.
pub fn generate(file: &File, source: &SourceFile) -> Result<GeneratedRust, SynError> {
    let mut pending = Vec::new();
    let items = file
        .items
        .iter()
        .map(|item| emit_item(item, &mut pending))
        .collect::<Result<Vec<_>, _>>()?;
    let tokens = quote! { #(#items)* };
    let parsed = syn::parse2::<syn::File>(tokens)?;
    let code = prettyplease::unparse(&parsed);
    let reparsed = syn::parse_file(&code)?;
    let generated_tokens = TokenStream::from_str(&code)
        .map_err(|error| SynError::new(RustSpan::call_site(), error.to_string()))?;
    let mut target = Vec::new();
    flatten_tokens(generated_tokens, &mut target);

    let mut mappings = Vec::new();
    let mut cursor = 0;
    for entry in pending {
        if entry.tokens.is_empty() {
            continue;
        }
        let Some(start) = find_token_sequence(&target, &entry.tokens, cursor) else {
            continue;
        };
        let end = start + entry.tokens.len() - 1;
        if let Some(mapping) = make_mapping(
            source,
            entry.source,
            target[start].1.start(),
            target[end].1.end(),
        ) {
            mappings.push(mapping);
        }
        cursor = end + 1;
    }

    // Un token de error dentro de un item siempre puede retroceder al span del item fuente.
    for (source_item, generated_item) in file.items.iter().zip(&reparsed.items) {
        let source_span = match source_item {
            Item::Function(function) => function.span,
            Item::RustFragment(fragment) => fragment.span,
        };
        let generated_span = generated_item.span();
        if let Some(mapping) = make_mapping(
            source,
            source_span,
            generated_span.start(),
            generated_span.end(),
        ) {
            mappings.push(mapping);
        }
    }
    mappings.sort_by_key(|mapping| {
        (
            mapping.generated_start_line,
            mapping.generated_start_column,
            mapping.source_end - mapping.source_start,
        )
    });
    Ok(GeneratedRust { code, mappings })
}

/// Importa a sintaxis de Óxido el subconjunto reversible de funciones libres.
pub fn import_reversible(source: &SourceFile) -> Result<String, SynError> {
    let file = syn::parse_file(source.text())?;
    let mut output = String::from("sintaxis llaves\n");
    for item in &file.items {
        let syn::Item::Fn(function) = item else {
            return Err(SynError::new(
                item.span(),
                "la importación reversible inicial solo admite funciones libres",
            ));
        };
        if function.sig.constness.is_some()
            || function.sig.unsafety.is_some()
            || function.sig.abi.is_some()
            || function.sig.variadic.is_some()
        {
            return Err(SynError::new(
                function.sig.span(),
                "esta función usa un modificador fuera del subconjunto reversible",
            ));
        }
        let attributes = function
            .attrs
            .iter()
            .map(|attribute| quote! { #attribute }.to_string())
            .map(|attribute| format!("{attribute}\n"))
            .collect::<String>();
        let visibility = match &function.vis {
            syn::Visibility::Inherited => String::new(),
            visibility => format!("{} ", quote! { #visibility }),
        };
        let mut signature = function.sig.clone();
        let asynchronous = signature.asyncness.take().is_some();
        let signature = quote! { #signature }.to_string();
        let signature = signature
            .strip_prefix("fn ")
            .ok_or_else(|| SynError::new(function.sig.span(), "no se pudo reconstruir la firma"))?;
        let asynchronous = if asynchronous { "async " } else { "" };
        let block = &function.block;
        let body = quote! { #block };
        output.push_str(&attributes);
        output.push_str(&visibility);
        output.push_str(asynchronous);
        output.push_str("funcion ");
        output.push_str(signature);
        output.push(' ');
        output.push_str(&body.to_string());
        output.push('\n');
    }
    Ok(output)
}

fn emit_item(item: &Item, mappings: &mut Vec<PendingMapping>) -> Result<TokenStream, SynError> {
    match item {
        Item::Function(function) => emit_function(function, mappings),
        Item::RustFragment(fragment) => {
            let parsed = syn::parse2::<syn::File>(parse_tokens(&fragment.text)?)?;
            register_fragment(fragment, mappings)?;
            let items = parsed.items;
            Ok(quote! { #(#items)* })
        }
    }
}

fn emit_function(
    function: &Function,
    mappings: &mut Vec<PendingMapping>,
) -> Result<TokenStream, SynError> {
    let mut attributes = Vec::new();
    for attribute in &function.attributes {
        let mut parsed = syn::Attribute::parse_outer.parse_str(&attribute.text)?;
        attributes.push(
            parsed
                .drain(..)
                .next()
                .ok_or_else(|| SynError::new(RustSpan::call_site(), "atributo Rust vacío"))?,
        );
        register_fragment(attribute, mappings)?;
    }
    let visibility = function
        .visibility
        .as_ref()
        .map(|fragment| {
            register_fragment(fragment, mappings)?;
            syn::parse2::<syn::Visibility>(parse_tokens(&fragment.text)?)
        })
        .transpose()?;

    let signature_tokens = parse_tokens(&function.signature.text)?;
    let signature = syn::parse2::<syn::ItemFn>(quote! { fn #signature_tokens {} })?.sig;
    register_fragment(&function.signature, mappings)?;
    if signature.ident != function.name.as_str() {
        return Err(SynError::new(
            RustSpan::call_site(),
            "el nombre del AST no coincide con la firma de función",
        ));
    }

    let asynchronous = function.asynchronous.then(|| quote! { async });
    let body = emit_block(&function.body, mappings)?;
    Ok(quote! {
        #(#attributes)*
        #visibility
        #asynchronous
        #signature
        #body
    })
}

fn emit_block(block: &Block, mappings: &mut Vec<PendingMapping>) -> Result<TokenStream, SynError> {
    let tail_control_index = block
        .statements
        .last()
        .filter(|statement| matches!(statement, Statement::Control(_)))
        .map(|_| block.statements.len() - 1);
    let mut statements = Vec::new();
    let mut tail_control = None;
    for (index, statement) in block.statements.iter().enumerate() {
        if Some(index) == tail_control_index {
            match statement {
                Statement::Control(control) => {
                    tail_control = Some(emit_control(control, mappings)?)
                }
                _ => return Err(SynError::new(RustSpan::call_site(), "AST inconsistente")),
            }
        } else {
            statements.push(emit_statement(statement, mappings)?);
        }
    }
    let tail = block
        .tail
        .as_ref()
        .map(|expression| {
            let tokens = parse_tokens(&expression.value.text)?;
            register_fragment(&expression.value, mappings)?;
            Ok::<_, SynError>(tokens)
        })
        .transpose()?;
    Ok(quote! { { #(#statements)* #tail_control #tail } })
}

fn emit_statement(
    statement: &Statement,
    mappings: &mut Vec<PendingMapping>,
) -> Result<TokenStream, SynError> {
    match statement {
        Statement::Let(statement) => {
            let pattern = parse_tokens(&statement.pattern.text)?;
            register_fragment(&statement.pattern, mappings)?;
            let annotation = statement
                .type_annotation
                .as_ref()
                .map(|fragment| {
                    let tokens = parse_tokens(&fragment.text)?;
                    register_fragment(fragment, mappings)?;
                    Ok::<_, SynError>(quote! { : #tokens })
                })
                .transpose()?;
            let initializer = parse_tokens(&statement.initializer.text)?;
            register_fragment(&statement.initializer, mappings)?;
            Ok(quote! { let #pattern #annotation = #initializer; })
        }
        Statement::Return(statement) => {
            let value = statement
                .value
                .as_ref()
                .map(|fragment| {
                    let tokens = parse_tokens(&fragment.text)?;
                    register_fragment(fragment, mappings)?;
                    Ok::<_, SynError>(tokens)
                })
                .transpose()?;
            Ok(quote! { return #value; })
        }
        Statement::Break(statement) => {
            let value = statement
                .value
                .as_ref()
                .map(|fragment| {
                    let tokens = parse_tokens(&fragment.text)?;
                    register_fragment(fragment, mappings)?;
                    Ok::<_, SynError>(tokens)
                })
                .transpose()?;
            Ok(quote! { break #value; })
        }
        Statement::Continue(statement) => {
            let value = statement
                .value
                .as_ref()
                .map(|fragment| {
                    let tokens = parse_tokens(&fragment.text)?;
                    register_fragment(fragment, mappings)?;
                    Ok::<_, SynError>(tokens)
                })
                .transpose()?;
            Ok(quote! { continue #value; })
        }
        Statement::Expression(expression) => {
            let value = parse_tokens(&expression.value.text)?;
            register_fragment(&expression.value, mappings)?;
            Ok(quote! { #value; })
        }
        Statement::Control(control) => {
            let expression = emit_control(control, mappings)?;
            Ok(quote! { #expression; })
        }
        Statement::RustItem(fragment) => {
            let parsed = syn::parse2::<syn::File>(parse_tokens(&fragment.text)?)?;
            register_fragment(fragment, mappings)?;
            let items = parsed.items;
            Ok(quote! { #(#items)* })
        }
    }
}

fn emit_control(
    control: &ControlExpression,
    mappings: &mut Vec<PendingMapping>,
) -> Result<TokenStream, SynError> {
    let header = if control.kind == ControlKind::Loop {
        None
    } else {
        let tokens = parse_tokens(&control.header.text)?;
        register_fragment(&control.header, mappings)?;
        Some(tokens)
    };
    let body = emit_block(&control.body, mappings)?;
    let alternative = control
        .alternative
        .as_ref()
        .map(|alternative| match alternative {
            ControlAlternative::Block(block) => {
                emit_block(block, mappings).map(|block| quote! { else #block })
            }
            ControlAlternative::ElseIf(nested) => {
                emit_control(nested, mappings).map(|nested| quote! { else #nested })
            }
        })
        .transpose()?;

    match control.kind {
        ControlKind::If => Ok(quote! { if #header #body #alternative }),
        ControlKind::Loop => Ok(quote! { loop #body }),
        ControlKind::While => Ok(quote! { while #header #body }),
        ControlKind::For => Ok(quote! { for #header #body }),
    }
}

fn parse_tokens(fragment: &str) -> Result<TokenStream, SynError> {
    let tokens = TokenStream::from_str(fragment)
        .map_err(|error| SynError::new(RustSpan::call_site(), error.to_string()))?;
    Ok(normalize_aliases(tokens))
}

/// Traduce aliases fuera de macros y atributos. Sus grupos son Rust opaco y
/// se conserva incluso cuando contiene palabras que Óxido también reconoce.
fn normalize_aliases(tokens: TokenStream) -> TokenStream {
    let trees = tokens.into_iter().collect::<Vec<_>>();
    let mut normalized = TokenStream::new();
    let mut index = 0;
    while index < trees.len() {
        if matches!(trees.get(index), Some(TokenTree::Punct(punct)) if punct.as_char() == '#') {
            normalized.extend([trees[index].clone()]);
            index += 1;
            if matches!(trees.get(index), Some(TokenTree::Punct(punct)) if punct.as_char() == '!') {
                normalized.extend([trees[index].clone()]);
                index += 1;
            }
            if matches!(trees.get(index), Some(TokenTree::Group(group)) if group.delimiter() == Delimiter::Bracket)
            {
                normalized.extend([trees[index].clone()]);
                index += 1;
            }
            continue;
        }
        if matches!(trees.get(index), Some(TokenTree::Ident(_)))
            && matches!(trees.get(index + 1), Some(TokenTree::Punct(punct)) if punct.as_char() == '!')
        {
            normalized.extend([trees[index].clone(), trees[index + 1].clone()]);
            index += 2;
            if matches!(trees.get(index), Some(TokenTree::Group(_))) {
                normalized.extend([trees[index].clone()]);
                index += 1;
            }
            continue;
        }
        match &trees[index] {
            TokenTree::Ident(ident) if !ident.to_string().starts_with("r#") => {
                let spelling = ident.to_string();
                if let Some(translated) =
                    oxido_language::domain::syntax::keyword_translation(&spelling)
                        .filter(|translated| *translated != spelling)
                {
                    normalized.extend([TokenTree::Ident(proc_macro2::Ident::new(
                        translated,
                        ident.span(),
                    ))]);
                } else {
                    normalized.extend([trees[index].clone()]);
                }
            }
            TokenTree::Group(group) => {
                let mut replacement =
                    proc_macro2::Group::new(group.delimiter(), normalize_aliases(group.stream()));
                replacement.set_span(group.span());
                normalized.extend([TokenTree::Group(replacement)]);
            }
            _ => normalized.extend([trees[index].clone()]),
        }
        index += 1;
    }
    normalized
}

fn register_fragment(
    fragment: &RustFragment,
    mappings: &mut Vec<PendingMapping>,
) -> Result<(), SynError> {
    let mut tokens = Vec::new();
    flatten_tokens(parse_tokens(&fragment.text)?, &mut tokens);
    mappings.push(PendingMapping {
        source: fragment.span,
        tokens: tokens.into_iter().map(|(token, _)| token).collect(),
    });
    Ok(())
}

fn flatten_tokens(stream: TokenStream, output: &mut Vec<(String, RustSpan)>) {
    for token in stream {
        match token {
            TokenTree::Group(group) => {
                let delimiter = match group.delimiter() {
                    Delimiter::Parenthesis => "paren",
                    Delimiter::Brace => "brace",
                    Delimiter::Bracket => "bracket",
                    Delimiter::None => "none",
                };
                output.push((format!("open:{delimiter}"), group.span()));
                flatten_tokens(group.stream(), output);
                output.push((format!("close:{delimiter}"), group.span()));
            }
            token => output.push((token.to_string(), token.span())),
        }
    }
}

fn find_token_sequence(
    target: &[(String, RustSpan)],
    needle: &[String],
    cursor: usize,
) -> Option<usize> {
    if needle.is_empty() || needle.len() > target.len() || cursor > target.len() - needle.len() {
        return None;
    }
    (cursor..=target.len() - needle.len()).find(|start| {
        target[*start..*start + needle.len()]
            .iter()
            .map(|(token, _)| token)
            .eq(needle.iter())
    })
}

fn make_mapping(
    source: &SourceFile,
    source_span: Span,
    generated_start: LineColumn,
    generated_end: LineColumn,
) -> Option<SpanMapping> {
    let source_start = source.position(source_span.start())?;
    let source_end = source.position(source_span.end())?;
    Some(SpanMapping {
        source_start: source_span.start(),
        source_end: source_span.end(),
        source_start_line: source_start.line,
        source_start_column: source_start.column,
        source_end_line: source_end.line,
        source_end_column: source_end.column,
        generated_start_line: generated_start.line,
        generated_start_column: generated_start.column,
        generated_end_line: generated_end.line,
        generated_end_column: generated_end.column,
    })
}

/// Rust parseado; la representación concreta de `syn` queda confinada al adapter.
pub struct RustFile(syn::File);

/// Analiza una fuente completa como archivo Rust.
pub fn parse(source: &SourceFile) -> Result<RustFile, SynError> {
    syn::parse_file(source.text()).map(RustFile)
}

/// Formatea un AST Rust sin concatenar o interpolar texto de entrada.
#[must_use]
pub fn format(file: &RustFile) -> String {
    prettyplease::unparse(&file.0)
}

impl RustFile {
    /// Cantidad de items top-level presentes en el archivo.
    #[must_use]
    pub fn item_count(&self) -> usize {
        self.0.items.len()
    }
}

/// Valida un fragmento de expresión Rust sin analizar macros expandidas ni semántica.
pub fn validate_expression(fragment: &str) -> Result<(), SynError> {
    syn::parse2::<syn::Expr>(parse_tokens(fragment)?).map(|_| ())
}

/// Valida una cabecera `for` Rust conservándola como tokens.
pub fn validate_for_header(fragment: &str) -> Result<(), SynError> {
    let header = parse_tokens(fragment)?;
    let expression = quote::quote! { for #header {} };
    syn::parse2::<syn::Expr>(expression).map(|_| ())
}

/// Valida argumentos opcionales de `break` o `continue` como una expresión Rust.
pub fn validate_jump(kind: &str, fragment: Option<&str>) -> Result<(), SynError> {
    let argument = parse_tokens(fragment.unwrap_or_default())?;
    let statement = match kind {
        "break" => quote::quote! { break #argument; },
        "continue" => quote::quote! { continue #argument; },
        _ => {
            return Err(SynError::new(
                proc_macro2::Span::call_site(),
                "se esperaba `break` o `continue`",
            ));
        }
    };
    syn::parse2::<syn::Stmt>(statement).map(|_| ())
}

/// Valida un fragmento de tipo Rust.
pub fn validate_type(fragment: &str) -> Result<(), SynError> {
    syn::parse2::<syn::Type>(parse_tokens(fragment)?).map(|_| ())
}

/// Valida un fragmento de patrón Rust.
pub fn validate_pattern(fragment: &str) -> Result<(), SynError> {
    use syn::parse::Parser;
    syn::Pat::parse_single
        .parse2(parse_tokens(fragment)?)
        .map(|_| ())
}

/// Valida la firma de una función preservada por el AST.
pub fn validate_signature(fragment: &str) -> Result<(), SynError> {
    let tokens = parse_tokens(fragment)?;
    syn::parse2::<syn::Signature>(quote! { fn #tokens }).map(|_| ())
}

/// Valida un atributo Rust preservado como fragmento opaco.
pub fn validate_attribute(fragment: &str) -> Result<(), SynError> {
    use syn::parse::Parser;
    syn::Attribute::parse_outer.parse_str(fragment).map(|_| ())
}

/// Valida una visibilidad Rust, incluida `pub(...)`.
pub fn validate_visibility(fragment: &str) -> Result<(), SynError> {
    syn::parse2::<syn::Visibility>(parse_tokens(fragment)?).map(|_| ())
}

/// Valida un item Rust passthrough completo.
pub fn validate_item(fragment: &str) -> Result<(), SynError> {
    syn::parse2::<syn::File>(parse_tokens(fragment)?).map(|_| ())
}

#[cfg(test)]
mod generation_tests {
    use super::*;
    use oxido_language::domain::syntax::{LexOptions, parse};

    #[test]
    fn emits_a_formatted_rust_function_and_maps_source_fragments() {
        let source = SourceFile::new(
            "sintaxis indentacion\nfuncion main():\n    println!(\"Hola, mundo.\");\n",
        );
        let parsed = parse(&source, LexOptions::default());
        assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
        let generated = generate(&parsed.file.unwrap(), &source).unwrap();
        assert!(generated.code.contains("fn main()"), "{}", generated.code);
        assert!(generated.code.contains("println!"), "{}", generated.code);
        syn::parse_file(&generated.code).unwrap();
        assert!(
            generated.mappings.iter().any(|mapping| {
                mapping.source_start_line == 3 && mapping.generated_start_line >= 2
            }),
            "{:?}",
            generated.mappings
        );
    }

    #[test]
    fn normalizes_aliases_in_rust_fragments_but_keeps_macros_and_raw_identifiers_opaque() {
        let source = SourceFile::new(
            "sintaxis indentacion\n#[allow(tipo)]\nfuncion main():\n    dejar activo = verdadero\n    assert_eq!(verdadero, r#tipo);\n    activo\n",
        );
        let parsed = parse(&source, LexOptions::default());
        assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
        let generated = generate(&parsed.file.unwrap(), &source).unwrap();
        assert!(
            generated.code.contains("let activo = true;"),
            "{}",
            generated.code
        );
        assert!(
            generated.code.contains("assert_eq!(verdadero, r#tipo);"),
            "{}",
            generated.code
        );
        assert!(
            generated.code.contains("#[allow(tipo)]"),
            "{}",
            generated.code
        );
    }
}
