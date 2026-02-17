use crate::AiviError;
use crate::hir::HirProgram;
use std::path::Path;

pub fn build_with_rustc(
    _program: HirProgram,
    _out: &Path,
    _rustc_args: &[String],
) -> Result<(), AiviError> {
    Err(AiviError::Codegen(
        "The rustc backend has been removed. Use --target=rust-native instead.".to_string(),
    ))
}

pub fn emit_rustc_source(_program: crate::rust_ir::RustIrProgram) -> Result<String, AiviError> {
    Err(AiviError::Codegen(
        "The rustc backend has been removed. Use --target=rust-native instead.".to_string(),
    ))
}

