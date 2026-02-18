impl Parser {
    fn parse_pattern(&mut self) -> Option<Pattern> {
        if self.consume_symbol("-") {
            let minus_span = self.previous_span();
            if let Some(number) = self.consume_number() {
                let span = merge_span(minus_span, number.span.clone());
                return Some(Pattern::Literal(Literal::Number {
                    text: format!("-{}", number.text),
                    span,
                }));
            }
        }
        if let Some(ident) = self.consume_ident() {
            if crate::syntax::KEYWORDS_ALL.contains(&ident.name.as_str()) {
                // Keywords can't appear as patterns. In particular, `when` must terminate
                // constructor-arg parsing so match guards work.
                self.pos -= 1;
                return None;
            }
            if ident.name == "_" {
                return Some(Pattern::Wildcard(ident.span));
            }
            if ident
                .name
                .chars()
                .next()
                .map(|c| c.is_uppercase())
                .unwrap_or(false)
            {
                let mut args = Vec::new();
                while let Some(pattern) = self.parse_pattern() {
                    args.push(pattern);
                }
                let span = merge_span(
                    ident.span.clone(),
                    args.last().map(pattern_span).unwrap_or(ident.span.clone()),
                );
                return Some(Pattern::Constructor {
                    name: ident,
                    args,
                    span,
                });
            }
            let subject = false; // `!` deconstructor head removed (Change 4)
            if self.match_keyword("as") || self.consume_symbol("@") {
                if self.tokens.get(self.pos - 1).is_some_and(|t| t.text == "@") {
                    self.emit_diag(
                        "E1603",
                        "`@` in destructuring is deprecated; use `as` instead",
                        self.previous_span(),
                    );
                }
                let at_span = self.previous_span();
                let inner = self
                    .parse_pattern()
                    .unwrap_or(Pattern::Wildcard(at_span.clone()));
                let span = merge_span(ident.span.clone(), pattern_span(&inner));
                return Some(Pattern::At {
                    name: ident,
                    pattern: Box::new(inner),
                    subject,
                    span,
                });
            }
            if subject {
                return Some(Pattern::SubjectIdent(ident));
            }
            return Some(Pattern::Ident(ident));
        }
        if self.consume_symbol("(") {
            if self.consume_symbol(")") {
                return Some(Pattern::Tuple {
                    items: Vec::new(),
                    span: self.previous_span(),
                });
            }
            let mut items = Vec::new();
            if let Some(pattern) = self.parse_pattern() {
                items.push(pattern);
            }
            if self.consume_symbol(",") {
                self.consume_newlines();
                while !self.check_symbol(")") && self.pos < self.tokens.len() {
                    if let Some(pattern) = self.parse_pattern() {
                        items.push(pattern);
                    }
                    self.consume_newlines();
                    if !self.consume_symbol(",") {
                        break;
                    }
                    self.consume_newlines();
                }
                self.consume_newlines();
                let end = self.expect_symbol(")", "expected ')' to close tuple pattern");
                let span = merge_span(
                    pattern_span(&items[0]),
                    end.unwrap_or(pattern_span(&items[0])),
                );
                return Some(Pattern::Tuple { items, span });
            }
            let end = self.expect_symbol(")", "expected ')' to close pattern");
            let _ = end;
            return items.into_iter().next();
        }
        if self.consume_symbol("[") {
            let mut items = Vec::new();
            let mut rest = None;
            while !self.check_symbol("]") && self.pos < self.tokens.len() {
                if self.consume_symbol("...") {
                    if let Some(pattern) = self.parse_pattern() {
                        rest = Some(Box::new(pattern));
                    }
                } else if let Some(pattern) = self.parse_pattern() {
                    items.push(pattern);
                }
                if !self.consume_symbol(",") {
                    break;
                }
            }
            let end = self.expect_symbol("]", "expected ']' to close list pattern");
            let span = merge_span(
                items
                    .first()
                    .map(pattern_span)
                    .unwrap_or(self.previous_span()),
                end.unwrap_or(self.previous_span()),
            );
            return Some(Pattern::List { items, rest, span });
        }
        if self.consume_symbol("{") {
            let mut fields = Vec::new();
            self.consume_newlines();
            while !self.check_symbol("}") && self.pos < self.tokens.len() {
                if let Some(field) = self.parse_record_pattern_field() {
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

                    let is_next_field_start = self
                        .tokens
                        .get(self.pos)
                        .is_some_and(|tok| match tok.kind {
                            TokenKind::Ident => tok
                                .text
                                .chars()
                                .next()
                                .is_some_and(|ch| ch.is_ascii_lowercase()),
                            _ => false,
                        });
                    if is_next_field_start {
                        if !had_newline {
                            let span = self.peek_span().unwrap_or_else(|| self.previous_span());
                            self.emit_diag(
                                "E1538",
                                "expected ',' between record pattern fields",
                                span,
                            );
                        }
                        continue;
                    }
                    continue;
                }
                self.pos += 1;
            }
            let end = self.expect_symbol("}", "expected '}' to close record pattern");
            let span = merge_span(
                fields
                    .first()
                    .map(|field| field.span.clone())
                    .unwrap_or(self.previous_span()),
                end.unwrap_or(self.previous_span()),
            );
            return Some(Pattern::Record { fields, span });
        }
        if let Some(number) = self.consume_number() {
            return Some(Pattern::Literal(Literal::Number {
                text: number.text,
                span: number.span,
            }));
        }
        if let Some(string) = self.consume_string() {
            return Some(Pattern::Literal(self.parse_text_literal_plain(string)));
        }
        if let Some(sigil) = self.consume_sigil() {
            if let Some((tag, body, flags)) = parse_sigil_text(&sigil.text) {
                return Some(Pattern::Literal(Literal::Sigil {
                    tag,
                    body,
                    flags,
                    span: sigil.span,
                }));
            }
            return Some(Pattern::Literal(Literal::Sigil {
                tag: "?".to_string(),
                body: sigil.text,
                flags: String::new(),
                span: sigil.span,
            }));
        }
        None
    }

    fn parse_record_pattern_field(&mut self) -> Option<RecordPatternField> {
        let mut path = Vec::new();
        let start = self.pos;
        if let Some(name) = self.consume_ident() {
            if !name
                .name
                .chars()
                .next()
                .is_some_and(|ch| ch.is_ascii_lowercase())
            {
                self.pos = start;
                return None;
            }
            path.push(name);
        } else {
            self.pos = start;
            return None;
        }
        while self.consume_symbol(".") {
            if let Some(name) = self.consume_ident() {
                if !name
                    .name
                    .chars()
                    .next()
                    .is_some_and(|ch| ch.is_ascii_lowercase())
                {
                    break;
                }
                path.push(name);
            } else {
                break;
            }
        }

        let shorthand_subject = false; // `!` deconstructor head removed (Change 4)
        let pattern = if self.match_keyword("as") || self.consume_symbol("@") || self.consume_symbol(":") {
            if self.tokens.get(self.pos - 1).is_some_and(|t| t.text == "@") {
                self.emit_diag(
                    "E1603",
                    "`@` in destructuring is deprecated; use `as` instead",
                    self.previous_span(),
                );
            }
            self.parse_pattern()
                .unwrap_or(Pattern::Wildcard(self.previous_span()))
        } else {
            // Shorthand `{ field }` / `{ a.b.c }` is only valid when followed by a field
            // separator. Otherwise it's almost certainly garbage like `{ field * 2 }`,
            // which should become a diagnostic instead of silently parsing extra "fields".
            let is_separator = self.peek_newline()
                || self.check_symbol(",")
                || self.check_symbol("}")
                || self.pos >= self.tokens.len();
            if !is_separator {
                let span = self.peek_span().unwrap_or_else(|| self.previous_span());
                self.emit_diag(
                    "E1537",
                    "expected ':', '@', ',', or '}' after record pattern field",
                    span,
                );
                while self.pos < self.tokens.len() {
                    if self.peek_newline() || self.check_symbol(",") || self.check_symbol("}") {
                        break;
                    }
                    self.pos += 1;
                }
            }

            let last = path.last().cloned().unwrap();
            if shorthand_subject {
                Pattern::SubjectIdent(last)
            } else {
                Pattern::Ident(last)
            }
        };
        let span = merge_span(path.first().unwrap().span.clone(), pattern_span(&pattern));
        Some(RecordPatternField {
            path,
            pattern,
            span,
        })
    }
    fn parse_domain_type_decl(&mut self, decorators: Vec<Decorator>) -> Option<TypeDecl> {
        let name = self.consume_ident()?;
        let mut params = Vec::new();
        while let Some(param) = self.consume_ident() {
            params.push(param);
        }
        self.expect_symbol("=", "expected '=' in type declaration");

        let mut ctors = Vec::new();
        while let Some(ctor_name) = self.consume_ident() {
            let mut args = Vec::new();
            while !self.check_symbol("|")
                && !self.peek_newline()
                && !self.check_symbol("}")
                && self.pos < self.tokens.len()
            {
                if let Some(ty) = self.parse_type_expr() {
                    args.push(ty);
                } else {
                    break;
                }
            }
            let span = merge_span(
                ctor_name.span.clone(),
                args.last().map(type_span).unwrap_or(ctor_name.span.clone()),
            );
            ctors.push(TypeCtor {
                name: ctor_name,
                args,
                span,
            });
            if !self.consume_symbol("|") {
                break;
            }
        }

        let span = merge_span(
            name.span.clone(),
            ctors
                .last()
                .map(|ctor| ctor.span.clone())
                .unwrap_or(name.span.clone()),
        );
        Some(TypeDecl {
            decorators,
            name,
            params,
            constructors: ctors,
            span,
        })
    }
}
