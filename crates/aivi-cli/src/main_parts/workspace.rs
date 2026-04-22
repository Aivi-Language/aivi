struct WorkspaceFrontend {
    db: RootDatabase,
    entry: QuerySourceFile,
}

#[derive(Clone, Copy)]
struct BackendQueryContext<'a> {
    db: &'a RootDatabase,
    entry: QuerySourceFile,
}

impl WorkspaceFrontend {
    fn load(path: &Path) -> Result<Self, String> {
        let text = fs::read_to_string(path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        let db = RootDatabase::new();
        let entry = QuerySourceFile::new(&db, path.to_path_buf(), text);
        Ok(Self { db, entry })
    }

    fn warm(&self) {
        let _ = query_hir_module(&self.db, self.entry);
    }

    fn files(&self) -> Vec<QuerySourceFile> {
        self.db.files()
    }

    fn sources(&self) -> SourceDatabase {
        self.db.source_database()
    }
}

struct WorkspaceHirSnapshot {
    frontend: WorkspaceFrontend,
    sources: SourceDatabase,
    files: Vec<QuerySourceFile>,
}

impl WorkspaceHirSnapshot {
    fn load(path: &Path) -> Result<Self, String> {
        let frontend = WorkspaceFrontend::load(path)?;
        frontend.warm();
        let sources = frontend.sources();
        let files = frontend.files();
        Ok(Self {
            frontend,
            sources,
            files,
        })
    }

    fn entry_parsed(&self) -> Arc<aivi_query::ParsedFileResult> {
        query_parsed_file(&self.frontend.db, self.frontend.entry)
    }

    fn entry_hir(&self) -> Arc<aivi_query::HirModuleResult> {
        query_hir_module(&self.frontend.db, self.frontend.entry)
    }

    fn backend_query_context(&self) -> BackendQueryContext<'_> {
        BackendQueryContext {
            db: &self.frontend.db,
            entry: self.frontend.entry,
        }
    }
}

/// Collect all non-entry workspace HIR modules in topological dependency order
/// (dependencies before dependents) so that workspace function bodies are
/// available when later modules reference them.
fn collect_workspace_hirs_sorted(
    snapshot: &WorkspaceHirSnapshot,
) -> Vec<(String, Arc<HirModuleResult>)> {
    reachable_workspace_hir_modules(&snapshot.frontend.db, snapshot.frontend.entry)
        .iter()
        .map(|module| (module.name().to_string(), module.hir_arc()))
        .collect()
}

fn workspace_syntax_failed(
    snapshot: &WorkspaceHirSnapshot,
    mut print: impl FnMut(&SourceDatabase, &[Diagnostic]) -> bool,
) -> bool {
    let mut failed = false;
    for file in &snapshot.files {
        let parsed = query_parsed_file(&snapshot.frontend.db, *file);
        failed |= print(&snapshot.sources, parsed.diagnostics());
    }
    failed
}

fn workspace_hir_failed(
    snapshot: &WorkspaceHirSnapshot,
    mut print_hir: impl FnMut(&SourceDatabase, &[Diagnostic]) -> bool,
    mut print_validation: impl FnMut(&SourceDatabase, &[Diagnostic]) -> bool,
) -> (bool, bool) {
    let mut lowering_failed = false;
    let mut validation_failed = false;
    for file in &snapshot.files {
        let hir = query_hir_module(&snapshot.frontend.db, *file);
        let file_lowering_failed = print_hir(&snapshot.sources, hir.hir_diagnostics());
        lowering_failed |= file_lowering_failed;
        let validation_mode = if file_lowering_failed {
            ValidationMode::Structural
        } else {
            ValidationMode::RequireResolvedNames
        };
        let validation = hir.module().validate(validation_mode);
        validation_failed |= print_validation(&snapshot.sources, validation.diagnostics());
    }
    (lowering_failed, validation_failed)
}
