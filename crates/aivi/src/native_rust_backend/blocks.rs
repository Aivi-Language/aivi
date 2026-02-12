use crate::rust_ir::{RustIrBlockItem, RustIrBlockKind};
use crate::AiviError;

use super::expr::emit_expr;
use super::pattern::emit_pattern_bind_stmts;
use super::utils::{collect_free_locals_in_items, rust_local_name};

pub(super) fn emit_block(
    kind: RustIrBlockKind,
    items: &[RustIrBlockItem],
    indent: usize,
) -> Result<String, AiviError> {
    let ind = "    ".repeat(indent);
    let ind2 = "    ".repeat(indent + 1);
    let ind3 = "    ".repeat(indent + 2);
    let mut tmp_id = 0usize;
    let mut s = String::new();
    match kind {
        RustIrBlockKind::Plain => {
            s.push_str("{\n");
            for (i, item) in items.iter().enumerate() {
                let last = i + 1 == items.len();
                match item {
                    RustIrBlockItem::Bind { pattern, expr } => {
                        let expr_code = emit_expr(expr, indent + 1)?;
                        let b_ident = format!("__b{tmp_id}");
                        let ok_ident = format!("__ok{tmp_id}");
                        tmp_id += 1;
                        s.push_str(&ind2);
                        s.push_str(&format!("let __v = ({expr_code})?;\n"));
                        s.push_str(&emit_pattern_bind_stmts(
                            pattern,
                            "__v",
                            &b_ident,
                            &ok_ident,
                            indent + 1,
                            "pattern match failed",
                        )?);
                        if last {
                            s.push_str(&ind2);
                            s.push_str("aivi_ok(Value::Unit)\n");
                        }
                    }
                    RustIrBlockItem::Expr { expr } => {
                        let expr_code = emit_expr(expr, indent + 1)?;
                        s.push_str(&ind2);
                        if last {
                            s.push_str(&expr_code);
                            s.push('\n');
                        } else {
                            s.push_str(&format!("let _ = ({expr_code})?;\n"));
                        }
                    }
                    RustIrBlockItem::Filter { .. }
                    | RustIrBlockItem::Yield { .. }
                    | RustIrBlockItem::Recurse { .. } => {
                        return Err(AiviError::Codegen(
                            "filter/yield/recurse are not supported in plain blocks".to_string(),
                        ))
                    }
                }
            }
            if items.is_empty() {
                s.push_str(&ind2);
                s.push_str("aivi_ok(Value::Unit)\n");
            }
            s.push_str(&ind);
            s.push('}');
        }
        RustIrBlockKind::Effect => {
            // Effect blocks manage resource cleanups. We run cleanups even if the body errors,
            // and prefer the original error over cleanup errors.
            s.push_str("aivi_ok(Value::Effect(Arc::new(EffectValue::Thunk {\n");
            s.push_str(&ind2);
            s.push_str("func: Arc::new(move |rt: &mut Runtime| {\n");
            s.push_str(&ind3);
            s.push_str("let mut __cleanups: Vec<Value> = Vec::new();\n");
            s.push_str(&ind3);
            s.push_str("let __result: R = (|| {\n");

            for (i, item) in items.iter().enumerate() {
                let last = i + 1 == items.len();
                match item {
                    RustIrBlockItem::Bind { pattern, expr } => {
                        let expr_code = emit_expr(expr, indent + 3)?;
                        let b_ident = format!("__b{tmp_id}");
                        let ok_ident = format!("__ok{tmp_id}");
                        tmp_id += 1;
                        s.push_str(&"    ".repeat(indent + 4));
                        s.push_str(&format!("let __tmp = ({expr_code})?;\n"));
                        s.push_str(&"    ".repeat(indent + 4));
                        s.push_str("let __v = match __tmp {\n");
                        s.push_str(&"    ".repeat(indent + 5));
                        s.push_str("Value::Resource(res) => {\n");
                        s.push_str(&"    ".repeat(indent + 6));
                        s.push_str("let (v, cleanup) = rt.acquire_resource(res)?;\n");
                        s.push_str(&"    ".repeat(indent + 6));
                        s.push_str("__cleanups.push(cleanup);\n");
                        s.push_str(&"    ".repeat(indent + 6));
                        s.push_str("v\n");
                        s.push_str(&"    ".repeat(indent + 5));
                        s.push_str("}\n");
                        s.push_str(&"    ".repeat(indent + 5));
                        s.push_str("Value::Effect(_) => rt.run_effect_value(__tmp)?,\n");
                        s.push_str(&"    ".repeat(indent + 5));
                        s.push_str("other => other,\n");
                        s.push_str(&"    ".repeat(indent + 4));
                        s.push_str("};\n");
                        s.push_str(&emit_pattern_bind_stmts(
                            pattern,
                            "__v",
                            &b_ident,
                            &ok_ident,
                            indent + 3,
                            "pattern match failed",
                        )?);
                        if last {
                            s.push_str(&"    ".repeat(indent + 4));
                            s.push_str("Ok(Value::Unit)\n");
                        }
                    }
                    RustIrBlockItem::Expr { expr } => {
                        let expr_code = emit_expr(expr, indent + 3)?;
                        s.push_str(&"    ".repeat(indent + 4));
                        if last {
                            s.push_str(&format!("let __e = ({expr_code})?;\n"));
                            s.push_str(&"    ".repeat(indent + 4));
                            s.push_str("rt.run_effect_value(__e)\n");
                        } else {
                            s.push_str(&format!("let __e = ({expr_code})?;\n"));
                            s.push_str(&"    ".repeat(indent + 4));
                            s.push_str("let _ = rt.run_effect_value(__e)?;\n");
                        }
                    }
                    RustIrBlockItem::Filter { .. }
                    | RustIrBlockItem::Yield { .. }
                    | RustIrBlockItem::Recurse { .. } => {
                        return Err(AiviError::Codegen(
                            "filter/yield/recurse are not supported in effect blocks".to_string(),
                        ))
                    }
                }
            }

            if items.is_empty() {
                s.push_str(&"    ".repeat(indent + 4));
                s.push_str("Ok(Value::Unit)\n");
            }

            s.push_str(&ind3);
            s.push_str("})();\n");
            s.push_str(&ind3);
            s.push_str("let __cleanup_result: Result<(), RuntimeError> = (|| {\n");
            s.push_str(&"    ".repeat(indent + 4));
            s.push_str("for cleanup in __cleanups.into_iter().rev() {\n");
            s.push_str(&"    ".repeat(indent + 5));
            s.push_str("let _ = rt.uncancelable(|rt| rt.run_effect_value(cleanup));\n");
            s.push_str(&"    ".repeat(indent + 4));
            s.push_str("}\n");
            s.push_str(&"    ".repeat(indent + 4));
            s.push_str("Ok(())\n");
            s.push_str(&ind3);
            s.push_str("})();\n");
            s.push_str(&ind3);
            s.push_str("match (__result, __cleanup_result) {\n");
            s.push_str(&"    ".repeat(indent + 4));
            s.push_str("(Err(err), _) => Err(err),\n");
            s.push_str(&"    ".repeat(indent + 4));
            s.push_str("(Ok(_), Err(err)) => Err(err),\n");
            s.push_str(&"    ".repeat(indent + 4));
            s.push_str("(Ok(v), Ok(())) => Ok(v),\n");
            s.push_str(&ind3);
            s.push_str("}\n");
            s.push_str(&ind2);
            s.push_str("}),\n");
            s.push_str(&ind);
            s.push_str("})))");
        }
        RustIrBlockKind::Generate => {
            s.push_str(&emit_generate_block(items, indent)?);
        }
        RustIrBlockKind::Resource => {
            s.push_str(&emit_resource_block(items, indent)?);
        }
    }
    Ok(s)
}

fn emit_generate_block(items: &[RustIrBlockItem], indent: usize) -> Result<String, AiviError> {
    let ind = "    ".repeat(indent);
    let ind2 = "    ".repeat(indent + 1);
    let ind3 = "    ".repeat(indent + 2);
    let mut s = String::new();

    let mut tmp_id = 0usize;
    s.push_str("{\n");
    s.push_str(&ind2);
    s.push_str("let mut __vals: Vec<Value> = Vec::new();\n");
    s.push_str(&ind2);
    s.push_str("(|| -> Result<(), RuntimeError> {\n");
    s.push_str(&emit_generate_materialize_items(
        items,
        "__vals",
        indent + 2,
        &mut tmp_id,
    )?);
    s.push_str(&ind2);
    s.push_str("})()?;\n");
    s.push_str(&ind2);
    s.push_str("let __vals = Arc::new(__vals);\n");
    s.push_str(&ind2);
    s.push_str("aivi_ok(Value::Builtin(BuiltinValue {\n");
    s.push_str(&ind3);
    s.push_str("imp: Arc::new(BuiltinImpl {\n");
    s.push_str(&"    ".repeat(indent + 3));
    s.push_str("name: \"<generator>\".to_string(),\n");
    s.push_str(&"    ".repeat(indent + 3));
    s.push_str("arity: 2,\n");
    s.push_str(&"    ".repeat(indent + 3));
    s.push_str("func: Arc::new(move |mut args, rt| {\n");
    s.push_str(&"    ".repeat(indent + 4));
    s.push_str("let z = args.pop().unwrap();\n");
    s.push_str(&"    ".repeat(indent + 4));
    s.push_str("let k = args.pop().unwrap();\n");
    s.push_str(&"    ".repeat(indent + 4));
    s.push_str("let mut acc = z;\n");
    s.push_str(&"    ".repeat(indent + 4));
    s.push_str("for v in __vals.iter() {\n");
    s.push_str(&"    ".repeat(indent + 5));
    s.push_str("let partial = rt.apply(k.clone(), acc)?;\n");
    s.push_str(&"    ".repeat(indent + 5));
    s.push_str("acc = rt.apply(partial, v.clone())?;\n");
    s.push_str(&"    ".repeat(indent + 4));
    s.push_str("}\n");
    s.push_str(&"    ".repeat(indent + 4));
    s.push_str("Ok(acc)\n");
    s.push_str(&"    ".repeat(indent + 3));
    s.push_str("}),\n");
    s.push_str(&ind3);
    s.push_str("}),\n");
    s.push_str(&ind3);
    s.push_str("args: Vec::new(),\n");
    s.push_str(&ind2);
    s.push_str("}))\n");
    s.push_str(&ind);
    s.push('}');

    Ok(s)
}

fn emit_generate_materialize_items(
    items: &[RustIrBlockItem],
    out_ident: &str,
    indent: usize,
    tmp_id: &mut usize,
) -> Result<String, AiviError> {
    let ind = "    ".repeat(indent);
    let ind2 = "    ".repeat(indent + 1);
    let mut s = String::new();

    if items.is_empty() {
        s.push_str(&ind);
        s.push_str("Ok(())\n");
        return Ok(s);
    }

    match &items[0] {
        RustIrBlockItem::Yield { expr } => {
            let expr_code = emit_expr(expr, indent)?;
            s.push_str(&ind);
            s.push_str(&format!("{out_ident}.push(({expr_code})?);\n"));
            s.push_str(&emit_generate_materialize_items(
                &items[1..],
                out_ident,
                indent,
                tmp_id,
            )?);
        }
        RustIrBlockItem::Expr { expr } => {
            let expr_code = emit_expr(expr, indent)?;
            let sub = format!("__sub{tmp_id}");
            let sub_items = format!("__sub_items{tmp_id}");
            *tmp_id += 1;
            s.push_str(&ind);
            s.push_str(&format!("let {sub} = ({expr_code})?;\n"));
            s.push_str(&ind);
            s.push_str(&format!("let {sub_items} = rt.generator_to_vec({sub})?;\n"));
            s.push_str(&ind);
            s.push_str(&format!("{out_ident}.extend({sub_items});\n"));
            s.push_str(&emit_generate_materialize_items(
                &items[1..],
                out_ident,
                indent,
                tmp_id,
            )?);
        }
        RustIrBlockItem::Filter { expr } => {
            let expr_code = emit_expr(expr, indent)?;
            let cond = format!("__cond{tmp_id}");
            *tmp_id += 1;
            s.push_str(&ind);
            s.push_str(&format!("let {cond} = ({expr_code})?;\n"));
            s.push_str(&ind);
            s.push_str(&format!("if matches!({cond}, Value::Bool(true)) {{\n"));
            s.push_str(&emit_generate_materialize_items(
                &items[1..],
                out_ident,
                indent + 1,
                tmp_id,
            )?);
            s.push_str(&ind);
            s.push_str("}\n");
            s.push_str(&ind);
            s.push_str("Ok(())\n");
        }
        RustIrBlockItem::Bind { pattern, expr } => {
            let expr_code = emit_expr(expr, indent)?;
            let src = format!("__src{tmp_id}");
            let src_items = format!("__src_items{tmp_id}");
            let it = format!("__it{tmp_id}");
            let b_ident = format!("__b{tmp_id}");
            let ok_ident = format!("__ok{tmp_id}");
            *tmp_id += 1;

            s.push_str(&ind);
            s.push_str(&format!("let {src} = ({expr_code})?;\n"));
            s.push_str(&ind);
            s.push_str(&format!("let {src_items} = rt.generator_to_vec({src})?;\n"));
            s.push_str(&ind);
            s.push_str(&format!("for {it} in {src_items} {{\n"));
            s.push_str(&ind2);
            s.push_str(&format!("let __v = {it};\n"));
            s.push_str(&emit_pattern_bind_stmts(
                pattern,
                "__v",
                &b_ident,
                &ok_ident,
                indent + 1,
                "pattern match failed in generator bind",
            )?);
            s.push_str(&emit_generate_materialize_items(
                &items[1..],
                out_ident,
                indent + 1,
                tmp_id,
            )?);
            s.push_str(&ind);
            s.push_str("}\n");
            s.push_str(&ind);
            s.push_str("Ok(())\n");
        }
        RustIrBlockItem::Recurse { .. } => {
            // Unsupported for now.
            s.push_str(&emit_generate_materialize_items(
                &items[1..],
                out_ident,
                indent,
                tmp_id,
            )?);
        }
    }

    Ok(s)
}

fn emit_resource_block(items: &[RustIrBlockItem], indent: usize) -> Result<String, AiviError> {
    let ind = "    ".repeat(indent);
    let ind2 = "    ".repeat(indent + 1);
    let mut tmp_id = 0usize;
    let captured = collect_free_locals_in_items(items);

    let mut s = String::new();
    s.push_str("{\n");
    for name in captured {
        let rust_name = rust_local_name(&name);
        s.push_str(&ind2);
        s.push_str(&format!("let {rust_name} = {rust_name}.clone();\n"));
    }
    s.push_str(&ind2);
    s.push_str("aivi_ok(Value::Resource(Arc::new(ResourceValue {\n");
    s.push_str(&"    ".repeat(indent + 2));
    s.push_str("acquire: Mutex::new(Some(Box::new(move |rt: &mut Runtime| {\n");
    s.push_str(&emit_resource_acquire(items, indent + 3, &mut tmp_id)?);
    s.push_str(&"    ".repeat(indent + 2));
    s.push_str("}))),\n");
    s.push_str(&ind2);
    s.push_str("})))\n");
    s.push_str(&ind);
    s.push_str("}");
    Ok(s)
}

fn emit_resource_acquire(
    items: &[RustIrBlockItem],
    indent: usize,
    tmp_id: &mut usize,
) -> Result<String, AiviError> {
    let ind = "    ".repeat(indent);
    let ind2 = "    ".repeat(indent + 1);
    let mut s = String::new();

    for (i, item) in items.iter().enumerate() {
        match item {
            RustIrBlockItem::Bind { pattern, expr } => {
                let expr_code = emit_expr(expr, indent)?;
                let b_ident = format!("__b{tmp_id}");
                let ok_ident = format!("__ok{tmp_id}");
                *tmp_id += 1;
                s.push_str(&ind);
                s.push_str(&format!("let __e = ({expr_code})?;\n"));
                s.push_str(&ind);
                s.push_str("let __v = rt.run_effect_value(__e)?;\n");
                s.push_str(&emit_pattern_bind_stmts(
                    pattern,
                    "__v",
                    &b_ident,
                    &ok_ident,
                    indent,
                    "pattern match failed in resource bind",
                )?);
            }
            RustIrBlockItem::Expr { expr } => {
                let expr_code = emit_expr(expr, indent)?;
                s.push_str(&ind);
                s.push_str(&format!("let __tmp = ({expr_code})?;\n"));
                s.push_str(&ind);
                s.push_str("if matches!(__tmp, Value::Effect(_)) {\n");
                s.push_str(&ind2);
                s.push_str("let _ = rt.run_effect_value(__tmp)?;\n");
                s.push_str(&ind);
                s.push_str("}\n");
            }
            RustIrBlockItem::Yield { expr } => {
                let expr_code = emit_expr(expr, indent)?;
                s.push_str(&ind);
                s.push_str(&format!("let __value = ({expr_code})?;\n"));
                let cleanup_items = &items[i + 1..];
                s.push_str(&ind);
                s.push_str("let __cleanup = Value::Effect(Arc::new(EffectValue::Thunk {\n");
                s.push_str(&ind2);
                s.push_str("func: Arc::new(move |rt: &mut Runtime| {\n");
                s.push_str(&emit_resource_cleanup(cleanup_items, indent + 2, tmp_id)?);
                s.push_str(&ind2);
                s.push_str("}),\n");
                s.push_str(&ind);
                s.push_str("}));\n");
                s.push_str(&ind);
                s.push_str("return Ok((__value, __cleanup));\n");
            }
            RustIrBlockItem::Filter { .. } | RustIrBlockItem::Recurse { .. } => {
                return Err(AiviError::Codegen(
                    "filter/recurse are not supported in resource blocks".to_string(),
                ))
            }
        }
    }

    s.push_str(&ind);
    s.push_str("Err(RuntimeError::Message(\"resource block missing yield\".to_string()))\n");
    Ok(s)
}

fn emit_resource_cleanup(
    items: &[RustIrBlockItem],
    indent: usize,
    tmp_id: &mut usize,
) -> Result<String, AiviError> {
    let ind = "    ".repeat(indent);
    let mut s = String::new();
    for item in items {
        match item {
            RustIrBlockItem::Bind { pattern, expr } => {
                let expr_code = emit_expr(expr, indent)?;
                let b_ident = format!("__b{tmp_id}");
                let ok_ident = format!("__ok{tmp_id}");
                *tmp_id += 1;
                s.push_str(&ind);
                s.push_str(&format!("let __e = ({expr_code})?;\n"));
                s.push_str(&ind);
                s.push_str("let __v = rt.run_effect_value(__e)?;\n");
                s.push_str(&emit_pattern_bind_stmts(
                    pattern,
                    "__v",
                    &b_ident,
                    &ok_ident,
                    indent,
                    "pattern match failed in resource cleanup bind",
                )?);
            }
            RustIrBlockItem::Expr { expr } => {
                let expr_code = emit_expr(expr, indent)?;
                s.push_str(&ind);
                s.push_str(&format!("let __tmp = ({expr_code})?;\n"));
                s.push_str(&ind);
                s.push_str("if matches!(__tmp, Value::Effect(_)) {\n");
                s.push_str(&"    ".repeat(indent + 1));
                s.push_str("let _ = rt.run_effect_value(__tmp)?;\n");
                s.push_str(&ind);
                s.push_str("}\n");
            }
            RustIrBlockItem::Yield { .. } => {
                return Err(AiviError::Codegen(
                    "yield is not supported in resource cleanup".to_string(),
                ))
            }
            RustIrBlockItem::Filter { .. } | RustIrBlockItem::Recurse { .. } => {
                return Err(AiviError::Codegen(
                    "filter/recurse are not supported in resource cleanup".to_string(),
                ))
            }
        }
    }
    s.push_str(&ind);
    s.push_str("Ok(Value::Unit)\n");
    Ok(s)
}
