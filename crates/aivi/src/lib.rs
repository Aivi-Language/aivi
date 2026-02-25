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
        lower_modules_to_arena, parse_modules, parse_modules_from_tokens, ArenaBlockItem,
        ArenaBlockKind, ArenaClassDecl, ArenaClassMember, ArenaDecorator, ArenaDef,
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
        HirModule, HirPathSegment, HirPattern, HirProgram, HirRecordField, HirRecordPatternField,
        HirTextPart,
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
        infer_value_types_full, InferResult,
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
pub use aivi_core::{
    check_types, check_types_including_stdlib, elaborate_expected_coercions, infer_value_types,
    infer_value_types_full, InferResult,
};
pub use aivi_core::{
    embedded_stdlib_modules, embedded_stdlib_source, lower_modules_to_arena, parse_modules,
    parse_modules_from_tokens, ArenaBlockItem, ArenaBlockKind, ArenaClassDecl, ArenaClassMember,
    ArenaDecorator, ArenaDef, ArenaDomainDecl, ArenaDomainItem, ArenaExpr, ArenaInstanceDecl,
    ArenaListItem, ArenaLiteral, ArenaMachineDecl, ArenaMachineState, ArenaMachineTransition,
    ArenaMatchArm, ArenaModule, ArenaModuleItem, ArenaPathSegment, ArenaPattern, ArenaRecordField,
    ArenaRecordPatternField, ArenaScopeItem, ArenaTextPart, ArenaTypeAlias, ArenaTypeCtor,
    ArenaTypeDecl, ArenaTypeExpr, ArenaTypeSig, ArenaTypeVarConstraint, ArenaUseDecl, AstArena,
    BlockItem, BlockKind, ClassDecl, Decorator, Def, DomainDecl, DomainItem, Expr, InstanceDecl,
    ListItem, Literal, MatchArm, Module, ModuleItem, PathSegment, Pattern, RecordField,
    RecordPatternField, SpannedName, SpannedSymbol, TextPart, TypeAlias, TypeCtor, TypeDecl,
    TypeExpr, TypeSig, UseDecl,
};
pub use aivi_core::{
    file_diagnostics_have_errors, render_diagnostics, Diagnostic, DiagnosticLabel,
    DiagnosticSeverity, FileDiagnostic, Position, Span,
};
pub use aivi_core::{format_text, format_text_with_options, BraceStyle, FormatOptions};
pub use aivi_core::{
    lower_kernel, KernelBlockItem, KernelBlockKind, KernelDef, KernelExpr, KernelListItem,
    KernelLiteral, KernelMatchArm, KernelModule, KernelPathSegment, KernelPattern, KernelProgram,
    KernelRecordField, KernelRecordPatternField, KernelTextPart,
};
pub use aivi_core::{CstBundle, CstFile, CstToken};
pub use aivi_core::{HirModule, HirProgram};
pub use cranelift_backend::{
    compile_to_object, destroy_aot_runtime, init_aot_runtime, init_aot_runtime_base,
    run_cranelift_jit,
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
pub use runtime::{run_test_suite, TestFailure, TestReport, TestSuccess};
pub use rust_ir::cg_type::CgType;
pub use rust_ir::{lower_kernel as lower_rust_ir, RustIrProgram};

pub use aivi_driver::{
    desugar_target, desugar_target_lenient, desugar_target_typed, desugar_target_with_cg_types,
    format_target, kernel_target, load_module_diagnostics, load_modules, load_modules_from_paths,
    parse_file, parse_target, resolve_target, test_target_program_and_names, AiviError,
};

pub fn rust_ir_target(target: &str) -> Result<rust_ir::RustIrProgram, AiviError> {
    let kernel = kernel_target(target)?;
    rust_ir::lower_kernel(kernel)
}
