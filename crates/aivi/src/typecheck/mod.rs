use std::collections::{HashMap, HashSet};

use crate::diagnostics::FileDiagnostic;
use crate::surface::Module;

mod builtins;
mod checker;
mod constraints;
mod query_engine;
mod types;

mod check;
mod class_env;
mod elaborate;
mod global;
mod infer;
mod ordering;

#[cfg(test)]
mod builtins_parity_tests;
#[cfg(test)]
mod class_constraints_tests;
#[cfg(test)]
mod expected_coercions_tests;

pub use check::{
    check_types, check_types_including_stdlib, check_types_stdlib_checkpoint,
    check_types_with_checkpoint, CheckTypesCheckpoint,
};
pub use elaborate::{
    elaborate_expected_coercions, elaborate_stdlib_checkpoint, elaborate_with_checkpoint,
    ElaborationCheckpoint,
};
pub use infer::{infer_value_types, infer_value_types_fast, infer_value_types_full, InferResult};

use checker::TypeChecker;
use class_env::{
    collect_imported_class_env, collect_local_class_env, expand_classes,
    synthesize_auto_forward_instances, ClassDeclInfo, InstanceDeclInfo,
};
use types::{Scheme, TypeEnv};

/// Result of per-module registration: the local type environment and collected type signatures.
struct ModuleSetup {
    env: TypeEnv,
    sigs: HashMap<String, Vec<Scheme>>,
}

/// Runs the full per-module registration sequence shared by all type-checking passes:
/// reset → register types → collect type-expr diagnostics → collect signatures →
/// register constructors → register imports → build class env → register defs.
fn setup_module(
    checker: &mut TypeChecker,
    module: &Module,
    module_exports: &HashMap<String, HashMap<String, Vec<Scheme>>>,
    module_domain_exports: &HashMap<String, HashMap<String, Vec<String>>>,
    module_class_exports: &HashMap<String, HashMap<String, ClassDeclInfo>>,
    module_instance_exports: &HashMap<String, Vec<InstanceDeclInfo>>,
    diagnostics: &mut Vec<FileDiagnostic>,
) -> ModuleSetup {
    checker.reset_module_context(module);
    let mut env = checker.builtins.clone();
    checker.register_module_types(module);
    diagnostics.extend(checker.collect_type_expr_diags(module));
    let sigs = checker.collect_type_sigs(module);
    checker.register_module_constructors(module, &mut env);
    checker.register_imports(module, module_exports, module_domain_exports, &mut env);

    let (imported_classes, imported_instances) =
        collect_imported_class_env(module, module_class_exports, module_instance_exports);
    let (local_classes, local_instances) = collect_local_class_env(module);
    let local_class_names: HashSet<String> = local_classes.keys().cloned().collect();
    let mut classes = imported_classes;
    classes.extend(local_classes);
    let classes = expand_classes(classes);
    let mut instances: Vec<InstanceDeclInfo> = imported_instances
        .into_iter()
        .filter(|inst| !local_class_names.contains(&inst.class_name))
        .collect();
    instances.extend(local_instances);
    instances.extend(synthesize_auto_forward_instances(module, &instances));
    checker.set_class_env(classes, instances);
    checker.register_module_defs(module, &sigs, &mut env);

    ModuleSetup { env, sigs }
}
