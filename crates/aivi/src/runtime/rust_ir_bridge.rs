// RustIR -> HIR bridge utilities used by runtime-backed Cranelift helpers.

pub(crate) fn lower_runtime_rust_ir_block_items(
    items: &[rust_ir::RustIrBlockItem],
) -> Result<Vec<HirBlockItem>, RuntimeError> {
    items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            lower_runtime_rust_ir_block_item(item).ok_or_else(|| {
                RuntimeError::Message(format!(
                    "failed to lower jitted block item at index {index}"
                ))
            })
        })
        .collect()
}

fn lower_runtime_rust_ir_expr(expr: &rust_ir::RustIrExpr) -> Option<HirExpr> {
    Some(match expr {
        rust_ir::RustIrExpr::Local { id, name } | rust_ir::RustIrExpr::Global { id, name } => {
            HirExpr::Var {
                id: *id,
                name: name.clone(),
            }
        }
        rust_ir::RustIrExpr::Builtin { id, builtin } => HirExpr::Var {
            id: *id,
            name: builtin.clone(),
        },
        rust_ir::RustIrExpr::ConstructorValue { id, name } => HirExpr::Var {
            id: *id,
            name: name.clone(),
        },
        rust_ir::RustIrExpr::LitNumber { id, text } => HirExpr::LitNumber {
            id: *id,
            text: text.clone(),
        },
        rust_ir::RustIrExpr::LitString { id, text } => HirExpr::LitString {
            id: *id,
            text: text.clone(),
        },
        rust_ir::RustIrExpr::TextInterpolate { id, parts } => HirExpr::TextInterpolate {
            id: *id,
            parts: parts
                .iter()
                .map(lower_runtime_rust_ir_text_part)
                .collect::<Option<Vec<_>>>()?,
        },
        rust_ir::RustIrExpr::LitSigil {
            id,
            tag,
            body,
            flags,
        } => HirExpr::LitSigil {
            id: *id,
            tag: tag.clone(),
            body: body.clone(),
            flags: flags.clone(),
        },
        rust_ir::RustIrExpr::LitBool { id, value } => HirExpr::LitBool {
            id: *id,
            value: *value,
        },
        rust_ir::RustIrExpr::LitDateTime { id, text } => HirExpr::LitDateTime {
            id: *id,
            text: text.clone(),
        },
        rust_ir::RustIrExpr::Lambda { id, param, body } => HirExpr::Lambda {
            id: *id,
            param: param.clone(),
            body: Box::new(lower_runtime_rust_ir_expr(body)?),
        },
        rust_ir::RustIrExpr::App { id, func, arg } => HirExpr::App {
            id: *id,
            func: Box::new(lower_runtime_rust_ir_expr(func)?),
            arg: Box::new(lower_runtime_rust_ir_expr(arg)?),
        },
        rust_ir::RustIrExpr::Call { id, func, args } => HirExpr::Call {
            id: *id,
            func: Box::new(lower_runtime_rust_ir_expr(func)?),
            args: args
                .iter()
                .map(lower_runtime_rust_ir_expr)
                .collect::<Option<Vec<_>>>()?,
        },
        rust_ir::RustIrExpr::DebugFn {
            id,
            fn_name,
            arg_vars,
            log_args,
            log_return,
            log_time,
            body,
        } => HirExpr::DebugFn {
            id: *id,
            fn_name: fn_name.clone(),
            arg_vars: arg_vars.clone(),
            log_args: *log_args,
            log_return: *log_return,
            log_time: *log_time,
            body: Box::new(lower_runtime_rust_ir_expr(body)?),
        },
        rust_ir::RustIrExpr::Pipe {
            id,
            pipe_id,
            step,
            label,
            log_time,
            func,
            arg,
        } => HirExpr::Pipe {
            id: *id,
            pipe_id: *pipe_id,
            step: *step,
            label: label.clone(),
            log_time: *log_time,
            func: Box::new(lower_runtime_rust_ir_expr(func)?),
            arg: Box::new(lower_runtime_rust_ir_expr(arg)?),
        },
        rust_ir::RustIrExpr::List { id, items } => HirExpr::List {
            id: *id,
            items: items
                .iter()
                .map(|item| {
                    Some(HirListItem {
                        expr: lower_runtime_rust_ir_expr(&item.expr)?,
                        spread: item.spread,
                    })
                })
                .collect::<Option<Vec<_>>>()?,
        },
        rust_ir::RustIrExpr::Tuple { id, items } => HirExpr::Tuple {
            id: *id,
            items: items
                .iter()
                .map(lower_runtime_rust_ir_expr)
                .collect::<Option<Vec<_>>>()?,
        },
        rust_ir::RustIrExpr::Record { id, fields } => HirExpr::Record {
            id: *id,
            fields: fields
                .iter()
                .map(lower_runtime_rust_ir_record_field)
                .collect::<Option<Vec<_>>>()?,
        },
        rust_ir::RustIrExpr::Patch { id, target, fields } => HirExpr::Patch {
            id: *id,
            target: Box::new(lower_runtime_rust_ir_expr(target)?),
            fields: fields
                .iter()
                .map(lower_runtime_rust_ir_record_field)
                .collect::<Option<Vec<_>>>()?,
        },
        rust_ir::RustIrExpr::FieldAccess { id, base, field } => HirExpr::FieldAccess {
            id: *id,
            base: Box::new(lower_runtime_rust_ir_expr(base)?),
            field: field.clone(),
        },
        rust_ir::RustIrExpr::Index { id, base, index, location } => HirExpr::Index {
            id: *id,
            base: Box::new(lower_runtime_rust_ir_expr(base)?),
            index: Box::new(lower_runtime_rust_ir_expr(index)?),
            location: location.clone(),
        },
        rust_ir::RustIrExpr::Match {
            id,
            scrutinee,
            arms,
        } => HirExpr::Match {
            id: *id,
            scrutinee: Box::new(lower_runtime_rust_ir_expr(scrutinee)?),
            arms: arms
                .iter()
                .map(lower_runtime_rust_ir_match_arm)
                .collect::<Option<Vec<_>>>()?,
        },
        rust_ir::RustIrExpr::If {
            id,
            cond,
            then_branch,
            else_branch,
        } => HirExpr::If {
            id: *id,
            cond: Box::new(lower_runtime_rust_ir_expr(cond)?),
            then_branch: Box::new(lower_runtime_rust_ir_expr(then_branch)?),
            else_branch: Box::new(lower_runtime_rust_ir_expr(else_branch)?),
        },
        rust_ir::RustIrExpr::Binary {
            id,
            op,
            left,
            right,
        } => HirExpr::Binary {
            id: *id,
            op: op.clone(),
            left: Box::new(lower_runtime_rust_ir_expr(left)?),
            right: Box::new(lower_runtime_rust_ir_expr(right)?),
        },
        rust_ir::RustIrExpr::Block {
            id,
            block_kind,
            items,
        } => HirExpr::Block {
            id: *id,
            block_kind: lower_runtime_rust_ir_block_kind(block_kind)?,
            items: items
                .iter()
                .map(lower_runtime_rust_ir_block_item)
                .collect::<Option<Vec<_>>>()?,
        },
        rust_ir::RustIrExpr::Raw { id, text } => HirExpr::Raw {
            id: *id,
            text: text.clone(),
        },
    })
}

fn lower_runtime_rust_ir_text_part(part: &rust_ir::RustIrTextPart) -> Option<HirTextPart> {
    Some(match part {
        rust_ir::RustIrTextPart::Text { text } => HirTextPart::Text { text: text.clone() },
        rust_ir::RustIrTextPart::Expr { expr } => HirTextPart::Expr {
            expr: lower_runtime_rust_ir_expr(expr)?,
        },
    })
}

fn lower_runtime_rust_ir_record_field(
    field: &rust_ir::RustIrRecordField,
) -> Option<HirRecordField> {
    Some(HirRecordField {
        spread: field.spread,
        path: field
            .path
            .iter()
            .map(lower_runtime_rust_ir_path_segment)
            .collect::<Option<Vec<_>>>()?,
        value: lower_runtime_rust_ir_expr(&field.value)?,
    })
}

fn lower_runtime_rust_ir_path_segment(seg: &rust_ir::RustIrPathSegment) -> Option<HirPathSegment> {
    Some(match seg {
        rust_ir::RustIrPathSegment::Field(name) => HirPathSegment::Field(name.clone()),
        rust_ir::RustIrPathSegment::IndexValue(expr)
        | rust_ir::RustIrPathSegment::IndexPredicate(expr) => {
            HirPathSegment::Index(lower_runtime_rust_ir_expr(expr)?)
        }
        rust_ir::RustIrPathSegment::IndexFieldBool(name) => HirPathSegment::Index(HirExpr::Var {
            id: 0,
            name: name.clone(),
        }),
        rust_ir::RustIrPathSegment::IndexAll => HirPathSegment::All,
    })
}

fn lower_runtime_rust_ir_match_arm(arm: &rust_ir::RustIrMatchArm) -> Option<HirMatchArm> {
    Some(HirMatchArm {
        pattern: lower_runtime_rust_ir_pattern(&arm.pattern),
        guard: match arm.guard.as_ref() {
            Some(guard) => Some(lower_runtime_rust_ir_expr(guard)?),
            None => None,
        },
        body: lower_runtime_rust_ir_expr(&arm.body)?,
    })
}

fn lower_runtime_rust_ir_pattern(pattern: &rust_ir::RustIrPattern) -> HirPattern {
    match pattern {
        rust_ir::RustIrPattern::Wildcard { id } => HirPattern::Wildcard { id: *id },
        rust_ir::RustIrPattern::Var { id, name } => HirPattern::Var {
            id: *id,
            name: name.clone(),
        },
        rust_ir::RustIrPattern::At { id, name, pattern } => HirPattern::At {
            id: *id,
            name: name.clone(),
            pattern: Box::new(lower_runtime_rust_ir_pattern(pattern)),
        },
        rust_ir::RustIrPattern::Literal { id, value } => HirPattern::Literal {
            id: *id,
            value: lower_runtime_rust_ir_literal(value),
        },
        rust_ir::RustIrPattern::Constructor { id, name, args } => HirPattern::Constructor {
            id: *id,
            name: name.clone(),
            args: args.iter().map(lower_runtime_rust_ir_pattern).collect(),
        },
        rust_ir::RustIrPattern::Tuple { id, items } => HirPattern::Tuple {
            id: *id,
            items: items.iter().map(lower_runtime_rust_ir_pattern).collect(),
        },
        rust_ir::RustIrPattern::List { id, items, rest } => HirPattern::List {
            id: *id,
            items: items.iter().map(lower_runtime_rust_ir_pattern).collect(),
            rest: rest.as_ref().map(|rest| Box::new(lower_runtime_rust_ir_pattern(rest.as_ref()))),
        },
        rust_ir::RustIrPattern::Record { id, fields } => HirPattern::Record {
            id: *id,
            fields: fields
                .iter()
                .map(|field| crate::hir::HirRecordPatternField {
                    path: field.path.clone(),
                    pattern: lower_runtime_rust_ir_pattern(&field.pattern),
                })
                .collect(),
        },
    }
}

fn lower_runtime_rust_ir_literal(literal: &rust_ir::RustIrLiteral) -> HirLiteral {
    match literal {
        rust_ir::RustIrLiteral::Number(value) => HirLiteral::Number(value.clone()),
        rust_ir::RustIrLiteral::String(value) => HirLiteral::String(value.clone()),
        rust_ir::RustIrLiteral::Sigil { tag, body, flags } => HirLiteral::Sigil {
            tag: tag.clone(),
            body: body.clone(),
            flags: flags.clone(),
        },
        rust_ir::RustIrLiteral::Bool(value) => HirLiteral::Bool(*value),
        rust_ir::RustIrLiteral::DateTime(value) => HirLiteral::DateTime(value.clone()),
    }
}

fn lower_runtime_rust_ir_block_kind(
    kind: &rust_ir::RustIrBlockKind,
) -> Option<crate::hir::HirBlockKind> {
    Some(match kind {
        rust_ir::RustIrBlockKind::Plain => crate::hir::HirBlockKind::Plain,
        rust_ir::RustIrBlockKind::Do { monad } => crate::hir::HirBlockKind::Do {
            monad: monad.clone(),
        },
        rust_ir::RustIrBlockKind::Generate => crate::hir::HirBlockKind::Generate,
        rust_ir::RustIrBlockKind::Resource => crate::hir::HirBlockKind::Resource,
    })
}

fn lower_runtime_rust_ir_block_item(item: &rust_ir::RustIrBlockItem) -> Option<HirBlockItem> {
    Some(match item {
        rust_ir::RustIrBlockItem::Bind { pattern, expr } => HirBlockItem::Bind {
            pattern: lower_runtime_rust_ir_pattern(pattern),
            expr: lower_runtime_rust_ir_expr(expr)?,
        },
        rust_ir::RustIrBlockItem::Filter { expr } => HirBlockItem::Filter {
            expr: lower_runtime_rust_ir_expr(expr)?,
        },
        rust_ir::RustIrBlockItem::Yield { expr } => HirBlockItem::Yield {
            expr: lower_runtime_rust_ir_expr(expr)?,
        },
        rust_ir::RustIrBlockItem::Recurse { expr } => HirBlockItem::Recurse {
            expr: lower_runtime_rust_ir_expr(expr)?,
        },
        rust_ir::RustIrBlockItem::Expr { expr } => HirBlockItem::Expr {
            expr: lower_runtime_rust_ir_expr(expr)?,
        },
    })
}
