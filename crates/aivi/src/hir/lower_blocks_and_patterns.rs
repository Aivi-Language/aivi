
fn lower_block_item_ctx(
    item: BlockItem,
    surface_kind: &BlockKind,
    hir_kind: &HirBlockKind,
    id_gen: &mut IdGen,
    ctx: &mut LowerCtx<'_>,
) -> HirBlockItem {
    match item {
        BlockItem::Bind { pattern, expr, .. } => HirBlockItem::Bind {
            pattern: lower_pattern(pattern, id_gen),
            expr: lower_expr_ctx(expr, id_gen, ctx, false),
        },
        BlockItem::Let { pattern, expr, .. } => {
            let lowered_expr = lower_expr_ctx(expr, id_gen, ctx, false);
            let expr = if matches!(surface_kind, BlockKind::Do { .. })
                && matches!(hir_kind, HirBlockKind::Effect)
            {
                // `name = expr` inside `effect { ... }` is a pure let-binding and must not
                // implicitly run effects even if `expr` produces an `Effect` value.
                HirExpr::Call {
                    id: id_gen.next(),
                    func: Box::new(HirExpr::Var {
                        id: id_gen.next(),
                        name: "pure".to_string(),
                    }),
                    args: vec![lowered_expr],
                }
            } else {
                lowered_expr
            };
            HirBlockItem::Bind {
                pattern: lower_pattern(pattern, id_gen),
                expr,
            }
        }
        BlockItem::Filter { expr, .. } => HirBlockItem::Filter {
            expr: lower_expr_ctx(expr, id_gen, ctx, false),
        },
        BlockItem::Yield { expr, .. } => HirBlockItem::Yield {
            expr: lower_expr_ctx(expr, id_gen, ctx, false),
        },
        BlockItem::Recurse { expr, .. } => HirBlockItem::Recurse {
            expr: lower_expr_ctx(expr, id_gen, ctx, false),
        },
        BlockItem::Expr { expr, .. } => HirBlockItem::Expr {
            expr: lower_expr_ctx(expr, id_gen, ctx, false),
        },
        // Desugar `when cond <- eff` → `_ <- if cond then eff else pure Unit`
        BlockItem::When { cond, effect, .. } => {
            let cond_hir = lower_expr_ctx(cond, id_gen, ctx, false);
            let eff_hir = lower_expr_ctx(effect, id_gen, ctx, false);
            let pure_unit = HirExpr::Call {
                id: id_gen.next(),
                func: Box::new(HirExpr::Var {
                    id: id_gen.next(),
                    name: "pure".to_string(),
                }),
                args: vec![HirExpr::Var {
                    id: id_gen.next(),
                    name: "Unit".to_string(),
                }],
            };
            HirBlockItem::Bind {
                pattern: HirPattern::Wildcard { id: id_gen.next() },
                expr: HirExpr::If {
                    id: id_gen.next(),
                    cond: Box::new(cond_hir),
                    then_branch: Box::new(eff_hir),
                    else_branch: Box::new(pure_unit),
                },
            }
        }
        // Desugar `given cond or failExpr` → `_ <- if cond then pure Unit else failExpr`
        BlockItem::Given { cond, fail_expr, .. } => {
            let cond_hir = lower_expr_ctx(cond, id_gen, ctx, false);
            let fail_hir = lower_expr_ctx(fail_expr, id_gen, ctx, false);
            let pure_unit = HirExpr::Call {
                id: id_gen.next(),
                func: Box::new(HirExpr::Var {
                    id: id_gen.next(),
                    name: "pure".to_string(),
                }),
                args: vec![HirExpr::Var {
                    id: id_gen.next(),
                    name: "Unit".to_string(),
                }],
            };
            HirBlockItem::Bind {
                pattern: HirPattern::Wildcard { id: id_gen.next() },
                expr: HirExpr::If {
                    id: id_gen.next(),
                    cond: Box::new(cond_hir),
                    then_branch: Box::new(pure_unit),
                    else_branch: Box::new(fail_hir),
                },
            }
        }
        // `on transition => handler` — lower as expression statement for now
        BlockItem::On { handler, .. } => HirBlockItem::Expr {
            expr: lower_expr_ctx(handler, id_gen, ctx, false),
        },
    }
}

fn lower_pattern(pattern: Pattern, id_gen: &mut IdGen) -> HirPattern {
    match pattern {
        Pattern::Wildcard(_) => HirPattern::Wildcard { id: id_gen.next() },
        Pattern::Ident(name) => HirPattern::Var {
            id: id_gen.next(),
            name: name.name,
        },
        Pattern::SubjectIdent(name) => HirPattern::Var {
            id: id_gen.next(),
            name: name.name,
        },
        Pattern::At {
            name, pattern, ..
        } => HirPattern::At {
            id: id_gen.next(),
            name: name.name,
            pattern: Box::new(lower_pattern(*pattern, id_gen)),
        },
        Pattern::Literal(literal) => HirPattern::Literal {
            id: id_gen.next(),
            value: match literal {
                crate::surface::Literal::Number { text, .. } => HirLiteral::Number(text),
                crate::surface::Literal::String { text, .. } => HirLiteral::String(text),
                crate::surface::Literal::Sigil {
                    tag, body, flags, ..
                } => HirLiteral::Sigil { tag, body, flags },
                crate::surface::Literal::Bool { value, .. } => HirLiteral::Bool(value),
                crate::surface::Literal::DateTime { text, .. } => HirLiteral::DateTime(text),
            },
        },
        Pattern::Constructor { name, args, .. } => HirPattern::Constructor {
            id: id_gen.next(),
            name: name.name,
            args: args
                .into_iter()
                .map(|arg| lower_pattern(arg, id_gen))
                .collect(),
        },
        Pattern::Tuple { items, .. } => HirPattern::Tuple {
            id: id_gen.next(),
            items: items
                .into_iter()
                .map(|item| lower_pattern(item, id_gen))
                .collect(),
        },
        Pattern::List { items, rest, .. } => HirPattern::List {
            id: id_gen.next(),
            items: items
                .into_iter()
                .map(|item| lower_pattern(item, id_gen))
                .collect(),
            rest: rest.map(|rest| Box::new(lower_pattern(*rest, id_gen))),
        },
        Pattern::Record { fields, .. } => HirPattern::Record {
            id: id_gen.next(),
            fields: fields
                .into_iter()
                .map(|field| HirRecordPatternField {
                    path: field.path.into_iter().map(|name| name.name).collect(),
                    pattern: lower_pattern(field.pattern, id_gen),
                })
                .collect(),
        },
    }
}

fn contains_placeholder(expr: &Expr) -> bool {
    match expr {
        Expr::Ident(name) => name.name == "_",
        Expr::Literal(_) => false,
        Expr::UnaryNeg { expr, .. } => contains_placeholder(expr),
        Expr::Suffixed { base, .. } => contains_placeholder(base),
        Expr::TextInterpolate { parts, .. } => parts.iter().any(|part| match part {
            TextPart::Text { .. } => false,
            TextPart::Expr { expr, .. } => contains_placeholder(expr),
        }),
        Expr::List { items, .. } => items.iter().any(|item| contains_placeholder(&item.expr)),
        Expr::Tuple { items, .. } => items.iter().any(contains_placeholder),
        Expr::Record { fields, .. } => fields.iter().any(|field| {
            field.path.iter().any(|segment| match segment {
                crate::surface::PathSegment::Index(expr, _) => contains_placeholder(expr),
                crate::surface::PathSegment::Field(_) | crate::surface::PathSegment::All(_) => {
                    false
                }
            }) || contains_placeholder(&field.value)
        }),
        Expr::PatchLit { fields, .. } => fields.iter().any(|field| {
            field.path.iter().any(|segment| match segment {
                crate::surface::PathSegment::Index(expr, _) => contains_placeholder(expr),
                crate::surface::PathSegment::Field(_) | crate::surface::PathSegment::All(_) => {
                    false
                }
            }) || contains_placeholder(&field.value)
        }),
        Expr::FieldAccess { base, .. } => contains_placeholder(base),
        // Field sections (`.field`) are handled directly during lowering.
        Expr::FieldSection { .. } => false,
        Expr::Index { base, index, .. } => {
            contains_placeholder(base) || contains_placeholder(index)
        }
        Expr::Call { func, args, .. } => {
            contains_placeholder(func) || args.iter().any(contains_placeholder)
        }
        Expr::Lambda { body, .. } => contains_placeholder(body),
        Expr::Match {
            scrutinee, arms, ..
        } => {
            scrutinee.as_deref().is_some_and(contains_placeholder)
                || arms.iter().any(|arm| {
                    arm.guard.as_ref().is_some_and(contains_placeholder)
                        || contains_placeholder(&arm.body)
                })
        }
        Expr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            contains_placeholder(cond)
                || contains_placeholder(then_branch)
                || contains_placeholder(else_branch)
        }
        Expr::Binary { left, right, .. } => {
            contains_placeholder(left) || contains_placeholder(right)
        }
        Expr::Block { items, .. } => items.iter().any(|item| match item {
            BlockItem::Bind { expr, .. } => contains_placeholder(expr),
            BlockItem::Let { expr, .. } => contains_placeholder(expr),
            BlockItem::Filter { expr, .. }
            | BlockItem::Yield { expr, .. }
            | BlockItem::Recurse { expr, .. }
            | BlockItem::Expr { expr, .. } => contains_placeholder(expr),
            BlockItem::When { cond, effect, .. } => {
                contains_placeholder(cond) || contains_placeholder(effect)
            }
            BlockItem::Given { cond, fail_expr, .. } => {
                contains_placeholder(cond) || contains_placeholder(fail_expr)
            }
            BlockItem::On { transition, handler, .. } => {
                contains_placeholder(transition) || contains_placeholder(handler)
            }
        }),
        Expr::Raw { .. } => false,
    }
}

fn desugar_placeholder_lambdas(expr: Expr) -> Expr {
    let expr = match expr {
        Expr::UnaryNeg { expr, span } => Expr::UnaryNeg {
            expr: Box::new(desugar_placeholder_lambdas(*expr)),
            span,
        },
        Expr::Ident(name) => {
            // Don't desugar a placeholder `_` at the leaf; let the smallest
            // enclosing expression scope capture it. A bare `_` is handled in
            // `lower_expr_ctx` (which special-cases a leaf `_` into a lambda).
            if name.name == "_" {
                return Expr::Ident(name);
            }
            Expr::Ident(name)
        }
        Expr::Literal(_) | Expr::Raw { .. } | Expr::FieldSection { .. } => expr,
        Expr::Suffixed { base, suffix, span } => Expr::Suffixed {
            base: Box::new(desugar_placeholder_lambdas(*base)),
            suffix,
            span,
        },
        Expr::TextInterpolate { parts, span } => Expr::TextInterpolate {
            parts: parts
                .into_iter()
                .map(|part| match part {
                    TextPart::Text { .. } => part,
                    TextPart::Expr { expr, span } => TextPart::Expr {
                        expr: Box::new(desugar_placeholder_lambdas(*expr)),
                        span,
                    },
                })
                .collect(),
            span,
        },
        Expr::List { items, span } => Expr::List {
            items: items
                .into_iter()
                .map(|item| crate::surface::ListItem {
                    expr: desugar_placeholder_lambdas(item.expr),
                    spread: item.spread,
                    span: item.span,
                })
                .collect(),
            span,
        },
        Expr::Tuple { items, span } => Expr::Tuple {
            items: items.into_iter().map(desugar_placeholder_lambdas).collect(),
            span,
        },
        Expr::Record { fields, span } => Expr::Record {
            fields: fields
                .into_iter()
                .map(|field| crate::surface::RecordField {
                    spread: field.spread,
                    path: field
                        .path
                        .into_iter()
                        .map(|segment| match segment {
                            crate::surface::PathSegment::Field(name) => {
                                crate::surface::PathSegment::Field(name)
                            }
                            crate::surface::PathSegment::Index(expr, span) => {
                                crate::surface::PathSegment::Index(
                                    desugar_placeholder_lambdas(expr),
                                    span,
                                )
                            }
                            crate::surface::PathSegment::All(span) => {
                                crate::surface::PathSegment::All(span)
                            }
                        })
                        .collect(),
                    value: desugar_placeholder_lambdas(field.value),
                    span: field.span,
                })
                .collect(),
            span,
        },
        Expr::PatchLit { fields, span } => Expr::PatchLit {
            fields: fields
                .into_iter()
                .map(|field| crate::surface::RecordField {
                    spread: field.spread,
                    path: field
                        .path
                        .into_iter()
                        .map(|segment| match segment {
                            crate::surface::PathSegment::Field(name) => {
                                crate::surface::PathSegment::Field(name)
                            }
                            crate::surface::PathSegment::Index(expr, span) => {
                                crate::surface::PathSegment::Index(
                                    desugar_placeholder_lambdas(expr),
                                    span,
                                )
                            }
                            crate::surface::PathSegment::All(span) => {
                                crate::surface::PathSegment::All(span)
                            }
                        })
                        .collect(),
                    value: desugar_placeholder_lambdas(field.value),
                    span: field.span,
                })
                .collect(),
            span,
        },
        Expr::FieldAccess { base, field, span } => Expr::FieldAccess {
            base: Box::new(desugar_placeholder_lambdas(*base)),
            field,
            span,
        },
        Expr::Index { base, index, span } => Expr::Index {
            base: Box::new(desugar_placeholder_lambdas(*base)),
            index: Box::new(desugar_placeholder_lambdas(*index)),
            span,
        },
        Expr::Call { func, args, span } => Expr::Call {
            func: Box::new(desugar_placeholder_lambdas(*func)),
            args: args.into_iter().map(desugar_placeholder_lambdas).collect(),
            span,
        },
        Expr::Lambda { params, body, span } => Expr::Lambda {
            params,
            body: Box::new(desugar_placeholder_lambdas(*body)),
            span,
        },
        Expr::Match {
            scrutinee,
            arms,
            span,
        } => Expr::Match {
            scrutinee: scrutinee.map(|expr| Box::new(desugar_placeholder_lambdas(*expr))),
            arms: arms
                .into_iter()
                .map(|arm| crate::surface::MatchArm {
                    pattern: arm.pattern,
                    guard: arm.guard.map(desugar_placeholder_lambdas),
                    body: desugar_placeholder_lambdas(arm.body),
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
            cond: Box::new(desugar_placeholder_lambdas(*cond)),
            then_branch: Box::new(desugar_placeholder_lambdas(*then_branch)),
            else_branch: Box::new(desugar_placeholder_lambdas(*else_branch)),
            span,
        },
        Expr::Binary {
            op,
            left,
            right,
            span,
        } => Expr::Binary {
            op,
            left: Box::new(desugar_placeholder_lambdas(*left)),
            right: Box::new(desugar_placeholder_lambdas(*right)),
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
                        expr: desugar_placeholder_lambdas(expr),
                        span,
                    },
                    BlockItem::Let {
                        pattern,
                        expr,
                        span,
                    } => BlockItem::Let {
                        pattern,
                        expr: desugar_placeholder_lambdas(expr),
                        span,
                    },
                    BlockItem::Filter { expr, span } => BlockItem::Filter {
                        expr: desugar_placeholder_lambdas(expr),
                        span,
                    },
                    BlockItem::Yield { expr, span } => BlockItem::Yield {
                        expr: desugar_placeholder_lambdas(expr),
                        span,
                    },
                    BlockItem::Recurse { expr, span } => BlockItem::Recurse {
                        expr: desugar_placeholder_lambdas(expr),
                        span,
                    },
                    BlockItem::Expr { expr, span } => BlockItem::Expr {
                        expr: desugar_placeholder_lambdas(expr),
                        span,
                    },
                    BlockItem::When { cond, effect, span } => BlockItem::When {
                        cond: desugar_placeholder_lambdas(cond),
                        effect: desugar_placeholder_lambdas(effect),
                        span,
                    },
                    BlockItem::Given { cond, fail_expr, span } => BlockItem::Given {
                        cond: desugar_placeholder_lambdas(cond),
                        fail_expr: desugar_placeholder_lambdas(fail_expr),
                        span,
                    },
                    BlockItem::On { transition, handler, span } => BlockItem::On {
                        transition: desugar_placeholder_lambdas(transition),
                        handler: desugar_placeholder_lambdas(handler),
                        span,
                    },
                })
                .collect(),
            span,
        },
    };

    if !contains_placeholder(&expr) {
        return expr;
    }

    let (rewritten, params) = replace_holes(expr);
    let span = match &rewritten {
        Expr::Ident(name) => name.span.clone(),
        Expr::Literal(lit) => match lit {
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
    };

    Expr::Lambda {
        params: params
            .into_iter()
            .map(|name| {
                Pattern::Ident(crate::surface::SpannedName {
                    name,
                    span: span.clone(),
                })
            })
            .collect(),
        body: Box::new(rewritten),
        span,
    }
}

fn replace_holes(expr: Expr) -> (Expr, Vec<String>) {
    let mut counter = 0;
    let mut params = Vec::new();
    let rewritten = replace_holes_inner(expr, &mut counter, &mut params);
    (rewritten, params)
}

fn replace_holes_inner(expr: Expr, counter: &mut u32, params: &mut Vec<String>) -> Expr {
    match expr {
        Expr::UnaryNeg { expr, span } => Expr::UnaryNeg {
            expr: Box::new(replace_holes_inner(*expr, counter, params)),
            span,
        },
        Expr::Ident(name) if name.name == "_" => {
            let param = format!("_arg{}", counter);
            *counter += 1;
            params.push(param.clone());
            Expr::Ident(crate::surface::SpannedName {
                name: param,
                span: name.span,
            })
        }
        Expr::Ident(_) | Expr::Literal(_) | Expr::Raw { .. } => expr,
        Expr::Suffixed { base, suffix, span } => Expr::Suffixed {
            base: Box::new(replace_holes_inner(*base, counter, params)),
            suffix,
            span,
        },
        Expr::TextInterpolate { parts, span } => Expr::TextInterpolate {
            parts: parts
                .into_iter()
                .map(|part| match part {
                    TextPart::Text { .. } => part,
                    TextPart::Expr { expr, span } => TextPart::Expr {
                        expr: Box::new(replace_holes_inner(*expr, counter, params)),
                        span,
                    },
                })
                .collect(),
            span,
        },
        Expr::List { items, span } => Expr::List {
            items: items
                .into_iter()
                .map(|item| crate::surface::ListItem {
                    expr: replace_holes_inner(item.expr, counter, params),
                    spread: item.spread,
                    span: item.span,
                })
                .collect(),
            span,
        },
        Expr::Tuple { items, span } => Expr::Tuple {
            items: items
                .into_iter()
                .map(|item| replace_holes_inner(item, counter, params))
                .collect(),
            span,
        },
        Expr::Record { fields, span } => Expr::Record {
            fields: fields
                .into_iter()
                .map(|field| crate::surface::RecordField {
                    spread: field.spread,
                    path: field
                        .path
                        .into_iter()
                        .map(|segment| match segment {
                            crate::surface::PathSegment::Field(name) => {
                                crate::surface::PathSegment::Field(name)
                            }
                            crate::surface::PathSegment::Index(expr, span) => {
                                crate::surface::PathSegment::Index(
                                    replace_holes_inner(expr, counter, params),
                                    span,
                                )
                            }
                            crate::surface::PathSegment::All(span) => {
                                crate::surface::PathSegment::All(span)
                            }
                        })
                        .collect(),
                    value: replace_holes_inner(field.value, counter, params),
                    span: field.span,
                })
                .collect(),
            span,
        },
        Expr::PatchLit { fields, span } => Expr::PatchLit {
            fields: fields
                .into_iter()
                .map(|field| crate::surface::RecordField {
                    spread: field.spread,
                    path: field
                        .path
                        .into_iter()
                        .map(|segment| match segment {
                            crate::surface::PathSegment::Field(name) => {
                                crate::surface::PathSegment::Field(name)
                            }
                            crate::surface::PathSegment::Index(expr, span) => {
                                crate::surface::PathSegment::Index(
                                    replace_holes_inner(expr, counter, params),
                                    span,
                                )
                            }
                            crate::surface::PathSegment::All(span) => {
                                crate::surface::PathSegment::All(span)
                            }
                        })
                        .collect(),
                    value: replace_holes_inner(field.value, counter, params),
                    span: field.span,
                })
                .collect(),
            span,
        },
        Expr::FieldAccess { base, field, span } => Expr::FieldAccess {
            base: Box::new(replace_holes_inner(*base, counter, params)),
            field,
            span,
        },
        Expr::FieldSection { .. } => expr,
        Expr::Index { base, index, span } => Expr::Index {
            base: Box::new(replace_holes_inner(*base, counter, params)),
            index: Box::new(replace_holes_inner(*index, counter, params)),
            span,
        },
        Expr::Call { func, args, span } => Expr::Call {
            func: Box::new(replace_holes_inner(*func, counter, params)),
            args: args
                .into_iter()
                .map(|arg| replace_holes_inner(arg, counter, params))
                .collect(),
            span,
        },
        Expr::Lambda {
            params: lambda_params,
            body,
            span,
        } => Expr::Lambda {
            params: lambda_params,
            body: Box::new(replace_holes_inner(*body, counter, params)),
            span,
        },
        Expr::Match {
            scrutinee,
            arms,
            span,
        } => Expr::Match {
            scrutinee: scrutinee.map(|expr| Box::new(replace_holes_inner(*expr, counter, params))),
            arms: arms
                .into_iter()
                .map(|arm| crate::surface::MatchArm {
                    pattern: arm.pattern,
                    guard: arm
                        .guard
                        .map(|guard| replace_holes_inner(guard, counter, params)),
                    body: replace_holes_inner(arm.body, counter, params),
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
            cond: Box::new(replace_holes_inner(*cond, counter, params)),
            then_branch: Box::new(replace_holes_inner(*then_branch, counter, params)),
            else_branch: Box::new(replace_holes_inner(*else_branch, counter, params)),
            span,
        },
        Expr::Binary {
            op,
            left,
            right,
            span,
        } => Expr::Binary {
            op,
            left: Box::new(replace_holes_inner(*left, counter, params)),
            right: Box::new(replace_holes_inner(*right, counter, params)),
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
                        expr: replace_holes_inner(expr, counter, params),
                        span,
                    },
                    BlockItem::Let {
                        pattern,
                        expr,
                        span,
                    } => BlockItem::Let {
                        pattern,
                        expr: replace_holes_inner(expr, counter, params),
                        span,
                    },
                    BlockItem::Filter { expr, span } => BlockItem::Filter {
                        expr: replace_holes_inner(expr, counter, params),
                        span,
                    },
                    BlockItem::Yield { expr, span } => BlockItem::Yield {
                        expr: replace_holes_inner(expr, counter, params),
                        span,
                    },
                    BlockItem::Recurse { expr, span } => BlockItem::Recurse {
                        expr: replace_holes_inner(expr, counter, params),
                        span,
                    },
                    BlockItem::Expr { expr, span } => BlockItem::Expr {
                        expr: replace_holes_inner(expr, counter, params),
                        span,
                    },
                    BlockItem::When { cond, effect, span } => BlockItem::When {
                        cond: replace_holes_inner(cond, counter, params),
                        effect: replace_holes_inner(effect, counter, params),
                        span,
                    },
                    BlockItem::Given { cond, fail_expr, span } => BlockItem::Given {
                        cond: replace_holes_inner(cond, counter, params),
                        fail_expr: replace_holes_inner(fail_expr, counter, params),
                        span,
                    },
                    BlockItem::On { transition, handler, span } => BlockItem::On {
                        transition: replace_holes_inner(transition, counter, params),
                        handler: replace_holes_inner(handler, counter, params),
                        span,
                    },
                })
                .collect(),
            span,
        },
    }
}

#[derive(Default)]
struct IdGen {
    next: u32,
}

impl IdGen {
    fn next(&mut self) -> u32 {
        let id = self.next;
        self.next += 1;
        id
    }
}
