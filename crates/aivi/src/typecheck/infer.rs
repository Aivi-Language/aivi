use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};

use crate::cg_type::CgType;
use crate::diagnostics::{FileDiagnostic, Span};
use crate::surface::{DomainItem, Module, ModuleItem};

use super::check::summarize_module_export_surface;
use super::checker::TypeChecker;
use super::class_env::{
    collect_imported_class_env, collect_local_class_env, expand_classes,
    synthesize_auto_forward_instances, InstanceDeclInfo,
};
use super::global::collect_global_type_info;
use super::ordering::ordered_modules;
use super::{build_module_interface, ModuleInterface, ModuleInterfaceMaps};

/// Result of type inference: diagnostics, pretty-printed type strings, and codegen-friendly
/// type annotations.
#[derive(Clone)]
pub struct InferResult {
    pub diagnostics: Vec<FileDiagnostic>,
    /// Module → definition name → rendered type string (for LSP / display).
    pub type_strings: HashMap<String, HashMap<String, String>>,
    /// Module → definition name → codegen-friendly type (for the typed codegen path).
    pub cg_types: HashMap<String, HashMap<String, CgType>>,
    /// Qualified callee name → list of concrete CgType instantiations observed at call sites.
    /// Used by the monomorphization pass to specialize polymorphic definitions.
    pub monomorph_plan: HashMap<String, Vec<CgType>>,
    /// Module → list of (span, rendered type) for LSP hover / quick info.
    pub span_types: HashMap<String, Vec<(Span, String)>>,
    /// `"module.def"` → ordered list of inner CgTypes for `load` calls in that def.
    /// Used to inject JSON validation schemas at source boundaries.
    pub source_schemas: HashMap<String, Vec<CgType>>,
}

#[derive(Clone)]
pub struct InferCheckpoint {
    state: ModuleInterfaceMaps,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InferMode {
    Full,
    Fast,
}

#[derive(Clone)]
pub struct InferModuleCache {
    pub module_name: String,
    pub diagnostics: Vec<FileDiagnostic>,
    pub type_strings: HashMap<String, String>,
    pub cg_types: HashMap<String, CgType>,
    pub monomorph_plan: HashMap<String, Vec<CgType>>,
    pub span_types: Vec<(Span, String)>,
    pub source_schemas: HashMap<String, Vec<CgType>>,
    pub invalidate_fingerprint: u64,
    interface: ModuleInterface,
}

#[derive(Clone)]
pub struct InferValueTypesIncrementalResult {
    pub result: InferResult,
    pub checkpoint: InferCheckpoint,
    pub modules: Vec<InferModuleCache>,
}

#[allow(clippy::type_complexity)]
pub fn infer_value_types(
    modules: &[Module],
) -> (
    Vec<FileDiagnostic>,
    HashMap<String, HashMap<String, String>>,
    HashMap<String, Vec<(Span, String)>>,
) {
    let result = infer_value_types_full(modules);
    (result.diagnostics, result.type_strings, result.span_types)
}

pub fn infer_value_types_full(modules: &[Module]) -> InferResult {
    infer_value_types_full_incremental(modules, modules, &InferCheckpoint::empty()).result
}

/// Like [`infer_value_types_full`] but skips `check_module_defs` for embedded stdlib modules.
///
/// Stdlib modules have explicit type signatures for all exported functions, so their bodies
/// do not need to be re-checked at runtime. This avoids ~8s of redundant work on every
/// `aivi run` invocation. CgTypes for stdlib definitions are derived from their declared
/// type signatures instead of from inference.
pub fn infer_value_types_fast(modules: &[Module]) -> InferResult {
    infer_value_types_fast_incremental(modules, modules, &InferCheckpoint::empty()).result
}

pub fn infer_value_types_full_incremental(
    all_modules: &[Module],
    modules: &[Module],
    checkpoint: &InferCheckpoint,
) -> InferValueTypesIncrementalResult {
    infer_value_types_incremental_impl(all_modules, modules, checkpoint, InferMode::Full)
}

pub fn infer_value_types_fast_incremental(
    all_modules: &[Module],
    modules: &[Module],
    checkpoint: &InferCheckpoint,
) -> InferValueTypesIncrementalResult {
    infer_value_types_incremental_impl(all_modules, modules, checkpoint, InferMode::Fast)
}

fn infer_value_types_incremental_impl(
    all_modules: &[Module],
    modules: &[Module],
    checkpoint: &InferCheckpoint,
    mode: InferMode,
) -> InferValueTypesIncrementalResult {
    let skip_stdlib_body_check = matches!(mode, InferMode::Fast);
    let trace = std::env::var("AIVI_TRACE_TIMING").is_ok_and(|v| v == "1");
    let mut checker = TypeChecker::new();
    checker.compact_subst_between_defs = skip_stdlib_body_check;
    let mut diagnostics = Vec::new();
    let mut state = checkpoint.state.clone();
    let mut inferred: HashMap<String, HashMap<String, String>> = HashMap::new();
    let mut cg_types: HashMap<String, HashMap<String, CgType>> = HashMap::new();
    let mut monomorph_plan: HashMap<String, Vec<CgType>> = HashMap::new();
    let mut all_span_types: HashMap<String, Vec<(Span, String)>> = HashMap::new();
    let mut all_source_schemas: HashMap<String, Vec<CgType>> = HashMap::new();
    let mut module_results = Vec::new();

    let mut t_reset = 0u128;
    let mut t_reg_types = 0u128;
    let mut t_type_expr_diags = 0u128;
    let mut t_collect_sigs = 0u128;
    let mut t_reg_ctors = 0u128;
    let mut t_reg_imports = 0u128;
    let mut t_class_env = 0u128;
    let mut t_reg_defs = 0u128;
    let mut t_check_defs = 0u128;
    let mut t_export_collect = 0u128;

    let (global_type_constructors, global_aliases, global_opaque_types) =
        collect_global_type_info(&mut checker, all_modules);
    checker.set_global_type_info(
        global_type_constructors,
        global_aliases,
        global_opaque_types,
    );

    for module in ordered_modules(modules) {
        let is_embedded = module.path.starts_with("<embedded:");
        macro_rules! timed {
            ($acc:expr, $block:expr) => {{
                if trace {
                    let t0 = std::time::Instant::now();
                    let r = $block;
                    $acc += t0.elapsed().as_nanos();
                    r
                } else {
                    $block
                }
            }};
        }

        let start = diagnostics.len();
        timed!(t_reset, checker.reset_module_context(module));
        let mut env = if trace {
            let t0 = std::time::Instant::now();
            let r = checker.builtins.clone();
            t_reset += t0.elapsed().as_nanos();
            r
        } else {
            checker.builtins.clone()
        };
        timed!(t_reg_types, checker.register_module_types(module));
        timed!(
            t_reg_types,
            checker.register_imported_type_names(module, &state.module_type_exports)
        );
        checker.rewrite_env_type_names(&mut env);
        timed!(
            t_type_expr_diags,
            diagnostics.extend(checker.collect_type_expr_diags(module))
        );
        let sigs = timed!(t_collect_sigs, checker.collect_type_sigs(module));
        timed!(
            t_reg_ctors,
            checker.register_module_constructors(module, &mut env)
        );
        timed!(
            t_reg_imports,
            checker.register_imports(
                module,
                &state.module_exports,
                &state.module_domain_exports,
                &mut env,
            )
        );

        let (imported_classes, imported_instances) = timed!(
            t_class_env,
            collect_imported_class_env(
                module,
                &state.module_class_exports,
                &state.module_instance_exports
            )
        );
        let (local_classes, local_instances) = timed!(t_class_env, collect_local_class_env(module));
        let local_class_names: HashSet<String> =
            timed!(t_class_env, local_classes.keys().cloned().collect());
        let mut classes = imported_classes;
        classes.extend(local_classes);
        let classes = timed!(t_class_env, expand_classes(classes));
        let mut instances: Vec<InstanceDeclInfo> = imported_instances
            .into_iter()
            .filter(|instance| !local_class_names.contains(&instance.class_name))
            .collect();
        instances.extend(local_instances);
        timed!(
            t_class_env,
            instances.extend(synthesize_auto_forward_instances(module, &instances))
        );
        timed!(t_class_env, checker.set_class_env(classes, instances));
        timed!(
            t_reg_defs,
            checker.register_module_defs(module, &sigs, &mut env)
        );
        let mut module_monomorph_plan: HashMap<String, Vec<CgType>> = HashMap::new();

        if !(skip_stdlib_body_check && is_embedded) {
            let t_module_check = if trace {
                Some(std::time::Instant::now())
            } else {
                None
            };
            let mut module_diags = timed!(
                t_check_defs,
                checker.check_module_defs(module, &sigs, &mut env)
            );
            if let Some(t0) = t_module_check {
                let elapsed = t0.elapsed().as_millis();
                if elapsed > 50 {
                    eprintln!(
                        "[AIVI_TIMING_MODULE] {:<60} {:>6}ms",
                        module.name.name, elapsed
                    );
                }
            }
            diagnostics.append(&mut module_diags);

            for (qname, resolved_type) in checker.take_poly_instantiations() {
                let cg = checker.type_to_cg_type(&resolved_type, &env);
                if cg.is_closed() {
                    let entry = monomorph_plan.entry(qname.clone()).or_default();
                    if !entry.contains(&cg) {
                        entry.push(cg.clone());
                    }
                    let module_entry = module_monomorph_plan.entry(qname).or_default();
                    if !module_entry.contains(&cg) {
                        module_entry.push(cg);
                    }
                }
            }

            for (mod_name, def_name, inner_cg) in checker.take_load_source_schemas() {
                let key = format!("{}.{}", mod_name, def_name);
                all_source_schemas.entry(key).or_default().push(inner_cg);
            }

            let module_span_types = checker.take_span_types();
            if !module_span_types.is_empty() {
                all_span_types.insert(module.name.name.clone(), module_span_types);
            }
        }

        let mut local_names = HashSet::new();
        for item in module.items.iter() {
            match item {
                ModuleItem::Def(def) => {
                    local_names.insert(def.name.name.clone());
                }
                ModuleItem::TypeSig(sig) => {
                    local_names.insert(sig.name.name.clone());
                }
                ModuleItem::DomainDecl(domain) => {
                    for domain_item in domain.items.iter() {
                        match domain_item {
                            DomainItem::TypeSig(sig) => {
                                local_names.insert(sig.name.name.clone());
                            }
                            DomainItem::Def(def) | DomainItem::LiteralDef(def) => {
                                local_names.insert(def.name.name.clone());
                            }
                            DomainItem::TypeAlias(_) => {}
                        }
                    }
                }
                _ => {}
            }
        }

        let mut module_types = HashMap::new();
        let mut module_cg_types = HashMap::new();
        for name in local_names {
            if let Some(schemes) = env.get_all(&name) {
                if schemes.len() == 1 {
                    let rendered = checker
                        .query_cache
                        .get_binding_type(&module.name.name, &name)
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| checker.scheme_to_string(&schemes[0]));
                    checker.query_cache.store_binding_type(
                        module.name.name.clone(),
                        name.clone(),
                        rendered.clone(),
                    );
                    module_types.insert(name.clone(), rendered);
                    if schemes[0].vars.is_empty() {
                        module_cg_types.insert(name, checker.type_to_cg_type(&schemes[0].ty, &env));
                    } else {
                        module_cg_types.insert(name, CgType::Dynamic);
                    }
                } else {
                    let rendered = checker
                        .query_cache
                        .get_binding_type(&module.name.name, &name)
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| {
                            let mut rendered = String::new();
                            for (idx, scheme) in schemes.iter().enumerate() {
                                if idx > 0 {
                                    rendered.push_str(" | ");
                                }
                                rendered.push_str(&checker.scheme_to_string(scheme));
                            }
                            rendered
                        });
                    checker.query_cache.store_binding_type(
                        module.name.name.clone(),
                        name.clone(),
                        rendered.clone(),
                    );
                    module_types.insert(name.clone(), rendered);
                    module_cg_types.insert(name, CgType::Dynamic);
                }
            }
        }
        inferred.insert(module.name.name.clone(), module_types.clone());
        cg_types.insert(module.name.name.clone(), module_cg_types.clone());

        timed!(t_export_collect, {
            let interface = build_module_interface(module, &checker, &sigs, &env);
            state.apply_module_interface(&module.name.name, &interface);
            let invalidate_fingerprint = infer_invalidation_fingerprint(module, &module_types);
            module_results.push(InferModuleCache {
                module_name: module.name.name.clone(),
                diagnostics: diagnostics[start..].to_vec(),
                type_strings: module_types,
                cg_types: module_cg_types,
                monomorph_plan: module_monomorph_plan,
                span_types: all_span_types
                    .get(&module.name.name)
                    .cloned()
                    .unwrap_or_default(),
                source_schemas: source_schemas_for_module(&all_source_schemas, &module.name.name),
                invalidate_fingerprint,
                interface,
            });
        });
    }

    if trace {
        eprintln!(
            "[AIVI_TIMING_INFER] reset+clone_builtins       {:>8.1}ms",
            t_reset as f64 / 1_000_000.0
        );
        eprintln!(
            "[AIVI_TIMING_INFER] register_module_types       {:>8.1}ms",
            t_reg_types as f64 / 1_000_000.0
        );
        eprintln!(
            "[AIVI_TIMING_INFER] collect_type_expr_diags     {:>8.1}ms",
            t_type_expr_diags as f64 / 1_000_000.0
        );
        eprintln!(
            "[AIVI_TIMING_INFER] collect_type_sigs           {:>8.1}ms",
            t_collect_sigs as f64 / 1_000_000.0
        );
        eprintln!(
            "[AIVI_TIMING_INFER] register_module_constructors{:>8.1}ms",
            t_reg_ctors as f64 / 1_000_000.0
        );
        eprintln!(
            "[AIVI_TIMING_INFER] register_imports            {:>8.1}ms",
            t_reg_imports as f64 / 1_000_000.0
        );
        eprintln!(
            "[AIVI_TIMING_INFER] class_env (all steps)       {:>8.1}ms",
            t_class_env as f64 / 1_000_000.0
        );
        eprintln!(
            "[AIVI_TIMING_INFER] register_module_defs        {:>8.1}ms",
            t_reg_defs as f64 / 1_000_000.0
        );
        eprintln!(
            "[AIVI_TIMING_INFER] check_module_defs           {:>8.1}ms",
            t_check_defs as f64 / 1_000_000.0
        );
        eprintln!(
            "[AIVI_TIMING_INFER] export_collect              {:>8.1}ms",
            t_export_collect as f64 / 1_000_000.0
        );
    }

    InferValueTypesIncrementalResult {
        result: InferResult {
            diagnostics,
            type_strings: inferred,
            cg_types,
            monomorph_plan,
            span_types: all_span_types,
            source_schemas: all_source_schemas,
        },
        checkpoint: InferCheckpoint { state },
        modules: module_results,
    }
}

fn infer_invalidation_fingerprint(module: &Module, module_types: &HashMap<String, String>) -> u64 {
    let summary = summarize_module_export_surface(module);
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    summary.fingerprint.hash(&mut hasher);
    summary.body_sensitive.hash(&mut hasher);
    for export in &module.exports {
        if export.kind != crate::surface::ScopeItemKind::Value {
            continue;
        }
        export.name.name.hash(&mut hasher);
        module_types.get(&export.name.name).hash(&mut hasher);
    }
    hasher.finish()
}

fn source_schemas_for_module(
    source_schemas: &HashMap<String, Vec<CgType>>,
    module_name: &str,
) -> HashMap<String, Vec<CgType>> {
    let prefix = format!("{module_name}.");
    source_schemas
        .iter()
        .filter(|(name, _)| name.starts_with(&prefix))
        .map(|(name, schemas)| (name.clone(), schemas.clone()))
        .collect()
}

impl InferCheckpoint {
    pub fn empty() -> Self {
        Self {
            state: ModuleInterfaceMaps::default(),
        }
    }

    pub fn apply_cached_module(&mut self, module: &InferModuleCache) {
        self.state
            .apply_module_interface(&module.module_name, &module.interface);
    }

    pub fn remove_module(&mut self, module_name: &str) {
        self.state.remove_module(module_name);
    }
}
