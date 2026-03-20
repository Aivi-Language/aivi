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
                if let HirTextPart::Expr { expr } = part {
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
        HirExpr::App { id, func, arg, .. } => {
            if *id > *max {
                *max = *id;
            }
            find_max_id_expr(func, max);
            find_max_id_expr(arg, max);
        }
        HirExpr::Call { id, func, args, .. } => {
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
            ..
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
            ..
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
