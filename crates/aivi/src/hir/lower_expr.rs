fn lower_expr_inner_ctx(
    expr: Expr,
    id_gen: &mut IdGen,
    ctx: &mut LowerCtx<'_>,
    in_pipe_left: bool,
) -> HirExpr {
    if let Some(lowered) = maybe_lower_query_expr(&expr, id_gen, ctx) {
        return lowered;
    }
    match expr {
        Expr::Ident(name) => HirExpr::Var {
            id: id_gen.next(),
            name: name.name,
            location: ctx
                .source_path
                .map(|path| crate::diagnostics::SourceOrigin::new(path.to_string(), name.span)),
        },
        Expr::UnaryNeg { expr, .. } => HirExpr::Binary {
            id: id_gen.next(),
            op: "-".to_string(),
            left: Box::new(HirExpr::LitNumber {
                id: id_gen.next(),
                text: "0".to_string(),
            }),
            right: Box::new(lower_expr_ctx(*expr, id_gen, ctx, false)),
            location: None,
        },
        Expr::TextInterpolate { parts, .. } => HirExpr::TextInterpolate {
            id: id_gen.next(),
            parts: parts
                .into_iter()
                .map(|part| match part {
                    TextPart::Text { text, .. } => HirTextPart::Text { text },
                    TextPart::Expr { expr, .. } => HirTextPart::Expr {
                        expr: lower_expr_ctx(*expr, id_gen, ctx, false),
                    },
                })
                .collect(),
        },
        Expr::Flow { root, lines, span } => lower_expr_ctx(
            desugar_flow_lowering_fallback(*root, &lines, span),
            id_gen,
            ctx,
            false,
        ),
        Expr::Literal(literal) => match literal {
            crate::surface::Literal::Number { text, .. } => {
                fn split_suffixed(text: &str) -> Option<(String, String)> {
                    let mut chars = text.chars().peekable();
                    let mut number = String::new();
                    if matches!(chars.peek(), Some('-')) {
                        number.push('-');
                        chars.next();
                    }
                    let mut saw_digit = false;
                    let mut saw_dot = false;
                    while let Some(&ch) = chars.peek() {
                        if ch.is_ascii_digit() {
                            saw_digit = true;
                            number.push(ch);
                            chars.next();
                            continue;
                        }
                        if ch == '.' && !saw_dot {
                            saw_dot = true;
                            number.push(ch);
                            chars.next();
                            continue;
                        }
                        break;
                    }
                    if !saw_digit {
                        return None;
                    }
                    let suffix: String = chars.collect();
                    if suffix.is_empty() {
                        return None;
                    }
                    if !suffix
                        .chars()
                        .all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
                    {
                        return None;
                    }
                    Some((number, suffix))
                }

                if let Some((number, suffix)) = split_suffixed(&text) {
                    let template_name = format!("1{suffix}");
                    return HirExpr::App {
                        id: id_gen.next(),
                        func: Box::new(HirExpr::Var {
                            id: id_gen.next(),
                            name: template_name,
                        
                            location: None,
                        }),
                        arg: Box::new(HirExpr::LitNumber {
                            id: id_gen.next(),
                            text: number,
                        }),
                        location: None,
                    };
                }

                HirExpr::LitNumber {
                    id: id_gen.next(),
                    text,
                }
            }
            crate::surface::Literal::String { text, .. } => HirExpr::LitString {
                id: id_gen.next(),
                text,
            },
            crate::surface::Literal::Sigil {
                tag, body, flags, ..
            } => HirExpr::LitSigil {
                id: id_gen.next(),
                tag,
                body,
                flags,
            },
            crate::surface::Literal::Bool { value, .. } => HirExpr::LitBool {
                id: id_gen.next(),
                value,
            },
            crate::surface::Literal::DateTime { text, .. } => HirExpr::LitDateTime {
                id: id_gen.next(),
                text,
            },
        },
        Expr::Suffixed { base, suffix, .. } => {
            let template_name = format!("1{}", suffix.name);
            HirExpr::App {
                id: id_gen.next(),
                func: Box::new(HirExpr::Var {
                    id: id_gen.next(),
                    name: template_name,
                
                    location: None,
                }),
                arg: Box::new(lower_expr_ctx(*base, id_gen, ctx, false)),
                location: None,
            }
        }
        Expr::List { items, .. } => HirExpr::List {
            id: id_gen.next(),
            items: items
                .into_iter()
                .map(|item| {
                    // Range items like `1..3` behave like implicit list spreads in surface syntax
                    // (i.e. `[0, 1..3, 4]` becomes `[0, 1, 2, 3, 4]`).
                    let is_range = matches!(&item.expr, Expr::Binary { op, .. } if op == "..");
                    HirListItem {
                        expr: lower_expr_ctx(item.expr, id_gen, ctx, false),
                        spread: item.spread || is_range,
                    }
                })
                .collect(),
        },
        Expr::Tuple { items, .. } => HirExpr::Tuple {
            id: id_gen.next(),
            items: items
                .into_iter()
                .map(|item| lower_expr_ctx(item, id_gen, ctx, false))
                .collect(),
        },
        Expr::Record { fields, .. } => {
            let lowered_fields: Vec<HirRecordField> = fields
                .into_iter()
                .map(|field| HirRecordField {
                    spread: field.spread,
                    path: field
                        .path
                        .into_iter()
                        .map(|segment| match segment {
                            crate::surface::PathSegment::Field(name) => {
                                HirPathSegment::Field(name.name)
                            }
                            crate::surface::PathSegment::Index(expr, _) => {
                                HirPathSegment::Index(lower_expr_ctx(expr, id_gen, ctx, false))
                            }
                            crate::surface::PathSegment::All(_) => HirPathSegment::All,
                        })
                        .collect(),
                    value: lower_expr_ctx(field.value, id_gen, ctx, false),
                })
                .collect();
            HirExpr::Record {
                id: id_gen.next(),
                fields: lowered_fields,
            }
        }
        Expr::PatchLit { fields, .. } => {
            let compiled_patch = compile_static_db_patch(&fields);
            let fallback_patch = build_patch_lambda_hir(fields, id_gen, ctx);
            build_db_patch_hir(compiled_patch, fallback_patch, id_gen, ctx)
        }
        Expr::FieldAccess { base, field, span } => HirExpr::FieldAccess {
            id: id_gen.next(),
            base: Box::new(lower_expr_ctx(*base, id_gen, ctx, false)),
            field: field.name,
            location: ctx
                .source_path
                .map(|path| crate::diagnostics::SourceOrigin::new(path.to_string(), span)),
        },
        Expr::FieldSection { field, span } => {
            let param = "_arg0".to_string();
            let var = HirExpr::Var {
                id: id_gen.next(),
                name: param.clone(),
            
                location: None,
            };
            let body = HirExpr::FieldAccess {
                id: id_gen.next(),
                base: Box::new(var),
                field: field.name,
                location: ctx
                    .source_path
                    .map(|path| crate::diagnostics::SourceOrigin::new(path.to_string(), span.clone())),
            };
            HirExpr::Lambda {
                id: id_gen.next(),
                param,
                body: Box::new(body),
                location: ctx
                    .source_path
                    .map(|path| crate::diagnostics::SourceOrigin::new(path.to_string(), span)),
            }
        }
        Expr::Index { base, index, span } => HirExpr::Index {
            id: id_gen.next(),
            base: Box::new(lower_expr_ctx(*base, id_gen, ctx, false)),
            index: Box::new(lower_expr_ctx(*index, id_gen, ctx, false)),
            location: ctx
                .source_path
                .map(|path| crate::diagnostics::SourceOrigin::new(path.to_string(), span.clone())),
        },
        Expr::Call { func, args, span } => {
            if let Some(lowered) = maybe_lower_db_patch_call(&func, &args, span.clone(), id_gen, ctx) {
                lowered
            } else {
                HirExpr::Call {
                    id: id_gen.next(),
                    func: Box::new(lower_expr_ctx(*func, id_gen, ctx, false)),
                    args: args
                        .into_iter()
                        .map(|arg| lower_expr_ctx(arg, id_gen, ctx, false))
                        .collect(),
                    location: ctx
                        .source_path
                        .map(|path| crate::diagnostics::SourceOrigin::new(path.to_string(), span)),
                }
            }
        }
        Expr::Lambda { params, body, span } => {
            let body = lower_expr_ctx(*body, id_gen, ctx, false);
            let location = ctx
                .source_path
                .map(|path| crate::diagnostics::SourceOrigin::new(path.to_string(), span));
            lower_lambda_hir(params, body, location, id_gen)
        }
        Expr::Match {
            scrutinee, arms, span,
        } => {
            let location = ctx
                .source_path
                .map(|path| crate::diagnostics::SourceOrigin::new(path.to_string(), span.clone()));
            let scrutinee = if let Some(scrutinee) = scrutinee {
                lower_expr_ctx(*scrutinee, id_gen, ctx, false)
            } else {
                let param = "_arg0".to_string();
                let var = HirExpr::Var {
                    id: id_gen.next(),
                    name: param.clone(),
                    location: None,
                };
                let match_expr = HirExpr::Match {
                    id: id_gen.next(),
                    scrutinee: Box::new(var),
                    arms: arms
                        .into_iter()
                        .map(|arm| HirMatchArm {
                            pattern: lower_pattern(arm.pattern, id_gen),
                            guard: arm
                                .guard
                                .map(|guard| lower_expr_ctx(guard, id_gen, ctx, false)),
                            guard_negated: arm.guard_negated,
                            body: lower_expr_ctx(arm.body, id_gen, ctx, false),
                        })
                        .collect(),
                    location: location.clone(),
                };
                return HirExpr::Lambda {
                    id: id_gen.next(),
                    param,
                    body: Box::new(match_expr),
                    location,
                };
            };
            HirExpr::Match {
                id: id_gen.next(),
                scrutinee: Box::new(scrutinee),
                arms: arms
                    .into_iter()
                    .map(|arm| HirMatchArm {
                        pattern: lower_pattern(arm.pattern, id_gen),
                        guard: arm
                            .guard
                            .map(|guard| lower_expr_ctx(guard, id_gen, ctx, false)),
                        guard_negated: arm.guard_negated,
                        body: lower_expr_ctx(arm.body, id_gen, ctx, false),
                    })
                    .collect(),
                location,
            }
        }
        Expr::If {
            cond,
            then_branch,
            else_branch,
            span,
        } => HirExpr::If {
            id: id_gen.next(),
            cond: Box::new(lower_expr_ctx(*cond, id_gen, ctx, false)),
            then_branch: Box::new(lower_expr_ctx(*then_branch, id_gen, ctx, false)),
            else_branch: Box::new(lower_expr_ctx(*else_branch, id_gen, ctx, false)),
            location: ctx
                .source_path
                .map(|path| crate::diagnostics::SourceOrigin::new(path.to_string(), span)),
        },
        Expr::Binary {
            op, left, right, span,
        } => {
            if op == "&&" {
                let cond = lower_expr_ctx(*left, id_gen, ctx, false);
                let then_branch = lower_expr_ctx(*right, id_gen, ctx, false);
                let else_branch = HirExpr::LitBool {
                    id: id_gen.next(),
                    value: false,
                };
                return HirExpr::If {
                    id: id_gen.next(),
                    cond: Box::new(cond),
                    then_branch: Box::new(then_branch),
                    else_branch: Box::new(else_branch),
                    location: ctx.source_path.map(|path| {
                        crate::diagnostics::SourceOrigin::new(path.to_string(), span.clone())
                    }),
                };
            }
            if op == "||" {
                let cond = lower_expr_ctx(*left, id_gen, ctx, false);
                let then_branch = HirExpr::LitBool {
                    id: id_gen.next(),
                    value: true,
                };
                let else_branch = lower_expr_ctx(*right, id_gen, ctx, false);
                return HirExpr::If {
                    id: id_gen.next(),
                    cond: Box::new(cond),
                    then_branch: Box::new(then_branch),
                    else_branch: Box::new(else_branch),
                    location: ctx.source_path.map(|path| {
                        crate::diagnostics::SourceOrigin::new(path.to_string(), span.clone())
                    }),
                };
            }
            if op == "|>" {
                let debug_pipes = ctx.debug.as_ref().is_some_and(|d| d.params.pipes);
                if debug_pipes && !in_pipe_left {
                    return lower_pipe_chain(*left, *right, id_gen, ctx);
                }
                let left = lower_expr_ctx(*left, id_gen, ctx, true);
                let right = lower_expr_ctx(*right, id_gen, ctx, false);
                return HirExpr::App {
                    id: id_gen.next(),
                    func: Box::new(right),
                    arg: Box::new(left),
                    location: ctx.source_path.map(|path| {
                        crate::diagnostics::SourceOrigin::new(path.to_string(), span.clone())
                    }),
                };
            }
            if op == "<|" {
                let left_is_db_selector = looks_like_db_selector_target(left.as_ref());
                match *right {
                    Expr::Record {
                        fields,
                        span: record_span,
                    }
                    | Expr::PatchLit {
                        fields,
                        span: record_span,
                    } => {
                        if left_is_db_selector {
                            return lower_named_call_hir(
                                "aivi.database.update",
                                vec![
                                    lower_expr_ctx(*left, id_gen, ctx, false),
                                    lower_expr_ctx(
                                        Expr::PatchLit {
                                            fields,
                                            span: record_span,
                                        },
                                        id_gen,
                                        ctx,
                                        false,
                                    ),
                                ],
                                id_gen,
                                ctx,
                                span.clone(),
                            );
                        }
                        return HirExpr::Patch {
                            id: id_gen.next(),
                            target: Box::new(lower_expr_ctx(*left, id_gen, ctx, false)),
                            fields: fields
                                .into_iter()
                                .map(|field| HirRecordField {
                                    spread: field.spread,
                                    path: field
                                        .path
                                        .into_iter()
                                        .map(|segment| match segment {
                                            crate::surface::PathSegment::Field(name) => {
                                                HirPathSegment::Field(name.name)
                                            }
                                            crate::surface::PathSegment::Index(expr, _) => {
                                                HirPathSegment::Index(lower_expr_ctx(
                                                    expr, id_gen, ctx, false,
                                                ))
                                            }
                                            crate::surface::PathSegment::All(_) => {
                                                HirPathSegment::All
                                            }
                                        })
                                        .collect(),
                                    value: lower_expr_ctx(field.value, id_gen, ctx, false),
                                })
                                .collect(),
                        };
                    }
                    right => {
                            return HirExpr::Binary {
                                id: id_gen.next(),
                                op,
                                left: Box::new(lower_expr_ctx(*left, id_gen, ctx, false)),
                                right: Box::new(lower_expr_ctx(right, id_gen, ctx, false)),
                                location: ctx.source_path.map(|path| {
                                    crate::diagnostics::SourceOrigin::new(
                                        path.to_string(),
                                        span.clone(),
                                    )
                                }),
                            };
                    }
                }
            }
            let location = ctx
                .source_path
                .map(|path| crate::diagnostics::SourceOrigin::new(path.to_string(), span.clone()));
            HirExpr::Binary {
                id: id_gen.next(),
                op,
                left: Box::new(lower_expr_ctx(*left, id_gen, ctx, false)),
                right: Box::new(lower_expr_ctx(*right, id_gen, ctx, false)),
                location,
            }
        }
        Expr::Block { kind, items, .. } => {
            let block_kind = lower_block_kind(&kind);
            // Event blocks desugar to `reactive.eventFrom(...)` around an effect-style block.
            if let BlockKind::Do { ref monad } = kind {
                if monad.name == "Event" {
                    let effect_monad = SpannedName {
                        name: "Effect".to_string(),
                        span: monad.span.clone(),
                    };
                    let effect_kind = BlockKind::Do {
                        monad: effect_monad,
                    };
                    let effect_block_kind = lower_block_kind(&effect_kind);
                    let effect_block = HirExpr::Block {
                        id: id_gen.next(),
                        block_kind: effect_block_kind.clone(),
                        items: items
                            .into_iter()
                            .map(|item| {
                                lower_block_item_ctx(
                                    item,
                                    &effect_kind,
                                    &effect_block_kind,
                                    id_gen,
                                    ctx,
                                )
                            })
                            .collect(),
                    };
                    return HirExpr::Call {
                        id: id_gen.next(),
                        func: Box::new(HirExpr::FieldAccess {
                            id: id_gen.next(),
                            base: Box::new(HirExpr::Var {
                                id: id_gen.next(),
                                name: "reactive".to_string(),
                            
                                location: None,
                            }),
                            field: "eventFrom".to_string(),
                            location: None,
                        }),
                        args: vec![effect_block],
                        location: None,
                    };
                }
            }
            // Generic `do M { ... }` blocks (where M ≠ Effect) desugar into
            // nested `chain` / lambda calls so that the runtime resolves
            // `chain`/`of` through the normal Monad instance dispatch.
            if let BlockKind::Do { ref monad } = kind {
                if monad.name == "Applicative" {
                    return desugar_applicative_do_block(items, &kind, id_gen, ctx);
                }
                if monad.name != "Effect" {
                    return desugar_generic_do_block(items, &kind, &block_kind, id_gen, ctx);
                }
            }
            HirExpr::Block {
                id: id_gen.next(),
                block_kind: block_kind.clone(),
                items: items
                    .into_iter()
                    .map(|item| lower_block_item_ctx(item, &kind, &block_kind, id_gen, ctx))
                .collect(),
            }
        }
        Expr::Raw { text, .. } => HirExpr::Raw {
            id: id_gen.next(),
            text,
        },
        Expr::Mock {
            substitutions,
            body,
            ..
        } => HirExpr::Mock {
            id: id_gen.next(),
            substitutions: substitutions
                .into_iter()
                .map(|sub| HirMockSubstitution {
                    path: sub.path.iter().map(|s| s.name.as_str()).collect::<Vec<_>>().join("."),
                    snapshot: sub.snapshot,
                    value: sub.value.map(|v| lower_expr_inner_ctx(v, id_gen, ctx, false)),
                })
                .collect(),
            body: Box::new(lower_expr_inner_ctx(*body, id_gen, ctx, false)),
        },
    }
}

fn desugar_flow_lowering_fallback(
    root: Expr,
    lines: &[FlowLine],
    _span: crate::diagnostics::Span,
) -> Expr {
    desugar_flow_lowering_tail(root.clone(), lines).unwrap_or(root)
}

fn desugar_flow_lowering_tail(root: Expr, lines: &[FlowLine]) -> Option<Expr> {
    if lines.is_empty() {
        return Some(root);
    }
    match &lines[0] {
        FlowLine::Anchor(_) => desugar_flow_lowering_tail(root, &lines[1..]),
        FlowLine::Step(step) if step.kind == FlowStepKind::Flow => {
            let applied = Expr::Binary {
                op: "|>".to_string(),
                left: Box::new(root),
                right: Box::new(step.expr.clone()),
                span: step.span.clone(),
            };
            if let Some(binding) = &step.binding {
                let rest = desugar_flow_lowering_tail(Expr::Ident(binding.name.clone()), &lines[1..])?;
                Some(Expr::Block {
                    kind: BlockKind::Plain,
                    items: vec![
                        BlockItem::Let {
                            pattern: Pattern::Ident(binding.name.clone()),
                            expr: applied,
                            span: binding.span.clone(),
                        },
                        BlockItem::Expr {
                            expr: rest,
                            span: step.span.clone(),
                        },
                    ],
                    span: step.span.clone(),
                })
            } else {
                desugar_flow_lowering_tail(applied, &lines[1..])
            }
        }
        FlowLine::Step(step) if step.kind == FlowStepKind::Tap => {
            let observed = Expr::Binary {
                op: "|>".to_string(),
                left: Box::new(root.clone()),
                right: Box::new(step.expr.clone()),
                span: step.span.clone(),
            };
            let rest = desugar_flow_lowering_tail(root, &lines[1..])?;
            Some(Expr::Block {
                kind: BlockKind::Plain,
                items: vec![
                    BlockItem::Let {
                        pattern: step
                            .binding
                            .as_ref()
                            .map(|binding| Pattern::Ident(binding.name.clone()))
                            .unwrap_or_else(|| Pattern::Wildcard(step.span.clone())),
                        expr: observed,
                        span: step.span.clone(),
                    },
                    BlockItem::Expr {
                        expr: rest,
                        span: step.span.clone(),
                    },
                ],
                span: step.span.clone(),
            })
        }
        FlowLine::Guard(guard) if guard.fail_expr.is_some() => {
            let rest = desugar_flow_lowering_tail(root.clone(), &lines[1..])?;
            Some(Expr::Block {
                kind: BlockKind::Do {
                    monad: SpannedName {
                        name: "Effect".to_string(),
                        span: guard.span.clone(),
                    },
                },
                items: vec![
                    BlockItem::Given {
                        cond: Expr::Binary {
                            op: "|>".to_string(),
                            left: Box::new(root),
                            right: Box::new(guard.predicate.clone()),
                            span: guard.span.clone(),
                        },
                        fail_expr: Expr::Call {
                            func: Box::new(Expr::Ident(SpannedName {
                                name: "fail".to_string(),
                                span: guard.span.clone(),
                            })),
                            args: vec![guard.fail_expr.clone().expect("guard fail expr")],
                            span: guard.span.clone(),
                        },
                        span: guard.span.clone(),
                    },
                    BlockItem::Expr {
                        expr: rest,
                        span: guard.span.clone(),
                    },
                ],
                span: guard.span.clone(),
            })
        }
        FlowLine::Branch(_) => {
            let mut consumed = 0;
            let mut arms = Vec::new();
            while let Some(FlowLine::Branch(arm)) = lines.get(consumed) {
                arms.push(MatchArm {
                    pattern: arm.pattern.clone(),
                    guard: arm.guard.clone(),
                    guard_negated: arm.guard_negated,
                    body: arm.body.clone(),
                    span: arm.span.clone(),
                });
                consumed += 1;
            }
            let matched = Expr::Match {
                scrutinee: Some(Box::new(root)),
                arms,
                span: flow_line_surface_span(&lines[0]),
            };
            desugar_flow_lowering_tail(matched, &lines[consumed..])
        }
        FlowLine::Step(step)
            if matches!(
                step.kind,
                FlowStepKind::Attempt
                    | FlowStepKind::Applicative
                    | FlowStepKind::FanOut
            ) =>
        {
            Some(Expr::Binary {
                op: "|>".to_string(),
                left: Box::new(root),
                right: Box::new(step.expr.clone()),
                span: step.span.clone(),
            })
        }
        FlowLine::Guard(_) | FlowLine::Recover(_) => Some(root),
        FlowLine::Step(_) => Some(root),
    }
}

fn flow_line_surface_span(line: &FlowLine) -> crate::diagnostics::Span {
    match line {
        FlowLine::Step(step) => step.span.clone(),
        FlowLine::Guard(guard) => guard.span.clone(),
        FlowLine::Branch(arm) | FlowLine::Recover(arm) => arm.span.clone(),
        FlowLine::Anchor(anchor) => anchor.span.clone(),
    }
}

fn surface_expr_span(expr: &Expr) -> crate::diagnostics::Span {
    match expr {
        Expr::Ident(name) => name.span.clone(),
        Expr::Literal(literal) => match literal {
            crate::surface::Literal::Number { span, .. }
            | crate::surface::Literal::String { span, .. }
            | crate::surface::Literal::Sigil { span, .. }
            | crate::surface::Literal::Bool { span, .. }
            | crate::surface::Literal::DateTime { span, .. } => span.clone(),
        },
        Expr::UnaryNeg { span, .. }
        | Expr::TextInterpolate { span, .. }
        | Expr::List { span, .. }
        | Expr::Tuple { span, .. }
        | Expr::Record { span, .. }
        | Expr::PatchLit { span, .. }
        | Expr::Suffixed { span, .. }
        | Expr::FieldAccess { span, .. }
        | Expr::FieldSection { span, .. }
        | Expr::Index { span, .. }
        | Expr::Call { span, .. }
        | Expr::Lambda { span, .. }
        | Expr::Match { span, .. }
        | Expr::If { span, .. }
        | Expr::Binary { span, .. }
        | Expr::Flow { span, .. }
        | Expr::Block { span, .. }
        | Expr::Mock { span, .. }
        | Expr::Raw { span, .. } => span.clone(),
    }
}

fn slice_source_by_span(source: &str, span: &crate::diagnostics::Span) -> Option<String> {
    let lines: Vec<&str> = source.split('\n').collect();
    let start_line = span.start.line.checked_sub(1)?;
    let end_line = span.end.line.checked_sub(1)?;
    if start_line >= lines.len() || end_line >= lines.len() {
        return None;
    }

    fn slice_line(line: &str, start_col: usize, end_col: usize) -> String {
        let chars: Vec<char> = line.chars().collect();
        let start = start_col.saturating_sub(1).min(chars.len());
        let end = end_col.min(chars.len());
        chars[start..end].iter().collect()
    }

    if start_line == end_line {
        return Some(slice_line(
            lines[start_line],
            span.start.column,
            span.end.column,
        ));
    }

    let mut out = String::new();
    out.push_str(&slice_line(
        lines[start_line],
        span.start.column,
        lines[start_line].chars().count(),
    ));
    out.push('\n');
    for line in lines.iter().take(end_line).skip(start_line + 1) {
        out.push_str(line);
        out.push('\n');
    }
    out.push_str(&slice_line(lines[end_line], 1, span.end.column));
    Some(out)
}

fn normalize_debug_label(label: &str) -> String {
    let mut out = String::new();
    let mut prev_ws = false;
    for ch in label.chars() {
        if ch.is_whitespace() {
            if !prev_ws {
                out.push(' ');
                prev_ws = true;
            }
        } else {
            out.push(ch);
            prev_ws = false;
        }
    }
    out.trim().to_string()
}

fn lower_pipe_chain(
    left: Expr,
    right: Expr,
    id_gen: &mut IdGen,
    ctx: &mut LowerCtx<'_>,
) -> HirExpr {
    let right_span = surface_expr_span(&right);
    let Some(_) = ctx.debug.as_ref() else {
        let left = lower_expr_ctx(left, id_gen, ctx, true);
        let right = lower_expr_ctx(right, id_gen, ctx, false);
        return HirExpr::App {
            id: id_gen.next(),
            func: Box::new(right),
            arg: Box::new(left),
            location: ctx
                .source_path
                .map(|path| crate::diagnostics::SourceOrigin::new(path.to_string(), right_span)),
        };
    };

    let mut steps: Vec<(Expr, crate::diagnostics::Span)> = vec![(right, right_span)];
    let mut base = left;
    while let Expr::Binary {
        op,
        left,
        right,
        span,
    } = base
    {
        if op != "|>" {
            base = Expr::Binary {
                op,
                left,
                right,
                span,
            };
            break;
        }
        let step_span = surface_expr_span(&right);
        steps.push((*right, step_span));
        base = *left;
    }
    steps.reverse();

    let Some(debug) = ctx.debug.as_mut() else {
        let mut acc = lower_expr_ctx(base, id_gen, ctx, false);
        for (step_expr, step_span) in steps {
            let func = lower_expr_ctx(step_expr, id_gen, ctx, false);
            acc = HirExpr::App {
                id: id_gen.next(),
                func: Box::new(func),
                arg: Box::new(acc),
                location: ctx.source_path.map(|path| {
                    crate::diagnostics::SourceOrigin::new(path.to_string(), step_span)
                }),
            };
        }
        return acc;
    };
    let (pipe_id, source, log_time) = (debug.alloc_pipe_id(), debug.source, debug.params.time);
    let mut acc = lower_expr_ctx(base, id_gen, ctx, false);
    for (idx, (step_expr, step_span)) in steps.into_iter().enumerate() {
        let func = lower_expr_ctx(step_expr, id_gen, ctx, false);
        let label = source
            .and_then(|src| slice_source_by_span(src, &step_span))
            .map(|s| normalize_debug_label(&s))
            .unwrap_or_else(|| "<unknown>".to_string());
        acc = HirExpr::Pipe {
            id: id_gen.next(),
            pipe_id,
            step: (idx as u32) + 1,
            label,
            log_time,
            func: Box::new(func),
            arg: Box::new(acc),
            location: ctx.source_path.map(|path| {
                crate::diagnostics::SourceOrigin::new(path.to_string(), step_span)
            }),
        };
    }
    acc
}

fn lower_lambda_hir(
    params: Vec<Pattern>,
    body: HirExpr,
    location: Option<crate::diagnostics::SourceOrigin>,
    id_gen: &mut IdGen,
) -> HirExpr {
    let mut acc = body;
    for (index, param) in params.into_iter().rev().enumerate() {
        match param {
            Pattern::Ident(name) => {
                acc = HirExpr::Lambda {
                    id: id_gen.next(),
                    param: name.name,
                    body: Box::new(acc),
                    location: location.clone(),
                };
            }
            Pattern::SubjectIdent(name) => {
                acc = HirExpr::Lambda {
                    id: id_gen.next(),
                    param: name.name,
                    body: Box::new(acc),
                    location: location.clone(),
                };
            }
            Pattern::Wildcard(_) => {
                acc = HirExpr::Lambda {
                    id: id_gen.next(),
                    param: format!("_arg{}", index),
                    body: Box::new(acc),
                    location: location.clone(),
                };
            }
            other => {
                let param_name = format!("_arg{}", index);
                let match_expr = HirExpr::Match {
                    id: id_gen.next(),
                    scrutinee: Box::new(HirExpr::Var {
                        id: id_gen.next(),
                        name: param_name.clone(),
                    
                        location: None,
                    }),
                    arms: vec![HirMatchArm {
                        pattern: lower_pattern(other, id_gen),
                        guard: None,
                        guard_negated: false,
                        body: acc,
                    }],
                    location: None,
                };
                acc = HirExpr::Lambda {
                    id: id_gen.next(),
                    param: param_name,
                    body: Box::new(match_expr),
                    location: location.clone(),
                };
            }
        }
    }
    acc
}

fn desugar_applicative_do_block(
    items: Vec<BlockItem>,
    _surface_kind: &BlockKind,
    id_gen: &mut IdGen,
    ctx: &mut LowerCtx<'_>,
) -> HirExpr {
    if items.is_empty() {
        return HirExpr::Call {
            id: id_gen.next(),
            func: Box::new(HirExpr::Var {
                id: id_gen.next(),
                name: "of".to_string(),
            
                location: None,
            }),
            args: vec![HirExpr::Var {
                id: id_gen.next(),
                name: "Unit".to_string(),
            
                location: None,
            }],
            location: None,
        };
    }

    let Some(BlockItem::Expr {
        expr: final_expr,
        ..
    }) = items.last()
    else {
        return HirExpr::Var {
            id: id_gen.next(),
            name: "Unit".to_string(),
        
            location: None,
        };
    };

    let mut body = lower_expr_ctx(final_expr.clone(), id_gen, ctx, false);
    let mut applicative_inputs_rev: Vec<HirExpr> = Vec::new();

    for item in items[..items.len() - 1].iter().rev() {
        match item {
            BlockItem::Bind {
                pattern,
                expr,
                ..
            } => {
                let param = format!("__do_applicative{}", id_gen.next());
                body = make_pattern_lambda(pattern.clone(), body, &param, id_gen);
                applicative_inputs_rev.push(lower_expr_ctx(expr.clone(), id_gen, ctx, false));
            }
            BlockItem::Let {
                pattern,
                expr,
                ..
            } => {
                let rhs = lower_expr_ctx(expr.clone(), id_gen, ctx, false);
                let param = format!("__do_applicative_let{}", id_gen.next());
                body = HirExpr::App {
                    id: id_gen.next(),
                    func: Box::new(make_pattern_lambda(pattern.clone(), body, &param, id_gen)),
                    arg: Box::new(rhs),
                    location: None,
                };
            }
            _ => {
                return HirExpr::Var {
                    id: id_gen.next(),
                    name: "Unit".to_string(),
                
                    location: None,
                };
            }
        }
    }

    if applicative_inputs_rev.is_empty() {
        return HirExpr::Call {
            id: id_gen.next(),
            func: Box::new(HirExpr::Var {
                id: id_gen.next(),
                name: "of".to_string(),
            
                location: None,
            }),
            args: vec![body],
            location: None,
        };
    }

    applicative_inputs_rev.reverse();
    let mut acc = HirExpr::Call {
        id: id_gen.next(),
        func: Box::new(HirExpr::Var {
            id: id_gen.next(),
            name: "map".to_string(),
        
            location: None,
        }),
        args: vec![body, applicative_inputs_rev[0].clone()],
        location: None,
    };
    for expr in applicative_inputs_rev.into_iter().skip(1) {
        acc = HirExpr::Call {
            id: id_gen.next(),
            func: Box::new(HirExpr::Var {
                id: id_gen.next(),
                name: "ap".to_string(),
            
                location: None,
            }),
            args: vec![acc, expr],
            location: None,
        };
    }
    acc
}

/// Desugar `do M { ... }` (where M ≠ Effect) into nested `chain`/lambda calls.
///
/// - `x <- e; rest`  →  `chain (λx. <rest>) e`
/// - `x = e; rest`   →  `(λx. <rest>) e`       (plain let)
/// - `e; rest`        →  `chain (λ_. <rest>) e`
/// - `e`  (final)     →  `e`
/// - `{}`  (empty)    →  `of Unit`
///
fn desugar_generic_do_block(
    items: Vec<BlockItem>,
    surface_kind: &BlockKind,
    hir_kind: &HirBlockKind,
    id_gen: &mut IdGen,
    ctx: &mut LowerCtx<'_>,
) -> HirExpr {
    let ops = do_block_ops(surface_kind);

    if items.is_empty() {
        // `do M {}` → `of Unit`
        return HirExpr::Call {
            id: id_gen.next(),
            func: Box::new(HirExpr::Var {
                id: id_gen.next(),
                name: ops.of.to_string(),
            
                location: None,
            }),
            args: vec![HirExpr::Var {
                id: id_gen.next(),
                name: "Unit".to_string(),
            
                location: None,
            }],
            location: None,
        };
    }

    desugar_do_items(&items, 0, surface_kind, hir_kind, &ops, id_gen, ctx)
}

/// Returns the `chain`/`of` function names for a generic do-block.
fn do_block_ops(surface_kind: &BlockKind) -> DoOps {
    let _ = surface_kind;
    DoOps {
        chain: "chain",
        of: "of",
    }
}

/// Binds the `chain` and `of` function names used when desugaring a `do M` block.
struct DoOps {
    chain: &'static str,
    of: &'static str,
}

/// Recursively desugar do-block items starting at `index`.
fn desugar_do_items(
    items: &[BlockItem],
    index: usize,
    surface_kind: &BlockKind,
    hir_kind: &HirBlockKind,
    ops: &DoOps,
    id_gen: &mut IdGen,
    ctx: &mut LowerCtx<'_>,
) -> HirExpr {
    let item = &items[index];
    let is_last = index + 1 == items.len();

    match item {
        // Bind: `x <- e`
        BlockItem::Bind { pattern, expr, .. } => {
            let rhs = lower_expr_ctx(expr.clone(), id_gen, ctx, false);
            if is_last {
                // Final bind is unusual but we still desugar it as `chain (λx. x) e`
                // which is equivalent to the expression being the block result.
                // Actually, a final bind makes no sense semantically; just return the
                // monadic value as-is (the user is expected to use `<-` in non-tail position).
                // For pragmatic compat, treat it as `chain (λpat. of pat) e`.
                let param = format!("__do_bind{}", id_gen.next());
                let body = HirExpr::Var {
                    id: id_gen.next(),
                    name: param.clone(),
                
                    location: None,
                };
                // Wrap in `of` for final bind: `chain (λx. of x) e`
                let wrapped = HirExpr::Call {
                    id: id_gen.next(),
                    func: Box::new(HirExpr::Var {
                        id: id_gen.next(),
                        name: ops.of.to_string(),
                    
                        location: None,
                    }),
                    args: vec![body],
                    location: None,
                };
                let continuation = make_pattern_lambda(pattern.clone(), wrapped, &param, id_gen);
                // chain continuation rhs
                HirExpr::Call {
                    id: id_gen.next(),
                    func: Box::new(HirExpr::Var {
                        id: id_gen.next(),
                        name: ops.chain.to_string(),
                    
                        location: None,
                    }),
                    args: vec![continuation, rhs],
                    location: None,
                }
            } else {
                let rest = desugar_do_items(items, index + 1, surface_kind, hir_kind, ops, id_gen, ctx);
                let param = format!("__do_bind{}", id_gen.next());
                let continuation = make_pattern_lambda(pattern.clone(), rest, &param, id_gen);
                // chain continuation rhs
                HirExpr::Call {
                    id: id_gen.next(),
                    func: Box::new(HirExpr::Var {
                        id: id_gen.next(),
                        name: ops.chain.to_string(),
                    
                        location: None,
                    }),
                    args: vec![continuation, rhs],
                    location: None,
                }
            }
        }
        // Pure let-binding: `x = e`
        BlockItem::Let { pattern, expr, .. } => {
            let rhs = lower_expr_ctx(expr.clone(), id_gen, ctx, false);
            if is_last {
                // Final let — the value itself becomes the result.
                // Wrap in `of` since the block must produce `M A`.
                HirExpr::Call {
                    id: id_gen.next(),
                    func: Box::new(HirExpr::Var {
                        id: id_gen.next(),
                        name: ops.of.to_string(),
                    
                        location: None,
                    }),
                    args: vec![rhs],
                    location: None,
                }
            } else {
                let rest = desugar_do_items(items, index + 1, surface_kind, hir_kind, ops, id_gen, ctx);
                let param = format!("__do_let{}", id_gen.next());
                let body = make_pattern_lambda(pattern.clone(), rest, &param, id_gen);
                // (λpat. rest) rhs
                HirExpr::App {
                    id: id_gen.next(),
                    func: Box::new(body),
                    arg: Box::new(rhs),
                    location: None,
                }
            }
        }
        // Expression statement: `e`
        BlockItem::Expr { expr, span } => {
            let rhs = lower_expr_ctx(expr.clone(), id_gen, ctx, false);
            if is_last {
                // Final expression: must have type `M A`, returned directly.
                rhs
            } else {
                let rest = desugar_do_items(items, index + 1, surface_kind, hir_kind, ops, id_gen, ctx);
                let param = format!("__do_seq{}", id_gen.next());
                let continuation_location = ctx.source_path.map(|path| {
                    crate::diagnostics::SourceOrigin::new(path.to_string(), span.clone())
                });
                let continuation = HirExpr::Lambda {
                    id: id_gen.next(),
                    param,
                    body: Box::new(rest),
                    location: continuation_location,
                };
                // chain (λ_. rest) rhs
                HirExpr::Call {
                    id: id_gen.next(),
                    func: Box::new(HirExpr::Var {
                        id: id_gen.next(),
                        name: ops.chain.to_string(),
                    
                        location: None,
                    }),
                    args: vec![continuation, rhs],
                    location: None,
                }
            }
        }
        // Filter, Yield, Recurse, When, Unless, Given, On — not allowed in generic do blocks
        _ => {
            // These should have been rejected by the parser. Lower them as-is
            // (they'll hit a runtime error via the normal block path).
            let lowered = lower_block_item_ctx(item.clone(), surface_kind, hir_kind, id_gen, ctx);
            match lowered {
                HirBlockItem::Expr { expr } => expr,
                _ => HirExpr::Var {
                    id: id_gen.next(),
                    name: "Unit".to_string(),
                
                    location: None,
                },
            }
        }
    }
}

/// Build a lambda that binds a pattern: If the pattern is a simple variable,
/// emit `λname. body`. Otherwise emit `λparam. match param { pat => body }`.
fn make_pattern_lambda(
    pattern: Pattern,
    body: HirExpr,
    fallback_param: &str,
    id_gen: &mut IdGen,
) -> HirExpr {
    match &pattern {
        Pattern::Ident(name) => HirExpr::Lambda {
            id: id_gen.next(),
            param: name.name.clone(),
            body: Box::new(body),
            location: None,
        },
        Pattern::SubjectIdent(name) => HirExpr::Lambda {
            id: id_gen.next(),
            param: name.name.clone(),
            body: Box::new(body),
            location: None,
        },
        Pattern::Wildcard(_) => HirExpr::Lambda {
            id: id_gen.next(),
            param: fallback_param.to_string(),
            body: Box::new(body),
            location: None,
        },
        _ => {
            let match_expr = HirExpr::Match {
                id: id_gen.next(),
                scrutinee: Box::new(HirExpr::Var {
                    id: id_gen.next(),
                    name: fallback_param.to_string(),
                
                    location: None,
                }),
                arms: vec![HirMatchArm {
                    pattern: lower_pattern(pattern, id_gen),
                    guard: None,
                    guard_negated: false,
                    body,
                }],
                location: None,
            };
            HirExpr::Lambda {
                id: id_gen.next(),
                param: fallback_param.to_string(),
                body: Box::new(match_expr),
                location: None,
            }
        }
    }
}

fn looks_like_db_selector_target(expr: &Expr) -> bool {
    match expr {
        Expr::Index { index, .. } => looks_like_db_selector_predicate(index),
        Expr::Record { fields, .. } => {
            let mut has_table = false;
            let mut has_pred = false;
            for field in fields {
                if field.spread {
                    return false;
                }
                match field.path.as_slice() {
                    [crate::surface::PathSegment::Field(name)] if name.name == "table" => {
                        has_table = true
                    }
                    [crate::surface::PathSegment::Field(name)] if name.name == "pred" => {
                        has_pred = true
                    }
                    _ => {}
                }
            }
            has_table && has_pred
        }
        _ => false,
    }
}

fn looks_like_db_selector_predicate(expr: &Expr) -> bool {
    match expr {
        Expr::Lambda { .. }
        | Expr::Match { .. }
        | Expr::If { .. }
        | Expr::Call { .. }
        | Expr::FieldAccess { .. }
        | Expr::FieldSection { .. }
        | Expr::Ident(_) => true,
        Expr::Binary { op, .. } => matches!(
            op.as_str(),
            "==" | "!=" | "<" | "<=" | ">" | ">=" | "&&" | "||"
        ),
        Expr::Literal(crate::surface::Literal::Bool { .. }) => true,
        Expr::UnaryNeg { .. }
        | Expr::Literal(_)
        | Expr::TextInterpolate { .. }
        | Expr::List { .. }
        | Expr::Tuple { .. }
        | Expr::Record { .. }
        | Expr::PatchLit { .. }
        | Expr::Index { .. }
        | Expr::Raw { .. }
        | Expr::Suffixed { .. }
        | Expr::Flow { .. }
        | Expr::Block { .. }
        | Expr::Mock { .. } => false,
    }
}

fn lower_named_call_hir(
    name: &str,
    args: Vec<HirExpr>,
    id_gen: &mut IdGen,
    ctx: &mut LowerCtx<'_>,
    span: crate::diagnostics::Span,
) -> HirExpr {
    HirExpr::Call {
        id: id_gen.next(),
        func: Box::new(HirExpr::Var {
            id: id_gen.next(),
            name: name.to_string(),
            location: None,
        }),
        args,
        location: ctx
            .source_path
            .map(|path| crate::diagnostics::SourceOrigin::new(path.to_string(), span)),
    }
}

fn maybe_lower_db_patch_call(
    func: &Expr,
    args: &[Expr],
    span: crate::diagnostics::Span,
    id_gen: &mut IdGen,
    ctx: &mut LowerCtx<'_>,
) -> Option<HirExpr> {
    let helper = call_leaf_name(func)?;
    let patch_arg_index = match (helper, args.len()) {
        ("update", 2) => Some(1),
        ("updateOn", 3) => Some(2),
        ("upsert", 3) => Some(2),
        ("upsertOn", 4) => Some(3),
        _ => None,
    }?;

    let mut args = args.to_vec();
    let patch_arg = args.remove(patch_arg_index);
    let lowered_patch = lower_db_patch_arg(patch_arg, id_gen, ctx)?;
    let mut lowered_args = Vec::with_capacity(args.len() + 1);
    for (index, arg) in args.into_iter().enumerate() {
        if index == patch_arg_index {
            lowered_args.push(lowered_patch.clone());
        }
        lowered_args.push(lower_expr_ctx(arg, id_gen, ctx, false));
    }
    if patch_arg_index == lowered_args.len() {
        lowered_args.push(lowered_patch);
    }

    Some(HirExpr::Call {
        id: id_gen.next(),
        func: Box::new(lower_expr_ctx(func.clone(), id_gen, ctx, false)),
        args: lowered_args,
        location: ctx
            .source_path
            .map(|path| crate::diagnostics::SourceOrigin::new(path.to_string(), span)),
    })
}

fn lower_db_patch_arg(
    expr: Expr,
    id_gen: &mut IdGen,
    ctx: &mut LowerCtx<'_>,
) -> Option<HirExpr> {
    match expr {
        Expr::Record { fields, .. } | Expr::PatchLit { fields, .. } => {
            let compiled_patch = compile_static_db_patch(&fields);
            let fallback_patch = build_patch_lambda_hir(fields, id_gen, ctx);
            Some(build_db_patch_hir(compiled_patch, fallback_patch, id_gen, ctx))
        }
        _ => None,
    }
}

fn call_leaf_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Ident(name) => Some(name.name.rsplit('.').next().unwrap_or(&name.name)),
        Expr::FieldAccess { base, field, .. } => {
            call_leaf_name(base)?;
            Some(field.name.as_str())
        }
        _ => None,
    }
}

fn build_patch_lambda_hir(
    fields: Vec<crate::surface::RecordField>,
    id_gen: &mut IdGen,
    ctx: &mut LowerCtx<'_>,
) -> HirExpr {
    let param = format!("__patch_target{}", id_gen.next());
    let target = HirExpr::Var {
        id: id_gen.next(),
        name: param.clone(),
        location: None,
    };
    let patch = HirExpr::Patch {
        id: id_gen.next(),
        target: Box::new(target),
        fields: fields
            .into_iter()
            .map(|field| HirRecordField {
                spread: field.spread,
                path: field
                    .path
                    .into_iter()
                    .map(|segment| match segment {
                        crate::surface::PathSegment::Field(name) => {
                            HirPathSegment::Field(name.name)
                        }
                        crate::surface::PathSegment::Index(expr, _) => {
                            HirPathSegment::Index(lower_expr_ctx(expr, id_gen, ctx, false))
                        }
                        crate::surface::PathSegment::All(_) => HirPathSegment::All,
                    })
                    .collect(),
                value: lower_expr_ctx(field.value, id_gen, ctx, false),
            })
            .collect(),
    };
    HirExpr::Lambda {
        id: id_gen.next(),
        param,
        body: Box::new(patch),
        location: None,
    }
}

fn build_db_patch_hir(
    meta: Result<StaticCompiledDbPatch, String>,
    fallback_patch: HirExpr,
    id_gen: &mut IdGen,
    ctx: &mut LowerCtx<'_>,
) -> HirExpr {
    match meta {
        Ok(compiled) => {
            let plan_json = match serde_json::to_string(&compiled.plan) {
                Ok(json) => json,
                Err(err) => {
                    return HirExpr::Call {
                        id: id_gen.next(),
                        func: Box::new(HirExpr::Var {
                            id: id_gen.next(),
                            name: DB_PATCH_ERROR_BUILTIN.to_string(),
                            location: None,
                        }),
                        args: vec![
                            HirExpr::LitString {
                                id: id_gen.next(),
                                text: format!("failed to serialize lowered selector patch: {err}"),
                            },
                            fallback_patch,
                        ],
                        location: None,
                    };
                }
            };
            let captures = HirExpr::List {
                id: id_gen.next(),
                items: compiled
                    .capture_exprs
                    .into_iter()
                    .map(|expr| HirListItem {
                        expr: lower_expr_ctx(expr, id_gen, ctx, false),
                        spread: false,
                    })
                    .collect(),
            };
            HirExpr::Call {
                id: id_gen.next(),
                func: Box::new(HirExpr::Var {
                    id: id_gen.next(),
                    name: DB_PATCH_COMPILED_BUILTIN.to_string(),
                    location: None,
                }),
                args: vec![
                    HirExpr::LitString {
                        id: id_gen.next(),
                        text: plan_json,
                    },
                    captures,
                    fallback_patch,
                ],
                location: None,
            }
        }
        Err(message) => HirExpr::Call {
            id: id_gen.next(),
            func: Box::new(HirExpr::Var {
                id: id_gen.next(),
                name: DB_PATCH_ERROR_BUILTIN.to_string(),
                location: None,
            }),
            args: vec![
                HirExpr::LitString {
                    id: id_gen.next(),
                    text: message,
                },
                fallback_patch,
            ],
            location: None,
        },
    }
}

fn maybe_lower_query_expr(
    expr: &Expr,
    id_gen: &mut IdGen,
    ctx: &mut LowerCtx<'_>,
) -> Option<HirExpr> {
    if !expr_requires_relation_query_lowering(expr) {
        return None;
    }

    Some(match compile_static_query(expr, ctx.surface_db_index, ctx.current_module) {
        Ok(compiled) => build_static_query_hir(compiled, id_gen, ctx),
        Err(message) => build_query_error_hir(message, id_gen),
    })
}

fn build_static_query_hir(
    compiled: StaticCompiledQuery,
    id_gen: &mut IdGen,
    ctx: &mut LowerCtx<'_>,
) -> HirExpr {
    let plan_json = match serde_json::to_string(&compiled.plan) {
        Ok(json) => json,
        Err(err) => {
            return build_query_error_hir(
                format!("failed to serialize lowered query plan: {err}"),
                id_gen,
            )
        }
    };

    let sources = HirExpr::List {
        id: id_gen.next(),
        items: compiled
            .source_exprs
            .into_iter()
            .map(|expr| HirListItem {
                expr: lower_expr_ctx(expr, id_gen, ctx, false),
                spread: false,
            })
            .collect(),
    };
    let captures = HirExpr::List {
        id: id_gen.next(),
        items: compiled
            .capture_exprs
            .into_iter()
            .map(|expr| HirListItem {
                expr: lower_expr_ctx(expr, id_gen, ctx, false),
                spread: false,
            })
            .collect(),
    };

    HirExpr::Call {
        id: id_gen.next(),
        func: Box::new(HirExpr::Var {
            id: id_gen.next(),
            name: DB_QUERY_COMPILED_BUILTIN.to_string(),
        
            location: None,
        }),
        args: vec![
            HirExpr::LitString {
                id: id_gen.next(),
                text: plan_json,
            },
            sources,
            captures,
        ],
        location: None,
    }
}

fn build_query_error_hir(message: String, id_gen: &mut IdGen) -> HirExpr {
    HirExpr::Call {
        id: id_gen.next(),
        func: Box::new(HirExpr::Var {
            id: id_gen.next(),
            name: DB_QUERY_ERROR_BUILTIN.to_string(),
        
            location: None,
        }),
        args: vec![HirExpr::LitString {
            id: id_gen.next(),
            text: message,
        }],
        location: None,
    }
}

fn lower_block_kind(kind: &BlockKind) -> HirBlockKind {
    match kind {
        BlockKind::Plain => HirBlockKind::Plain,
        BlockKind::Do { monad } => HirBlockKind::Do {
            monad: monad.name.clone(),
        },
        BlockKind::Managed => HirBlockKind::Managed,
    }
}
