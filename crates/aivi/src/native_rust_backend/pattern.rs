use crate::rust_ir::{RustIrExpr, RustIrMatchArm, RustIrPattern};
use crate::AiviError;

use super::expr::emit_expr;
use super::utils::{collect_pattern_vars, rust_local_name};

pub(super) fn emit_match(
    scrutinee: &RustIrExpr,
    arms: &[RustIrMatchArm],
    indent: usize,
) -> Result<String, AiviError> {
    let scrut_code = emit_expr(scrutinee, indent)?;
    let ind = "    ".repeat(indent);
    let ind2 = "    ".repeat(indent + 1);
    let ind3 = "    ".repeat(indent + 2);
    let ind4 = "    ".repeat(indent + 3);

    let mut s = String::new();
    s.push_str(&format!("({scrut_code}).and_then(|__scrut| {{\n"));

    for (arm_index, arm) in arms.iter().enumerate() {
        let fn_name = format!("__match_arm_{arm_index}");
        s.push_str(&ind2);
        s.push_str(&format!(
            "fn {fn_name}(v: &Value, b: &mut HashMap<&'static str, Value>) -> bool {{\n"
        ));
        s.push_str(&emit_pattern_fn_body(&arm.pattern, "v", "b", indent + 2)?);
        s.push_str(&ind2);
        s.push_str("}\n\n");

        s.push_str(&ind2);
        s.push_str("{\n");
        s.push_str(&ind3);
        s.push_str("let mut __b: HashMap<&'static str, Value> = HashMap::new();\n");
        s.push_str(&ind3);
        s.push_str(&format!("if {fn_name}(&__scrut, &mut __b) {{\n"));

        let mut vars = Vec::new();
        collect_pattern_vars(&arm.pattern, &mut vars);
        vars.sort();
        vars.dedup();
        for var in vars {
            let rust_name = rust_local_name(&var);
            s.push_str(&ind4);
            s.push_str(&format!(
                "let {rust_name} = __b.remove({var:?}).expect(\"pattern binder\");\n"
            ));
        }

        if let Some(guard) = &arm.guard {
            let guard_code = emit_expr(guard, indent + 3)?;
            s.push_str(&ind4);
            s.push_str(&format!("let __g = ({guard_code})?;\n"));
            s.push_str(&ind4);
            s.push_str("if matches!(__g, Value::Bool(true)) {\n");
            let body_code = emit_expr(&arm.body, indent + 4)?;
            s.push_str(&"    ".repeat(indent + 4));
            s.push_str(&format!("return {body_code};\n"));
            s.push_str(&ind4);
            s.push_str("}\n");
        } else {
            let body_code = emit_expr(&arm.body, indent + 3)?;
            s.push_str(&ind4);
            s.push_str(&format!("return {body_code};\n"));
        }

        s.push_str(&ind3);
        s.push_str("}\n");
        s.push_str(&ind2);
        s.push_str("}\n\n");
    }

    s.push_str(&ind2);
    s.push_str("Err(RuntimeError::Message(\"non-exhaustive match\".to_string()))\n");
    s.push_str(&ind);
    s.push_str("})");
    Ok(s)
}

fn emit_pattern_fn_body(
    pattern: &RustIrPattern,
    value_ident: &str,
    bindings_ident: &str,
    indent: usize,
) -> Result<String, AiviError> {
    let ind = "    ".repeat(indent);
    let ind2 = "    ".repeat(indent + 1);
    let mut s = String::new();
    s.push_str(&ind);
    s.push_str("{\n");
    s.push_str(&ind2);
    s.push_str("use Value::*;\n");
    s.push_str(&ind2);
    s.push_str(&emit_pattern_check(
        pattern,
        value_ident,
        bindings_ident,
        indent + 1,
    )?);
    s.push('\n');
    s.push_str(&ind);
    s.push_str("}\n");
    Ok(s)
}

fn emit_pattern_check(
    pattern: &RustIrPattern,
    value_ident: &str,
    bindings_ident: &str,
    indent: usize,
) -> Result<String, AiviError> {
    let ind = "    ".repeat(indent);
    Ok(match pattern {
        RustIrPattern::Wildcard { .. } => "true".to_string(),
        RustIrPattern::Var { name, .. } => {
            format!("{{ {bindings_ident}.insert({name:?}, {value_ident}.clone()); true }}")
        }
        RustIrPattern::At { name, pattern, .. } => {
            let check = emit_pattern_check(pattern, value_ident, bindings_ident, indent + 1)?;
            format!("{{ {bindings_ident}.insert({name:?}, {value_ident}.clone()); ({check}) }}")
        }
        RustIrPattern::Literal { value, .. } => match value {
            crate::rust_ir::RustIrLiteral::Bool(b) => {
                format!("matches!({value_ident}, Value::Bool(v) if *v == {b})")
            }
            crate::rust_ir::RustIrLiteral::String(text) => {
                format!("matches!({value_ident}, Value::Text(v) if v == {text:?})")
            }
            crate::rust_ir::RustIrLiteral::DateTime(text) => {
                format!("matches!({value_ident}, Value::DateTime(v) if v == {text:?})")
            }
            crate::rust_ir::RustIrLiteral::Number(text) => {
                if let Ok(int) = text.parse::<i64>() {
                    format!(
                        "matches!({value_ident}, Value::Int(v) if *v == {int}) || matches!({value_ident}, Value::Float(v) if *v == ({int} as f64))"
                    )
                } else if let Ok(float) = text.parse::<f64>() {
                    format!("matches!({value_ident}, Value::Float(v) if *v == {float})")
                } else {
                    return Err(AiviError::Codegen(format!(
                        "unsupported numeric literal in pattern: {text}"
                    )));
                }
            }
            crate::rust_ir::RustIrLiteral::Sigil { tag, body, flags } => {
                // Sigils are represented as records today.
                format!(
                    "match {value_ident} {{ Value::Record(map) => {{\n{ind}    matches!(map.get(\"tag\"), Some(Value::Text(v)) if v == {tag:?}) &&\n{ind}    matches!(map.get(\"body\"), Some(Value::Text(v)) if v == {body:?}) &&\n{ind}    matches!(map.get(\"flags\"), Some(Value::Text(v)) if v == {flags:?})\n{ind}}}, _ => false }}",
                )
            }
        },
        RustIrPattern::Constructor { name, args, .. } => {
            let mut inner = String::new();
            inner.push_str(&format!(
                "match {value_ident} {{ Value::Constructor {{ name, args }} if name == {name:?} && args.len() == {} => {{\n",
                args.len()
            ));
            for (i, arg_pat) in args.iter().enumerate() {
                inner.push_str(&format!("{ind}    let v{i} = &args[{i}];\n"));
                let check =
                    emit_pattern_check(arg_pat, &format!("v{i}"), bindings_ident, indent + 1)?;
                inner.push_str(&format!("{ind}    if !({check}) {{ return false; }}\n"));
            }
            inner.push_str(&format!("{ind}    true\n{ind}}}, _ => false }}"));
            inner
        }
        RustIrPattern::Tuple { items, .. } => {
            let mut inner = String::new();
            inner.push_str(&format!(
                "match {value_ident} {{ Value::Tuple(items) if items.len() == {} => {{\n",
                items.len()
            ));
            for (i, item_pat) in items.iter().enumerate() {
                inner.push_str(&format!("{ind}    let v{i} = &items[{i}];\n"));
                let check =
                    emit_pattern_check(item_pat, &format!("v{i}"), bindings_ident, indent + 1)?;
                inner.push_str(&format!("{ind}    if !({check}) {{ return false; }}\n"));
            }
            inner.push_str(&format!("{ind}    true\n{ind}}}, _ => false }}"));
            inner
        }
        RustIrPattern::List { items, rest, .. } => {
            let mut inner = String::new();
            inner.push_str(&format!(
                "match {value_ident} {{ Value::List(items) => {{\n{ind}    let items = items.as_ref();\n{ind}    if items.len() < {} {{ return false; }}\n",
                items.len()
            ));
            for (i, item_pat) in items.iter().enumerate() {
                inner.push_str(&format!("{ind}    let v{i} = &items[{i}];\n"));
                let check =
                    emit_pattern_check(item_pat, &format!("v{i}"), bindings_ident, indent + 1)?;
                inner.push_str(&format!("{ind}    if !({check}) {{ return false; }}\n"));
            }
            if let Some(rest_pat) = rest.as_deref() {
                inner.push_str(&format!(
                    "{ind}    let tail = Value::List(Arc::new(items[{}..].to_vec()));\n",
                    items.len()
                ));
                let check = emit_pattern_check(rest_pat, "(&tail)", bindings_ident, indent + 1)?;
                inner.push_str(&format!("{ind}    {check}\n"));
            } else {
                inner.push_str(&format!("{ind}    items.len() == {}\n", items.len()));
            }
            inner.push_str(&format!("{ind}}}, _ => false }}"));
            inner
        }
        RustIrPattern::Record { fields, .. } => {
            let mut inner = String::new();
            inner.push_str(&format!("match {value_ident} {{ Value::Record(_) => {{\n"));
            for (i, field) in fields.iter().enumerate() {
                let path = &field.path;
                if path.is_empty() {
                    return Err(AiviError::Codegen("empty record pattern path".to_string()));
                }
                inner.push_str(&format!(
                    "{ind}    let mut cur{i}: &Value = {value_ident};\n"
                ));
                for seg in path.iter() {
                    inner.push_str(&format!("{ind}    cur{i} = match cur{i} {{\n"));
                    inner.push_str(&format!(
                        "{ind}        Value::Record(m) => match m.get({seg:?}) {{\n"
                    ));
                    inner.push_str(&format!("{ind}            Some(v) => v,\n"));
                    inner.push_str(&format!("{ind}            None => return false,\n"));
                    inner.push_str(&format!("{ind}        }},\n"));
                    inner.push_str(&format!("{ind}        _ => return false,\n"));
                    inner.push_str(&format!("{ind}    }};\n"));
                }
                let check = emit_pattern_check(
                    &field.pattern,
                    &format!("cur{i}"),
                    bindings_ident,
                    indent + 1,
                )?;
                inner.push_str(&format!("{ind}    if !({check}) {{ return false; }}\n"));
            }
            inner.push_str(&format!("{ind}    true\n{ind}}}, _ => false }}"));
            inner
        }
    })
}

pub(super) fn emit_pattern_bind_stmts(
    pattern: &RustIrPattern,
    value_ident: &str,
    bindings_ident: &str,
    ok_ident: &str,
    indent: usize,
    err_message: &str,
) -> Result<String, AiviError> {
    let ind = "    ".repeat(indent);
    let mut s = String::new();

    s.push_str(&ind);
    s.push_str(&format!(
        "let mut {bindings_ident}: HashMap<&'static str, Value> = HashMap::new();\n"
    ));
    s.push_str(&ind);
    s.push_str(&format!(
        "let {ok_ident} = (|v: &Value, b: &mut HashMap<&'static str, Value>| -> bool {{\n"
    ));
    s.push_str(&emit_pattern_fn_body(pattern, "v", "b", indent + 2)?);
    s.push('\n');
    s.push_str(&ind);
    s.push_str(&format!("}})(&{value_ident}, &mut {bindings_ident});\n"));
    s.push_str(&ind);
    s.push_str(&format!(
        "if !{ok_ident} {{ return Err(RuntimeError::Message({err_message:?}.to_string())); }}\n"
    ));

    let mut vars = Vec::new();
    collect_pattern_vars(pattern, &mut vars);
    vars.sort();
    vars.dedup();
    for var in vars {
        let rust_name = rust_local_name(&var);
        s.push_str(&ind);
        s.push_str(&format!(
            "let {rust_name} = {bindings_ident}.remove({var:?}).expect(\"pattern binder\");\n"
        ));
    }
    Ok(s)
}
