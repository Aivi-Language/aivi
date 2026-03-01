pub mod builtin_names;
mod cranelift_backend;
mod i18n;
mod i18n_codegen;
pub mod intern;
mod mcp;
mod pm;
mod runtime;
mod rust_ir;

pub mod cst {
    pub use aivi_core::{CstBundle, CstFile, CstToken};
}

pub mod diagnostics {
    pub use aivi_core::{
        file_diagnostics_have_errors, render_diagnostics, Diagnostic, DiagnosticLabel,
        DiagnosticSeverity, FileDiagnostic, Position, Span,
    };
}

pub mod formatter {
    pub use aivi_core::{format_text, format_text_with_options, BraceStyle, FormatOptions};
}

pub mod surface {
    pub use aivi_core::{
        lower_modules_to_arena, parse_modules, parse_modules_from_tokens, resolve_import_names,
        ArenaBlockItem, ArenaBlockKind, ArenaClassDecl, ArenaClassMember, ArenaDecorator, ArenaDef,
        ArenaDomainDecl, ArenaDomainItem, ArenaExpr, ArenaInstanceDecl, ArenaListItem,
        ArenaLiteral, ArenaMachineDecl, ArenaMachineState, ArenaMachineTransition, ArenaMatchArm,
        ArenaModule, ArenaModuleItem, ArenaPathSegment, ArenaPattern, ArenaRecordField,
        ArenaRecordPatternField, ArenaScopeItem, ArenaTextPart, ArenaTypeAlias, ArenaTypeCtor,
        ArenaTypeDecl, ArenaTypeExpr, ArenaTypeSig, ArenaTypeVarConstraint, ArenaUseDecl, AstArena,
        BlockItem, BlockKind, ClassDecl, Decorator, Def, DomainDecl, DomainItem, Expr,
        InstanceDecl, ListItem, Literal, MatchArm, Module, ModuleItem, PathSegment, Pattern,
        RecordField, RecordPatternField, ScopeItemKind, SpannedName, SpannedSymbol, TextPart,
        TypeAlias, TypeCtor, TypeDecl, TypeExpr, TypeSig, UseDecl,
    };
}

pub mod hir {
    pub use aivi_core::{
        HirBlockItem, HirBlockKind, HirDef, HirExpr, HirListItem, HirLiteral, HirMatchArm,
        HirMockSubstitution, HirModule, HirPathSegment, HirPattern, HirProgram, HirRecordField,
        HirRecordPatternField, HirTextPart,
    };

    pub fn desugar_modules(modules: &[crate::surface::Module]) -> crate::hir::HirProgram {
        aivi_core::desugar_modules(modules)
    }
}

pub mod kernel {
    pub use aivi_core::desugar_blocks;
}

pub mod resolver {
    pub use aivi_core::check_modules;
}

pub mod typecheck {
    pub use aivi_core::{
        check_types, check_types_including_stdlib, elaborate_expected_coercions,
        elaborate_stdlib_checkpoint, elaborate_with_checkpoint, infer_value_types,
        infer_value_types_fast, infer_value_types_full, ElaborationCheckpoint, InferResult,
    };
}

pub mod cg_type {
    pub use aivi_core::cg_type::*;
}

pub mod stdlib {
    pub use aivi_core::{embedded_stdlib_modules, embedded_stdlib_source};
}

pub mod syntax {
    pub use aivi_core::syntax::*;
}

pub mod lexer {
    pub use aivi_core::lexer::*;
}

pub use aivi_core::desugar_modules;
pub use aivi_core::lex_cst;

pub use aivi_core::check_modules;
pub use aivi_core::desugar_blocks;
pub use aivi_core::{
    check_types, check_types_including_stdlib, elaborate_expected_coercions,
    elaborate_stdlib_checkpoint, elaborate_with_checkpoint, infer_value_types,
    infer_value_types_fast, infer_value_types_full, ElaborationCheckpoint, InferResult,
};
pub use aivi_core::{
    embedded_stdlib_modules, embedded_stdlib_source, lower_modules_to_arena, parse_modules,
    parse_modules_from_tokens, resolve_import_names, ArenaBlockItem, ArenaBlockKind,
    ArenaClassDecl, ArenaClassMember, ArenaDecorator, ArenaDef, ArenaDomainDecl, ArenaDomainItem,
    ArenaExpr, ArenaInstanceDecl, ArenaListItem, ArenaLiteral, ArenaMachineDecl, ArenaMachineState,
    ArenaMachineTransition, ArenaMatchArm, ArenaModule, ArenaModuleItem, ArenaPathSegment,
    ArenaPattern, ArenaRecordField, ArenaRecordPatternField, ArenaScopeItem, ArenaTextPart,
    ArenaTypeAlias, ArenaTypeCtor, ArenaTypeDecl, ArenaTypeExpr, ArenaTypeSig,
    ArenaTypeVarConstraint, ArenaUseDecl, AstArena, BlockItem, BlockKind, ClassDecl, Decorator,
    Def, DomainDecl, DomainItem, Expr, InstanceDecl, ListItem, Literal, MatchArm, Module,
    ModuleItem, PathSegment, Pattern, RecordField, RecordPatternField, SpannedName, SpannedSymbol,
    TextPart, TypeAlias, TypeCtor, TypeDecl, TypeExpr, TypeSig, UseDecl,
};
pub use aivi_core::{
    file_diagnostics_have_errors, render_diagnostics, Diagnostic, DiagnosticLabel,
    DiagnosticSeverity, FileDiagnostic, Position, Span,
};
pub use aivi_core::{format_text, format_text_with_options, BraceStyle, FormatOptions};
pub use aivi_core::{CstBundle, CstFile, CstToken};
pub use aivi_core::{HirModule, HirProgram};
use cranelift_backend::run_cranelift_jit_cancellable;
pub use cranelift_backend::{
    compile_to_object, destroy_aot_runtime, init_aot_runtime, init_aot_runtime_base,
    run_cranelift_jit, run_test_suite_jit,
};
pub use i18n_codegen::{
    generate_i18n_module_from_properties, parse_properties_catalog, PropertiesEntry,
};
pub use mcp::{
    bundled_specs_manifest, serve_mcp_stdio, serve_mcp_stdio_with_policy, McpManifest, McpPolicy,
    McpResource, McpTool,
};
pub use pm::{
    collect_aivi_sources, edit_cargo_toml_dependencies, ensure_aivi_dependency,
    parse_aivi_cargo_metadata, read_aivi_toml, validate_publish_preflight, write_scaffold,
    AiviCargoMetadata, AiviToml, AiviTomlBuild, AiviTomlProject, CargoDepSpec,
    CargoDepSpecParseError, CargoManifestEdits, NativeUiTarget, ProjectKind,
};
pub use runtime::{TestFailure, TestReport, TestSuccess};

/// Opaque handle for cancelling a running AIVI program from another thread.
/// Used by `--watch` mode to stop the current execution before restarting.
#[derive(Clone)]
pub struct CancelHandle {
    token: std::sync::Arc<runtime::CancelToken>,
}

impl Default for CancelHandle {
    fn default() -> Self {
        Self::new()
    }
}

impl CancelHandle {
    /// Create a new cancel handle.
    pub fn new() -> Self {
        Self {
            token: runtime::CancelToken::root(),
        }
    }

    /// Signal cancellation. Any runtime using this handle's token will stop.
    pub fn cancel(&self) {
        self.token.cancel();
    }
}

/// Run JIT with an external cancel handle so the caller can stop execution.
pub fn run_cranelift_jit_with_handle(
    program: HirProgram,
    cg_types: std::collections::HashMap<String, std::collections::HashMap<String, CgType>>,
    monomorph_plan: std::collections::HashMap<String, Vec<CgType>>,
    handle: &CancelHandle,
    surface_modules: &[aivi_core::Module],
) -> Result<(), AiviError> {
    run_cranelift_jit_cancellable(
        program,
        cg_types,
        monomorph_plan,
        handle.token.clone(),
        surface_modules,
    )
}

/// Run the AIVI test suite via JIT compilation.
pub fn run_test_suite(
    program: HirProgram,
    test_entries: &[(String, String)],
    surface_modules: &[aivi_core::Module],
    update_snapshots: bool,
    project_root: Option<std::path::PathBuf>,
) -> Result<TestReport, AiviError> {
    run_test_suite_jit(
        program,
        test_entries,
        surface_modules,
        update_snapshots,
        project_root,
    )
}
pub use rust_ir::cg_type::CgType;
pub use rust_ir::{lower_kernel as lower_rust_ir, RustIrProgram};

pub use aivi_driver::{
    desugar_target, desugar_target_lenient, desugar_target_typed, desugar_target_with_cg_types,
    desugar_target_with_cg_types_and_surface, format_target, kernel_target,
    load_module_diagnostics, load_modules, load_modules_from_paths, parse_file, parse_target,
    resolve_target, test_target_program_and_names, AiviError,
};

pub fn rust_ir_target(target: &str) -> Result<rust_ir::RustIrProgram, AiviError> {
    let hir = desugar_target_typed(target)?;
    let desugared = aivi_core::desugar_blocks(hir);
    rust_ir::lower_kernel(desugared)
}
