impl Parser {
    fn is_negative_number_literal_start(&self) -> bool {
        let Some(token) = self.tokens.get(self.pos) else {
            return false;
        };
        if token.kind != TokenKind::Symbol || token.text != "-" {
            return false;
        }
        let Some(next) = self.tokens.get(self.pos + 1) else {
            return false;
        };
        next.kind == TokenKind::Number && is_adjacent(&token.span, &next.span)
    }

    fn is_expr_start(&self) -> bool {
        if let Some(token) = self.tokens.get(self.pos) {
            match token.kind {
                TokenKind::Ident => {
                    if crate::syntax::KEYWORDS_ALL.contains(&token.text.as_str()) {
                        return matches!(
                            token.text.as_str(),
                            "if" | "do" | "generate" | "resource" | "patch"
                        );
                    }
                    return true;
                }
                TokenKind::Number | TokenKind::String | TokenKind::Sigil => return true,
                TokenKind::Symbol => {
                    // Note: `-` is intentionally *not* an expression-start token. Negative
                    // numeric literals are handled in `parse_primary` (as a prefix on numbers),
                    // while `-` in the general case is an infix operator.
                    return matches!(token.text.as_str(), "(" | "[" | "{" | ".");
                }
                TokenKind::Newline => return false,
            }
        }
        false
    }

    fn is_record_field_start(&self) -> bool {
        let Some(token) = self.tokens.get(self.pos) else {
            return false;
        };
        match token.kind {
            TokenKind::Ident => token
                .text
                .chars()
                .next()
                .is_some_and(|ch| ch.is_ascii_lowercase()),
            TokenKind::Symbol => token.text == "...",
            _ => false,
        }
    }

    fn is_pattern_start(&self) -> bool {
        if let Some(token) = self.tokens.get(self.pos) {
            match token.kind {
                TokenKind::Ident | TokenKind::Number | TokenKind::String | TokenKind::Sigil => {
                    return true
                }
                TokenKind::Symbol => {
                    if matches!(token.text.as_str(), "(" | "[" | "{" | "~") {
                        return true;
                    }
                    if token.text == "-" {
                        return self
                            .tokens
                            .get(self.pos + 1)
                            .is_some_and(|next| next.kind == TokenKind::Number);
                    }
                    return false;
                }
                TokenKind::Newline => return false,
            }
        }
        false
    }

    fn parse_primary(&mut self) -> Option<Expr> {
        if self.consume_symbol("!") {
            let bang_span = self.previous_span();
            let rhs = self.parse_primary()?;
            let true_lit = Expr::Literal(Literal::Bool {
                value: true,
                span: bang_span.clone(),
            });
            let false_lit = Expr::Literal(Literal::Bool {
                value: false,
                span: bang_span.clone(),
            });
            let span = merge_span(bang_span, expr_span(&rhs));
            // `!p` desugars to `if p then False else True`.
            return Some(Expr::If {
                cond: Box::new(rhs),
                then_branch: Box::new(false_lit),
                else_branch: Box::new(true_lit),
                span,
            });
        }
        if self.peek_symbol("-") {
            let checkpoint = self.pos;
            self.consume_symbol("-");
            let minus_span = self.previous_span();

            // Negative numeric literal.
            if let Some(number) = self.consume_number() {
                let (text, span) = self.consume_number_suffix(number, Some(minus_span));
                return Some(Expr::Literal(Literal::Number { text, span }));
            }

            // Unary negation: desugar as `0 - expr` at the surface layer so downstream stages
            // only need to handle the binary operator.
            let rhs = match self.parse_primary() {
                Some(expr) => expr,
                None => {
                    self.pos = checkpoint;
                    return None;
                }
            };
            let span = merge_span(minus_span, expr_span(&rhs));
            return Some(Expr::UnaryNeg {
                expr: Box::new(rhs),
                span,
            });
        }
        if let Some(expr) = self.parse_structured_sigil() {
            return Some(expr);
        }
        if self.consume_symbol("(") {
            let open_span = self.previous_span();
            if self.consume_symbol(")") {
                let span = self.previous_span();
                return Some(Expr::Tuple {
                    items: Vec::new(),
                    span,
                });
            }
            let expr = self.parse_expr()?;
            if self.consume_symbol(",") {
                let mut items = vec![expr];
                self.consume_newlines();
                while !self.check_symbol(")") && self.pos < self.tokens.len() {
                    if let Some(item) = self.parse_expr() {
                        items.push(item);
                    }
                    self.consume_newlines();
                    if !self.consume_symbol(",") {
                        break;
                    }
                    self.consume_newlines();
                }
                self.consume_newlines();
                let end = self.expect_symbol(")", "expected ')' to close tuple");
                let span = merge_span(expr_span(&items[0]), end.unwrap_or(expr_span(&items[0])));
                return Some(Expr::Tuple { items, span });
            }
            let close_span = self
                .expect_symbol(")", "expected ')' to close group")
                .unwrap_or_else(|| expr_span(&expr));

            // Postfix suffix application: `(expr)suffix` where `suffix` is adjacent to `)`.
            // This keeps `xkg` from becoming ambiguous with identifiers, while allowing
            // variables/expressions as the numeric part.
            if let Some(suffix_token) = self.consume_adjacent_suffix(&close_span) {
                let suffix = SpannedName {
                    name: suffix_token.text,
                    span: suffix_token.span,
                };
                let span = merge_span(open_span, suffix.span.clone());
                return Some(Expr::Suffixed {
                    base: Box::new(expr),
                    suffix,
                    span,
                });
            }

            return Some(expr);
        }

        if self.consume_symbol("[") {
            let mut items = Vec::new();
            self.consume_newlines();
            while !self.check_symbol("]") && self.pos < self.tokens.len() {
                let spread = self.consume_symbol("...");
                if let Some(expr) = self.parse_expr() {
                    let span = expr_span(&expr);
                    items.push(ListItem { expr, spread, span });
                }
                let had_newline = self.peek_newline();
                self.consume_newlines();
                if self.consume_symbol(",") {
                    self.consume_newlines();
                    continue;
                }
                if self.check_symbol("]") {
                    break;
                }
                if self.is_expr_start() {
                    if !had_newline {
                        let span = self.peek_span().unwrap_or_else(|| self.previous_span());
                        self.emit_diag("E1524", "expected ',' between list items", span);
                    }
                    continue;
                }
                break;
            }
            let end = self.expect_symbol("]", "expected ']' to close list");
            let span = merge_span(
                items
                    .first()
                    .map(|item| item.span.clone())
                    .unwrap_or(self.previous_span()),
                end.unwrap_or(self.previous_span()),
            );
            return Some(Expr::List { items, span });
        }

        if self.peek_symbol("{") {
            let checkpoint = self.pos;
            let diag_checkpoint = self.diagnostics.len();
            self.pos += 1;
            self.consume_newlines();
            let is_record = self.parse_record_field().is_some();
            self.pos = checkpoint;
            self.diagnostics.truncate(diag_checkpoint);

            if is_record {
                self.consume_symbol("{");
                let mut fields = Vec::new();
                self.consume_newlines();
                while !self.check_symbol("}") && self.pos < self.tokens.len() {
                    if let Some(field) = self.parse_record_field() {
                        fields.push(field);
                        let had_newline = self.peek_newline();
                        self.consume_newlines();
                        if self.consume_symbol(",") {
                            self.consume_newlines();
                            continue;
                        }
                        if self.check_symbol("}") {
                            break;
                        }
                        if self.is_record_field_start() {
                            if !had_newline {
                                let span = self.peek_span().unwrap_or_else(|| self.previous_span());
                                self.emit_diag("E1525", "expected ',' between record fields", span);
                            }
                            continue;
                        }
                        continue;
                    }
                    // Emit diagnostic for unexpected token inside record literal.
                    let skip_span = self.peek_span().unwrap_or_else(|| self.previous_span());
                    let skip_text = self.tokens.get(self.pos).map(|t| t.text.as_str()).unwrap_or("?");
                    // Check if this is `name = value` (common mistake: should be `name: value`).
                    if self.pos + 1 < self.tokens.len()
                        && self.tokens[self.pos].kind == TokenKind::Ident
                        && self.tokens.get(self.pos + 1).is_some_and(|t| t.text == "=")
                    {
                        let field_name = &self.tokens[self.pos].text;
                        self.emit_diag(
                            "E1526",
                            &format!(
                                "invalid record field syntax: use ':' instead of '=' (write `{field_name}: value`)"
                            ),
                            skip_span,
                        );
                    } else {
                        self.emit_diag(
                            "E1527",
                            &format!("unexpected token '{skip_text}' in record literal"),
                            skip_span,
                        );
                    }
                    self.pos += 1;
                }
                let end = self.expect_symbol("}", "expected '}' to close record");
                let span = merge_span(
                    fields
                        .first()
                        .map(|field| field.span.clone())
                        .unwrap_or(self.previous_span()),
                    end.unwrap_or(self.previous_span()),
                );
                return Some(Expr::Record { fields, span });
            }

            return Some(self.parse_block(BlockKind::Plain));
        }

        if self.consume_symbol(".") {
            if let Some(field) = self.consume_ident() {
                let span = merge_span(field.span.clone(), field.span.clone());
                return Some(Expr::FieldSection { field, span });
            }
        }

        if self.match_keyword("if") {
            let cond = self.parse_expr()?;
            self.consume_newlines();
            self.expect_keyword("then", "expected 'then' in if expression");
            let then_branch = self.parse_expr()?;
            self.consume_newlines();
            self.expect_keyword("else", "expected 'else' in if expression");
            let else_branch = self.parse_expr()?;
            let span = merge_span(expr_span(&cond), expr_span(&else_branch));
            return Some(Expr::If {
                cond: Box::new(cond),
                then_branch: Box::new(then_branch),
                else_branch: Box::new(else_branch),
                span,
            });
        }

        if self.match_keyword("do") {
            let monad = self.consume_ident().unwrap_or(SpannedName {
                name: "Effect".to_string(),
                span: self.previous_span(),
            });
            return Some(self.parse_block(BlockKind::Do { monad }));
        }
        if self.match_keyword("effect") {
            // Legacy: accept `effect {` with a deprecation diagnostic
            let effect_span = self.previous_span();
            self.emit_diag(
                "W1600",
                "`effect { }` is deprecated; use `do Effect { }` instead",
                effect_span.clone(),
            );
            let monad = SpannedName {
                name: "Effect".to_string(),
                span: effect_span,
            };
            return Some(self.parse_block(BlockKind::Do { monad }));
        }
        if self.match_keyword("generate") {
            return Some(self.parse_block(BlockKind::Generate));
        }
        if self.match_keyword("resource") {
            return Some(self.parse_block(BlockKind::Resource));
        }
        if self.match_keyword("patch") {
            let start = self.previous_span();
            return Some(self.parse_patch_literal(start));
        }

        if let Some(number) = self.consume_number() {
            if let Some(dt) = self.try_parse_datetime(number.clone()) {
                return Some(Expr::Literal(dt));
            }
            let (text, span) = self.consume_number_suffix(number, None);
            return Some(Expr::Literal(Literal::Number { text, span }));
        }

        if let Some(string) = self.consume_string() {
            return Some(self.parse_text_literal_expr(string));
        }

        if let Some(sigil) = self.consume_sigil() {
            let span = sigil.span.clone();
            if let Some(body) = parse_html_angle_sigil_text(&sigil.text) {
                return Some(self.parse_html_sigil(&sigil, &body));
            }
            if let Some((tag, body, flags)) = parse_sigil_text(&sigil.text) {
                if tag == "html" && flags.is_empty() {
                    // Legacy HTML sigil syntax (~html~> ... <~html). Keep parsing for recovery,
                    // but emit an error so compilation fails and users migrate to ~<html>.
                    self.emit_diag(
                        "E1602",
                        "legacy html sigil syntax; use `~<html>...</html>`",
                        span.clone(),
                    );
                    return Some(self.parse_html_sigil(&sigil, &body));
                }
                if (tag == "u" || tag == "url") && !is_probably_url(&body) {
                    self.emit_diag("E1510", "invalid url sigil", span.clone());
                }
                if (tag == "t" || tag == "dt") && !is_probably_datetime(&body) {
                    self.emit_diag("E1511", "invalid datetime sigil", span.clone());
                }
                if tag == "d" && !is_probably_date(&body) {
                    self.emit_diag("E1512", "invalid date sigil", span.clone());
                }
                if tag == "k" {
                    if let Err(msg) = crate::i18n::validate_key_text(&body) {
                        self.emit_diag(
                            "E1514",
                            &format!("invalid i18n key sigil: {msg}"),
                            span.clone(),
                        );
                    }
                }
                if tag == "m" {
                    if let Err(msg) = crate::i18n::parse_message_template(&body) {
                        self.emit_diag(
                            "E1515",
                            &format!("invalid i18n message sigil: {msg}"),
                            span.clone(),
                        );
                    }
                }
                return Some(Expr::Literal(Literal::Sigil {
                    tag,
                    body,
                    flags,
                    span,
                }));
            }
            self.emit_diag("E1513", "invalid sigil literal", span.clone());
            return Some(Expr::Literal(Literal::Sigil {
                tag: "?".to_string(),
                body: sigil.text,
                flags: String::new(),
                span,
            }));
        }

        if let Some(ident) = self.consume_ident() {
            if ident.name == "True" || ident.name == "False" {
                let value = ident.name == "True";
                return Some(Expr::Literal(Literal::Bool {
                    value,
                    span: ident.span.clone(),
                }));
            }
            return Some(Expr::Ident(ident));
        }

        None
    }
}
