use std::collections::HashSet;

use crate::i18n::{parse_message_template, validate_key_text, MessagePart};
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
            match tag.as_str() {
                "k" => {
                    validate_key_text(body).map_err(|msg| {
                        AiviError::Codegen(format!("invalid i18n key literal: {msg}"))
                    })?;
                    format!(
                        "{{\n{ind2}let mut map = HashMap::new();\n{ind3}map.insert(\"tag\".to_string(), Value::Text({tag:?}.to_string()));\n{ind3}map.insert(\"body\".to_string(), Value::Text({trimmed:?}.to_string()));\n{ind3}map.insert(\"flags\".to_string(), Value::Text({flags:?}.to_string()));\n{ind2}aivi_ok(Value::Record(Arc::new(map)))\n{ind}}}",
                        trimmed = body.trim()
                    )
                }
                "m" => {
                    let parsed = parse_message_template(body).map_err(|msg| {
                        AiviError::Codegen(format!("invalid i18n message literal: {msg}"))
                    })?;
                    let parts_code = emit_i18n_message_parts(&parsed.parts, indent + 2);
                    format!(
                        "{{\n{ind2}let mut map = HashMap::new();\n{ind3}map.insert(\"tag\".to_string(), Value::Text({tag:?}.to_string()));\n{ind3}map.insert(\"body\".to_string(), Value::Text({body:?}.to_string()));\n{ind3}map.insert(\"flags\".to_string(), Value::Text({flags:?}.to_string()));\n{ind3}map.insert(\"parts\".to_string(), {parts_code});\n{ind2}aivi_ok(Value::Record(Arc::new(map)))\n{ind}}}"
                    )
                }
                _ => format!(
                    "{{\n{ind2}let mut map = HashMap::new();\n{ind3}map.insert(\"tag\".to_string(), Value::Text({tag:?}.to_string()));\n{ind3}map.insert(\"body\".to_string(), Value::Text({body:?}.to_string()));\n{ind3}map.insert(\"flags\".to_string(), Value::Text({flags:?}.to_string()));\n{ind2}aivi_ok(Value::Record(Arc::new(map)))\n{ind}}}"
                ),
            }
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
            // Clone captured variables OUTSIDE the `move` closure so the
            // originals remain available in the surrounding scope.
            //
            // `__loop`-prefixed captures use a deferred-init holder
            // (`Arc<Mutex<Value>>`) because the variable is self-referential:
            // the Bind that defines `__loop0` contains the very lambda that
            // captures it, so the plain value does not exist yet at capture
            // time.
            let mut pre_clone = String::new();
            let mut loop_reads = String::new();
            for name in &captured {
                let rust_name = rust_local_name(name);
                if name.starts_with("__loop") {
                    let holder_name = format!("{rust_name}_holder");
                    pre_clone.push_str(&format!(
                        "{ind2}let {holder_name} = {holder_name}.clone();\n"
                    ));
                    loop_reads.push_str(&format!(
                        "{ind2}let {rust_name} = (*{holder_name}.lock().unwrap()).clone();\n"
                    ));
                } else {
                    pre_clone.push_str(&format!("{ind2}let {rust_name} = {rust_name}.clone();\n"));
                }
            }
            if captured.is_empty() {
                format!(
                    "aivi_ok(Value::Closure(Arc::new(aivi_native_runtime::ClosureValue {{ func: Arc::new(move |{param_name}: Value, rt: &mut Runtime| {{\n{ind2}{body_code}\n{ind}}}) }})))"
                )
            } else {
                format!(
                    "{{\n{pre_clone}{ind2}aivi_ok(Value::Closure(Arc::new(aivi_native_runtime::ClosureValue {{ func: Arc::new(move |{param_name}: Value, rt: &mut Runtime| {{\n{loop_reads}{ind2}{body_code}\n{ind}}}) }})))\n{ind}}}"
                )
            }
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
        RustIrExpr::DebugFn {
            fn_name,
            arg_vars,
            log_args,
            log_return,
            log_time,
            body,
            ..
        } => {
            let ind = "    ".repeat(indent);
            let ind2 = "    ".repeat(indent + 1);
            let body_code = emit_expr(body, indent + 1)?;

            let args_vec = if *log_args {
                let rendered_args = arg_vars
                    .iter()
                    .map(|name| format!("{}.clone()", rust_local_name(name)))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("Some(vec![{rendered_args}])")
            } else {
                "None".to_string()
            };

            format!(
                "{{\n{ind2}rt.debug_fn_enter({fn_name:?}, {args_vec}, {log_time});\n{ind2}let __aivi_dbg_out: R = {body_code};\n{ind2}rt.debug_fn_exit(&__aivi_dbg_out, {log_return}, {log_time});\n{ind2}__aivi_dbg_out\n{ind}}}"
            )
        }
        RustIrExpr::Pipe {
            pipe_id,
            step,
            label,
            log_time,
            func,
            arg,
            ..
        } => {
            let func_code = emit_expr(func, indent)?;
            let arg_code = emit_expr(arg, indent)?;
            let ind = "    ".repeat(indent);
            let ind2 = "    ".repeat(indent + 1);
            format!(
                "{{\n{ind2}let f = ({func_code})?;\n{ind2}let a = ({arg_code})?;\n{ind2}rt.debug_pipe_in({pipe_id}, {step}, {label:?}, &a, {log_time});\n{ind2}let __aivi_dbg_step_start = if {log_time} {{ Some(std::time::Instant::now()) }} else {{ None }};\n{ind2}let out = rt.apply(f, a)?;\n{ind2}rt.debug_pipe_out({pipe_id}, {step}, {label:?}, &out, __aivi_dbg_step_start, {log_time});\n{ind2}aivi_ok(out)\n{ind}}}"
            )
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
                "({base_code}).and_then(|b| match b {{ Value::Record(map) => map.get({:?}).cloned().ok_or_else(|| RuntimeError::Message({:?}.to_string())), other => Err(RuntimeError::Message(format!(\"expected Record, got {{}}\", aivi_native_runtime::format_value(&other)))), }})",
                field,
                format!("missing field {}", field)
            )
        }
        RustIrExpr::Index { base, index, .. } => {
            let base_code = emit_expr(base, indent)?;
            let index_code = emit_expr(index, indent)?;
            format!(
                "({base_code}).and_then(|b| ({index_code}).and_then(|i| match (b, i) {{ (Value::List(items), Value::Int(idx)) => items.get(idx as usize).cloned().ok_or_else(|| RuntimeError::Message(\"index out of bounds\".to_string())), (Value::Tuple(items), Value::Int(idx)) => items.get(idx as usize).cloned().ok_or_else(|| RuntimeError::Message(\"index out of bounds\".to_string())), (Value::Map(entries), idx) => {{ let Some(key) = KeyValue::try_from_value(&idx) else {{ return Err(RuntimeError::Message(format!(\"map key is not a valid key type: {{}}\", aivi_native_runtime::format_value(&idx)))); }}; entries.get(&key).cloned().ok_or_else(|| RuntimeError::Message(\"missing map key\".to_string())) }}, (other, _) => Err(RuntimeError::Message(format!(\"index on unsupported value {{}}\", aivi_native_runtime::format_value(&other)))), }}))"
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
        } => emit_block(block_kind.clone(), items, indent)?,
        RustIrExpr::Raw { text, .. } => {
            format!("aivi_ok(Value::Text({text:?}.to_string()))")
        }
        RustIrExpr::Match {
            scrutinee, arms, ..
        } => emit_match(scrutinee, arms, indent)?,
    })
}

fn emit_i18n_message_parts(parts: &[MessagePart], indent: usize) -> String {
    let ind = "    ".repeat(indent);
    let ind2 = "    ".repeat(indent + 1);
    let ind3 = "    ".repeat(indent + 2);

    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&ind2);
    out.push_str("let mut items: Vec<Value> = Vec::new();\n");
    for part in parts {
        match part {
            MessagePart::Lit(text) => {
                out.push_str(&ind2);
                out.push_str("items.push(Value::Record(Arc::new(HashMap::from([\n");
                out.push_str(&ind3);
                out.push_str(&format!(
                    "(\"kind\".to_string(), Value::Text({:?}.to_string())),\n",
                    "lit"
                ));
                out.push_str(&ind3);
                out.push_str(&format!(
                    "(\"text\".to_string(), Value::Text({text:?}.to_string())),\n"
                ));
                out.push_str(&ind2);
                out.push_str("]))));\n");
            }
            MessagePart::Hole { name, ty } => {
                let ty_code = match ty {
                    Some(t) => format!(
                        "Value::Constructor {{ name: \"Some\".to_string(), args: vec![Value::Text({t:?}.to_string())] }}"
                    ),
                    None => "Value::Constructor { name: \"None\".to_string(), args: Vec::new() }"
                        .to_string(),
                };
                out.push_str(&ind2);
                out.push_str("items.push(Value::Record(Arc::new(HashMap::from([\n");
                out.push_str(&ind3);
                out.push_str(&format!(
                    "(\"kind\".to_string(), Value::Text({:?}.to_string())),\n",
                    "hole"
                ));
                out.push_str(&ind3);
                out.push_str(&format!(
                    "(\"name\".to_string(), Value::Text({name:?}.to_string())),\n"
                ));
                out.push_str(&ind3);
                out.push_str(&format!("(\"ty\".to_string(), {ty_code}),\n"));
                out.push_str(&ind2);
                out.push_str("]))));\n");
            }
        }
    }
    out.push_str(&ind2);
    out.push_str("Value::List(Arc::new(items))\n");
    out.push_str(&ind);
    out.push('}');
    out
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
        if field.path.is_empty() {
            return Err(AiviError::Codegen(
                "record field path must not be empty".to_string(),
            ));
        }
        let value_code = emit_expr(&field.value, indent)?;
        let has_index_segment = field
            .path
            .iter()
            .any(|segment| !matches!(segment, RustIrPathSegment::Field(_)));
        if has_index_segment {
            let path_code = emit_path(&field.path, indent)?;
            stmts.push(format!(
                "map = match patch(rt, Value::Record(Arc::new(map)), vec![({path_code}, ({value_code})?)])? {{ Value::Record(m) => m.as_ref().clone(), other => return Err(RuntimeError::Message(format!(\"record literal patch expected Record result, got {{}}\", aivi_native_runtime::format_value(&other)))), }};"
            ));
            continue;
        }
        let field_names: Vec<&str> = field
            .path
            .iter()
            .map(|seg| match seg {
                RustIrPathSegment::Field(name) => name.as_str(),
                _ => unreachable!("non-field segments handled above"),
            })
            .collect();
        if field_names.len() == 1 {
            // Simple case: flat field insertion.
            stmts.push(format!(
                "map.insert({:?}.to_string(), ({value_code})?);",
                field_names[0]
            ));
        } else {
            // Nested record path: e.g. `person.name = "Alice"` creates intermediate records.
            stmts.push(emit_nested_record_insert(&field_names, &value_code));
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

/// Emit Rust code that inserts a value into a nested record path.
///
/// For `["person", "address", "city"]`, generates code that:
/// 1. Navigates/creates `map["person"]["address"]` as nested Records
/// 2. Inserts the value at the leaf key `"city"`
fn emit_nested_record_insert(field_names: &[&str], value_code: &str) -> String {
    // Build a nested insertion using a helper closure. The generated code:
    // { let __val = (VALUE)?;
    //   let mut __cur = &mut map;
    //   // for each intermediate segment, ensure a Record entry exists:
    //   __cur = Arc::make_mut(__cur.entry("a").or_insert_with(|| Value::Record(Arc::new(HashMap::new())))...);
    //   // insert at leaf
    // }
    let mut s = String::new();
    s.push_str(&format!("{{ let __val = ({value_code})?; "));

    // Navigate intermediates
    let (intermediates, leaf) = field_names.split_at(field_names.len() - 1);
    let leaf = leaf[0];

    // We need to drill into nested records. Each intermediate gets or creates a Record entry.
    // We operate on the top-level `map` directly.
    let mut depth = 0;
    for name in intermediates {
        let var = if depth == 0 {
            "map".to_string()
        } else {
            format!("__nested_{}", depth - 1)
        };
        s.push_str(&format!(
            "let __entry_{depth} = {var}.entry({name:?}.to_string()).or_insert_with(|| Value::Record(Arc::new(HashMap::new()))); "
        ));
        s.push_str(&format!(
            "let __nested_{depth} = match __entry_{depth} {{ Value::Record(ref mut m) => Arc::make_mut(m), _ => return Err(RuntimeError::Message(format!(\"record path conflict at {name}\"))), }}; "
        ));
        depth += 1;
    }

    let final_var = if depth == 0 {
        "map".to_string()
    } else {
        format!("__nested_{}", depth - 1)
    };
    s.push_str(&format!(
        "{final_var}.insert({leaf:?}.to_string(), __val); }}"
    ));
    s
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
            RustIrPathSegment::IndexAll => {
                out.push_str("PathSeg::IndexAll");
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
        "+" | "-" | "*" | "/" | "%" => {
            let template = r#"({LEFT}).and_then(|l| ({RIGHT}).and_then(|r| match (l, r) {
        (Value::Int(a), Value::Int(b)) => aivi_ok(Value::Int(a <OP> b)),
        (Value::BigInt(a), Value::BigInt(b)) => aivi_ok(Value::BigInt(Arc::new(&*a <OP> &*b))),
        (Value::Decimal(a), Value::Decimal(b)) => aivi_ok(Value::Decimal(a <OP> b)),
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
        (Value::BigInt(a), Value::BigInt(b)) => aivi_ok(Value::Bool(&*a <OP> &*b)),
        (Value::Decimal(a), Value::Decimal(b)) => aivi_ok(Value::Bool(a <OP> b)),
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
