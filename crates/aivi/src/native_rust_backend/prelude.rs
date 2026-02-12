pub(super) fn emit_runtime_prelude() -> String {
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

    out.push_str(
        "fn patch(rt: &mut Runtime, target: Value, fields: Vec<(Vec<PathSeg>, Value)>) -> R {\n",
    );
    out.push_str("    let mut acc = target;\n");
    out.push_str("    for (path, updater) in fields {\n");
    out.push_str("        acc = patch_path(rt, acc, &path, updater)?;\n");
    out.push_str("    }\n");
    out.push_str("    aivi_ok(acc)\n");
    out.push_str("}\n\n");
    out
}
