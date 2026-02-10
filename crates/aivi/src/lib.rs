mod cst;
mod diagnostics;
mod formatter;
mod hir;
mod kernel;
mod lexer;
mod mcp;
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
    render_diagnostics, Diagnostic, DiagnosticLabel, FileDiagnostic, Position, Span,
};
pub use formatter::format_text;
pub use hir::{HirModule, HirProgram};
pub use kernel::{lower_hir as lower_kernel, KernelProgram};
pub use mcp::{
    collect_mcp_manifest, serve_mcp_stdio, serve_mcp_stdio_with_policy, McpManifest, McpPolicy,
    McpResource, McpTool,
};
pub use pm::{
    collect_aivi_sources, edit_cargo_toml_dependencies, read_aivi_toml, write_scaffold, AiviToml,
    CargoDepSpec, CargoDepSpecParseError, CargoManifestEdits, ProjectKind,
};
pub use resolver::check_modules;
pub use runtime::run_native;
pub use rust_codegen::{compile_rust, compile_rust_lib};
pub use rust_ir::{lower_kernel as lower_rust_ir, RustIrProgram};
pub use rustc_backend::{build_with_rustc, emit_rustc_source};
pub use surface::{
    parse_modules, parse_modules_from_tokens, BlockItem, ClassDecl, Def, DomainDecl, DomainItem,
    Expr, InstanceDecl, ListItem, Literal, MatchArm, Module, ModuleItem, PathSegment, Pattern,
    RecordField, RecordPatternField, SpannedName, TextPart, TypeAlias, TypeCtor, TypeDecl,
    TypeExpr, TypeSig, UseDecl,
};
pub use stdlib::embedded_stdlib_modules;
pub use typecheck::{check_types, infer_value_types};

#[derive(Debug)]
pub enum AiviError {
    Io(std::io::Error),
    InvalidPath(String),
    Diagnostics,
    InvalidCommand(String),
    Codegen(String),
    Wasm(String),
    Runtime(String),
    Config(String),
    Cargo(String),
}

impl std::fmt::Display for AiviError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AiviError::Io(err) => write!(f, "IO error: {err}"),
            AiviError::InvalidPath(path) => write!(f, "Invalid path: {path}"),
            AiviError::Diagnostics => write!(f, "Diagnostics emitted"),
            AiviError::InvalidCommand(command) => write!(f, "Invalid command: {command}"),
            AiviError::Codegen(message) => write!(f, "Codegen error: {message}"),
            AiviError::Wasm(message) => write!(f, "WASM error: {message}"),
            AiviError::Runtime(message) => write!(f, "Runtime error: {message}"),
            AiviError::Config(message) => write!(f, "Config error: {message}"),
            AiviError::Cargo(message) => write!(f, "Cargo error: {message}"),
        }
    }
}

impl std::error::Error for AiviError {}

impl From<std::io::Error> for AiviError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
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
    let paths = workspace::expand_target(target)?;
    let mut modules = Vec::new();
    for path in paths {
        let content = fs::read_to_string(&path)?;
        let (mut parsed, _) = parse_modules(&path, &content);
        modules.append(&mut parsed);
    }
    let mut stdlib_modules = stdlib::embedded_stdlib_modules();
    stdlib_modules.append(&mut modules);
    Ok(hir::desugar_modules(&stdlib_modules))
}

pub fn kernel_target(target: &str) -> Result<kernel::KernelProgram, AiviError> {
    let hir = desugar_target(target)?;
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
