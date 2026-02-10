use crate::rust_ir::{
    Builtin, RustIrBlockItem, RustIrBlockKind, RustIrDef, RustIrExpr, RustIrModule,
    RustIrPathSegment, RustIrProgram, RustIrRecordField,
};
use crate::{kernel, rust_ir, AiviError, HirProgram};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn build_with_rustc(
    program: HirProgram,
    out: &Path,
    rustc_args: &[String],
) -> Result<(), AiviError> {
    let kernel = kernel::lower_hir(program);
    let rust_ir = rust_ir::lower_kernel(kernel)?;
    let source = emit_rustc_source(rust_ir)?;

    let mut hasher = Sha256::new();
    hasher.update(source.as_bytes());
    let hash = hex_lower(&hasher.finalize());

    let gen_dir = PathBuf::from("target/aivi-rustc").join(&hash);
    let src_path = gen_dir.join("main.rs");
    std::fs::create_dir_all(&gen_dir)?;
    std::fs::write(&src_path, source)?;

    if let Some(parent) = out.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }

    let mut cmd = Command::new("rustc");
    cmd.arg(&src_path);
    cmd.arg("--edition=2021");
    cmd.arg("--crate-type=bin");
    cmd.arg("-o");
    cmd.arg(out);
    cmd.args(rustc_args);
    let status = cmd.status()?;
    if !status.success() {
        return Err(AiviError::Codegen("rustc failed".to_string()));
    }
    Ok(())
}

pub fn emit_rustc_source(program: RustIrProgram) -> Result<String, AiviError> {
    if program.modules.len() != 1 {
        return Err(AiviError::Codegen(
            "rustc backend currently supports a single module".to_string(),
        ));
    }
    let module = program.modules.into_iter().next().unwrap();
    emit_module(module)
}

fn emit_module(module: RustIrModule) -> Result<String, AiviError> {
    if !module.defs.iter().any(|d| d.name == "main") {
        return Err(AiviError::Codegen(
            "rustc backend expects a main definition".to_string(),
        ));
    }
    let mut out = String::new();
    out.push_str("use std::collections::HashMap;\n");
    out.push_str("use std::rc::Rc;\n");
    out.push_str("use std::io::Write;\n\n");
    out.push_str("#[derive(Clone)]\n");
    out.push_str("enum Value {\n");
    out.push_str("    Unit,\n");
    out.push_str("    Bool(bool),\n");
    out.push_str("    Int(i64),\n");
    out.push_str("    Float(f64),\n");
    out.push_str("    Text(String),\n");
    out.push_str("    DateTime(String),\n");
    out.push_str("    List(Vec<Value>),\n");
    out.push_str("    Tuple(Vec<Value>),\n");
    out.push_str("    Record(HashMap<String, Value>),\n");
    out.push_str("    Closure(Rc<dyn Fn(Value) -> Result<Value, String>>),\n");
    out.push_str("    Effect(Rc<dyn Fn() -> Result<Value, String>>),\n");
    out.push_str("}\n\n");

    out.push_str("fn format_value(value: &Value) -> String {\n");
    out.push_str("    match value {\n");
    out.push_str("        Value::Unit => \"Unit\".to_string(),\n");
    out.push_str("        Value::Bool(v) => v.to_string(),\n");
    out.push_str("        Value::Int(v) => v.to_string(),\n");
    out.push_str("        Value::Float(v) => v.to_string(),\n");
    out.push_str("        Value::Text(v) => v.clone(),\n");
    out.push_str("        Value::DateTime(v) => v.clone(),\n");
    out.push_str("        Value::List(items) => {\n");
    out.push_str("            let inner = items.iter().map(format_value).collect::<Vec<_>>().join(\", \");\n");
    out.push_str("            format!(\"[{}]\", inner)\n");
    out.push_str("        }\n");
    out.push_str("        Value::Tuple(items) => {\n");
    out.push_str("            let inner = items.iter().map(format_value).collect::<Vec<_>>().join(\", \");\n");
    out.push_str("            format!(\"({})\", inner)\n");
    out.push_str("        }\n");
    out.push_str("        Value::Record(map) => {\n");
    out.push_str("            let mut keys = map.keys().cloned().collect::<Vec<_>>();\n");
    out.push_str("            keys.sort();\n");
    out.push_str("            let inner = keys\n");
    out.push_str("                .into_iter()\n");
    out.push_str("                .map(|k| format!(\"{}: {}\", k, format_value(&map[&k])))\n");
    out.push_str("                .collect::<Vec<_>>()\n");
    out.push_str("                .join(\", \");\n");
    out.push_str("            format!(\"{{{}}}\", inner)\n");
    out.push_str("        }\n");
    out.push_str("        Value::Closure(_) => \"<closure>\".to_string(),\n");
    out.push_str("        Value::Effect(_) => \"<effect>\".to_string(),\n");
    out.push_str("    }\n");
    out.push_str("}\n\n");

    out.push_str("fn apply(func: Value, arg: Value) -> Result<Value, String> {\n");
    out.push_str("    match func {\n");
    out.push_str("        Value::Closure(f) => f(arg),\n");
    out.push_str("        _ => Err(\"attempted to call a non-function\".to_string()),\n");
    out.push_str("    }\n");
    out.push_str("}\n\n");

    out.push_str("fn call(func: Value, args: Vec<Value>) -> Result<Value, String> {\n");
    out.push_str("    let mut acc = func;\n");
    out.push_str("    for arg in args {\n");
    out.push_str("        acc = apply(acc, arg)?;\n");
    out.push_str("    }\n");
    out.push_str("    Ok(acc)\n");
    out.push_str("}\n\n");

    out.push_str("fn run_effect(effect: Value) -> Result<Value, String> {\n");
    out.push_str("    match effect {\n");
    out.push_str("        Value::Effect(f) => f(),\n");
    out.push_str(
        "        other => Err(format!(\"expected Effect, got {}\", format_value(&other))),\n",
    );
    out.push_str("    }\n");
    out.push_str("}\n\n");

    out.push_str("fn builtin_pure(arg: Value) -> Value {\n");
    out.push_str("    Value::Effect(Rc::new(move || Ok(arg.clone())))\n");
    out.push_str("}\n\n");

    out.push_str("fn builtin_bind(effect: Value, func: Value) -> Result<Value, String> {\n");
    out.push_str("    Ok(Value::Effect(Rc::new(move || {\n");
    out.push_str("        let value = run_effect(effect.clone())?;\n");
    out.push_str("        let applied = apply(func.clone(), value)?;\n");
    out.push_str("        run_effect(applied)\n");
    out.push_str("    })))\n");
    out.push_str("}\n\n");

    out.push_str("fn builtin_print(value: Value) -> Value {\n");
    out.push_str("    Value::Effect(Rc::new(move || {\n");
    out.push_str("        let text = format_value(&value);\n");
    out.push_str("        print!(\"{}\", text);\n");
    out.push_str("        let mut out = std::io::stdout();\n");
    out.push_str("        let _ = out.flush();\n");
    out.push_str("        Ok(Value::Unit)\n");
    out.push_str("    }))\n");
    out.push_str("}\n\n");

    out.push_str("#[derive(Clone)]\n");
    out.push_str("enum PathSeg {\n");
    out.push_str("    Field(String),\n");
    out.push_str("    IndexValue(Value),\n");
    out.push_str("    IndexFieldBool(String),\n");
    out.push_str("}\n\n");

    out.push_str("fn patch_apply(old: Value, updater: Value) -> Result<Value, String> {\n");
    out.push_str("    match updater {\n");
    out.push_str("        Value::Closure(_) => apply(updater, old),\n");
    out.push_str("        other => Ok(other),\n");
    out.push_str("    }\n");
    out.push_str("}\n\n");

    out.push_str("fn patch_path(target: Value, path: &[PathSeg], updater: Value) -> Result<Value, String> {\n");
    out.push_str("    if path.is_empty() {\n");
    out.push_str("        return patch_apply(target, updater);\n");
    out.push_str("    }\n");
    out.push_str("    match &path[0] {\n");
    out.push_str("        PathSeg::Field(name) => match target {\n");
    out.push_str("            Value::Record(mut map) => {\n");
    out.push_str("                let old = map.remove(name).unwrap_or(Value::Unit);\n");
    out.push_str("                let new_val = patch_path(old, &path[1..], updater)?;\n");
    out.push_str("                map.insert(name.clone(), new_val);\n");
    out.push_str("                Ok(Value::Record(map))\n");
    out.push_str("            }\n");
    out.push_str("            other => Err(format!(\"expected Record for field patch, got {}\", format_value(&other))),\n");
    out.push_str("        },\n");
    out.push_str("        PathSeg::IndexValue(idx) => match (target, idx.clone()) {\n");
    out.push_str("            (Value::List(mut items), Value::Int(i)) => {\n");
    out.push_str("                let i = i as usize;\n");
    out.push_str("                if i >= items.len() { return Err(\"index out of bounds\".to_string()); }\n");
    out.push_str("                let old = items[i].clone();\n");
    out.push_str("                items[i] = patch_path(old, &path[1..], updater)?;\n");
    out.push_str("                Ok(Value::List(items))\n");
    out.push_str("            }\n");
    out.push_str("            (other, _) => Err(format!(\"expected List/Int for index patch, got {}\", format_value(&other))),\n");
    out.push_str("        },\n");
    out.push_str("        PathSeg::IndexFieldBool(field) => match target {\n");
    out.push_str("            Value::List(items) => {\n");
    out.push_str("                let mut out_items = Vec::with_capacity(items.len());\n");
    out.push_str("                for item in items {\n");
    out.push_str("                    let should_patch = match &item {\n");
    out.push_str("                        Value::Record(map) => matches!(map.get(field), Some(Value::Bool(true))),\n");
    out.push_str("                        _ => false,\n");
    out.push_str("                    };\n");
    out.push_str("                    if should_patch {\n");
    out.push_str(
        "                        out_items.push(patch_path(item, &path[1..], updater.clone())?);\n",
    );
    out.push_str("                    } else {\n");
    out.push_str("                        out_items.push(item);\n");
    out.push_str("                    }\n");
    out.push_str("                }\n");
    out.push_str("                Ok(Value::List(out_items))\n");
    out.push_str("            }\n");
    out.push_str("            other => Err(format!(\"expected List for traversal patch, got {}\", format_value(&other))),\n");
    out.push_str("        },\n");
    out.push_str("    }\n");
    out.push_str("}\n\n");

    out.push_str(
        "fn patch(target: Value, fields: Vec<(Vec<PathSeg>, Value)>) -> Result<Value, String> {\n",
    );
    out.push_str("    let mut acc = target;\n");
    out.push_str("    for (path, updater) in fields {\n");
    out.push_str("        acc = patch_path(acc, &path, updater)?;\n");
    out.push_str("    }\n");
    out.push_str("    Ok(acc)\n");
    out.push_str("}\n\n");

    for def in &module.defs {
        out.push_str(&emit_def_sig(def));
        out.push_str("{\n");
        out.push_str("    ");
        out.push_str(&emit_expr(&def.expr, 1)?);
        out.push_str("\n}\n\n");
    }

    let main_fn = rust_global_fn_name("main");
    out.push_str("fn main() {\n");
    out.push_str(&format!(
        "    let result = {}().and_then(run_effect);\n",
        main_fn
    ));
    out.push_str("    match result {\n");
    out.push_str("        Ok(_) => {}\n");
    out.push_str("        Err(err) => {\n");
    out.push_str("            eprintln!(\"{err}\");\n");
    out.push_str("            std::process::exit(1);\n");
    out.push_str("        }\n");
    out.push_str("    }\n");
    out.push_str("}\n");

    Ok(out)
}

fn emit_def_sig(def: &RustIrDef) -> String {
    format!(
        "fn {}() -> Result<Value, String> ",
        rust_global_fn_name(&def.name)
    )
}

fn emit_expr(expr: &RustIrExpr, indent: usize) -> Result<String, AiviError> {
    Ok(match expr {
        RustIrExpr::Local { name, .. } => format!("Ok({})", rust_local_name(name)),
        RustIrExpr::Global { name, .. } => format!("{}()", rust_global_fn_name(name)),
        RustIrExpr::Builtin { builtin, .. } => {
            return Err(AiviError::Codegen(format!(
                "builtin {builtin:?} used as a value is not supported by the rustc backend yet"
            )))
        }

        RustIrExpr::LitNumber { text, .. } => {
            if let Ok(value) = text.parse::<i64>() {
                format!("Ok(Value::Int({value}))")
            } else if let Ok(value) = text.parse::<f64>() {
                format!("Ok(Value::Float({value}))")
            } else {
                return Err(AiviError::Codegen(format!(
                    "unsupported numeric literal {text}"
                )));
            }
        }
        RustIrExpr::LitString { text, .. } => format!("Ok(Value::Text({:?}.to_string()))", text),
        RustIrExpr::LitSigil {
            tag, body, flags, ..
        } => {
            let ind = "    ".repeat(indent);
            let ind2 = "    ".repeat(indent + 1);
            let ind3 = "    ".repeat(indent + 2);
            format!(
                "{{\n{ind2}let mut map = HashMap::new();\n{ind3}map.insert(\"tag\".to_string(), Value::Text({tag:?}.to_string()));\n{ind3}map.insert(\"body\".to_string(), Value::Text({body:?}.to_string()));\n{ind3}map.insert(\"flags\".to_string(), Value::Text({flags:?}.to_string()));\n{ind2}Ok(Value::Record(map))\n{ind}}}"
            )
        }
        RustIrExpr::LitBool { value, .. } => format!("Ok(Value::Bool({value}))"),
        RustIrExpr::LitDateTime { text, .. } => {
            format!("Ok(Value::DateTime({:?}.to_string()))", text)
        }

        RustIrExpr::Lambda { param, body, .. } => {
            let param_name = rust_local_name(param);
            let body_code = emit_expr(body, indent + 1)?;
            let ind = "    ".repeat(indent);
            let ind2 = "    ".repeat(indent + 1);
            format!(
                "Ok(Value::Closure(Rc::new(move |{param_name}: Value| {{\n{ind2}{body_code}\n{ind}}})))"
            )
        }
        RustIrExpr::App { func, arg, .. } => {
            let func_code = emit_expr(func, indent)?;
            let arg_code = emit_expr(arg, indent)?;
            format!("({func_code}).and_then(|f| ({arg_code}).and_then(|a| apply(f, a)))")
        }
        RustIrExpr::Call { func, args, .. } => {
            if let RustIrExpr::Builtin { builtin, .. } = func.as_ref() {
                return emit_builtin_call(*builtin, args, indent);
            }
            let func_code = emit_expr(func, indent)?;
            let mut rendered_args = Vec::new();
            for arg in args {
                rendered_args.push(emit_expr(arg, indent)?);
            }
            let args_code = rendered_args
                .into_iter()
                .map(|a| format!("({a})?"))
                .collect::<Vec<_>>()
                .join(", ");
            format!("({func_code}).and_then(|f| call(f, vec![{args_code}]))")
        }
        RustIrExpr::List { items, .. } => {
            let mut parts = Vec::new();
            for item in items {
                let expr_code = emit_expr(&item.expr, indent)?;
                if item.spread {
                    parts.push(format!(
                        "{{ let v = ({expr_code})?; match v {{ Value::List(xs) => xs, other => return Err(format!(\"expected List for spread, got {{}}\", format_value(&other))), }} }}"
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
            format!("Ok(Value::List({concat}))")
        }
        RustIrExpr::Tuple { items, .. } => {
            let mut rendered = Vec::new();
            for item in items {
                rendered.push(format!("({})?", emit_expr(item, indent)?));
            }
            format!("Ok(Value::Tuple(vec![{}]))", rendered.join(", "))
        }
        RustIrExpr::Record { fields, .. } => emit_record(fields, indent)?,
        RustIrExpr::Patch { target, fields, .. } => {
            let target_code = emit_expr(target, indent)?;
            let fields_code = emit_patch_fields(fields, indent)?;
            format!("({target_code}).and_then(|t| patch(t, {fields_code}))")
        }
        RustIrExpr::FieldAccess { base, field, .. } => {
            let base_code = emit_expr(base, indent)?;
            format!(
                "({base_code}).and_then(|b| match b {{ Value::Record(map) => map.get({:?}).cloned().ok_or_else(|| \"missing field\".to_string()), other => Err(format!(\"expected Record, got {{}}\", format_value(&other))), }})",
                field
            )
        }
        RustIrExpr::Index { base, index, .. } => {
            let base_code = emit_expr(base, indent)?;
            let index_code = emit_expr(index, indent)?;
            format!(
                "({base_code}).and_then(|b| ({index_code}).and_then(|i| match (b, i) {{ (Value::List(items), Value::Int(idx)) => items.get(idx as usize).cloned().ok_or_else(|| \"index out of bounds\".to_string()), (Value::Tuple(items), Value::Int(idx)) => items.get(idx as usize).cloned().ok_or_else(|| \"index out of bounds\".to_string()), (other, _) => Err(format!(\"index on unsupported value {{}}\", format_value(&other))), }}))"
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
                "({cond_code}).and_then(|c| match c {{ Value::Bool(true) => {then_code}, Value::Bool(false) => {else_code}, other => Err(format!(\"expected Bool, got {{}}\", format_value(&other))), }})"
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
                "raw expressions are not supported by the rustc backend yet: {text}"
            )))
        }
        RustIrExpr::Match { .. } => unreachable!("match lowering should have failed earlier"),
    })
}

fn emit_builtin_call(
    builtin: Builtin,
    args: &[RustIrExpr],
    indent: usize,
) -> Result<String, AiviError> {
    match builtin {
        Builtin::Pure => {
            if args.len() != 1 {
                return Err(AiviError::Codegen("pure expects 1 arg".to_string()));
            }
            let arg_code = emit_expr(&args[0], indent)?;
            Ok(format!("({arg_code}).map(builtin_pure)"))
        }
        Builtin::Print => {
            if args.len() != 1 {
                return Err(AiviError::Codegen("print expects 1 arg".to_string()));
            }
            let arg_code = emit_expr(&args[0], indent)?;
            Ok(format!("({arg_code}).map(builtin_print)"))
        }
        Builtin::Bind => {
            if args.len() != 2 {
                return Err(AiviError::Codegen("bind expects 2 args".to_string()));
            }
            let eff_code = emit_expr(&args[0], indent)?;
            let func_code = emit_expr(&args[1], indent)?;
            Ok(format!(
                "({eff_code}).and_then(|e| ({func_code}).and_then(|f| builtin_bind(e, f)))"
            ))
        }
    }
}

fn emit_record(fields: &[RustIrRecordField], indent: usize) -> Result<String, AiviError> {
    let mut assigns = Vec::new();
    for field in fields {
        if field.path.len() != 1 {
            return Err(AiviError::Codegen(
                "nested record paths are not supported in record literals yet".to_string(),
            ));
        }
        match &field.path[0] {
            RustIrPathSegment::Field(name) => {
                let value_code = emit_expr(&field.value, indent)?;
                assigns.push(format!(
                    "map.insert({:?}.to_string(), ({value_code})?);",
                    name
                ));
            }
            _ => {
                return Err(AiviError::Codegen(
                    "index segments are not supported in record literals".to_string(),
                ))
            }
        }
    }
    let mut s = String::new();
    s.push_str("Ok({ let mut map = HashMap::new(); ");
    for assign in assigns {
        s.push_str(&assign);
        s.push(' ');
    }
    s.push_str("Value::Record(map) })");
    Ok(s)
}

fn emit_patch_fields(fields: &[RustIrRecordField], indent: usize) -> Result<String, AiviError> {
    let mut out = String::new();
    out.push_str("vec![");
    for (i, field) in fields.iter().enumerate() {
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

fn emit_binary(op: &str, left: String, right: String) -> String {
    match op {
        "+" | "-" | "*" | "/" => {
            let template = r#"({LEFT}).and_then(|l| ({RIGHT}).and_then(|r| match (l, r) {
        (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a <OP> b)),
        (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a <OP> b)),
        (Value::Int(a), Value::Float(b)) => Ok(Value::Float((a as f64) <OP> b)),
        (Value::Float(a), Value::Int(b)) => Ok(Value::Float(a <OP> (b as f64))),
        _ => Err("unsupported operands for binary op".to_string()),
    }))"#;
            template
                .replace("{LEFT}", &left)
                .replace("{RIGHT}", &right)
                .replace("<OP>", op)
        }
        _ => format!(
            "({left}).and_then(|_| ({right}).and_then(|_| Err(\"unsupported binary op\".to_string())))"
        ),
    }
}

fn emit_block(
    kind: RustIrBlockKind,
    items: &[RustIrBlockItem],
    indent: usize,
) -> Result<String, AiviError> {
    match kind {
        RustIrBlockKind::Plain => emit_plain_block(items, indent),
        RustIrBlockKind::Effect => emit_effect_block(items, indent),
    }
}

fn emit_plain_block(items: &[RustIrBlockItem], indent: usize) -> Result<String, AiviError> {
    let ind = "    ".repeat(indent);
    let ind2 = "    ".repeat(indent + 1);
    let mut s = String::new();
    s.push_str("Ok({\n");
    if items.is_empty() {
        s.push_str(&ind2);
        s.push_str("Value::Unit\n");
        s.push_str(&ind);
        s.push_str("})");
        return Ok(s);
    }

    for item in &items[..items.len() - 1] {
        match item {
            RustIrBlockItem::Bind { pattern, expr } => match pattern {
                crate::rust_ir::RustIrPattern::Wildcard { .. } => {
                    s.push_str(&ind2);
                    s.push_str(&format!("let _ = ({} )?;\n", emit_expr(expr, indent + 1)?));
                }
                crate::rust_ir::RustIrPattern::Var { name, .. } => {
                    s.push_str(&ind2);
                    s.push_str(&format!(
                        "let {} = ({} )?;\n",
                        rust_local_name(name),
                        emit_expr(expr, indent + 1)?
                    ));
                }
            },
            RustIrBlockItem::Expr { expr } => {
                s.push_str(&ind2);
                s.push_str(&format!("let _ = ({} )?;\n", emit_expr(expr, indent + 1)?));
            }
        }
    }

    match items.last().unwrap() {
        RustIrBlockItem::Bind { pattern, expr } => {
            match pattern {
                crate::rust_ir::RustIrPattern::Wildcard { .. } => {
                    s.push_str(&ind2);
                    s.push_str(&format!("let _ = ({} )?;\n", emit_expr(expr, indent + 1)?));
                }
                crate::rust_ir::RustIrPattern::Var { name, .. } => {
                    s.push_str(&ind2);
                    s.push_str(&format!(
                        "let {} = ({} )?;\n",
                        rust_local_name(name),
                        emit_expr(expr, indent + 1)?
                    ));
                }
            }
            s.push_str(&ind2);
            s.push_str("Value::Unit\n");
        }
        RustIrBlockItem::Expr { expr } => {
            s.push_str(&ind2);
            s.push_str(&format!("({} )?\n", emit_expr(expr, indent + 1)?));
        }
    }

    s.push_str(&ind);
    s.push_str("})");
    Ok(s)
}

fn emit_effect_block(items: &[RustIrBlockItem], indent: usize) -> Result<String, AiviError> {
    let ind2 = "    ".repeat(indent + 1);
    let ind3 = "    ".repeat(indent + 2);
    let mut s = String::new();
    s.push_str("Ok(Value::Effect(Rc::new(move || {\n");

    for (idx, item) in items.iter().enumerate() {
        let last = idx + 1 == items.len();
        match item {
            RustIrBlockItem::Bind { pattern, expr } => {
                let expr_code = emit_expr(expr, indent + 2)?;
                match pattern {
                    crate::rust_ir::RustIrPattern::Wildcard { .. } => {
                        s.push_str(&ind3);
                        s.push_str(&format!("let _ = run_effect(({} )?)?;\n", expr_code));
                    }
                    crate::rust_ir::RustIrPattern::Var { name, .. } => {
                        s.push_str(&ind3);
                        s.push_str(&format!(
                            "let {} = run_effect(({} )?)?;\n",
                            rust_local_name(name),
                            expr_code
                        ));
                    }
                }
                if last {
                    s.push_str(&ind3);
                    s.push_str("Ok(Value::Unit)\n");
                }
            }
            RustIrBlockItem::Expr { expr } => {
                let expr_code = emit_expr(expr, indent + 2)?;
                if last {
                    s.push_str(&ind3);
                    s.push_str(&format!("run_effect(({} )?)\n", expr_code));
                } else {
                    s.push_str(&ind3);
                    s.push_str(&format!("let _ = run_effect(({} )?)?;\n", expr_code));
                }
            }
        }
    }

    if items.is_empty() {
        s.push_str(&ind3);
        s.push_str("Ok(Value::Unit)\n");
    }

    s.push_str(&ind2);
    s.push_str("})))");
    Ok(s)
}

fn rust_local_name(name: &str) -> String {
    let mut s = sanitize_ident(name);
    if s.is_empty() {
        s = "_".to_string();
    }
    if is_rust_keyword(&s) {
        s = format!("v_{s}");
    }
    s
}

fn rust_global_fn_name(name: &str) -> String {
    format!("def_{}", rust_local_name(name))
}

fn sanitize_ident(name: &str) -> String {
    let mut out = String::new();
    for (i, ch) in name.chars().enumerate() {
        let ok = ch == '_' || ch.is_ascii_alphanumeric();
        if ok {
            if i == 0 && ch.is_ascii_digit() {
                out.push('_');
            }
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    out
}

fn is_rust_keyword(ident: &str) -> bool {
    matches!(
        ident,
        "as" | "break"
            | "const"
            | "continue"
            | "crate"
            | "else"
            | "enum"
            | "extern"
            | "false"
            | "fn"
            | "for"
            | "if"
            | "impl"
            | "in"
            | "let"
            | "loop"
            | "match"
            | "mod"
            | "move"
            | "mut"
            | "pub"
            | "ref"
            | "return"
            | "self"
            | "Self"
            | "static"
            | "struct"
            | "super"
            | "trait"
            | "true"
            | "type"
            | "unsafe"
            | "use"
            | "where"
            | "while"
    )
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{:02x}", b));
    }
    out
}
