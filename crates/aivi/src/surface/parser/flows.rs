const FLOW_OPERATOR_TEXTS: &[&str] = &[
    "|>", "~|>", ">|>", "?|>", "!|>", "||>", "*|>", "*-|", "&|>", "@|>",
];

impl Parser {
    fn parse_flow_suffix_after(&mut self, root: Expr, min_alignment: usize) -> Expr {
        let checkpoint = self.pos;
        let mut saw_newline = false;
        while self.peek_newline() {
            self.pos += 1;
            saw_newline = true;
        }
        if !saw_newline {
            return root;
        }

        let Some((op, span)) = self.peek_flow_operator() else {
            self.pos = checkpoint;
            return root;
        };

        let root_span = expr_span(&root);
        let alignment = flow_operator_alignment(op, &span);
        if alignment <= min_alignment {
            self.pos = checkpoint;
            return root;
        }

        let lines = self.parse_flow_lines(alignment);
        if lines.is_empty() {
            self.pos = checkpoint;
            return root;
        }

        let end_span = flow_lines_last_span(&lines).unwrap_or_else(|| root_span.clone());
        Expr::Flow {
            root: Box::new(root),
            lines,
            span: merge_span(root_span, end_span),
        }
    }

    fn parse_flow_lines(&mut self, alignment: usize) -> Vec<FlowLine> {
        let mut lines = Vec::new();
        loop {
            let Some((op, span)) = self.peek_flow_operator() else {
                break;
            };
            if flow_operator_alignment(op, &span) != alignment {
                break;
            }
            lines.push(self.parse_flow_line(span.start.column));

            let checkpoint = self.pos;
            let mut saw_newline = false;
            while self.peek_newline() {
                self.pos += 1;
                saw_newline = true;
            }
            if !saw_newline {
                break;
            }
            let Some((next_op, next_span)) = self.peek_flow_operator() else {
                self.pos = checkpoint;
                break;
            };
            if flow_operator_alignment(next_op, &next_span) != alignment {
                self.pos = checkpoint;
                break;
            }
        }
        lines
    }

    fn parse_flow_line(&mut self, _column: usize) -> FlowLine {
        let op_token = self
            .tokens
            .get(self.pos)
            .cloned()
            .expect("flow line requires operator token");
        self.pos += 1;
        let outer_alignment = flow_operator_alignment(op_token.text.as_str(), &op_token.span);

        match op_token.text.as_str() {
            "@|>" => {
                let name = self.consume_ident().unwrap_or_else(|| {
                    let span = self.peek_span().unwrap_or_else(|| op_token.span.clone());
                    self.emit_diag("E1602", "expected anchor name after '@|>'", span.clone());
                    SpannedName {
                        name: "__missing_anchor".into(),
                        span,
                    }
                });
                let span = merge_span(op_token.span, name.span.clone());
                FlowLine::Anchor(FlowAnchor { name, span })
            }
            ">|>" => {
                let predicate = self
                    .parse_expr_without_result_or_with_min_flow_alignment(outer_alignment)
                    .unwrap_or(Expr::Raw {
                        text: String::new(),
                        span: op_token.span.clone(),
                    });
                let line_no = expr_span(&predicate).end.line;
                let fail_expr = if self.on_same_line(line_no) && self.match_keyword("or") {
                    match self.consume_ident() {
                        Some(name) if name.name == "fail" => {}
                        Some(name) => self.emit_diag(
                            "E1603",
                            "expected 'fail' after 'or' in guard line",
                            name.span.clone(),
                        ),
                        None => {
                            let span = self.peek_span().unwrap_or_else(|| self.previous_span());
                            self.emit_diag(
                                "E1603",
                                "expected 'fail' after 'or' in guard line",
                                span,
                            );
                        }
                    }
                    Some(
                        self.parse_expr_without_result_or_with_min_flow_alignment(outer_alignment)
                            .unwrap_or(Expr::Raw {
                                text: String::new(),
                                span: self.previous_span(),
                            }),
                    )
                } else {
                    None
                };
                let end = fail_expr
                    .as_ref()
                    .map(expr_span)
                    .unwrap_or_else(|| expr_span(&predicate));
                FlowLine::Guard(FlowGuard {
                    predicate,
                    fail_expr,
                    span: merge_span(op_token.span, end),
                })
            }
            "!|>" | "||>" => {
                let pattern = self
                    .parse_pattern()
                    .unwrap_or(Pattern::Wildcard(op_token.span.clone()));
                let (guard, guard_negated) = self.parse_optional_match_guard();
                self.expect_symbol("=>", "expected '=>' in flow arm");
                let body = self
                    .parse_expr_without_result_or_with_min_flow_alignment(outer_alignment)
                    .unwrap_or(Expr::Raw {
                        text: String::new(),
                        span: op_token.span.clone(),
                    });
                let span = merge_span(op_token.span, expr_span(&body));
                let arm = FlowArm {
                    pattern,
                    guard,
                    guard_negated,
                    body,
                    span,
                };
                if op_token.text == "!|>" {
                    FlowLine::Recover(arm)
                } else {
                    FlowLine::Branch(arm)
                }
            }
            "|>" | "~|>" | "?|>" | "*|>" | "&|>" => {
                let expr = self
                    .parse_expr_without_result_or_with_min_flow_alignment(outer_alignment)
                    .unwrap_or(Expr::Raw {
                        text: String::new(),
                        span: op_token.span.clone(),
                    });
                let line_no = expr_span(&expr).end.line;
                let modifiers = self.parse_flow_modifiers(line_no);
                let binding = self.parse_flow_binding(line_no);
                let (subflow, fanout_end) = if op_token.text == "*|>" {
                    self.parse_fanout_subflow(outer_alignment)
                } else {
                    (Vec::new(), None)
                };
                let mut end = binding
                    .as_ref()
                    .map(|binding| binding.span.clone())
                    .unwrap_or_else(|| {
                        modifiers
                            .last()
                            .map(flow_modifier_span)
                            .unwrap_or_else(|| expr_span(&expr))
                    });
                if let Some(subflow_end) = fanout_end.or_else(|| flow_lines_last_span(&subflow)) {
                    end = subflow_end;
                }
                let kind = match op_token.text.as_str() {
                    "|>" => FlowStepKind::Flow,
                    "~|>" => FlowStepKind::Tap,
                    "?|>" => FlowStepKind::Attempt,
                    "*|>" => FlowStepKind::FanOut,
                    "&|>" => FlowStepKind::Applicative,
                    _ => unreachable!("validated flow operator"),
                };
                FlowLine::Step(FlowStep {
                    kind,
                    expr,
                    modifiers,
                    binding,
                    subflow,
                    span: merge_span(op_token.span, end),
                })
            }
            "*-|" => {
                self.emit_diag(
                    "E1622",
                    "unexpected `*-|` outside a `*|>` fan-out block",
                    op_token.span.clone(),
                );
                FlowLine::Step(FlowStep {
                    kind: FlowStepKind::Flow,
                    expr: Expr::Raw {
                        text: String::new(),
                        span: op_token.span.clone(),
                    },
                    modifiers: Vec::new(),
                    binding: None,
                    subflow: Vec::new(),
                    span: op_token.span,
                })
            }
            other => {
                self.emit_diag(
                    "E1604",
                    &format!("unsupported flow operator '{other}'"),
                    op_token.span.clone(),
                );
                FlowLine::Step(FlowStep {
                    kind: FlowStepKind::Flow,
                    expr: Expr::Raw {
                        text: String::new(),
                        span: op_token.span.clone(),
                    },
                    modifiers: Vec::new(),
                    binding: None,
                    subflow: Vec::new(),
                    span: op_token.span,
                })
            }
        }
    }

    fn parse_fanout_subflow(&mut self, alignment: usize) -> (Vec<FlowLine>, Option<Span>) {
        let mut lines = Vec::new();
        loop {
            let checkpoint = self.pos;
            let mut saw_newline = false;
            while self.peek_newline() {
                self.pos += 1;
                saw_newline = true;
            }
            if !saw_newline {
                let span = self.peek_span().unwrap_or_else(|| self.previous_span());
                self.emit_diag(
                    "E1621",
                    "expected `*-|` to end this `*|>` fan-out block",
                    span,
                );
                return (lines, None);
            }
            let Some(token) = self.tokens.get(self.pos).cloned() else {
                self.emit_diag(
                    "E1621",
                    "expected `*-|` to end this `*|>` fan-out block",
                    self.previous_span(),
                );
                return (lines, None);
            };
            if token.kind == TokenKind::Symbol
                && token.text == "*-|"
                && flow_operator_alignment(token.text.as_str(), &token.span) == alignment
            {
                self.pos += 1;
                return (lines, Some(token.span));
            }
            let Some((op, span)) = self.peek_flow_operator() else {
                self.emit_diag(
                    "E1621",
                    "expected `*-|` to end this `*|>` fan-out block",
                    token.span.clone(),
                );
                self.pos = checkpoint;
                return (lines, None);
            };
            if flow_operator_alignment(op, &span) != alignment {
                self.emit_diag(
                    "E1621",
                    "expected `*-|` to end this `*|>` fan-out block",
                    span.clone(),
                );
                self.pos = checkpoint;
                return (lines, None);
            }
            lines.push(self.parse_flow_line(span.start.column));
        }
    }

    fn parse_flow_modifiers(&mut self, line: usize) -> Vec<FlowModifier> {
        let mut modifiers = Vec::new();
        while self.on_same_line(line) && self.consume_symbol("@") {
            let Some(name) = self.consume_ident() else {
                let span = self.peek_span().unwrap_or_else(|| self.previous_span());
                self.emit_diag("E1605", "expected flow modifier name after '@'", span);
                break;
            };
            let modifier = match name.name.as_str() {
                "timeout" => {
                    let duration = self.parse_expr_without_result_or().unwrap_or(Expr::Raw {
                        text: String::new(),
                        span: name.span.clone(),
                    });
                    let span = merge_span(name.span.clone(), expr_span(&duration));
                    FlowModifier::Timeout { duration, span }
                }
                "delay" => {
                    let duration = self.parse_expr_without_result_or().unwrap_or(Expr::Raw {
                        text: String::new(),
                        span: name.span.clone(),
                    });
                    let span = merge_span(name.span.clone(), expr_span(&duration));
                    FlowModifier::Delay { duration, span }
                }
                "concurrent" => {
                    let limit = self.parse_expr_without_result_or().unwrap_or(Expr::Raw {
                        text: String::new(),
                        span: name.span.clone(),
                    });
                    let span = merge_span(name.span.clone(), expr_span(&limit));
                    FlowModifier::Concurrent { limit, span }
                }
                "retry" => self.parse_flow_retry_modifier(name),
                "cleanup" => {
                    let expr = self.parse_expr_without_result_or().unwrap_or(Expr::Raw {
                        text: String::new(),
                        span: name.span.clone(),
                    });
                    let span = merge_span(name.span.clone(), expr_span(&expr));
                    FlowModifier::Cleanup { expr, span }
                }
                _ => {
                    self.emit_diag(
                        "E1606",
                        &format!("unknown flow modifier '@{}'", name.name),
                        name.span.clone(),
                    );
                    let span = name.span.clone();
                    FlowModifier::Cleanup {
                        expr: Expr::Raw {
                            text: String::new(),
                            span: span.clone(),
                        },
                        span,
                    }
                }
            };
            modifiers.push(modifier);
        }
        modifiers
    }

    fn parse_flow_retry_modifier(&mut self, name: SpannedName) -> FlowModifier {
        let attempts_token = self.consume_number();
        let attempts = attempts_token
            .as_ref()
            .and_then(|token| token.text.parse::<u32>().ok())
            .unwrap_or(0);
        let attempts_span = attempts_token
            .as_ref()
            .map(|token| token.span.clone())
            .unwrap_or_else(|| name.span.clone());
        let suffix = attempts_token
            .as_ref()
            .and_then(|token| self.consume_adjacent_suffix(&token.span));
        let bad_suffix = match suffix {
            Some(ref suffix) => suffix.text != "x",
            None => true,
        };
        if bad_suffix {
            self.emit_diag(
                "E1607",
                "expected retry count in the form '<count>x'",
                attempts_span.clone(),
            );
        }
        let interval = self.parse_expr_without_result_or().unwrap_or(Expr::Raw {
            text: String::new(),
            span: attempts_span.clone(),
        });
        let exponential = if self.on_same_line(expr_span(&interval).end.line) {
            self.match_keyword("exp")
        } else {
            false
        };
        let end_span = if exponential {
            self.previous_span()
        } else {
            expr_span(&interval)
        };
        FlowModifier::Retry {
            attempts,
            interval,
            exponential,
            span: merge_span(name.span, end_span),
        }
    }

    fn parse_flow_binding(&mut self, line: usize) -> Option<FlowBinding> {
        if !self.on_same_line(line) || !self.consume_symbol("#") {
            return None;
        }
        let name = self.consume_ident().unwrap_or_else(|| {
            let span = self.peek_span().unwrap_or_else(|| self.previous_span());
            self.emit_diag("E1608", "expected binding name after '#'", span.clone());
            SpannedName {
                name: "__missing_binding".into(),
                span,
            }
        });
        let mut span = name.span.clone();
        if self.on_same_line(line) && self.consume_symbol("!") {
            let bang_span = self.previous_span();
            span = merge_span(name.span.clone(), bang_span.clone());
            self.emit_diag(
                "E1620",
                "`#name!` was removed; bind with `#name` and choose the next subject explicitly",
                span.clone(),
            );
        }
        Some(FlowBinding { name, span })
    }

    fn peek_flow_operator(&self) -> Option<(&'static str, Span)> {
        let token = self.tokens.get(self.pos)?;
        if token.kind != TokenKind::Symbol {
            return None;
        }
        let op = FLOW_OPERATOR_TEXTS
            .iter()
            .copied()
            .find(|candidate| *candidate == token.text)?;
        Some((op, token.span.clone()))
    }

    fn on_same_line(&self, line: usize) -> bool {
        self.peek_span().is_some_and(|span| span.start.line == line)
    }
}

fn flow_modifier_span(modifier: &FlowModifier) -> Span {
    match modifier {
        FlowModifier::Timeout { span, .. }
        | FlowModifier::Delay { span, .. }
        | FlowModifier::Concurrent { span, .. }
        | FlowModifier::Retry { span, .. }
        | FlowModifier::Cleanup { span, .. } => span.clone(),
    }
}

fn flow_line_span(line: &FlowLine) -> Span {
    match line {
        FlowLine::Step(step) => step.span.clone(),
        FlowLine::Guard(guard) => guard.span.clone(),
        FlowLine::Branch(arm) | FlowLine::Recover(arm) => arm.span.clone(),
        FlowLine::Anchor(anchor) => anchor.span.clone(),
    }
}

fn flow_lines_last_span(lines: &[FlowLine]) -> Option<Span> {
    lines.last().map(flow_line_span)
}

fn flow_operator_alignment(op: &str, span: &Span) -> usize {
    span.start.column + op.chars().count().saturating_sub(2)
}
