#![deny(clippy::unwrap_used)]

mod workspace;

use std::fs;
use std::path::{Path, PathBuf};

use aivi_core::{
    check_modules, elaborate_expected_coercions, embedded_stdlib_modules,
    file_diagnostics_have_errors, format_text, parse_modules, parse_modules_from_tokens, CstBundle,
    CstFile, Diagnostic, FileDiagnostic, HirProgram, Module, ModuleItem,
};

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
    let (tokens, mut diagnostics) = aivi_core::lexer::lex(&content);
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

pub fn load_modules(target: &str) -> Result<Vec<Module>, AiviError> {
    let paths = workspace::expand_target(target)?;
    load_modules_from_paths(&paths)
}

pub fn load_modules_from_paths(paths: &[PathBuf]) -> Result<Vec<Module>, AiviError> {
    let mut modules = Vec::new();
    for path in paths {
        let content = fs::read_to_string(path)?;
        let (mut file_modules, _) = parse_modules(path.as_path(), &content);
        modules.append(&mut file_modules);
    }
    let mut stdlib_modules = embedded_stdlib_modules();
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
    let mut stdlib_modules = embedded_stdlib_modules();
    stdlib_modules.append(&mut modules);
    Ok(aivi_core::desugar_modules(&stdlib_modules))
}

pub fn test_target_program_and_names(
    target: &str,
    check_stdlib: bool,
) -> Result<(HirProgram, Vec<String>, Vec<FileDiagnostic>), AiviError> {
    let paths = workspace::expand_target(target)?;
    let mut test_paths = Vec::new();
    for path in &paths {
        if path.extension().and_then(|s| s.to_str()) != Some("aivi") {
            continue;
        }
        let content = fs::read_to_string(path)?;
        if content.contains("@test") {
            test_paths.push(path.clone());
        }
    }

    let mut test_names = Vec::new();
    for path in &test_paths {
        let content = fs::read_to_string(path)?;
        let (modules, _) = parse_modules(path.as_path(), &content);
        for module in modules {
            for item in module.items {
                let ModuleItem::Def(def) = item else {
                    continue;
                };
                if def.decorators.iter().any(|d| d.name.name == "test") {
                    test_names.push(format!("{}.{}", module.name.name, def.name.name));
                }
            }
        }
    }
    test_names.sort();
    test_names.dedup();
    if test_names.is_empty() {
        return Err(AiviError::InvalidCommand(format!(
            "no @test definitions found under {target}"
        )));
    }

    let mut diagnostics = Vec::new();
    for path in &test_paths {
        let content = fs::read_to_string(path)?;
        let (_, mut file_diags) = parse_modules(path.as_path(), &content);
        diagnostics.append(&mut file_diags);
    }
    if file_diagnostics_have_errors(&diagnostics) {
        return Err(AiviError::Diagnostics);
    }

    let mut modules = load_modules_from_paths(&test_paths)?;
    let mut check_diags = check_modules(&modules);
    if !file_diagnostics_have_errors(&check_diags) {
        check_diags.extend(elaborate_expected_coercions(&mut modules));
    }
    if !check_stdlib {
        check_diags.retain(|diag| !diag.path.starts_with("<embedded:"));
    }
    diagnostics.extend(check_diags);
    if file_diagnostics_have_errors(&diagnostics) {
        return Err(AiviError::Diagnostics);
    }

    let program = aivi_core::desugar_modules(&modules);
    Ok((program, test_names, diagnostics))
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
    let mut stdlib_modules = embedded_stdlib_modules();
    stdlib_modules.append(&mut modules);

    let mut diagnostics = check_modules(&stdlib_modules);
    if diagnostics.is_empty() {
        diagnostics.extend(elaborate_expected_coercions(&mut stdlib_modules));
    }
    if file_diagnostics_have_errors(&diagnostics) {
        return Err(AiviError::Diagnostics);
    }

    Ok(aivi_core::desugar_modules(&stdlib_modules))
}

pub fn kernel_target(target: &str) -> Result<aivi_core::KernelProgram, AiviError> {
    let hir = desugar_target_typed(target)?;
    Ok(aivi_core::lower_kernel(hir))
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
