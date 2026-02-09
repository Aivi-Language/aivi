mod cst;
mod diagnostics;
mod formatter;
mod hir;
mod lexer;
mod resolver;
mod surface;
mod typecheck;
mod runtime;
mod rust_codegen;
mod pm;
mod kernel;
mod rust_ir;
mod rustc_backend;

use std::fs;
use std::path::{Path, PathBuf};

pub use cst::{CstBundle, CstFile, CstToken};
pub use diagnostics::{render_diagnostics, Diagnostic, DiagnosticLabel, FileDiagnostic, Position, Span};
pub use formatter::format_text;
pub use hir::{HirModule, HirProgram};
pub use resolver::check_modules;
pub use surface::{
    parse_modules, parse_modules_from_tokens, BlockItem, ClassDecl, Def, DomainDecl, DomainItem,
    Expr, InstanceDecl, ListItem,
    MatchArm, Module, ModuleItem, PathSegment, Pattern, RecordField, RecordPatternField,
    SpannedName, TypeAlias, TypeCtor, TypeDecl, TypeExpr, TypeSig, UseDecl,
};
pub use typecheck::check_types;
pub use runtime::run_native;
pub use rust_codegen::{compile_rust, compile_rust_lib};
pub use kernel::{KernelProgram, lower_hir as lower_kernel};
pub use rust_ir::{RustIrProgram, lower_kernel as lower_rust_ir};
pub use rustc_backend::{build_with_rustc, emit_rustc_source};
pub use pm::{
    collect_aivi_sources, edit_cargo_toml_dependencies, read_aivi_toml, write_scaffold, AiviToml,
    CargoDepSpec, CargoDepSpecParseError, CargoManifestEdits, ProjectKind,
};

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
    let paths = expand_target(target)?;
    for path in paths {
        files.push(parse_file(&path)?);
    }
    Ok(CstBundle { files })
}

pub fn parse_file(path: &Path) -> Result<CstFile, AiviError> {
    let content = fs::read_to_string(path)?;
    let lines: Vec<String> = content.lines().map(|line| line.to_string()).collect();
    let byte_count = content.as_bytes().len();
    let line_count = lines.len();
    let (tokens, mut diagnostics) = lexer::lex(&content);
    let (_, parse_diags) = parse_modules_from_tokens(path, &tokens);
    let mut parse_diags: Vec<Diagnostic> =
        parse_diags.into_iter().map(|diag| diag.diagnostic).collect();
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

pub fn load_modules(target: &str) -> Result<Vec<Module>, AiviError> {
    let paths = expand_target(target)?;
    let mut modules = Vec::new();
    for path in paths {
        let content = fs::read_to_string(&path)?;
        let (mut file_modules, _) = parse_modules(&path, &content);
        modules.append(&mut file_modules);
    }
    Ok(modules)
}

pub fn load_module_diagnostics(target: &str) -> Result<Vec<FileDiagnostic>, AiviError> {
    let paths = expand_target(target)?;
    let mut diagnostics = Vec::new();
    for path in paths {
        let content = fs::read_to_string(&path)?;
        let (_, mut file_diags) = parse_modules(&path, &content);
        diagnostics.append(&mut file_diags);
    }
    Ok(diagnostics)
}

pub fn desugar_target(target: &str) -> Result<HirProgram, AiviError> {
    let paths = expand_target(target)?;
    let mut modules = Vec::new();
    for path in paths {
        let content = fs::read_to_string(&path)?;
        let (mut parsed, _) = parse_modules(&path, &content);
        modules.append(&mut parsed);
    }
    Ok(hir::desugar_modules(&modules))
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
    let paths = expand_target(target)?;
    if paths.len() != 1 {
        return Err(AiviError::InvalidCommand(
            "fmt expects a single file path".to_string(),
        ));
    }
    let content = fs::read_to_string(&paths[0])?;
    Ok(format_text(&content))
}

pub fn resolve_target(target: &str) -> Result<Vec<PathBuf>, AiviError> {
    expand_target(target)
}

fn expand_target(target: &str) -> Result<Vec<PathBuf>, AiviError> {
    let mut paths = Vec::new();
    let (base, recursive) = match target.strip_suffix("/...") {
        Some(base) => (if base.is_empty() { "." } else { base }, true),
        None => (target, false),
    };

    let Some(path) = resolve_target_path(base) else {
        return Err(AiviError::InvalidPath(target.to_string()));
    };

    if path.is_file() {
        return Ok(vec![path.to_path_buf()]);
    }

    if path.is_dir() {
        if recursive {
            collect_files(&path, &mut paths)?;
        } else {
            for entry in fs::read_dir(path)? {
                let entry = entry?;
                let entry_path = entry.path();
                if entry_path.is_file() {
                    paths.push(entry_path);
                }
            }
        }
    }

    paths.sort();
    if paths.is_empty() {
        return Err(AiviError::InvalidPath(target.to_string()));
    }

    Ok(paths)
}

fn resolve_target_path(target: &str) -> Option<PathBuf> {
    let target_path = Path::new(target);
    if target_path.is_absolute() {
        return target_path.exists().then(|| target_path.to_path_buf());
    }

    if target_path.exists() {
        return Some(target_path.to_path_buf());
    }

    let Ok(mut dir) = std::env::current_dir() else {
        return None;
    };

    loop {
        if dir.join("Cargo.toml").exists() {
            let candidate = dir.join(target);
            if candidate.exists() {
                return Some(candidate);
            }
        }

        let Some(parent) = dir.parent() else {
            break;
        };
        dir = parent.to_path_buf();
    }

    None
}

fn collect_files(dir: &Path, paths: &mut Vec<PathBuf>) -> Result<(), AiviError> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let entry_path = entry.path();
        if entry_path.is_dir() {
            collect_files(&entry_path, paths)?;
            continue;
        }

        if entry_path.extension().and_then(|ext| ext.to_str()) == Some("aivi") {
            paths.push(entry_path);
        }
    }
    Ok(())
}
