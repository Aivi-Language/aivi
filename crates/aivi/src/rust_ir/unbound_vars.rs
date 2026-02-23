
fn collect_unbound_vars_in_kernel_expr(
    expr: &KernelExpr,
    globals: &[String],
    locals: &[String],
    bound: &mut Vec<String>,
    out: &mut HashSet<String>,
) {
    match expr {
        KernelExpr::Var { name, .. } => {
            let reserved = is_reserved_selector_name(name);
            let is_bound = bound.iter().rev().any(|b| b == name)
                || locals.iter().rev().any(|b| b == name)
                || (!reserved && globals.iter().any(|g| g == name))
                || (!reserved && resolve_builtin(name).is_some());
            if !is_bound {
                out.insert(name.clone());
            }
        }
        KernelExpr::LitNumber { .. }
        | KernelExpr::LitString { .. }
        | KernelExpr::LitSigil { .. }
        | KernelExpr::LitBool { .. }
        | KernelExpr::LitDateTime { .. }
        | KernelExpr::Raw { .. } => {}
        KernelExpr::TextInterpolate { parts, .. } => {
            for part in parts {
                if let crate::kernel::KernelTextPart::Expr { expr } = part {
                    collect_unbound_vars_in_kernel_expr(expr, globals, locals, bound, out);
                }
            }
        }
        KernelExpr::Lambda { param, body, .. } => {
            bound.push(param.clone());
            collect_unbound_vars_in_kernel_expr(body, globals, locals, bound, out);
            bound.pop();
        }
        KernelExpr::App { func, arg, .. } => {
            collect_unbound_vars_in_kernel_expr(func, globals, locals, bound, out);
            collect_unbound_vars_in_kernel_expr(arg, globals, locals, bound, out);
        }
        KernelExpr::Call { func, args, .. } => {
            collect_unbound_vars_in_kernel_expr(func, globals, locals, bound, out);
            for arg in args {
                collect_unbound_vars_in_kernel_expr(arg, globals, locals, bound, out);
            }
        }
        KernelExpr::DebugFn { body, .. } => {
            collect_unbound_vars_in_kernel_expr(body, globals, locals, bound, out);
        }
        KernelExpr::Pipe { func, arg, .. } => {
            collect_unbound_vars_in_kernel_expr(func, globals, locals, bound, out);
            collect_unbound_vars_in_kernel_expr(arg, globals, locals, bound, out);
        }
        KernelExpr::List { items, .. } => {
            for item in items {
                collect_unbound_vars_in_kernel_expr(&item.expr, globals, locals, bound, out);
            }
        }
        KernelExpr::Tuple { items, .. } => {
            for item in items {
                collect_unbound_vars_in_kernel_expr(item, globals, locals, bound, out);
            }
        }
        KernelExpr::Record { fields, .. } | KernelExpr::Patch { fields, .. } => {
            for field in fields {
                for seg in &field.path {
                    if let crate::kernel::KernelPathSegment::Index(expr) = seg {
                        collect_unbound_vars_in_kernel_expr(expr, globals, locals, bound, out);
                    }
                }
                collect_unbound_vars_in_kernel_expr(&field.value, globals, locals, bound, out);
            }
            if let KernelExpr::Patch { target, .. } = expr {
                collect_unbound_vars_in_kernel_expr(target, globals, locals, bound, out);
            }
        }
        KernelExpr::FieldAccess { base, .. } => {
            collect_unbound_vars_in_kernel_expr(base, globals, locals, bound, out);
        }
        KernelExpr::Index { base, index, .. } => {
            collect_unbound_vars_in_kernel_expr(base, globals, locals, bound, out);
            collect_unbound_vars_in_kernel_expr(index, globals, locals, bound, out);
        }
        KernelExpr::Match {
            scrutinee, arms, ..
        } => {
            collect_unbound_vars_in_kernel_expr(scrutinee, globals, locals, bound, out);
            for arm in arms {
                let mut binders = Vec::new();
                collect_kernel_pattern_binders(&arm.pattern, &mut binders);
                bound.extend(binders.iter().cloned());
                if let Some(guard) = &arm.guard {
                    collect_unbound_vars_in_kernel_expr(guard, globals, locals, bound, out);
                }
                collect_unbound_vars_in_kernel_expr(&arm.body, globals, locals, bound, out);
                for _ in 0..binders.len() {
                    bound.pop();
                }
            }
        }
        KernelExpr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            collect_unbound_vars_in_kernel_expr(cond, globals, locals, bound, out);
            collect_unbound_vars_in_kernel_expr(then_branch, globals, locals, bound, out);
            collect_unbound_vars_in_kernel_expr(else_branch, globals, locals, bound, out);
        }
        KernelExpr::Binary { left, right, .. } => {
            collect_unbound_vars_in_kernel_expr(left, globals, locals, bound, out);
            collect_unbound_vars_in_kernel_expr(right, globals, locals, bound, out);
        }
        KernelExpr::Block { items, .. } => {
            let before = bound.len();
            for item in items {
                match item {
                    crate::kernel::KernelBlockItem::Bind { pattern, expr } => {
                        collect_unbound_vars_in_kernel_expr(expr, globals, locals, bound, out);
                        let mut binders = Vec::new();
                        collect_kernel_pattern_binders(pattern, &mut binders);
                        bound.extend(binders);
                    }
                    crate::kernel::KernelBlockItem::Filter { expr }
                    | crate::kernel::KernelBlockItem::Yield { expr }
                    | crate::kernel::KernelBlockItem::Recurse { expr }
                    | crate::kernel::KernelBlockItem::Expr { expr } => {
                        collect_unbound_vars_in_kernel_expr(expr, globals, locals, bound, out);
                    }
                }
            }
            bound.truncate(before);
        }
    }
}

fn collect_kernel_pattern_binders(pat: &crate::kernel::KernelPattern, out: &mut Vec<String>) {
    match pat {
        crate::kernel::KernelPattern::Wildcard { .. } => {}
        crate::kernel::KernelPattern::Var { name, .. } => out.push(name.clone()),
        crate::kernel::KernelPattern::Literal { .. } => {}
        crate::kernel::KernelPattern::At { name, pattern, .. } => {
            out.push(name.clone());
            collect_kernel_pattern_binders(pattern, out);
        }
        crate::kernel::KernelPattern::Constructor { args, .. } => {
            for arg in args {
                collect_kernel_pattern_binders(arg, out);
            }
        }
        crate::kernel::KernelPattern::Tuple { items, .. } => {
            for item in items {
                collect_kernel_pattern_binders(item, out);
            }
        }
        crate::kernel::KernelPattern::List { items, rest, .. } => {
            for item in items {
                collect_kernel_pattern_binders(item, out);
            }
            if let Some(rest) = rest.as_deref() {
                collect_kernel_pattern_binders(rest, out);
            }
        }
        crate::kernel::KernelPattern::Record { fields, .. } => {
            for field in fields {
                collect_kernel_pattern_binders(&field.pattern, out);
            }
        }
    }
}

fn is_reserved_selector_name(name: &str) -> bool {
    matches!(name, "key" | "value")
}

fn rewrite_implicit_field_vars(
    expr: KernelExpr,
    implicit_param: &str,
    unbound: &HashSet<String>,
) -> KernelExpr {
    match expr {
        KernelExpr::Var { id, name } if unbound.contains(&name) => KernelExpr::FieldAccess {
            id,
            base: Box::new(KernelExpr::Var {
                id,
                name: implicit_param.to_string(),
            }),
            field: name,
        },
        KernelExpr::Lambda { id, param, body } => KernelExpr::Lambda {
            id,
            param: param.clone(),
            body: {
                if unbound.contains(&param) {
                    let mut unbound2 = unbound.clone();
                    unbound2.remove(&param);
                    Box::new(rewrite_implicit_field_vars(
                        *body,
                        implicit_param,
                        &unbound2,
                    ))
                } else {
                    Box::new(rewrite_implicit_field_vars(*body, implicit_param, unbound))
                }
            },
        },
        KernelExpr::App { id, func, arg } => KernelExpr::App {
            id,
            func: Box::new(rewrite_implicit_field_vars(*func, implicit_param, unbound)),
            arg: Box::new(rewrite_implicit_field_vars(*arg, implicit_param, unbound)),
        },
        KernelExpr::Call { id, func, args } => KernelExpr::Call {
            id,
            func: Box::new(rewrite_implicit_field_vars(*func, implicit_param, unbound)),
            args: args
                .into_iter()
                .map(|a| rewrite_implicit_field_vars(a, implicit_param, unbound))
                .collect(),
        },
        KernelExpr::DebugFn {
            id,
            fn_name,
            arg_vars,
            log_args,
            log_return,
            log_time,
            body,
        } => KernelExpr::DebugFn {
            id,
            fn_name,
            arg_vars,
            log_args,
            log_return,
            log_time,
            body: Box::new(rewrite_implicit_field_vars(*body, implicit_param, unbound)),
        },
        KernelExpr::Pipe {
            id,
            pipe_id,
            step,
            label,
            log_time,
            func,
            arg,
        } => KernelExpr::Pipe {
            id,
            pipe_id,
            step,
            label,
            log_time,
            func: Box::new(rewrite_implicit_field_vars(*func, implicit_param, unbound)),
            arg: Box::new(rewrite_implicit_field_vars(*arg, implicit_param, unbound)),
        },
        KernelExpr::List { id, items } => KernelExpr::List {
            id,
            items: items
                .into_iter()
                .map(|item| crate::kernel::KernelListItem {
                    expr: rewrite_implicit_field_vars(item.expr, implicit_param, unbound),
                    spread: item.spread,
                })
                .collect(),
        },
        KernelExpr::Tuple { id, items } => KernelExpr::Tuple {
            id,
            items: items
                .into_iter()
                .map(|e| rewrite_implicit_field_vars(e, implicit_param, unbound))
                .collect(),
        },
        KernelExpr::Record { id, fields } => KernelExpr::Record {
            id,
            fields: fields
                .into_iter()
                .map(|f| crate::kernel::KernelRecordField {
                    spread: f.spread,
                    path: f
                        .path
                        .into_iter()
                        .map(|seg| match seg {
                            crate::kernel::KernelPathSegment::Field(name) => {
                                crate::kernel::KernelPathSegment::Field(name)
                            }
                            crate::kernel::KernelPathSegment::All => {
                                crate::kernel::KernelPathSegment::All
                            }
                            crate::kernel::KernelPathSegment::Index(expr) => {
                                crate::kernel::KernelPathSegment::Index(
                                    rewrite_implicit_field_vars(expr, implicit_param, unbound),
                                )
                            }
                        })
                        .collect(),
                    value: rewrite_implicit_field_vars(f.value, implicit_param, unbound),
                })
                .collect(),
        },
        KernelExpr::Patch { id, target, fields } => KernelExpr::Patch {
            id,
            target: Box::new(rewrite_implicit_field_vars(
                *target,
                implicit_param,
                unbound,
            )),
            fields: fields
                .into_iter()
                .map(|f| crate::kernel::KernelRecordField {
                    spread: f.spread,
                    path: f
                        .path
                        .into_iter()
                        .map(|seg| match seg {
                            crate::kernel::KernelPathSegment::Field(name) => {
                                crate::kernel::KernelPathSegment::Field(name)
                            }
                            crate::kernel::KernelPathSegment::All => {
                                crate::kernel::KernelPathSegment::All
                            }
                            crate::kernel::KernelPathSegment::Index(expr) => {
                                crate::kernel::KernelPathSegment::Index(
                                    rewrite_implicit_field_vars(expr, implicit_param, unbound),
                                )
                            }
                        })
                        .collect(),
                    value: rewrite_implicit_field_vars(f.value, implicit_param, unbound),
                })
                .collect(),
        },
        KernelExpr::FieldAccess { id, base, field } => KernelExpr::FieldAccess {
            id,
            base: Box::new(rewrite_implicit_field_vars(*base, implicit_param, unbound)),
            field,
        },
        KernelExpr::Index { id, base, index } => KernelExpr::Index {
            id,
            base: Box::new(rewrite_implicit_field_vars(*base, implicit_param, unbound)),
            index: Box::new(rewrite_implicit_field_vars(*index, implicit_param, unbound)),
        },
        KernelExpr::Match {
            id,
            scrutinee,
            arms,
        } => KernelExpr::Match {
            id,
            scrutinee: Box::new(rewrite_implicit_field_vars(
                *scrutinee,
                implicit_param,
                unbound,
            )),
            arms: arms
                .into_iter()
                .map(|arm| crate::kernel::KernelMatchArm {
                    pattern: arm.pattern,
                    guard: arm
                        .guard
                        .map(|g| rewrite_implicit_field_vars(g, implicit_param, unbound)),
                    body: rewrite_implicit_field_vars(arm.body, implicit_param, unbound),
                })
                .collect(),
        },
        KernelExpr::If {
            id,
            cond,
            then_branch,
            else_branch,
        } => KernelExpr::If {
            id,
            cond: Box::new(rewrite_implicit_field_vars(*cond, implicit_param, unbound)),
            then_branch: Box::new(rewrite_implicit_field_vars(
                *then_branch,
                implicit_param,
                unbound,
            )),
            else_branch: Box::new(rewrite_implicit_field_vars(
                *else_branch,
                implicit_param,
                unbound,
            )),
        },
        KernelExpr::Binary {
            id,
            op,
            left,
            right,
        } => KernelExpr::Binary {
            id,
            op,
            left: Box::new(rewrite_implicit_field_vars(*left, implicit_param, unbound)),
            right: Box::new(rewrite_implicit_field_vars(*right, implicit_param, unbound)),
        },
        KernelExpr::Block {
            id,
            block_kind,
            items,
        } => KernelExpr::Block {
            id,
            block_kind,
            items: items
                .into_iter()
                .map(|item| match item {
                    crate::kernel::KernelBlockItem::Bind { pattern, expr } => {
                        crate::kernel::KernelBlockItem::Bind {
                            pattern,
                            expr: rewrite_implicit_field_vars(expr, implicit_param, unbound),
                        }
                    }
                    crate::kernel::KernelBlockItem::Filter { expr } => {
                        crate::kernel::KernelBlockItem::Filter {
                            expr: rewrite_implicit_field_vars(expr, implicit_param, unbound),
                        }
                    }
                    crate::kernel::KernelBlockItem::Yield { expr } => {
                        crate::kernel::KernelBlockItem::Yield {
                            expr: rewrite_implicit_field_vars(expr, implicit_param, unbound),
                        }
                    }
                    crate::kernel::KernelBlockItem::Recurse { expr } => {
                        crate::kernel::KernelBlockItem::Recurse {
                            expr: rewrite_implicit_field_vars(expr, implicit_param, unbound),
                        }
                    }
                    crate::kernel::KernelBlockItem::Expr { expr } => {
                        crate::kernel::KernelBlockItem::Expr {
                            expr: rewrite_implicit_field_vars(expr, implicit_param, unbound),
                        }
                    }
                })
                .collect(),
        },
        other => other,
    }
}

fn lower_block_kind(kind: KernelBlockKind) -> Result<RustIrBlockKind, AiviError> {
    match kind {
        KernelBlockKind::Plain => Ok(RustIrBlockKind::Plain),
        KernelBlockKind::Do { monad } => Ok(RustIrBlockKind::Do { monad }),
        KernelBlockKind::Generate => Ok(RustIrBlockKind::Generate),
        KernelBlockKind::Resource => Ok(RustIrBlockKind::Resource),
    }
}

fn lower_block_item(
    item: KernelBlockItem,
    globals: &[String],
    locals: &mut Vec<String>,
) -> Result<RustIrBlockItem, AiviError> {
    match item {
        KernelBlockItem::Bind { pattern, expr } => {
            let pat = lower_pattern(pattern)?;
            // Compiler-generated let bindings (e.g. __loop from loop/recurse
            // desugaring) may be self-referential: the lambda body references
            // the binding name recursively. Pre-add such binders to locals so
            // lower_expr can resolve the recursive reference.
            let mut binders = Vec::new();
            collect_rust_ir_pattern_binders(&pat, &mut binders);
            let pre_added: Vec<String> = binders
                .iter()
                .filter(|n| n.starts_with("__"))
                .cloned()
                .collect();
            for name in &pre_added {
                locals.push(name.clone());
            }
            let expr = lower_expr(expr, globals, locals)?;
            // Add any remaining (non-pre-added) binders for subsequent items.
            for name in binders {
                if !pre_added.contains(&name) {
                    locals.push(name);
                }
            }
            Ok(RustIrBlockItem::Bind { pattern: pat, expr })
        }
        KernelBlockItem::Filter { expr } => Ok(RustIrBlockItem::Filter {
            expr: lower_expr(expr, globals, locals)?,
        }),
        KernelBlockItem::Yield { expr } => Ok(RustIrBlockItem::Yield {
            expr: lower_expr(expr, globals, locals)?,
        }),
        KernelBlockItem::Recurse { expr } => Ok(RustIrBlockItem::Recurse {
            expr: lower_expr(expr, globals, locals)?,
        }),
        KernelBlockItem::Expr { expr } => Ok(RustIrBlockItem::Expr {
            expr: lower_expr(expr, globals, locals)?,
        }),
    }
}

fn collect_rust_ir_pattern_binders(pattern: &RustIrPattern, out: &mut Vec<String>) {
    match pattern {
        RustIrPattern::Wildcard { .. } => {}
        RustIrPattern::Var { name, .. } => out.push(name.clone()),
        RustIrPattern::Literal { .. } => {}
        RustIrPattern::At { name, pattern, .. } => {
            out.push(name.clone());
            collect_rust_ir_pattern_binders(pattern, out);
        }
        RustIrPattern::Constructor { args, .. } => {
            for arg in args {
                collect_rust_ir_pattern_binders(arg, out);
            }
        }
        RustIrPattern::Tuple { items, .. } => {
            for item in items {
                collect_rust_ir_pattern_binders(item, out);
            }
        }
        RustIrPattern::List { items, rest, .. } => {
            for item in items {
                collect_rust_ir_pattern_binders(item, out);
            }
            if let Some(rest) = rest.as_deref() {
                collect_rust_ir_pattern_binders(rest, out);
            }
        }
        RustIrPattern::Record { fields, .. } => {
            for field in fields {
                collect_rust_ir_pattern_binders(&field.pattern, out);
            }
        }
    }
}

fn lower_pattern(pattern: KernelPattern) -> Result<RustIrPattern, AiviError> {
    match pattern {
        KernelPattern::Wildcard { id } => Ok(RustIrPattern::Wildcard { id }),
        KernelPattern::Var { id, name } => Ok(RustIrPattern::Var { id, name }),
        KernelPattern::At { id, name, pattern } => Ok(RustIrPattern::At {
            id,
            name,
            pattern: Box::new(lower_pattern(*pattern)?),
        }),
        KernelPattern::Literal { id, value } => Ok(RustIrPattern::Literal {
            id,
            value: lower_literal(value),
        }),
        KernelPattern::Constructor { id, name, args } => Ok(RustIrPattern::Constructor {
            id,
            name,
            args: args
                .into_iter()
                .map(lower_pattern)
                .collect::<Result<Vec<_>, _>>()?,
        }),
        KernelPattern::Tuple { id, items } => Ok(RustIrPattern::Tuple {
            id,
            items: items
                .into_iter()
                .map(lower_pattern)
                .collect::<Result<Vec<_>, _>>()?,
        }),
        KernelPattern::List { id, items, rest } => Ok(RustIrPattern::List {
            id,
            items: items
                .into_iter()
                .map(lower_pattern)
                .collect::<Result<Vec<_>, _>>()?,
            rest: rest.map(|p| lower_pattern(*p).map(Box::new)).transpose()?,
        }),
        KernelPattern::Record { id, fields } => Ok(RustIrPattern::Record {
            id,
            fields: fields
                .into_iter()
                .map(|f| {
                    Ok::<RustIrRecordPatternField, AiviError>(RustIrRecordPatternField {
                        path: f.path,
                        pattern: lower_pattern(f.pattern)?,
                    })
                })
                .collect::<Result<Vec<_>, _>>()?,
        }),
    }
}

fn lower_match_arm(
    arm: KernelMatchArm,
    globals: &[String],
    locals: &mut Vec<String>,
) -> Result<RustIrMatchArm, AiviError> {
    // Pattern bindings are introduced as locals for the arm's guard/body.
    // We conservatively extend `locals` while lowering the guard/body.
    let before = locals.len();
    let mut binders = Vec::new();
    collect_pattern_binders(&arm.pattern, &mut binders);
    for name in binders {
        locals.push(name);
    }
    let guard = arm
        .guard
        .map(|g| lower_expr(g, globals, locals))
        .transpose()?;
    let body = lower_expr(arm.body, globals, locals)?;
    locals.truncate(before);
    Ok(RustIrMatchArm {
        pattern: lower_pattern(arm.pattern)?,
        guard,
        body,
    })
}

fn lower_literal(lit: crate::kernel::KernelLiteral) -> RustIrLiteral {
    match lit {
        crate::kernel::KernelLiteral::Number(text) => RustIrLiteral::Number(text),
        crate::kernel::KernelLiteral::String(text) => RustIrLiteral::String(text),
        crate::kernel::KernelLiteral::Sigil { tag, body, flags } => {
            RustIrLiteral::Sigil { tag, body, flags }
        }
        crate::kernel::KernelLiteral::Bool(value) => RustIrLiteral::Bool(value),
        crate::kernel::KernelLiteral::DateTime(text) => RustIrLiteral::DateTime(text),
    }
}

fn collect_pattern_binders(pattern: &KernelPattern, out: &mut Vec<String>) {
    match pattern {
        KernelPattern::Wildcard { .. } => {}
        KernelPattern::Var { name, .. } => out.push(name.clone()),
        KernelPattern::Literal { .. } => {}
        KernelPattern::At { name, pattern, .. } => {
            out.push(name.clone());
            collect_pattern_binders(pattern, out);
        }
        KernelPattern::Constructor { args, .. } => {
            for arg in args {
                collect_pattern_binders(arg, out);
            }
        }
        KernelPattern::Tuple { items, .. } => {
            for item in items {
                collect_pattern_binders(item, out);
            }
        }
        KernelPattern::List { items, rest, .. } => {
            for item in items {
                collect_pattern_binders(item, out);
            }
            if let Some(rest) = rest.as_deref() {
                collect_pattern_binders(rest, out);
            }
        }
        KernelPattern::Record { fields, .. } => {
            for field in fields {
                collect_pattern_binders(&field.pattern, out);
            }
        }
    }
}

fn is_constructor_name(name: &str) -> bool {
    let seg = name.rsplit('.').next().unwrap_or(name);
    seg.chars()
        .next()
        .map(|ch| ch.is_ascii_uppercase())
        .unwrap_or(false)
}

fn resolve_builtin(name: &str) -> Option<BuiltinName> {
    // Keep this list in sync with `aivi_native_runtime::builtins`.
    let ok = matches!(
        name,
        "Unit"
            | "True"
            | "False"
            | "None"
            | "Some"
            | "Ok"
            | "Err"
            | "Closed"
            | "foldGen"
            | "pure"
            | "fail"
            | "attempt"
            | "load"
            | "bind"
            | "print"
            | "println"
            | "map"
            | "chain"
            | "assertEq"
            | "file"
            | "env"
            | "system"
            | "clock"
            | "random"
            | "channel"
            | "concurrent"
            | "httpServer"
            | "ui"
            | "gtk4"
            | "text"
            | "regex"
            | "math"
            | "calendar"
            | "color"
            | "linalg"
            | "signal"
            | "graph"
            | "bigint"
            | "rational"
            | "decimal"
            | "url"
            | "http"
            | "https"
            | "sockets"
            | "streams"
            | "instant"
            | "List"
            | "collections"
            | "console"
            | "crypto"
            | "logger"
            | "database"
            | "i18n"
            | "Map"
            | "Set"
            | "Queue"
            | "Deque"
            | "Heap"
    );
    if ok {
        Some(name.to_string())
    } else {
        None
    }
}
