use std::collections::HashMap;

mod blocks;
mod expr;
mod pattern;
mod prelude;
pub(crate) mod typed_expr;
mod utils;

use crate::rust_ir::cg_type::CgType;
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

    // Collect global CgType map for the typed emitter context.
    let mut global_cg_types: HashMap<String, CgType> = HashMap::new();
    for def in &module.defs {
        if let Some(ref cg_ty) = def.cg_type {
            if cg_ty.is_closed() {
                global_cg_types.insert(def.name.clone(), cg_ty.clone());
            }
        }
    }

    let mut order: Vec<String> = Vec::new();
    let mut groups: HashMap<String, Vec<&RustIrDef>> = HashMap::new();
    for def in &module.defs {
        let entry = groups.entry(def.name.clone()).or_default();
        if entry.is_empty() {
            order.push(def.name.clone());
        }
        entry.push(def);
    }

    for name in &order {
        let defs = groups.get(name).expect("def group");
        if *name == "main" && defs.len() != 1 {
            return Err(AiviError::Codegen(
                "native backend expects exactly one main definition".to_string(),
            ));
        }
        if defs.len() == 1 {
            if defs[0].inline {
                out.push_str("#[inline(always)]\n");
            }
            // Always emit the original Value-returning function.
            out.push_str(&format!(
                "{def_vis}fn {}(rt: &mut Runtime) -> R {{\n",
                utils::rust_global_fn_name(name)
            ));
            out.push_str("    ");
            out.push_str(&expr::emit_expr(&defs[0].expr, 1)?);
            out.push_str("\n}\n\n");

            // Additionally emit a typed version if CgType is closed.
            if let Some(ref cg_ty) = defs[0].cg_type {
                if cg_ty.is_closed() && *name != "main" {
                    emit_typed_def(&mut out, name, defs[0], cg_ty, &global_cg_types, def_vis)?;
                }
            }
            continue;
        }

        // Multiple defs with the same name become a runtime `MultiClause` value, matching the
        // native runtime's behavior.
        for (i, def) in defs.iter().enumerate() {
            let clause_fn = format!("{}_clause_{i}", utils::rust_global_fn_name(name));
            if def.inline {
                out.push_str("#[inline(always)]\n");
            }
            out.push_str(&format!("fn {clause_fn}(rt: &mut Runtime) -> R {{\n"));
            out.push_str("    ");
            out.push_str(&expr::emit_expr(&def.expr, 1)?);
            out.push_str("\n}\n\n");
        }

        if defs.iter().any(|def| def.inline) {
            out.push_str("#[inline(always)]\n");
        }
        out.push_str(&format!(
            "{def_vis}fn {}(rt: &mut Runtime) -> R {{\n",
            utils::rust_global_fn_name(name)
        ));
        out.push_str("    aivi_ok(Value::MultiClause(vec![\n");
        for i in 0..defs.len() {
            let clause_fn = format!("{}_clause_{i}", utils::rust_global_fn_name(name));
            out.push_str(&format!("        ({clause_fn}(rt))?,\n"));
        }
        out.push_str("    ]))\n");
        out.push_str("}\n\n");
    }

    if matches!(kind, EmitKind::Bin) {
        let main_fn = utils::rust_global_fn_name("main");
        out.push_str("fn main() {\n");
        out.push_str("    let mut rt = Runtime::new();\n");
        out.push_str("    let result: Result<(), RuntimeError> = (|| {\n");
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

/// Emit a typed version of a definition (unboxed Rust types).
///
/// Produces a `_typed` variant alongside the original `Value`-returning function. The typed
/// variant uses native Rust types (i64, f64, bool, etc.) and direct operations instead of the
/// `Value` enum, yielding significantly better performance for closed types.
///
/// If typed emission fails for any reason (unsupported expression, type mismatch), the function
/// is silently skipped — the Value-returning version is always available as fallback.
fn emit_typed_def(
    out: &mut String,
    name: &str,
    def: &RustIrDef,
    cg_ty: &CgType,
    global_cg_types: &HashMap<String, CgType>,
    vis: &str,
) -> Result<(), AiviError> {
    let mut ctx = typed_expr::TypedCtx::new(global_cg_types.clone());

    // Try to emit the typed body
    let body_code = match typed_expr::emit_typed_expr(&def.expr, cg_ty, &mut ctx, 1) {
        Ok(Some(code)) => code,
        Ok(None) => return Ok(()), // Can't emit typed — silently skip
        Err(_) => return Ok(()),   // Error — silently skip
    };

    let rust_ty = cg_ty.rust_type();
    let fn_name = format!("{}_typed", utils::rust_global_fn_name(name));

    if def.inline {
        out.push_str("#[inline(always)]\n");
    }
    out.push_str("#[allow(dead_code)]\n");
    out.push_str(&format!(
        "{vis}fn {fn_name}(rt: &mut Runtime) -> Result<{rust_ty}, RuntimeError> {{\n"
    ));
    out.push_str(&format!("    Ok({body_code})\n"));
    out.push_str("}\n\n");

    Ok(())
}
