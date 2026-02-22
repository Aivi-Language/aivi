impl Parser {
    fn parse_literal_type_sig(&mut self, decorators: Vec<Decorator>) -> Option<TypeSig> {
        self.reject_debug_decorators(&decorators, "type signatures");
        let start = self.pos;
        let number = self.consume_number()?;
        let suffix = if let Some(suffix) = self.consume_ident() {
            Some(suffix)
        } else if self.check_symbol("%") {
            let token = self.tokens.get(self.pos)?.clone();
            self.pos += 1;
            Some(SpannedName {
                name: "%".into(),
                span: token.span,
            })
        } else {
            None
        };
        let Some(suffix) = suffix else {
            self.pos = start;
            return None;
        };
        if !self.consume_symbol(":") {
            self.pos = start;
            return None;
        }

        let name_span = merge_span(number.span.clone(), suffix.span.clone());
        let name = SpannedName {
            name: format!("{}{}", number.text, suffix.name).into(),
            span: name_span.clone(),
        };
        let ty = self.parse_type_expr().unwrap_or(TypeExpr::Unknown {
            span: name_span.clone(),
        });
        let span = merge_span(name_span, type_span(&ty));

        // Same rule as `parse_type_sig`: signatures must be standalone.
        if let Some(next) = self.tokens.get(self.pos) {
            let same_line = next.span.start.line == span.end.line;
            let allowed_terminator = next.kind == TokenKind::Newline
                || (next.kind == TokenKind::Symbol && next.text == "}");
            if same_line && !allowed_terminator {
                let next_span = next.span.clone();
                let line = next.span.start.line;
                self.emit_diag(
                    "E1528",
                    "type signatures must be written on their own line (write `name = ...` on the next line)",
                    merge_span(span.clone(), next_span.clone()),
                );
                while self.pos < self.tokens.len() {
                    let tok = &self.tokens[self.pos];
                    if tok.kind == TokenKind::Newline
                        || (tok.kind == TokenKind::Symbol && tok.text == "}")
                        || tok.span.start.line != line
                    {
                        break;
                    }
                    self.pos += 1;
                }
            }
        }

        Some(TypeSig {
            decorators,
            name,
            ty,
            span,
        })
    }

    fn parse_literal_def(&mut self, decorators: Vec<Decorator>) -> Option<Def> {
        let start = self.pos;
        let number = self.consume_number()?;
        let suffix = if let Some(suffix) = self.consume_ident() {
            Some(suffix)
        } else if self.check_symbol("%") {
            let token = self.tokens.get(self.pos)?.clone();
            self.pos += 1;
            Some(SpannedName {
                name: "%".into(),
                span: token.span,
            })
        } else {
            None
        };
        let Some(suffix) = suffix else {
            self.pos = start;
            return None;
        };
        self.expect_symbol("=", "expected '=' after domain literal");
        let expr = self.parse_expr().unwrap_or(Expr::Raw {
            text: String::new(),
            span: number.span.clone(),
        });

        // `1suffix = n => ...` already defines the literal template explicitly.
        // Only synthesize a template parameter for the shorthand form `1suffix = <expr-with-1>`.
        if matches!(expr, Expr::Lambda { .. }) {
            let name_span = merge_span(number.span.clone(), suffix.span.clone());
            let name = SpannedName {
                name: format!("{}{}", number.text, suffix.name).into(),
                span: name_span.clone(),
            };
            let span = merge_span(name_span, expr_span(&expr));
            return Some(Def {
                decorators,
                name,
                params: Vec::new(),
                expr,
                span,
            });
        }

        fn rewrite_literal_template(expr: Expr, needle: &str, param: &str) -> Expr {
            match expr {
                Expr::Literal(Literal::Number { text, span }) if text == needle => {
                    Expr::Ident(SpannedName {
                        name: param.into(),
                        span,
                    })
                }
                Expr::List { items, span } => Expr::List {
                    items: items
                        .into_iter()
                        .map(|item| ListItem {
                            expr: rewrite_literal_template(item.expr, needle, param),
                            spread: item.spread,
                            span: item.span,
                        })
                        .collect(),
                    span,
                },
                Expr::Tuple { items, span } => Expr::Tuple {
                    items: items
                        .into_iter()
                        .map(|item| rewrite_literal_template(item, needle, param))
                        .collect(),
                    span,
                },
                Expr::Record { fields, span } => Expr::Record {
                    fields: fields
                        .into_iter()
                        .map(|field| RecordField {
                            spread: field.spread,
                            path: field.path,
                            value: rewrite_literal_template(field.value, needle, param),
                            span: field.span,
                        })
                        .collect(),
                    span,
                },
                Expr::PatchLit { fields, span } => Expr::PatchLit {
                    fields: fields
                        .into_iter()
                        .map(|field| RecordField {
                            spread: field.spread,
                            path: field.path,
                            value: rewrite_literal_template(field.value, needle, param),
                            span: field.span,
                        })
                        .collect(),
                    span,
                },
                Expr::FieldAccess { base, field, span } => Expr::FieldAccess {
                    base: Box::new(rewrite_literal_template(*base, needle, param)),
                    field,
                    span,
                },
                Expr::Index { base, index, span } => Expr::Index {
                    base: Box::new(rewrite_literal_template(*base, needle, param)),
                    index: Box::new(rewrite_literal_template(*index, needle, param)),
                    span,
                },
                Expr::Call { func, args, span } => Expr::Call {
                    func: Box::new(rewrite_literal_template(*func, needle, param)),
                    args: args
                        .into_iter()
                        .map(|arg| rewrite_literal_template(arg, needle, param))
                        .collect(),
                    span,
                },
                Expr::Lambda { params, body, span } => Expr::Lambda {
                    params,
                    body: Box::new(rewrite_literal_template(*body, needle, param)),
                    span,
                },
                Expr::Match {
                    scrutinee,
                    arms,
                    span,
                } => Expr::Match {
                    scrutinee: scrutinee.map(|scrutinee| {
                        Box::new(rewrite_literal_template(*scrutinee, needle, param))
                    }),
                    arms: arms
                        .into_iter()
                        .map(|arm| MatchArm {
                            pattern: arm.pattern,
                            guard: arm
                                .guard
                                .map(|guard| rewrite_literal_template(guard, needle, param)),
                            body: rewrite_literal_template(arm.body, needle, param),
                            span: arm.span,
                        })
                        .collect(),
                    span,
                },
                Expr::If {
                    cond,
                    then_branch,
                    else_branch,
                    span,
                } => Expr::If {
                    cond: Box::new(rewrite_literal_template(*cond, needle, param)),
                    then_branch: Box::new(rewrite_literal_template(*then_branch, needle, param)),
                    else_branch: Box::new(rewrite_literal_template(*else_branch, needle, param)),
                    span,
                },
                Expr::Binary {
                    op,
                    left,
                    right,
                    span,
                } => Expr::Binary {
                    op,
                    left: Box::new(rewrite_literal_template(*left, needle, param)),
                    right: Box::new(rewrite_literal_template(*right, needle, param)),
                    span,
                },
                Expr::Block { kind, items, span } => Expr::Block {
                    kind,
                    items: items
                        .into_iter()
                        .map(|item| match item {
                            BlockItem::Bind {
                                pattern,
                                expr,
                                span,
                            } => BlockItem::Bind {
                                pattern,
                                expr: rewrite_literal_template(expr, needle, param),
                                span,
                            },
                            BlockItem::Let {
                                pattern,
                                expr,
                                span,
                            } => BlockItem::Let {
                                pattern,
                                expr: rewrite_literal_template(expr, needle, param),
                                span,
                            },
                            BlockItem::Filter { expr, span } => BlockItem::Filter {
                                expr: rewrite_literal_template(expr, needle, param),
                                span,
                            },
                            BlockItem::Yield { expr, span } => BlockItem::Yield {
                                expr: rewrite_literal_template(expr, needle, param),
                                span,
                            },
                            BlockItem::Recurse { expr, span } => BlockItem::Recurse {
                                expr: rewrite_literal_template(expr, needle, param),
                                span,
                            },
                            BlockItem::Expr { expr, span } => BlockItem::Expr {
                                expr: rewrite_literal_template(expr, needle, param),
                                span,
                            },
                            BlockItem::When { cond, effect, span } => BlockItem::When {
                                cond: rewrite_literal_template(cond, needle, param),
                                effect: rewrite_literal_template(effect, needle, param),
                                span,
                            },
                            BlockItem::Unless { cond, effect, span } => BlockItem::Unless {
                                cond: rewrite_literal_template(cond, needle, param),
                                effect: rewrite_literal_template(effect, needle, param),
                                span,
                            },
                            BlockItem::Given { cond, fail_expr, span } => BlockItem::Given {
                                cond: rewrite_literal_template(cond, needle, param),
                                fail_expr: rewrite_literal_template(fail_expr, needle, param),
                                span,
                            },
                            BlockItem::On { transition, handler, span } => BlockItem::On {
                                transition: rewrite_literal_template(transition, needle, param),
                                handler: rewrite_literal_template(handler, needle, param),
                                span,
                            },
                        })
                        .collect(),
                    span,
                },
                other => other,
            }
        }

        let param = format!("__lit_{}", suffix.name);
        let expr = rewrite_literal_template(expr, &number.text, &param);

        let name_span = merge_span(number.span.clone(), suffix.span.clone());
        let name = SpannedName {
            name: format!("{}{}", number.text, suffix.name).into(),
            span: name_span.clone(),
        };
        let span = merge_span(name_span, expr_span(&expr));
        Some(Def {
            decorators,
            name,
            params: vec![Pattern::Ident(SpannedName {
                name: param,
                span: number.span.clone(),
            })],
            expr,
            span,
        })
    }

    fn parse_def(&mut self, decorators: Vec<Decorator>) -> Option<Def> {
        self.consume_newlines();
        let name = self.consume_name()?;
        self.consume_newlines();

        // v0.1 surface: parameters must be written as an explicit lambda on the RHS:
        //   f = x y => ...
        //
        // For error recovery (LSP), we still recognize the legacy form:
        //   f x y = ...
        // but emit a hard diagnostic and desugar it to the explicit lambda.
        let (params, expr) = if self.check_symbol("=") {
            self.expect_symbol("=", "expected '=' in definition");
            self.consume_newlines();
            let expr = self.parse_expr().unwrap_or(Expr::Raw {
                text: String::new(),
                span: name.span.clone(),
            });
            (Vec::new(), expr)
        } else if self.is_pattern_start() {
            let start_span = name.span.clone();
            let mut legacy_params = Vec::new();
            while {
                self.consume_newlines();
                !self.check_symbol("=") && self.pos < self.tokens.len()
            } {
                if let Some(pattern) = self.parse_pattern() {
                    legacy_params.push(pattern);
                    continue;
                }
                break;
            }
            self.consume_newlines();
            self.expect_symbol("=", "expected '=' in definition");
            self.consume_newlines();
            let body = self.parse_expr().unwrap_or(Expr::Raw {
                text: String::new(),
                span: start_span.clone(),
            });

            let legacy_span = merge_span(start_span, expr_span(&body));
            self.emit_diag(
                "E1539",
                "function parameters must be written after '=' (use `f = x y => ...`)",
                legacy_span.clone(),
            );

            let expr = Expr::Lambda {
                params: legacy_params.clone(),
                body: Box::new(body),
                span: legacy_span,
            };
            (Vec::new(), expr)
        } else {
            self.expect_symbol("=", "expected '=' in definition");
            self.consume_newlines();
            let expr = self.parse_expr().unwrap_or(Expr::Raw {
                text: String::new(),
                span: name.span.clone(),
            });
            (Vec::new(), expr)
        };

        let span = merge_span(name.span.clone(), expr_span(&expr));
        Some(Def {
            decorators,
            name,
            params,
            expr,
            span,
        })
    }
}
