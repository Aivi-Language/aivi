impl Parser {
    fn parse_type_expr(&mut self) -> Option<TypeExpr> {
        let lhs = self.parse_type_and()?;
        // Allow `->` on a continuation line so multi-line type signatures work:
        //   foo : A
        //     -> B
        //     -> C
        let checkpoint = self.pos;
        self.consume_newlines();
        if self.consume_symbol("->") {
            let result = self.parse_type_expr().unwrap_or(TypeExpr::Unknown {
                span: type_span(&lhs),
            });
            let span = merge_span(type_span(&lhs), type_span(&result));
            return Some(TypeExpr::Func {
                params: vec![lhs],
                result: Box::new(result),
                span,
            });
        }
        self.pos = checkpoint;
        Some(lhs)
    }

    fn parse_type_and(&mut self) -> Option<TypeExpr> {
        let mut items = vec![self.parse_type_pipe()?];
        while self.consume_ident_text("with").is_some() {
            let rhs = self.parse_type_pipe().unwrap_or(TypeExpr::Unknown {
                span: type_span(items.last().expect("infallible")),
            });
            items.push(rhs);
        }
        if items.len() == 1 {
            return Some(items.remove(0));
        }
        let span = merge_span(type_span(&items[0]), type_span(items.last().expect("infallible")));
        Some(TypeExpr::And { items, span })
    }

    fn parse_type_pipe(&mut self) -> Option<TypeExpr> {
        let mut lhs = self.parse_type_apply()?;
        while self.consume_symbol("|>") {
            let rhs = self.parse_type_apply().unwrap_or(TypeExpr::Unknown {
                span: type_span(&lhs),
            });
            lhs = self.apply_type_pipe(lhs, rhs);
        }
        Some(lhs)
    }

    fn apply_type_pipe(&mut self, left: TypeExpr, right: TypeExpr) -> TypeExpr {
        let span = merge_span(type_span(&left), type_span(&right));
        match right {
            TypeExpr::Apply { base, mut args, .. } => {
                args.push(left);
                TypeExpr::Apply { base, args, span }
            }
            other => TypeExpr::Apply {
                base: Box::new(other),
                args: vec![left],
                span,
            },
        }
    }

    fn parse_type_apply(&mut self) -> Option<TypeExpr> {
        let base = self.parse_type_atom()?;
        let mut args = Vec::new();
        while let Some(arg) = self.parse_type_atom() {
            args.push(arg);
        }
        if args.is_empty() {
            return Some(base);
        }
        let span = merge_span(type_span(&base), type_span(args.last().expect("infallible")));
        Some(TypeExpr::Apply {
            base: Box::new(base),
            args,
            span,
        })
    }

    fn parse_type_atom(&mut self) -> Option<TypeExpr> {
        if self.consume_symbol("(") {
            let open_span = self.previous_span();
            let mut items = Vec::new();
            if let Some(item) = self.parse_type_expr() {
                items.push(item);
                while self.consume_symbol(",") {
                    if let Some(item) = self.parse_type_expr() {
                        items.push(item);
                    }
                }
            }
            let close_span = self
                .expect_symbol(")", "expected ')' to close type tuple")
                .unwrap_or_else(|| open_span.clone());
            if items.len() == 1 {
                return Some(items.remove(0));
            }
            let start = items.first().map(type_span).unwrap_or_else(|| open_span.clone());
            let end = items.last().map(type_span).unwrap_or(close_span);
            let span = merge_span(start, end);
            return Some(TypeExpr::Tuple { items, span });
        }
        if self.consume_symbol("{") {
            let mut fields = Vec::new();
            self.consume_newlines();
            while !self.check_symbol("}") && self.pos < self.tokens.len() {
                self.consume_newlines();
                if self.check_symbol("}") {
                    break;
                }
                if let Some(name) = self.consume_ident() {
                    self.consume_newlines();
                    self.expect_symbol(":", "expected ':' in record type");
                    self.consume_newlines();
                    if let Some(ty) = self.parse_type_expr() {
                        fields.push((name, ty));
                    }
                } else {
                    // Recovery: skip unexpected tokens inside record types.
                    self.pos += 1;
                    continue;
                }
                self.consume_newlines();
                if self.consume_symbol(",") {
                    self.consume_newlines();
                    continue;
                }
                // Newline-separated fields are allowed (FieldSep includes Sep).
                if self.check_symbol("}") {
                    break;
                }
            }
            self.expect_symbol("}", "expected '}' to close record type");
            let span = fields
                .first()
                .map(|field| field.0.span.clone())
                .unwrap_or(self.previous_span());
            return Some(TypeExpr::Record { fields, span });
        }
        if self.consume_symbol("*") {
            let span = self.previous_span();
            return Some(TypeExpr::Star { span });
        }
        if let Some(name) = self.parse_dotted_name() {
            if name.name == "with" {
                // `with` is reserved in type position (composition operator).
                self.pos -= 1;
                return None;
            }
            return Some(TypeExpr::Name(name));
        }
        None
    }

    fn try_parse_datetime(&mut self, head: Token) -> Option<Literal> {
        let checkpoint = self.pos;
        let result = (|| {
            if !self.consume_symbol("-") {
                return None;
            }
            let month = self.consume_number()?;
            if !self.consume_symbol("-") {
                return None;
            }
            let day = self.consume_number()?;
            let t_token = self.consume_ident()?;
            if !t_token.name.starts_with('T') {
                return None;
            }
            let hour_text = t_token.name.trim_start_matches('T');
            let hour = if hour_text.is_empty() {
                self.consume_number()?
            } else {
                Token {
                    kind: TokenKind::Number,
                    text: hour_text.to_string(),
                    span: t_token.span.clone(),
                }
            };
            if !self.consume_symbol(":") {
                return None;
            }
            let minute = self.consume_number()?;
            if !self.consume_symbol(":") {
                return None;
            }
            let second = self.consume_number()?;
            self.consume_ident_text("Z")?;

            let text = format!(
                "{}-{}-{}T{}:{}:{}Z",
                head.text, month.text, day.text, hour.text, minute.text, second.text
            );
            let span = merge_span(head.span.clone(), second.span.clone());
            Some(Literal::DateTime { text, span })
        })();

        if result.is_none() {
            self.pos = checkpoint;
        }
        result
    }

    fn parse_dotted_name(&mut self) -> Option<SpannedName> {
        let mut name = self.consume_ident()?;
        let mut text = name.name.as_str().to_string();
        while self.consume_symbol(".") {
            if let Some(part) = self.consume_ident() {
                text.push('.');
                text.push_str(part.name.as_str());
                name.span = merge_span(name.span.clone(), part.span.clone());
            } else {
                break;
            }
        }
        name.name = text;
        Some(name)
    }

    fn consume_ident_text(&mut self, expected: &str) -> Option<SpannedName> {
        let name = self.consume_ident()?;
        if name.name == expected {
            return Some(name);
        }
        self.pos -= 1;
        None
    }

    fn consume_name(&mut self) -> Option<SpannedName> {
        self.consume_newlines();
        if let Some(name) = self.consume_ident() {
            return Some(name);
        }
        if self.consume_symbol("(") {
            let op_token = self.consume_symbol_token()?;
            let end = self.expect_symbol(")", "expected ')' after operator name");
            let span = merge_span(op_token.span.clone(), end.unwrap_or(op_token.span.clone()));
            return Some(SpannedName {
                name: format!("({})", op_token.text),
                span,
            });
        }
        None
    }

    fn consume_ident(&mut self) -> Option<SpannedName> {
        let token = self.tokens.get(self.pos)?;
        if token.kind != TokenKind::Ident {
            return None;
        }
        self.pos += 1;
        Some(SpannedName {
            name: token.text.as_str().into(),
            span: token.span.clone(),
        })
    }

    fn consume_number(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.pos)?;
        if token.kind != TokenKind::Number {
            return None;
        }
        self.pos += 1;
        Some(token.clone())
    }

    fn consume_string(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.pos)?;
        if token.kind != TokenKind::String {
            return None;
        }
        self.pos += 1;
        Some(token.clone())
    }

    fn consume_sigil(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.pos)?;
        if token.kind != TokenKind::Sigil {
            return None;
        }
        self.pos += 1;
        Some(token.clone())
    }

    fn parse_text_literal_plain(&mut self, token: Token) -> Literal {
        let span = token.span.clone();
        Literal::String {
            text: decode_text_literal(&token.text).unwrap_or_else(|| token.text.clone()),
            span,
        }
    }

    fn parse_text_literal_expr(&mut self, token: Token) -> Expr {
        let span = token.span.clone();
        let Some(inner) = strip_text_literal_quotes(&token.text) else {
            return Expr::Literal(Literal::String {
                text: token.text,
                span,
            });
        };

        let raw_chars: Vec<char> = inner.chars().collect();
        let mut parts: Vec<TextPart> = Vec::new();

        let mut text_buf = String::new();
        let mut text_start = 0usize;
        let mut i = 0usize;

        while i < raw_chars.len() {
            let ch = raw_chars[i];
            if ch == '\\' {
                if i + 1 >= raw_chars.len() {
                    self.emit_diag(
                        "E1520",
                        "unterminated escape sequence in text literal",
                        span.clone(),
                    );
                    text_buf.push('\\');
                    i += 1;
                    continue;
                }
                let esc = raw_chars[i + 1];
                match decode_escape(esc) {
                    Some(decoded) => text_buf.push(decoded),
                    None => {
                        let esc_span = span_in_text_literal(&token.span, i, i + 2);
                        self.emit_diag(
                            "E1521",
                            &format!("unknown escape sequence '\\{esc}'"),
                            esc_span,
                        );
                        text_buf.push(esc);
                    }
                }
                i += 2;
                continue;
            }

            if ch == '{' {
                if !text_buf.is_empty() {
                    let part_span = span_in_text_literal(&token.span, text_start, i);
                    parts.push(TextPart::Text {
                        text: std::mem::take(&mut text_buf),
                        span: part_span,
                    });
                }

                let open_index = i;
                let remainder: String = raw_chars[i + 1..].iter().collect();
                let Some(close_offset) = find_interpolation_close(&remainder) else {
                    let open_span = span_in_text_literal(&token.span, open_index, open_index + 1);
                    self.emit_diag("E1522", "unterminated text interpolation", open_span);
                    text_buf.push('{');
                    text_start = i;
                    i += 1;
                    continue;
                };

                let close_index = i + 1 + close_offset;
                let expr_raw: String = raw_chars[i + 1..close_index].iter().collect();
                let (expr_decoded, expr_raw_map) = decode_interpolation_source_with_map(&expr_raw);
                let expr_start_col = token.span.start.column + 1 + open_index + 1; // opening quote + '{'
                let expr_line = token.span.start.line;

                match self.parse_embedded_expr(
                    &expr_decoded,
                    &expr_raw_map,
                    expr_line,
                    expr_start_col,
                ) {
                    Some(expr) => {
                        let part_span =
                            span_in_text_literal(&token.span, open_index, close_index + 1);
                        parts.push(TextPart::Expr {
                            expr: Box::new(expr),
                            span: part_span,
                        });
                    }
                    None => {
                        let part_span =
                            span_in_text_literal(&token.span, open_index, close_index + 1);
                        parts.push(TextPart::Text {
                            text: format!("{{{expr_raw}}}"),
                            span: part_span,
                        });
                    }
                }

                i = close_index + 1;
                text_start = i;
                continue;
            }

            text_buf.push(ch);
            i += 1;
        }

        if !text_buf.is_empty() {
            let part_span = span_in_text_literal(&token.span, text_start, raw_chars.len());
            parts.push(TextPart::Text {
                text: text_buf,
                span: part_span,
            });
        }

        let has_expr = parts
            .iter()
            .any(|part| matches!(part, TextPart::Expr { .. }));
        if !has_expr {
            let mut out = String::new();
            for part in parts {
                if let TextPart::Text { text, .. } = part {
                    out.push_str(&text);
                }
            }
            return Expr::Literal(Literal::String { text: out, span });
        }

        Expr::TextInterpolate { parts, span }
    }

    fn parse_embedded_expr(
        &mut self,
        text: &str,
        raw_map: &[usize],
        line: usize,
        column: usize,
    ) -> Option<Expr> {
        let (cst_tokens, lex_diags) = lex(text);
        for diag in lex_diags {
            let mapped_span = map_span_columns(&diag.span, raw_map);
            self.diagnostics.push(FileDiagnostic {
                path: self.path.clone(),
                diagnostic: Diagnostic {
                    code: diag.code,
                    severity: diag.severity,
                    message: diag.message,
                    span: shift_span(&mapped_span, line - 1, column - 1),
                    labels: diag
                        .labels
                        .into_iter()
                        .map(|label| DiagnosticLabel {
                            message: label.message,
                            span: shift_span(
                                &map_span_columns(&label.span, raw_map),
                                line - 1,
                                column - 1,
                            ),
                        })
                        .collect(),
                },
            });
        }
        let mut tokens = filter_tokens(&cst_tokens);
        for token in &mut tokens {
            let mapped_span = map_span_columns(&token.span, raw_map);
            token.span = shift_span(&mapped_span, line - 1, column - 1);
        }

        let mut parser = Parser::new(tokens, Path::new(&self.path));
        let expr = parser.parse_expr();
        parser.consume_newlines();
        if parser.pos < parser.tokens.len() {
            let span = parser.peek_span().unwrap_or_else(|| parser.previous_span());
            parser.emit_diag("E1523", "unexpected tokens in text interpolation", span);
        }
        self.diagnostics.append(&mut parser.diagnostics);
        expr
    }

    fn consume_number_suffix(&mut self, number: Token, prefix: Option<Span>) -> (String, Span) {
        let mut text = number.text.clone();
        let mut span = number.span.clone();
        if let Some(prefix_span) = prefix {
            text = format!("-{text}");
            span = merge_span(prefix_span, span);
        }
        if let Some(suffix) = self.consume_adjacent_suffix(&number.span) {
            text.push_str(&suffix.text);
            span = merge_span(span, suffix.span);
        }
        (text, span)
    }

    fn consume_adjacent_suffix(&mut self, number_span: &Span) -> Option<Token> {
        let token = self.tokens.get(self.pos)?;
        if !is_adjacent(number_span, &token.span) {
            return None;
        }
        if token.kind == TokenKind::Ident || (token.kind == TokenKind::Symbol && token.text == "%")
        {
            self.pos += 1;
            return Some(token.clone());
        }
        None
    }

    fn consume_symbol_token(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.pos)?;
        if token.kind != TokenKind::Symbol || token.text == ")" {
            return None;
        }
        self.pos += 1;
        Some(token.clone())
    }

    fn consume_newlines(&mut self) {
        while self.peek_newline() {
            self.pos += 1;
        }
    }

    fn peek_newline(&self) -> bool {
        matches!(
            self.tokens.get(self.pos).map(|token| &token.kind),
            Some(TokenKind::Newline)
        )
    }

    fn peek_symbol_text(&self) -> Option<String> {
        let token = self.tokens.get(self.pos)?;
        if token.kind != TokenKind::Symbol {
            return None;
        }
        Some(token.text.clone())
    }

    fn consume_symbol(&mut self, symbol: &str) -> bool {
        let token = match self.tokens.get(self.pos) {
            Some(token) => token,
            None => return false,
        };
        if token.kind == TokenKind::Symbol && token.text == symbol {
            self.pos += 1;
            return true;
        }
        false
    }

    fn match_keyword(&mut self, keyword: &str) -> bool {
        if let Some(token) = self.tokens.get(self.pos) {
            if token.kind == TokenKind::Ident && token.text == keyword {
                self.pos += 1;
                return true;
            }
        }
        false
    }

    fn expect_keyword(&mut self, keyword: &str, message: &str) {
        if !self.match_keyword(keyword) {
            let span = self.peek_span().unwrap_or_else(|| self.previous_span());
            self.emit_diag("E1500", message, span);
        }
    }

    fn expect_symbol(&mut self, symbol: &str, message: &str) -> Option<Span> {
        if self.consume_symbol(symbol) {
            return Some(self.previous_span());
        }
        let span = self.peek_span().unwrap_or_else(|| self.previous_span());
        self.emit_diag("E1501", message, span.clone());
        None
    }

    fn check_symbol(&self, symbol: &str) -> bool {
        self.peek_symbol(symbol)
    }

    fn peek_symbol(&self, symbol: &str) -> bool {
        if let Some(token) = self.tokens.get(self.pos) {
            return token.kind == TokenKind::Symbol && token.text == symbol;
        }
        false
    }

    fn previous_span(&self) -> Span {
        if self.pos == 0 {
            return Span {
                start: Position { line: 1, column: 1 },
                end: Position { line: 1, column: 1 },
            };
        }
        if self.pos > self.tokens.len() {
            return self.tokens.last().map(|t| t.span.clone()).unwrap_or(Span {
                start: Position { line: 1, column: 1 },
                end: Position { line: 1, column: 1 },
            });
        }
        if self.pos >= self.tokens.len() {
            return self.tokens.last().map(|t| t.span.clone()).unwrap_or(Span {
                start: Position { line: 1, column: 1 },
                end: Position { line: 1, column: 1 },
            });
        }
        self.tokens[self.pos - 1].span.clone()
    }

    fn peek_span(&self) -> Option<Span> {
        self.tokens.get(self.pos).map(|token| token.span.clone())
    }

    fn emit_diag(&mut self, code: &str, message: &str, span: Span) {
        // Deprecation codes are warnings, not errors
        let severity = if code.starts_with('W') {
            DiagnosticSeverity::Warning
        } else {
            DiagnosticSeverity::Error
        };
        self.diagnostics.push(FileDiagnostic {
            path: self.path.clone(),
            diagnostic: Diagnostic {
                code: code.to_string(),
                severity,
                message: message.to_string(),
                span,
                labels: Vec::new(),
            },
        });
    }

    fn recover_to_item(&mut self) {
        let start = self.pos;
        while self.pos < self.tokens.len() {
            if self.peek_symbol("}") {
                break;
            }
            if self.peek_keyword("export")
                || self.peek_keyword("use")
                || self.peek_keyword("class")
                || self.peek_keyword("instance")
                || self.peek_keyword("domain")
            {
                break;
            }
            // Stop at an identifier at column 1 preceded by a newline — this is
            // likely the start of a new top-level definition (e.g. `name = ...`
            // or `TypeName`).
            if let Some(tok) = self.tokens.get(self.pos) {
                if tok.kind == TokenKind::Ident
                    && tok.span.start.column == 1
                    && self.pos > start
                    && self
                        .tokens
                        .get(self.pos.wrapping_sub(1))
                        .is_some_and(|prev| prev.kind == TokenKind::Newline)
                {
                    break;
                }
            }
            self.pos += 1;
        }
        // Always advance at least one token to prevent caller loops
        if self.pos == start && self.pos < self.tokens.len() {
            self.pos += 1;
        }
    }

    fn recover_to_module(&mut self) {
        while self.pos < self.tokens.len() {
            if self.peek_keyword("module") {
                break;
            }
            self.pos += 1;
        }
    }

    fn peek_keyword(&self, keyword: &str) -> bool {
        if let Some(token) = self.tokens.get(self.pos) {
            return token.kind == TokenKind::Ident && token.text == keyword;
        }
        false
    }
}
