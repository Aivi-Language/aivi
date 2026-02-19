fn expr_span(expr: &Expr) -> Span {
    match expr {
        Expr::Ident(name) => name.span.clone(),
        Expr::Literal(literal) => literal_span(literal),
        Expr::UnaryNeg { span, .. } => span.clone(),
        Expr::Suffixed { span, .. } => span.clone(),
        Expr::TextInterpolate { span, .. } => span.clone(),
        Expr::List { span, .. }
        | Expr::Tuple { span, .. }
        | Expr::Record { span, .. }
        | Expr::PatchLit { span, .. }
        | Expr::FieldAccess { span, .. }
        | Expr::FieldSection { span, .. }
        | Expr::Index { span, .. }
        | Expr::Call { span, .. }
        | Expr::Lambda { span, .. }
        | Expr::Match { span, .. }
        | Expr::If { span, .. }
        | Expr::Binary { span, .. }
        | Expr::Block { span, .. } => span.clone(),
        Expr::Raw { span, .. } => span.clone(),
    }
}

fn pattern_span(pattern: &Pattern) -> Span {
    match pattern {
        Pattern::Wildcard(span) => span.clone(),
        Pattern::Ident(name) => name.span.clone(),
        Pattern::SubjectIdent(name) => name.span.clone(),
        Pattern::Literal(literal) => literal_span(literal),
        Pattern::At { span, .. } => span.clone(),
        Pattern::Constructor { span, .. }
        | Pattern::Tuple { span, .. }
        | Pattern::List { span, .. }
        | Pattern::Record { span, .. } => span.clone(),
    }
}

fn literal_span(literal: &Literal) -> Span {
    match literal {
        Literal::Number { span, .. }
        | Literal::String { span, .. }
        | Literal::Sigil { span, .. }
        | Literal::Bool { span, .. }
        | Literal::DateTime { span, .. } => span.clone(),
    }
}

fn is_range_expr(expr: &Expr) -> bool {
    matches!(expr, Expr::Binary { op, .. } if op == "..")
}

fn desugar_holes(expr: Expr) -> Expr {
    desugar_holes_inner(expr, true)
}

fn desugar_holes_inner(expr: Expr, is_root: bool) -> Expr {
    let expr = match expr {
        Expr::UnaryNeg { expr, span } => Expr::UnaryNeg {
            expr: Box::new(desugar_holes_inner(*expr, false)),
            span,
        },
        Expr::Suffixed { base, suffix, span } => Expr::Suffixed {
            base: Box::new(desugar_holes_inner(*base, false)),
            suffix,
            span,
        },
        Expr::TextInterpolate { parts, span } => Expr::TextInterpolate {
            parts: parts
                .into_iter()
                .map(|part| match part {
                    TextPart::Text { .. } => part,
                    TextPart::Expr { expr, span } => TextPart::Expr {
                        expr: Box::new(desugar_holes_inner(*expr, false)),
                        span,
                    },
                })
                .collect(),
            span,
        },
        Expr::List { items, span } => {
            let items = items
                .into_iter()
                .map(|mut item| {
                    item.expr = desugar_holes_inner(item.expr, false);
                    item
                })
                .collect();
            Expr::List { items, span }
        }
        Expr::Tuple { items, span } => Expr::Tuple {
            items: items
                .into_iter()
                .map(|item| desugar_holes_inner(item, false))
                .collect(),
            span,
        },
        Expr::Record { fields, span } => {
            let fields = fields
                .into_iter()
                .map(|mut field| {
                    let path = field
                        .path
                        .into_iter()
                        .map(|segment| match segment {
                            PathSegment::Index(expr, span) => {
                                PathSegment::Index(desugar_holes_inner(expr, false), span)
                            }
                            PathSegment::Field(name) => PathSegment::Field(name),
                            PathSegment::All(span) => PathSegment::All(span),
                        })
                        .collect();
                    field.path = path;
                    field.value = desugar_holes_inner(field.value, false);
                    field
                })
                .collect();
            Expr::Record { fields, span }
        }
        Expr::PatchLit { fields, span } => {
            let fields = fields
                .into_iter()
                .map(|mut field| {
                    let path = field
                        .path
                        .into_iter()
                        .map(|segment| match segment {
                            PathSegment::Index(expr, span) => {
                                PathSegment::Index(desugar_holes_inner(expr, false), span)
                            }
                            PathSegment::Field(name) => PathSegment::Field(name),
                            PathSegment::All(span) => PathSegment::All(span),
                        })
                        .collect();
                    field.path = path;
                    field.value = desugar_holes_inner(field.value, false);
                    field
                })
                .collect();
            Expr::PatchLit { fields, span }
        }
        Expr::FieldAccess { base, field, span } => Expr::FieldAccess {
            base: Box::new(desugar_holes_inner(*base, false)),
            field,
            span,
        },
        Expr::FieldSection { field, span } => Expr::FieldSection { field, span },
        Expr::Index { base, index, span } => Expr::Index {
            base: Box::new(desugar_holes_inner(*base, false)),
            index: Box::new(desugar_holes_inner(*index, false)),
            span,
        },
        Expr::Call { func, args, span } => Expr::Call {
            func: Box::new(desugar_holes_inner(*func, false)),
            args: args
                .into_iter()
                .map(|arg| desugar_holes_inner(arg, false))
                .collect(),
            span,
        },
        Expr::Lambda { params, body, span } => Expr::Lambda {
            params,
            body: Box::new(desugar_holes_inner(*body, false)),
            span,
        },
        Expr::Match {
            scrutinee,
            arms,
            span,
        } => {
            let scrutinee = scrutinee.map(|expr| Box::new(desugar_holes_inner(*expr, false)));
            let arms = arms
                .into_iter()
                .map(|mut arm| {
                    arm.guard = arm.guard.map(|guard| desugar_holes_inner(guard, false));
                    arm.body = desugar_holes_inner(arm.body, false);
                    arm
                })
                .collect();
            Expr::Match {
                scrutinee,
                arms,
                span,
            }
        }
        Expr::If {
            cond,
            then_branch,
            else_branch,
            span,
        } => Expr::If {
            cond: Box::new(desugar_holes_inner(*cond, false)),
            then_branch: Box::new(desugar_holes_inner(*then_branch, false)),
            else_branch: Box::new(desugar_holes_inner(*else_branch, false)),
            span,
        },
        Expr::Binary {
            op,
            left,
            right,
            span,
        } => Expr::Binary {
            op,
            left: Box::new(desugar_holes_inner(*left, false)),
            right: Box::new(desugar_holes_inner(*right, false)),
            span,
        },
        Expr::Block { kind, items, span } => {
            let items = items
                .into_iter()
                .map(|mut item| {
                    match &mut item {
                        BlockItem::Bind { expr, .. }
                        | BlockItem::Let { expr, .. }
                        | BlockItem::Yield { expr, .. }
                        | BlockItem::Recurse { expr, .. }
                        | BlockItem::Expr { expr, .. } => {
                            *expr = desugar_holes_inner(expr.clone(), false);
                        }
                        BlockItem::Filter { .. } => {}
                        BlockItem::When { cond, effect, .. }
                        | BlockItem::Unless { cond, effect, .. } => {
                            *cond = desugar_holes_inner(cond.clone(), false);
                            *effect = desugar_holes_inner(effect.clone(), false);
                        }
                        BlockItem::Given { cond, fail_expr, .. } => {
                            *cond = desugar_holes_inner(cond.clone(), false);
                            *fail_expr = desugar_holes_inner(fail_expr.clone(), false);
                        }
                        BlockItem::On { transition, handler, .. } => {
                            *transition = desugar_holes_inner(transition.clone(), false);
                            *handler = desugar_holes_inner(handler.clone(), false);
                        }
                    }
                    item
                })
                .collect();
            Expr::Block { kind, items, span }
        }
        Expr::Ident(name) => Expr::Ident(name),
        Expr::Literal(literal) => Expr::Literal(literal),
        Expr::Raw { text, span } => Expr::Raw { text, span },
    };
    if !is_root && matches!(&expr, Expr::Ident(name) if name.name == "_") {
        return expr;
    }
    if !contains_hole(&expr) {
        return expr;
    }
    let (rewritten, params) = replace_holes(expr);
    let mut acc = rewritten;
    for param in params.into_iter().rev() {
        let span = expr_span(&acc);
        acc = Expr::Lambda {
            params: vec![Pattern::Ident(SpannedName {
                name: param,
                span: span.clone(),
            })],
            body: Box::new(acc),
            span,
        };
    }
    acc
}

fn contains_hole(expr: &Expr) -> bool {
    match expr {
        Expr::Ident(name) => name.name == "_",
        Expr::Literal(_) => false,
        Expr::UnaryNeg { expr, .. } => contains_hole(expr),
        Expr::Suffixed { base, .. } => contains_hole(base),
        Expr::TextInterpolate { parts, .. } => parts.iter().any(|part| match part {
            TextPart::Text { .. } => false,
            TextPart::Expr { expr, .. } => contains_hole(expr),
        }),
        Expr::List { items, .. } => items.iter().any(|item| contains_hole(&item.expr)),
        Expr::Tuple { items, .. } => items.iter().any(contains_hole),
        Expr::Record { fields, .. } => fields.iter().any(|field| {
            field.path.iter().any(|segment| match segment {
                PathSegment::Index(expr, _) => contains_hole(expr),
                PathSegment::Field(_) | PathSegment::All(_) => false,
            }) || contains_hole(&field.value)
        }),
        Expr::PatchLit { fields, .. } => fields.iter().any(|field| {
            field.path.iter().any(|segment| match segment {
                PathSegment::Index(expr, _) => contains_hole(expr),
                PathSegment::Field(_) | PathSegment::All(_) => false,
            }) || contains_hole(&field.value)
        }),
        Expr::FieldAccess { base, .. } => contains_hole(base),
        Expr::FieldSection { .. } => true,
        Expr::Index { base, index, .. } => contains_hole(base) || contains_hole(index),
        Expr::Call { func, args, .. } => contains_hole(func) || args.iter().any(contains_hole),
        Expr::Lambda { body, .. } => contains_hole(body),
        Expr::Match {
            scrutinee, arms, ..
        } => {
            scrutinee.as_deref().is_some_and(contains_hole)
                || arms.iter().any(|arm| {
                    arm.guard.as_ref().is_some_and(contains_hole) || contains_hole(&arm.body)
                })
        }
        Expr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => contains_hole(cond) || contains_hole(then_branch) || contains_hole(else_branch),
        Expr::Binary { left, right, .. } => contains_hole(left) || contains_hole(right),
        Expr::Block { items, .. } => items.iter().any(|item| match item {
            BlockItem::Bind { expr, .. } => contains_hole(expr),
            BlockItem::Let { expr, .. } => contains_hole(expr),
            BlockItem::Filter { expr, .. }
            | BlockItem::Yield { expr, .. }
            | BlockItem::Recurse { expr, .. }
            | BlockItem::Expr { expr, .. } => contains_hole(expr),
            BlockItem::When { cond, effect, .. }
            | BlockItem::Unless { cond, effect, .. } => contains_hole(cond) || contains_hole(effect),
            BlockItem::Given { cond, fail_expr, .. } => contains_hole(cond) || contains_hole(fail_expr),
            BlockItem::On { transition, handler, .. } => contains_hole(transition) || contains_hole(handler),
        }),
        Expr::Raw { .. } => false,
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
            Expr::Ident(SpannedName {
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
                .map(|field| RecordField {
                    spread: field.spread,
                    path: field
                        .path
                        .into_iter()
                        .map(|segment| match segment {
                            PathSegment::Field(name) => PathSegment::Field(name),
                            PathSegment::Index(expr, span) => {
                                PathSegment::Index(replace_holes_inner(expr, counter, params), span)
                            }
                            PathSegment::All(span) => PathSegment::All(span),
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
                .map(|field| RecordField {
                    spread: field.spread,
                    path: field
                        .path
                        .into_iter()
                        .map(|segment| match segment {
                            PathSegment::Field(name) => PathSegment::Field(name),
                            PathSegment::Index(expr, span) => {
                                PathSegment::Index(replace_holes_inner(expr, counter, params), span)
                            }
                            PathSegment::All(span) => PathSegment::All(span),
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
                    BlockItem::Unless { cond, effect, span } => BlockItem::Unless {
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
