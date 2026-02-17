use crate::diagnostics::FileDiagnostic;
use crate::surface::Module;

mod builtins;
mod checker;
mod types;

mod check;
mod class_env;
mod elaborate;
mod global;
mod infer;
mod ordering;

#[cfg(test)]
mod class_constraints_tests;
#[cfg(test)]
mod expected_coercions_tests;

pub use check::{check_types, check_types_including_stdlib};
pub use elaborate::elaborate_expected_coercions;
pub use infer::infer_value_types;

pub(super) use checker::TypeChecker;
pub(super) use class_env::{ClassDeclInfo, InstanceDeclInfo};

// Exposed for integration points (CLI/LSP) without requiring them to depend on checker internals.
pub type TypeDiagnostics = Vec<FileDiagnostic>;
pub type ModuleList = Vec<Module>;
