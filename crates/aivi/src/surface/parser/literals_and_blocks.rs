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

    fn parse_block(&mut self, kind: BlockKind) -> Expr {
        // Inside a `loop` body, promote plain `{ ... }` blocks to the parent
        // block kind (e.g. `Do { monad }`) so that keywords like `recurse`,
        // `when`, `unless`, and bind (`<-`) work correctly.
        let kind = if matches!(kind, BlockKind::Plain) {
            if let Some(promoted) = self.loop_block_kind.take() {
                promoted
            } else {
                kind
            }
        } else {
            kind
        };
        let start = self.previous_span();
        self.expect_symbol("{", "expected '{' to start block");
        let mut items = Vec::new();
        while !self.check_symbol("}") && self.pos < self.tokens.len() {
            self.consume_newlines();
            if self.check_symbol("}") {
                break;
            }
            if self.match_keyword("loop") {
                let loop_start = self.previous_span();
                if !matches!(kind, BlockKind::Generate | BlockKind::Do { .. }) {
                    self.emit_diag(
                        "E1533",
                        "`loop` is only allowed inside `generate { ... }` or `do Effect { ... }` blocks",
                        loop_start.clone(),
                    );
                }
                let pattern = self.parse_pattern().unwrap_or(Pattern::Wildcard(loop_start.clone()));
                self.expect_symbol("=", "expected '=' in loop binding");
                self.consume_newlines();
                let init = self.parse_match_or_binary().unwrap_or(Expr::Raw {
                    text: String::new(),
                    span: loop_start.clone(),
                });
                self.expect_symbol("=>", "expected '=>' in loop binding");

                if matches!(kind, BlockKind::Do { .. } | BlockKind::Generate) {
                    // --- Loop: desugar at parse time for Effect + Generate blocks ---
                    // Generate a fresh internal name for the recursive function.
                    let fn_name = self.fresh_internal_name("loop", loop_start.clone());

                    // Set the promotion flag so the body's `{ ... }` block is
                    // parsed as the same Do kind (enables `<-`, `recurse`, etc.).
                    self.loop_block_kind = Some(kind.clone());

                    let body = self.parse_expr().unwrap_or(Expr::Raw {
                        text: String::new(),
                        span: loop_start.clone(),
                    });

                    // Walk the body and replace every `BlockItem::Recurse { expr }`
                    // with `BlockItem::Expr { Call(fn_name, expr) }`.
                    let body = replace_recurse_in_expr(body, &fn_name);

                    let body_span = expr_span(&body);
                    let outer_span = merge_span(loop_start, body_span.clone());

                    // Emit: __loop_N = pattern => body
                    items.push(BlockItem::Let {
                        pattern: Pattern::Ident(fn_name.clone()),
                        expr: Expr::Lambda {
                            params: vec![pattern],
                            body: Box::new(body),
                            span: outer_span.clone(),
                        },
                        span: outer_span.clone(),
                    });

                    // Emit: __loop_N init
                    items.push(BlockItem::Expr {
                        expr: Expr::Call {
                            func: Box::new(Expr::Ident(fn_name)),
                            args: vec![init],
                            span: outer_span.clone(),
                        },
                        span: outer_span,
                    });
                } else {
                    // Recovery path for unsupported loop contexts.
                    let body = self.parse_expr().unwrap_or(Expr::Raw {
                        text: String::new(),
                        span: loop_start.clone(),
                    });
                    let span = merge_span(loop_start, expr_span(&body));
                    items.push(BlockItem::Expr { expr: body, span });
                }
                continue;
            }
            if self.match_keyword("yield") {
                let yield_kw = self.previous_span();
                if !matches!(kind, BlockKind::Generate | BlockKind::Resource) {
                    self.emit_diag(
                        "E1534",
                        "`yield` is only allowed inside `generate { ... }` or `resource { ... }` blocks",
                        yield_kw.clone(),
                    );
                }
                let expr = self.parse_expr().unwrap_or(Expr::Raw {
                    text: String::new(),
                    span: yield_kw.clone(),
                });
                let span = merge_span(yield_kw, expr_span(&expr));
                if matches!(kind, BlockKind::Generate | BlockKind::Resource) {
                    items.push(BlockItem::Yield { expr, span });
                } else {
                    // Recovery: treat as a plain expression statement to keep parsing.
                    items.push(BlockItem::Expr { expr, span });
                }
                continue;
            }
            if self.match_keyword("recurse") {
                let recurse_kw = self.previous_span();
                if !matches!(kind, BlockKind::Generate | BlockKind::Do { .. }) {
                    self.emit_diag(
                        "E1535",
                        "`recurse` is only allowed inside `generate { ... }` or `do Effect { ... }` blocks (within a `loop`)",
                        recurse_kw.clone(),
                    );
                }
                let expr = self.parse_expr().unwrap_or(Expr::Raw {
                    text: String::new(),
                    span: recurse_kw.clone(),
                });
                let span = merge_span(recurse_kw, expr_span(&expr));
                if matches!(kind, BlockKind::Generate | BlockKind::Do { .. }) {
                    items.push(BlockItem::Recurse { expr, span });
                } else {
                    items.push(BlockItem::Expr { expr, span });
                }
                continue;
            }
            // `when cond <- eff` — conditional effect (Change 6)
            if self.match_keyword("when") {
                let when_kw = self.previous_span();
                if !matches!(kind, BlockKind::Do { .. }) {
                    self.emit_diag(
                        "E1540",
                        "`when` is only allowed inside `do Monad { ... }` blocks",
                        when_kw.clone(),
                    );
                }
                let cond = self.parse_binary(0).unwrap_or(Expr::Raw {
                    text: String::new(),
                    span: when_kw.clone(),
                });
                self.expect_symbol("<-", "expected '<-' after `when` condition");
                let effect = self.parse_expr().unwrap_or(Expr::Raw {
                    text: String::new(),
                    span: when_kw.clone(),
                });
                let span = merge_span(when_kw, expr_span(&effect));
                items.push(BlockItem::When { cond, effect, span });
                continue;
            }
            // `unless cond <- eff` — negated conditional effect
            if self.match_keyword("unless") {
                let unless_kw = self.previous_span();
                if !matches!(kind, BlockKind::Do { .. }) {
                    self.emit_diag(
                        "E1543",
                        "`unless` is only valid inside a `do Effect { … }` block",
                        unless_kw.clone(),
                    );
                }
                let cond = self.parse_binary(0).unwrap_or(Expr::Raw {
                    text: String::new(),
                    span: unless_kw.clone(),
                });
                self.expect_symbol("<-", "expected '<-' after `unless` condition");
                let effect = self.parse_expr().unwrap_or(Expr::Raw {
                    text: String::new(),
                    span: unless_kw.clone(),
                });
                let span = merge_span(unless_kw, expr_span(&effect));
                items.push(BlockItem::Unless { cond, effect, span });
                continue;
            }
            // `given cond or failExpr` — precondition guard (Change 8)
            if self.match_keyword("given") {
                let given_kw = self.previous_span();
                if !matches!(kind, BlockKind::Do { .. }) {
                    self.emit_diag(
                        "E1541",
                        "`given` is only allowed inside `do Monad { ... }` blocks",
                        given_kw.clone(),
                    );
                }
                let cond = self.parse_binary(0).unwrap_or(Expr::Raw {
                    text: String::new(),
                    span: given_kw.clone(),
                });
                self.expect_keyword("or", "expected 'or' after `given` condition");
                self.consume_newlines();
                let fail_expr = if self.consume_symbol("|") {
                    // match form: given cond or | Pattern => failExpr | ...
                    let mut arms = Vec::new();
                    loop {
                        let pattern = self
                            .parse_pattern()
                            .unwrap_or(Pattern::Wildcard(given_kw.clone()));
                        self.expect_symbol("=>", "expected '=>' in given or arm");
                        let body = self.parse_expr().unwrap_or(Expr::Raw {
                            text: String::new(),
                            span: given_kw.clone(),
                        });
                        let span = merge_span(pattern_span(&pattern), expr_span(&body));
                        arms.push(MatchArm {
                            pattern,
                            guard: None,
                            body,
                            span,
                        });
                        self.consume_newlines();
                        if !self.consume_symbol("|") {
                            break;
                        }
                    }
                    let span = merge_span(
                        given_kw.clone(),
                        arms.last().map(|a| a.span.clone()).unwrap_or(given_kw.clone()),
                    );
                    Expr::Match {
                        scrutinee: Some(Box::new(cond.clone())),
                        arms,
                        span,
                    }
                } else {
                    self.parse_expr().unwrap_or(Expr::Raw {
                        text: String::new(),
                        span: given_kw.clone(),
                    })
                };
                let span = merge_span(given_kw, expr_span(&fail_expr));
                items.push(BlockItem::Given { cond, fail_expr, span });
                continue;
            }
            // `on Transition => effect` — transition event wiring (Change 7)
            if self.match_keyword("on") {
                let on_kw = self.previous_span();
                if !matches!(kind, BlockKind::Do { .. }) {
                    self.emit_diag(
                        "E1542",
                        "`on` is only allowed inside `do Monad { ... }` blocks",
                        on_kw.clone(),
                    );
                }
                let transition = self.parse_postfix().unwrap_or(Expr::Raw {
                    text: String::new(),
                    span: on_kw.clone(),
                });
                self.expect_symbol("=>", "expected '=>' after `on` transition");
                let handler = self.parse_expr().unwrap_or(Expr::Raw {
                    text: String::new(),
                    span: on_kw.clone(),
                });
                let span = merge_span(on_kw, expr_span(&handler));
                items.push(BlockItem::On { transition, handler, span });
                continue;
            }
            let checkpoint = self.pos;
            let diag_checkpoint = self.diagnostics.len();
            if let Some(pattern) = self.parse_pattern() {
                if self.consume_symbol("<-") {
                    let expr = self.parse_expr_without_result_or().unwrap_or(Expr::Raw {
                        text: String::new(),
                        span: pattern_span(&pattern),
                    });
                    if !matches!(
                        kind,
                        BlockKind::Do { .. } | BlockKind::Generate | BlockKind::Resource
                    ) {
                        self.emit_diag(
                            "E1536",
                            "`<-` is only allowed inside `do Monad { ... }`, `generate { ... }`, or `resource { ... }` blocks",
                            merge_span(pattern_span(&pattern), expr_span(&expr)),
                        );
                        let span = merge_span(pattern_span(&pattern), expr_span(&expr));
                        items.push(BlockItem::Let {
                            pattern,
                            expr,
                            span,
                        });
                        continue;
                    }
                    let expr = if matches!(kind, BlockKind::Do { .. }) && self.peek_keyword("or") {
                        // Disambiguation:
                        // - `x <- eff or | NotFound m => ...` is effect-fallback (patterns match E)
                        // - `x <- (res or "boom")` is result-fallback (expression-level)
                        // - `x <- res or | Err _ => ...` is treated as result-fallback for ergonomics
                        let checkpoint = self.pos;
                        self.pos += 1; // consume `or` for lookahead
                        self.consume_newlines();
                        let mut looks_like_result_or = false;
                        if self.consume_symbol("|") {
                            self.consume_newlines();
                            if let Some(token) = self.tokens.get(self.pos) {
                                looks_like_result_or =
                                    token.kind == TokenKind::Ident && token.text == "Err";
                            }
                        }
                        self.pos = checkpoint;

                        let _ = self.match_keyword("or");
                        if looks_like_result_or {
                            self.parse_result_or_suffix(expr).unwrap_or(Expr::Raw {
                                text: String::new(),
                                span: self.previous_span(),
                            })
                        } else {
                            self.parse_effect_or_suffix(expr)
                        }
                    } else {
                        expr
                    };
                    let span = merge_span(pattern_span(&pattern), expr_span(&expr));
                    items.push(BlockItem::Bind {
                        pattern,
                        expr,
                        span,
                    });
                    continue;
                }
                if self.consume_symbol("->") {
                    let expr = self.parse_expr().unwrap_or(Expr::Raw {
                        text: String::new(),
                        span: pattern_span(&pattern),
                    });
                    let span = merge_span(pattern_span(&pattern), expr_span(&expr));
                    items.push(BlockItem::Filter { expr, span });
                    continue;
                }
                if self.consume_symbol("=") {
                    let expr = self.parse_expr().unwrap_or(Expr::Raw {
                        text: String::new(),
                        span: pattern_span(&pattern),
                    });
                    let span = merge_span(pattern_span(&pattern), expr_span(&expr));
                    items.push(BlockItem::Let {
                        pattern,
                        expr,
                        span,
                    });
                    continue;
                }
            }
            // `parse_pattern()` above is speculative (it might be the start of an expression
            // statement). If we didn't commit to a pattern-led statement (`<-`/`->`/`=`),
            // roll back both the token position and any diagnostics it emitted.
            self.diagnostics.truncate(diag_checkpoint);
            self.pos = checkpoint;
            if let Some(expr) = self.parse_expr() {
                let span = expr_span(&expr);
                items.push(BlockItem::Expr { expr, span });
                continue;
            }
            self.pos += 1;
        }
        let end = self.expect_symbol("}", "expected '}' to close block");
        let span = merge_span(start.clone(), end.unwrap_or(start));
        Expr::Block { kind, items, span }
    }

    fn parse_effect_or_suffix(&mut self, effect_expr: Expr) -> Expr {
        let or_span = self.previous_span();
        self.consume_newlines();

        // Parse either `or <expr>` or `or | pat => expr | ...` where patterns match the error value.
        let (patterns, fallback_expr) = if self.consume_symbol("|") {
            let mut arms = Vec::new();
            loop {
                let mut pat = self
                    .parse_pattern()
                    .unwrap_or(Pattern::Wildcard(or_span.clone()));
                // If someone wrote `Err ...` here, recover by stripping the outer `Err` and
                // still treat it as an error-pattern arm.
                if let Pattern::Constructor { name, args, .. } = &pat {
                    if name.name == "Err" && args.len() == 1 {
                        pat = args[0].clone();
                        self.emit_diag(
                            "E1532",
                            "effect `or` arms match the error value; omit the leading `Err`",
                            pattern_span(&pat),
                        );
                    }
                }

                self.expect_symbol("=>", "expected '=>' in effect or arm");
                let body = self.parse_expr().unwrap_or(Expr::Raw {
                    text: String::new(),
                    span: or_span.clone(),
                });
                arms.push((pat, body));

                self.consume_newlines();
                if !self.consume_symbol("|") {
                    break;
                }
            }
            (Some(arms), None)
        } else {
            let rhs = self.parse_expr().unwrap_or(Expr::Raw {
                text: String::new(),
                span: or_span.clone(),
            });
            (None, Some(rhs))
        };
        let has_pattern_arms = patterns.is_some();

        // Desugar to:
        //   effect {
        //     __res <- attempt effect_expr
        //     __res ?
        //       | Ok x => pure x
        //       | Err <pat> => pure <body>
        //       | Err e => fail e
        //   }
        //
        // This keeps error-handling explicit in core terms (attempt/?/pure/fail).
        let res_name = self.fresh_internal_name("or_res", or_span.clone());
        let res_pat = Pattern::Ident(res_name.clone());
        let attempt_call = self.build_call_expr(
            self.build_ident_expr("attempt", or_span.clone()),
            vec![effect_expr],
            or_span.clone(),
        );
        let bind_item = BlockItem::Bind {
            pattern: res_pat,
            expr: attempt_call,
            span: or_span.clone(),
        };

        let ok_value = self.fresh_internal_name("or_ok", or_span.clone());
        let ok_arm = MatchArm {
            pattern: self.build_ctor_pattern(
                "Ok",
                vec![Pattern::Ident(ok_value.clone())],
                ok_value.span.clone(),
            ),
            guard: None,
            body: self.build_call_expr(
                self.build_ident_expr("pure", ok_value.span.clone()),
                vec![Expr::Ident(ok_value.clone())],
                ok_value.span.clone(),
            ),
            span: ok_value.span.clone(),
        };

        let mut match_arms = vec![ok_arm];
        if let Some(rhs) = fallback_expr {
            let err_pat = self.build_ctor_pattern(
                "Err",
                vec![Pattern::Wildcard(or_span.clone())],
                or_span.clone(),
            );
            let rhs_span = expr_span(&rhs);
            let body = self.build_call_expr(
                self.build_ident_expr("pure", rhs_span.clone()),
                vec![rhs],
                rhs_span,
            );
            match_arms.push(MatchArm {
                pattern: err_pat,
                guard: None,
                body,
                span: or_span.clone(),
            });
        } else if let Some(arms) = patterns {
            for (pat, body_expr) in arms {
                let err_pat = self.build_ctor_pattern("Err", vec![pat], or_span.clone());
                let body_span = expr_span(&body_expr);
                let body = self.build_call_expr(
                    self.build_ident_expr("pure", body_span.clone()),
                    vec![body_expr],
                    body_span,
                );
                match_arms.push(MatchArm {
                    pattern: err_pat,
                    guard: None,
                    body,
                    span: or_span.clone(),
                });
            }
        }

        // If the user provided explicit error-pattern arms, propagate unmatched errors.
        // For `or <fallbackExpr>`, the `Err _ => pure fallbackExpr` arm is exhaustive.
        if has_pattern_arms {
            let err_name = self.fresh_internal_name("or_err", or_span.clone());
            let err_pat = self.build_ctor_pattern(
                "Err",
                vec![Pattern::Ident(err_name.clone())],
                or_span.clone(),
            );
            let err_body = self.build_call_expr(
                self.build_ident_expr("fail", or_span.clone()),
                vec![Expr::Ident(err_name)],
                or_span.clone(),
            );
            match_arms.push(MatchArm {
                pattern: err_pat,
                guard: None,
                body: err_body,
                span: or_span.clone(),
            });
        }

        let match_expr = Expr::Match {
            scrutinee: Some(Box::new(Expr::Ident(res_name))),
            arms: match_arms,
            span: or_span.clone(),
        };

        let span = merge_span(or_span.clone(), or_span.clone());
        Expr::Block {
            kind: BlockKind::Do { monad: SpannedName { name: "Effect".into(), span: or_span.clone() } },
            items: vec![
                bind_item,
                BlockItem::Expr {
                    expr: match_expr,
                    span,
                },
            ],
            span: or_span,
        }
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

/// Walk an expression tree and replace every `BlockItem::Recurse { expr, span }`
/// with `BlockItem::Expr { expr: Call(fn_name, expr), span }`.
/// This is used to desugar `loop`/`recurse` inside `do Effect { ... }` blocks
/// at parse time, avoiding new AST variants.
fn replace_recurse_in_expr(expr: Expr, fn_name: &SpannedName) -> Expr {
    match expr {
        Expr::Block { kind, items, span } => {
            let items = items
                .into_iter()
                .map(|item| replace_recurse_in_block_item(item, fn_name))
                .collect();
            Expr::Block { kind, items, span }
        }
        Expr::If {
            cond,
            then_branch,
            else_branch,
            span,
        } => Expr::If {
            cond: Box::new(replace_recurse_in_expr(*cond, fn_name)),
            then_branch: Box::new(replace_recurse_in_expr(*then_branch, fn_name)),
            else_branch: Box::new(replace_recurse_in_expr(*else_branch, fn_name)),
            span,
        },
        Expr::Lambda { params, body, span } => Expr::Lambda {
            params,
            body: Box::new(replace_recurse_in_expr(*body, fn_name)),
            span,
        },
        Expr::Match {
            scrutinee,
            arms,
            span,
        } => Expr::Match {
            scrutinee: scrutinee.map(|e| Box::new(replace_recurse_in_expr(*e, fn_name))),
            arms: arms
                .into_iter()
                .map(|arm| MatchArm {
                    pattern: arm.pattern,
                    guard: arm.guard.map(|g| replace_recurse_in_expr(g, fn_name)),
                    body: replace_recurse_in_expr(arm.body, fn_name),
                    span: arm.span,
                })
                .collect(),
            span,
        },
        Expr::Call { func, args, span } => Expr::Call {
            func: Box::new(replace_recurse_in_expr(*func, fn_name)),
            args: args
                .into_iter()
                .map(|a| replace_recurse_in_expr(a, fn_name))
                .collect(),
            span,
        },
        Expr::Binary {
            op,
            left,
            right,
            span,
        } => Expr::Binary {
            op,
            left: Box::new(replace_recurse_in_expr(*left, fn_name)),
            right: Box::new(replace_recurse_in_expr(*right, fn_name)),
            span,
        },
        Expr::Tuple { items, span } => Expr::Tuple {
            items: items
                .into_iter()
                .map(|e| replace_recurse_in_expr(e, fn_name))
                .collect(),
            span,
        },
        Expr::List { items, span } => Expr::List {
            items: items
                .into_iter()
                .map(|li| ListItem {
                    expr: replace_recurse_in_expr(li.expr, fn_name),
                    spread: li.spread,
                    span: li.span,
                })
                .collect(),
            span,
        },
        Expr::Record { fields, span } => Expr::Record {
            fields: fields
                .into_iter()
                .map(|f| RecordField {
                    spread: f.spread,
                    path: f.path,
                    value: replace_recurse_in_expr(f.value, fn_name),
                    span: f.span,
                })
                .collect(),
            span,
        },
        Expr::Suffixed { base, suffix, span } => Expr::Suffixed {
            base: Box::new(replace_recurse_in_expr(*base, fn_name)),
            suffix,
            span,
        },
        Expr::FieldAccess { base, field, span } => Expr::FieldAccess {
            base: Box::new(replace_recurse_in_expr(*base, fn_name)),
            field,
            span,
        },
        Expr::Index { base, index, span } => Expr::Index {
            base: Box::new(replace_recurse_in_expr(*base, fn_name)),
            index: Box::new(replace_recurse_in_expr(*index, fn_name)),
            span,
        },
        Expr::UnaryNeg { expr, span } => Expr::UnaryNeg {
            expr: Box::new(replace_recurse_in_expr(*expr, fn_name)),
            span,
        },
        Expr::TextInterpolate { parts, span } => Expr::TextInterpolate {
            parts: parts
                .into_iter()
                .map(|p| match p {
                    TextPart::Expr { expr, span } => TextPart::Expr {
                        expr: Box::new(replace_recurse_in_expr(*expr, fn_name)),
                        span,
                    },
                    other => other,
                })
                .collect(),
            span,
        },
        Expr::PatchLit { fields, span } => Expr::PatchLit {
            fields: fields
                .into_iter()
                .map(|f| RecordField {
                    spread: f.spread,
                    path: f.path,
                    value: replace_recurse_in_expr(f.value, fn_name),
                    span: f.span,
                })
                .collect(),
            span,
        },
        // Leaf expressions: no sub-expressions to walk.
        other @ (Expr::Ident(_)
        | Expr::Literal(_)
        | Expr::Raw { .. }
        | Expr::FieldSection { .. }) => other,
        Expr::Mock { substitutions, body, span } => {
            let substitutions = substitutions
                .into_iter()
                .map(|mut sub| {
                    sub.value = sub.value.map(|v| replace_recurse_in_expr(v, fn_name));
                    sub
                })
                .collect();
            Expr::Mock {
                substitutions,
                body: Box::new(replace_recurse_in_expr(*body, fn_name)),
                span,
            }
        }
    }
}

/// Replace `Recurse` block items with function calls to `fn_name`.
fn replace_recurse_in_block_item(item: BlockItem, fn_name: &SpannedName) -> BlockItem {
    match item {
        BlockItem::Recurse { expr, span } => BlockItem::Expr {
            expr: Expr::Call {
                func: Box::new(Expr::Ident(fn_name.clone())),
                args: vec![replace_recurse_in_expr(expr, fn_name)],
                span: span.clone(),
            },
            span,
        },
        BlockItem::Bind {
            pattern,
            expr,
            span,
        } => BlockItem::Bind {
            pattern,
            expr: replace_recurse_in_expr(expr, fn_name),
            span,
        },
        BlockItem::Let {
            pattern,
            expr,
            span,
        } => BlockItem::Let {
            pattern,
            expr: replace_recurse_in_expr(expr, fn_name),
            span,
        },
        BlockItem::Expr { expr, span } => BlockItem::Expr {
            expr: replace_recurse_in_expr(expr, fn_name),
            span,
        },
        BlockItem::Filter { expr, span } => BlockItem::Filter {
            expr: replace_recurse_in_expr(expr, fn_name),
            span,
        },
        BlockItem::Yield { expr, span } => BlockItem::Yield {
            expr: replace_recurse_in_expr(expr, fn_name),
            span,
        },
        BlockItem::When { cond, effect, span } => BlockItem::When {
            cond: replace_recurse_in_expr(cond, fn_name),
            effect: replace_recurse_in_expr(effect, fn_name),
            span,
        },
        BlockItem::Unless {
            cond,
            effect,
            span,
        } => BlockItem::Unless {
            cond: replace_recurse_in_expr(cond, fn_name),
            effect: replace_recurse_in_expr(effect, fn_name),
            span,
        },
        BlockItem::Given {
            cond,
            fail_expr,
            span,
        } => BlockItem::Given {
            cond: replace_recurse_in_expr(cond, fn_name),
            fail_expr: replace_recurse_in_expr(fail_expr, fn_name),
            span,
        },
        BlockItem::On {
            transition,
            handler,
            span,
        } => BlockItem::On {
            transition: replace_recurse_in_expr(transition, fn_name),
            handler: replace_recurse_in_expr(handler, fn_name),
            span,
        },
    }
}
