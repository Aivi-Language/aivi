#![deny(clippy::unwrap_used)]

// NOTE: This crate is an incremental extraction of the pure compiler logic from `crates/aivi`.
// For now it reuses the existing module implementation files via `#[path = "..")]` to avoid a
// huge, noisy move diff. Once the crate boundary is stable, we can physically move files.

#[path = "../../aivi/src/cst.rs"]
mod cst;
#[path = "../../aivi/src/diagnostics.rs"]
mod diagnostics;
#[path = "../../aivi/src/formatter.rs"]
mod formatter;
#[path = "../../aivi/src/hir.rs"]
mod hir;
#[path = "../../aivi/src/kernel.rs"]
mod kernel;
#[path = "../../aivi/src/lexer.rs"]
pub mod lexer;
#[path = "../../aivi/src/resolver.rs"]
mod resolver;
#[path = "../../aivi/src/stdlib/mod.rs"]
mod stdlib;
#[path = "../../aivi/src/surface/mod.rs"]
mod surface;
#[path = "../../aivi/src/syntax.rs"]
pub mod syntax;
#[path = "../../aivi/src/typecheck/mod.rs"]
mod typecheck;

pub use cst::{CstBundle, CstFile, CstToken};
pub use diagnostics::{
    file_diagnostics_have_errors, render_diagnostics, Diagnostic, DiagnosticLabel,
    DiagnosticSeverity, FileDiagnostic, Position, Span,
};
pub use formatter::{format_text, format_text_with_options, BraceStyle, FormatOptions};
pub use hir::{HirModule, HirProgram};
pub use kernel::{
    lower_hir as lower_kernel, KernelBlockItem, KernelBlockKind, KernelDef, KernelExpr,
    KernelListItem, KernelLiteral, KernelMatchArm, KernelModule, KernelPathSegment, KernelPattern,
    KernelProgram, KernelRecordField, KernelRecordPatternField, KernelTextPart,
};
pub use resolver::check_modules;
pub use stdlib::{embedded_stdlib_modules, embedded_stdlib_source};
pub use surface::{
    parse_modules, parse_modules_from_tokens, BlockItem, BlockKind, ClassDecl, Decorator, Def,
    DomainDecl, DomainItem, Expr, InstanceDecl, ListItem, Literal, MatchArm, Module, ModuleItem,
    PathSegment, Pattern, RecordField, RecordPatternField, SpannedName, TextPart, TypeAlias,
    TypeCtor, TypeDecl, TypeExpr, TypeSig, UseDecl,
};
pub use typecheck::{
    check_types, check_types_including_stdlib, elaborate_expected_coercions, infer_value_types,
};

pub fn desugar_modules(modules: &[Module]) -> HirProgram {
    hir::desugar_modules(modules)
}

pub fn lex_cst(content: &str) -> (Vec<CstToken>, Vec<Diagnostic>) {
    lexer::lex(content)
}
