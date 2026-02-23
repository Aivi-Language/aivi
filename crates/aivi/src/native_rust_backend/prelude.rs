pub(super) fn emit_runtime_prelude() -> String {
    let mut out = String::new();
    out.push_str("#![allow(warnings)]\n");
    out.push_str("use std::collections::HashMap;\n");
    out.push_str("use std::sync::{Arc, Mutex};\n\n");
    out.push_str(
        "use aivi_native_runtime::{get_builtin, ok as aivi_ok, BuiltinImpl, BuiltinValue, EffectValue, KeyValue, ResourceValue, Runtime, RuntimeError, R, Value};\n\n",
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
    out.push_str("    IndexAll,\n");
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
    out.push_str("            Value::Record(mut map) => {\n");
    out.push_str(
        "                let old = Arc::make_mut(&mut map).remove(name).unwrap_or(Value::Unit);\n",
    );
    out.push_str("                let new_val = patch_path(rt, old, &path[1..], updater)?;\n");
    out.push_str("                Arc::make_mut(&mut map).insert(name.clone(), new_val);\n");
    out.push_str("                aivi_ok(Value::Record(map))\n");
    out.push_str("            }\n");
    out.push_str(
        "            other => Err(RuntimeError::Message(format!(\"expected Record for field patch, got {}\", aivi_native_runtime::format_value(&other)))),\n",
    );
    out.push_str("        },\n");
    out.push_str("        PathSeg::IndexAll => match target {\n");
    out.push_str("            Value::List(items) => {\n");
    out.push_str("                let mut items = items;\n");
    out.push_str("                let out_items = Arc::make_mut(&mut items);\n");
    out.push_str("                for item in out_items.iter_mut() {\n");
    out.push_str(
        "                    *item = patch_path(rt, item.clone(), &path[1..], updater.clone())?;\n",
    );
    out.push_str("                }\n");
    out.push_str("                aivi_ok(Value::List(items))\n");
    out.push_str("            }\n");
    out.push_str("            Value::Map(entries) => {\n");
    out.push_str("                let mut entries = entries;\n");
    out.push_str("                let snapshot = entries.as_ref().clone();\n");
    out.push_str("                for (k, v) in snapshot.iter() {\n");
    out.push_str("                    let new_val = patch_path(rt, v.clone(), &path[1..], updater.clone())?;\n");
    out.push_str("                    Arc::make_mut(&mut entries).insert(k.clone(), new_val);\n");
    out.push_str("                }\n");
    out.push_str("                aivi_ok(Value::Map(entries))\n");
    out.push_str("            }\n");
    out.push_str("            other => Err(RuntimeError::Message(format!(\"expected List/Map for traversal patch, got {}\", aivi_native_runtime::format_value(&other)))),\n");
    out.push_str("        },\n");
    out.push_str("        PathSeg::IndexValue(idx) => match (target, idx.clone()) {\n");
    out.push_str("            (Value::List(items), Value::Int(i)) => {\n");
    out.push_str("                let i = i as usize;\n");
    out.push_str("                if i >= items.len() { return Err(RuntimeError::Message(\"index out of bounds\".to_string())); }\n");
    out.push_str("                let mut items = items;\n");
    out.push_str("                let out = Arc::make_mut(&mut items);\n");
    out.push_str("                let old = out[i].clone();\n");
    out.push_str("                out[i] = patch_path(rt, old, &path[1..], updater)?;\n");
    out.push_str("                aivi_ok(Value::List(items))\n");
    out.push_str("            }\n");
    out.push_str("            (Value::Tuple(mut items), Value::Int(i)) => {\n");
    out.push_str("                let i = i as usize;\n");
    out.push_str("                if i >= items.len() { return Err(RuntimeError::Message(\"index out of bounds\".to_string())); }\n");
    out.push_str("                let old = items[i].clone();\n");
    out.push_str("                items[i] = patch_path(rt, old, &path[1..], updater)?;\n");
    out.push_str("                aivi_ok(Value::Tuple(items))\n");
    out.push_str("            }\n");
    out.push_str("            (Value::Map(entries), idx) => {\n");
    out.push_str("                let Some(key) = KeyValue::try_from_value(&idx) else {\n");
    out.push_str("                    return Err(RuntimeError::Message(format!(\"map key is not a valid key type: {}\", aivi_native_runtime::format_value(&idx))));\n");
    out.push_str("                };\n");
    out.push_str("                let mut entries = entries;\n");
    out.push_str("                let old = entries.get(&key).cloned().unwrap_or(Value::Unit);\n");
    out.push_str("                let new_val = patch_path(rt, old, &path[1..], updater)?;\n");
    out.push_str("                Arc::make_mut(&mut entries).insert(key, new_val);\n");
    out.push_str("                aivi_ok(Value::Map(entries))\n");
    out.push_str("            }\n");
    out.push_str(
        "            (other, _) => Err(RuntimeError::Message(format!(\"expected List/Tuple + Int for index patch, got {}\", aivi_native_runtime::format_value(&other)))),\n",
    );
    out.push_str("        },\n");
    out.push_str("        PathSeg::IndexFieldBool(field) => match target {\n");
    out.push_str("            Value::List(items) => {\n");
    out.push_str("                let mut items = items;\n");
    out.push_str("                let out_items = Arc::make_mut(&mut items);\n");
    out.push_str("                for item in out_items.iter_mut() {\n");
    out.push_str("                    let should_patch = match item {\n");
    out.push_str("                        Value::Record(map) => matches!(map.get(field), Some(Value::Bool(true))),\n");
    out.push_str("                        _ => false,\n");
    out.push_str("                    };\n");
    out.push_str("                    if should_patch {\n");
    out.push_str("                        *item = patch_path(rt, item.clone(), &path[1..], updater.clone())?;\n");
    out.push_str("                    }\n");
    out.push_str("                }\n");
    out.push_str("                aivi_ok(Value::List(items))\n");
    out.push_str("            }\n");
    out.push_str(
        "            other => Err(RuntimeError::Message(format!(\"expected List for traversal patch, got {}\", aivi_native_runtime::format_value(&other)))),\n",
    );
    out.push_str("        },\n");
    out.push_str("        PathSeg::IndexPredicate(pred) => match target {\n");
    out.push_str("            Value::List(items) => {\n");
    out.push_str("                let mut items = items;\n");
    out.push_str("                let out_items = Arc::make_mut(&mut items);\n");
    out.push_str("                for item in out_items.iter_mut() {\n");
    out.push_str("                    let keep = match rt.apply(pred.clone(), item.clone())? {\n");
    out.push_str("                        Value::Bool(true) => true,\n");
    out.push_str("                        Value::Bool(false) => false,\n");
    out.push_str(
        "                        other => return Err(RuntimeError::Message(format!(\"expected Bool predicate, got {}\", aivi_native_runtime::format_value(&other)))),\n",
    );
    out.push_str("                    };\n");
    out.push_str("                    if keep {\n");
    out.push_str("                        *item = patch_path(rt, item.clone(), &path[1..], updater.clone())?;\n");
    out.push_str("                    }\n");
    out.push_str("                }\n");
    out.push_str("                aivi_ok(Value::List(items))\n");
    out.push_str("            }\n");
    out.push_str("            Value::Map(entries) => {\n");
    out.push_str("                let mut entries = entries;\n");
    out.push_str("                let snapshot = entries.as_ref().clone();\n");
    out.push_str("                for (k, v) in snapshot.iter() {\n");
    out.push_str("                    let mut rec = HashMap::new();\n");
    out.push_str("                    rec.insert(\"key\".to_string(), k.to_value());\n");
    out.push_str("                    rec.insert(\"value\".to_string(), v.clone());\n");
    out.push_str("                    let entry = Value::Record(Arc::new(rec));\n");
    out.push_str("                    let keep = match rt.apply(pred.clone(), entry)? {\n");
    out.push_str("                        Value::Bool(true) => true,\n");
    out.push_str("                        Value::Bool(false) => false,\n");
    out.push_str(
        "                        other => return Err(RuntimeError::Message(format!(\"expected Bool predicate, got {}\", aivi_native_runtime::format_value(&other)))),\n",
    );
    out.push_str("                    };\n");
    out.push_str("                    if keep {\n");
    out.push_str("                        let new_val = patch_path(rt, v.clone(), &path[1..], updater.clone())?;\n");
    out.push_str(
        "                        Arc::make_mut(&mut entries).insert(k.clone(), new_val);\n",
    );
    out.push_str("                    }\n");
    out.push_str("                }\n");
    out.push_str("                aivi_ok(Value::Map(entries))\n");
    out.push_str("            }\n");
    out.push_str(
        "            other => Err(RuntimeError::Message(format!(\"expected List/Map for predicate traversal patch, got {}\", aivi_native_runtime::format_value(&other)))),\n",
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
