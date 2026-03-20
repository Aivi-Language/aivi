impl Parser {
    fn parse_block(&mut self, kind: BlockKind) -> Expr {
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
                self.emit_diag(
                    "E1624",
                    "`loop` was removed in AIVI v0.2; use plain recursion or `@|>`/`recurse` flow anchors",
                    loop_start.clone(),
                );
                let _pattern = self.parse_pattern().unwrap_or(Pattern::Wildcard(loop_start.clone()));
                self.expect_symbol("=", "expected '=' in loop binding");
                self.consume_newlines();
                let init = self.parse_match_or_binary().unwrap_or(Expr::Raw {
                    text: String::new(),
                    span: loop_start.clone(),
                });
                self.expect_symbol("=>", "expected '=>' in loop binding");
                let body = self.parse_expr().unwrap_or(Expr::Raw {
                    text: String::new(),
                    span: loop_start.clone(),
                });
                let span = merge_span(loop_start, expr_span(&body));
                items.push(BlockItem::Expr {
                    expr: Expr::Tuple {
                        items: vec![init, body],
                        span: span.clone(),
                    },
                    span,
                });
                continue;
            }
            if self.match_keyword("yield") {
                let yield_kw = self.previous_span();
                self.emit_diag(
                    "E1625",
                    "`yield` was removed in AIVI v0.2; use list literals, list transforms, or `*|>` fan-out",
                    yield_kw.clone(),
                );
                let expr = self.parse_expr().unwrap_or(Expr::Raw {
                    text: String::new(),
                    span: yield_kw.clone(),
                });
                let span = merge_span(yield_kw, expr_span(&expr));
                items.push(BlockItem::Expr { expr, span });
                continue;
            }
            if self.match_keyword("recurse") {
                let recurse_kw = self.previous_span();
                if !matches!(kind, BlockKind::Do { .. }) {
                    self.emit_diag(
                        "E1535",
                        "`recurse` is only allowed inside effect-style blocks (within legacy `do` loops) or flow bodies",
                        recurse_kw.clone(),
                    );
                }
                let expr = self.parse_expr().unwrap_or(Expr::Raw {
                    text: String::new(),
                    span: recurse_kw.clone(),
                });
                let span = merge_span(recurse_kw, expr_span(&expr));
                if matches!(kind, BlockKind::Do { .. }) {
                    items.push(BlockItem::Recurse { expr, span });
                } else {
                    items.push(BlockItem::Expr { expr, span });
                }
                continue;
            }
            // `when cond <- eff` — conditional effect (Change 6)
            if self.match_keyword("when") {
                let when_kw = self.previous_span();
                if !matches!(kind, BlockKind::Do { ref monad } if monad.name == "Effect") {
                    self.emit_diag(
                        "E1540",
                        "`when` is only allowed inside effect-style blocks",
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
                if !matches!(kind, BlockKind::Do { ref monad } if monad.name == "Effect") {
                    self.emit_diag(
                        "E1543",
                        "`unless` is only valid inside an effect-style block",
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
                if !matches!(kind, BlockKind::Do { ref monad } if monad.name == "Effect") {
                    self.emit_diag(
                        "E1541",
                        "`given` is only allowed inside effect-style blocks",
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
                            guard_negated: false,
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
                        BlockKind::Do { .. }
                    ) {
                        self.emit_diag(
                            "E1536",
                            "`<-` is only allowed inside `do Monad { ... }` blocks",
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
                    self.emit_diag(
                        "E1626",
                        "block guard statements were removed in AIVI v0.2; use flow guards or collection filters instead",
                        pattern_span(&pattern),
                    );
                    let expr = self.parse_expr().unwrap_or(Expr::Raw {
                        text: String::new(),
                        span: pattern_span(&pattern),
                    });
                    let span = merge_span(pattern_span(&pattern), expr_span(&expr));
                    items.push(BlockItem::Expr { expr, span });
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
            guard_negated: false,
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
                guard_negated: false,
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
                    guard_negated: false,
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
                guard_negated: false,
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
}

