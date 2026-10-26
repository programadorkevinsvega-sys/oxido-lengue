//! Índice local de declaraciones `.ox`.
//!
//! No es un analizador de tipos: solo justifica nombres, contenedores y rangos
//! léxicos/estructurales que el editor puede usar como fallback local. La
//! semántica Rust sigue delegada en `rust-analyzer` y `rustc`.

use oxido_language::{
    SourceFile,
    domain::{
        ast::{AggregateKind, ContainerKind, File as OxFile, Item, RustFragment},
        syntax::{LexOptions, SyntaxMode, TokenKind, lex},
    },
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
    let lexed = lex(
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
}
