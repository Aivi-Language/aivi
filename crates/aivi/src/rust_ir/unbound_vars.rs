fn collect_unbound_vars_in_hir_expr(
    expr: &HirExpr,
    globals: &[String],
    locals: &[String],
    bound: &mut Vec<String>,
    out: &mut HashSet<String>,
) {
    match expr {
        HirExpr::Var { name, .. } => {
            let reserved = is_reserved_selector_name(name);
            let is_bound = bound.iter().rev().any(|b| b == name)
                || locals.iter().rev().any(|b| b == name)
                || (!reserved && globals.iter().any(|g| g == name))
                || (!reserved && resolve_builtin(name).is_some());
            if !is_bound {
                out.insert(name.clone());
            }
        }
        HirExpr::LitNumber { .. }
        | HirExpr::LitString { .. }
        | HirExpr::LitSigil { .. }
        | HirExpr::LitBool { .. }
        | HirExpr::LitDateTime { .. }
        | HirExpr::Raw { .. } => {}
        HirExpr::TextInterpolate { parts, .. } => {
            for part in parts {
                if let HirTextPart::Expr { expr } = part {
                    collect_unbound_vars_in_hir_expr(expr, globals, locals, bound, out);
                }
            }
        }
        HirExpr::Lambda { param, body, .. } => {
            bound.push(param.clone());
            collect_unbound_vars_in_hir_expr(body, globals, locals, bound, out);
            bound.pop();
        }
        HirExpr::App { func, arg, .. } => {
            collect_unbound_vars_in_hir_expr(func, globals, locals, bound, out);
            collect_unbound_vars_in_hir_expr(arg, globals, locals, bound, out);
        }
        HirExpr::Call { func, args, .. } => {
            collect_unbound_vars_in_hir_expr(func, globals, locals, bound, out);
            for arg in args {
                collect_unbound_vars_in_hir_expr(arg, globals, locals, bound, out);
            }
        }
        HirExpr::DebugFn { body, .. } => {
            collect_unbound_vars_in_hir_expr(body, globals, locals, bound, out);
        }
        HirExpr::Pipe { func, arg, .. } => {
            collect_unbound_vars_in_hir_expr(func, globals, locals, bound, out);
            collect_unbound_vars_in_hir_expr(arg, globals, locals, bound, out);
        }
        HirExpr::List { items, .. } => {
            for item in items {
                collect_unbound_vars_in_hir_expr(&item.expr, globals, locals, bound, out);
            }
        }
        HirExpr::Tuple { items, .. } => {
            for item in items {
                collect_unbound_vars_in_hir_expr(item, globals, locals, bound, out);
            }
        }
        HirExpr::Record { fields, .. } | HirExpr::Patch { fields, .. } => {
            for field in fields {
                for seg in &field.path {
                    if let HirPathSegment::Index(expr) = seg {
                        collect_unbound_vars_in_hir_expr(expr, globals, locals, bound, out);
                    }
                }
                collect_unbound_vars_in_hir_expr(&field.value, globals, locals, bound, out);
            }
            if let HirExpr::Patch { target, .. } = expr {
                collect_unbound_vars_in_hir_expr(target, globals, locals, bound, out);
            }
        }
        HirExpr::FieldAccess { base, .. } => {
            collect_unbound_vars_in_hir_expr(base, globals, locals, bound, out);
        }
        HirExpr::Index { base, index, .. } => {
            collect_unbound_vars_in_hir_expr(base, globals, locals, bound, out);
            collect_unbound_vars_in_hir_expr(index, globals, locals, bound, out);
        }
        HirExpr::Match {
            scrutinee, arms, ..
        } => {
            collect_unbound_vars_in_hir_expr(scrutinee, globals, locals, bound, out);
            for arm in arms {
                let mut binders = Vec::new();
                collect_hir_pattern_binders(&arm.pattern, &mut binders);
                bound.extend(binders.iter().cloned());
                if let Some(guard) = &arm.guard {
                    collect_unbound_vars_in_hir_expr(guard, globals, locals, bound, out);
                }
                collect_unbound_vars_in_hir_expr(&arm.body, globals, locals, bound, out);
                for _ in 0..binders.len() {
                    bound.pop();
                }
            }
        }
        HirExpr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            collect_unbound_vars_in_hir_expr(cond, globals, locals, bound, out);
            collect_unbound_vars_in_hir_expr(then_branch, globals, locals, bound, out);
            collect_unbound_vars_in_hir_expr(else_branch, globals, locals, bound, out);
        }
        HirExpr::Binary { left, right, .. } => {
            collect_unbound_vars_in_hir_expr(left, globals, locals, bound, out);
            collect_unbound_vars_in_hir_expr(right, globals, locals, bound, out);
        }

        HirExpr::Mock { substitutions, body, .. } => {
            for sub in substitutions {
                if let Some(value) = &sub.value {
                    collect_unbound_vars_in_hir_expr(value, globals, locals, bound, out);
                }
            }
            collect_unbound_vars_in_hir_expr(body, globals, locals, bound, out);
        }
        HirExpr::Block { .. } => unreachable!("Block should be desugared before RustIr lowering"),
    }
}

fn collect_hir_pattern_binders(pat: &HirPattern, out: &mut Vec<String>) {
    match pat {
        HirPattern::Wildcard { .. } => {}
        HirPattern::Var { name, .. } => out.push(name.clone()),
        HirPattern::Literal { .. } => {}
        HirPattern::At { name, pattern, .. } => {
            out.push(name.clone());
            collect_hir_pattern_binders(pattern, out);
        }
        HirPattern::Constructor { args, .. } => {
            for arg in args {
                collect_hir_pattern_binders(arg, out);
            }
        }
        HirPattern::Tuple { items, .. } => {
            for item in items {
                collect_hir_pattern_binders(item, out);
            }
        }
        HirPattern::List { items, rest, .. } => {
            for item in items {
                collect_hir_pattern_binders(item, out);
            }
            if let Some(rest) = rest.as_deref() {
                collect_hir_pattern_binders(rest, out);
            }
        }
        HirPattern::Record { fields, .. } => {
            for field in fields {
                collect_hir_pattern_binders(&field.pattern, out);
            }
        }
    }
}

fn is_reserved_selector_name(name: &str) -> bool {
    matches!(name, "key" | "value")
}

fn rewrite_implicit_field_vars(
    expr: HirExpr,
    implicit_param: &str,
    unbound: &HashSet<String>,
) -> HirExpr {
    match expr {
        HirExpr::Var { id, name } if unbound.contains(&name) => HirExpr::FieldAccess {
            id,
            base: Box::new(HirExpr::Var {
                id,
                name: implicit_param.to_string(),
            }),
            field: name,
        },
        HirExpr::Lambda { id, param, body } => HirExpr::Lambda {
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
        HirExpr::App { id, func, arg } => HirExpr::App {
            id,
            func: Box::new(rewrite_implicit_field_vars(*func, implicit_param, unbound)),
            arg: Box::new(rewrite_implicit_field_vars(*arg, implicit_param, unbound)),
        },
        HirExpr::Call { id, func, args } => HirExpr::Call {
            id,
            func: Box::new(rewrite_implicit_field_vars(*func, implicit_param, unbound)),
            args: args
                .into_iter()
                .map(|a| rewrite_implicit_field_vars(a, implicit_param, unbound))
                .collect(),
        },
        HirExpr::DebugFn {
            id,
            fn_name,
            arg_vars,
            log_args,
            log_return,
            log_time,
            body,
        } => HirExpr::DebugFn {
            id,
            fn_name,
            arg_vars,
            log_args,
            log_return,
            log_time,
            body: Box::new(rewrite_implicit_field_vars(*body, implicit_param, unbound)),
        },
        HirExpr::Pipe {
            id,
            pipe_id,
            step,
            label,
            log_time,
            func,
            arg,
        } => HirExpr::Pipe {
            id,
            pipe_id,
            step,
            label,
            log_time,
            func: Box::new(rewrite_implicit_field_vars(*func, implicit_param, unbound)),
            arg: Box::new(rewrite_implicit_field_vars(*arg, implicit_param, unbound)),
        },
        HirExpr::List { id, items } => HirExpr::List {
            id,
            items: items
                .into_iter()
                .map(|item| HirListItem {
                    expr: rewrite_implicit_field_vars(item.expr, implicit_param, unbound),
                    spread: item.spread,
                })
                .collect(),
        },
        HirExpr::Tuple { id, items } => HirExpr::Tuple {
            id,
            items: items
                .into_iter()
                .map(|e| rewrite_implicit_field_vars(e, implicit_param, unbound))
                .collect(),
        },
        HirExpr::Record { id, fields } => HirExpr::Record {
            id,
            fields: fields
                .into_iter()
                .map(|f| HirRecordField {
                    spread: f.spread,
                    path: f
                        .path
                        .into_iter()
                        .map(|seg| match seg {
                            HirPathSegment::Field(name) => {
                                HirPathSegment::Field(name)
                            }
                            HirPathSegment::All => {
                                HirPathSegment::All
                            }
                            HirPathSegment::Index(expr) => {
                                HirPathSegment::Index(
                                    rewrite_implicit_field_vars(expr, implicit_param, unbound),
                                )
                            }
                        })
                        .collect(),
                    value: rewrite_implicit_field_vars(f.value, implicit_param, unbound),
                })
                .collect(),
        },
        HirExpr::Patch { id, target, fields } => HirExpr::Patch {
            id,
            target: Box::new(rewrite_implicit_field_vars(
                *target,
                implicit_param,
                unbound,
            )),
            fields: fields
                .into_iter()
                .map(|f| HirRecordField {
                    spread: f.spread,
                    path: f
                        .path
                        .into_iter()
                        .map(|seg| match seg {
                            HirPathSegment::Field(name) => {
                                HirPathSegment::Field(name)
                            }
                            HirPathSegment::All => {
                                HirPathSegment::All
                            }
                            HirPathSegment::Index(expr) => {
                                HirPathSegment::Index(
                                    rewrite_implicit_field_vars(expr, implicit_param, unbound),
                                )
                            }
                        })
                        .collect(),
                    value: rewrite_implicit_field_vars(f.value, implicit_param, unbound),
                })
                .collect(),
        },
        HirExpr::FieldAccess { id, base, field } => HirExpr::FieldAccess {
            id,
            base: Box::new(rewrite_implicit_field_vars(*base, implicit_param, unbound)),
            field,
        },
        HirExpr::Index { id, base, index, location } => HirExpr::Index {
            id,
            base: Box::new(rewrite_implicit_field_vars(*base, implicit_param, unbound)),
            index: Box::new(rewrite_implicit_field_vars(*index, implicit_param, unbound)),
            location,
        },
        HirExpr::Match {
            id,
            scrutinee,
            arms,
        } => HirExpr::Match {
            id,
            scrutinee: Box::new(rewrite_implicit_field_vars(
                *scrutinee,
                implicit_param,
                unbound,
            )),
            arms: arms
                .into_iter()
                .map(|arm| HirMatchArm {
                    pattern: arm.pattern,
                    guard: arm
                        .guard
                        .map(|g| rewrite_implicit_field_vars(g, implicit_param, unbound)),
                    body: rewrite_implicit_field_vars(arm.body, implicit_param, unbound),
                })
                .collect(),
        },
        HirExpr::If {
            id,
            cond,
            then_branch,
            else_branch,
        } => HirExpr::If {
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
        HirExpr::Binary {
            id,
            op,
            left,
            right,
        } => HirExpr::Binary {
            id,
            op,
            left: Box::new(rewrite_implicit_field_vars(*left, implicit_param, unbound)),
            right: Box::new(rewrite_implicit_field_vars(*right, implicit_param, unbound)),
        },
        other => other,
    }
}

fn lower_pattern(pattern: HirPattern) -> Result<RustIrPattern, AiviError> {
    match pattern {
        HirPattern::Wildcard { id } => Ok(RustIrPattern::Wildcard { id }),
        HirPattern::Var { id, name } => Ok(RustIrPattern::Var { id, name }),
        HirPattern::At { id, name, pattern } => Ok(RustIrPattern::At {
            id,
            name,
            pattern: Box::new(lower_pattern(*pattern)?),
        }),
        HirPattern::Literal { id, value } => Ok(RustIrPattern::Literal {
            id,
            value: lower_literal(value),
        }),
        HirPattern::Constructor { id, name, args } => Ok(RustIrPattern::Constructor {
            id,
            name,
            args: args
                .into_iter()
                .map(lower_pattern)
                .collect::<Result<Vec<_>, _>>()?,
        }),
        HirPattern::Tuple { id, items } => Ok(RustIrPattern::Tuple {
            id,
            items: items
                .into_iter()
                .map(lower_pattern)
                .collect::<Result<Vec<_>, _>>()?,
        }),
        HirPattern::List { id, items, rest } => Ok(RustIrPattern::List {
            id,
            items: items
                .into_iter()
                .map(lower_pattern)
                .collect::<Result<Vec<_>, _>>()?,
            rest: rest.map(|p| lower_pattern(*p).map(Box::new)).transpose()?,
        }),
        HirPattern::Record { id, fields } => Ok(RustIrPattern::Record {
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
    arm: HirMatchArm,
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

fn lower_literal(lit: HirLiteral) -> RustIrLiteral {
    match lit {
        HirLiteral::Number(text) => RustIrLiteral::Number(text),
        HirLiteral::String(text) => RustIrLiteral::String(text),
        HirLiteral::Sigil { tag, body, flags } => {
            RustIrLiteral::Sigil { tag, body, flags }
        }
        HirLiteral::Bool(value) => RustIrLiteral::Bool(value),
        HirLiteral::DateTime(text) => RustIrLiteral::DateTime(text),
    }
}

fn collect_pattern_binders(pattern: &HirPattern, out: &mut Vec<String>) {
    match pattern {
        HirPattern::Wildcard { .. } => {}
        HirPattern::Var { name, .. } => out.push(name.clone()),
        HirPattern::Literal { .. } => {}
        HirPattern::At { name, pattern, .. } => {
            out.push(name.clone());
            collect_pattern_binders(pattern, out);
        }
        HirPattern::Constructor { args, .. } => {
            for arg in args {
                collect_pattern_binders(arg, out);
            }
        }
        HirPattern::Tuple { items, .. } => {
            for item in items {
                collect_pattern_binders(item, out);
            }
        }
        HirPattern::List { items, rest, .. } => {
            for item in items {
                collect_pattern_binders(item, out);
            }
            if let Some(rest) = rest.as_deref() {
                collect_pattern_binders(rest, out);
            }
        }
        HirPattern::Record { fields, .. } => {
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
    crate::builtin_names::resolve_builtin(name)
}
