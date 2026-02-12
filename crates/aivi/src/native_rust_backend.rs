use std::collections::{HashMap, HashSet};

use crate::rust_ir::{
    RustIrBlockItem, RustIrBlockKind, RustIrDef, RustIrExpr, RustIrMatchArm, RustIrModule,
    RustIrPathSegment, RustIrPattern, RustIrProgram, RustIrRecordField,
};
use crate::AiviError;

pub fn emit_native_rust_source(program: RustIrProgram) -> Result<String, AiviError> {
    emit_native_rust_source_inner(program, EmitKind::Bin)
}

pub fn emit_native_rust_source_lib(program: RustIrProgram) -> Result<String, AiviError> {
    emit_native_rust_source_inner(program, EmitKind::Lib)
}

#[derive(Clone, Copy)]
enum EmitKind {
    Bin,
    Lib,
}

fn emit_native_rust_source_inner(program: RustIrProgram, kind: EmitKind) -> Result<String, AiviError> {
    let mut modules = program.modules.into_iter();
    let Some(first) = modules.next() else {
        return Err(AiviError::Codegen("no modules to build".to_string()));
    };
    let mut defs = first.defs;
    for module in modules {
        defs.extend(module.defs);
    }
    emit_module(
        RustIrModule {
            name: first.name,
            defs,
        },
        kind,
    )
}

fn emit_module(module: RustIrModule, kind: EmitKind) -> Result<String, AiviError> {
    let public_api = matches!(kind, EmitKind::Lib);
    if matches!(kind, EmitKind::Bin) && !module.defs.iter().any(|d| d.name == "main") {
        return Err(AiviError::Codegen(
            "native backend expects a main definition".to_string(),
        ));
    }

    let def_vis = if public_api { "pub " } else { "" };

    let mut out = String::new();
    out.push_str("use std::collections::HashMap;\n");
    out.push_str("use std::sync::{Arc, Mutex};\n\n");
    out.push_str(
        "use aivi_native_runtime::{get_builtin, ok as aivi_ok, BuiltinImpl, BuiltinValue, EffectValue, ResourceValue, Runtime, RuntimeError, R, Value};\n\n",
    );
    out.push_str("fn __builtin(name: &str) -> Value {\n");
    out.push_str("    get_builtin(name).unwrap_or_else(|| panic!(\"missing builtin {name}\"))\n");
    out.push_str("}\n\n");

    out.push_str("#[derive(Clone)]\n");
    out.push_str("enum PathSeg {\n");
    out.push_str("    Field(String),\n");
    out.push_str("    IndexValue(Value),\n");
    out.push_str("    IndexFieldBool(String),\n");
    out.push_str("    IndexPredicate(Value),\n");
    out.push_str("}\n\n");

    out.push_str("fn patch_apply(rt: &mut Runtime, old: Value, updater: Value) -> R {\n");
    out.push_str("    match updater {\n");
    out.push_str("        Value::Closure(_) | Value::Builtin(_) | Value::MultiClause(_) => rt.apply(updater, old),\n");
    out.push_str("        other => aivi_ok(other),\n");
    out.push_str("    }\n");
    out.push_str("}\n\n");

    out.push_str(
        "fn patch_path(rt: &mut Runtime, target: Value, path: &[PathSeg], updater: Value) -> R {\n",
    );
    out.push_str("    if path.is_empty() {\n");
    out.push_str("        return patch_apply(rt, target, updater);\n");
    out.push_str("    }\n");
    out.push_str("    match &path[0] {\n");
    out.push_str("        PathSeg::Field(name) => match target {\n");
    out.push_str("            Value::Record(map) => {\n");
    out.push_str("                let mut map = map.as_ref().clone();\n");
    out.push_str("                let old = map.remove(name).unwrap_or(Value::Unit);\n");
    out.push_str("                let new_val = patch_path(rt, old, &path[1..], updater)?;\n");
    out.push_str("                map.insert(name.clone(), new_val);\n");
    out.push_str("                aivi_ok(Value::Record(Arc::new(map)))\n");
    out.push_str("            }\n");
    out.push_str(
        "            other => Err(RuntimeError::Message(format!(\"expected Record for field patch, got {}\", aivi_native_runtime::format_value(&other)))),\n",
    );
    out.push_str("        },\n");
    out.push_str("        PathSeg::IndexValue(idx) => match (target, idx.clone()) {\n");
    out.push_str("            (Value::List(items), Value::Int(i)) => {\n");
    out.push_str("                let i = i as usize;\n");
    out.push_str("                if i >= items.len() { return Err(RuntimeError::Message(\"index out of bounds\".to_string())); }\n");
    out.push_str("                let mut out = items.as_ref().clone();\n");
    out.push_str("                let old = out[i].clone();\n");
    out.push_str("                out[i] = patch_path(rt, old, &path[1..], updater)?;\n");
    out.push_str("                aivi_ok(Value::List(Arc::new(out)))\n");
    out.push_str("            }\n");
    out.push_str("            (Value::Tuple(mut items), Value::Int(i)) => {\n");
    out.push_str("                let i = i as usize;\n");
    out.push_str("                if i >= items.len() { return Err(RuntimeError::Message(\"index out of bounds\".to_string())); }\n");
    out.push_str("                let old = items[i].clone();\n");
    out.push_str("                items[i] = patch_path(rt, old, &path[1..], updater)?;\n");
    out.push_str("                aivi_ok(Value::Tuple(items))\n");
    out.push_str("            }\n");
    out.push_str(
        "            (other, _) => Err(RuntimeError::Message(format!(\"expected List/Tuple + Int for index patch, got {}\", aivi_native_runtime::format_value(&other)))),\n",
    );
    out.push_str("        },\n");
    out.push_str("        PathSeg::IndexFieldBool(field) => match target {\n");
    out.push_str("            Value::List(items) => {\n");
    out.push_str("                let mut out_items = Vec::with_capacity(items.len());\n");
    out.push_str("                for item in items.iter().cloned() {\n");
    out.push_str("                    let should_patch = match &item {\n");
    out.push_str("                        Value::Record(map) => matches!(map.get(field), Some(Value::Bool(true))),\n");
    out.push_str("                        _ => false,\n");
    out.push_str("                    };\n");
    out.push_str("                    if should_patch {\n");
    out.push_str("                        out_items.push(patch_path(rt, item, &path[1..], updater.clone())?);\n");
    out.push_str("                    } else {\n");
    out.push_str("                        out_items.push(item);\n");
    out.push_str("                    }\n");
    out.push_str("                }\n");
    out.push_str("                aivi_ok(Value::List(Arc::new(out_items)))\n");
    out.push_str("            }\n");
    out.push_str(
        "            other => Err(RuntimeError::Message(format!(\"expected List for traversal patch, got {}\", aivi_native_runtime::format_value(&other)))),\n",
    );
    out.push_str("        },\n");
    out.push_str("        PathSeg::IndexPredicate(pred) => match target {\n");
    out.push_str("            Value::List(items) => {\n");
    out.push_str("                let mut out_items = Vec::with_capacity(items.len());\n");
    out.push_str("                for item in items.iter().cloned() {\n");
    out.push_str("                    let keep = match rt.apply(pred.clone(), item.clone())? {\n");
    out.push_str("                        Value::Bool(true) => true,\n");
    out.push_str("                        Value::Bool(false) => false,\n");
    out.push_str(
        "                        other => return Err(RuntimeError::Message(format!(\"expected Bool predicate, got {}\", aivi_native_runtime::format_value(&other)))),\n",
    );
    out.push_str("                    };\n");
    out.push_str("                    if keep {\n");
    out.push_str("                        out_items.push(patch_path(rt, item, &path[1..], updater.clone())?);\n");
    out.push_str("                    } else {\n");
    out.push_str("                        out_items.push(item);\n");
    out.push_str("                    }\n");
    out.push_str("                }\n");
    out.push_str("                aivi_ok(Value::List(Arc::new(out_items)))\n");
    out.push_str("            }\n");
    out.push_str(
        "            other => Err(RuntimeError::Message(format!(\"expected List for predicate traversal patch, got {}\", aivi_native_runtime::format_value(&other)))),\n",
    );
    out.push_str("        },\n");
    out.push_str("    }\n");
    out.push_str("}\n\n");

    out.push_str("fn patch(rt: &mut Runtime, target: Value, fields: Vec<(Vec<PathSeg>, Value)>) -> R {\n");
    out.push_str("    let mut acc = target;\n");
    out.push_str("    for (path, updater) in fields {\n");
    out.push_str("        acc = patch_path(rt, acc, &path, updater)?;\n");
    out.push_str("    }\n");
    out.push_str("    aivi_ok(acc)\n");
    out.push_str("}\n\n");

    let mut order: Vec<String> = Vec::new();
    let mut groups: HashMap<String, Vec<&RustIrDef>> = HashMap::new();
    for def in &module.defs {
        let entry = groups.entry(def.name.clone()).or_default();
        if entry.is_empty() {
            order.push(def.name.clone());
        }
        entry.push(def);
    }

    for name in order {
        let defs = groups.get(&name).expect("def group");
        if name == "main" && defs.len() != 1 {
            return Err(AiviError::Codegen(
                "native backend expects exactly one main definition".to_string(),
            ));
        }
        if defs.len() == 1 {
            out.push_str(&format!(
                "{def_vis}fn {}(rt: &mut Runtime) -> R {{\n",
                rust_global_fn_name(&name)
            ));
            out.push_str("    ");
            out.push_str(&emit_expr(&defs[0].expr, 1)?);
            out.push_str("\n}\n\n");
            continue;
        }

        // Multiple defs with the same name become a runtime `MultiClause` value, matching the
        // interpreter's behavior.
        for (i, def) in defs.iter().enumerate() {
            let clause_fn = format!("{}_clause_{i}", rust_global_fn_name(&name));
            out.push_str(&format!(
                "fn {clause_fn}(rt: &mut Runtime) -> R {{\n"
            ));
            out.push_str("    ");
            out.push_str(&emit_expr(&def.expr, 1)?);
            out.push_str("\n}\n\n");
        }

        out.push_str(&format!(
            "{def_vis}fn {}(rt: &mut Runtime) -> R {{\n",
            rust_global_fn_name(&name)
        ));
        out.push_str("    aivi_ok(Value::MultiClause(vec![\n");
        for i in 0..defs.len() {
            let clause_fn = format!("{}_clause_{i}", rust_global_fn_name(&name));
            out.push_str(&format!("        ({clause_fn}(rt))?,\n"));
        }
        out.push_str("    ]))\n");
        out.push_str("}\n\n");
    }

    if matches!(kind, EmitKind::Bin) {
        let main_fn = rust_global_fn_name("main");
        out.push_str("fn main() {\n");
        out.push_str("    let mut rt = Runtime::new();\n");
        out.push_str(&format!("    let result: Result<(), RuntimeError> = (|| {{\n"));
        out.push_str(&format!("        let v = {main_fn}(&mut rt)?;\n"));
        out.push_str("        let _ = rt.run_effect_value(v)?;\n");
        out.push_str("        Ok(())\n");
        out.push_str("    })();\n");
        out.push_str("    match result {\n");
        out.push_str("        Ok(_) => {}\n");
        out.push_str("        Err(err) => {\n");
        out.push_str("            eprintln!(\"{err}\");\n");
        out.push_str("            std::process::exit(1);\n");
        out.push_str("        }\n");
        out.push_str("    }\n");
        out.push_str("}\n");
    }

    Ok(out)
}

fn emit_expr(expr: &RustIrExpr, indent: usize) -> Result<String, AiviError> {
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
            rendered.push_str(&format!("{ind2}let mut args: Vec<Value> = Vec::new();\n"));
            for arg in args {
                let arg_code = emit_expr(arg, indent + 1)?;
                rendered.push_str(&format!("{ind2}args.push(({arg_code})?);\n"));
            }
            rendered.push_str(&format!("{ind2}rt.call(f, args)\n{ind}}}"));
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
        RustIrExpr::Binary { op, left, right, .. } => {
            let left_code = emit_expr(left, indent)?;
            let right_code = emit_expr(right, indent)?;
            emit_binary(op, left_code, right_code)
        }
        RustIrExpr::Block { block_kind, items, .. } => emit_block(*block_kind, items, indent)?,
        RustIrExpr::Raw { text, .. } => {
            return Err(AiviError::Codegen(format!(
                "raw expressions are not supported by the native backend yet: {text}"
            )))
        }
        RustIrExpr::Match { scrutinee, arms, .. } => emit_match(scrutinee, arms, indent)?,
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

fn emit_match(scrutinee: &RustIrExpr, arms: &[RustIrMatchArm], indent: usize) -> Result<String, AiviError> {
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

fn collect_pattern_vars(pattern: &RustIrPattern, out: &mut Vec<String>) {
    match pattern {
        RustIrPattern::Wildcard { .. } => {}
        RustIrPattern::Var { name, .. } => out.push(name.clone()),
        RustIrPattern::Literal { .. } => {}
        RustIrPattern::Constructor { args, .. } => {
            for arg in args {
                collect_pattern_vars(arg, out);
            }
        }
        RustIrPattern::Tuple { items, .. } => {
            for item in items {
                collect_pattern_vars(item, out);
            }
        }
        RustIrPattern::List { items, rest, .. } => {
            for item in items {
                collect_pattern_vars(item, out);
            }
            if let Some(rest) = rest.as_deref() {
                collect_pattern_vars(rest, out);
            }
        }
        RustIrPattern::Record { fields, .. } => {
            for field in fields {
                collect_pattern_vars(&field.pattern, out);
            }
        }
    }
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
    s.push_str(&emit_pattern_check(pattern, value_ident, bindings_ident, indent + 1)?);
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
        RustIrPattern::Var { name, .. } => format!(
            "{{ {bindings_ident}.insert({name:?}, {value_ident}.clone()); true }}"
        ),
        RustIrPattern::Literal { value, .. } => match value {
            crate::rust_ir::RustIrLiteral::Bool(b) => {
                format!("matches!({value_ident}, Value::Bool(v) if *v == {b})")
            }
            crate::rust_ir::RustIrLiteral::String(text) => format!(
                "matches!({value_ident}, Value::Text(v) if v == {text:?})"
            ),
            crate::rust_ir::RustIrLiteral::DateTime(text) => format!(
                "matches!({value_ident}, Value::DateTime(v) if v == {text:?})"
            ),
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
                inner.push_str(&format!(
                    "{ind}    let v{i} = &args[{i}];\n"
                ));
                let check = emit_pattern_check(arg_pat, &format!("v{i}"), bindings_ident, indent + 1)?;
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
                let check = emit_pattern_check(item_pat, &format!("v{i}"), bindings_ident, indent + 1)?;
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
                let check = emit_pattern_check(item_pat, &format!("v{i}"), bindings_ident, indent + 1)?;
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
                inner.push_str(&format!(
                    "{ind}    items.len() == {}\n",
                    items.len()
                ));
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
                    return Err(AiviError::Codegen(
                        "empty record pattern path".to_string(),
                    ));
                }
                inner.push_str(&format!("{ind}    let mut cur{i}: &Value = {value_ident};\n"));
                for seg in path.iter() {
                    inner.push_str(&format!("{ind}    cur{i} = match cur{i} {{\n"));
                    inner.push_str(&format!("{ind}        Value::Record(m) => match m.get({seg:?}) {{\n"));
                    inner.push_str(&format!("{ind}            Some(v) => v,\n"));
                    inner.push_str(&format!("{ind}            None => return false,\n"));
                    inner.push_str(&format!("{ind}        }},\n"));
                    inner.push_str(&format!("{ind}        _ => return false,\n"));
                    inner.push_str(&format!("{ind}    }};\n"));
                }
                let check = emit_pattern_check(&field.pattern, &format!("cur{i}"), bindings_ident, indent + 1)?;
                inner.push_str(&format!("{ind}    if !({check}) {{ return false; }}\n"));
            }
            inner.push_str(&format!("{ind}    true\n{ind}}}, _ => false }}"));
            inner
        }
    })
}

fn emit_block(
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
                        s.push_str(
                            "let __v = match __tmp {\n",
                        );
                        s.push_str(&"    ".repeat(indent + 5));
                        s.push_str(
                            "Value::Resource(res) => {\n",
                        );
                        s.push_str(&"    ".repeat(indent + 6));
                        s.push_str(
                            "let (v, cleanup) = rt.acquire_resource(res)?;\n",
                        );
                        s.push_str(&"    ".repeat(indent + 6));
                        s.push_str("__cleanups.push(cleanup);\n");
                        s.push_str(&"    ".repeat(indent + 6));
                        s.push_str("v\n");
                        s.push_str(&"    ".repeat(indent + 5));
                        s.push_str("}\n");
                        s.push_str(&"    ".repeat(indent + 5));
                        s.push_str(
                            "Value::Effect(_) => rt.run_effect_value(__tmp)?,\n",
                        );
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

fn emit_pattern_bind_stmts(
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
    s.push_str(&format!(
        "}})(&{value_ident}, &mut {bindings_ident});\n"
    ));
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
            s.push_str(&format!(
                "let {sub_items} = rt.generator_to_vec({sub})?;\n"
            ));
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
            s.push_str(&format!(
                "let {src_items} = rt.generator_to_vec({src})?;\n"
            ));
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

fn collect_free_locals_in_items(items: &[RustIrBlockItem]) -> Vec<String> {
    let mut bound: Vec<String> = Vec::new();
    let mut out: HashSet<String> = HashSet::new();

    for item in items {
        match item {
            RustIrBlockItem::Bind { pattern, expr } => {
                collect_free_locals_in_expr(expr, &mut bound, &mut out);
                let mut binders = Vec::new();
                collect_pattern_vars(pattern, &mut binders);
                for binder in binders {
                    bound.push(binder);
                }
            }
            RustIrBlockItem::Filter { expr }
            | RustIrBlockItem::Yield { expr }
            | RustIrBlockItem::Recurse { expr }
            | RustIrBlockItem::Expr { expr } => {
                collect_free_locals_in_expr(expr, &mut bound, &mut out);
            }
        }
    }

    let mut out = out.into_iter().collect::<Vec<_>>();
    out.sort();
    out
}

fn collect_free_locals_in_expr(expr: &RustIrExpr, bound: &mut Vec<String>, out: &mut HashSet<String>) {
    match expr {
        RustIrExpr::Local { name, .. } => {
            if !bound.iter().rev().any(|b| b == name) {
                out.insert(name.clone());
            }
        }
        RustIrExpr::Global { .. }
        | RustIrExpr::Builtin { .. }
        | RustIrExpr::ConstructorValue { .. }
        | RustIrExpr::LitNumber { .. }
        | RustIrExpr::LitString { .. }
        | RustIrExpr::LitSigil { .. }
        | RustIrExpr::LitBool { .. }
        | RustIrExpr::LitDateTime { .. }
        | RustIrExpr::Raw { .. } => {}
        RustIrExpr::TextInterpolate { parts, .. } => {
            for part in parts {
                if let crate::rust_ir::RustIrTextPart::Expr { expr } = part {
                    collect_free_locals_in_expr(expr, bound, out);
                }
            }
        }
        RustIrExpr::Lambda { param, body, .. } => {
            bound.push(param.clone());
            collect_free_locals_in_expr(body, bound, out);
            bound.pop();
        }
        RustIrExpr::App { func, arg, .. } => {
            collect_free_locals_in_expr(func, bound, out);
            collect_free_locals_in_expr(arg, bound, out);
        }
        RustIrExpr::Call { func, args, .. } => {
            collect_free_locals_in_expr(func, bound, out);
            for arg in args {
                collect_free_locals_in_expr(arg, bound, out);
            }
        }
        RustIrExpr::List { items, .. } => {
            for item in items {
                collect_free_locals_in_expr(&item.expr, bound, out);
            }
        }
        RustIrExpr::Tuple { items, .. } => {
            for item in items {
                collect_free_locals_in_expr(item, bound, out);
            }
        }
        RustIrExpr::Record { fields, .. } | RustIrExpr::Patch { fields, .. } => {
            for field in fields {
                for seg in &field.path {
                    match seg {
                        RustIrPathSegment::IndexValue(expr) | RustIrPathSegment::IndexPredicate(expr) => {
                            collect_free_locals_in_expr(expr, bound, out);
                        }
                        RustIrPathSegment::Field(_) | RustIrPathSegment::IndexFieldBool(_) => {}
                    }
                }
                collect_free_locals_in_expr(&field.value, bound, out);
            }
            if let RustIrExpr::Patch { target, .. } = expr {
                collect_free_locals_in_expr(target, bound, out);
            }
        }
        RustIrExpr::FieldAccess { base, .. } => collect_free_locals_in_expr(base, bound, out),
        RustIrExpr::Index { base, index, .. } => {
            collect_free_locals_in_expr(base, bound, out);
            collect_free_locals_in_expr(index, bound, out);
        }
        RustIrExpr::Match { scrutinee, arms, .. } => {
            collect_free_locals_in_expr(scrutinee, bound, out);
            for arm in arms {
                let mut binders = Vec::new();
                collect_pattern_vars(&arm.pattern, &mut binders);
                bound.extend(binders.iter().cloned());
                if let Some(guard) = &arm.guard {
                    collect_free_locals_in_expr(guard, bound, out);
                }
                collect_free_locals_in_expr(&arm.body, bound, out);
                for _ in 0..binders.len() {
                    bound.pop();
                }
            }
        }
        RustIrExpr::If { cond, then_branch, else_branch, .. } => {
            collect_free_locals_in_expr(cond, bound, out);
            collect_free_locals_in_expr(then_branch, bound, out);
            collect_free_locals_in_expr(else_branch, bound, out);
        }
        RustIrExpr::Binary { left, right, .. } => {
            collect_free_locals_in_expr(left, bound, out);
            collect_free_locals_in_expr(right, bound, out);
        }
        RustIrExpr::Block { items, .. } => {
            let before = bound.len();
            for item in items {
                match item {
                    RustIrBlockItem::Bind { pattern, expr } => {
                        collect_free_locals_in_expr(expr, bound, out);
                        let mut binders = Vec::new();
                        collect_pattern_vars(pattern, &mut binders);
                        bound.extend(binders);
                    }
                    RustIrBlockItem::Filter { expr }
                    | RustIrBlockItem::Yield { expr }
                    | RustIrBlockItem::Recurse { expr }
                    | RustIrBlockItem::Expr { expr } => {
                        collect_free_locals_in_expr(expr, bound, out);
                    }
                }
            }
            bound.truncate(before);
        }
    }
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
                s.push_str(&format!(
                    "let __tmp = ({expr_code})?;\n"
                ));
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
                s.push_str(&format!(
                    "let __tmp = ({expr_code})?;\n"
                ));
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
    let base = rust_local_name(name);
    let hash = fnv1a64(name);
    format!("def_{base}__{hash:016x}")
}

fn fnv1a64(value: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for b in value.as_bytes() {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
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
