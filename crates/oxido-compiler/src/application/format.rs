//! Formateo canónico del modo indentación compartido por la CLI y el LSP.

use oxido_language::{
    SourceFile,
    domain::syntax::{TokenKind, lexer::LexOutput},
};

const FORMAT_LINE_WIDTH: usize = 100;

pub fn format_indented_source(source: &SourceFile, lexed: &LexOutput) -> String {
    let lines = source.text().split_inclusive('\n').collect::<Vec<_>>();
    let line_count = lines.len().max(1);
    let mut indentation = vec![None; line_count];
    let mut preserve_lines = vec![false; line_count];
    let mut comment_lines = vec![false; line_count];
    let mut opaque_invocation_lines = vec![false; line_count];
    let mut tokens_by_line = vec![Vec::new(); line_count];
    let mut block_depth = 0usize;
    let mut group_depth = 0usize;
    let mut group_depth_stack = Vec::new();
    let mut opaque_macro_lines = Vec::<Option<usize>>::new();
    let mut last_significant_text = None;
    for token in &lexed.tokens {
        match token.kind {
            TokenKind::Indent => {
                block_depth = block_depth.saturating_add(1);
                continue;
            }
            TokenKind::Dedent => {
                block_depth = block_depth.saturating_sub(1);
                continue;
            }
            TokenKind::Newline => continue,
            _ => {}
        }
        let Some(start) = source.position(token.span.start()) else {
            continue;
        };
        let Some(end) = source.position(token.span.end()) else {
            continue;
        };
        let Some(line_indent) = indentation.get_mut(start.line.saturating_sub(1)) else {
            continue;
        };
        if let Some(line_tokens) = tokens_by_line.get_mut(start.line.saturating_sub(1)) {
            line_tokens.push(token);
        }
        let token_text = source.slice(token.span);
        let closing_group = matches!(token_text, ")" | "]" | "}");
        let depth = block_depth + group_depth.saturating_sub(usize::from(closing_group));
        if line_indent.is_none() {
            *line_indent = Some(depth);
        }
        match token_text {
            "(" | "[" | "{" => {
                group_depth_stack.push(group_depth);
                group_depth = if token_text == "{" {
                    1
                } else {
                    group_depth.saturating_add(1)
                };
            }
            ")" | "]" | "}" => {
                group_depth = group_depth_stack.pop().unwrap_or_default();
            }
            _ => {}
        }
        if matches!(token.kind, TokenKind::Comment)
            && let Some(preserve) = comment_lines.get_mut(start.line.saturating_sub(1))
        {
            *preserve = true;
        }
        if matches!(
            token.kind,
            TokenKind::StringLiteral | TokenKind::CharacterLiteral
        ) && start.line != end.line
        {
            for line in start.line.saturating_add(1)..=end.line {
                if let Some(preserve) = preserve_lines.get_mut(line.saturating_sub(1)) {
                    *preserve = true;
                }
            }
        }
        if token.opaque {
            let opening = matches!(token_text, "(" | "[" | "{");
            let closing = matches!(token_text, ")" | "]" | "}");
            if opening {
                let invocation_line = if last_significant_text == Some("!") {
                    Some(start.line)
                } else {
                    opaque_macro_lines.last().copied().flatten()
                };
                opaque_macro_lines.push(invocation_line);
            }
            let invocation_line = opaque_macro_lines.last().copied().flatten();
            for line in start.line..=end.line {
                let line_index = line.saturating_sub(1);
                if invocation_line == Some(line) {
                    if let Some(preserve) = opaque_invocation_lines.get_mut(line_index) {
                        *preserve = true;
                    }
                } else if let Some(preserve) = preserve_lines.get_mut(line_index) {
                    *preserve = true;
                }
            }
            if closing {
                opaque_macro_lines.pop();
            }
        }
        if !matches!(
            token.kind,
            TokenKind::Comment | TokenKind::Newline | TokenKind::Indent | TokenKind::Dedent
        ) {
            last_significant_text = Some(token_text);
        }
    }

    let mut formatted = String::with_capacity(source.text().len() + 1);
    let mut source_line_start = 0usize;
    for (index, line) in lines.iter().enumerate() {
        let (content, ending) = match line.strip_suffix('\n') {
            Some(content) => (content.strip_suffix('\r').unwrap_or(content), {
                if content.ends_with('\r') {
                    "\r\n"
                } else {
                    "\n"
                }
            }),
            None => (*line, ""),
        };
        let (leading, body) = split_indentation(content);
        let mut expected_depth = indentation
            .get(index)
            .copied()
            .flatten()
            .unwrap_or_else(|| {
                if index == 0 {
                    0
                } else {
                    indentation[..index]
                        .iter()
                        .rev()
                        .find_map(|depth| *depth)
                        .unwrap_or(0)
                }
            });
        let is_doc_comment = comment_lines.get(index).copied().unwrap_or(false)
            && tokens_by_line[index]
                .iter()
                .find(|token| matches!(token.kind, TokenKind::Comment))
                .is_some_and(|token| {
                    let text = source.slice(token.span);
                    text.starts_with("///") || text.starts_with("//!")
                });
        if is_doc_comment {
            expected_depth = indentation[index + 1..]
                .iter()
                .find_map(|depth| *depth)
                .unwrap_or(expected_depth);
        }
        if body.is_empty() {
            // Las líneas vacías no conservan espacios de sangría.
        } else if preserve_lines.get(index).copied().unwrap_or(false) {
            formatted.push_str(leading);
            formatted.push_str(body);
        } else {
            formatted.push_str(&" ".repeat(expected_depth * 4));
            if comment_lines.get(index).copied().unwrap_or(false) {
                formatted.push_str(&format_indented_comment_line(
                    body,
                    source,
                    source_line_start + leading.len(),
                    source_line_start + content.len(),
                    expected_depth,
                    ending,
                    &tokens_by_line[index],
                ));
            } else if opaque_invocation_lines.get(index).copied().unwrap_or(false) {
                formatted.push_str(body);
            } else if let Some(formatted_prefix) = format_line_before_multiline_literal(
                source,
                source_line_start + leading.len(),
                source_line_start + content.len(),
                expected_depth,
                ending,
                &tokens_by_line[index],
            ) {
                formatted.push_str(&formatted_prefix);
            } else {
                formatted.push_str(&format_indented_code_line(
                    body.trim_end_matches([' ', '\t']),
                    source,
                    expected_depth,
                    ending,
                    &tokens_by_line[index],
                ));
            }
        }
        formatted.push_str(ending);
        source_line_start += line.len();
    }
    if !formatted.is_empty() && !formatted.ends_with('\n') {
        formatted.push('\n');
    }
    formatted
}

fn format_line_before_multiline_literal(
    source: &SourceFile,
    content_start: usize,
    content_end: usize,
    expected_depth: usize,
    line_ending: &str,
    line_tokens: &[&oxido_language::domain::syntax::Token],
) -> Option<String> {
    let literal = line_tokens.iter().find(|token| {
        matches!(
            token.kind,
            TokenKind::StringLiteral | TokenKind::CharacterLiteral
        ) && source
            .position(token.span.start())
            .zip(source.position(token.span.end()))
            .is_some_and(|(start, end)| start.line < end.line)
    })?;
    let prefix_tokens = line_tokens
        .iter()
        .copied()
        .filter(|token| {
            token.span.end() <= literal.span.start()
                && !matches!(
                    token.kind,
                    TokenKind::Newline | TokenKind::Indent | TokenKind::Dedent | TokenKind::Comment
                )
        })
        .collect::<Vec<_>>();
    let prefix = source
        .text()
        .get(content_start..literal.span.start())?
        .trim_end_matches([' ', '\t']);
    let literal_start = literal.span.start().max(content_start);
    let literal_suffix = source.text().get(literal_start..content_end)?;
    let formatted_prefix =
        format_indented_code_line(prefix, source, expected_depth, line_ending, &prefix_tokens);
    let literal_text = source.slice(literal.span);
    let previous = prefix_tokens.last().map(|token| source.slice(token.span));
    let before_previous = prefix_tokens
        .len()
        .checked_sub(2)
        .map(|index| source.slice(prefix_tokens[index].span));
    let spacing = previous.map_or("", |previous| {
        canonical_token_spacing(before_previous, previous, literal_text, None, "")
    });
    Some(format!("{formatted_prefix}{spacing}{literal_suffix}"))
}

fn split_indentation(line: &str) -> (&str, &str) {
    let leading_end = line
        .char_indices()
        .take_while(|(_, character)| matches!(character, ' ' | '\t'))
        .map(|(index, character)| index + character.len_utf8())
        .last()
        .unwrap_or(0);
    line.split_at(leading_end)
}

fn format_indented_code_line(
    line: &str,
    source: &SourceFile,
    expected_depth: usize,
    line_ending: &str,
    line_tokens: &[&oxido_language::domain::syntax::Token],
) -> String {
    let tokens = line_tokens
        .iter()
        .filter(|token| {
            !matches!(
                token.kind,
                TokenKind::Newline | TokenKind::Indent | TokenKind::Dedent | TokenKind::Comment
            )
        })
        .collect::<Vec<_>>();
    if tokens.len() < 2 || tokens.iter().any(|token| token.opaque) {
        return line.to_owned();
    }
    let mut output = String::with_capacity(line.len());
    let mut previous_end = tokens.first().map_or(0, |token| token.span.start());
    let token_texts = tokens
        .iter()
        .map(|token| source.slice(token.span))
        .collect::<Vec<_>>();
    let should_wrap = line.chars().count() + expected_depth * 4 > FORMAT_LINE_WIDTH;
    let wrap_ending = if line_ending.is_empty() {
        "\n"
    } else {
        line_ending
    };
    let mut groups = Vec::<&str>::new();
    let mut brace_spacing = Vec::<bool>::new();
    let mut wrapped_after_comma = false;
    let mut generic_depth = 0usize;
    for (index, token) in tokens.iter().enumerate() {
        let text = token_texts[index];
        let original_gap = source
            .text()
            .get(previous_end..token.span.start())
            .unwrap_or("");
        if index > 0 {
            let before_previous = index.checked_sub(2).map(|prior| token_texts[prior]);
            let next = token_texts.get(index + 1).copied();
            let previous = token_texts[index - 1];
            let space_before_brace_close = text == "}"
                && groups.last() == Some(&"{")
                && brace_spacing.last().copied().unwrap_or(false)
                && previous != "{";
            if should_wrap && matches!(text, ")" | "]") && groups.len() == 1 {
                while output.ends_with(' ') {
                    output.truncate(output.len().saturating_sub(1));
                }
                if !output.ends_with('\n') {
                    output.push_str(wrap_ending);
                }
                output.push_str(&" ".repeat(expected_depth * 4));
                wrapped_after_comma = false;
            }
            let generic_opener = text == "<"
                && !original_gap.chars().any(char::is_whitespace)
                && (previous.chars().next().is_some_and(char::is_uppercase)
                    || previous == ">"
                    || previous == "::"
                    || matches!(
                        previous,
                        "funcion"
                            | "fn"
                            | "estructura"
                            | "struct"
                            | "enumeracion"
                            | "enum"
                            | "impl"
                            | "implementar"
                            | "rasgo"
                            | "trait"
                            | "tipo"
                            | "type"
                    )
                    || before_previous.is_some_and(|word| {
                        matches!(
                            word,
                            "funcion"
                                | "fn"
                                | "estructura"
                                | "struct"
                                | "enumeracion"
                                | "enum"
                                | "impl"
                                | "implementar"
                                | "rasgo"
                                | "trait"
                                | "tipo"
                                | "type"
                        )
                    }));
            let spacing = if space_before_brace_close {
                " "
            } else if generic_opener
                || wrapped_after_comma
                || (generic_depth > 0 && text == ">")
                || (generic_depth > 0 && matches!(previous, "<" | "::"))
            {
                ""
            } else {
                canonical_token_spacing(before_previous, previous, text, next, original_gap)
            };
            output.push_str(spacing);
            if generic_opener {
                generic_depth = generic_depth.saturating_add(1);
            } else if text == ">" && generic_depth > 0 {
                generic_depth = generic_depth.saturating_sub(1);
            }
        }
        output.push_str(text);
        match text {
            "(" | "[" | "{" => {
                groups.push(text);
                let prior = index.checked_sub(1).map(|prior| token_texts[prior]);
                brace_spacing.push(text == "{" && prior != Some("::"));
            }
            ")" | "]" | "}" => {
                groups.truncate(groups.len().saturating_sub(1));
                brace_spacing.truncate(brace_spacing.len().saturating_sub(1));
            }
            _ => {}
        }
        let wrap_after_open =
            should_wrap && matches!(text, "(" | "[") && token_texts[index + 1..].contains(&",");
        let wrap_after_comma = should_wrap
            && text == ","
            && groups
                .last()
                .is_some_and(|group| matches!(*group, "(" | "["));
        if wrap_after_open || wrap_after_comma {
            output.push_str(wrap_ending);
            output.push_str(&" ".repeat((expected_depth + groups.len()) * 4));
            wrapped_after_comma = true;
        } else {
            wrapped_after_comma = false;
        }
        previous_end = token.span.end();
    }
    output
}

fn format_indented_comment_line(
    line: &str,
    source: &SourceFile,
    content_start: usize,
    content_end: usize,
    expected_depth: usize,
    line_ending: &str,
    line_tokens: &[&oxido_language::domain::syntax::Token],
) -> String {
    let Some(comment) = line_tokens
        .iter()
        .find(|token| matches!(token.kind, TokenKind::Comment))
    else {
        return line.trim_end_matches([' ', '\t']).to_owned();
    };
    let comment_text = source.slice(comment.span);
    if !comment_text.starts_with("//") || comment.span.start() <= content_start {
        return line.to_owned();
    }
    let code_tokens = line_tokens
        .iter()
        .copied()
        .filter(|token| {
            token.span.end() <= comment.span.start()
                && !matches!(
                    token.kind,
                    TokenKind::Newline | TokenKind::Indent | TokenKind::Dedent | TokenKind::Comment
                )
        })
        .collect::<Vec<_>>();
    let code_end = comment.span.start();
    let code = source
        .text()
        .get(content_start..code_end)
        .unwrap_or_default()
        .trim_end_matches([' ', '\t']);
    let formatted_code =
        format_indented_code_line(code, source, expected_depth, line_ending, &code_tokens);
    let comment_and_suffix = source
        .text()
        .get(comment.span.start()..content_end)
        .unwrap_or(comment_text)
        .trim_end_matches([' ', '\t']);
    format!("{formatted_code}  {comment_and_suffix}")
}

fn canonical_token_spacing(
    before_previous: Option<&str>,
    previous: &str,
    current: &str,
    next: Option<&str>,
    original_gap: &str,
) -> &'static str {
    if matches!(current, "," | ";" | ")" | "]" | "}" | "." | "?" | "::")
        || matches!(previous, "(" | "[" | "." | "::" | "!")
    {
        return "";
    }
    if current == "(" {
        return if matches!(previous, "=" | "=>" | "," | "->" | "return" | "retornar")
            || matches!(
                previous,
                "si" | "if" | "mientras" | "while" | "coincidir" | "match"
            ) {
            " "
        } else {
            ""
        };
    }
    if current == "!" {
        return if matches!(previous, "si" | "if" | "mientras" | "while") {
            " "
        } else {
            ""
        };
    }
    if current == "{" {
        return if matches!(previous, "!" | "::" | "[" | "(") {
            ""
        } else {
            " "
        };
    }
    if matches!(current, ":") {
        return "";
    }
    if previous == ">" && current == "(" {
        return "";
    }
    if previous == "{" {
        return if before_previous == Some("::") {
            ""
        } else {
            " "
        };
    }
    if previous == "*" && before_previous == Some("*") {
        return "";
    }
    if matches!(current, "<") {
        let type_name = previous.chars().next().is_some_and(char::is_uppercase);
        let declaration = before_previous.is_some_and(|word| {
            matches!(
                word,
                "funcion"
                    | "fn"
                    | "estructura"
                    | "struct"
                    | "enumeracion"
                    | "enum"
                    | "impl"
                    | "implementar"
                    | "rasgo"
                    | "trait"
                    | "tipo"
                    | "type"
            )
        });
        if (type_name || declaration || previous == "::" || previous == ">")
            && !original_gap.chars().any(char::is_whitespace)
        {
            return "";
        }
        return " ";
    }
    if matches!(current, ">") {
        let type_name = before_previous
            .and_then(|text| text.chars().next())
            .is_some_and(char::is_uppercase);
        if (type_name || matches!(next, Some("(" | ")" | "]" | "," | ";" | "::" | ">")))
            && !original_gap.chars().any(char::is_whitespace)
        {
            return "";
        }
        return " ";
    }
    if matches!(previous, "," | ";" | ":") {
        return " ";
    }
    if matches!(
        previous,
        "=" | "+"
            | "-"
            | "*"
            | "/"
            | "%"
            | "=="
            | "!="
            | "<="
            | ">="
            | "&&"
            | "||"
            | "+="
            | "-="
            | "->"
            | "=>"
            | "<"
            | ">"
    ) || matches!(
        current,
        "=" | "+"
            | "-"
            | "*"
            | "/"
            | "%"
            | "=="
            | "!="
            | "<="
            | ">="
            | "&&"
            | "||"
            | "+="
            | "-="
            | "->"
            | "=>"
            | "<"
            | ">"
    ) {
        if previous == "*"
            && current == "*"
            && before_previous
                .is_some_and(|token| matches!(token, "|" | "(" | "[" | "," | "=" | "=>"))
        {
            return "";
        }
        if matches!(previous, "-" | "*" | "&")
            && before_previous.is_none_or(|token| {
                matches!(
                    token,
                    "=" | "+="
                        | "-="
                        | "*="
                        | "/="
                        | "%="
                        | "&="
                        | "|="
                        | "^="
                        | "<<="
                        | ">>="
                        | "("
                        | "["
                        | ","
                        | ":"
                        | "->"
                        | "=>"
                        | "|"
                        | "retornar"
                        | "return"
                )
            })
        {
            return "";
        }
        return " ";
    }
    if original_gap.chars().any(char::is_whitespace)
        || (previous
            .chars()
            .all(|character| character.is_alphanumeric() || character == '_')
            && current
                .chars()
                .next()
                .is_some_and(|character| character.is_alphanumeric() || character == '_'))
    {
        return " ";
    }
    ""
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxido_language::domain::syntax::{LexOptions, SyntaxMode, lex};

    fn format(text: &str) -> String {
        let source = SourceFile::new(text);
        let lexed = lex(
            &source,
            LexOptions {
                project_mode: Some(SyntaxMode::Indentation),
            },
        );
        format_indented_source(&source, &lexed)
    }

    #[test]
    fn normalizes_spacing_and_nested_indentation() {
        let formatted = format(
            "sintaxis indentacion\nfuncion identidad<T:Clone>( valor:T )->T:   \n  retornar valor\n\nfuncion suma(a:i32,b:i32)->i32:\n  retornar a+b\n\nfuncion principal( ):   \n  dejar resultado=suma(\n      1,\n  2\n ) // llamada   \n  si resultado>0:\n      afirmar_igual!(identidad(resultado),3)\n  sino:\n    afirmar_igual!(resultado,0)\n",
        );
        assert_eq!(
            formatted,
            "sintaxis indentacion\nfuncion identidad<T: Clone>(valor: T) -> T:\n    retornar valor\n\nfuncion suma(a: i32, b: i32) -> i32:\n    retornar a + b\n\nfuncion principal():\n    dejar resultado = suma(\n        1,\n        2\n    )  // llamada\n    si resultado > 0:\n        afirmar_igual!(identidad(resultado),3)\n    sino:\n        afirmar_igual!(resultado,0)\n"
        );
        assert_eq!(format(&formatted), formatted);
    }

    #[test]
    fn preserves_comments_and_multiline_literals() {
        let formatted = format(
            "sintaxis indentacion\n// comentario inicial con espacios   \nfuncion principal():   \n    imprimir!(r#\"primera línea  \nsegunda línea\"#)   \n    // comentario interno con espacios   \n",
        );
        assert!(formatted.contains("// comentario inicial con espacios   \n"));
        assert!(formatted.contains("    // comentario interno con espacios   \n"));
        assert!(formatted.contains("r#\"primera línea  \nsegunda línea\"#"));
        assert!(formatted.contains("funcion principal():\n"));
        assert_eq!(format(&formatted), formatted);
    }
}
