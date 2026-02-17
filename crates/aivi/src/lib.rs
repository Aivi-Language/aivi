mod i18n;
mod i18n_codegen;
mod mcp;
mod native_rust_backend;
mod pm;
mod runtime;
mod rust_codegen;
mod rust_ir;
mod rustc_backend;

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
    pub use aivi_core::{
        format_text, format_text_with_options, BraceStyle, FormatOptions,
    };
}

pub mod surface {
    pub use aivi_core::{
        parse_modules, parse_modules_from_tokens, BlockItem, BlockKind, ClassDecl, Decorator, Def,
        DomainDecl, DomainItem, Expr, InstanceDecl, ListItem, Literal, MatchArm, Module,
        ModuleItem, PathSegment, Pattern, RecordField, RecordPatternField, SpannedName, TextPart,
        TypeAlias, TypeCtor, TypeDecl, TypeExpr, TypeSig, UseDecl,
    };
}

pub mod hir {
    pub use aivi_core::{
        HirBlockItem, HirBlockKind, HirDef, HirExpr, HirListItem, HirLiteral, HirMatchArm,
        HirModule, HirPathSegment, HirPattern, HirProgram, HirRecordField,
        HirRecordPatternField, HirTextPart,
    };

    pub fn desugar_modules(modules: &[crate::surface::Module]) -> crate::hir::HirProgram {
        aivi_core::desugar_modules(modules)
    }
}

pub mod kernel {
    pub use aivi_core::{
        KernelBlockItem, KernelBlockKind, KernelDef, KernelExpr, KernelListItem, KernelLiteral,
        KernelMatchArm, KernelModule, KernelPathSegment, KernelPattern, KernelProgram,
        KernelRecordField, KernelRecordPatternField, KernelTextPart,
    };

    pub fn lower_hir(program: crate::hir::HirProgram) -> crate::kernel::KernelProgram {
        aivi_core::lower_kernel(program)
    }
}

pub mod resolver {
    pub use aivi_core::check_modules;
}

pub mod typecheck {
    pub use aivi_core::{
        check_types, check_types_including_stdlib, elaborate_expected_coercions, infer_value_types,
    };
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

pub use aivi_core::{CstBundle, CstFile, CstToken};
pub use aivi_core::{
    file_diagnostics_have_errors, render_diagnostics, Diagnostic, DiagnosticLabel,
    DiagnosticSeverity, FileDiagnostic, Position, Span,
};
pub use aivi_core::{format_text, format_text_with_options, BraceStyle, FormatOptions};
pub use aivi_core::{HirModule, HirProgram};
pub use i18n_codegen::{
    generate_i18n_module_from_properties, parse_properties_catalog, PropertiesEntry,
};
pub use aivi_core::{
    lower_kernel, KernelBlockItem, KernelBlockKind, KernelDef, KernelExpr, KernelListItem,
    KernelLiteral, KernelMatchArm, KernelModule, KernelPathSegment, KernelPattern, KernelProgram,
    KernelRecordField, KernelRecordPatternField, KernelTextPart,
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
pub use aivi_core::check_modules;
pub use runtime::{run_native, run_native_with_fuel, run_test_suite, TestFailure, TestReport};
pub use rust_codegen::{compile_rust_native, compile_rust_native_lib};
pub use rust_ir::{lower_kernel as lower_rust_ir, RustIrProgram};
pub use rustc_backend::{build_with_rustc, emit_rustc_source};
pub use aivi_core::{
    embedded_stdlib_modules, embedded_stdlib_source, parse_modules, parse_modules_from_tokens,
    BlockItem, BlockKind, ClassDecl, Decorator, Def, DomainDecl, DomainItem, Expr, InstanceDecl,
    ListItem, Literal, MatchArm, Module, ModuleItem, PathSegment, Pattern, RecordField,
    RecordPatternField, SpannedName, TextPart, TypeAlias, TypeCtor, TypeDecl, TypeExpr, TypeSig,
    UseDecl,
};
pub use aivi_core::{
    check_types, check_types_including_stdlib, elaborate_expected_coercions, infer_value_types,
};

pub use aivi_driver::{
    desugar_target, desugar_target_typed, format_target, kernel_target, load_module_diagnostics,
    load_modules, load_modules_from_paths, parse_file, parse_target, resolve_target,
    test_target_program_and_names, AiviError,
};

pub fn rust_ir_target(target: &str) -> Result<rust_ir::RustIrProgram, AiviError> {
    let kernel = kernel_target(target)?;
    rust_ir::lower_kernel(kernel)
}
