mod cst;
mod diagnostics;
mod formatter;
mod hir;
mod i18n;
mod i18n_codegen;
mod kernel;
pub mod lexer;
mod mcp;
mod native_rust_backend;
mod pm;
mod resolver;
mod runtime;
mod rust_codegen;
mod rust_ir;
mod rustc_backend;
mod stdlib;
mod surface;
pub mod syntax;
mod typecheck;
mod workspace;

use std::fs;
use std::path::{Path, PathBuf};

pub use cst::{CstBundle, CstFile, CstToken};
pub use diagnostics::{
    file_diagnostics_have_errors, render_diagnostics, Diagnostic, DiagnosticLabel,
    DiagnosticSeverity, FileDiagnostic, Position, Span,
};
pub use formatter::{format_text, format_text_with_options, FormatOptions};
pub use hir::{HirModule, HirProgram};
pub use i18n_codegen::{
    generate_i18n_module_from_properties, parse_properties_catalog, PropertiesEntry,
};
pub use kernel::{
    lower_hir as lower_kernel, KernelBlockItem, KernelBlockKind, KernelDef, KernelExpr,
    KernelListItem, KernelLiteral, KernelMatchArm, KernelModule, KernelPathSegment, KernelPattern,
    KernelProgram, KernelRecordField, KernelRecordPatternField, KernelTextPart,
};
pub use mcp::{
    bundled_specs_manifest, serve_mcp_stdio, serve_mcp_stdio_with_policy, McpManifest, McpPolicy,
    McpResource, McpTool,
};
pub use native_rust_backend::{emit_native_rust_source, emit_native_rust_source_lib};
pub use pm::{
    collect_aivi_sources, edit_cargo_toml_dependencies, ensure_aivi_dependency, read_aivi_toml,
    validate_publish_preflight, write_scaffold, AiviCargoMetadata, AiviToml, CargoDepSpec,
    CargoDepSpecParseError, CargoManifestEdits, ProjectKind,
};
pub use resolver::check_modules;
pub use runtime::{run_native, run_native_with_fuel, run_test_suite, TestFailure, TestReport};
pub use rust_codegen::{compile_rust_native, compile_rust_native_lib};
pub use rust_ir::{lower_kernel as lower_rust_ir, RustIrProgram};
pub use rustc_backend::{build_with_rustc, emit_rustc_source};
pub use stdlib::{embedded_stdlib_modules, embedded_stdlib_source};
pub use surface::{
    parse_modules, parse_modules_from_tokens, BlockItem, BlockKind, ClassDecl, Decorator, Def,
    DomainDecl, DomainItem, Expr, InstanceDecl, ListItem, Literal, MatchArm, Module, ModuleItem,
    PathSegment, Pattern, RecordField, RecordPatternField, SpannedName, TextPart, TypeAlias,
    TypeCtor, TypeDecl, TypeExpr, TypeSig, UseDecl,
};
pub use typecheck::{check_types, elaborate_expected_coercions, infer_value_types};

// Expose a small, deterministic building block for tests and fuzzers without forcing callers
// through the filesystem/stdlib-loading CLI entrypoints.
pub fn desugar_modules(modules: &[Module]) -> HirProgram {
    hir::desugar_modules(modules)
}

#[derive(Debug, thiserror::Error)]
pub enum AiviError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid path: {0}")]
    InvalidPath(String),
    #[error("Diagnostics emitted")]
    Diagnostics,
    #[error("Invalid command: {0}")]
    InvalidCommand(String),
    #[error("Codegen error: {0}")]
    Codegen(String),
    #[error("WASM error: {0}")]
    Wasm(String),
    #[error("Runtime error: {0}")]
    Runtime(String),
    #[error("Config error: {0}")]
    Config(String),
    #[error("Cargo error: {0}")]
    Cargo(String),
}

pub fn parse_target(target: &str) -> Result<CstBundle, AiviError> {
    let mut files = Vec::new();
    let paths = workspace::expand_target(target)?;
    for path in paths {
        files.push(parse_file(&path)?);
    }
    Ok(CstBundle { files })
}

pub fn parse_file(path: &Path) -> Result<CstFile, AiviError> {
    let content = fs::read_to_string(path)?;
    let lines: Vec<String> = content.lines().map(|line| line.to_string()).collect();
    let byte_count = content.len();
    let line_count = lines.len();
    let (tokens, mut diagnostics) = lexer::lex(&content);
    let (_, parse_diags) = parse_modules_from_tokens(path, &tokens);
    let mut parse_diags: Vec<Diagnostic> = parse_diags
        .into_iter()
        .map(|diag| diag.diagnostic)
        .collect();
    diagnostics.append(&mut parse_diags);
    Ok(CstFile {
        path: path.display().to_string(),
        byte_count,
        line_count,
        lines,
        tokens,
        diagnostics,
    })
}

pub fn lex_cst(content: &str) -> (Vec<CstToken>, Vec<Diagnostic>) {
    lexer::lex(content)
}

pub fn load_modules(target: &str) -> Result<Vec<Module>, AiviError> {
    let paths = workspace::expand_target(target)?;
    let mut modules = Vec::new();
    for path in paths {
        let content = fs::read_to_string(&path)?;
        let (mut file_modules, _) = parse_modules(&path, &content);
        modules.append(&mut file_modules);
    }
    let mut stdlib_modules = stdlib::embedded_stdlib_modules();
    stdlib_modules.append(&mut modules);
    Ok(stdlib_modules)
}

pub fn load_module_diagnostics(target: &str) -> Result<Vec<FileDiagnostic>, AiviError> {
    let paths = workspace::expand_target(target)?;
    let mut diagnostics = Vec::new();
    for path in paths {
        let content = fs::read_to_string(&path)?;
        let (_, mut file_diags) = parse_modules(&path, &content);
        diagnostics.append(&mut file_diags);
    }
    Ok(diagnostics)
}

pub fn desugar_target(target: &str) -> Result<HirProgram, AiviError> {
    let diagnostics = load_module_diagnostics(target)?;
    if file_diagnostics_have_errors(&diagnostics) {
        return Err(AiviError::Diagnostics);
    }
    let paths = workspace::expand_target(target)?;
    let mut modules = Vec::new();
    for path in &paths {
        let content = fs::read_to_string(path)?;
        let (mut parsed, _) = parse_modules(path.as_path(), &content);
        modules.append(&mut parsed);
    }
    let mut stdlib_modules = stdlib::embedded_stdlib_modules();
    stdlib_modules.append(&mut modules);
    Ok(hir::desugar_modules(&stdlib_modules))
}

pub fn desugar_target_typed(target: &str) -> Result<HirProgram, AiviError> {
    let diagnostics = load_module_diagnostics(target)?;
    if file_diagnostics_have_errors(&diagnostics) {
        return Err(AiviError::Diagnostics);
    }
    let paths = workspace::expand_target(target)?;
    let mut modules = Vec::new();
    for path in &paths {
        let content = fs::read_to_string(path)?;
        let (mut parsed, _) = parse_modules(path.as_path(), &content);
        modules.append(&mut parsed);
    }
    let mut stdlib_modules = stdlib::embedded_stdlib_modules();
    stdlib_modules.append(&mut modules);

    let mut diagnostics = check_modules(&stdlib_modules);
    if diagnostics.is_empty() {
        diagnostics.extend(elaborate_expected_coercions(&mut stdlib_modules));
    }
    if file_diagnostics_have_errors(&diagnostics) {
        return Err(AiviError::Diagnostics);
    }

    Ok(hir::desugar_modules(&stdlib_modules))
}

pub fn kernel_target(target: &str) -> Result<kernel::KernelProgram, AiviError> {
    let hir = desugar_target_typed(target)?;
    Ok(kernel::lower_hir(hir))
}

pub fn rust_ir_target(target: &str) -> Result<rust_ir::RustIrProgram, AiviError> {
    let kernel = kernel_target(target)?;
    rust_ir::lower_kernel(kernel)
}

pub fn format_target(target: &str) -> Result<String, AiviError> {
    let paths = workspace::expand_target(target)?;
    if paths.len() != 1 {
        return Err(AiviError::InvalidCommand(
            "fmt expects a single file path".to_string(),
        ));
    }
    let content = fs::read_to_string(&paths[0])?;
    Ok(format_text(&content))
}

pub fn resolve_target(target: &str) -> Result<Vec<PathBuf>, AiviError> {
    workspace::expand_target(target)
}
