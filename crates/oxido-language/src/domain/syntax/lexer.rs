use crate::domain::{
    diagnostics::{Diagnostic, Severity},
    source::{SourceFile, Span},
    syntax::{SyntaxMode, Token, TokenKind, token::keyword},
};
use unicode_ident::{is_xid_continue, is_xid_start};

const MAX_SOURCE_BYTES: usize = 4 * 1024 * 1024;
const MAX_GROUP_DEPTH: usize = 256;
const MAX_TOKENS: usize = 1_000_000;

#[derive(Clone, Copy, Debug, Default)]
pub struct LexOptions {
    pub project_mode: Option<SyntaxMode>,
}

#[derive(Clone, Debug)]
pub struct LexOutput {
    pub tokens: Vec<Token>,
    pub diagnostics: Vec<Diagnostic>,
    pub mode: Option<SyntaxMode>,
}

impl LexOutput {
    #[must_use]
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|item| item.severity == Severity::Error)
    }
}

#[derive(Clone, Copy)]
struct Header {
    start: usize,
    end: usize,
    mode: SyntaxMode,
}

#[derive(Clone, Copy)]
struct Group {
    delimiter: char,
    span: Span,
    opaque: bool,
}

/// Tokeniza una fuente sin transformar su texto ni entrar en pánico por entradas inválidas.
pub fn lex(source: &SourceFile, options: LexOptions) -> LexOutput {
    let text = source.text();
    let mut scanner = Scanner {
        source,
        text,
        options,
        tokens: Vec::new(),
        diagnostics: Vec::new(),
        position: 0,
        line_start: 0,
        prefix_done: false,
        pending_indent: None,
        indent_stack: vec![0],
        groups: Vec::new(),
        mode: None,
        header: detect_header(text),
    };
    scanner.initialize_mode();
    if text.len() > MAX_SOURCE_BYTES {
        let span = scanner.span(0, text.len());
        scanner.error("OX6001", "la fuente supera el límite de 4 MiB", span);
        return scanner.finish();
    }
    for (offset, ch) in text.char_indices() {
        if is_bidi_control(ch) {
            scanner.error(
                "OX1002",
                "carácter de control bidireccional rechazado",
                scanner.span(offset, offset + ch.len_utf8()),
            );
        }
    }
    scanner.scan();
    scanner.finish()
}

struct Scanner<'a> {
    source: &'a SourceFile,
    text: &'a str,
    options: LexOptions,
    tokens: Vec<Token>,
    diagnostics: Vec<Diagnostic>,
    position: usize,
    line_start: usize,
    prefix_done: bool,
    pending_indent: Option<(usize, Span)>,
    indent_stack: Vec<usize>,
    groups: Vec<Group>,
    mode: Option<SyntaxMode>,
    header: Option<Header>,
}

impl Scanner<'_> {
    fn initialize_mode(&mut self) {
        let header_mode = self.header.map(|header| header.mode);
        self.mode = self.options.project_mode.or(header_mode);
        if let (Some(project), Some(file_mode), Some(header)) =
            (self.options.project_mode, header_mode, self.header)
        {
            if project != file_mode {
                self.error(
                    "OX0002",
                    "el modo del archivo contradice la configuración del proyecto",
                    self.span(header.start, header.end),
                );
            }
        }
        if self.mode.is_none() {
            self.error(
                "OX0003",
                "falta el modo de sintaxis del proyecto o del archivo",
                self.span(0, 0),
            );
            self.mode = Some(SyntaxMode::Braces);
        }
    }

    fn scan(&mut self) {
        while self.position < self.text.len() && self.tokens.len() < MAX_TOKENS {
            if self.at_line_start() {
                self.scan_line_prefix();
                continue;
            }
            let start = self.position;
            let Some(ch) = self.peek_char() else { break };
            if ch == '\n'
                || (ch == '\r' && self.text.as_bytes().get(self.position + 1) == Some(&b'\n'))
            {
                self.scan_newline();
                continue;
            }
            if ch == '\r' || ch == ' ' || ch == '\t' {
                self.advance_char();
                continue;
            }
            if ch.is_whitespace() {
                self.advance_char();
                self.error(
                    "OX1001",
                    "espacio Unicode no admitido fuera de literales",
                    self.span(start, self.position),
                );
                continue;
            }
            if self.is_comment_start() {
                self.scan_comment();
                continue;
            }
            if let Some(header) = self.header.filter(|header| header.start == start) {
                self.pending_indent = None;
                self.position = header.end;
                self.emit(TokenKind::ModeHeader(header.mode), header.start, header.end);
                continue;
            }
            self.apply_pending_indent();
            if self.scan_literal() {
                continue;
            }
            if self.scan_identifier() {
                continue;
            }
            if ch.is_ascii_digit() {
                self.scan_number();
                continue;
            }
            if is_punctuation_start(ch) {
                self.scan_punctuation();
                continue;
            }
            self.advance_char();
            self.error(
                "OX1001",
                format!("carácter no válido: {ch:?}"),
                self.span(start, self.position),
            );
        }
        if self.tokens.len() >= MAX_TOKENS {
            self.error(
                "OX6001",
                "la fuente supera el límite de un millón de tokens",
                self.span(self.position, self.position),
            );
        }
        if !self.groups.is_empty() {
            let unclosed = std::mem::take(&mut self.groups);
            for group in unclosed {
                self.error(
                    "OX1001",
                    format!("delimitador '{}' sin cerrar", group.delimiter),
                    group.span,
                );
            }
        }
        while self.indent_stack.len() > 1 {
            self.indent_stack.pop();
            self.emit(TokenKind::Dedent, self.position, self.position);
        }
    }

    fn finish(self) -> LexOutput {
        LexOutput {
            tokens: self.tokens,
            diagnostics: self.diagnostics,
            mode: self.mode,
        }
    }

    fn scan_line_prefix(&mut self) {
        let start = self.position;
        let mut spaces = 0usize;
        while let Some(ch) = self.peek_char() {
            if ch == ' ' {
                spaces += 1;
                self.advance_char();
            } else if ch == '\t' {
                let tab_start = self.position;
                self.advance_char();
                spaces += 1;
                if self.mode == Some(SyntaxMode::Indentation) {
                    self.error(
                        "OX2001",
                        "no se permiten tabuladores en la indentación",
                        self.span(tab_start, self.position),
                    );
                }
            } else {
                break;
            }
        }
        self.prefix_done = true;
        let prefix_span = self.span(start, self.position);
        self.pending_indent = Some((spaces, prefix_span));
        if self.position >= self.text.len() {
            return;
        }
        if self.peek_char() == Some('\n')
            || (self.peek_char() == Some('\r')
                && self.text.as_bytes().get(self.position + 1) == Some(&b'\n'))
        {
            return;
        }
        if self.text[self.position..].starts_with("//") {
            self.scan_comment();
            return;
        }
        if let Some(header) = self.header {
            if self.position == header.start {
                self.pending_indent = None;
                self.position = header.end;
                self.emit(TokenKind::ModeHeader(header.mode), header.start, header.end);
            }
        }
    }

    fn apply_pending_indent(&mut self) {
        let Some((width, span)) = self.pending_indent.take() else {
            return;
        };
        if self.mode != Some(SyntaxMode::Indentation) || !self.groups.is_empty() {
            return;
        }
        if width % 4 != 0 {
            self.error(
                "OX2001",
                "la indentación debe avanzar en múltiplos de cuatro espacios",
                span,
            );
        }
        let current = *self.indent_stack.last().unwrap_or(&0);
        if width > current {
            if width != current + 4 {
                self.error("OX2001", "la indentación no puede saltar niveles", span);
            }
            self.indent_stack.push(width);
            self.emit(TokenKind::Indent, span.start(), span.end());
        } else if width < current {
            while self.indent_stack.len() > 1 && width < *self.indent_stack.last().unwrap_or(&0) {
                self.indent_stack.pop();
                self.emit(TokenKind::Dedent, span.start(), span.end());
            }
            if width != *self.indent_stack.last().unwrap_or(&0) {
                self.error(
                    "OX2001",
                    "la dedentación no coincide con un nivel abierto",
                    span,
                );
            }
        }
    }

    fn scan_comment(&mut self) {
        let start = self.position;
        if self.text[self.position..].starts_with("//") {
            while self.position < self.text.len() && self.peek_char() != Some('\n') {
                self.advance_char();
            }
            self.emit(TokenKind::Comment, start, self.position);
            return;
        }
        self.position += 2;
        let mut depth = 1usize;
        while self.position < self.text.len() && depth > 0 {
            if self.text[self.position..].starts_with("/*") {
                depth += 1;
                self.position += 2;
                if depth > MAX_GROUP_DEPTH {
                    self.error(
                        "OX6001",
                        "los comentarios anidados superan la profundidad máxima",
                        self.span(start, self.position),
                    );
                    self.position = self.text.len();
                    depth = 0;
                }
            } else if self.text[self.position..].starts_with("*/") {
                depth -= 1;
                self.position += 2;
            } else {
                self.advance_char();
            }
        }
        if depth != 0 {
            self.error(
                "OX1001",
                "comentario de bloque sin cerrar",
                self.span(start, self.position),
            );
        }
        self.emit(TokenKind::Comment, start, self.position);
        self.reset_line_state_after_multiline(start);
    }

    fn scan_literal(&mut self) -> bool {
        let start = self.position;
        let rest = &self.text[self.position..];
        let raw_prefix = if rest.starts_with("br") || rest.starts_with("cr") {
            2
        } else if rest.starts_with('r') {
            1
        } else {
            0
        };
        if raw_prefix > 0 {
            let mut cursor = self.position + raw_prefix;
            let mut hashes = 0usize;
            while self.text.as_bytes().get(cursor) == Some(&b'#') {
                hashes += 1;
                cursor += 1;
            }
            if self.text.as_bytes().get(cursor) == Some(&b'"') {
                cursor += 1;
                let mut closed = None;
                while cursor < self.text.len() {
                    if self.text.as_bytes()[cursor] == b'"' {
                        let mut after = cursor + 1;
                        let mut found = 0;
                        while found < hashes && self.text.as_bytes().get(after) == Some(&b'#') {
                            found += 1;
                            after += 1;
                        }
                        if found == hashes {
                            closed = Some(after);
                            break;
                        }
                    }
                    cursor += self.text[cursor..]
                        .chars()
                        .next()
                        .map(char::len_utf8)
                        .unwrap_or(1);
                }
                self.position = closed.unwrap_or(self.text.len());
                if closed.is_none() {
                    self.error(
                        "OX1001",
                        "cadena raw sin cerrar",
                        self.span(start, self.position),
                    );
                }
                self.emit(TokenKind::StringLiteral, start, self.position);
                self.reset_line_state_after_multiline(start);
                return true;
            }
        }
        let byte_or_c_prefix = if rest.starts_with("b\"") || rest.starts_with("c\"") {
            1
        } else {
            0
        };
        if self.peek_char() == Some('"') || byte_or_c_prefix == 1 {
            if byte_or_c_prefix == 1 {
                self.position += 1;
            }
            self.position += 1;
            let mut closed = false;
            while self.position < self.text.len() {
                match self.peek_char() {
                    Some('"') => {
                        self.advance_char();
                        closed = true;
                        break;
                    }
                    Some('\\') => {
                        self.advance_char();
                        if self.position < self.text.len() {
                            self.advance_char();
                        }
                    }
                    Some('\n') => break,
                    Some(_) => {
                        self.advance_char();
                    }
                    None => break,
                }
            }
            if !closed {
                self.error(
                    "OX1001",
                    "cadena sin cerrar",
                    self.span(start, self.position),
                );
            }
            self.emit(TokenKind::StringLiteral, start, self.position);
            return true;
        }
        let byte_char = rest.starts_with("b'");
        if self.peek_char() == Some('\'') || byte_char {
            if byte_char {
                self.position += 1;
            }
            self.position += 1;
            let content_start = self.position;
            let mut char_literal = false;
            if self.peek_char() == Some('\\') {
                self.advance_char();
                if self.peek_char() == Some('u') && self.text[self.position..].starts_with("u{") {
                    while self.position < self.text.len()
                        && self.peek_char() != Some('}')
                        && self.peek_char() != Some('\n')
                    {
                        self.advance_char();
                    }
                    if self.peek_char() == Some('}') {
                        self.advance_char();
                    }
                } else if self.position < self.text.len() {
                    self.advance_char();
                }
                if self.peek_char() == Some('\'') {
                    self.advance_char();
                    char_literal = true;
                }
            } else if let Some(first) = self.peek_char() {
                self.advance_char();
                if self.peek_char() == Some('\'') {
                    self.advance_char();
                    char_literal = true;
                } else if !byte_char && (is_xid_start(first) || first == '_') {
                    while self.peek_char().is_some_and(is_xid_continue) {
                        self.advance_char();
                    }
                    if self.peek_char() == Some('\'') {
                        self.advance_char();
                        char_literal = true;
                    } else {
                        self.emit(TokenKind::Lifetime, start, self.position);
                        return true;
                    }
                }
            }
            if char_literal {
                self.emit(TokenKind::CharacterLiteral, start, self.position);
            } else {
                self.position = self.position.max(content_start);
                self.error(
                    "OX1001",
                    "literal de carácter mal formado",
                    self.span(start, self.position),
                );
                self.emit(TokenKind::CharacterLiteral, start, self.position);
            }
            return true;
        }
        false
    }

    fn scan_identifier(&mut self) -> bool {
        let start = self.position;
        let mut raw = false;
        if self.text[self.position..].starts_with("r#") {
            let after = self.position + 2;
            if self.text[after..]
                .chars()
                .next()
                .is_some_and(|ch| is_xid_start(ch) || ch == '_')
            {
                raw = true;
                self.position = after;
            }
        }
        let Some(first) = self.peek_char() else {
            return false;
        };
        if !is_xid_start(first) && first != '_' {
            self.position = start;
            return false;
        }
        self.advance_char();
        while self.peek_char().is_some_and(is_xid_continue) {
            self.advance_char();
        }
        let word = &self.text[if raw { start + 2 } else { start }..self.position];
        if raw {
            if matches!(word, "_" | "crate" | "self" | "Self" | "super") {
                self.error(
                    "OX1001",
                    format!("r#{word} no es un identificador crudo válido en Rust"),
                    self.span(start, self.position),
                );
            }
            self.emit(TokenKind::Identifier { raw: true }, start, self.position);
        } else if word == "_" {
            self.emit(TokenKind::Underscore, start, self.position);
        } else if let Some(info) = keyword(word) {
            self.emit(
                TokenKind::Keyword {
                    rust_keyword: info.rust_keyword,
                    reserved: info.reserved,
                },
                start,
                self.position,
            );
        } else {
            if has_mixed_confusable_scripts(word) {
                self.warning(
                    "OX1003",
                    "identificador combina alfabetos visualmente confundibles",
                    self.span(start, self.position),
                );
            }
            self.emit(TokenKind::Identifier { raw: false }, start, self.position);
        }
        true
    }

    fn scan_number(&mut self) {
        let start = self.position;
        let bytes = self.text.as_bytes();
        while self.position < bytes.len() {
            let byte = bytes[self.position];
            if byte.is_ascii_alphanumeric() || byte == b'_' {
                self.position += 1;
                continue;
            }
            if byte == b'.' && bytes.get(self.position + 1).is_some_and(u8::is_ascii_digit) {
                self.position += 1;
                continue;
            }
            if (byte == b'+' || byte == b'-')
                && self.position > start
                && matches!(bytes[self.position - 1], b'e' | b'E')
            {
                self.position += 1;
                continue;
            }
            break;
        }
        self.emit(TokenKind::Number, start, self.position);
    }

    fn scan_punctuation(&mut self) {
        let start = self.position;
        let rest = &self.text[self.position..];
        let punct = [
            "..=", "::", "->", "=>", "..", "==", "!=", "<=", ">=", "&&", "||", "+=", "-=", "*=",
            "/=", "%=", "^=", "&=", "|=", "<<", ">>",
        ]
        .into_iter()
        .find(|candidate| rest.starts_with(candidate))
        .unwrap_or_else(|| {
            let ch = self.peek_char().unwrap_or('?');
            let len = ch.len_utf8();
            &self.text[self.position..self.position + len]
        });
        self.position += punct.len();
        let ch = punct.chars().next().unwrap_or('?');
        if matches!(ch, '(' | '[' | '{') {
            if self.groups.len() >= MAX_GROUP_DEPTH {
                self.error(
                    "OX6001",
                    "los grupos anidados superan la profundidad máxima",
                    self.span(start, self.position),
                );
            } else {
                let opaque =
                    self.groups.iter().any(|group| group.opaque) || self.is_opaque_group_start(ch);
                self.groups.push(Group {
                    delimiter: ch,
                    span: self.span(start, self.position),
                    opaque,
                });
            }
        } else if matches!(ch, ')' | ']' | '}') {
            let expected = match ch {
                ')' => '(',
                ']' => '[',
                '}' => '{',
                _ => '?',
            };
            if self
                .groups
                .last()
                .is_some_and(|group| group.delimiter == expected)
            {
                let closed = self.groups.pop();
                self.emit(TokenKind::Punctuation, start, self.position);
                if let (Some(group), Some(token)) = (closed, self.tokens.last_mut()) {
                    token.opaque = group.opaque || self.groups.iter().any(|parent| parent.opaque);
                }
                return;
            } else {
                self.error(
                    "OX1001",
                    format!("delimitador de cierre '{ch}' sin pareja"),
                    self.span(start, self.position),
                );
            }
        }
        self.emit(TokenKind::Punctuation, start, self.position);
    }

    fn scan_newline(&mut self) {
        let start = self.position;
        if self.text.as_bytes().get(self.position) == Some(&b'\r') {
            self.position += 2;
        } else {
            self.position += 1;
        }
        self.emit(TokenKind::Newline, start, self.position);
        self.line_start = self.position;
        self.prefix_done = false;
        self.pending_indent = None;
    }

    fn reset_line_state_after_multiline(&mut self, start: usize) {
        if let Some(relative) = self.text[start..self.position].rfind('\n') {
            self.line_start = start + relative + 1;
            self.prefix_done = true;
            self.pending_indent = None;
        }
    }

    fn is_comment_start(&self) -> bool {
        self.text[self.position..].starts_with("//") || self.text[self.position..].starts_with("/*")
    }
    fn at_line_start(&self) -> bool {
        !self.prefix_done && self.position == self.line_start
    }
    fn peek_char(&self) -> Option<char> {
        self.text.get(self.position..)?.chars().next()
    }
    fn advance_char(&mut self) {
        if let Some(ch) = self.peek_char() {
            self.position += ch.len_utf8();
        }
    }
    fn span(&self, start: usize, end: usize) -> Span {
        self.source.span(start, end).unwrap_or_else(Span::empty)
    }
    fn emit(&mut self, kind: TokenKind, start: usize, end: usize) {
        if self.tokens.len() < MAX_TOKENS {
            let opaque = self.groups.iter().any(|group| group.opaque);
            self.tokens.push(Token {
                kind,
                span: self.span(start, end),
                opaque,
            });
        }
    }

    fn is_opaque_group_start(&self, delimiter: char) -> bool {
        let significant: Vec<_> = self
            .tokens
            .iter()
            .filter(|token| !matches!(token.kind, TokenKind::Newline | TokenKind::Comment))
            .rev()
            .take(3)
            .collect();
        let text = |index: usize| {
            significant
                .get(index)
                .map(|token| self.source.slice(token.span))
        };
        if delimiter == '['
            && (text(0) == Some("#") || (text(0) == Some("!") && text(1) == Some("#")))
        {
            return true;
        }
        if delimiter == '{'
            && text(1) == Some("!")
            && text(2) == Some("macro_rules")
            && significant
                .first()
                .is_some_and(|token| matches!(token.kind, TokenKind::Identifier { .. }))
        {
            return true;
        }
        matches!(delimiter, '(' | '[' | '{')
            && text(0) == Some("!")
            && significant.get(1).is_some_and(|token| {
                matches!(
                    token.kind,
                    TokenKind::Identifier { .. } | TokenKind::Keyword { .. }
                )
            })
    }
    fn error(&mut self, code: &'static str, message: impl Into<String>, span: Span) {
        self.diagnostics
            .push(Diagnostic::error(code, message, span));
    }
    fn warning(&mut self, code: &'static str, message: impl Into<String>, span: Span) {
        self.diagnostics
            .push(Diagnostic::warning(code, message, span));
    }
}

fn detect_header(text: &str) -> Option<Header> {
    let mut position = 0;
    while position < text.len() {
        while let Some(ch) = text[position..].chars().next() {
            if ch.is_whitespace() {
                position += ch.len_utf8();
            } else {
                break;
            }
        }
        if text[position..].starts_with("//") {
            while position < text.len() && text.as_bytes()[position] != b'\n' {
                position += 1;
            }
            continue;
        }
        if text[position..].starts_with("/*") {
            position += 2;
            let mut depth = 1usize;
            while position < text.len() && depth > 0 {
                if text[position..].starts_with("/*") {
                    depth += 1;
                    position += 2;
                } else if text[position..].starts_with("*/") {
                    depth -= 1;
                    position += 2;
                } else if let Some(ch) = text[position..].chars().next() {
                    position += ch.len_utf8();
                } else {
                    break;
                }
            }
            if depth != 0 {
                return None;
            }
            continue;
        }
        let line_end = text[position..]
            .find('\n')
            .map(|offset| position + offset)
            .unwrap_or(text.len());
        let line = text[position..line_end].trim_end_matches('\r');
        let trimmed = line.trim();
        let mode = match trimmed {
            "sintaxis indentacion" => SyntaxMode::Indentation,
            "sintaxis llaves" => SyntaxMode::Braces,
            _ => return None,
        };
        let leading = line.len() - line.trim_start().len();
        let start = position + leading;
        return Some(Header {
            start,
            end: start + trimmed.len(),
            mode,
        });
    }
    None
}

fn is_punctuation_start(ch: char) -> bool {
    "()[]{};,. :#!?+-*/%&|^=!<>@'$".contains(ch)
}
fn is_bidi_control(ch: char) -> bool {
    matches!(ch as u32, 0x061c | 0x200e..=0x200f | 0x202a..=0x202e | 0x2066..=0x2069)
}

fn has_mixed_confusable_scripts(word: &str) -> bool {
    let mut latin = false;
    let mut other = false;
    for ch in word.chars() {
        let point = ch as u32;
        if ch.is_ascii_alphabetic() || (0x00c0..=0x024f).contains(&point) {
            latin = true;
        }
        if (0x0370..=0x03ff).contains(&point) || (0x0400..=0x052f).contains(&point) {
            other = true;
        }
    }
    latin && other
}
