use std::collections::HashMap;

use crate::cg_type::CgType;
use crate::hir::HirProgram;
use crate::AiviError;
use crate::{emit_native_rust_source, emit_native_rust_source_lib, kernel, rust_ir};

/// Experimental backend: lower to Kernel -> Rust IR and emit standalone Rust.
///
/// Limitations are those of `rust_ir` + `rustc_backend` (e.g. `match` not supported yet).
pub fn compile_rust_native(program: HirProgram) -> Result<String, AiviError> {
    compile_rust_native_inner(program, None, EmitVariant::Bin)
}

/// Like `compile_rust_native` but with typed codegen: closed-type definitions additionally
/// emit unboxed `_typed` function variants alongside the standard `Value`-returning ones.
pub fn compile_rust_native_typed(
    program: HirProgram,
    cg_types: HashMap<String, HashMap<String, CgType>>,
) -> Result<String, AiviError> {
    compile_rust_native_inner(program, Some(cg_types), EmitVariant::Bin)
}

/// Experimental backend: emit a Rust library with exported definitions.
pub fn compile_rust_native_lib(program: HirProgram) -> Result<String, AiviError> {
    compile_rust_native_inner(program, None, EmitVariant::Lib)
}

/// Like `compile_rust_native_lib` but with typed codegen.
pub fn compile_rust_native_lib_typed(
    program: HirProgram,
    cg_types: HashMap<String, HashMap<String, CgType>>,
) -> Result<String, AiviError> {
    compile_rust_native_inner(program, Some(cg_types), EmitVariant::Lib)
}

#[derive(Clone, Copy)]
enum EmitVariant {
    Bin,
    Lib,
}

fn compile_rust_native_inner(
    program: HirProgram,
    cg_types: Option<HashMap<String, HashMap<String, CgType>>>,
    variant: EmitVariant,
) -> Result<String, AiviError> {
    let kernel = kernel::lower_hir(program);
    let mut rust_ir = rust_ir::lower_kernel(kernel)?;

    // Inject CgType annotations from the type checker into each RustIrDef so the backend can
    // emit typed (unboxed) function variants for closed types.
    if let Some(types) = cg_types {
        for module in &mut rust_ir.modules {
            if let Some(mod_types) = types.get(&module.name) {
                for def in &mut module.defs {
                    if let Some(cg) = mod_types.get(&def.name) {
                        def.cg_type = Some(cg.clone());
                    }
                }
            }
        }
    }

    match variant {
        EmitVariant::Bin => emit_native_rust_source(rust_ir),
        EmitVariant::Lib => emit_native_rust_source_lib(rust_ir),
    }
}
