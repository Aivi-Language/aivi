use std::collections::HashMap;

mod blocks;
mod expr;
mod pattern;
mod prelude;
mod utils;

use crate::rust_ir::{RustIrDef, RustIrModule, RustIrProgram};
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

fn emit_native_rust_source_inner(
    program: RustIrProgram,
    kind: EmitKind,
) -> Result<String, AiviError> {
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

    let mut out = prelude::emit_runtime_prelude();

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
                utils::rust_global_fn_name(&name)
            ));
            out.push_str("    ");
            out.push_str(&expr::emit_expr(&defs[0].expr, 1)?);
            out.push_str("\n}\n\n");
            continue;
        }

        // Multiple defs with the same name become a runtime `MultiClause` value, matching the
        // interpreter's behavior.
        for (i, def) in defs.iter().enumerate() {
            let clause_fn = format!("{}_clause_{i}", utils::rust_global_fn_name(&name));
            out.push_str(&format!("fn {clause_fn}(rt: &mut Runtime) -> R {{\n"));
            out.push_str("    ");
            out.push_str(&expr::emit_expr(&def.expr, 1)?);
            out.push_str("\n}\n\n");
        }

        out.push_str(&format!(
            "{def_vis}fn {}(rt: &mut Runtime) -> R {{\n",
            utils::rust_global_fn_name(&name)
        ));
        out.push_str("    aivi_ok(Value::MultiClause(vec![\n");
        for i in 0..defs.len() {
            let clause_fn = format!("{}_clause_{i}", utils::rust_global_fn_name(&name));
            out.push_str(&format!("        ({clause_fn}(rt))?,\n"));
        }
        out.push_str("    ]))\n");
        out.push_str("}\n\n");
    }

    if matches!(kind, EmitKind::Bin) {
        let main_fn = utils::rust_global_fn_name("main");
        out.push_str("fn main() {\n");
        out.push_str("    let mut rt = Runtime::new();\n");
        out.push_str(&format!(
            "    let result: Result<(), RuntimeError> = (|| {{\n"
        ));
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
