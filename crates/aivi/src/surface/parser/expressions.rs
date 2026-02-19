impl Parser {
    fn parse_expr(&mut self) -> Option<Expr> {
        self.parse_expr_with_result_or()
    }

    fn parse_expr_with_result_or(&mut self) -> Option<Expr> {
        self.consume_newlines();
        if self.check_symbol("|") {
            let start = self.peek_span().unwrap_or_else(|| self.previous_span());
            let mut arms = Vec::new();
            loop {
                self.consume_newlines();
                if !self.consume_symbol("|") {
                    break;
                }
                let pattern = self
                    .parse_pattern()
                    .unwrap_or(Pattern::Wildcard(start.clone()));
                let guard = if self.match_keyword("when") {
                    self.parse_guard_expr()
                } else {
                    None
                };
                self.expect_symbol("=>", "expected '=>' in match arm");
                let body = self.parse_expr().unwrap_or(Expr::Raw {
                    text: String::new(),
                    span: start.clone(),
                });
                let span = merge_span(pattern_span(&pattern), expr_span(&body));
                arms.push(MatchArm {
                    pattern,
                    guard,
                    body,
                    span,
                });
            }
            let span = merge_span(
                start.clone(),
                arms.last().map(|arm| arm.span.clone()).unwrap_or(start),
            );
            return Some(Expr::Match {
                scrutinee: None,
                arms,
                span,
            });
        }
        let mut expr = self.parse_lambda_or_binary()?;
        // Result fallback sugar:
        //   res or "boom"
        //   res or | Err NotFound m => m | Err _ => "boom"
        //
        // This form is result-only (arms must match `Err ...` at the top level).
        loop {
            let checkpoint = self.pos;
            self.consume_newlines();
            if !self.match_keyword("or") {
                self.pos = checkpoint;
                break;
            }
            expr = self.parse_result_or_suffix(expr)?;
        }
        Some(expr)
    }
    fn parse_expr_without_result_or(&mut self) -> Option<Expr> {
        self.consume_newlines();
        if self.check_symbol("|") {
            // Multi-clause unary function. `or` isn't allowed in the function head.
            // The bodies are parsed with the normal expression parser.
            let start = self.peek_span().unwrap_or_else(|| self.previous_span());
            let mut arms = Vec::new();
            loop {
                self.consume_newlines();
                if !self.consume_symbol("|") {
                    break;
                }
                let pattern = self
                    .parse_pattern()
                    .unwrap_or(Pattern::Wildcard(start.clone()));
                let guard = if self.match_keyword("when") {
                    self.parse_guard_expr()
                } else {
                    None
                };
                self.expect_symbol("=>", "expected '=>' in match arm");
                let body = self.parse_expr().unwrap_or(Expr::Raw {
                    text: String::new(),
                    span: start.clone(),
                });
                let span = merge_span(pattern_span(&pattern), expr_span(&body));
                arms.push(MatchArm {
                    pattern,
                    guard,
                    body,
                    span,
                });
            }
            let span = merge_span(
                start.clone(),
                arms.last().map(|arm| arm.span.clone()).unwrap_or(start),
            );
            return Some(Expr::Match {
                scrutinee: None,
                arms,
                span,
            });
        }
        self.parse_lambda_or_binary()
    }

    fn parse_result_or_suffix(&mut self, base: Expr) -> Option<Expr> {
        let or_span = self.previous_span();
        self.consume_newlines();

        // Parse either `or <expr>` or `or | ... => ... | ...`
        let (arms, fallback_expr) = if self.consume_symbol("|") {
            let mut arms = Vec::new();
            loop {
                let pattern = self
                    .parse_pattern()
                    .unwrap_or(Pattern::Wildcard(or_span.clone()));
                let guard = if self.match_keyword("when") {
                    self.parse_guard_expr()
                } else {
                    None
                };
                self.expect_symbol("=>", "expected '=>' in or arm");
                let body = self.parse_expr().unwrap_or(Expr::Raw {
                    text: String::new(),
                    span: or_span.clone(),
                });
                let span = merge_span(pattern_span(&pattern), expr_span(&body));
                arms.push(MatchArm {
                    pattern,
                    guard,
                    body,
                    span,
                });

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

        // Validate result-or arms: fallback-only, no success arms, and no wildcard arms.
        if let Some(arms) = &arms {
            let mut has_catch_all_err = false;
            for arm in arms {
                match &arm.pattern {
                    Pattern::Constructor { name, args, .. } if name.name == "Err" => {
                        if args.len() == 1 && matches!(&args[0], Pattern::Wildcard(_)) {
                            has_catch_all_err = true;
                        }
                    }
                    _ => {
                        self.emit_diag(
                            "E1530",
                            "`or` arms must match only `Err ...` (fallback-only)",
                            pattern_span(&arm.pattern),
                        );
                    }
                }
            }
            if !has_catch_all_err {
                // Without `Err _`, the desugared match would be non-exhaustive.
                self.emit_diag(
                    "E1531",
                    "`or` arms must include a final `| Err _ => ...` catch-all",
                    or_span.clone(),
                );
            }
        }

        // Desugar:
        //   res or rhs
        //     => res ? | Ok x => x | Err _ => rhs
        //
        //   res or | Err p => rhs | Err _ => rhs2
        //     => res ? | Ok x => x | Err p => rhs | Err _ => rhs2
        let ok_value = self.fresh_internal_name("or_ok", expr_span(&base));
        let ok_arm = MatchArm {
            pattern: self.build_ctor_pattern(
                "Ok",
                vec![Pattern::Ident(ok_value.clone())],
                ok_value.span.clone(),
            ),
            guard: None,
            body: Expr::Ident(ok_value.clone()),
            span: ok_value.span.clone(),
        };

        let mut out_arms = vec![ok_arm];
        if let Some(rhs) = fallback_expr {
            let err_pat = self.build_ctor_pattern(
                "Err",
                vec![Pattern::Wildcard(or_span.clone())],
                or_span.clone(),
            );
            out_arms.push(MatchArm {
                pattern: err_pat,
                guard: None,
                body: rhs,
                span: or_span.clone(),
            });
        } else if let Some(mut parsed_arms) = arms {
            out_arms.append(&mut parsed_arms);
        }

        let span = merge_span(
            expr_span(&base),
            out_arms.last().map(|a| a.span.clone()).unwrap_or(or_span),
        );
        Some(Expr::Match {
            scrutinee: Some(Box::new(base)),
            arms: out_arms,
            span,
        })
    }

    fn parse_lambda_or_binary(&mut self) -> Option<Expr> {
        let checkpoint = self.pos;
        let diag_checkpoint = self.diagnostics.len();
        let mut params = Vec::new();
        while let Some(pattern) = self.parse_pattern() {
            params.push(pattern);
        }
        let saw_pattern_diags = self.diagnostics.len() != diag_checkpoint;
        if !params.is_empty() && !saw_pattern_diags && self.consume_symbol("=>") {
            let body = self.parse_expr()?;
            let span = merge_span(pattern_span(&params[0]), expr_span(&body));
            return Some(Expr::Lambda {
                params,
                body: Box::new(body),
                span,
            });
        }

        self.pos = checkpoint;
        self.diagnostics.truncate(diag_checkpoint);
        self.parse_match_or_binary()
    }

    fn parse_match_or_binary(&mut self) -> Option<Expr> {
        let expr = self.parse_binary(0)?;
        if self.match_keyword("match") {
            let mut arms = Vec::new();
            loop {
                self.consume_newlines();
                if !self.consume_symbol("|") {
                    break;
                }
                let pattern = self
                    .parse_pattern()
                    .unwrap_or(Pattern::Wildcard(expr_span(&expr)));
                let guard = if self.match_keyword("when") {
                    self.parse_guard_expr()
                } else {
                    None
                };
                self.expect_symbol("=>", "expected '=>' in match arm");
                let body = self.parse_expr().unwrap_or(Expr::Raw {
                    text: String::new(),
                    span: expr_span(&expr),
                });
                let span = merge_span(pattern_span(&pattern), expr_span(&body));
                arms.push(MatchArm {
                    pattern,
                    guard,
                    body,
                    span,
                });
            }
            let span = merge_span(
                expr_span(&expr),
                arms.last()
                    .map(|arm| arm.span.clone())
                    .unwrap_or(expr_span(&expr)),
            );
            return Some(Expr::Match {
                scrutinee: Some(Box::new(expr)),
                arms,
                span,
            });
        }
        // Legacy `?` support with deprecation warning
        if self.consume_symbol("?") {
            self.emit_diag(
                "W1601",
                "`?` for pattern matching is deprecated; use `match` instead",
                self.previous_span(),
            );
            let mut arms = Vec::new();
            loop {
                self.consume_newlines();
                if !self.consume_symbol("|") {
                    break;
                }
                let pattern = self
                    .parse_pattern()
                    .unwrap_or(Pattern::Wildcard(expr_span(&expr)));
                let guard = if self.match_keyword("when") {
                    self.parse_guard_expr()
                } else {
                    None
                };
                self.expect_symbol("=>", "expected '=>' in match arm");
                let body = self.parse_expr().unwrap_or(Expr::Raw {
                    text: String::new(),
                    span: expr_span(&expr),
                });
                let span = merge_span(pattern_span(&pattern), expr_span(&body));
                arms.push(MatchArm {
                    pattern,
                    guard,
                    body,
                    span,
                });
            }
            let span = merge_span(
                expr_span(&expr),
                arms.last()
                    .map(|arm| arm.span.clone())
                    .unwrap_or(expr_span(&expr)),
            );
            return Some(Expr::Match {
                scrutinee: Some(Box::new(expr)),
                arms,
                span,
            });
        }
        Some(expr)
    }
    fn parse_binary(&mut self, min_prec: u8) -> Option<Expr> {
        let mut left = self.parse_application()?;
        while let Some(op) = self.peek_symbol_text() {
            let prec = binary_prec(&op);
            if prec < min_prec || prec == 0 {
                break;
            }
            self.pos += 1;
            let right = self.parse_binary(prec + 1)?;
            let span = merge_span(expr_span(&left), expr_span(&right));
            left = Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
                span,
            };
        }
        Some(left)
    }

    fn parse_guard_expr(&mut self) -> Option<Expr> {
        self.consume_newlines();
        self.parse_binary(0)
    }

    /// Returns `true` when `expr` is something that can syntactically appear
    /// as the head of a juxtaposition-style function application (`f x y`).
    fn is_callable_head(expr: &Expr) -> bool {
        matches!(
            expr,
            Expr::Ident(_)
                | Expr::Call { .. }
                | Expr::FieldAccess { .. }
                | Expr::FieldSection { .. }
                | Expr::Index { .. }
                | Expr::Lambda { .. }
                | Expr::Block { .. }
        )
    }

    fn parse_application(&mut self) -> Option<Expr> {
        let mut expr = self.parse_postfix()?;
        // Only collect juxtaposition arguments when the head expression is
        // something that could plausibly be called — identifiers, field access,
        // indexing, lambdas, blocks, etc.  Literals, lists, records, tuples,
        // if-expressions and match-expressions are never callable; treating
        // adjacent tokens as arguments would hide missing-comma diagnostics
        // inside [list] and {record} contexts and produce confusing ASTs like
        // `Call(1, [2])` for `[1 2]`.
        if !Self::is_callable_head(&expr) {
            return Some(expr);
        }
        let mut args = Vec::new();
        while self.is_expr_start()
            || (self.is_negative_number_literal_start()
                && self
                    .peek_span()
                    .is_some_and(|span| !is_adjacent(&expr_span(&expr), &span)))
        {
            // Don't consume tokens that start at column 1 on a new line – they
            // are almost certainly new top-level definitions rather than
            // continuation arguments.  This is especially important for error
            // recovery when unclosed delimiters cause `consume_newlines()` to
            // skip past line boundaries inside tuple/group parsing.
            if let Some(next_span) = self.peek_span() {
                let last_line = if let Some(last_arg) = args.last() {
                    expr_span(last_arg).end.line
                } else {
                    expr_span(&expr).end.line
                };
                if next_span.start.line > last_line && next_span.start.column == 1 {
                    break;
                }
            }
            let arg = self.parse_postfix()?;
            args.push(arg);
        }
        if args.is_empty() {
            return Some(expr);
        }
        let span = merge_span(expr_span(&expr), expr_span(args.last().unwrap()));
        expr = Expr::Call {
            func: Box::new(expr),
            args,
            span,
        };
        Some(expr)
    }
    fn parse_postfix(&mut self) -> Option<Expr> {
        let mut expr = self.parse_primary()?;
        loop {
            if self.peek_symbol("(") {
                if let Some(span) = self.peek_span() {
                    if is_adjacent(&expr_span(&expr), &span) {
                        self.consume_symbol("(");
                        let mut args = Vec::new();
                        while !self.check_symbol(")") && self.pos < self.tokens.len() {
                            if let Some(arg) = self.parse_expr() {
                                args.push(arg);
                            }
                            if !self.consume_symbol(",") {
                                break;
                            }
                        }
                        let end = self.expect_symbol(")", "expected ')' to close call");
                        let span = merge_span(expr_span(&expr), end.unwrap_or(expr_span(&expr)));
                        expr = Expr::Call {
                            func: Box::new(expr),
                            args,
                            span,
                        };
                        continue;
                    }
                }
            }
            if self.peek_symbol("[") {
                if let Some(span) = self.peek_span() {
                    if is_adjacent(&expr_span(&expr), &span) {
                        // `_` is the placeholder used heavily in patching; `_["x"]` is almost
                        // always meant as `_ ["x"]` (a list literal argument) rather than an
                        // index expression. Let the application parser treat `["x"]` as a
                        // separate expression/argument.
                        if matches!(&expr, Expr::Ident(name) if name.name == "_") {
                            break;
                        }

                        // Similarly, `"users"[]` in stdlib examples is intended as a second
                        // argument `[]` (an empty list literal), not an index on the string.
                        if matches!(&expr, Expr::Literal(Literal::String { .. }))
                            && self
                                .tokens
                                .get(self.pos + 1)
                                .is_some_and(|tok| tok.kind == TokenKind::Symbol && tok.text == "]")
                        {
                            break;
                        }

                        self.consume_symbol("[");
                        self.consume_newlines();
                        let spread = self.consume_symbol("...");
                        let base_allows_single_bracket_call =
                            matches!(expr, Expr::FieldAccess { .. });

                        // Empty bracket-list call: `f[]` => `f []`
                        if self.check_symbol("]") && base_allows_single_bracket_call {
                            let end = self
                                .expect_symbol("]", "expected ']' to close bracket list")
                                .unwrap_or_else(|| expr_span(&expr));
                            let list = Expr::List {
                                items: Vec::new(),
                                span: end.clone(),
                            };
                            let span = merge_span(expr_span(&expr), end);
                            expr = Expr::Call {
                                func: Box::new(expr),
                                args: vec![list],
                                span,
                            };
                            continue;
                        }

                        let first = self.parse_expr().unwrap_or_else(|| {
                            let span = self.peek_span().unwrap_or_else(|| expr_span(&expr));
                            self.emit_diag(
                                "E1529",
                                "expected expression inside brackets",
                                span.clone(),
                            );
                            Expr::Raw {
                                text: String::new(),
                                span,
                            }
                        });
                        let first_span = expr_span(&first);
                        self.consume_newlines();

                        // `base[index]` (single expr) vs `base[ a, b, c ]` (bracket-list call)
                        if self.consume_symbol(",") {
                            let mut items = vec![ListItem {
                                expr: first,
                                spread,
                                span: first_span.clone(),
                            }];
                            self.consume_newlines();
                            while !self.check_symbol("]") && self.pos < self.tokens.len() {
                                let spread = self.consume_symbol("...");
                                if let Some(item_expr) = self.parse_expr() {
                                    let span = expr_span(&item_expr);
                                    items.push(ListItem {
                                        expr: item_expr,
                                        spread,
                                        span,
                                    });
                                }
                                self.consume_newlines();
                                if !self.consume_symbol(",") {
                                    break;
                                }
                                self.consume_newlines();
                            }
                            let end = self.expect_symbol("]", "expected ']' to close bracket list");
                            let list_span = merge_span(
                                first_span.clone(),
                                end.unwrap_or_else(|| first_span.clone()),
                            );
                            let list = Expr::List {
                                items,
                                span: list_span.clone(),
                            };
                            let span = merge_span(expr_span(&expr), list_span);
                            expr = Expr::Call {
                                func: Box::new(expr),
                                args: vec![list],
                                span,
                            };
                        } else if base_allows_single_bracket_call {
                            // Single-element bracket-list call: `f[x]` => `f [x]`
                            let end = self.expect_symbol("]", "expected ']' to close bracket list");
                            let list_span = merge_span(
                                first_span.clone(),
                                end.unwrap_or_else(|| first_span.clone()),
                            );
                            let list = Expr::List {
                                items: vec![ListItem {
                                    expr: first,
                                    spread,
                                    span: first_span.clone(),
                                }],
                                span: list_span.clone(),
                            };
                            let span = merge_span(expr_span(&expr), list_span);
                            expr = Expr::Call {
                                func: Box::new(expr),
                                args: vec![list],
                                span,
                            };
                        } else {
                            let end = self.expect_symbol("]", "expected ']' after index");
                            let span =
                                merge_span(expr_span(&expr), end.unwrap_or(expr_span(&expr)));
                            expr = Expr::Index {
                                base: Box::new(expr),
                                index: Box::new(first),
                                span,
                            };
                        }
                        continue;
                    }
                }
            }
            if self.consume_symbol(".") {
                if let Some(field) = self.consume_ident() {
                    let span = merge_span(expr_span(&expr), field.span.clone());
                    expr = Expr::FieldAccess {
                        base: Box::new(expr),
                        field,
                        span,
                    };
                    continue;
                }
            }
            break;
        }
        Some(expr)
    }
}
