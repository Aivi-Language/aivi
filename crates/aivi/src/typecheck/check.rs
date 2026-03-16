use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};

use crate::diagnostics::FileDiagnostic;
use crate::surface::{DomainDecl, DomainItem, Module, ModuleItem, ScopeItemKind};

use super::checker::TypeChecker;
use super::class_env::{ClassDeclInfo, InstanceDeclInfo};
use super::types::Scheme;
use super::{
    build_module_interface, module_interface_from_setup, setup_module, ModuleInterface,
    ModuleInterfaceMaps,
};

use super::global::collect_global_type_info;
use super::ordering::ordered_modules;

pub fn check_types(modules: &[Module]) -> Vec<FileDiagnostic> {
    check_types_impl(modules, false)
}

pub fn check_types_including_stdlib(modules: &[Module]) -> Vec<FileDiagnostic> {
    check_types_impl(modules, true)
}

/// Cached stdlib type-setup maps for `check_types`.
/// Avoids re-running `setup_module` on all embedded stdlib modules per keystroke.
#[derive(Clone)]
pub struct CheckTypesCheckpoint {
    state: ModuleInterfaceMaps,
}

#[derive(Clone)]
pub struct CheckedModule {
    pub module_name: String,
    pub diagnostics: Vec<FileDiagnostic>,
    interface: ModuleInterface,
}

#[derive(Clone)]
pub struct CheckTypesIncrementalResult {
    pub diagnostics: Vec<FileDiagnostic>,
    pub checkpoint: CheckTypesCheckpoint,
    pub modules: Vec<CheckedModule>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModuleExportSurfaceSummary {
    pub fingerprint: u64,
    pub body_sensitive: bool,
}

/// Build a checkpoint by running type setup on stdlib (embedded) modules only.
/// Intended to be built once at LSP startup and reused for every keystroke.
pub fn check_types_stdlib_checkpoint(stdlib_modules: &[Module]) -> CheckTypesCheckpoint {
    let mut checker = TypeChecker::new();
    let mut state = ModuleInterfaceMaps::default();

    let (global_type_constructors, global_aliases, global_opaque_types) =
        collect_global_type_info(&mut checker, stdlib_modules);
    checker.set_global_type_info(
        global_type_constructors,
        global_aliases,
        global_opaque_types,
    );

    let mut discarded = Vec::new();
    for module in ordered_modules(stdlib_modules) {
        let setup = setup_module(
            &mut checker,
            module,
            &state.module_exports,
            &state.module_type_exports,
            &state.module_domain_exports,
            &state.module_class_exports,
            &state.module_instance_exports,
            &mut discarded,
        );
        // Don't check_module_defs — stdlib bodies may be incomplete in v0.1.
        let interface = module_interface_from_setup(module, &checker, &setup);
        state.apply_module_interface(&module.name.name, &interface);
    }

    CheckTypesCheckpoint { state }
}

/// Run type-checking using a pre-built stdlib checkpoint.
/// Skips `setup_module` for embedded stdlib modules; only processes user modules.
/// `modules` must contain all modules (stdlib + user).
pub fn check_types_with_checkpoint(
    modules: &[Module],
    checkpoint: &CheckTypesCheckpoint,
) -> Vec<FileDiagnostic> {
    check_types_with_checkpoint_incremental(modules, modules, checkpoint).diagnostics
}

/// Run type-checking on a subset of `modules`, using a checkpoint built from prior modules.
/// `all_modules` must contain the full active workspace so global type information stays correct.
pub fn check_types_with_checkpoint_incremental(
    all_modules: &[Module],
    modules: &[Module],
    checkpoint: &CheckTypesCheckpoint,
) -> CheckTypesIncrementalResult {
    let mut checker = TypeChecker::new();
    let mut diagnostics = Vec::new();
    let mut state = checkpoint.state.clone();
    let mut module_results = Vec::new();

    // collect_global_type_info is cheap (just extracts type names); run on all modules
    // so user-defined types are visible alongside stdlib types.
    let (global_type_constructors, global_aliases, global_opaque_types) =
        collect_global_type_info(&mut checker, all_modules);
    checker.set_global_type_info(
        global_type_constructors,
        global_aliases,
        global_opaque_types,
    );

    for module in ordered_modules(modules) {
        if module.path.starts_with("<embedded:") {
            // Stdlib already registered via checkpoint; skip setup_module entirely.
            continue;
        }
        let start = diagnostics.len();
        let setup = setup_module(
            &mut checker,
            module,
            &state.module_exports,
            &state.module_type_exports,
            &state.module_domain_exports,
            &state.module_class_exports,
            &state.module_instance_exports,
            &mut diagnostics,
        );
        let mut checked_env = setup.env.clone();
        let mut module_diags = checker.check_module_defs(module, &setup.sigs, &mut checked_env);
        diagnostics.append(&mut module_diags);
        let interface = build_module_interface(module, &checker, &setup.sigs, &checked_env);
        state.apply_module_interface(&module.name.name, &interface);
        module_results.push(CheckedModule {
            module_name: module.name.name.clone(),
            diagnostics: diagnostics[start..].to_vec(),
            interface,
        });
    }

    CheckTypesIncrementalResult {
        diagnostics,
        checkpoint: CheckTypesCheckpoint { state },
        modules: module_results,
    }
}

impl CheckTypesCheckpoint {
    pub fn empty() -> Self {
        Self {
            state: ModuleInterfaceMaps::default(),
        }
    }

    pub fn apply_cached_module(&mut self, module: &CheckedModule) {
        self.state
            .apply_module_interface(&module.module_name, &module.interface);
    }

    pub fn remove_module(&mut self, module_name: &str) {
        self.state.remove_module(module_name);
    }
}

pub fn summarize_module_export_surface(module: &Module) -> ModuleExportSurfaceSummary {
    let exported_values: HashSet<&str> = module
        .exports
        .iter()
        .filter(|export| export.kind == ScopeItemKind::Value)
        .map(|export| export.name.name.as_str())
        .collect();
    let exported_domains: HashSet<&str> = module
        .exports
        .iter()
        .filter(|export| export.kind == ScopeItemKind::Domain)
        .map(|export| export.name.name.as_str())
        .collect();
    let declared_value_sigs: HashSet<&str> = module
        .items
        .iter()
        .filter_map(|item| match item {
            ModuleItem::TypeSig(sig) => Some(sig.name.name.as_str()),
            _ => None,
        })
        .collect();
    let body_sensitive = exported_values
        .iter()
        .any(|name| !declared_value_sigs.contains(name))
        || module.items.iter().any(|item| match item {
            ModuleItem::DomainDecl(domain)
                if exported_domains.contains(domain.name.name.as_str()) =>
            {
                exported_domain_is_body_sensitive(domain)
            }
            _ => false,
        });

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    module.name.name.hash(&mut hasher);
    module.exports.len().hash(&mut hasher);
    format!("{:?}", module.exports).hash(&mut hasher);
    format!("{:?}", module.uses).hash(&mut hasher);
    format!("{:?}", module.annotations).hash(&mut hasher);

    if body_sensitive {
        format!("{:?}", module.items).hash(&mut hasher);
    } else {
        for item in &module.items {
            match item {
                ModuleItem::TypeSig(sig) if exported_values.contains(sig.name.name.as_str()) => {
                    format!("value-sig:{sig:?}").hash(&mut hasher);
                }
                ModuleItem::TypeDecl(decl) => format!("type-decl:{decl:?}").hash(&mut hasher),
                ModuleItem::TypeAlias(alias) => format!("type-alias:{alias:?}").hash(&mut hasher),
                ModuleItem::ClassDecl(class_decl) => {
                    format!("class-decl:{class_decl:?}").hash(&mut hasher)
                }
                ModuleItem::InstanceDecl(instance_decl) => {
                    format!("instance-decl:{instance_decl:?}").hash(&mut hasher)
                }
                ModuleItem::DomainDecl(domain)
                    if exported_domains.contains(domain.name.name.as_str()) =>
                {
                    format!("domain:{domain:?}").hash(&mut hasher);
                }
                _ => {}
            }
        }
    }

    ModuleExportSurfaceSummary {
        fingerprint: hasher.finish(),
        body_sensitive,
    }
}

fn exported_domain_is_body_sensitive(domain: &DomainDecl) -> bool {
    let declared_sigs: HashSet<&str> = domain
        .items
        .iter()
        .filter_map(|item| match item {
            DomainItem::TypeSig(sig) => Some(sig.name.name.as_str()),
            _ => None,
        })
        .collect();
    domain.items.iter().any(|item| match item {
        DomainItem::Def(def) | DomainItem::LiteralDef(def) => {
            !declared_sigs.contains(def.name.name.as_str())
        }
        DomainItem::TypeAlias(_) | DomainItem::TypeSig(_) => false,
    })
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::parse_modules;

    use super::summarize_module_export_surface;

    fn module_export_summary(source: &str) -> super::ModuleExportSurfaceSummary {
        let (modules, diagnostics) = parse_modules(Path::new("summary_test.aivi"), source);
        assert!(
            diagnostics.is_empty(),
            "expected parse success for summary test, got: {diagnostics:?}"
        );
        let module = modules
            .into_iter()
            .next()
            .expect("summary test should contain one module");
        summarize_module_export_surface(&module)
    }

    #[test]
    fn export_surface_summary_ignores_private_body_changes_for_annotated_exports() {
        let before = r#"
module test.summary
export value

helper = 1

value : Text
value = "ok"
"#;
        let after = r#"
module test.summary
export value

helper = 2

value : Text
value = "ok"
"#;

        let before_summary = module_export_summary(before);
        let after_summary = module_export_summary(after);

        assert!(
            !before_summary.body_sensitive && !after_summary.body_sensitive,
            "annotated exports should not force body-sensitive invalidation"
        );
        assert_eq!(
            before_summary.fingerprint, after_summary.fingerprint,
            "private body-only edits should not perturb the export surface fingerprint"
        );
    }

    #[test]
    fn export_surface_summary_stays_conservative_for_inferred_exports() {
        let before = r#"
module test.summary
export value

helper = "ok"
value = helper
"#;
        let after = r#"
module test.summary
export value

helper = "still ok"
value = helper
"#;

        let before_summary = module_export_summary(before);
        let after_summary = module_export_summary(after);

        assert!(
            before_summary.body_sensitive && after_summary.body_sensitive,
            "inferred exports should stay body-sensitive for correctness"
        );
        assert_ne!(
            before_summary.fingerprint, after_summary.fingerprint,
            "body-sensitive modules should invalidate dependents on private body edits"
        );
    }
}

fn check_types_impl(modules: &[Module], check_embedded_stdlib: bool) -> Vec<FileDiagnostic> {
    let mut checker = TypeChecker::new();
    let mut diagnostics = Vec::new();
    let mut module_exports: HashMap<String, HashMap<String, Vec<Scheme>>> = HashMap::new();
    let mut module_type_exports: HashMap<String, HashMap<String, super::TypeSurface>> =
        HashMap::new();
    let mut module_domain_exports: HashMap<String, HashMap<String, Vec<String>>> = HashMap::new();
    let mut module_class_exports: HashMap<String, HashMap<String, ClassDeclInfo>> = HashMap::new();
    let mut module_instance_exports: HashMap<String, Vec<InstanceDeclInfo>> = HashMap::new();

    let (global_type_constructors, global_aliases, global_opaque_types) =
        collect_global_type_info(&mut checker, modules);
    checker.set_global_type_info(
        global_type_constructors,
        global_aliases,
        global_opaque_types,
    );

    for module in ordered_modules(modules) {
        let setup = setup_module(
            &mut checker,
            module,
            &module_exports,
            &module_type_exports,
            &module_domain_exports,
            &module_class_exports,
            &module_instance_exports,
            &mut diagnostics,
        );
        let interface_env = if check_embedded_stdlib || !module.path.starts_with("<embedded:") {
            let mut checked_env = setup.env.clone();

            // v0.1 embedded stdlib is allowed to be incomplete; typechecking its bodies can
            // hang/crash. Still collect its signatures/classes/instances so user modules can
            // typecheck.
            let mut module_diags = checker.check_module_defs(module, &setup.sigs, &mut checked_env);
            diagnostics.append(&mut module_diags);
            checked_env
        } else {
            setup.env.clone()
        };

        let interface = build_module_interface(module, &checker, &setup.sigs, &interface_env);
        module_exports.insert(module.name.name.clone(), interface.exports.clone());
        module_type_exports.insert(module.name.name.clone(), interface.type_exports.clone());
        module_domain_exports.insert(module.name.name.clone(), interface.domain_exports.clone());
        module_class_exports.insert(module.name.name.clone(), interface.class_exports.clone());
        module_instance_exports
            .insert(module.name.name.clone(), interface.instance_exports.clone());
    }

    diagnostics
}
