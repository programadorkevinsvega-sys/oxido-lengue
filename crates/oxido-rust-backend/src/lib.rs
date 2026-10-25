#![forbid(unsafe_code)]

//! Adaptador que valida y formatea archivos Rust mediante AST.

use oxido_language::{
    SourceFile,
    domain::{
        ast::{
            AggregateKind, Block, ContainerKind, ControlAlternative, ControlExpression,
            ControlKind, File, Function, IndentedAggregate, IndentedContainer, IndentedUse, Item,
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
            Item::IndentedAggregate(aggregate) => aggregate.span,
            Item::IndentedUse(item) => item.span,
            Item::IndentedContainer(container) => container.span,
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
        Item::IndentedAggregate(aggregate) => emit_indented_aggregate(aggregate, mappings),
        Item::IndentedUse(item) => emit_indented_use(item, mappings),
        Item::IndentedContainer(container) => emit_indented_container(container, mappings),
    }
}

fn emit_indented_container(
    container: &IndentedContainer,
    mappings: &mut Vec<PendingMapping>,
) -> Result<TokenStream, SynError> {
    let mut attributes = Vec::new();
    for attribute in &container.attributes {
        let parsed = syn::Attribute::parse_outer.parse2(parse_tokens(&attribute.text)?)?;
        attributes.extend(parsed);
        register_fragment(attribute, mappings)?;
    }
    let visibility = container
        .visibility
        .as_ref()
        .map(|fragment| {
            register_fragment(fragment, mappings)?;
            syn::parse2::<syn::Visibility>(parse_tokens(&fragment.text)?)
        })
        .transpose()?;
    let header = parse_tokens(&container.header.text)?;
    register_fragment(&container.header, mappings)?;
    let mut members = Vec::new();
    for member in &container.members {
        let tokens = match (container.kind, member) {
            (ContainerKind::Impl, Item::Function(function)) => {
                let tokens = emit_function(function, mappings)?;
                let item = syn::parse2::<syn::ImplItem>(tokens)?;
                quote! { #item }
            }
            (ContainerKind::Trait, Item::Function(function)) => {
                let tokens = emit_function(function, mappings)?;
                let item = syn::parse2::<syn::TraitItem>(tokens)?;
                quote! { #item }
            }
            (ContainerKind::Impl, Item::RustFragment(fragment)) => {
                register_fragment(fragment, mappings)?;
                let item = syn::parse2::<syn::ImplItem>(parse_tokens(&fragment.text)?)?;
                quote! { #item }
            }
            (ContainerKind::Trait, Item::RustFragment(fragment)) => {
                register_fragment(fragment, mappings)?;
                let item = syn::parse2::<syn::TraitItem>(parse_tokens(&fragment.text)?)?;
                quote! { #item }
            }
            (_, item) => emit_item(item, mappings)?,
        };
        members.push(tokens);
    }
    match container.kind {
        ContainerKind::Impl => Ok(quote! {
            #(#attributes)* #visibility impl #header { #(#members)* }
        }),
        ContainerKind::Trait => Ok(quote! {
            #(#attributes)* #visibility trait #header { #(#members)* }
        }),
        ContainerKind::Module => {
            let name = syn::parse2::<syn::Ident>(header)?;
            Ok(quote! { #(#attributes)* #visibility mod #name { #(#members)* } })
        }
    }
}

fn emit_indented_use(
    item: &IndentedUse,
    mappings: &mut Vec<PendingMapping>,
) -> Result<TokenStream, SynError> {
    let mut attributes = Vec::new();
    for attribute in &item.attributes {
        let parsed = syn::Attribute::parse_outer.parse2(parse_tokens(&attribute.text)?)?;
        attributes.push(
            parsed
                .into_iter()
                .next()
                .ok_or_else(|| SynError::new(RustSpan::call_site(), "atributo Rust vacío"))?,
        );
        register_fragment(attribute, mappings)?;
    }
    let visibility = item
        .visibility
        .as_ref()
        .map(|fragment| {
            register_fragment(fragment, mappings)?;
            syn::parse2::<syn::Visibility>(parse_tokens(&fragment.text)?)
        })
        .transpose()?;
    let tree = parse_tokens(&item.tree.text)?;
    register_fragment(&item.tree, mappings)?;
    Ok(quote! { #(#attributes)* #visibility use #tree; })
}

fn emit_indented_aggregate(
    aggregate: &IndentedAggregate,
    mappings: &mut Vec<PendingMapping>,
) -> Result<TokenStream, SynError> {
    let mut attributes = Vec::new();
    for attribute in &aggregate.attributes {
        let parsed = syn::Attribute::parse_outer.parse2(parse_tokens(&attribute.text)?)?;
        attributes.push(
            parsed
                .into_iter()
                .next()
                .ok_or_else(|| SynError::new(RustSpan::call_site(), "atributo Rust vacío"))?,
        );
        register_fragment(attribute, mappings)?;
    }
    let visibility = aggregate
        .visibility
        .as_ref()
        .map(|fragment| {
            register_fragment(fragment, mappings)?;
            syn::parse2::<syn::Visibility>(parse_tokens(&fragment.text)?)
        })
        .transpose()?;
    let header = parse_tokens(&aggregate.header.text)?;
    register_fragment(&aggregate.header, mappings)?;
    let mut members = Vec::new();
    for member in &aggregate.members {
        let mut member_attributes = Vec::new();
        for attribute in &member.attributes {
            let parsed = syn::Attribute::parse_outer.parse2(parse_tokens(&attribute.text)?)?;
            member_attributes.push(
                parsed
                    .into_iter()
                    .next()
                    .ok_or_else(|| SynError::new(RustSpan::call_site(), "atributo Rust vacío"))?,
            );
            register_fragment(attribute, mappings)?;
        }
        let value = parse_tokens(&member.value.text)?;
        register_fragment(&member.value, mappings)?;
        members.push(quote! { #(#member_attributes)* #value });
    }
    match aggregate.kind {
        AggregateKind::Struct => Ok(quote! {
            #(#attributes)* #visibility struct #header { #(#members,)* }
        }),
        AggregateKind::Enum => Ok(quote! {
            #(#attributes)* #visibility enum #header { #(#members,)* }
        }),
    }
}

fn emit_function(
    function: &Function,
    mappings: &mut Vec<PendingMapping>,
) -> Result<TokenStream, SynError> {
    let mut attributes = Vec::new();
    for attribute in &function.attributes {
        let mut parsed = syn::Attribute::parse_outer.parse2(parse_tokens(&attribute.text)?)?;
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
            let initializer = if let Some(control) = &statement.match_initializer {
                emit_control(control, mappings)?
            } else {
                let tokens = parse_tokens(&statement.initializer.text)?;
                register_fragment(&statement.initializer, mappings)?;
                tokens
            };
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

    if control.kind == ControlKind::Match {
        let mut arms = Vec::new();
        for arm in &control.match_arms {
            let pattern = syn::Pat::parse_single.parse2(parse_tokens(&arm.pattern.text)?)?;
            register_fragment(&arm.pattern, mappings)?;
            let body = if let Some(expression) = &arm.expression {
                let body = parse_tokens(&expression.text)?;
                register_fragment(expression, mappings)?;
                body
            } else if let Some(block) = &arm.body {
                emit_block(block, mappings)?
            } else {
                return Err(SynError::new(
                    RustSpan::call_site(),
                    "brazo de `coincidir` sin cuerpo",
                ));
            };
            arms.push(quote! { #pattern => #body });
        }
        let Some(header) = header else {
            return Err(SynError::new(
                RustSpan::call_site(),
                "falta la expresión de `coincidir`",
            ));
        };
        return Ok(quote! { match #header { #(#arms,)* } });
    }

    match control.kind {
        ControlKind::If => Ok(quote! { if #header #body #alternative }),
        ControlKind::Loop => Ok(quote! { loop #body }),
        ControlKind::While => Ok(quote! { while #header #body }),
        ControlKind::For => Ok(quote! { for #header #body }),
        ControlKind::Match => Err(SynError::new(
            RustSpan::call_site(),
            "no se pudo emitir la expresión `coincidir`",
        )),
    }
}

fn parse_tokens(fragment: &str) -> Result<TokenStream, SynError> {
    let tokens = TokenStream::from_str(fragment)
        .map_err(|error| SynError::new(RustSpan::call_site(), error.to_string()))?;
    Ok(normalize_aliases(tokens))
}

/// Normaliza nombres de Óxido. Solo recorre argumentos de macros estándar con
/// expresiones Rust; las DSL de macros desconocidas/externas permanecen opacas.
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
                if let TokenTree::Group(group) = &trees[index] {
                    normalized.extend([normalize_known_attribute(group)]);
                }
                index += 1;
            }
            continue;
        }
        if matches!(trees.get(index), Some(TokenTree::Ident(_)))
            && matches!(trees.get(index + 1), Some(TokenTree::Punct(punct)) if punct.as_char() == '!')
        {
            let spelling = match &trees[index] {
                TokenTree::Ident(ident) if !ident.to_string().starts_with("r#") => {
                    Some(ident.to_string())
                }
                _ => None,
            };
            let qualified = index >= 2
                && matches!(&trees[index - 1], TokenTree::Punct(punct) if punct.as_char() == ':')
                && matches!(&trees[index - 2], TokenTree::Punct(punct) if punct.as_char() == ':');
            let standard_path = !qualified
                || (index >= 3
                    && matches!(&trees[index - 3], TokenTree::Ident(parent) if parent == "std")
                    && (index == 3
                        || !matches!(&trees[index - 4], TokenTree::Punct(punct) if punct.as_char() == ':')));
            let canonical = spelling.as_deref().and_then(|name| {
                if !standard_path {
                    return None;
                }
                oxido_language::domain::syntax::macro_translation(name)
                    .or_else(|| is_standard_macro_name(name).then_some(name))
            });
            let macro_name = match (&trees[index], spelling.as_deref(), canonical) {
                (TokenTree::Ident(ident), Some(_), Some(canonical)) => {
                    TokenTree::Ident(proc_macro2::Ident::new(canonical, ident.span()))
                }
                _ => trees[index].clone(),
            };
            normalized.extend([macro_name, trees[index + 1].clone()]);
            index += 2;
            if let Some(TokenTree::Group(group)) = trees.get(index) {
                let arguments = if canonical.is_some_and(macro_arguments_are_expressions) {
                    let mut mapped = proc_macro2::Group::new(
                        group.delimiter(),
                        normalize_aliases(group.stream()),
                    );
                    mapped.set_span(group.span());
                    TokenTree::Group(mapped)
                } else {
                    TokenTree::Group(group.clone())
                };
                normalized.extend([arguments]);
                index += 1;
            }
            continue;
        }
        match &trees[index] {
            TokenTree::Ident(ident) if !ident.to_string().starts_with("r#") => {
                let spelling = ident.to_string();
                let previous = index.checked_sub(1).and_then(|at| trees.get(at));
                let is_method = matches!(previous, Some(TokenTree::Punct(punct)) if punct.as_char() == '.')
                    || matches!(previous, Some(TokenTree::Ident(keyword)) if keyword == "fn");
                let is_std_path = index >= 3
                    && matches!(&trees[index - 1], TokenTree::Punct(punct) if punct.as_char() == ':')
                    && matches!(&trees[index - 2], TokenTree::Punct(punct) if punct.as_char() == ':')
                    && matches!(&trees[index - 3], TokenTree::Ident(parent) if parent == "std");
                let translated = oxido_language::domain::syntax::keyword_translation(&spelling)
                    .filter(|translated| *translated != spelling)
                    .or_else(|| oxido_language::domain::syntax::prelude_translation(&spelling))
                    .or_else(|| {
                        is_method
                            .then(|| oxido_language::domain::syntax::method_translation(&spelling))
                            .flatten()
                    })
                    .or_else(|| {
                        is_std_path
                            .then(|| {
                                oxido_language::domain::syntax::std_module_translation(&spelling)
                            })
                            .flatten()
                    });
                if let Some(translated) = translated {
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

fn is_standard_macro_name(name: &str) -> bool {
    matches!(
        name,
        "println"
            | "print"
            | "eprintln"
            | "eprint"
            | "format"
            | "format_args"
            | "write"
            | "writeln"
            | "vec"
            | "panic"
            | "assert"
            | "assert_eq"
            | "assert_ne"
            | "debug_assert"
            | "debug_assert_eq"
            | "debug_assert_ne"
            | "dbg"
            | "todo"
            | "unimplemented"
            | "unreachable"
            | "matches"
            | "include_str"
            | "include_bytes"
            | "env"
            | "option_env"
            | "concat"
            | "stringify"
            | "line"
            | "file"
            | "column"
            | "cfg"
            | "compile_error"
            | "thread_local"
    )
}

/// Estos macros reciben expresiones o argumentos de formato con expresiones.
/// Las macros de metaprogramación y de compilación conservan el árbol opaco.
fn macro_arguments_are_expressions(name: &str) -> bool {
    matches!(
        name,
        "println"
            | "print"
            | "eprintln"
            | "eprint"
            | "format"
            | "format_args"
            | "write"
            | "writeln"
            | "vec"
            | "panic"
            | "assert"
            | "assert_eq"
            | "assert_ne"
            | "debug_assert"
            | "debug_assert_eq"
            | "debug_assert_ne"
            | "dbg"
            | "todo"
            | "unimplemented"
            | "unreachable"
            | "matches"
    )
}

fn normalize_known_attribute(group: &proc_macro2::Group) -> TokenTree {
    let trees = group.stream().into_iter().collect::<Vec<_>>();
    let Some(TokenTree::Ident(name)) = trees.first() else {
        return TokenTree::Group(group.clone());
    };
    let replacement = match name.to_string().as_str() {
        "prueba" => Some("test"),
        "permitir" => Some("allow"),
        "derivar" => Some("derive"),
        _ => None,
    };
    let Some(replacement) = replacement else {
        return TokenTree::Group(group.clone());
    };
    let mut content = TokenStream::new();
    content.extend([TokenTree::Ident(proc_macro2::Ident::new(
        replacement,
        name.span(),
    ))]);
    for tree in trees.into_iter().skip(1) {
        match tree {
            TokenTree::Group(inner)
                if replacement == "derive" && inner.delimiter() == Delimiter::Parenthesis =>
            {
                let derives = inner.stream().into_iter().map(|token| match token {
                    TokenTree::Ident(ident) if !ident.to_string().starts_with("r#") => {
                        let spelling = ident.to_string();
                        oxido_language::domain::syntax::prelude_translation(&spelling)
                            .map(|translated| {
                                TokenTree::Ident(proc_macro2::Ident::new(translated, ident.span()))
                            })
                            .unwrap_or(TokenTree::Ident(ident))
                    }
                    token => token,
                });
                let mut mapped = proc_macro2::Group::new(Delimiter::Parenthesis, derives.collect());
                mapped.set_span(inner.span());
                content.extend([TokenTree::Group(mapped)]);
            }
            token => content.extend([token]),
        }
    }
    let mut mapped = proc_macro2::Group::new(Delimiter::Bracket, content);
    mapped.set_span(group.span());
    TokenTree::Group(mapped)
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

/// Valida un brazo indentado de `match` como sintaxis Rust.
pub fn validate_match_arm(fragment: &str) -> Result<(), SynError> {
    syn::parse2::<syn::Arm>(parse_tokens(fragment)?).map(|_| ())
}

/// Valida la firma de una función preservada por el AST.
pub fn validate_signature(fragment: &str) -> Result<(), SynError> {
    let tokens = parse_tokens(fragment)?;
    syn::parse2::<syn::Signature>(quote! { fn #tokens }).map(|_| ())
}

/// Valida un atributo Rust preservado como fragmento opaco.
pub fn validate_attribute(fragment: &str) -> Result<(), SynError> {
    use syn::parse::Parser;
    syn::Attribute::parse_outer
        .parse2(parse_tokens(fragment)?)
        .map(|_| ())
}

/// Valida una visibilidad Rust, incluida `pub(...)`.
pub fn validate_visibility(fragment: &str) -> Result<(), SynError> {
    syn::parse2::<syn::Visibility>(parse_tokens(fragment)?).map(|_| ())
}

/// Valida un item Rust passthrough completo.
pub fn validate_item(fragment: &str) -> Result<(), SynError> {
    syn::parse2::<syn::File>(parse_tokens(fragment)?).map(|_| ())
}

/// Valida una declaración estructural indentada en su contexto final de item.
pub fn validate_indented_aggregate(aggregate: &IndentedAggregate) -> Result<(), SynError> {
    let tokens = emit_indented_aggregate(aggregate, &mut Vec::new())?;
    syn::parse2::<syn::Item>(tokens).map(|_| ())
}

/// Valida una importación estructural indentada.
pub fn validate_indented_use(item: &IndentedUse) -> Result<(), SynError> {
    let tokens = emit_indented_use(item, &mut Vec::new())?;
    syn::parse2::<syn::Item>(tokens).map(|_| ())
}

/// Valida un contenedor indentado en su contexto final Rust.
pub fn validate_indented_container(container: &IndentedContainer) -> Result<(), SynError> {
    let tokens = emit_indented_container(container, &mut Vec::new())?;
    syn::parse2::<syn::Item>(tokens).map(|_| ())
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
    fn normalizes_standard_macro_arguments_but_keeps_raw_identifiers_opaque() {
        let source = SourceFile::new(
            "sintaxis indentacion\n#[permitir(tipo)]\nfuncion principal():\n    variable activo = verdadero\n    afirmar_igual!(verdadero, r#tipo);\n    activo\n",
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
            generated.code.contains("assert_eq!(true, r#tipo);"),
            "{}",
            generated.code
        );
        assert!(
            generated.code.contains("#[allow(tipo)]"),
            "{}",
            generated.code
        );
    }

    #[test]
    fn normalizes_prelude_aliases_methods_and_known_macros() {
        let source = SourceFile::new(
            "sintaxis indentacion\nfuncion principal():\n    variable mutable nombres: Vector<Cadena> = vector![String::from(\"Ada\")]\n    nombres.agregar(formato!(\"{}\", nombres.len()))\n    imprimirln!(\"{}\", nombres[0].to_string())\n",
        );
        let parsed = parse(&source, LexOptions::default());
        assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
        let generated = generate(&parsed.file.unwrap(), &source).unwrap();
        for expected in [
            "fn main()",
            "let mut nombres: Vec<String> = vec![String::from(\"Ada\")]",
            "nombres.push(format!(\"{}\", nombres.len()))",
            "println!(\"{}\", nombres[0].to_string())",
        ] {
            assert!(
                generated.code.contains(expected),
                "missing {expected}: {}",
                generated.code
            );
        }
    }

    #[test]
    fn known_attributes_translate_and_external_macro_arguments_stay_opaque() {
        assert!(
            parse_tokens("#[prueba]")
                .unwrap()
                .to_string()
                .contains("test")
        );
        assert!(
            parse_tokens("#[permitir(tipo)]")
                .unwrap()
                .to_string()
                .contains("allow (tipo)")
        );
        let unknown = parse_tokens("#[serde(rename = \"tipo\")]")
            .unwrap()
            .to_string();
        assert!(unknown.contains("serde") && unknown.contains("tipo"));
        assert_eq!(
            parse_tokens("afirmar_igual!(verdadero)")
                .unwrap()
                .to_string(),
            "assert_eq ! (true)"
        );
        let std_path = parse_tokens("std::colecciones::HashMap<String, Vector<texto>>")
            .unwrap()
            .to_string();
        assert!(
            std_path.contains("std :: collections :: HashMap"),
            "{std_path}"
        );
        assert!(std_path.contains("Vec < str >"), "{std_path}");
    }

    #[test]
    fn every_published_macro_and_std_module_alias_is_registered() {
        let macros = [
            ("imprimirln", "println"),
            ("imprimir_error", "eprintln"),
            ("imprimir_error_sin_salto", "eprint"),
            ("imprimir", "print"),
            ("imprimir_sin_salto", "print"),
            ("formato", "format"),
            ("formatear", "format"),
            ("argumentos_formato", "format_args"),
            ("escribir", "write"),
            ("escribir_linea", "writeln"),
            ("vector", "vec"),
            ("panico", "panic"),
            ("entrar_en_panico", "panic"),
            ("afirmar", "assert"),
            ("afirmar_igual", "assert_eq"),
            ("afirmar_distinto", "assert_ne"),
            ("afirmar_debug", "debug_assert"),
            ("afirmar_igual_debug", "debug_assert_eq"),
            ("afirmar_distinto_debug", "debug_assert_ne"),
            ("pendiente", "todo"),
            ("no_implementado", "unimplemented"),
            ("sin_implementar", "unimplemented"),
            ("inalcanzable", "unreachable"),
            ("depurar", "dbg"),
            ("coincide", "matches"),
            ("incluir_texto", "include_str"),
            ("incluir_bytes", "include_bytes"),
            ("entorno", "env"),
            ("entorno_opcional", "option_env"),
            ("concatenar", "concat"),
            ("a_texto", "stringify"),
            ("linea", "line"),
            ("archivo", "file"),
            ("columna", "column"),
            ("config", "cfg"),
            ("error_de_compilacion", "compile_error"),
            ("local_del_hilo", "thread_local"),
        ];
        for (alias, rust) in macros {
            let normalized = parse_tokens(&format!("{alias}!()")).unwrap().to_string();
            assert!(normalized.starts_with(rust), "{alias}: {normalized}");
            assert_eq!(
                parse_tokens(&format!("{rust}!()")).unwrap().to_string(),
                format!("{rust} ! ()"),
                "el nombre Rust original {rust} debe seguir funcionando"
            );
        }
        assert_eq!(
            parse_tokens("matches!(true, true)").unwrap().to_string(),
            "matches ! (true , true)"
        );
        assert_eq!(
            parse_tokens("afirmar!(verdadero == vector![1].len() == 1)")
                .unwrap()
                .to_string(),
            "assert ! (true == vec ! [1] . len () == 1)"
        );
        assert_eq!(
            parse_tokens("serde_json::json!(tipo)").unwrap().to_string(),
            "serde_json :: json ! (tipo)"
        );
        assert_eq!(
            parse_tokens("concatenar!(tipo)").unwrap().to_string(),
            "concat ! (tipo)"
        );

        for (alias, rust) in [
            ("colecciones", "collections"),
            ("archivos", "fs"),
            ("es", "io"),
            ("hilo", "thread"),
        ] {
            let normalized = parse_tokens(&format!("std::{alias}::x"))
                .unwrap()
                .to_string();
            assert!(normalized.contains(rust), "{alias}: {normalized}");
        }
    }
}
