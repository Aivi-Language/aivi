use std::collections::HashSet;

use crate::rust_ir::{RustIrExpr, RustIrPathSegment, RustIrRecordField};
use crate::AiviError;

use super::blocks::emit_block;
use super::pattern::emit_match;
use super::utils::{collect_free_locals_in_expr, rust_global_fn_name, rust_local_name};

pub(super) fn emit_expr(expr: &RustIrExpr, indent: usize) -> Result<String, AiviError> {
    Ok(match expr {
        RustIrExpr::Local { name, .. } => format!("aivi_ok({}.clone())", rust_local_name(name)),
        RustIrExpr::Global { name, .. } => format!("{}(rt)", rust_global_fn_name(name)),
        RustIrExpr::Builtin { builtin, .. } => format!("aivi_ok(__builtin({builtin:?}))"),
        RustIrExpr::ConstructorValue { name, .. } => format!(
            "aivi_ok(Value::Constructor {{ name: {:?}.to_string(), args: Vec::new() }})",
            name
        ),

        RustIrExpr::LitNumber { text, .. } => {
            if let Ok(value) = text.parse::<i64>() {
                format!("aivi_ok(Value::Int({value}))")
            } else if let Ok(value) = text.parse::<f64>() {
                format!("aivi_ok(Value::Float({value:?}))")
            } else {
                return Err(AiviError::Codegen(format!(
                    "unsupported numeric literal {text}"
                )));
            }
        }
        RustIrExpr::LitString { text, .. } => {
            format!("aivi_ok(Value::Text({:?}.to_string()))", text)
        }
        RustIrExpr::TextInterpolate { parts, .. } => {
            let ind = "    ".repeat(indent);
            let ind2 = "    ".repeat(indent + 1);
            let mut out = String::new();
            out.push_str("{\n");
            out.push_str(&ind2);
            out.push_str("let mut s = String::new();\n");
            for part in parts {
                match part {
                    crate::rust_ir::RustIrTextPart::Text { text } => {
                        out.push_str(&ind2);
                        out.push_str(&format!("s.push_str({text:?});\n"));
                    }
                    crate::rust_ir::RustIrTextPart::Expr { expr } => {
                        let expr_code = emit_expr(expr, indent + 1)?;
                        out.push_str(&ind2);
                        out.push_str(&format!("let v = ({expr_code})?;\n"));
                        out.push_str(&ind2);
                        out.push_str("s.push_str(&aivi_native_runtime::format_value(&v));\n");
                    }
                }
            }
            out.push_str(&ind2);
            out.push_str("aivi_ok(Value::Text(s))\n");
            out.push_str(&ind);
            out.push('}');
            out
        }
        RustIrExpr::LitSigil {
            tag, body, flags, ..
        } => {
            let ind = "    ".repeat(indent);
            let ind2 = "    ".repeat(indent + 1);
            let ind3 = "    ".repeat(indent + 2);
            format!(
                "{{\n{ind2}let mut map = HashMap::new();\n{ind3}map.insert(\"tag\".to_string(), Value::Text({tag:?}.to_string()));\n{ind3}map.insert(\"body\".to_string(), Value::Text({body:?}.to_string()));\n{ind3}map.insert(\"flags\".to_string(), Value::Text({flags:?}.to_string()));\n{ind2}aivi_ok(Value::Record(Arc::new(map)))\n{ind}}}"
            )
        }
        RustIrExpr::LitBool { value, .. } => format!("aivi_ok(Value::Bool({value}))"),
        RustIrExpr::LitDateTime { text, .. } => {
            format!("aivi_ok(Value::DateTime({:?}.to_string()))", text)
        }

        RustIrExpr::Lambda { param, body, .. } => {
            let param_name = rust_local_name(param);
            let mut bound = vec![param.clone()];
            let mut captured: HashSet<String> = HashSet::new();
            collect_free_locals_in_expr(body, &mut bound, &mut captured);
            let mut captured = captured.into_iter().collect::<Vec<_>>();
            captured.sort();
            let body_code = emit_expr(body, indent + 1)?;
            let ind = "    ".repeat(indent);
            let ind2 = "    ".repeat(indent + 1);
            let mut capture_lines = String::new();
            for name in captured {
                let rust_name = rust_local_name(&name);
                capture_lines.push_str(&format!("{ind2}let {rust_name} = {rust_name}.clone();\n"));
            }
            format!(
                "aivi_ok(Value::Closure(Arc::new(aivi_native_runtime::ClosureValue {{ func: Arc::new(move |{param_name}: Value, rt: &mut Runtime| {{\n{capture_lines}{ind2}{body_code}\n{ind}}}) }})))"
            )
        }
        RustIrExpr::App { func, arg, .. } => {
            let func_code = emit_expr(func, indent)?;
            let arg_code = emit_expr(arg, indent)?;
            let ind = "    ".repeat(indent);
            let ind2 = "    ".repeat(indent + 1);
            format!(
                "{{\n{ind2}let f = ({func_code})?;\n{ind2}let a = ({arg_code})?;\n{ind2}rt.apply(f, a)\n{ind}}}"
            )
        }
        RustIrExpr::Call { func, args, .. } => {
            let func_code = emit_expr(func, indent)?;
            let ind = "    ".repeat(indent);
            let ind2 = "    ".repeat(indent + 1);
            let mut rendered = String::new();
            rendered.push_str(&format!("{{\n{ind2}let f = ({func_code})?;\n"));
            // Avoid collisions with user variables named `args`.
            rendered.push_str(&format!(
                "{ind2}let mut __aivi_call_args: Vec<Value> = Vec::new();\n"
            ));
            for arg in args {
                let arg_code = emit_expr(arg, indent + 1)?;
                rendered.push_str(&format!("{ind2}__aivi_call_args.push(({arg_code})?);\n"));
            }
            rendered.push_str(&format!("{ind2}rt.call(f, __aivi_call_args)\n{ind}}}"));
            rendered
        }
        RustIrExpr::List { items, .. } => {
            let mut parts = Vec::new();
            for item in items {
                let expr_code = emit_expr(&item.expr, indent)?;
                if item.spread {
                    parts.push(format!(
                        "{{ let v = ({expr_code})?; match v {{ Value::List(xs) => (*xs).clone(), other => return Err(RuntimeError::Message(format!(\"expected List for spread, got {{}}\", aivi_native_runtime::format_value(&other)))), }} }}"
                    ));
                } else {
                    parts.push(format!("vec![({expr_code})?]"));
                }
            }
            let concat = if parts.is_empty() {
                "Vec::new()".to_string()
            } else if parts.len() == 1 {
                parts[0].clone()
            } else {
                let mut s = String::new();
                s.push_str("{ let mut out = Vec::new();");
                for part in parts {
                    s.push_str(" out.extend(");
                    s.push_str(&part);
                    s.push_str(");");
                }
                s.push_str(" out }");
                s
            };
            format!("aivi_ok(Value::List(Arc::new({concat})))")
        }
        RustIrExpr::Tuple { items, .. } => {
            let mut rendered = Vec::new();
            for item in items {
                rendered.push(format!("({})?", emit_expr(item, indent)?));
            }
            format!("aivi_ok(Value::Tuple(vec![{}]))", rendered.join(", "))
        }
        RustIrExpr::Record { fields, .. } => emit_record(fields, indent)?,
        RustIrExpr::Patch { target, fields, .. } => {
            let target_code = emit_expr(target, indent)?;
            let fields_code = emit_patch_fields(fields, indent)?;
            let ind = "    ".repeat(indent);
            let ind2 = "    ".repeat(indent + 1);
            format!(
                "{{\n{ind2}let t = ({target_code})?;\n{ind2}let fields = {fields_code};\n{ind2}patch(rt, t, fields)\n{ind}}}"
            )
        }
        RustIrExpr::FieldAccess { base, field, .. } => {
            let base_code = emit_expr(base, indent)?;
            format!(
                "({base_code}).and_then(|b| match b {{ Value::Record(map) => map.get({:?}).cloned().ok_or_else(|| RuntimeError::Message(\"missing field\".to_string())), other => Err(RuntimeError::Message(format!(\"expected Record, got {{}}\", aivi_native_runtime::format_value(&other)))), }})",
                field
            )
        }
        RustIrExpr::Index { base, index, .. } => {
            let base_code = emit_expr(base, indent)?;
            let index_code = emit_expr(index, indent)?;
            format!(
                "({base_code}).and_then(|b| ({index_code}).and_then(|i| match (b, i) {{ (Value::List(items), Value::Int(idx)) => items.get(idx as usize).cloned().ok_or_else(|| RuntimeError::Message(\"index out of bounds\".to_string())), (Value::Tuple(items), Value::Int(idx)) => items.get(idx as usize).cloned().ok_or_else(|| RuntimeError::Message(\"index out of bounds\".to_string())), (other, _) => Err(RuntimeError::Message(format!(\"index on unsupported value {{}}\", aivi_native_runtime::format_value(&other)))), }}))"
            )
        }
        RustIrExpr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            let cond_code = emit_expr(cond, indent)?;
            let then_code = emit_expr(then_branch, indent)?;
            let else_code = emit_expr(else_branch, indent)?;
            format!(
                "({cond_code}).and_then(|c| match c {{ Value::Bool(true) => {then_code}, Value::Bool(false) => {else_code}, other => Err(RuntimeError::Message(format!(\"expected Bool, got {{}}\", aivi_native_runtime::format_value(&other)))), }})"
            )
        }
        RustIrExpr::Binary {
            op, left, right, ..
        } => {
            let left_code = emit_expr(left, indent)?;
            let right_code = emit_expr(right, indent)?;
            emit_binary(op, left_code, right_code)
        }
        RustIrExpr::Block {
            block_kind, items, ..
        } => emit_block(*block_kind, items, indent)?,
        RustIrExpr::Raw { text, .. } => {
            return Err(AiviError::Codegen(format!(
                "raw expressions are not supported by the native backend yet: {text}"
            )))
        }
        RustIrExpr::Match {
            scrutinee, arms, ..
        } => emit_match(scrutinee, arms, indent)?,
    })
}

fn emit_record(fields: &[RustIrRecordField], indent: usize) -> Result<String, AiviError> {
    let mut stmts = Vec::new();
    for field in fields {
        if field.spread {
            let value_code = emit_expr(&field.value, indent)?;
            stmts.push(format!(
                "match ({value_code})? {{ Value::Record(m) => {{ map.extend(m.as_ref().clone()); }}, _ => return Err(RuntimeError::Message(\"record spread expects a record\".to_string())), }};"
            ));
            continue;
        }
        if field.path.len() != 1 {
            return Err(AiviError::Codegen(
                "nested record paths are not supported in record literals yet".to_string(),
            ));
        }
        match &field.path[0] {
            RustIrPathSegment::Field(name) => {
                let value_code = emit_expr(&field.value, indent)?;
                stmts.push(format!(
                    "map.insert({:?}.to_string(), ({value_code})?);",
                    name
                ));
            }
            _ => {
                return Err(AiviError::Codegen(
                    "index paths are not supported in record literals yet".to_string(),
                ))
            }
        }
    }
    let ind = "    ".repeat(indent);
    let ind2 = "    ".repeat(indent + 1);
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&ind2);
    out.push_str("let mut map = HashMap::new();\n");
    for stmt in stmts {
        out.push_str(&ind2);
        out.push_str(&stmt);
        out.push('\n');
    }
    out.push_str(&ind2);
    out.push_str("aivi_ok(Value::Record(Arc::new(map)))\n");
    out.push_str(&ind);
    out.push('}');
    Ok(out)
}

fn emit_patch_fields(fields: &[RustIrRecordField], indent: usize) -> Result<String, AiviError> {
    let mut out = String::new();
    out.push_str("vec![");
    for (i, field) in fields.iter().enumerate() {
        if field.spread {
            return Err(AiviError::Codegen(
                "record spread is not supported in patch literals".to_string(),
            ));
        }
        if i != 0 {
            out.push_str(", ");
        }
        out.push('(');
        out.push_str(&emit_path(&field.path, indent)?);
        out.push_str(", ");
        out.push_str(&format!("({})?", emit_expr(&field.value, indent)?));
        out.push(')');
    }
    out.push(']');
    Ok(out)
}

fn emit_path(path: &[RustIrPathSegment], indent: usize) -> Result<String, AiviError> {
    let mut out = String::new();
    out.push_str("vec![");
    for (i, seg) in path.iter().enumerate() {
        if i != 0 {
            out.push_str(", ");
        }
        match seg {
            RustIrPathSegment::Field(name) => {
                out.push_str(&format!("PathSeg::Field({:?}.to_string())", name));
            }
            RustIrPathSegment::IndexFieldBool(name) => {
                out.push_str(&format!("PathSeg::IndexFieldBool({:?}.to_string())", name));
            }
            RustIrPathSegment::IndexPredicate(expr) => {
                out.push_str("PathSeg::IndexPredicate(");
                out.push_str(&format!("({})?", emit_expr(expr, indent)?));
                out.push(')');
            }
            RustIrPathSegment::IndexValue(expr) => {
                out.push_str("PathSeg::IndexValue(");
                out.push_str(&format!("({})?", emit_expr(expr, indent)?));
                out.push(')');
            }
        }
    }
    out.push(']');
    Ok(out)
}

fn emit_binary(op: &str, left_code: String, right_code: String) -> String {
    match op {
        "==" => format!(
            "({left_code}).and_then(|a| ({right_code}).map(|b| Value::Bool(aivi_native_runtime::values_equal(&a, &b))))"
        ),
        "!=" => format!(
            "({left_code}).and_then(|a| ({right_code}).map(|b| Value::Bool(!aivi_native_runtime::values_equal(&a, &b))))"
        ),
        "+" | "-" | "*" | "/" => {
            let template = r#"({LEFT}).and_then(|l| ({RIGHT}).and_then(|r| match (l, r) {
        (Value::Int(a), Value::Int(b)) => aivi_ok(Value::Int(a <OP> b)),
        (Value::Float(a), Value::Float(b)) => aivi_ok(Value::Float(a <OP> b)),
        (Value::Int(a), Value::Float(b)) => aivi_ok(Value::Float((a as f64) <OP> b)),
        (Value::Float(a), Value::Int(b)) => aivi_ok(Value::Float(a <OP> (b as f64))),
        (l, r) => Err(RuntimeError::Message(format!("unsupported operands for {OP}: {} and {}", aivi_native_runtime::format_value(&l), aivi_native_runtime::format_value(&r)))),
    }))"#;
            template
                .replace("{LEFT}", &left_code)
                .replace("{RIGHT}", &right_code)
                .replace("<OP>", op)
                .replace("{OP}", op)
        }
        "<" | "<=" | ">" | ">=" => {
            let template = r#"({LEFT}).and_then(|l| ({RIGHT}).and_then(|r| match (l, r) {
        (Value::Int(a), Value::Int(b)) => aivi_ok(Value::Bool(a <OP> b)),
        (Value::Float(a), Value::Float(b)) => aivi_ok(Value::Bool(a <OP> b)),
        (Value::Int(a), Value::Float(b)) => aivi_ok(Value::Bool((a as f64) <OP> b)),
        (Value::Float(a), Value::Int(b)) => aivi_ok(Value::Bool(a <OP> (b as f64))),
        (l, r) => Err(RuntimeError::Message(format!("unsupported operands for {OP}: {} and {}", aivi_native_runtime::format_value(&l), aivi_native_runtime::format_value(&r)))),
    }))"#;
            template
                .replace("{LEFT}", &left_code)
                .replace("{RIGHT}", &right_code)
                .replace("<OP>", op)
                .replace("{OP}", op)
        }
        "&&" | "||" => {
            let template = r#"({LEFT}).and_then(|l| ({RIGHT}).and_then(|r| match (l, r) {
        (Value::Bool(a), Value::Bool(b)) => aivi_ok(Value::Bool(a <OP> b)),
        (l, r) => Err(RuntimeError::Message(format!("unsupported operands for {OP}: {} and {}", aivi_native_runtime::format_value(&l), aivi_native_runtime::format_value(&r)))),
    }))"#;
            template
                .replace("{LEFT}", &left_code)
                .replace("{RIGHT}", &right_code)
                .replace("<OP>", op)
                .replace("{OP}", op)
        }
        _ => "Err(RuntimeError::Message(\"unsupported binary operator\".to_string()))".to_string(),
    }
}
