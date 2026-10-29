//! Índice local de declaraciones `.ox`.
//!
//! No es un analizador de tipos: solo justifica nombres, contenedores y rangos
//! léxicos/estructurales que el editor puede usar como fallback local. La
//! semántica Rust sigue delegada en `rust-analyzer` y `rustc`.

use oxido_language::{
    SourceFile,
    domain::{
        ast::{AggregateKind, ContainerKind, File as OxFile, Item, RustFragment},
        syntax::{LexOptions, SyntaxMode, TokenKind},
    },
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum SymbolKind {
    Function,
    Method,
    Struct,
    Enum,
    Trait,
    TypeAlias,
    Module,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DeclaredSymbol {
    pub name: String,
    pub kind: SymbolKind,
    /// Tipo o rasgo que contiene a un método.
    pub container: Option<String>,
    /// Módulos lógicos desde la raíz del archivo hasta el símbolo.
    pub module_path: Vec<String>,
    pub name_start: usize,
    pub name_end: usize,
    pub range_start: usize,
    pub range_end: usize,
}

pub(crate) fn declared_symbols(text: &str, mode: Option<SyntaxMode>) -> Vec<DeclaredSymbol> {
    let parsed =
        oxido_compiler::parse_ox(&SourceFile::new(text), LexOptions { project_mode: mode });
    match parsed.file {
        Some(file) => ast_symbols(&file, text),
        None if mode != Some(SyntaxMode::Indentation) => rust_symbols(text),
        None => Vec::new(),
    }
}

fn ast_symbols(file: &OxFile, text: &str) -> Vec<DeclaredSymbol> {
    fn visit(
        items: &[Item],
        text: &str,
        module_path: &[String],
        method_container: Option<&str>,
        output: &mut Vec<DeclaredSymbol>,
    ) {
        for item in items {
            match item {
                Item::Function(function) => {
                    let kind = if method_container.is_some() {
                        SymbolKind::Method
                    } else {
                        SymbolKind::Function
                    };
                    push_named(
                        output,
                        text,
                        SymbolSpec {
                            name: &function.name,
                            kind,
                            container: method_container,
                            module_path,
                        },
                        function.span.start(),
                        function.span.end(),
                    );
                }
                Item::IndentedAggregate(aggregate) => {
                    let kind = match aggregate.kind {
                        AggregateKind::Struct => SymbolKind::Struct,
                        AggregateKind::Enum => SymbolKind::Enum,
                    };
                    push_from_header(
                        output,
                        text,
                        &aggregate.header,
                        SymbolSpec {
                            name: "",
                            kind,
                            container: None,
                            module_path,
                        },
                        aggregate.span.start(),
                        aggregate.span.end(),
                    );
                }
                Item::IndentedContainer(container) => match container.kind {
                    ContainerKind::Module => {
                        push_from_header(
                            output,
                            text,
                            &container.header,
                            SymbolSpec {
                                name: "",
                                kind: SymbolKind::Module,
                                container: None,
                                module_path,
                            },
                            container.span.start(),
                            container.span.end(),
                        );
                        let name = header_identifier(&container.header.text).map(|(name, _)| name);
                        let mut nested = module_path.to_vec();
                        if let Some(name) = name {
                            nested.push(name);
                        }
                        visit(&container.members, text, &nested, None, output);
                    }
                    ContainerKind::Impl => {
                        let type_name = impl_type_name(&container.header.text);
                        visit(
                            &container.members,
                            text,
                            module_path,
                            type_name.as_deref(),
                            output,
                        );
                    }
                    ContainerKind::Trait => {
                        let trait_name =
                            header_identifier(&container.header.text).map(|(name, _)| name);
                        push_from_header(
                            output,
                            text,
                            &container.header,
                            SymbolSpec {
                                name: "",
                                kind: SymbolKind::Trait,
                                container: None,
                                module_path,
                            },
                            container.span.start(),
                            container.span.end(),
                        );
                        visit(
                            &container.members,
                            text,
                            module_path,
                            trait_name.as_deref(),
                            output,
                        );
                    }
                },
                Item::RustFragment(fragment) => {
                    output.extend(rust_fragment_symbols(text, fragment));
                }
                Item::IndentedUse(_) => {}
            }
        }
    }

    let mut output = Vec::new();
    visit(&file.items, text, &[], None, &mut output);
    output
}

fn rust_fragment_symbols(text: &str, fragment: &RustFragment) -> Vec<DeclaredSymbol> {
    let Some(source) = text.get(fragment.span.start()..fragment.span.end()) else {
        return Vec::new();
    };
    rust_symbols_from(source, fragment.span.start(), text)
}

fn rust_symbols(text: &str) -> Vec<DeclaredSymbol> {
    rust_symbols_from(text, 0, text)
}

fn rust_symbols_from(source: &str, base: usize, full_text: &str) -> Vec<DeclaredSymbol> {
    let lexed = crate::lex_analyzed(
        &SourceFile::new(source),
        LexOptions {
            project_mode: Some(SyntaxMode::Braces),
        },
    );
    let tokens = lexed.tokens;
    let mut output = Vec::new();
    let mut depth = 0_usize;
    let mut containers = Vec::<(&str, usize, ContainerKind)>::new();
    let mut pending_container: Option<(&str, ContainerKind)> = None;
    let mut index = 0_usize;
    while index < tokens.len() {
        let token = &tokens[index];
        let word = source
            .get(token.span.start()..token.span.end())
            .unwrap_or_default();
        match word {
            "{" => {
                depth = depth.saturating_add(1);
                if let Some((name, kind)) = pending_container.take() {
                    containers.push((name, depth, kind));
                }
            }
            "}" => {
                depth = depth.saturating_sub(1);
                while containers
                    .last()
                    .is_some_and(|(_, level, _)| *level > depth)
                {
                    containers.pop();
                }
            }
            ";" => {
                pending_container = None;
            }
            "fn" | "funcion" => {
                if let Some((name, start, end)) = next_identifier(source, &tokens, index + 1) {
                    let kind = match containers.last() {
                        Some((_, level, ContainerKind::Impl | ContainerKind::Trait))
                            if *level == depth =>
                        {
                            SymbolKind::Method
                        }
                        _ => SymbolKind::Function,
                    };
                    let container = match containers.last() {
                        Some((name, level, ContainerKind::Impl | ContainerKind::Trait))
                            if *level == depth =>
                        {
                            Some(*name)
                        }
                        _ => None,
                    };
                    let module_path = module_path_from(&containers);
                    push_absolute(
                        &mut output,
                        full_text,
                        SymbolSpec {
                            name,
                            kind,
                            container,
                            module_path: &module_path,
                        },
                        base + start,
                        base + end,
                    );
                }
            }
            "struct" | "estructura" => {
                if let Some((name, start, end)) = next_identifier(source, &tokens, index + 1) {
                    let module_path = module_path_from(&containers);
                    push_absolute(
                        &mut output,
                        full_text,
                        SymbolSpec {
                            name,
                            kind: SymbolKind::Struct,
                            container: None,
                            module_path: &module_path,
                        },
                        base + start,
                        base + end,
                    );
                }
            }
            "enum" | "enumeracion" => {
                if let Some((name, start, end)) = next_identifier(source, &tokens, index + 1) {
                    let module_path = module_path_from(&containers);
                    push_absolute(
                        &mut output,
                        full_text,
                        SymbolSpec {
                            name,
                            kind: SymbolKind::Enum,
                            container: None,
                            module_path: &module_path,
                        },
                        base + start,
                        base + end,
                    );
                }
            }
            "trait" | "rasgo" => {
                if let Some((name, start, end)) = next_identifier(source, &tokens, index + 1) {
                    let module_path = module_path_from(&containers);
                    push_absolute(
                        &mut output,
                        full_text,
                        SymbolSpec {
                            name,
                            kind: SymbolKind::Trait,
                            container: None,
                            module_path: &module_path,
                        },
                        base + start,
                        base + end,
                    );
                    pending_container = Some((name, ContainerKind::Trait));
                }
            }
            "type" | "tipo" => {
                if let Some((name, start, end)) = next_identifier(source, &tokens, index + 1) {
                    let module_path = module_path_from(&containers);
                    push_absolute(
                        &mut output,
                        full_text,
                        SymbolSpec {
                            name,
                            kind: SymbolKind::TypeAlias,
                            container: None,
                            module_path: &module_path,
                        },
                        base + start,
                        base + end,
                    );
                }
            }
            "mod" | "modulo" => {
                if let Some((name, start, end)) = next_identifier(source, &tokens, index + 1) {
                    let module_path = module_path_from(&containers);
                    push_absolute(
                        &mut output,
                        full_text,
                        SymbolSpec {
                            name,
                            kind: SymbolKind::Module,
                            container: None,
                            module_path: &module_path,
                        },
                        base + start,
                        base + end,
                    );
                    pending_container = Some((name, ContainerKind::Module));
                }
            }
            "impl" | "implementar" => {
                if let Some(name) = impl_header_name(source, &tokens, index + 1) {
                    pending_container = Some((name, ContainerKind::Impl));
                }
            }
            _ => {}
        }
        index += 1;
    }
    output
}

fn module_path_from(containers: &[(&str, usize, ContainerKind)]) -> Vec<String> {
    containers
        .iter()
        .filter_map(|(name, _, kind)| {
            (*kind == ContainerKind::Module).then_some((*name).to_owned())
        })
        .collect()
}

fn next_identifier<'a>(
    source: &'a str,
    tokens: &[oxido_language::domain::syntax::Token],
    start: usize,
) -> Option<(&'a str, usize, usize)> {
    for token in tokens.get(start..)? {
        if matches!(
            token.kind,
            TokenKind::Identifier { .. } | TokenKind::Keyword { .. }
        ) {
            let text = source.get(token.span.start()..token.span.end())?;
            let (name, offset) = text.strip_prefix("r#").map_or((text, 0), |raw| (raw, 2));
            return Some((name, token.span.start() + offset, token.span.end()));
        }
        if matches!(
            token.kind,
            TokenKind::Newline | TokenKind::Indent | TokenKind::Dedent | TokenKind::Comment
        ) {
            continue;
        }
        return None;
    }
    None
}

fn impl_header_name<'a>(
    source: &'a str,
    tokens: &[oxido_language::domain::syntax::Token],
    start: usize,
) -> Option<&'a str> {
    let mut words = Vec::new();
    let mut angle_depth = 0_usize;
    let mut started = false;
    for token in tokens.get(start..)? {
        let word = source
            .get(token.span.start()..token.span.end())
            .unwrap_or_default();
        match word {
            "{" | ";" => break,
            "<" => angle_depth = angle_depth.saturating_add(1),
            ">" => angle_depth = angle_depth.saturating_sub(1),
            "for" if angle_depth == 0 => {
                started = true;
                words.push(None);
            }
            _ if angle_depth == 0
                && started
                && matches!(
                    token.kind,
                    TokenKind::Identifier { .. } | TokenKind::Keyword { .. }
                ) =>
            {
                words.push(Some(word));
            }
            _ if angle_depth == 0
                && !started
                && matches!(
                    token.kind,
                    TokenKind::Identifier { .. } | TokenKind::Keyword { .. }
                ) =>
            {
                started = true;
                words.push(Some(word));
            }
            _ => {}
        }
    }
    let for_index = words.iter().position(Option::is_none);
    match for_index {
        Some(index) => words.get(index + 1).copied().flatten(),
        None => words.first().copied().flatten(),
    }
}

struct SymbolSpec<'a> {
    name: &'a str,
    kind: SymbolKind,
    container: Option<&'a str>,
    module_path: &'a [String],
}

fn push_from_header(
    output: &mut Vec<DeclaredSymbol>,
    text: &str,
    header: &RustFragment,
    spec: SymbolSpec<'_>,
    range_start: usize,
    range_end: usize,
) {
    let Some((name, relative)) = header_identifier(&header.text) else {
        return;
    };
    let name_start = header.span.start() + relative;
    push_absolute(
        output,
        text,
        SymbolSpec {
            name: &name,
            ..spec
        },
        name_start,
        name_start + name.len(),
    );
    if let Some(last) = output.last_mut() {
        last.range_start = range_start;
        last.range_end = range_end;
    }
}

fn push_named(
    output: &mut Vec<DeclaredSymbol>,
    text: &str,
    spec: SymbolSpec<'_>,
    range_start: usize,
    range_end: usize,
) {
    let Some(source) = text.get(range_start..range_end) else {
        return;
    };
    let Some(relative) = find_word(source, spec.name) else {
        return;
    };
    let name_start = range_start + relative;
    let name_end = name_start + spec.name.len();
    push_absolute(output, text, spec, name_start, name_end);
    if let Some(last) = output.last_mut() {
        last.range_start = range_start;
        last.range_end = range_end;
    }
}

fn push_absolute(
    output: &mut Vec<DeclaredSymbol>,
    text: &str,
    spec: SymbolSpec<'_>,
    name_start: usize,
    name_end: usize,
) {
    let (Some(range_start), Some(range_end)) =
        (line_start(text, name_start), line_end(text, name_end))
    else {
        return;
    };
    output.push(DeclaredSymbol {
        name: spec.name.to_owned(),
        kind: spec.kind,
        container: spec.container.map(str::to_owned),
        module_path: spec.module_path.to_vec(),
        name_start,
        name_end,
        range_start,
        range_end,
    });
}

fn header_identifier(header: &str) -> Option<(String, usize)> {
    let mut angle_depth = 0_usize;
    let mut index = 0_usize;
    while let Some(character) = header[index..].chars().next() {
        let width = character.len_utf8();
        match character {
            '<' => angle_depth = angle_depth.saturating_add(1),
            '>' => angle_depth = angle_depth.saturating_sub(1),
            _ if angle_depth == 0 && (character == '_' || character.is_alphanumeric()) => {
                let start = index;
                let mut end = index + width;
                while let Some(next) = header[end..].chars().next()
                    && (next == '_' || next.is_alphanumeric())
                {
                    end += next.len_utf8();
                }
                let name = header.get(start..end)?.to_owned();
                return Some((name, start));
            }
            _ => {}
        }
        index += width;
    }
    None
}

fn impl_type_name(header: &str) -> Option<String> {
    let mut words = Vec::new();
    let mut angle_depth = 0_usize;
    let mut index = 0_usize;
    while let Some(character) = header[index..].chars().next() {
        let width = character.len_utf8();
        match character {
            '<' => angle_depth = angle_depth.saturating_add(1),
            '>' => angle_depth = angle_depth.saturating_sub(1),
            _ if angle_depth == 0 && (character == '_' || character.is_alphanumeric()) => {
                let start = index;
                let mut end = index + width;
                while let Some(next) = header[end..].chars().next()
                    && (next == '_' || next.is_alphanumeric())
                {
                    end += next.len_utf8();
                }
                words.push(header.get(start..end)?.to_owned());
                index = end;
                continue;
            }
            _ => {}
        }
        index += width;
    }
    let for_index = words.iter().position(|word| word == "for");
    match for_index {
        Some(index) => words.get(index + 1).cloned(),
        None => words.first().cloned(),
    }
}

fn find_word(haystack: &str, needle: &str) -> Option<usize> {
    haystack.match_indices(needle).find_map(|(index, _)| {
        let before = haystack[..index].chars().next_back();
        let after = haystack[index + needle.len()..].chars().next();
        let boundary = |character: Option<char>| {
            character.is_none_or(|character| !(character == '_' || character.is_alphanumeric()))
        };
        (boundary(before) && boundary(after)).then_some(index)
    })
}

fn line_start(text: &str, offset: usize) -> Option<usize> {
    let offset = offset.min(text.len());
    (text.is_char_boundary(offset)).then(|| {
        text[..offset]
            .rfind('\n')
            .map_or(0, |index| index.saturating_add(1))
    })
}

fn line_end(text: &str, offset: usize) -> Option<usize> {
    let offset = offset.min(text.len());
    (text.is_char_boundary(offset)).then(|| {
        text[offset..]
            .find('\n')
            .map_or(text.len(), |index| offset + index)
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LocalBindingKind {
    Variable,
    Parameter,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LocalBinding {
    pub name: String,
    pub kind: LocalBindingKind,
    pub name_start: usize,
    pub name_end: usize,
    pub scope_start: usize,
    pub scope_end: usize,
}

impl LocalBinding {
    #[must_use]
    pub fn contains(&self, offset: usize) -> bool {
        self.scope_start <= offset && offset < self.scope_end
    }
}

/// Ámbitos léxicos acotados del archivo `.ox`.
///
/// No infiere tipos ni evalúa expresiones: registra parámetros y enlaces
/// `dejar`/`let`/`para` para que el fallback local no confunda una variable con
/// una declaración del workspace. La resolución de tipos sigue delegada en
/// `rust-analyzer`.
pub(crate) fn local_bindings(text: &str, mode: Option<SyntaxMode>) -> Vec<LocalBinding> {
    let lexed = crate::lex_analyzed(&SourceFile::new(text), LexOptions { project_mode: mode });
    let tokens = lexed.tokens;
    let mut blocks = vec![(0_usize, text.len())];
    let mut braces = Vec::new();
    let mut indents = Vec::new();
    for token in &tokens {
        match &token.kind {
            TokenKind::Punctuation => match text.get(token.span.start()..token.span.end()) {
                Some("{") => braces.push(token.span.end()),
                Some("}") => {
                    if let Some(start) = braces.pop() {
                        blocks.push((start, token.span.start()));
                    }
                }
                _ => {}
            },
            TokenKind::Indent => indents.push(token.span.end()),
            TokenKind::Dedent => {
                if let Some(start) = indents.pop() {
                    blocks.push((start, token.span.start()));
                }
            }
            _ => {}
        }
    }
    let significant = tokens
        .iter()
        .enumerate()
        .filter(|(_, token)| {
            !matches!(
                token.kind,
                TokenKind::Comment
                    | TokenKind::Newline
                    | TokenKind::Indent
                    | TokenKind::Dedent
                    | TokenKind::ModeHeader(_)
            )
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    let mut bindings = Vec::new();
    for (position, index) in significant.iter().copied().enumerate() {
        let token = &tokens[index];
        let word = text
            .get(token.span.start()..token.span.end())
            .unwrap_or_default();
        match word {
            "funcion" | "fn" => {
                if let Some(binding) =
                    function_bindings(text, &tokens, &significant, position, &blocks)
                {
                    bindings.extend(binding);
                }
            }
            "dejar" | "let" | "para" | "for" => {
                if let Some(binding) =
                    declared_binding(text, &tokens, &significant, position, &blocks, word)
                {
                    bindings.push(binding);
                }
            }
            _ => {}
        }
    }
    bindings.sort_by_key(|binding| (binding.scope_start, binding.name_start));
    bindings
}

#[must_use]
pub(crate) fn local_binding_at<'a>(
    bindings: &'a [LocalBinding],
    name: &str,
    offset: usize,
) -> Option<&'a LocalBinding> {
    bindings
        .iter()
        .filter(|binding| binding.name == name && binding.contains(offset))
        .max_by_key(|binding| binding.scope_start)
}

fn declared_binding(
    text: &str,
    tokens: &[oxido_language::domain::syntax::Token],
    significant: &[usize],
    position: usize,
    blocks: &[(usize, usize)],
    keyword: &str,
) -> Option<LocalBinding> {
    let mut next = position.checked_add(1)?;
    while let Some(index) = significant.get(next).copied() {
        let word = word_at(text, &tokens[index]);
        if matches!(word, "mutable" | "mut") {
            next = next.checked_add(1)?;
            continue;
        }
        break;
    }
    let index = *significant.get(next)?;
    let token = &tokens[index];
    if !matches!(token.kind, TokenKind::Identifier { .. }) {
        return None;
    }
    let name = word_at(text, token).to_owned();
    if name == "_" {
        return None;
    }
    if keyword == "dejar" || keyword == "let" {
        let following = significant
            .get(next + 1)
            .and_then(|index| tokens.get(*index))
            .and_then(|token| text.get(token.span.start()..token.span.end()));
        if following.is_some_and(|following| !matches!(following, ":" | "=" | "," | ";" | ")")) {
            return None;
        }
    }
    let (scope_start, scope_end) = innermost_block(blocks, token.span.start())?;
    Some(LocalBinding {
        name,
        kind: LocalBindingKind::Variable,
        name_start: token.span.start(),
        name_end: token.span.end(),
        scope_start,
        scope_end,
    })
}

fn function_bindings(
    text: &str,
    tokens: &[oxido_language::domain::syntax::Token],
    significant: &[usize],
    position: usize,
    blocks: &[(usize, usize)],
) -> Option<Vec<LocalBinding>> {
    let parameters_start = significant
        .iter()
        .skip(position + 1)
        .copied()
        .find(|index| word_at(text, &tokens[*index]) == "(")?;
    let parameters_end = matching_paren(text, tokens, significant, parameters_start)?;
    let body_marker = significant
        .iter()
        .skip_while(|index| **index <= parameters_end)
        .copied()
        .find(|index| matches!(word_at(text, &tokens[*index]), "{" | ":"))?;
    let body_start = significant
        .iter()
        .copied()
        .find(|index| *index > body_marker)
        .and_then(|index| tokens.get(index))
        .map(|token| token.span.start())?;
    let (_, scope_end) = innermost_block(blocks, body_start)?;
    let bindings = parameter_tokens(text, tokens, significant, parameters_start, parameters_end)
        .into_iter()
        .map(|(index, kind)| {
            let token = &tokens[index];
            let start = token.span.start();
            LocalBinding {
                name: word_at(text, token).to_owned(),
                kind,
                name_start: start,
                name_end: token.span.end(),
                scope_start: start,
                scope_end,
            }
        })
        .collect::<Vec<_>>();
    Some(bindings)
}

fn parameter_tokens(
    text: &str,
    tokens: &[oxido_language::domain::syntax::Token],
    significant: &[usize],
    parameters_start: usize,
    parameters_end: usize,
) -> Vec<(usize, LocalBindingKind)> {
    let mut output = Vec::new();
    let mut depth = 0_usize;
    for index in significant
        .iter()
        .copied()
        .skip_while(|index| *index <= parameters_start)
        .take_while(|index| *index < parameters_end)
    {
        let word = word_at(text, &tokens[index]);
        match word {
            "(" | "[" => depth = depth.saturating_add(1),
            ")" | "]" => depth = depth.saturating_sub(1),
            _ if depth == 0 && matches!(tokens[index].kind, TokenKind::Identifier { .. }) => {
                let previous = significant
                    .iter()
                    .copied()
                    .take_while(|candidate| *candidate < index)
                    .last()
                    .map(|candidate| word_at(text, &tokens[candidate]));
                let next = significant
                    .iter()
                    .copied()
                    .find(|candidate| *candidate > index)
                    .map(|candidate| word_at(text, &tokens[candidate]));
                let declares = matches!(next, Some(":" | "," | ")"))
                    && !matches!(previous, Some(":" | "::" | "."));
                if declares {
                    output.push((index, LocalBindingKind::Parameter));
                }
            }
            _ => {}
        }
    }
    output
}

fn matching_paren(
    text: &str,
    tokens: &[oxido_language::domain::syntax::Token],
    significant: &[usize],
    open: usize,
) -> Option<usize> {
    let mut depth = 0_usize;
    for index in significant
        .iter()
        .copied()
        .skip_while(|index| *index < open)
    {
        match word_at(text, &tokens[index]) {
            "(" => depth = depth.checked_add(1)?,
            ")" => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

fn innermost_block(blocks: &[(usize, usize)], offset: usize) -> Option<(usize, usize)> {
    blocks
        .iter()
        .copied()
        .filter(|(start, end)| *start <= offset && offset <= *end)
        .min_by_key(|(start, end)| end.saturating_sub(*start))
}

fn word_at<'a>(text: &'a str, token: &oxido_language::domain::syntax::Token) -> &'a str {
    text.get(token.span.start()..token.span.end())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexes_nested_types_methods_and_module_headers_in_indentation_mode() {
        let text = "sintaxis indentacion\nmodulo dominio;\n\npublico estructura Alumno:\n    pub nombre: Cadena\n\nimplementar Alumno:\n    publico funcion nuevo(nombre: Cadena) -> mismo:\n        mismo { nombre }\n";
        let symbols = declared_symbols(text, Some(SyntaxMode::Indentation));
        assert!(
            symbols
                .iter()
                .any(|symbol| symbol.name == "Alumno" && symbol.kind == SymbolKind::Struct)
        );
        assert!(symbols.iter().any(|symbol| symbol.name == "nuevo"
            && symbol.kind == SymbolKind::Method
            && symbol.container.as_deref() == Some("Alumno")));
        assert!(
            symbols
                .iter()
                .any(|symbol| symbol.name == "dominio" && symbol.kind == SymbolKind::Module)
        );
    }

    #[test]
    fn indexes_rust_fragments_without_reading_strings_or_comments() {
        let text = "sintaxis llaves\nfn principal() {\n    let texto = \"fn falso()\";\n    // fn comentado()\n}\nstruct Caja { valor: i32 }\nimpl Caja { fn nuevo() -> Self { Self { valor: 0 } } }\n";
        let symbols = declared_symbols(text, Some(SyntaxMode::Braces));
        assert!(
            symbols
                .iter()
                .any(|symbol| symbol.name == "principal" && symbol.kind == SymbolKind::Function)
        );
        assert!(
            symbols
                .iter()
                .any(|symbol| symbol.name == "Caja" && symbol.kind == SymbolKind::Struct)
        );
        assert!(symbols.iter().any(|symbol| symbol.name == "nuevo"
            && symbol.kind == SymbolKind::Method
            && symbol.container.as_deref() == Some("Caja")));
        assert!(!symbols.iter().any(|symbol| symbol.name == "falso"));
        assert!(!symbols.iter().any(|symbol| symbol.name == "comentado"));
    }

    #[test]
    fn records_module_paths_for_symbols_nested_in_braced_modules() {
        let text = "sintaxis llaves\nmod alpha {\n    fn repetida() {}\n    mod interno {\n        struct Dato { valor: i32 }\n    }\n}\n";
        let symbols = declared_symbols(text, Some(SyntaxMode::Braces));
        let function = symbols
            .iter()
            .find(|symbol| symbol.name == "repetida")
            .expect("nested function is indexed");
        assert_eq!(function.module_path, vec!["alpha".to_owned()]);
        let data = symbols
            .iter()
            .find(|symbol| symbol.name == "Dato")
            .expect("nested struct is indexed");
        assert_eq!(
            data.module_path,
            vec!["alpha".to_owned(), "interno".to_owned()]
        );
    }

    #[test]
    fn resolves_impl_type_after_for_in_rust_headers() {
        assert_eq!(impl_type_name("Mostrar for Alumno"), Some("Alumno".into()));
        assert_eq!(impl_type_name("Alumno"), Some("Alumno".into()));
        assert_eq!(impl_type_name("<T> Caja<T>").as_deref(), Some("Caja"));
    }

    #[test]
    fn tracks_parameters_local_bindings_and_inner_shadowing() {
        let text = concat!(
            "sintaxis indentacion\n",
            "funcion suma(valor: i32):\n",
            "    dejar copia = valor\n",
            "    si copia > 0:\n",
            "        dejar valor = copia\n",
            "        retornar valor\n",
            "    retornar copia\n",
        );
        let bindings = local_bindings(text, Some(SyntaxMode::Indentation));
        let parameter = bindings
            .iter()
            .find(|binding| binding.name == "valor" && binding.kind == LocalBindingKind::Parameter)
            .expect("el parámetro se registra");
        assert_eq!(
            parameter.name_start,
            text.find("valor: i32").expect("parámetro en la firma")
        );
        let copia = bindings
            .iter()
            .find(|binding| binding.name == "copia")
            .expect("la variable local se registra");
        assert!(copia.scope_start > parameter.scope_start);
        let shadow = bindings
            .iter()
            .find(|binding| binding.name == "valor" && binding.kind == LocalBindingKind::Variable)
            .expect("el enlace interno sombrea al parámetro");
        let inner_use = text.rfind("retornar valor").expect("uso interno") + "retornar ".len();
        let resolved = local_binding_at(&bindings, "valor", inner_use)
            .expect("el uso interno se resuelve al enlace más cercano");
        assert_eq!(resolved.name_start, shadow.name_start);
        assert_eq!(resolved.kind, LocalBindingKind::Variable);
        let outer_use = text.find("= valor").expect("uso en el inicializador") + 2;
        let resolved = local_binding_at(&bindings, "valor", outer_use)
            .expect("el uso externo se resuelve al parámetro");
        assert_eq!(resolved.kind, LocalBindingKind::Parameter);
    }

    #[test]
    fn tracks_braced_let_and_for_bindings() {
        let text = concat!(
            "sintaxis llaves\n",
            "fn principal() {\n",
            "    let valor = 1;\n",
            "    for item in 0..2 {\n",
            "        let total = item + valor;\n",
            "    }\n",
            "}\n",
        );
        let bindings = local_bindings(text, Some(SyntaxMode::Braces));
        for name in ["valor", "item", "total"] {
            assert!(
                bindings.iter().any(|binding| binding.name == name),
                "falta el enlace {name}: {bindings:?}"
            );
        }
        let total = bindings
            .iter()
            .find(|binding| binding.name == "total")
            .expect("el enlace del bucle interno");
        assert!(total.scope_start > text.find("for item").expect("bucle"));
    }
}
