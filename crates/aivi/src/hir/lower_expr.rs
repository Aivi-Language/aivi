
fn lower_expr_inner_ctx(expr: Expr, id_gen: &mut IdGen, ctx: &mut LowerCtx<'_>, in_pipe_left: bool) -> HirExpr {
    match expr {
        Expr::Ident(name) => HirExpr::Var {
            id: id_gen.next(),
            name: name.name,
        },
        Expr::UnaryNeg { expr, .. } => HirExpr::Binary {
            id: id_gen.next(),
            op: "-".to_string(),
            left: Box::new(HirExpr::LitNumber {
                id: id_gen.next(),
                text: "0".to_string(),
            }),
            right: Box::new(lower_expr_ctx(*expr, id_gen, ctx, false)),
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
                        }),
                        arg: Box::new(HirExpr::LitNumber {
                            id: id_gen.next(),
                            text: number,
                        }),
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
                }),
                arg: Box::new(lower_expr_ctx(*base, id_gen, ctx, false)),
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
        Expr::Record { fields, .. } => HirExpr::Record {
            id: id_gen.next(),
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
        },
        Expr::PatchLit { fields, .. } => {
            let param = format!("__patch_target{}", id_gen.next());
            let target = HirExpr::Var {
                id: id_gen.next(),
                name: param.clone(),
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
            }
        }
        Expr::FieldAccess { base, field, .. } => HirExpr::FieldAccess {
            id: id_gen.next(),
            base: Box::new(lower_expr_ctx(*base, id_gen, ctx, false)),
            field: field.name,
        },
        Expr::FieldSection { field, .. } => {
            let param = "_arg0".to_string();
            let var = HirExpr::Var {
                id: id_gen.next(),
                name: param.clone(),
            };
            let body = HirExpr::FieldAccess {
                id: id_gen.next(),
                base: Box::new(var),
                field: field.name,
            };
            HirExpr::Lambda {
                id: id_gen.next(),
                param,
                body: Box::new(body),
            }
        }
        Expr::Index { base, index, span } => HirExpr::Index {
            id: id_gen.next(),
            base: Box::new(lower_expr_ctx(*base, id_gen, ctx, false)),
            index: Box::new(lower_expr_ctx(*index, id_gen, ctx, false)),
            location: ctx.source_path.map(|path| {
                format!("{}:{}:{}", path, span.start.line, span.start.column)
            }),
        },
        Expr::Call { func, args, .. } => HirExpr::Call {
            id: id_gen.next(),
            func: Box::new(lower_expr_ctx(*func, id_gen, ctx, false)),
            args: args
                .into_iter()
                .map(|arg| lower_expr_ctx(arg, id_gen, ctx, false))
                .collect(),
        },
        Expr::Lambda { params, body, .. } => {
            let body = lower_expr_ctx(*body, id_gen, ctx, false);
            lower_lambda_hir(params, body, id_gen)
        }
        Expr::Match {
            scrutinee, arms, ..
        } => {
            let scrutinee = if let Some(scrutinee) = scrutinee {
                lower_expr_ctx(*scrutinee, id_gen, ctx, false)
            } else {
                let param = "_arg0".to_string();
                let var = HirExpr::Var {
                    id: id_gen.next(),
                    name: param.clone(),
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
                            body: lower_expr_ctx(arm.body, id_gen, ctx, false),
                        })
                        .collect(),
                };
                return HirExpr::Lambda {
                    id: id_gen.next(),
                    param,
                    body: Box::new(match_expr),
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
                        body: lower_expr_ctx(arm.body, id_gen, ctx, false),
                    })
                    .collect(),
            }
        }
        Expr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => HirExpr::If {
            id: id_gen.next(),
            cond: Box::new(lower_expr_ctx(*cond, id_gen, ctx, false)),
            then_branch: Box::new(lower_expr_ctx(*then_branch, id_gen, ctx, false)),
            else_branch: Box::new(lower_expr_ctx(*else_branch, id_gen, ctx, false)),
        },
        Expr::Binary {
            op, left, right, ..
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
                };
            }
            if op == "<|" {
                if let Expr::Record { fields, .. } = *right.clone() {
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
                                            HirPathSegment::Index(lower_expr_ctx(expr, id_gen, ctx, false))
                                        }
                                        crate::surface::PathSegment::All(_) => HirPathSegment::All,
                                    })
                                    .collect(),
                                value: lower_expr_ctx(field.value, id_gen, ctx, false),
                            })
                            .collect(),
                    };
                }
            }
            HirExpr::Binary {
                id: id_gen.next(),
                op,
                left: Box::new(lower_expr_ctx(*left, id_gen, ctx, false)),
                right: Box::new(lower_expr_ctx(*right, id_gen, ctx, false)),
            }
        }
        Expr::Block { kind, items, .. } => {
            let block_kind = lower_block_kind(&kind);
            // Generic `do M { ... }` blocks (where M ≠ Effect) desugar into
            // nested `chain` / lambda calls so that the runtime resolves
            // `chain`/`of` through the normal Monad instance dispatch.
            if let BlockKind::Do { ref monad } = kind {
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
        | Expr::Block { span, .. }
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
        return Some(slice_line(lines[start_line], span.start.column, span.end.column));
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

fn lower_pipe_chain(left: Expr, right: Expr, id_gen: &mut IdGen, ctx: &mut LowerCtx<'_>) -> HirExpr {
    let Some(_) = ctx.debug.as_ref() else {
        let left = lower_expr_ctx(left, id_gen, ctx, true);
        let right = lower_expr_ctx(right, id_gen, ctx, false);
        return HirExpr::App {
            id: id_gen.next(),
            func: Box::new(right),
            arg: Box::new(left),
        };
    };

    let right_span = surface_expr_span(&right);
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
            base = Expr::Binary { op, left, right, span };
            break;
        }
        let step_span = surface_expr_span(&right);
        steps.push((*right, step_span));
        base = *left;
    }
    steps.reverse();

    let (pipe_id, source, log_time) = {
        let debug = ctx.debug.as_mut().expect("debug ctx");
        (debug.alloc_pipe_id(), debug.source, debug.params.time)
    };
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
        };
    }
    acc
}

fn lower_lambda_hir(params: Vec<Pattern>, body: HirExpr, id_gen: &mut IdGen) -> HirExpr {
    let mut acc = body;
    for (index, param) in params.into_iter().rev().enumerate() {
        match param {
            Pattern::Ident(name) => {
                acc = HirExpr::Lambda {
                    id: id_gen.next(),
                    param: name.name,
                    body: Box::new(acc),
                };
            }
            Pattern::SubjectIdent(name) => {
                acc = HirExpr::Lambda {
                    id: id_gen.next(),
                    param: name.name,
                    body: Box::new(acc),
                };
            }
            Pattern::Wildcard(_) => {
                acc = HirExpr::Lambda {
                    id: id_gen.next(),
                    param: format!("_arg{}", index),
                    body: Box::new(acc),
                };
            }
            other => {
                let param_name = format!("_arg{}", index);
                let match_expr = HirExpr::Match {
                    id: id_gen.next(),
                    scrutinee: Box::new(HirExpr::Var {
                        id: id_gen.next(),
                        name: param_name.clone(),
                    }),
                    arms: vec![HirMatchArm {
                        pattern: lower_pattern(other, id_gen),
                        guard: None,
                        body: acc,
                    }],
                };
                acc = HirExpr::Lambda {
                    id: id_gen.next(),
                    param: param_name,
                    body: Box::new(match_expr),
                };
            }
        }
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
fn desugar_generic_do_block(
    items: Vec<BlockItem>,
    surface_kind: &BlockKind,
    hir_kind: &HirBlockKind,
    id_gen: &mut IdGen,
    ctx: &mut LowerCtx<'_>,
) -> HirExpr {
    if items.is_empty() {
        // `do M {}` → `of Unit`
        return HirExpr::Call {
            id: id_gen.next(),
            func: Box::new(HirExpr::Var {
                id: id_gen.next(),
                name: "of".to_string(),
            }),
            args: vec![HirExpr::Var {
                id: id_gen.next(),
                name: "Unit".to_string(),
            }],
        };
    }

    desugar_do_items(&items, 0, surface_kind, hir_kind, id_gen, ctx)
}

/// Recursively desugar do-block items starting at `index`.
fn desugar_do_items(
    items: &[BlockItem],
    index: usize,
    surface_kind: &BlockKind,
    hir_kind: &HirBlockKind,
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
                };
                // Wrap in `of` for final bind: `chain (λx. of x) e`
                let wrapped = HirExpr::Call {
                    id: id_gen.next(),
                    func: Box::new(HirExpr::Var {
                        id: id_gen.next(),
                        name: "of".to_string(),
                    }),
                    args: vec![body],
                };
                let continuation = make_pattern_lambda(
                    pattern.clone(),
                    wrapped,
                    &param,
                    id_gen,
                );
                // chain continuation rhs
                HirExpr::Call {
                    id: id_gen.next(),
                    func: Box::new(HirExpr::Var {
                        id: id_gen.next(),
                        name: "chain".to_string(),
                    }),
                    args: vec![continuation, rhs],
                }
            } else {
                let rest = desugar_do_items(items, index + 1, surface_kind, hir_kind, id_gen, ctx);
                let param = format!("__do_bind{}", id_gen.next());
                let continuation = make_pattern_lambda(
                    pattern.clone(),
                    rest,
                    &param,
                    id_gen,
                );
                // chain continuation rhs
                HirExpr::Call {
                    id: id_gen.next(),
                    func: Box::new(HirExpr::Var {
                        id: id_gen.next(),
                        name: "chain".to_string(),
                    }),
                    args: vec![continuation, rhs],
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
                        name: "of".to_string(),
                    }),
                    args: vec![rhs],
                }
            } else {
                let rest = desugar_do_items(items, index + 1, surface_kind, hir_kind, id_gen, ctx);
                let param = format!("__do_let{}", id_gen.next());
                let body = make_pattern_lambda(
                    pattern.clone(),
                    rest,
                    &param,
                    id_gen,
                );
                // (λpat. rest) rhs
                HirExpr::App {
                    id: id_gen.next(),
                    func: Box::new(body),
                    arg: Box::new(rhs),
                }
            }
        }
        // Expression statement: `e`
        BlockItem::Expr { expr, .. } => {
            let rhs = lower_expr_ctx(expr.clone(), id_gen, ctx, false);
            if is_last {
                // Final expression: must have type `M A`, returned directly.
                rhs
            } else {
                let rest = desugar_do_items(items, index + 1, surface_kind, hir_kind, id_gen, ctx);
                let param = format!("__do_seq{}", id_gen.next());
                let continuation = HirExpr::Lambda {
                    id: id_gen.next(),
                    param,
                    body: Box::new(rest),
                };
                // chain (λ_. rest) rhs
                HirExpr::Call {
                    id: id_gen.next(),
                    func: Box::new(HirExpr::Var {
                        id: id_gen.next(),
                        name: "chain".to_string(),
                    }),
                    args: vec![continuation, rhs],
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
        },
        Pattern::SubjectIdent(name) => HirExpr::Lambda {
            id: id_gen.next(),
            param: name.name.clone(),
            body: Box::new(body),
        },
        Pattern::Wildcard(_) => HirExpr::Lambda {
            id: id_gen.next(),
            param: fallback_param.to_string(),
            body: Box::new(body),
        },
        _ => {
            let match_expr = HirExpr::Match {
                id: id_gen.next(),
                scrutinee: Box::new(HirExpr::Var {
                    id: id_gen.next(),
                    name: fallback_param.to_string(),
                }),
                arms: vec![HirMatchArm {
                    pattern: lower_pattern(pattern, id_gen),
                    guard: None,
                    body,
                }],
            };
            HirExpr::Lambda {
                id: id_gen.next(),
                param: fallback_param.to_string(),
                body: Box::new(match_expr),
            }
        }
    }
}

fn lower_block_kind(kind: &BlockKind) -> HirBlockKind {
    match kind {
        BlockKind::Plain => HirBlockKind::Plain,
        BlockKind::Do { monad } => HirBlockKind::Do { monad: monad.name.clone() },
        BlockKind::Generate => HirBlockKind::Generate,
        BlockKind::Resource => HirBlockKind::Resource,
    }
}
