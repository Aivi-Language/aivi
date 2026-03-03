impl Parser {
    fn parse_patch_literal(&mut self, start: Span) -> Expr {
        self.expect_symbol("{", "expected '{' to start patch literal");
        let mut fields = Vec::new();
        while !self.check_symbol("}") && self.pos < self.tokens.len() {
            if let Some(field) = self.parse_record_field() {
                fields.push(field);
                continue;
            }
            self.pos += 1;
        }
        let end = self.expect_symbol("}", "expected '}' to close patch literal");
        let span = merge_span(start.clone(), end.unwrap_or(start));
        Expr::PatchLit { fields, span }
    }

    fn parse_map_literal(&mut self, start_span: Span) -> Option<Expr> {
        self.expect_symbol("{", "expected '{' to start map literal");
        let mut entries: Vec<(bool, Expr, Option<Expr>)> = Vec::new();
        self.consume_newlines();
        while !self.check_symbol("}") && self.pos < self.tokens.len() {
            if self.consume_symbol("...") {
                if let Some(expr) = self.parse_expr() {
                    entries.push((true, expr, None));
                }
            } else if let Some(key) = self.parse_primary() {
                self.consume_newlines();
                self.expect_symbol("=>", "expected '=>' in map literal");
                let value = self.parse_expr().unwrap_or(Expr::Raw {
                    text: String::new(),
                    span: expr_span(&key),
                });
                entries.push((false, key, Some(value)));
            }
            let had_newline = self.peek_newline();
            self.consume_newlines();
            if self.consume_symbol(",") {
                self.consume_newlines();
                continue;
            }
            if self.check_symbol("}") {
                break;
            }
            if self.is_expr_start() {
                if !had_newline {
                    let span = self.peek_span().unwrap_or_else(|| self.previous_span());
                    self.emit_diag("E1526", "expected ',' between map entries", span);
                }
                continue;
            }
            break;
        }
        let end = self.expect_symbol("}", "expected '}' to close map literal");
        let span = merge_span(
            start_span.clone(),
            end.unwrap_or_else(|| start_span.clone()),
        );
        Some(self.build_map_literal_expr(entries, span))
    }

    fn parse_set_literal(&mut self, start_span: Span) -> Option<Expr> {
        self.expect_symbol("[", "expected '[' to start set literal");
        let mut entries: Vec<(bool, Expr)> = Vec::new();
        self.consume_newlines();
        while !self.check_symbol("]") && self.pos < self.tokens.len() {
            if self.consume_symbol("...") {
                if let Some(expr) = self.parse_expr() {
                    entries.push((true, expr));
                }
            } else if let Some(value) = self.parse_expr() {
                entries.push((false, value));
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
                    self.emit_diag("E1527", "expected ',' between set entries", span);
                }
                continue;
            }
            break;
        }
        let end = self.expect_symbol("]", "expected ']' to close set literal");
        let span = merge_span(
            start_span.clone(),
            end.unwrap_or_else(|| start_span.clone()),
        );
        Some(self.build_set_literal_expr(entries, span))
    }

    pub(super) fn parse_mat_literal(&mut self, start_span: Span) -> Option<Expr> {
        self.expect_symbol("[", "expected '[' to start matrix literal");
        let mut cells = Vec::new();
        self.consume_newlines();
        while !self.check_symbol("]") && self.pos < self.tokens.len() {
            if let Some(expr) = self.parse_primary() {
                cells.push(expr);
            } else {
                let span = self.peek_span().unwrap_or_else(|| self.previous_span());
                self.emit_diag("E1537", "expected expression in matrix literal", span);
                break;
            }
            self.consume_newlines();
        }
        let end = self.expect_symbol("]", "expected ']' to close matrix literal");
        let span = merge_span(
            start_span.clone(),
            end.unwrap_or_else(|| start_span.clone()),
        );

        let size = match cells.len() {
            4 => 2,
            9 => 3,
            16 => 4,
            _ => {
                self.emit_diag(
                    "E1538",
                    &format!("invalid matrix size: {} cells. Expected 4 (2x2), 9 (3x3), or 16 (4x4)", cells.len()),
                    span.clone(),
                );
                return Some(Expr::Record { fields: vec![], span });
            }
        };

        let mut fields = Vec::new();
        let mut idx = 0;
        for r in 0..size {
            for c in 0..size {
                let field_name = format!("m{}{}", r, c);
                let value_expr = cells.get(idx).expect("infallible").clone();
                fields.push(RecordField {
                    spread: false,
                    path: vec![PathSegment::Field(SpannedName {
                        name: field_name.clone(),
                        span: expr_span(&value_expr),
                    })],
                    value: value_expr,
                    span: span.clone(),
                });
                idx += 1;
            }
        }

        Some(Expr::Record { fields, span })
    }

    pub(super) fn parse_path_literal(&mut self, start_span: Span) -> Option<Expr> {
        self.expect_symbol("[", "expected '[' to start path literal");
        
        let mut segments = Vec::new();
        let mut absolute = false;

        let mut current_segment_span: Option<Span> = None;
        let mut current_segment_text = String::new();

        let finish_segment = |text: &mut String, segments: &mut Vec<Expr>, text_span: &Option<Span>| {
            if !text.is_empty() {
                segments.push(Expr::Literal(crate::surface::Literal::String {
                    text: text.clone(),
                    span: text_span.clone().unwrap_or(start_span.clone()),
                }));
                text.clear();
            }
        };

        if self.consume_symbol("/") {
            absolute = true;
        }

        while self.pos < self.tokens.len() {
            if self.check_symbol("]") {
                break;
            }

            if self.consume_symbol("/") {
                finish_segment(&mut current_segment_text, &mut segments, &current_segment_span);
                current_segment_span = None;
                continue;
            }

            let token = &self.tokens[self.pos];

            // Path sigils might include interpolations later, but for now we just take the literal text
            // of the token.
            if current_segment_span.is_none() {
                current_segment_span = Some(token.span.clone());
            } else {
                current_segment_span = Some(merge_span(current_segment_span.expect("infallible"), token.span.clone()));
            }
            current_segment_text.push_str(&token.text);
            self.pos += 1;
        }

        finish_segment(&mut current_segment_text, &mut segments, &current_segment_span);

        // Normalize `.` and `..` segments at parse time (mirrors `aivi.path.normalizeSegments`).
        let segments = {
            let mut acc: Vec<Expr> = Vec::new();
            for seg in segments {
                if let Expr::Literal(crate::surface::Literal::String { ref text, .. }) = seg {
                    if text == "." || text.is_empty() {
                        continue;
                    } else if text == ".." {
                        if absolute {
                            // absolute path: drop `..` at root
                            if acc.last().is_none_or(|e| matches!(e, Expr::Literal(crate::surface::Literal::String { text, .. }) if text == "..")) {
                                // nothing to pop or already `..`; skip for absolute
                            } else {
                                acc.pop();
                            }
                        } else {
                            // relative path: keep `..` when nothing to pop
                            if acc.last().is_none_or(|e| matches!(e, Expr::Literal(crate::surface::Literal::String { text, .. }) if text == "..")) {
                                acc.push(seg);
                            } else {
                                acc.pop();
                            }
                        }
                    } else {
                        acc.push(seg);
                    }
                } else {
                    // non-literal segment (e.g. interpolation) – keep as-is
                    acc.push(seg);
                }
            }
            acc
        };

        let end = self.expect_symbol("]", "expected ']' to close path literal");
        let span = merge_span(
            start_span.clone(),
            end.unwrap_or_else(|| start_span.clone()),
        );

        let list_expr = Expr::List {
            items: segments.into_iter().map(|e| ListItem {
                expr: e,
                spread: false,
                span: span.clone(), // We could use actual span for each part, but we cheat here a bit
            }).collect(),
            span: span.clone(),
        };

        let absolute_expr = Expr::Literal(crate::surface::Literal::Bool {
            value: absolute,
            span: start_span.clone(),
        });

        let fields = vec![
            RecordField {
                spread: false,
                path: vec![PathSegment::Field(SpannedName {
                    name: "absolute".into(),
                    span: span.clone(),
                })],
                value: absolute_expr,
                span: span.clone(),
            },
            RecordField {
                spread: false,
                path: vec![PathSegment::Field(SpannedName {
                    name: "segments".into(),
                    span: span.clone(),
                })],
                value: list_expr,
                span: span.clone(),
            }
        ];

        Some(Expr::Record { fields, span })
    }

    fn build_map_literal_expr(&self, entries: Vec<(bool, Expr, Option<Expr>)>, span: Span) -> Expr {
        let map_name = SpannedName {
            name: "Map".into(),
            span: span.clone(),
        };
        let empty = Expr::FieldAccess {
            base: Box::new(Expr::Ident(map_name.clone())),
            field: SpannedName {
                name: "empty".into(),
                span: span.clone(),
            },
            span: span.clone(),
        };
        let union_field = SpannedName {
            name: "union".into(),
            span: span.clone(),
        };
        let from_list_field = SpannedName {
            name: "fromList".into(),
            span: span.clone(),
        };
        let mut acc = empty;
        for (is_spread, key, value) in entries {
            let next = if is_spread {
                key
            } else {
                let value = value.unwrap_or(Expr::Raw {
                    text: String::new(),
                    span: span.clone(),
                });
                let tuple_span = merge_span(expr_span(&key), expr_span(&value));
                let tuple = Expr::Tuple {
                    items: vec![key, value],
                    span: tuple_span.clone(),
                };
                let list = Expr::List {
                    items: vec![ListItem {
                        expr: tuple,
                        spread: false,
                        span: tuple_span,
                    }],
                    span: span.clone(),
                };
                Expr::Call {
                    func: Box::new(Expr::FieldAccess {
                        base: Box::new(Expr::Ident(map_name.clone())),
                        field: from_list_field.clone(),
                        span: span.clone(),
                    }),
                    args: vec![list],
                    span: span.clone(),
                }
            };
            acc = Expr::Call {
                func: Box::new(Expr::FieldAccess {
                    base: Box::new(Expr::Ident(map_name.clone())),
                    field: union_field.clone(),
                    span: span.clone(),
                }),
                args: vec![acc, next],
                span: span.clone(),
            };
        }
        acc
    }

    fn build_set_literal_expr(&self, entries: Vec<(bool, Expr)>, span: Span) -> Expr {
        let set_name = SpannedName {
            name: "Set".into(),
            span: span.clone(),
        };
        let empty = Expr::FieldAccess {
            base: Box::new(Expr::Ident(set_name.clone())),
            field: SpannedName {
                name: "empty".into(),
                span: span.clone(),
            },
            span: span.clone(),
        };
        let union_field = SpannedName {
            name: "union".into(),
            span: span.clone(),
        };
        let from_list_field = SpannedName {
            name: "fromList".into(),
            span: span.clone(),
        };
        let mut acc = empty;
        for (is_spread, value) in entries {
            let next = if is_spread {
                value
            } else {
                let list = Expr::List {
                    items: vec![ListItem {
                        expr: value,
                        spread: false,
                        span: span.clone(),
                    }],
                    span: span.clone(),
                };
                Expr::Call {
                    func: Box::new(Expr::FieldAccess {
                        base: Box::new(Expr::Ident(set_name.clone())),
                        field: from_list_field.clone(),
                        span: span.clone(),
                    }),
                    args: vec![list],
                    span: span.clone(),
                }
            };
            acc = Expr::Call {
                func: Box::new(Expr::FieldAccess {
                    base: Box::new(Expr::Ident(set_name.clone())),
                    field: union_field.clone(),
                    span: span.clone(),
                }),
                args: vec![acc, next],
                span: span.clone(),
            };
        }
        acc
    }

    fn parse_record_field(&mut self) -> Option<RecordField> {
        // Record spread: `{ ...base, field: value }`
        if self.consume_symbol("...") {
            let start_span = self.previous_span();
            let value = self.parse_expr().unwrap_or(Expr::Raw {
                text: String::new(),
                span: start_span.clone(),
            });
            let span = merge_span(start_span, expr_span(&value));
            return Some(RecordField {
                spread: true,
                path: Vec::new(),
                value,
                span,
            });
        }

        let start = self.pos;
        let mut path = Vec::new();
        if let Some(name) = self.consume_ident() {
            path.push(PathSegment::Field(name));
        } else if !self.check_symbol("[") {
            self.pos = start;
            return None;
        }
        loop {
            if self.consume_symbol(".") {
                if let Some(name) = self.consume_ident() {
                    path.push(PathSegment::Field(name));
                    continue;
                }
            }
            if self.consume_symbol("[") {
                let seg_start = self.previous_span();
                self.consume_newlines();
                if self.consume_symbol("*") {
                    self.consume_newlines();
                    let end = self.expect_symbol("]", "expected ']' in record field path");
                    let end = end.unwrap_or(self.previous_span());
                    path.push(PathSegment::All(merge_span(seg_start, end)));
                    continue;
                }

                let expr = self.parse_expr().unwrap_or(Expr::Raw {
                    text: String::new(),
                    span: self.previous_span(),
                });
                self.consume_newlines();
                let end = self.expect_symbol("]", "expected ']' in record field path");
                let end = end.unwrap_or(self.previous_span());
                path.push(PathSegment::Index(expr, merge_span(seg_start, end)));
                continue;
            }
            break;
        }

        if !self.consume_symbol(":") {
            // Shorthand `{ name }` desugars to `{ name: name }` — only for a single plain ident.
            if path.len() == 1 {
                if let PathSegment::Field(name) = &path[0] {
                    let is_separator = self.peek_newline()
                        || self.check_symbol(",")
                        || self.check_symbol("}")
                        || self.pos >= self.tokens.len();
                    if is_separator {
                        let span = name.span.clone();
                        let value = Expr::Ident(name.clone());
                        return Some(RecordField {
                            spread: false,
                            path,
                            value,
                            span,
                        });
                    }
                }
            }
            self.pos = start;
            return None;
        }
        let value = self.parse_expr().unwrap_or(Expr::Raw {
            text: String::new(),
            span: self.previous_span(),
        });
        let span = merge_span(path_span(&path), expr_span(&value));
        Some(RecordField {
            spread: false,
            path,
            value,
            span,
        })
    }
}

include!("literals_and_blocks/blocks.rs");
