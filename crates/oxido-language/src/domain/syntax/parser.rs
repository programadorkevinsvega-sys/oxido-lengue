use crate::domain::{
    ast::{
        AggregateKind, Block, BlockStyle, ControlAlternative, ControlExpression, ControlKind,
        ControlStatement, Expression, File, Function, IndentedAggregate, IndentedMember,
        IndentedUse, Item, LetStatement, RustFragment, Statement,
    },
    diagnostics::{Diagnostic, DiagnosticSuggestion},
    source::{SourceFile, Span},
    syntax::{LexOptions, SyntaxMode, Token, TokenKind, lex},
};

const MAX_BLOCK_DEPTH: usize = 128;

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
        block_depth: 0,
        control_depth: 0,
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
    block_depth: usize,
    control_depth: usize,
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
            } else {
                self.position = before;
                if let Some(item) = self.parse_indented_item() {
                    items.push(item);
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
        let Some(name_token) = self.tokens.get(header_start) else {
            self.error("OX2003", "se esperaba el nombre de la función", None);
            return None;
        };
        let is_main_alias = matches!(
            name_token.kind,
            TokenKind::Keyword {
                rust_keyword: "main",
                ..
            }
        );
        if !matches!(name_token.kind, TokenKind::Identifier { .. }) && !is_main_alias {
            if matches!(name_token.kind, TokenKind::Keyword { .. }) {
                self.report_reserved_identifier(header_start);
            } else {
                self.error(
                    "OX2003",
                    "se esperaba un identificador como nombre de la función",
                    Some(name_token.span),
                );
            }
            return None;
        }
        let name = if is_main_alias {
            "main".to_string()
        } else {
            self.source.slice(name_token.span).to_string()
        };
        if matches!(name_token.kind, TokenKind::Identifier { raw: false })
            && crate::domain::syntax::prelude_translation(&name).is_some()
        {
            self.report_reserved_identifier(header_start + 1);
        }
        let signature = self.fragment(header_start, header_end);
        self.check_signature_patterns(header_start, header_end);
        self.position = body_open;
        let body = self.parse_block()?;
        let span = self.join_spans(
            attributes
                .first()
                .map(|fragment| fragment.span)
                .or_else(|| visibility.as_ref().map(|fragment| fragment.span))
                .unwrap_or(fn_token.span),
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
                if self.mode == SyntaxMode::Braces
                    && self.token_text(token) == ":"
                    && self
                        .next_significant_text(index + 1)
                        .is_none_or(|text| text == "\n")
                {
                    self.error(
                        "OX2002",
                        "un bloque con ':' no es válido en modo llaves",
                        Some(token.span),
                    );
                    return None;
                }
                if self.mode == SyntaxMode::Indentation && self.token_text(token) == "{" {
                    self.error(
                        "OX2002",
                        "un bloque con llaves no es válido en modo indentación",
                        Some(token.span),
                    );
                    return None;
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
        if self.block_depth >= MAX_BLOCK_DEPTH {
            self.error_current(
                "OX6001",
                format!("los bloques superan la profundidad máxima de {MAX_BLOCK_DEPTH}"),
            );
            return None;
        }
        self.block_depth += 1;
        let parsed = match self.mode {
            SyntaxMode::Indentation => self.parse_indented_block(),
            SyntaxMode::Braces => self.parse_braced_block(),
        };
        self.block_depth -= 1;
        parsed
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
        if self.is_text(";") {
            let span = self.current().map(|token| token.span);
            self.error(
                "OX4001",
                "no se admite una instrucción vacía; elimina este ';'",
                span,
            );
            self.position += 1;
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
        if left.is_empty() {
            self.error(
                "OX1001",
                "falta el patrón de la declaración `dejar`",
                self.span_for_indices(&parts),
            );
            return None;
        }
        let pattern_start = left.first().copied()?;
        let colon = self.find_colon_in_slice(left);
        let pattern_end = colon.unwrap_or(left.len());
        let pattern_last = *left.get(pattern_end.saturating_sub(1))?;
        self.check_pattern_keyword_identifier(pattern_start, pattern_last + 1);
        let pattern = self.fragment(pattern_start, pattern_last + 1);
        let type_annotation = colon.and_then(|colon| {
            let type_start = left.get(colon + 1).copied()?;
            let type_end = *left.last()? + 1;
            (type_start < type_end).then(|| self.fragment(type_start, type_end))
        });
        if colon.is_some() && type_annotation.is_none() {
            self.error(
                "OX1001",
                "falta el tipo después de ':' en la declaración",
                self.span_for_indices(left),
            );
            return None;
        }
        let Some(initializer_start) = parts.get(equal + 1).copied() else {
            self.error(
                "OX1001",
                "falta expresión inicializadora",
                self.span_for_indices(&parts),
            );
            return None;
        };
        let Some(initializer_end) = parts.get(equal + 1..).and_then(|rest| rest.last()).copied()
        else {
            self.error(
                "OX1001",
                "falta expresión inicializadora",
                self.span_for_indices(&parts),
            );
            return None;
        };
        let initializer_end = initializer_end + 1;
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
        if self.control_depth >= MAX_BLOCK_DEPTH {
            self.error_current(
                "OX6001",
                format!(
                    "las expresiones de control superan la profundidad máxima de {MAX_BLOCK_DEPTH}"
                ),
            );
            return None;
        }
        self.control_depth += 1;
        let control = self.parse_control_expression_inner(style, kind);
        self.control_depth -= 1;
        control
    }

    fn parse_control_expression_inner(
        &mut self,
        style: BlockStyle,
        kind: ControlKind,
    ) -> Option<ControlExpression> {
        let start = self.current()?.span;
        self.position += 1;
        let open = self.find_control_body_open(style)?;
        let header = self.fragment(self.position, open);
        if kind == ControlKind::For {
            if let Some(in_index) =
                (self.position..open).find(|index| self.is_keyword_at(*index, "in"))
            {
                self.check_pattern_keyword_identifier(self.position, in_index);
            }
        }
        self.position = open;
        let body = self.parse_block()?;
        let alternative = if matches!(kind, ControlKind::If) {
            self.skip_trivia();
            if self.is_keyword("else") {
                self.position += 1;
                self.skip_comments();
                let alternative = if self.is_keyword("if") {
                    self.parse_control_expression(style, ControlKind::If)
                        .map(|nested| ControlAlternative::ElseIf(Box::new(nested)))
                } else {
                    self.parse_block().map(ControlAlternative::Block)
                };
                if alternative.is_none() {
                    self.error_current("OX2003", "se esperaba un bloque después de `sino`");
                    return None;
                }
                alternative
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
                if style == BlockStyle::Braces
                    && self.token_text(token) == ":"
                    && self
                        .next_significant_text(index + 1)
                        .is_none_or(|text| text == "\n")
                {
                    self.error(
                        "OX2002",
                        "un bloque con ':' no es válido en modo llaves",
                        Some(token.span),
                    );
                    return None;
                }
                if style == BlockStyle::Indentation && self.token_text(token) == "{" {
                    self.error(
                        "OX2002",
                        "un bloque con llaves no es válido en modo indentación",
                        Some(token.span),
                    );
                    return None;
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
        let item_start = self.passthrough_item_start(start)?;
        let first = self.token_text(self.tokens.get(item_start)?);
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
        let macro_call = self.is_macro_invocation_start(item_start);
        if !item_keyword && !macro_call {
            return None;
        }
        let mut depth = 0usize;
        let mut consumed = false;
        while let Some(token) = self.current() {
            let text = self.token_text(token);
            if consumed
                && self.position >= item_start
                && depth == 0
                && matches!(token.kind, TokenKind::Newline)
            {
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
            self.check_passthrough_identifier_collisions(start, self.position);
            Some(self.fragment(start, self.position))
        } else {
            None
        }
    }

    /// Admite items Rust simples y AST propio de struct/enum indentados.
    fn parse_indented_item(&mut self) -> Option<Item> {
        let start = self.position;
        let attributes = self.parse_attributes();
        self.skip_comments();
        let visibility = self.parse_visibility();
        self.skip_comments();
        let item_start = self.position;
        let token = self.current()?;
        let first = self.token_text(token);
        let aggregate_kind = match first {
            "struct" | "estructura" => Some(AggregateKind::Struct),
            "enum" | "enumeracion" => Some(AggregateKind::Enum),
            _ => None,
        };
        let simple = matches!(
            first,
            "use"
                | "usar"
                | "type"
                | "tipo"
                | "const"
                | "constante"
                | "static"
                | "estatico"
                | "mod"
                | "modulo"
                | "extern"
                | "externo"
        );
        let macro_call = self.is_macro_invocation_start(item_start);
        if aggregate_kind.is_none() && !simple && !macro_call {
            self.position = start;
            return None;
        }
        if aggregate_kind.is_none() {
            let mut end = item_start;
            while end < self.tokens.len()
                && !matches!(
                    self.tokens[end].kind,
                    TokenKind::Newline | TokenKind::Dedent
                )
            {
                end += 1;
            }
            if end == item_start {
                self.position = start;
                return None;
            }
            if matches!(first, "use" | "usar") {
                let mut tree_end = end;
                if self.token_text(&self.tokens[end - 1]) == ";" {
                    tree_end -= 1;
                }
                if item_start + 1 >= tree_end {
                    self.error_current("OX2003", "se esperaba una ruta después de `usar`");
                    self.position = end;
                    self.consume_kind(|kind| matches!(kind, TokenKind::Newline));
                    return None;
                }
                let tree = self.fragment(item_start + 1, tree_end);
                let span = self.source.span(
                    self.tokens[start].span.start(),
                    self.tokens[end - 1].span.end(),
                )?;
                self.check_passthrough_identifier_collisions(start, end);
                self.position = end;
                self.consume_kind(|kind| matches!(kind, TokenKind::Newline));
                return Some(Item::IndentedUse(IndentedUse {
                    attributes,
                    visibility,
                    tree,
                    span,
                }));
            }
            if matches!(
                first,
                "type" | "tipo" | "const" | "constante" | "static" | "estatico"
            ) && self.token_text(&self.tokens[end - 1]) != ";"
            {
                self.error(
                    "OX2003",
                    "los items Rust terminados con ';' deben conservar el terminador en modo indentación",
                    Some(self.tokens[end - 1].span),
                );
            }
            self.check_passthrough_identifier_collisions(start, end);
            let item = self.fragment(start, end);
            self.position = end;
            self.consume_kind(|kind| matches!(kind, TokenKind::Newline));
            return Some(Item::RustFragment(item));
        }

        let mut depth = 0usize;
        let mut colon = None;
        for index in item_start..self.tokens.len() {
            let token = &self.tokens[index];
            if matches!(token.kind, TokenKind::Newline | TokenKind::Dedent) {
                break;
            }
            let text = self.token_text(token);
            if depth == 0 && text == ":" {
                colon = Some(index);
                break;
            }
            match text {
                "(" | "[" | "{" => depth += 1,
                ")" | "]" | "}" => depth = depth.saturating_sub(1),
                _ => {}
            }
        }
        let Some(colon) = colon else {
            self.position = start;
            return None;
        };
        if self.next_significant_text(colon + 1) != Some("\n") {
            self.position = start;
            return None;
        }
        let header = self.fragment(item_start + 1, colon);
        if matches!(
            self.tokens.get(item_start + 1).map(|token| &token.kind),
            Some(TokenKind::Keyword { .. } | TokenKind::Identifier { raw: false })
        ) && (matches!(self.tokens[item_start + 1].kind, TokenKind::Keyword { .. })
            || crate::domain::syntax::prelude_translation(
                self.source.slice(self.tokens[item_start + 1].span),
            )
            .is_some())
        {
            self.report_reserved_identifier(item_start + 1);
        }
        self.position = colon + 1;
        self.skip_comments();
        self.consume_kind(|kind| matches!(kind, TokenKind::Newline));
        self.skip_comments();
        if !self.consume_kind(|kind| matches!(kind, TokenKind::Indent)) {
            self.error_current("OX2003", "se esperaba un cuerpo indentado para struct/enum");
            return None;
        }
        let mut members = Vec::new();
        let mut pending_attributes = Vec::new();
        while !self.at_end() && !self.is_kind(|kind| matches!(kind, TokenKind::Dedent)) {
            self.skip_trivia();
            if self.at_end() || self.is_kind(|kind| matches!(kind, TokenKind::Dedent)) {
                break;
            }
            let member_start = self.position;
            let member_end = self.line_fragment_end(member_start);
            if member_end == member_start {
                self.position += 1;
                continue;
            }
            let fragment = self.fragment(member_start, member_end);
            if self.is_standalone_attribute(member_start, member_end) {
                pending_attributes.push(fragment);
            } else {
                if aggregate_kind == Some(AggregateKind::Enum)
                    && (matches!(self.tokens[member_start].kind, TokenKind::Keyword { .. })
                        || (matches!(
                            self.tokens[member_start].kind,
                            TokenKind::Identifier { raw: false }
                        ) && crate::domain::syntax::prelude_translation(
                            self.source.slice(self.tokens[member_start].span),
                        )
                        .is_some()))
                {
                    self.report_reserved_identifier(member_start);
                }
                members.push(IndentedMember {
                    attributes: std::mem::take(&mut pending_attributes),
                    value: fragment,
                });
            }
            self.position = member_end;
            self.consume_kind(|kind| matches!(kind, TokenKind::Newline));
        }
        let end = self.current().map_or_else(
            || self.tokens.last().map_or(0, |token| token.span.end()),
            |token| token.span.end(),
        );
        self.consume_kind(|kind| matches!(kind, TokenKind::Dedent));
        let span = self.source.span(self.tokens[start].span.start(), end)?;
        self.check_passthrough_identifier_collisions(start, self.position);
        Some(Item::IndentedAggregate(IndentedAggregate {
            kind: aggregate_kind?,
            attributes,
            visibility,
            header,
            members,
            span,
        }))
    }

    fn line_fragment_end(&self, start: usize) -> usize {
        let mut end = start;
        let mut group_depth = 0usize;
        while end < self.tokens.len() {
            let token = &self.tokens[end];
            if group_depth == 0 && matches!(token.kind, TokenKind::Newline | TokenKind::Dedent) {
                break;
            }
            match self.token_text(token) {
                "(" | "[" | "{" => group_depth += 1,
                ")" | "]" | "}" => group_depth = group_depth.saturating_sub(1),
                _ => {}
            }
            end += 1;
        }
        end
    }

    fn passthrough_item_start(&self, start: usize) -> Option<usize> {
        let mut index = start;
        loop {
            while self
                .tokens
                .get(index)
                .is_some_and(|token| matches!(token.kind, TokenKind::Comment | TokenKind::Newline))
            {
                index += 1;
            }
            if self
                .tokens
                .get(index)
                .is_none_or(|token| self.token_text(token) != "#")
            {
                break;
            }
            index += 1;
            if self
                .tokens
                .get(index)
                .is_some_and(|token| self.token_text(token) == "!")
            {
                index += 1;
            }
            if self
                .tokens
                .get(index)
                .is_none_or(|token| self.token_text(token) != "[")
            {
                return None;
            }
            index = self.skip_balanced_tokens(index, "[", "]")?;
        }
        while self
            .tokens
            .get(index)
            .is_some_and(|token| matches!(token.kind, TokenKind::Comment | TokenKind::Newline))
        {
            index += 1;
        }
        if self.tokens.get(index).is_some_and(|token| {
            matches!(
                token.kind,
                TokenKind::Keyword {
                    rust_keyword: "pub",
                    ..
                }
            )
        }) {
            index += 1;
            while self
                .tokens
                .get(index)
                .is_some_and(|token| matches!(token.kind, TokenKind::Comment | TokenKind::Newline))
            {
                index += 1;
            }
            if self
                .tokens
                .get(index)
                .is_some_and(|token| self.token_text(token) == "(")
            {
                index = self.skip_balanced_tokens(index, "(", ")")?;
            }
        }
        Some(index)
    }

    fn skip_balanced_tokens(&self, open_index: usize, open: &str, close: &str) -> Option<usize> {
        let mut depth = 0usize;
        for index in open_index..self.tokens.len() {
            match self.token_text(&self.tokens[index]) {
                text if text == open => depth += 1,
                text if text == close => {
                    depth = depth.checked_sub(1)?;
                    if depth == 0 {
                        return Some(index + 1);
                    }
                }
                _ => {}
            }
        }
        None
    }

    fn is_macro_invocation_start(&self, start: usize) -> bool {
        for token in self.tokens.iter().skip(start + 1) {
            if matches!(token.kind, TokenKind::Newline)
                || matches!(self.token_text(token), ";" | "{" | "}" | "(")
            {
                return false;
            }
            if self.token_text(token) == "!" {
                return true;
            }
        }
        false
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

    fn check_pattern_keyword_identifier(&mut self, start: usize, end: usize) {
        if end <= start {
            return;
        }
        for index in start..end {
            if self.tokens[index].opaque {
                continue;
            }
            let is_identifier_collision =
                !matches!(
                    self.tokens[index].kind,
                    TokenKind::Keyword {
                        rust_keyword: "true" | "false" | "mut" | "ref" | "self" | "Self",
                        ..
                    }
                ) && matches!(self.tokens[index].kind, TokenKind::Keyword { .. });
            let next = self
                .tokens
                .get(index + 1)
                .map(|token| self.token_text(token));
            let is_prelude_binding_collision = matches!(
                self.tokens[index].kind,
                TokenKind::Identifier { raw: false }
            ) && crate::domain::syntax::prelude_translation(
                self.source.slice(self.tokens[index].span),
            )
            .is_some()
                && matches!(next, Some(":" | "="));
            if is_identifier_collision || is_prelude_binding_collision {
                self.report_reserved_identifier(index);
            }
        }
    }

    fn check_signature_patterns(&mut self, start: usize, end: usize) {
        let Some(open) = (start..end).find(|index| self.token_text(&self.tokens[*index]) == "(")
        else {
            return;
        };
        let mut depth = 0usize;
        let mut close = None;
        for index in open..end {
            match self.token_text(&self.tokens[index]) {
                "(" => depth += 1,
                ")" => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        close = Some(index);
                        break;
                    }
                }
                _ => {}
            }
        }
        let Some(close) = close else { return };
        let parameter_start = open + 1;
        let mut segment_start = parameter_start;
        depth = 0;
        for index in parameter_start..=close {
            let text = self.token_text(&self.tokens[index]);
            if matches!(text, "(" | "[" | "{") {
                depth += 1;
            } else if matches!(text, ")" | "]" | "}") {
                depth = depth.saturating_sub(1);
            }
            if (text == "," && depth == 0) || index == close {
                let segment_end = index;
                if let Some(colon) = (segment_start..segment_end)
                    .find(|candidate| self.token_text(&self.tokens[*candidate]) == ":")
                {
                    self.check_pattern_keyword_identifier(segment_start, colon);
                }
                segment_start = index + 1;
            }
        }
    }

    fn check_passthrough_identifier_collisions(&mut self, start: usize, end: usize) {
        for index in start..end {
            if self.tokens[index].opaque {
                continue;
            }
            let is_keyword = matches!(self.tokens[index].kind, TokenKind::Keyword { .. });
            let is_prelude_alias = matches!(
                self.tokens[index].kind,
                TokenKind::Identifier { raw: false }
            ) && crate::domain::syntax::prelude_translation(
                self.source.slice(self.tokens[index].span),
            )
            .is_some();
            if !is_keyword && !is_prelude_alias {
                continue;
            }
            let function_name = index > start && self.is_keyword_at(index - 1, "fn");
            let declared_item_name = index > start
                && ["struct", "enum", "trait", "type", "mod", "const", "static"]
                    .iter()
                    .any(|keyword| self.is_keyword_at(index - 1, keyword));
            let field_name = self
                .tokens
                .get(index + 1)
                .is_some_and(|token| self.token_text(token) == ":");
            if function_name || declared_item_name || field_name {
                self.report_reserved_identifier(index);
            }
        }
    }

    fn is_standalone_attribute(&self, start: usize, end: usize) -> bool {
        if self
            .tokens
            .get(start)
            .is_none_or(|token| self.token_text(token) != "#")
        {
            return false;
        }
        let mut bracket = start + 1;
        if self
            .tokens
            .get(bracket)
            .is_some_and(|token| self.token_text(token) == "!")
        {
            bracket += 1;
        }
        self.skip_balanced_tokens(bracket, "[", "]") == Some(end)
    }

    fn report_reserved_identifier(&mut self, token_index: usize) {
        let Some(token) = self.tokens.get(token_index) else {
            return;
        };
        let prelude_alias = matches!(token.kind, TokenKind::Identifier { raw: false })
            && crate::domain::syntax::prelude_translation(self.source.slice(token.span)).is_some();
        if !matches!(token.kind, TokenKind::Keyword { .. }) && !prelude_alias {
            return;
        }
        let span = token.span;
        let spelling = self.source.slice(span).to_string();
        let raw_identifier_allowed =
            !matches!(spelling.as_str(), "crate" | "self" | "Self" | "super");
        let message = if raw_identifier_allowed {
            format!(
                "`{spelling}` es palabra reservada de Óxido; usa `r#{spelling}` o cambia el nombre"
            )
        } else {
            format!("`{spelling}` es palabra reservada de Rust; cambia el nombre")
        };
        let mut diagnostic = Diagnostic::error("OX3001", message, span);
        if raw_identifier_allowed {
            diagnostic.suggestions.push(DiagnosticSuggestion {
                span,
                replacement: format!("r#{spelling}"),
                message: "escapar el identificador".into(),
            });
        }
        self.diagnostics.push(diagnostic);
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
        self.is_keyword_at(self.position, rust)
    }
    fn is_keyword_at(&self, index: usize, rust: &str) -> bool {
        matches!(self.tokens.get(index).map(|token| &token.kind), Some(TokenKind::Keyword { rust_keyword, .. }) if *rust_keyword == rust)
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
