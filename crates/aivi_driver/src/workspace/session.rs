use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use aivi_core::{
    check_modules, check_types_stdlib_checkpoint, check_types_with_checkpoint_incremental,
    elaborate_stdlib_checkpoint, elaborate_with_checkpoint_incremental, embedded_stdlib_modules,
    file_diagnostics_have_errors, infer_value_types_fast_incremental,
    infer_value_types_full_incremental, parse_modules, resolve_import_names,
    summarize_module_export_surface, CheckTypesCheckpoint, CheckedModule, ElaboratedModule,
    ElaborationCheckpoint, FileDiagnostic, HirProgram, InferCheckpoint, InferMode,
    InferModuleCache, InferResult, Module, ModuleExportSurfaceSummary,
};

use crate::AiviError;

use super::expand_target;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrontendAssemblyMode {
    Check,
    InferFast,
    InferFull,
}

#[derive(Clone, Debug, Default)]
pub struct AssemblyStats {
    pub reparsed_paths: Vec<PathBuf>,
    pub invalidated_modules: Vec<String>,
    pub rechecked_modules: Vec<String>,
    pub reelaborated_modules: Vec<String>,
    pub reinferred_modules: Vec<String>,
    pub reused_modules: Vec<String>,
}

#[derive(Clone)]
pub struct FrontendAssembly {
    pub mode: FrontendAssemblyMode,
    pub paths: Vec<PathBuf>,
    pub modules: Vec<Module>,
    pub parse_diagnostics: Vec<FileDiagnostic>,
    pub resolver_diagnostics: Vec<FileDiagnostic>,
    pub typecheck_diagnostics: Vec<FileDiagnostic>,
    pub inference: Option<InferResult>,
    pub stats: AssemblyStats,
}

impl FrontendAssembly {
    pub fn all_diagnostics(&self) -> Vec<FileDiagnostic> {
        let mut diagnostics = self.parse_diagnostics.clone();
        diagnostics.extend(self.resolver_diagnostics.clone());
        diagnostics.extend(self.typecheck_diagnostics.clone());
        if let Some(inference) = &self.inference {
            diagnostics.extend(inference.diagnostics.clone());
        }
        diagnostics
    }

    pub fn has_errors(&self) -> bool {
        file_diagnostics_have_errors(&self.all_diagnostics())
    }

    pub fn desugar(&self) -> HirProgram {
        aivi_core::desugar_modules(&self.modules)
    }
}

#[derive(Default)]
pub struct WorkspaceSession {
    source_overrides: HashMap<PathBuf, String>,
    files: HashMap<PathBuf, FileCacheEntry>,
    modules: HashMap<String, ModuleCacheEntry>,
    last_active_paths: BTreeSet<PathBuf>,
    stdlib: Option<StdlibState>,
    force_invalidate_all: bool,
}

#[derive(Clone)]
struct FileCacheEntry {
    fingerprint: u64,
    parsed_modules: Vec<Module>,
    parse_diagnostics: Vec<FileDiagnostic>,
}

#[derive(Clone)]
struct ModuleCacheEntry {
    path: PathBuf,
    resolved_module: Module,
    import_dependencies: Vec<String>,
    export_summary: ModuleExportSurfaceSummary,
    global_type_fingerprint: Option<u64>,
    check_cache: Option<CheckedModule>,
    elaborated_module: Option<Module>,
    elaboration_cache: Option<ElaboratedModule>,
    infer_fast_cache: Option<InferModuleCache>,
    infer_full_cache: Option<InferModuleCache>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CompilerConfigStamp {
    stdlib_enabled: bool,
}

#[derive(Clone)]
struct StdlibInferState {
    checkpoint: InferCheckpoint,
    modules: Vec<InferModuleCache>,
}

#[derive(Clone)]
struct StdlibState {
    config: CompilerConfigStamp,
    modules: Vec<Module>,
    check_checkpoint: CheckTypesCheckpoint,
    elaboration_checkpoint: ElaborationCheckpoint,
    infer_fast: StdlibInferState,
    infer_full: StdlibInferState,
}

#[derive(Clone, Default)]
struct ModuleGraph {
    ordered_groups: Vec<Vec<String>>,
    reverse_deps: HashMap<String, Vec<String>>,
}

impl WorkspaceSession {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn upsert_source(&mut self, path: impl Into<PathBuf>, text: String) {
        self.source_overrides.insert(path.into(), text);
    }

    pub fn clear_source_override(&mut self, path: &Path) {
        self.source_overrides.remove(path);
    }

    pub fn invalidate_all(&mut self) {
        self.force_invalidate_all = true;
        self.clear_stage_caches();
    }

    pub fn invalidate_compiler_state(&mut self) {
        self.stdlib = None;
        self.invalidate_all();
    }

    pub fn assemble_target(
        &mut self,
        target: &str,
        mode: FrontendAssemblyMode,
    ) -> Result<FrontendAssembly, AiviError> {
        let paths = expand_target(target)?;
        self.assemble_paths(&paths, mode)
    }

    pub fn assemble_paths(
        &mut self,
        paths: &[PathBuf],
        mode: FrontendAssemblyMode,
    ) -> Result<FrontendAssembly, AiviError> {
        let mut stats = AssemblyStats::default();
        let mut normalized_paths = paths.to_vec();
        normalized_paths.sort();
        let active_paths: BTreeSet<PathBuf> = normalized_paths.iter().cloned().collect();
        let target_changed = self.last_active_paths != active_paths;
        let previous_active_names = self.active_module_names(&self.last_active_paths);
        let previous_graph_reverse = self.reverse_dependencies_for(&previous_active_names);
        let previous_global_fingerprint =
            self.workspace_global_type_fingerprint(&previous_active_names);
        let previous_modules = previous_active_names
            .iter()
            .filter_map(|name| {
                self.modules
                    .get(name)
                    .cloned()
                    .map(|entry| (name.clone(), entry))
            })
            .collect::<HashMap<_, _>>();

        let stdlib_invalidated = self.ensure_stdlib()?;

        let mut previous_file_names = HashMap::new();
        for path in self.last_active_paths.union(&active_paths) {
            previous_file_names.insert(path.clone(), self.file_module_names(path));
        }

        for path in &normalized_paths {
            if self.refresh_file_cache(path)? {
                stats.reparsed_paths.push(path.clone());
            }
        }

        let parse_diagnostics = self.parse_diagnostics_for_paths(&normalized_paths);
        let user_modules = self.modules_for_paths(&normalized_paths);
        let mut all_modules = self.stdlib_modules();
        all_modules.extend(user_modules);
        resolve_import_names(&mut all_modules);
        let stdlib_count = self.stdlib_modules().len();
        let resolved_user_modules = all_modules[stdlib_count..].to_vec();
        let (current_unique_modules, current_module_order) = unique_modules(resolved_user_modules);
        let current_graph = ModuleGraph::from_modules(&current_unique_modules);
        let current_names = current_module_order.iter().cloned().collect::<HashSet<_>>();
        let current_global_fingerprint =
            workspace_global_type_fingerprint_from_modules(current_unique_modules.values());
        let global_type_changed = previous_global_fingerprint != current_global_fingerprint;

        let mut changed_module_names = HashSet::new();
        if target_changed {
            changed_module_names.extend(previous_active_names.iter().cloned());
            changed_module_names.extend(current_names.iter().cloned());
        } else {
            for path in &stats.reparsed_paths {
                changed_module_names
                    .extend(previous_file_names.get(path).cloned().unwrap_or_default());
                changed_module_names
                    .extend(self.file_module_names(path).into_iter().collect::<Vec<_>>());
            }
        }

        let mut dirty_modules = HashSet::new();
        if self.force_invalidate_all || target_changed || stdlib_invalidated || global_type_changed
        {
            dirty_modules.extend(current_names.iter().cloned());
        }

        for module_name in &changed_module_names {
            if current_names.contains(module_name) {
                dirty_modules.insert(module_name.clone());
            }
            let previous_summary = previous_modules
                .get(module_name)
                .map(|entry| entry.export_summary.clone());
            let current_summary = current_unique_modules
                .get(module_name)
                .map(summarize_module_entry_export_surface);
            let summary_changed = previous_summary != current_summary;
            if summary_changed {
                for dependent in previous_graph_reverse
                    .get(module_name)
                    .into_iter()
                    .flat_map(|deps| deps.iter())
                {
                    if current_names.contains(dependent) {
                        dirty_modules.insert(dependent.clone());
                    }
                }
                for dependent in current_graph
                    .reverse_deps
                    .get(module_name)
                    .into_iter()
                    .flat_map(|deps| deps.iter())
                {
                    dirty_modules.insert(dependent.clone());
                }
            }
        }

        for module_name in previous_active_names.difference(&current_names) {
            self.modules.remove(module_name);
        }

        for module_name in &current_module_order {
            let module = current_unique_modules
                .get(module_name)
                .expect("current module order should reference an active module");
            let previous = self.modules.get(module_name).cloned();
            let entry = self
                .modules
                .entry(module_name.clone())
                .or_insert_with(|| ModuleCacheEntry::new(module.clone()));
            entry.path = PathBuf::from(&module.path);
            entry.resolved_module = module.clone();
            entry.import_dependencies = module_imports(module);
            entry.export_summary = summarize_module_export_surface(module);
            entry.global_type_fingerprint = module_global_type_fingerprint(module);
            if previous.is_none() || changed_module_names.contains(module_name) {
                entry.clear_stage_caches();
            }
        }

        let resolver_diagnostics = check_modules(&all_modules);
        self.last_active_paths = active_paths;
        self.force_invalidate_all = false;

        if file_diagnostics_have_errors(&parse_diagnostics)
            || file_diagnostics_have_errors(&resolver_diagnostics)
        {
            stats.invalidated_modules = sorted_strings(dirty_modules);
            return Ok(FrontendAssembly {
                mode,
                paths: normalized_paths,
                modules: all_modules,
                parse_diagnostics,
                resolver_diagnostics,
                typecheck_diagnostics: Vec::new(),
                inference: None,
                stats: finalize_stats(stats),
            });
        }

        let mut assembled_modules = self.stdlib_modules();
        let mut typecheck_diagnostics = Vec::new();
        let all_unique_modules = assembled_all_modules(
            &self.stdlib_modules(),
            &current_unique_modules,
            &current_module_order,
        );

        match mode {
            FrontendAssemblyMode::Check => {
                let mut checkpoint = self.stdlib_check_checkpoint();
                for group in &current_graph.ordered_groups {
                    let needs_recheck = group.iter().any(|name| {
                        dirty_modules.contains(name)
                            || self
                                .modules
                                .get(name)
                                .and_then(|entry| entry.check_cache.as_ref())
                                .is_none()
                    });
                    if needs_recheck {
                        let group_modules = group_modules(group, &current_unique_modules);
                        let result = check_types_with_checkpoint_incremental(
                            &all_unique_modules,
                            &group_modules,
                            &checkpoint,
                        );
                        checkpoint = result.checkpoint;
                        stats.rechecked_modules.extend(group.iter().cloned());
                        for cache in result.modules {
                            typecheck_diagnostics.extend(cache.diagnostics.clone());
                            if let Some(entry) = self.modules.get_mut(&cache.module_name) {
                                entry.check_cache = Some(cache);
                            }
                        }
                    } else {
                        for module_name in group {
                            let entry = self.modules.get(module_name).expect("cached module entry");
                            let cache = entry.check_cache.as_ref().expect("checked cache");
                            checkpoint.apply_cached_module(cache);
                            typecheck_diagnostics.extend(cache.diagnostics.clone());
                            stats.reused_modules.push(module_name.clone());
                        }
                    }
                }
                assembled_modules.extend(
                    current_module_order
                        .iter()
                        .filter_map(|name| current_unique_modules.get(name).cloned()),
                );
                stats.invalidated_modules = sorted_strings(dirty_modules);
                Ok(FrontendAssembly {
                    mode,
                    paths: normalized_paths,
                    modules: assembled_modules,
                    parse_diagnostics,
                    resolver_diagnostics,
                    typecheck_diagnostics,
                    inference: None,
                    stats: finalize_stats(stats),
                })
            }
            FrontendAssemblyMode::InferFast | FrontendAssemblyMode::InferFull => {
                let infer_mode = if matches!(mode, FrontendAssemblyMode::InferFast) {
                    InferMode::Fast
                } else {
                    InferMode::Full
                };
                let mut elaboration_checkpoint = self.stdlib_elaboration_checkpoint();
                let stdlib_infer = self.stdlib_infer_state(infer_mode).clone();
                let mut infer_checkpoint = stdlib_infer.checkpoint.clone();
                let mut inference = InferResult {
                    diagnostics: Vec::new(),
                    type_strings: HashMap::new(),
                    cg_types: HashMap::new(),
                    monomorph_plan: HashMap::new(),
                    span_types: HashMap::new(),
                    source_schemas: HashMap::new(),
                };
                for cache in &stdlib_infer.modules {
                    infer_checkpoint.apply_cached_module(cache);
                    merge_infer_cache(&mut inference, cache);
                }

                for group in &current_graph.ordered_groups {
                    let needs_rebuild = group.iter().any(|name| {
                        let Some(entry) = self.modules.get(name) else {
                            return true;
                        };
                        dirty_modules.contains(name)
                            || entry.elaboration_cache.is_none()
                            || entry.elaborated_module.is_none()
                            || entry.infer_cache(infer_mode).is_none()
                    });
                    if needs_rebuild {
                        let mut group_modules = group_modules(group, &current_unique_modules);
                        let elab_result = elaborate_with_checkpoint_incremental(
                            &all_unique_modules,
                            &mut group_modules,
                            &elaboration_checkpoint,
                        );
                        elaboration_checkpoint = elab_result.checkpoint;
                        typecheck_diagnostics.extend(elab_result.diagnostics.clone());
                        stats.reelaborated_modules.extend(group.iter().cloned());
                        for (module, cache) in group_modules.iter().zip(elab_result.modules.iter())
                        {
                            if let Some(entry) = self.modules.get_mut(&cache.module_name) {
                                entry.elaboration_cache = Some(cache.clone());
                                entry.elaborated_module = Some(module.clone());
                            }
                        }

                        let infer_result = match infer_mode {
                            InferMode::Fast => infer_value_types_fast_incremental(
                                &all_unique_modules,
                                &group_modules,
                                &infer_checkpoint,
                            ),
                            InferMode::Full => infer_value_types_full_incremental(
                                &all_unique_modules,
                                &group_modules,
                                &infer_checkpoint,
                            ),
                        };
                        infer_checkpoint = infer_result.checkpoint;
                        inference
                            .diagnostics
                            .extend(infer_result.result.diagnostics.clone());
                        stats.reinferred_modules.extend(group.iter().cloned());
                        for cache in infer_result.modules {
                            let previous_fingerprint = previous_modules
                                .get(&cache.module_name)
                                .and_then(|entry| entry.infer_cache(infer_mode))
                                .map(|cached| cached.invalidate_fingerprint);
                            if previous_fingerprint != Some(cache.invalidate_fingerprint) {
                                for dependent in current_graph
                                    .reverse_deps
                                    .get(&cache.module_name)
                                    .into_iter()
                                    .flat_map(|deps| deps.iter())
                                {
                                    dirty_modules.insert(dependent.clone());
                                }
                            }
                            merge_infer_cache(&mut inference, &cache);
                            if let Some(entry) = self.modules.get_mut(&cache.module_name) {
                                entry.set_infer_cache(infer_mode, cache);
                            }
                        }
                        assembled_modules.extend(group_modules);
                    } else {
                        for module_name in group {
                            let entry = self.modules.get(module_name).expect("cached module entry");
                            let elab_cache =
                                entry.elaboration_cache.as_ref().expect("elaboration cache");
                            let infer_cache = entry.infer_cache(infer_mode).expect("infer cache");
                            elaboration_checkpoint.apply_cached_module(elab_cache);
                            infer_checkpoint.apply_cached_module(infer_cache);
                            merge_infer_cache(&mut inference, infer_cache);
                            assembled_modules.push(
                                entry
                                    .elaborated_module
                                    .as_ref()
                                    .expect("elaborated module")
                                    .clone(),
                            );
                            stats.reused_modules.push(module_name.clone());
                        }
                    }
                }

                stats.invalidated_modules = sorted_strings(dirty_modules);
                Ok(FrontendAssembly {
                    mode,
                    paths: normalized_paths,
                    modules: assembled_modules,
                    parse_diagnostics,
                    resolver_diagnostics,
                    typecheck_diagnostics,
                    inference: Some(inference),
                    stats: finalize_stats(stats),
                })
            }
        }
    }

    fn ensure_stdlib(&mut self) -> Result<bool, AiviError> {
        let config = CompilerConfigStamp::capture();
        let needs_refresh = self
            .stdlib
            .as_ref()
            .is_none_or(|state| state.config != config);
        if !needs_refresh {
            return Ok(false);
        }

        let modules = embedded_stdlib_modules();
        let check_checkpoint = check_types_stdlib_checkpoint(&modules);
        let mut elaboration_modules = modules.clone();
        let elaboration_checkpoint = elaborate_stdlib_checkpoint(&mut elaboration_modules);
        let infer_fast = {
            let result =
                infer_value_types_fast_incremental(&modules, &modules, &InferCheckpoint::empty());
            StdlibInferState {
                checkpoint: result.checkpoint,
                modules: result.modules,
            }
        };
        let infer_full = {
            let result =
                infer_value_types_full_incremental(&modules, &modules, &InferCheckpoint::empty());
            StdlibInferState {
                checkpoint: result.checkpoint,
                modules: result.modules,
            }
        };
        self.stdlib = Some(StdlibState {
            config,
            modules,
            check_checkpoint,
            elaboration_checkpoint,
            infer_fast,
            infer_full,
        });
        self.clear_stage_caches();
        Ok(true)
    }

    fn clear_stage_caches(&mut self) {
        for entry in self.modules.values_mut() {
            entry.clear_stage_caches();
        }
    }

    fn refresh_file_cache(&mut self, path: &Path) -> Result<bool, AiviError> {
        let content = if let Some(text) = self.source_overrides.get(path) {
            text.clone()
        } else {
            fs::read_to_string(path)?
        };
        let fingerprint = text_fingerprint(&content);
        if self
            .files
            .get(path)
            .is_some_and(|entry| entry.fingerprint == fingerprint)
        {
            return Ok(false);
        }
        let (parsed_modules, parse_diagnostics) = parse_modules(path, &content);
        self.files.insert(
            path.to_path_buf(),
            FileCacheEntry {
                fingerprint,
                parsed_modules,
                parse_diagnostics,
            },
        );
        Ok(true)
    }

    fn modules_for_paths(&self, paths: &[PathBuf]) -> Vec<Module> {
        let mut modules = Vec::new();
        for path in paths {
            if let Some(entry) = self.files.get(path) {
                modules.extend(entry.parsed_modules.clone());
            }
        }
        modules
    }

    fn parse_diagnostics_for_paths(&self, paths: &[PathBuf]) -> Vec<FileDiagnostic> {
        let mut diagnostics = Vec::new();
        for path in paths {
            if let Some(entry) = self.files.get(path) {
                diagnostics.extend(entry.parse_diagnostics.clone());
            }
        }
        diagnostics
    }

    fn file_module_names(&self, path: &Path) -> HashSet<String> {
        self.files
            .get(path)
            .map(|entry| {
                entry
                    .parsed_modules
                    .iter()
                    .map(|module| module.name.name.clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    fn active_module_names(&self, paths: &BTreeSet<PathBuf>) -> HashSet<String> {
        paths
            .iter()
            .flat_map(|path| self.file_module_names(path))
            .collect()
    }

    fn reverse_dependencies_for(
        &self,
        module_names: &HashSet<String>,
    ) -> HashMap<String, Vec<String>> {
        let mut reverse = HashMap::<String, Vec<String>>::new();
        for module_name in module_names {
            reverse.entry(module_name.clone()).or_default();
        }
        for module_name in module_names {
            let Some(entry) = self.modules.get(module_name) else {
                continue;
            };
            for dependency in &entry.import_dependencies {
                if !module_names.contains(dependency) {
                    continue;
                }
                reverse
                    .entry(dependency.clone())
                    .or_default()
                    .push(module_name.clone());
            }
        }
        for dependents in reverse.values_mut() {
            dependents.sort();
            dependents.dedup();
        }
        reverse
    }

    fn workspace_global_type_fingerprint(&self, module_names: &HashSet<String>) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        let mut items = module_names
            .iter()
            .filter_map(|name| {
                self.modules.get(name).and_then(|entry| {
                    entry
                        .global_type_fingerprint
                        .map(|fingerprint| (name.clone(), fingerprint))
                })
            })
            .collect::<Vec<_>>();
        if items.is_empty() {
            return 0;
        }
        items.sort_by(|a, b| a.0.cmp(&b.0));
        for item in items {
            item.hash(&mut hasher);
        }
        hasher.finish()
    }

    fn stdlib_modules(&self) -> Vec<Module> {
        self.stdlib
            .as_ref()
            .map(|state| state.modules.clone())
            .unwrap_or_default()
    }

    fn stdlib_check_checkpoint(&self) -> CheckTypesCheckpoint {
        self.stdlib
            .as_ref()
            .map(|state| state.check_checkpoint.clone())
            .unwrap_or_else(CheckTypesCheckpoint::empty)
    }

    fn stdlib_elaboration_checkpoint(&self) -> ElaborationCheckpoint {
        self.stdlib
            .as_ref()
            .map(|state| state.elaboration_checkpoint.clone())
            .unwrap_or_else(ElaborationCheckpoint::empty)
    }

    fn stdlib_infer_state(&self, mode: InferMode) -> &StdlibInferState {
        let state = self
            .stdlib
            .as_ref()
            .expect("stdlib state should be initialized");
        match mode {
            InferMode::Fast => &state.infer_fast,
            InferMode::Full => &state.infer_full,
        }
    }
}

impl ModuleCacheEntry {
    fn new(module: Module) -> Self {
        Self {
            path: PathBuf::from(&module.path),
            resolved_module: module.clone(),
            import_dependencies: module_imports(&module),
            export_summary: summarize_module_export_surface(&module),
            global_type_fingerprint: module_global_type_fingerprint(&module),
            check_cache: None,
            elaborated_module: None,
            elaboration_cache: None,
            infer_fast_cache: None,
            infer_full_cache: None,
        }
    }

    fn clear_stage_caches(&mut self) {
        self.check_cache = None;
        self.elaboration_cache = None;
        self.elaborated_module = None;
        self.infer_fast_cache = None;
        self.infer_full_cache = None;
    }

    fn infer_cache(&self, mode: InferMode) -> Option<&InferModuleCache> {
        match mode {
            InferMode::Fast => self.infer_fast_cache.as_ref(),
            InferMode::Full => self.infer_full_cache.as_ref(),
        }
    }

    fn set_infer_cache(&mut self, mode: InferMode, cache: InferModuleCache) {
        match mode {
            InferMode::Fast => self.infer_fast_cache = Some(cache),
            InferMode::Full => self.infer_full_cache = Some(cache),
        }
    }
}

impl CompilerConfigStamp {
    fn capture() -> Self {
        Self {
            stdlib_enabled: !std::env::var("AIVI_NO_STDLIB")
                .is_ok_and(|value| value == "1" || value.eq_ignore_ascii_case("true")),
        }
    }
}

impl ModuleGraph {
    fn from_modules(modules: &HashMap<String, Module>) -> Self {
        if modules.is_empty() {
            return Self::default();
        }

        let mut names = modules.keys().cloned().collect::<Vec<_>>();
        names.sort();
        let mut edges = HashMap::<String, Vec<String>>::new();
        let mut reverse_edges = HashMap::<String, Vec<String>>::new();
        for name in &names {
            edges.entry(name.clone()).or_default();
            reverse_edges.entry(name.clone()).or_default();
        }
        for name in &names {
            let module = modules.get(name).expect("module should exist");
            for dependency in module_imports(module) {
                if dependency == *name || !modules.contains_key(&dependency) {
                    continue;
                }
                edges
                    .entry(name.clone())
                    .or_default()
                    .push(dependency.clone());
                reverse_edges
                    .entry(dependency)
                    .or_default()
                    .push(name.clone());
            }
        }

        let mut visited = HashSet::new();
        let mut order = Vec::new();
        for name in &names {
            dfs_order(name, &edges, &mut visited, &mut order);
        }

        visited.clear();
        let mut groups = Vec::<Vec<String>>::new();
        while let Some(name) = order.pop() {
            if visited.contains(&name) {
                continue;
            }
            let mut group = Vec::new();
            dfs_collect(&name, &reverse_edges, &mut visited, &mut group);
            group.sort();
            groups.push(group);
        }

        let mut module_to_group = HashMap::new();
        for (idx, group) in groups.iter().enumerate() {
            for module_name in group {
                module_to_group.insert(module_name.clone(), idx);
            }
        }

        let mut group_edges = vec![HashSet::<usize>::new(); groups.len()];
        let mut indegree = vec![0usize; groups.len()];
        for (module_name, dependencies) in &edges {
            let Some(&group_idx) = module_to_group.get(module_name) else {
                continue;
            };
            for dependency in dependencies {
                let Some(&dep_group_idx) = module_to_group.get(dependency) else {
                    continue;
                };
                if dep_group_idx == group_idx || !group_edges[dep_group_idx].insert(group_idx) {
                    continue;
                }
                indegree[group_idx] += 1;
            }
        }

        let mut ready = indegree
            .iter()
            .enumerate()
            .filter_map(|(idx, degree)| (*degree == 0).then_some(idx))
            .collect::<Vec<_>>();
        ready.sort_by(|left, right| groups[*left][0].cmp(&groups[*right][0]));

        let mut ordered_groups = Vec::new();
        while let Some(group_idx) = ready.first().copied() {
            ready.remove(0);
            ordered_groups.push(groups[group_idx].clone());
            for (next, next_indegree) in indegree.iter_mut().enumerate().take(groups.len()) {
                if !group_edges[group_idx].contains(&next) {
                    continue;
                }
                *next_indegree = next_indegree.saturating_sub(1);
                if *next_indegree == 0 && !ready.contains(&next) {
                    ready.push(next);
                }
            }
            ready.sort_by(|left, right| groups[*left][0].cmp(&groups[*right][0]));
        }

        let mut reverse_deps = HashMap::<String, Vec<String>>::new();
        for name in &names {
            reverse_deps.insert(name.clone(), Vec::new());
        }
        for (module_name, dependencies) in edges {
            for dependency in dependencies {
                reverse_deps
                    .entry(dependency)
                    .or_default()
                    .push(module_name.clone());
            }
        }
        for dependents in reverse_deps.values_mut() {
            dependents.sort();
            dependents.dedup();
        }

        Self {
            ordered_groups,
            reverse_deps,
        }
    }
}

fn dfs_order(
    name: &str,
    edges: &HashMap<String, Vec<String>>,
    visited: &mut HashSet<String>,
    order: &mut Vec<String>,
) {
    if !visited.insert(name.to_string()) {
        return;
    }
    if let Some(dependencies) = edges.get(name) {
        for dependency in dependencies {
            dfs_order(dependency, edges, visited, order);
        }
    }
    order.push(name.to_string());
}

fn dfs_collect(
    name: &str,
    reverse_edges: &HashMap<String, Vec<String>>,
    visited: &mut HashSet<String>,
    group: &mut Vec<String>,
) {
    if !visited.insert(name.to_string()) {
        return;
    }
    group.push(name.to_string());
    if let Some(dependents) = reverse_edges.get(name) {
        for dependent in dependents {
            dfs_collect(dependent, reverse_edges, visited, group);
        }
    }
}

fn assembled_all_modules(
    stdlib_modules: &[Module],
    user_modules: &HashMap<String, Module>,
    module_order: &[String],
) -> Vec<Module> {
    let mut modules = stdlib_modules.to_vec();
    modules.extend(
        module_order
            .iter()
            .filter_map(|module_name| user_modules.get(module_name).cloned()),
    );
    modules
}

fn group_modules(group: &[String], modules: &HashMap<String, Module>) -> Vec<Module> {
    group
        .iter()
        .filter_map(|module_name| modules.get(module_name).cloned())
        .collect()
}

fn unique_modules(modules: Vec<Module>) -> (HashMap<String, Module>, Vec<String>) {
    let mut unique = HashMap::new();
    let mut order = Vec::new();
    for module in modules {
        let module_name = module.name.name.clone();
        if unique.contains_key(&module_name) {
            continue;
        }
        order.push(module_name.clone());
        unique.insert(module_name, module);
    }
    (unique, order)
}

fn module_imports(module: &Module) -> Vec<String> {
    let mut imports = module
        .uses
        .iter()
        .map(|use_decl| use_decl.module.name.clone())
        .collect::<Vec<_>>();
    imports.sort();
    imports.dedup();
    imports
}

fn module_global_type_fingerprint(module: &Module) -> Option<u64> {
    let mut items = Vec::new();
    for item in &module.items {
        match item {
            aivi_core::ModuleItem::TypeDecl(type_decl) => {
                items.push(format!("type:{type_decl:?}"));
            }
            aivi_core::ModuleItem::TypeAlias(alias) => {
                items.push(format!("alias:{alias:?}"));
            }
            aivi_core::ModuleItem::DomainDecl(domain) => {
                for domain_item in &domain.items {
                    if let aivi_core::DomainItem::TypeAlias(alias) = domain_item {
                        items.push(format!("domain-alias:{alias:?}"));
                    }
                }
            }
            _ => {}
        }
    }
    if items.is_empty() {
        return None;
    }
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    module.name.name.hash(&mut hasher);
    items.sort();
    items.hash(&mut hasher);
    Some(hasher.finish())
}

fn workspace_global_type_fingerprint_from_modules<'a>(
    modules: impl Iterator<Item = &'a Module>,
) -> u64 {
    let mut items = modules
        .filter_map(|module| {
            module_global_type_fingerprint(module)
                .map(|fingerprint| (module.name.name.clone(), fingerprint))
        })
        .collect::<Vec<_>>();
    if items.is_empty() {
        return 0;
    }
    items.sort_by(|left, right| left.0.cmp(&right.0));
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    items.hash(&mut hasher);
    hasher.finish()
}

fn summarize_module_entry_export_surface(module: &Module) -> ModuleExportSurfaceSummary {
    summarize_module_export_surface(module)
}

fn merge_infer_cache(result: &mut InferResult, cache: &InferModuleCache) {
    result
        .type_strings
        .insert(cache.module_name.clone(), cache.type_strings.clone());
    result
        .cg_types
        .insert(cache.module_name.clone(), cache.cg_types.clone());
    for (qualified_name, cg_types) in &cache.monomorph_plan {
        let entry = result
            .monomorph_plan
            .entry(qualified_name.clone())
            .or_default();
        for cg_type in cg_types {
            if !entry.contains(cg_type) {
                entry.push(cg_type.clone());
            }
        }
    }
    if !cache.span_types.is_empty() {
        result
            .span_types
            .insert(cache.module_name.clone(), cache.span_types.clone());
    }
    for (qualified_name, schemas) in &cache.source_schemas {
        let entry = result
            .source_schemas
            .entry(qualified_name.clone())
            .or_default();
        entry.extend(schemas.clone());
    }
}

fn text_fingerprint(text: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

fn sorted_strings(values: HashSet<String>) -> Vec<String> {
    let mut values = values.into_iter().collect::<Vec<_>>();
    values.sort();
    values
}

fn finalize_stats(mut stats: AssemblyStats) -> AssemblyStats {
    stats.reparsed_paths.sort();
    stats.invalidated_modules.sort();
    stats.rechecked_modules.sort();
    stats.rechecked_modules.dedup();
    stats.reelaborated_modules.sort();
    stats.reelaborated_modules.dedup();
    stats.reinferred_modules.sort();
    stats.reinferred_modules.dedup();
    stats.reused_modules.sort();
    stats.reused_modules.dedup();
    stats
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_graph_groups_cycles_together() {
        let (mut modules_a, diagnostics_a) = parse_modules(
            Path::new("graph_a.aivi"),
            r#"
module testGraph.a

use testGraph.b (valueB)

valueA = valueB
"#,
        );
        let (mut modules_b, diagnostics_b) = parse_modules(
            Path::new("graph_b.aivi"),
            r#"
module testGraph.b

use testGraph.a (valueA)

valueB = valueA
"#,
        );
        assert!(diagnostics_a.is_empty());
        assert!(diagnostics_b.is_empty());
        let mut modules = Vec::new();
        modules.append(&mut modules_a);
        modules.append(&mut modules_b);
        resolve_import_names(&mut modules);
        let (unique, _) = unique_modules(modules);
        let graph = ModuleGraph::from_modules(&unique);
        assert_eq!(graph.ordered_groups.len(), 1);
        assert_eq!(
            graph.ordered_groups[0],
            vec!["testGraph.a".to_string(), "testGraph.b".to_string()]
        );
    }

    #[test]
    fn workspace_session_reuses_unaffected_dependents_when_export_surface_is_stable() {
        let temp =
            std::env::temp_dir().join(format!("aivi-driver-incremental-{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp);
        fs::create_dir_all(&temp).expect("create temp dir");

        let a_path = temp.join("a.aivi");
        let b_path = temp.join("b.aivi");
        fs::write(
            &a_path,
            r#"
module testIncremental.a
export value

helper = 1

value : Int
value = helper
"#,
        )
        .expect("write a");
        fs::write(
            &b_path,
            r#"
module testIncremental.b

use testIncremental.a (value)
export answer

answer : Int
answer = value
"#,
        )
        .expect("write b");

        let mut session = WorkspaceSession::new();
        let first = session
            .assemble_paths(
                &[a_path.clone(), b_path.clone()],
                FrontendAssemblyMode::InferFast,
            )
            .expect("first assembly");
        assert_eq!(
            first.stats.reinferred_modules,
            vec!["testIncremental.a", "testIncremental.b"]
        );

        fs::write(
            &a_path,
            r#"
module testIncremental.a
export value

helper = 2

value : Int
value = helper
"#,
        )
        .expect("rewrite a");
        let second = session
            .assemble_paths(
                &[a_path.clone(), b_path.clone()],
                FrontendAssemblyMode::InferFast,
            )
            .expect("second assembly");
        assert_eq!(second.stats.reinferred_modules, vec!["testIncremental.a"]);
        assert_eq!(second.stats.reused_modules, vec!["testIncremental.b"]);

        let _ = fs::remove_dir_all(&temp);
    }
}
