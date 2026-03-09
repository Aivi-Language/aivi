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
    HirProgram, Module,
};
pub use workspace::{AssemblyStats, FrontendAssembly, FrontendAssemblyMode, WorkspaceSession};

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

// ---------------------------------------------------------------------------
// Pipeline — single configurable entry point for all compilation stages
// ---------------------------------------------------------------------------

/// Holds parsed modules (user + stdlib) and parse-phase diagnostics.
/// All `desugar_*` / `kernel_target` / `load_*` functions delegate to this.
pub struct Pipeline {
    modules: Vec<Module>,
    parse_diagnostics: Vec<FileDiagnostic>,
}

impl Pipeline {
    /// Parse user files from a target string, prepend stdlib, resolve imports.
    pub fn from_target(target: &str) -> Result<Self, AiviError> {
        let paths = workspace::expand_target(target)?;
        Self::from_paths(&paths)
    }

    /// Parse user files from explicit paths, prepend stdlib, resolve imports.
    pub fn from_paths(paths: &[PathBuf]) -> Result<Self, AiviError> {
        let mut modules = Vec::new();
        let mut parse_diagnostics = Vec::new();
        for path in paths {
            let content = fs::read_to_string(path)?;
            let (mut parsed, mut diags) = parse_modules(path.as_path(), &content);
            parse_diagnostics.append(&mut diags);
            modules.append(&mut parsed);
        }
        let mut stdlib_modules = embedded_stdlib_modules();
        stdlib_modules.append(&mut modules);
        resolve_import_names(&mut stdlib_modules);
        Ok(Self {
            modules: stdlib_modules,
            parse_diagnostics,
        })
    }

    pub fn parse_diagnostics(&self) -> &[FileDiagnostic] {
        &self.parse_diagnostics
    }

    pub fn has_parse_errors(&self) -> bool {
        file_diagnostics_have_errors(&self.parse_diagnostics)
    }

    /// Run name-resolution checks and coercion elaboration. Returns check diagnostics.
    pub fn typecheck(&mut self) -> Vec<FileDiagnostic> {
        let mut diags = check_modules(&self.modules);
        if diags.is_empty() {
            diags.extend(elaborate_expected_coercions(&mut self.modules));
        }
        diags
    }

    pub fn infer_types_full(&self) -> aivi_core::InferResult {
        aivi_core::infer_value_types_full(&self.modules)
    }

    pub fn infer_types_fast(&self) -> aivi_core::InferResult {
        aivi_core::infer_value_types_fast(&self.modules)
    }

    pub fn desugar(&self) -> HirProgram {
        aivi_core::desugar_modules(&self.modules)
    }

    pub fn modules(&self) -> &[Module] {
        &self.modules
    }

    pub fn into_modules(self) -> Vec<Module> {
        self.modules
    }
}

// ---------------------------------------------------------------------------
// Public convenience functions (delegate to Pipeline)
// ---------------------------------------------------------------------------

/// Resolves a target and loads parsed modules with embedded stdlib.
pub fn load_modules(target: &str) -> Result<Vec<Module>, AiviError> {
    Ok(Pipeline::from_target(target)?.into_modules())
}

/// Parses modules from explicit file paths with embedded stdlib.
pub fn load_modules_from_paths(paths: &[PathBuf]) -> Result<Vec<Module>, AiviError> {
    Ok(Pipeline::from_paths(paths)?.into_modules())
}

/// Collects parser diagnostics for all files in a target.
pub fn load_module_diagnostics(target: &str) -> Result<Vec<FileDiagnostic>, AiviError> {
    Ok(Pipeline::from_target(target)?.parse_diagnostics().to_vec())
}

/// Produces desugared HIR after ensuring parse diagnostics are clean.
pub fn desugar_target(target: &str) -> Result<HirProgram, AiviError> {
    let pipeline = Pipeline::from_target(target)?;
    if pipeline.has_parse_errors() {
        return Err(AiviError::Diagnostics);
    }
    Ok(pipeline.desugar())
}

/// Like [`desugar_target`] but skips the diagnostic pre-check (best-effort).
pub fn desugar_target_lenient(target: &str) -> Result<HirProgram, AiviError> {
    Ok(Pipeline::from_target(target)?.desugar())
}

/// Produces typed desugared HIR (parse check → typecheck → elaborate → desugar).
pub fn desugar_target_typed(target: &str) -> Result<HirProgram, AiviError> {
    let mut pipeline = Pipeline::from_target(target)?;
    if pipeline.has_parse_errors() {
        return Err(AiviError::Diagnostics);
    }
    let diags = pipeline.typecheck();
    if file_diagnostics_have_errors(&diags) {
        return Err(AiviError::Diagnostics);
    }
    Ok(pipeline.desugar())
}

/// Typed HIR plus `CgType` map for the typed codegen path.
pub fn desugar_target_with_cg_types(
    target: &str,
) -> Result<(HirProgram, CgTypesMap, MonomorphPlan), AiviError> {
    let (program, cg_types, monomorph_plan, _) = desugar_target_with_cg_types_and_surface(target)?;
    Ok((program, cg_types, monomorph_plan))
}

/// Like [`desugar_target_with_cg_types`] but also returns the surface modules
/// and uses optional timing instrumentation (`AIVI_TRACE_TIMING=1`).
pub fn desugar_target_with_cg_types_and_surface(
    target: &str,
) -> Result<(HirProgram, CgTypesMap, MonomorphPlan, Vec<Module>), AiviError> {
    let trace = trace_timing();
    let t_total = if trace { Some(Instant::now()) } else { None };
    let mut session = WorkspaceSession::new();
    let assembly = timing_step!(trace, "frontend assembly", {
        session.assemble_target(target, FrontendAssemblyMode::InferFast)?
    });
    typed_codegen_from_assembly(assembly, trace, t_total)
}

/// Like [`desugar_target_with_cg_types_and_surface`] but reuses a caller-owned
/// [`WorkspaceSession`] so repeated invocations can preserve frontend caches.
pub fn desugar_target_with_cg_types_and_surface_in_session(
    session: &mut WorkspaceSession,
    target: &str,
) -> Result<(HirProgram, CgTypesMap, MonomorphPlan, Vec<Module>), AiviError> {
    let trace = trace_timing();
    let t_total = if trace { Some(Instant::now()) } else { None };
    let assembly = timing_step!(trace, "frontend assembly", {
        session.assemble_target(target, FrontendAssemblyMode::InferFast)?
    });
    typed_codegen_from_assembly(assembly, trace, t_total)
}

fn typed_codegen_from_assembly(
    assembly: FrontendAssembly,
    trace: bool,
    t_total: Option<Instant>,
) -> Result<(HirProgram, CgTypesMap, MonomorphPlan, Vec<Module>), AiviError> {
    if file_diagnostics_have_errors(&assembly.parse_diagnostics) {
        emit_diagnostics(&assembly.parse_diagnostics);
        return Err(AiviError::Diagnostics);
    }
    let mut diagnostics = assembly.resolver_diagnostics.clone();
    diagnostics.extend(assembly.typecheck_diagnostics.clone());
    if let Some(inference) = &assembly.inference {
        diagnostics.extend(inference.diagnostics.clone());
    }
    if file_diagnostics_have_errors(&diagnostics) {
        emit_diagnostics(&diagnostics);
        return Err(AiviError::Diagnostics);
    }
    let program = timing_step!(trace, "desugar_modules (HIR)", assembly.desugar());
    let infer = assembly
        .inference
        .expect("infer-fast assembly should always include inference");
    if let Some(t0) = t_total {
        eprintln!(
            "[AIVI_TIMING] {:40} {:>8.1}ms  ← TOTAL frontend",
            "frontend pipeline",
            t0.elapsed().as_secs_f64() * 1000.0
        );
    }
    Ok((
        program,
        infer.cg_types,
        infer.monomorph_plan,
        assembly.modules,
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

/// Assembles a target through the shared incremental frontend session API.
pub fn assemble_target(
    target: &str,
    mode: FrontendAssemblyMode,
) -> Result<FrontendAssembly, AiviError> {
    let mut session = WorkspaceSession::new();
    session.assemble_target(target, mode)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("workspace root")
            .to_path_buf()
    }

    fn existing_aivi_file() -> PathBuf {
        workspace_root().join("integration-tests/syntax/bindings/basic.aivi")
    }

    // -- AiviError display --

    #[test]
    fn error_invalid_path_display() {
        let e = AiviError::InvalidPath("no/such/path".to_string());
        assert!(e.to_string().contains("no/such/path"));
    }

    #[test]
    fn error_diagnostics_display() {
        let e = AiviError::Diagnostics;
        assert!(!e.to_string().is_empty());
    }

    #[test]
    fn error_invalid_command_display() {
        let e = AiviError::InvalidCommand("bad cmd".to_string());
        assert!(e.to_string().contains("bad cmd"));
    }

    #[test]
    fn error_cargo_display() {
        let e = AiviError::Cargo("oops".to_string());
        assert!(e.to_string().contains("oops"));
    }

    #[test]
    fn error_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
        let e: AiviError = io_err.into();
        assert!(e.to_string().contains("IO error"));
    }

    // -- resolve_target --

    #[test]
    fn resolve_target_returns_existing_file() {
        let path = existing_aivi_file();
        let target = path.to_string_lossy().to_string();
        let paths = resolve_target(&target).expect("resolve");
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0], path);
    }

    #[test]
    fn resolve_target_nonexistent_errors() {
        let result = resolve_target("/no/such/path/nowhere.aivi");
        assert!(result.is_err());
    }

    // -- parse_target --

    #[test]
    fn parse_target_single_file() {
        let path = existing_aivi_file();
        let target = path.to_string_lossy().to_string();
        let bundle = parse_target(&target).expect("parse_target");
        assert!(!bundle.files.is_empty());
    }

    // -- parse_file --

    #[test]
    fn parse_file_reads_and_parses() {
        let path = existing_aivi_file();
        let cst_file = parse_file(&path).expect("parse_file");
        assert!(cst_file.line_count > 0);
        assert!(cst_file.byte_count > 0);
    }

    // -- format_target --

    #[test]
    fn format_target_returns_formatted_text() {
        let path = existing_aivi_file();
        let target = path.to_string_lossy().to_string();
        let formatted = format_target(&target).expect("format_target");
        assert!(!formatted.is_empty());
    }

    // -- load_modules / Pipeline --

    #[test]
    fn pipeline_from_paths_loads_modules() {
        let path = existing_aivi_file();
        let pipeline = Pipeline::from_paths(&[path]).expect("from_paths");
        assert!(!pipeline.modules().is_empty());
    }

    #[test]
    fn pipeline_has_parse_errors_false_for_valid_file() {
        let path = existing_aivi_file();
        let pipeline = Pipeline::from_paths(&[path]).expect("from_paths");
        assert!(!pipeline.has_parse_errors());
    }

    #[test]
    fn load_modules_from_paths_returns_modules() {
        let path = existing_aivi_file();
        let modules = load_modules_from_paths(&[path]).expect("load_modules");
        assert!(!modules.is_empty());
    }

    #[test]
    fn desugar_target_lenient_succeeds() {
        let path = existing_aivi_file();
        let target = path.to_string_lossy().to_string();
        let _hir = desugar_target_lenient(&target).expect("desugar_lenient");
    }

    #[test]
    fn load_module_diagnostics_for_valid_file() {
        let path = existing_aivi_file();
        let target = path.to_string_lossy().to_string();
        let diags = load_module_diagnostics(&target).expect("diagnostics");
        let _ = diags;
    }
}
