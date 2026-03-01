
// genBind(g, f)
// \k -> \z -> g (\acc -> \x -> f(x) k acc) z
fn gen_bind(g: KernelExpr, f: KernelExpr, id_gen: &mut IdGen) -> KernelExpr {
    let k_name = format!("_k_{}", id_gen.next());
    let z_name = format!("_z_{}", id_gen.next());
    let k = KernelExpr::Var {
        id: id_gen.next(),
        name: k_name.clone(),
    };
    let z = KernelExpr::Var {
        id: id_gen.next(),
        name: z_name.clone(),
    };

    // \acc -> \x -> ...
    let acc_name = format!("_acc_{}", id_gen.next());
    let x_name = format!("_x_{}", id_gen.next());
    let acc = KernelExpr::Var {
        id: id_gen.next(),
        name: acc_name.clone(),
    };
    let x = KernelExpr::Var {
        id: id_gen.next(),
        name: x_name.clone(),
    };

    // f(x)
    let fx = KernelExpr::App {
        id: id_gen.next(),
        func: Box::new(f),
        arg: Box::new(x),
    };
    // f(x) k
    let fx_k = KernelExpr::App {
        id: id_gen.next(),
        func: Box::new(fx),
        arg: Box::new(k.clone()),
    };
    // f(x) k acc
    let fx_k_acc = KernelExpr::App {
        id: id_gen.next(),
        func: Box::new(fx_k),
        arg: Box::new(acc),
    };

    let step_fn = KernelExpr::Lambda {
        id: id_gen.next(),
        param: acc_name,
        body: Box::new(KernelExpr::Lambda {
            id: id_gen.next(),
            param: x_name,
            body: Box::new(fx_k_acc),
        }),
    };

    // g step_fn z
    let g_step = KernelExpr::App {
        id: id_gen.next(),
        func: Box::new(g),
        arg: Box::new(step_fn),
    };
    let g_step_z = KernelExpr::App {
        id: id_gen.next(),
        func: Box::new(g_step),
        arg: Box::new(z),
    };

    KernelExpr::Lambda {
        id: id_gen.next(),
        param: k_name,
        body: Box::new(KernelExpr::Lambda {
            id: id_gen.next(),
            param: z_name,
            body: Box::new(g_step_z),
        }),
    }
}

fn lower_list_item(item: HirListItem, id_gen: &mut IdGen) -> KernelListItem {
    KernelListItem {
        expr: lower_expr(item.expr, id_gen),
        spread: item.spread,
    }
}

fn lower_record_field(field: HirRecordField, id_gen: &mut IdGen) -> KernelRecordField {
    KernelRecordField {
        spread: field.spread,
        path: field
            .path
            .into_iter()
            .map(|s| lower_path_segment(s, id_gen))
            .collect(),
        value: lower_expr(field.value, id_gen),
    }
}

fn lower_path_segment(seg: HirPathSegment, id_gen: &mut IdGen) -> KernelPathSegment {
    match seg {
        HirPathSegment::Field(name) => KernelPathSegment::Field(name),
        HirPathSegment::Index(expr) => KernelPathSegment::Index(lower_expr(expr, id_gen)),
        HirPathSegment::All => KernelPathSegment::All,
    }
}

fn lower_match_arm(arm: HirMatchArm, id_gen: &mut IdGen) -> KernelMatchArm {
    KernelMatchArm {
        pattern: lower_pattern(arm.pattern, id_gen),
        guard: arm.guard.map(|e| lower_expr(e, id_gen)),
        body: lower_expr(arm.body, id_gen),
    }
}

fn lower_pattern(pattern: HirPattern, id_gen: &mut IdGen) -> KernelPattern {
    match pattern {
        HirPattern::Wildcard { id } => KernelPattern::Wildcard { id },
        HirPattern::Var { id, name } => KernelPattern::Var { id, name },
        HirPattern::At { id, name, pattern } => KernelPattern::At {
            id,
            name,
            pattern: Box::new(lower_pattern(*pattern, id_gen)),
        },
        HirPattern::Literal { id, value } => KernelPattern::Literal {
            id,
            value: lower_literal(value),
        },
        HirPattern::Constructor { id, name, args } => KernelPattern::Constructor {
            id,
            name,
            args: args.into_iter().map(|p| lower_pattern(p, id_gen)).collect(),
        },
        HirPattern::Tuple { id, items } => KernelPattern::Tuple {
            id,
            items: items
                .into_iter()
                .map(|p| lower_pattern(p, id_gen))
                .collect(),
        },
        HirPattern::List { id, items, rest } => KernelPattern::List {
            id,
            items: items
                .into_iter()
                .map(|p| lower_pattern(p, id_gen))
                .collect(),
            rest: rest.map(|p| Box::new(lower_pattern(*p, id_gen))),
        },
        HirPattern::Record { id, fields } => KernelPattern::Record {
            id,
            fields: fields
                .into_iter()
                .map(|f| lower_record_pattern_field(f, id_gen))
                .collect(),
        },
    }
}

fn lower_record_pattern_field(
    field: HirRecordPatternField,
    id_gen: &mut IdGen,
) -> KernelRecordPatternField {
    KernelRecordPatternField {
        path: field.path,
        pattern: lower_pattern(field.pattern, id_gen),
    }
}

fn lower_literal(lit: HirLiteral) -> KernelLiteral {
    match lit {
        HirLiteral::Number(text) => KernelLiteral::Number(text),
        HirLiteral::String(text) => KernelLiteral::String(text),
        HirLiteral::Sigil { tag, body, flags } => KernelLiteral::Sigil { tag, body, flags },
        HirLiteral::Bool(value) => KernelLiteral::Bool(value),
        HirLiteral::DateTime(text) => KernelLiteral::DateTime(text),
    }
}

fn lower_block_kind(kind: HirBlockKind) -> KernelBlockKind {
    match kind {
        HirBlockKind::Plain => KernelBlockKind::Plain,
        HirBlockKind::Do { monad } => KernelBlockKind::Do { monad },
        HirBlockKind::Generate => KernelBlockKind::Generate,
        HirBlockKind::Resource => KernelBlockKind::Resource,
    }
}

fn lower_block_item(item: HirBlockItem, id_gen: &mut IdGen) -> KernelBlockItem {
    match item {
        HirBlockItem::Bind { pattern, expr, .. } => KernelBlockItem::Bind {
            pattern: lower_pattern(pattern, id_gen),
            expr: lower_expr(expr, id_gen),
        },
        HirBlockItem::Filter { expr } => KernelBlockItem::Filter {
            expr: lower_expr(expr, id_gen),
        },
        HirBlockItem::Yield { expr } => KernelBlockItem::Yield {
            expr: lower_expr(expr, id_gen),
        },
        HirBlockItem::Recurse { expr } => KernelBlockItem::Recurse {
            expr: lower_expr(expr, id_gen),
        },
        HirBlockItem::Expr { expr } => KernelBlockItem::Expr {
            expr: lower_expr(expr, id_gen),
        },
    }
}

fn find_max_id_program(program: &HirProgram) -> u32 {
    let mut max = 0;
    for module in &program.modules {
        for def in &module.defs {
            find_max_id_expr(&def.expr, &mut max);
        }
    }
    max
}

fn find_max_id_expr(expr: &HirExpr, max: &mut u32) {
    match expr {
        HirExpr::Var { id, .. }
        | HirExpr::LitNumber { id, .. }
        | HirExpr::LitString { id, .. }
        | HirExpr::LitSigil { id, .. }
        | HirExpr::LitBool { id, .. }
        | HirExpr::LitDateTime { id, .. }
        | HirExpr::Raw { id, .. } => {
            if *id > *max {
                *max = *id;
            }
        }
        HirExpr::TextInterpolate { id, parts } => {
            if *id > *max {
                *max = *id;
            }
            for part in parts {
                if let crate::hir::HirTextPart::Expr { expr } = part {
                    find_max_id_expr(expr, max);
                }
            }
        }
        HirExpr::Lambda { id, body, .. } => {
            if *id > *max {
                *max = *id;
            }
            find_max_id_expr(body, max);
        }
        HirExpr::App { id, func, arg } => {
            if *id > *max {
                *max = *id;
            }
            find_max_id_expr(func, max);
            find_max_id_expr(arg, max);
        }
        HirExpr::Call { id, func, args } => {
            if *id > *max {
                *max = *id;
            }
            find_max_id_expr(func, max);
            for arg in args {
                find_max_id_expr(arg, max);
            }
        }
        HirExpr::DebugFn { id, body, .. } => {
            if *id > *max {
                *max = *id;
            }
            find_max_id_expr(body, max);
        }
        HirExpr::Pipe {
            id, func, arg, ..
        } => {
            if *id > *max {
                *max = *id;
            }
            find_max_id_expr(func, max);
            find_max_id_expr(arg, max);
        }
        HirExpr::List { id, items } => {
            if *id > *max {
                *max = *id;
            }
            for item in items {
                find_max_id_expr(&item.expr, max);
            }
        }
        HirExpr::Tuple { id, items } => {
            if *id > *max {
                *max = *id;
            }
            for item in items {
                find_max_id_expr(item, max);
            }
        }
        HirExpr::Record { id, fields } | HirExpr::Patch { id, fields, .. } => {
            if *id > *max {
                *max = *id;
            }
            if let HirExpr::Patch { target, .. } = expr {
                find_max_id_expr(target, max);
            }
            for field in fields {
                find_max_id_expr(&field.value, max);
                for seg in &field.path {
                    if let HirPathSegment::Index(idx) = seg {
                        find_max_id_expr(idx, max);
                    }
                }
            }
        }
        HirExpr::FieldAccess { id, base, .. } => {
            if *id > *max {
                *max = *id;
            }
            find_max_id_expr(base, max);
        }
        HirExpr::Index { id, base, index, .. } => {
            if *id > *max {
                *max = *id;
            }
            find_max_id_expr(base, max);
            find_max_id_expr(index, max);
        }
        HirExpr::Match {
            id,
            scrutinee,
            arms,
        } => {
            if *id > *max {
                *max = *id;
            }
            find_max_id_expr(scrutinee, max);
            for arm in arms {
                find_max_id_pattern(&arm.pattern, max);
                if let Some(guard) = &arm.guard {
                    find_max_id_expr(guard, max);
                }
                find_max_id_expr(&arm.body, max);
            }
        }
        HirExpr::If {
            id,
            cond,
            then_branch,
            else_branch,
        } => {
            if *id > *max {
                *max = *id;
            }
            find_max_id_expr(cond, max);
            find_max_id_expr(then_branch, max);
            find_max_id_expr(else_branch, max);
        }
        HirExpr::Binary {
            id, left, right, ..
        } => {
            if *id > *max {
                *max = *id;
            }
            find_max_id_expr(left, max);
            find_max_id_expr(right, max);
        }
        HirExpr::Block { id, items, .. } => {
            if *id > *max {
                *max = *id;
            }
            for item in items {
                match item {
                    HirBlockItem::Bind { pattern, expr, .. } => {
                        find_max_id_pattern(pattern, max);
                        find_max_id_expr(expr, max);
                    }
                    HirBlockItem::Filter { expr }
                    | HirBlockItem::Yield { expr }
                    | HirBlockItem::Recurse { expr }
                    | HirBlockItem::Expr { expr } => {
                        find_max_id_expr(expr, max);
                    }
                }
            }
        }
        HirExpr::Mock { id, substitutions, body } => {
            if *id > *max {
                *max = *id;
            }
            for sub in substitutions {
                if let Some(value) = &sub.value {
                    find_max_id_expr(value, max);
                }
            }
            find_max_id_expr(body, max);
        }
    }
}

fn find_max_id_pattern(pattern: &HirPattern, max: &mut u32) {
    match pattern {
        HirPattern::Wildcard { id } | HirPattern::Var { id, .. } | HirPattern::Literal { id, .. } => {
            if *id > *max {
                *max = *id;
            }
        }
        HirPattern::At { id, pattern, .. } => {
            if *id > *max {
                *max = *id;
            }
            find_max_id_pattern(pattern, max);
        }
        HirPattern::Constructor { id, args, .. } => {
            if *id > *max {
                *max = *id;
            }
            for arg in args {
                find_max_id_pattern(arg, max);
            }
        }
        HirPattern::Tuple { id, items } => {
            if *id > *max {
                *max = *id;
            }
            for item in items {
                find_max_id_pattern(item, max);
            }
        }
        HirPattern::List { id, items, rest } => {
            if *id > *max {
                *max = *id;
            }
            for item in items {
                find_max_id_pattern(item, max);
            }
            if let Some(rest) = rest {
                find_max_id_pattern(rest, max);
            }
        }
        HirPattern::Record { id, fields } => {
            if *id > *max {
                *max = *id;
            }
            for field in fields {
                find_max_id_pattern(&field.pattern, max);
            }
        }
    }
}
