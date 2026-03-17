#![allow(dead_code)]

//! Shared helpers for integration tests.

use std::path::Path;

use aivi::{desugar_target_with_cg_types, run_cranelift_jit, AiviError};
use tempfile::tempdir;

/// Write an `.aivi` source file to a temporary location and return its path
/// string.  The caller still owns the `TempDir`.
pub fn write_aivi_source(dir: &Path, name: &str, source: &str) -> String {
    let path = dir.join(name);
    std::fs::write(&path, source).expect("write aivi source");
    path.to_string_lossy().into_owned()
}

pub fn run_jit_err(thread_name: &str, source: &str) -> AiviError {
    let source = source.to_string();
    let thread_name = thread_name.to_string();
    let result = std::thread::Builder::new()
        .name(thread_name)
        .stack_size(64 * 1024 * 1024)
        .spawn(move || {
            let dir = tempdir().expect("tempdir");
            let source_path_str = write_aivi_source(dir.path(), "main.aivi", &source);
            let (program, cg_types, monomorph_plan) =
                desugar_target_with_cg_types(&source_path_str).expect("desugar");
            run_cranelift_jit(
                program,
                cg_types,
                monomorph_plan,
                std::collections::HashMap::new(),
                &[],
            )
            .expect_err("expected runtime error")
        })
        .expect("spawn test thread")
        .join();
    match result {
        Ok(err) => err,
        Err(payload) => std::panic::resume_unwind(payload),
    }
}
