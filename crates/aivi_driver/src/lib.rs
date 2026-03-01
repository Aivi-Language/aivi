#![deny(clippy::unwrap_used)]

mod workspace;

use std::fs;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::time::Instant;

use aivi_core::{
    check_modules, elaborate_expected_coercions, embedded_stdlib_modules,
    file_diagnostics_have_errors, format_text, parse_modules, parse_modules_from_tokens,
    render_diagnostics, resolve_import_names, CstBundle, CstFile, Diagnostic, FileDiagnostic,
    HirProgram, Module, ModuleItem,
};

type CgTypesMap = std::collections::HashMap<
    String,
    std::collections::HashMap<String, aivi_core::cg_type::CgType>,
>;
type MonomorphPlan = std::collections::HashMap<String, Vec<aivi_core::cg_type::CgType>>;

fn trace_timing() -> bool {
    std::env::var("AIVI_TRACE_TIMING").is_ok_and(|v| v == "1")
}

macro_rules! timing_step {
    ($trace:expr, $label:expr, $block:expr) => {{
        let _t0 = if $trace { Some(Instant::now()) } else { None };
        let result = $block;
        if let Some(t0) = _t0 {
            eprintln!(
                "[AIVI_TIMING] {:40} {:>8.1}ms",
                $label,
                t0.elapsed().as_secs_f64() * 1000.0
            );
        }
        result
    }};
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

/// Prints diagnostics to stderr so the user sees errors before a `Diagnostics` exit.
fn emit_diagnostics(diagnostics: &[FileDiagnostic]) {
    let use_color = std::io::stderr().is_terminal();
    for diag in diagnostics {
        let rendered = render_diagnostics(
            &diag.path,
            std::slice::from_ref(&diag.diagnostic),
            use_color,
        );
        if !rendered.is_empty() {
            eprintln!("{rendered}");
        }
    }
}

/// Expands a user target and parses each file into a CST bundle for editor/tooling entrypoints.
pub fn parse_target(target: &str) -> Result<CstBundle, AiviError> {
    let mut files = Vec::new();
    let paths = workspace::expand_target(target)?;
    for path in paths {
        files.push(parse_file(&path)?);
    }
    Ok(CstBundle { files })
}

/// Reads one source file, lexes/parses it, and returns file metadata plus diagnostics.
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

/// Resolves a target and loads parsed modules, including embedded stdlib modules for full compilation context.
pub fn load_modules(target: &str) -> Result<Vec<Module>, AiviError> {
    let paths = workspace::expand_target(target)?;
    load_modules_from_paths(&paths)
}

/// Parses modules from explicit file paths and prepends embedded stdlib modules for downstream phases.
pub fn load_modules_from_paths(paths: &[PathBuf]) -> Result<Vec<Module>, AiviError> {
    let mut modules = Vec::new();
    for path in paths {
        let content = fs::read_to_string(path)?;
        let (mut file_modules, _) = parse_modules(path.as_path(), &content);
        modules.append(&mut file_modules);
    }
    let mut stdlib_modules = embedded_stdlib_modules();
    stdlib_modules.append(&mut modules);
    resolve_import_names(&mut stdlib_modules);
    Ok(stdlib_modules)
}

/// Collects parser diagnostics for all files in a target before semantic/type phases run.
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

/// Produces desugared HIR for a target after ensuring syntax diagnostics are clean.
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
    resolve_import_names(&mut stdlib_modules);
    Ok(aivi_core::desugar_modules(&stdlib_modules))
}

/// Like [`desugar_target`] but skips the diagnostic pre-check so files with
/// parse warnings/errors are still desugared (best-effort).  Useful for codegen
/// tests that want to exercise as much of the pipeline as possible even when
/// some integration-test files have known issues.
pub fn desugar_target_lenient(target: &str) -> Result<HirProgram, AiviError> {
    let paths = workspace::expand_target(target)?;
    let mut modules = Vec::new();
    for path in &paths {
        let content = fs::read_to_string(path)?;
        let (mut parsed, _) = parse_modules(path.as_path(), &content);
        modules.append(&mut parsed);
    }
    let mut stdlib_modules = embedded_stdlib_modules();
    stdlib_modules.append(&mut modules);
    resolve_import_names(&mut stdlib_modules);
    Ok(aivi_core::desugar_modules(&stdlib_modules))
}

/// Builds a test-only program view by finding `@test` definitions and validating their modules.
#[allow(clippy::type_complexity)]
pub fn test_target_program_and_names(
    target: &str,
    check_stdlib: bool,
) -> Result<(HirProgram, Vec<(String, String)>, Vec<FileDiagnostic>), AiviError> {
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

    let mut test_entries: Vec<(String, String)> = Vec::new();
    for path in &test_paths {
        let content = fs::read_to_string(path)?;
        let (modules, _) = parse_modules(path.as_path(), &content);
        for module in modules {
            for item in module.items {
                let ModuleItem::Def(def) = item else {
                    continue;
                };
                if let Some(dec) = def.decorators.iter().find(|d| d.name.name == "test") {
                    let name = format!("{}.{}", module.name.name, def.name.name);
                    let description = match &dec.arg {
                        Some(aivi_core::Expr::Literal(aivi_core::Literal::String {
                            text, ..
                        })) => text.clone(),
                        _ => name.clone(),
                    };
                    test_entries.push((name, description));
                }
            }
        }
    }
    test_entries.sort();
    test_entries.dedup();
    if test_entries.is_empty() {
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
    Ok((program, test_entries, diagnostics))
}

/// Produces typed desugared HIR by running syntax checks, type checks, and expected coercion elaboration.
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
    resolve_import_names(&mut stdlib_modules);

    let mut diagnostics = check_modules(&stdlib_modules);
    if diagnostics.is_empty() {
        diagnostics.extend(elaborate_expected_coercions(&mut stdlib_modules));
    }
    if file_diagnostics_have_errors(&diagnostics) {
        return Err(AiviError::Diagnostics);
    }

    Ok(aivi_core::desugar_modules(&stdlib_modules))
}

/// Like `desugar_target_typed` but also runs type inference and returns the `CgType` map
/// for each module/definition. Used by the typed codegen path.
pub fn desugar_target_with_cg_types(
    target: &str,
) -> Result<(HirProgram, CgTypesMap, MonomorphPlan), AiviError> {
    let diagnostics = load_module_diagnostics(target)?;
    if file_diagnostics_have_errors(&diagnostics) {
        emit_diagnostics(&diagnostics);
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
    resolve_import_names(&mut stdlib_modules);

    let mut diagnostics = check_modules(&stdlib_modules);
    if diagnostics.is_empty() {
        diagnostics.extend(elaborate_expected_coercions(&mut stdlib_modules));
    }
    if file_diagnostics_have_errors(&diagnostics) {
        emit_diagnostics(&diagnostics);
        return Err(AiviError::Diagnostics);
    }

    let infer_result = aivi_core::infer_value_types_full(&stdlib_modules);
    let program = aivi_core::desugar_modules(&stdlib_modules);
    Ok((program, infer_result.cg_types, infer_result.monomorph_plan))
}

/// Like [`desugar_target_with_cg_types`] but also returns the surface modules
/// so the caller can process machine declarations, constructor ordinals, etc.
pub fn desugar_target_with_cg_types_and_surface(
    target: &str,
) -> Result<(HirProgram, CgTypesMap, MonomorphPlan, Vec<Module>), AiviError> {
    let trace = trace_timing();
    let t_total = if trace { Some(Instant::now()) } else { None };

    // Parse user files and collect diagnostics in one pass (avoid double-parsing).
    let paths = timing_step!(trace, "expand_target", workspace::expand_target(target)?);
    let mut modules = Vec::new();
    let mut parse_diagnostics = Vec::new();
    timing_step!(trace, "parse user files", {
        for path in &paths {
            let content = fs::read_to_string(path)?;
            let (mut parsed, mut diags) = parse_modules(path.as_path(), &content);
            parse_diagnostics.append(&mut diags);
            modules.append(&mut parsed);
        }
    });
    if file_diagnostics_have_errors(&parse_diagnostics) {
        emit_diagnostics(&parse_diagnostics);
        return Err(AiviError::Diagnostics);
    }

    let mut stdlib_modules = timing_step!(
        trace,
        "parse stdlib (embedded_stdlib_modules)",
        embedded_stdlib_modules()
    );
    stdlib_modules.append(&mut modules);
    timing_step!(
        trace,
        "resolve_import_names",
        resolve_import_names(&mut stdlib_modules)
    );

    let mut diagnostics = timing_step!(
        trace,
        "check_modules (name resolution)",
        check_modules(&stdlib_modules)
    );
    if diagnostics.is_empty() {
        diagnostics.extend(timing_step!(
            trace,
            "elaborate_expected_coercions",
            elaborate_expected_coercions(&mut stdlib_modules)
        ));
    }
    if file_diagnostics_have_errors(&diagnostics) {
        emit_diagnostics(&diagnostics);
        return Err(AiviError::Diagnostics);
    }

    // Use the fast path: skip body-checking embedded stdlib modules. They are pre-verified at
    // compiler build time and have explicit type signatures, so full re-inference is wasteful.
    let infer_result = timing_step!(
        trace,
        "infer_value_types_fast",
        aivi_core::infer_value_types_fast(&stdlib_modules)
    );
    let program = timing_step!(
        trace,
        "desugar_modules (HIR)",
        aivi_core::desugar_modules(&stdlib_modules)
    );

    if let Some(t0) = t_total {
        eprintln!(
            "[AIVI_TIMING] {:40} {:>8.1}ms  ← TOTAL frontend",
            "frontend pipeline",
            t0.elapsed().as_secs_f64() * 1000.0
        );
    }

    Ok((
        program,
        infer_result.cg_types,
        infer_result.monomorph_plan,
        stdlib_modules,
    ))
}

/// Lowers a typed HIR program through block desugaring for backend code generation.
pub fn kernel_target(target: &str) -> Result<HirProgram, AiviError> {
    let hir = desugar_target_typed(target)?;
    Ok(aivi_core::desugar_blocks(hir))
}

/// Formats exactly one target file via the shared formatter used by CLI and tooling.
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

/// Resolves a target string into concrete source file paths consumed by driver commands.
pub fn resolve_target(target: &str) -> Result<Vec<PathBuf>, AiviError> {
    workspace::expand_target(target)
}
