use crate::domain::{
    ast::{
        Block, BlockStyle, ControlAlternative, ControlExpression, ControlKind, ControlStatement,
        Expression, File, Function, Item, LetStatement, RustFragment, Statement,
    },
    diagnostics::Diagnostic,
    source::{SourceFile, Span},
    syntax::{LexOptions, SyntaxMode, Token, TokenKind, lex},
};

#[derive(Clone, Debug)]
pub struct ParseOutput {
    pub file: Option<File>,
    pub diagnostics: Vec<Diagnostic>,
}

impl ParseOutput {
    #[must_use]
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == crate::domain::diagnostics::Severity::Error)
    }
}

/// Lexes and parses the structural Óxido subset while preserving Rust fragments.
pub fn parse(source: &SourceFile, options: LexOptions) -> ParseOutput {
    let lexed = lex(source, options);
    let mut parser = Parser {
        source,
        tokens: lexed.tokens,
        position: 0,
        diagnostics: lexed.diagnostics,
        mode: lexed.mode.unwrap_or(SyntaxMode::Braces),
    };
    let file = parser.parse_file();
    let has_errors = parser.has_errors();
    ParseOutput {
        file: (!has_errors).then_some(file),
        diagnostics: parser.diagnostics,
    }
}

struct Parser<'a> {
    source: &'a SourceFile,
    tokens: Vec<Token>,
    position: usize,
    diagnostics: Vec<Diagnostic>,
    mode: SyntaxMode,
}

impl Parser<'_> {
    fn parse_file(&mut self) -> File {
        let start = 0;
        let mut items = Vec::new();
        let mut saw_item = false;
        while self.current_is_mode_header()
            || self.is_kind(|kind| matches!(kind, TokenKind::Newline | TokenKind::Comment))
        {
            self.position += 1;
        }
        loop {
            self.skip_trivia();
            if self.at_end() {
                break;
            }
            if self.current_is_mode_header() {
                let token = self.current().cloned();
                self.error(
                    "OX0002",
                    "la declaración de modo debe estar al inicio del archivo",
                    token.map(|t| t.span),
                );
                self.position += 1;
                continue;
            }
            let before = self.position;
            let attributes = self.parse_attributes();
            self.skip_comments();
            let visibility = self.parse_visibility();
            let asynchronous = self.consume_keyword("async");
            if self.is_keyword("fn") {
                if let Some(function) = self.parse_function(attributes, visibility, asynchronous) {
                    items.push(Item::Function(Box::new(function)));
                    saw_item = true;
                    continue;
                }
                self.recover_to_next_item();
                continue;
            } else if self.mode == SyntaxMode::Braces {
                self.position = before;
                if let Some(item) = self.parse_passthrough_item() {
                    items.push(Item::RustFragment(item));
                    saw_item = true;
                    continue;
                }
            }
            if self.position == before || self.current().is_some() {
                self.error_current(
                    "OX4001",
                    if saw_item {
                        "se esperaba una función o un item Rust compatible"
                    } else {
                        "construcción superior no admitida por el parser inicial"
                    },
                );
            }
            self.recover_to_next_item();
        }
        File {
            mode: self.mode,
            items,
            span: self
                .source
                .span(start, self.source.text().len())
                .unwrap_or_else(Span::empty),
        }
    }

    fn parse_function(
        &mut self,
        attributes: Vec<RustFragment>,
        visibility: Option<RustFragment>,
        asynchronous: bool,
    ) -> Option<Function> {
        let fn_token = self.current()?.clone();
        self.position += 1;
        let header_start = self.position;
        let body_open = self.find_function_body_open()?;
        let header_end = body_open;
        let name_token = self.tokens[header_start..header_end]
            .iter()
            .find(|token| matches!(token.kind, TokenKind::Identifier { .. }))?;
        let name = self.source.slice(name_token.span).to_string();
        let signature = self.fragment(header_start, header_end);
        self.position = body_open;
        let body = self.parse_block()?;
        let span = self.join_spans(
            visibility
                .as_ref()
                .map_or(fn_token.span, |fragment| fragment.span),
            body.span,
        );
        Some(Function {
            attributes,
            visibility,
            asynchronous,
            name,
            signature,
            body,
            span,
        })
    }

    fn find_function_body_open(&mut self) -> Option<usize> {
        let mut depth = 0usize;
        let mut index = self.position;
        while let Some(token) = self.tokens.get(index) {
            if depth == 0 {
                if self.mode == SyntaxMode::Braces && self.token_text(token) == "{" {
                    return Some(index);
                }
                if self.mode == SyntaxMode::Indentation
                    && self.token_text(token) == ":"
                    && self
                        .next_significant_text(index + 1)
                        .is_none_or(|text| text == "\n")
                {
                    return Some(index);
                }
                if matches!(token.kind, TokenKind::Newline) && self.mode == SyntaxMode::Braces {
                    // Rust signatures may span lines; do not treat a newline as the item boundary.
                }
            }
            match self.token_text(token) {
                "(" | "[" => depth += 1,
                ")" | "]" => depth = depth.saturating_sub(1),
                "{" if depth > 0 => depth += 1,
                "}" if depth > 0 => depth = depth.saturating_sub(1),
                _ => {}
            }
            index += 1;
        }
        let span = self.current().map(|token| token.span);
        self.error("OX2003", "se esperaba un cuerpo de función", span);
        None
    }

    fn parse_block(&mut self) -> Option<Block> {
        match self.mode {
            SyntaxMode::Indentation => self.parse_indented_block(),
            SyntaxMode::Braces => self.parse_braced_block(),
        }
    }

    fn parse_indented_block(&mut self) -> Option<Block> {
        let opening = self.current()?.clone();
        if self.token_text(&opening) != ":" {
            self.error(
                "OX2002",
                "se esperaba ':' para abrir un bloque indentado",
                Some(opening.span),
            );
            return None;
        }
        self.position += 1;
        self.skip_comments();
        if !self.consume_kind(|kind| matches!(kind, TokenKind::Newline)) {
            self.error_current("OX2003", "se esperaba un salto de línea después de ':'");
            return None;
        }
        self.skip_comments();
        if !self.consume_kind(|kind| matches!(kind, TokenKind::Indent)) {
            self.error_current(
                "OX2003",
                "se esperaba un cuerpo con cuatro espacios de indentación",
            );
            return None;
        }
        let mut statements = Vec::new();
        while !self.at_end() && !self.is_kind(|kind| matches!(kind, TokenKind::Dedent)) {
            self.skip_trivia();
            if self.is_kind(|kind| matches!(kind, TokenKind::Dedent)) || self.at_end() {
                break;
            }
            let before = self.position;
            if let Some(statement) = self.parse_statement(BlockStyle::Indentation) {
                statements.push(statement);
            } else {
                self.recover_line();
            }
            if before == self.position {
                self.position += 1;
            }
        }
        let end = self.current().map_or(opening.span, |token| token.span);
        self.consume_kind(|kind| matches!(kind, TokenKind::Dedent));
        Some(self.finish_block(BlockStyle::Indentation, statements, opening.span, end))
    }

    fn parse_braced_block(&mut self) -> Option<Block> {
        let opening = self.current()?.clone();
        if self.token_text(&opening) != "{" {
            self.error(
                "OX2002",
                "se esperaba '{' para abrir un bloque",
                Some(opening.span),
            );
            return None;
        }
        self.position += 1;
        let mut statements = Vec::new();
        while !self.at_end() && !self.is_text("}") {
            self.skip_trivia();
            if self.is_text("}") || self.at_end() {
                break;
            }
            let before = self.position;
            if let Some(statement) = self.parse_statement(BlockStyle::Braces) {
                statements.push(statement);
            } else {
                self.recover_line_or_semicolon();
            }
            if before == self.position {
                self.position += 1;
            }
        }
        let end = self.current().map_or(opening.span, |token| token.span);
        if !self.consume_text("}") {
            self.error("OX1001", "bloque '{' sin cierre '}'", Some(opening.span));
        }
        Some(self.finish_block(BlockStyle::Braces, statements, opening.span, end))
    }

    fn parse_statement(&mut self, style: BlockStyle) -> Option<Statement> {
        self.skip_comments();
        if self.at_end()
            || self.is_kind(|kind| matches!(kind, TokenKind::Dedent))
            || self.is_text("}")
        {
            return None;
        }
        if self.is_keyword("let") {
            return self.parse_let(style).map(Statement::Let);
        }
        if self.is_keyword("return") {
            return self.parse_control_statement(style).map(Statement::Return);
        }
        if self.is_keyword("break") {
            return self.parse_control_statement(style).map(Statement::Break);
        }
        if self.is_keyword("continue") {
            return self.parse_control_statement(style).map(Statement::Continue);
        }
        if let Some(kind) = self.control_kind() {
            return self
                .parse_control_expression(style, kind)
                .map(Statement::Control);
        }
        self.parse_expression(style).map(Statement::Expression)
    }

    fn parse_let(&mut self, style: BlockStyle) -> Option<LetStatement> {
        let start = self.current()?.span;
        self.position += 1;
        let (parts, had_semicolon) = self.collect_line_or_semicolon(style);
        let equal = self.find_top_level_text(&parts, "=");
        let Some(equal) = equal else {
            self.error(
                "OX1001",
                "declaración 'dejar' sin '=' e inicializador",
                self.span_for_indices(&parts),
            );
            return None;
        };
        let left = &parts[..equal];
        let pattern_start = left.first().copied()?;
        let colon = self.find_colon_in_slice(left);
        let pattern_end = colon.unwrap_or(left.len());
        let pattern = self.fragment(pattern_start, *left.get(pattern_end.saturating_sub(1))? + 1);
        let type_annotation = colon.and_then(|colon| {
            let type_start = left.get(colon + 1).copied()?;
            let type_end = *left.last()? + 1;
            (type_start < type_end).then(|| self.fragment(type_start, type_end))
        });
        let initializer_start = parts.get(equal + 1).copied()?;
        let initializer_end = parts.get(equal + 1..)?.last().copied()? + 1;
        if initializer_start >= initializer_end {
            self.error(
                "OX1001",
                "falta expresión inicializadora",
                self.span_for_indices(&parts),
            );
            return None;
        }
        Some(LetStatement {
            pattern,
            type_annotation,
            initializer: self.fragment(initializer_start, initializer_end),
            had_semicolon,
            span: self.join_spans(start, self.tokens[parts.last().copied().unwrap_or(0)].span),
        })
    }

    fn parse_control_statement(&mut self, style: BlockStyle) -> Option<ControlStatement> {
        let start = self.current()?.span;
        self.position += 1;
        let (parts, had_semicolon) = self.collect_line_or_semicolon(style);
        let value = parts
            .first()
            .zip(parts.last())
            .map(|(first, last)| self.fragment(*first, *last + 1));
        Some(ControlStatement {
            value,
            had_semicolon,
            span: self.join_spans(
                start,
                parts.last().map_or(start, |index| self.tokens[*index].span),
            ),
        })
    }

    fn parse_control_expression(
        &mut self,
        style: BlockStyle,
        kind: ControlKind,
    ) -> Option<ControlExpression> {
        let start = self.current()?.span;
        self.position += 1;
        let open = self.find_control_body_open(style)?;
        let header = self.fragment(self.position, open);
        self.position = open;
        let body = self.parse_block()?;
        let alternative = if matches!(kind, ControlKind::If) {
            self.skip_trivia();
            if self.is_keyword("else") {
                self.position += 1;
                self.skip_comments();
                if self.is_keyword("if") {
                    self.parse_control_expression(style, ControlKind::If)
                        .map(|nested| ControlAlternative::ElseIf(Box::new(nested)))
                } else {
                    self.parse_block().map(ControlAlternative::Block)
                }
            } else {
                None
            }
        } else {
            None
        };
        let end = alternative
            .as_ref()
            .map_or(body.span, |alternative| match alternative {
                ControlAlternative::Block(block) => block.span,
                ControlAlternative::ElseIf(nested) => nested.span,
            });
        Some(ControlExpression {
            kind,
            header,
            body,
            alternative,
            span: self.join_spans(start, end),
        })
    }

    fn find_control_body_open(&mut self, style: BlockStyle) -> Option<usize> {
        let mut depth = 0usize;
        for index in self.position..self.tokens.len() {
            let token = &self.tokens[index];
            if depth == 0 {
                if style == BlockStyle::Braces && self.token_text(token) == "{" {
                    return Some(index);
                }
                if style == BlockStyle::Indentation && self.token_text(token) == ":" {
                    let next = self.next_significant_text(index + 1);
                    if next.is_none_or(|text| text == "\n") {
                        return Some(index);
                    }
                }
                if style == BlockStyle::Indentation && matches!(token.kind, TokenKind::Newline) {
                    self.error(
                        "OX2003",
                        "la cabecera de control no abrió un bloque",
                        Some(token.span),
                    );
                    return None;
                }
            }
            match self.token_text(token) {
                "(" | "[" => depth += 1,
                ")" | "]" => depth = depth.saturating_sub(1),
                "{" if depth > 0 => depth += 1,
                "}" if depth > 0 => depth = depth.saturating_sub(1),
                _ => {}
            }
        }
        self.error_current("OX2003", "se esperaba el bloque de la expresión de control");
        None
    }

    fn parse_expression(&mut self, style: BlockStyle) -> Option<Expression> {
        let (parts, had_semicolon) = self.collect_line_or_semicolon(style);
        let first = *parts.first()?;
        let last = *parts.last()?;
        Some(Expression {
            value: self.fragment(first, last + 1),
            had_semicolon,
            span: self.join_spans(self.tokens[first].span, self.tokens[last].span),
        })
    }

    fn collect_line_or_semicolon(&mut self, style: BlockStyle) -> (Vec<usize>, bool) {
        let mut indices = Vec::new();
        let mut depth = 0usize;
        let mut semicolon = false;
        while let Some(token) = self.current() {
            if depth == 0 {
                if style == BlockStyle::Indentation
                    && matches!(token.kind, TokenKind::Newline | TokenKind::Dedent)
                {
                    break;
                }
                if style == BlockStyle::Braces
                    && (self.token_text(token) == "}" || matches!(token.kind, TokenKind::Dedent))
                {
                    break;
                }
                if self.token_text(token) == ";" {
                    semicolon = true;
                    self.position += 1;
                    break;
                }
            }
            let text = self.token_text(token);
            match text {
                "(" | "[" | "{" => depth += 1,
                ")" | "]" | "}" => depth = depth.saturating_sub(1),
                _ => {}
            }
            if !matches!(token.kind, TokenKind::Comment | TokenKind::Newline) {
                indices.push(self.position);
            }
            self.position += 1;
        }
        if style == BlockStyle::Braces && !semicolon && !self.is_text("}") {
            // Continuar hasta `;` o el cierre del bloque: los saltos de línea no separan Rust statements.
            while let Some(token) = self.current() {
                if self.token_text(token) == "}" && depth == 0 {
                    break;
                }
                if self.token_text(token) == ";" && depth == 0 {
                    semicolon = true;
                    self.position += 1;
                    break;
                }
                match self.token_text(token) {
                    "(" | "[" | "{" => depth += 1,
                    ")" | "]" | "}" => depth = depth.saturating_sub(1),
                    _ => {}
                }
                if !matches!(token.kind, TokenKind::Comment | TokenKind::Newline) {
                    indices.push(self.position);
                }
                self.position += 1;
            }
        }
        if style == BlockStyle::Indentation {
            self.consume_kind(|kind| matches!(kind, TokenKind::Newline));
        }
        (indices, semicolon)
    }

    fn finish_block(
        &mut self,
        style: BlockStyle,
        mut statements: Vec<Statement>,
        open: Span,
        close: Span,
    ) -> Block {
        let tail = match statements.last() {
            Some(Statement::Expression(expression)) if !expression.had_semicolon => {
                match statements.pop() {
                    Some(Statement::Expression(expression)) => Some(expression),
                    _ => None,
                }
            }
            _ => None,
        };
        let span = self.join_spans(open, close);
        Block {
            style,
            statements,
            tail,
            span,
        }
    }

    fn parse_attributes(&mut self) -> Vec<RustFragment> {
        let mut attributes = Vec::new();
        loop {
            self.skip_comments();
            if !self.is_text("#") {
                break;
            }
            let start = self.position;
            self.position += 1;
            if self.is_text("!") {
                self.position += 1;
            }
            if !self.consume_text("[") {
                self.error_current("OX1001", "se esperaba '[' después de '#' en atributo");
                break;
            }
            let mut depth = 1usize;
            while !self.at_end() && depth > 0 {
                match self.current_text() {
                    Some("[") => depth += 1,
                    Some("]") => depth -= 1,
                    _ => {}
                }
                self.position += 1;
            }
            if depth > 0 {
                self.error(
                    "OX1001",
                    "atributo sin cerrar",
                    self.tokens.get(start).map(|token| token.span),
                );
            }
            attributes.push(self.fragment(start, self.position));
            self.skip_trivia();
        }
        attributes
    }

    fn parse_visibility(&mut self) -> Option<RustFragment> {
        if !self.is_keyword("pub") {
            return None;
        }
        let start = self.position;
        self.position += 1;
        if self.is_text("(") {
            let mut depth = 0usize;
            while let Some(token) = self.current() {
                match self.token_text(token) {
                    "(" => depth += 1,
                    ")" => {
                        depth = depth.saturating_sub(1);
                        self.position += 1;
                        if depth == 0 {
                            break;
                        }
                        continue;
                    }
                    _ => {}
                }
                self.position += 1;
            }
        }
        Some(self.fragment(start, self.position))
    }

    fn parse_passthrough_item(&mut self) -> Option<RustFragment> {
        let start = self.position;
        let first = self.current_text()?;
        let item_keyword = matches!(
            first,
            "struct"
                | "estructura"
                | "enum"
                | "enumeracion"
                | "impl"
                | "implementar"
                | "trait"
                | "rasgo"
                | "mod"
                | "modulo"
                | "use"
                | "usar"
                | "type"
                | "tipo"
                | "const"
                | "constante"
                | "static"
                | "estatico"
                | "extern"
                | "externo"
                | "union"
                | "macro_rules"
        );
        let macro_call = self
            .tokens
            .get(start + 1)
            .is_some_and(|token| self.token_text(token) == "!");
        if !item_keyword && !macro_call {
            return None;
        }
        let mut depth = 0usize;
        let mut consumed = false;
        while let Some(token) = self.current() {
            let text = self.token_text(token);
            if consumed && depth == 0 && matches!(token.kind, TokenKind::Newline) {
                break;
            }
            if depth == 0 && text == ";" {
                self.position += 1;
                consumed = true;
                break;
            }
            match text {
                "(" | "[" | "{" => depth += 1,
                ")" | "]" | "}" => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 && text == "}" {
                        self.position += 1;
                        consumed = true;
                        break;
                    }
                }
                _ => {}
            }
            self.position += 1;
            consumed = true;
        }
        if consumed {
            Some(self.fragment(start, self.position))
        } else {
            None
        }
    }

    fn recover_to_next_item(&mut self) {
        let start = self.position;
        while !self.at_end() {
            if self.position > start
                && matches!(
                    self.current().map(|token| &token.kind),
                    Some(TokenKind::Newline)
                )
            {
                self.position += 1;
                self.skip_trivia();
                if self.is_keyword("fn")
                    || self.is_keyword("async")
                    || self.is_keyword("pub")
                    || self.is_kind(|kind| matches!(kind, TokenKind::ModeHeader(_)))
                {
                    break;
                }
                continue;
            }
            self.position += 1;
        }
    }

    fn recover_line(&mut self) {
        while !self.at_end()
            && !self.is_kind(|kind| matches!(kind, TokenKind::Newline | TokenKind::Dedent))
        {
            self.position += 1;
        }
        self.consume_kind(|kind| matches!(kind, TokenKind::Newline));
    }

    fn recover_line_or_semicolon(&mut self) {
        while !self.at_end()
            && !self.is_text("}")
            && !self.is_kind(|kind| matches!(kind, TokenKind::Newline | TokenKind::Dedent))
        {
            let semicolon = self.is_text(";");
            self.position += 1;
            if semicolon {
                break;
            }
        }
        self.consume_kind(|kind| matches!(kind, TokenKind::Newline));
    }

    fn find_top_level_text(&self, indices: &[usize], needle: &str) -> Option<usize> {
        let mut depth = 0usize;
        for (offset, index) in indices.iter().enumerate() {
            match self.token_text(&self.tokens[*index]) {
                "(" | "[" | "{" => depth += 1,
                ")" | "]" | "}" => depth = depth.saturating_sub(1),
                text if text == needle && depth == 0 => return Some(offset),
                _ => {}
            }
        }
        None
    }

    fn find_colon_in_slice(&self, indices: &[usize]) -> Option<usize> {
        self.find_top_level_text(indices, ":")
    }

    fn fragment(&self, start: usize, end: usize) -> RustFragment {
        let span = match (
            self.tokens.get(start),
            self.tokens.get(end.saturating_sub(1)),
        ) {
            (Some(first), Some(last)) => self.join_spans(first.span, last.span),
            _ => Span::empty(),
        };
        RustFragment {
            text: self.source.slice(span).to_string(),
            span,
        }
    }

    fn span_for_indices(&self, indices: &[usize]) -> Option<Span> {
        Some(self.join_spans(
            self.tokens.get(*indices.first()?)?.span,
            self.tokens.get(*indices.last()?)?.span,
        ))
    }

    fn join_spans(&self, start: Span, end: Span) -> Span {
        self.source
            .span(start.start(), end.end())
            .unwrap_or_else(Span::empty)
    }

    fn token_text<'a>(&'a self, token: &Token) -> &'a str {
        self.source.slice(token.span)
    }
    fn current_text(&self) -> Option<&str> {
        self.current().map(|token| self.token_text(token))
    }
    fn current(&self) -> Option<&Token> {
        self.tokens.get(self.position)
    }
    fn at_end(&self) -> bool {
        self.position >= self.tokens.len()
    }
    fn is_text(&self, text: &str) -> bool {
        self.current_text() == Some(text)
    }
    fn is_kind(&self, predicate: impl FnOnce(&TokenKind) -> bool) -> bool {
        self.current().is_some_and(|token| predicate(&token.kind))
    }
    fn consume_text(&mut self, text: &str) -> bool {
        if self.is_text(text) {
            self.position += 1;
            true
        } else {
            false
        }
    }
    fn consume_kind(&mut self, predicate: impl FnOnce(&TokenKind) -> bool) -> bool {
        if self.is_kind(predicate) {
            self.position += 1;
            true
        } else {
            false
        }
    }

    fn is_keyword(&self, rust: &str) -> bool {
        matches!(self.current().map(|token| &token.kind), Some(TokenKind::Keyword { rust_keyword, .. }) if *rust_keyword == rust)
    }
    fn consume_keyword(&mut self, rust: &str) -> bool {
        if self.is_keyword(rust) {
            self.position += 1;
            true
        } else {
            false
        }
    }
    fn control_kind(&self) -> Option<ControlKind> {
        match self.current().map(|token| &token.kind) {
            Some(TokenKind::Keyword {
                rust_keyword: "if", ..
            }) => Some(ControlKind::If),
            Some(TokenKind::Keyword {
                rust_keyword: "loop",
                ..
            }) => Some(ControlKind::Loop),
            Some(TokenKind::Keyword {
                rust_keyword: "while",
                ..
            }) => Some(ControlKind::While),
            Some(TokenKind::Keyword {
                rust_keyword: "for",
                ..
            }) => Some(ControlKind::For),
            _ => None,
        }
    }
    fn current_is_mode_header(&self) -> bool {
        self.is_kind(|kind| matches!(kind, TokenKind::ModeHeader(_)))
    }
    fn skip_comments(&mut self) {
        while self.is_kind(|kind| matches!(kind, TokenKind::Comment)) {
            self.position += 1;
        }
    }
    fn skip_trivia(&mut self) {
        while self.is_kind(|kind| matches!(kind, TokenKind::Comment | TokenKind::Newline)) {
            self.position += 1;
        }
    }
    fn next_significant_text(&self, from: usize) -> Option<&str> {
        self.tokens[from..]
            .iter()
            .find(|token| !matches!(token.kind, TokenKind::Comment))
            .map(|token| self.token_text(token))
    }
    fn error_current(&mut self, code: &'static str, message: impl Into<String>) {
        self.error(code, message, self.current().map(|token| token.span));
    }
    fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == crate::domain::diagnostics::Severity::Error)
    }
    fn error(&mut self, code: &'static str, message: impl Into<String>, span: Option<Span>) {
        self.diagnostics.push(Diagnostic::error(
            code,
            message,
            span.unwrap_or_else(Span::empty),
        ));
    }
}
