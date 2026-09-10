use std::{
    cmp::Ordering,
    collections::HashMap,
    sync::{Arc, Mutex},
};

use aivi_hir::{LspSymbol, LspSymbolKind};
use aivi_query::SourceFile;
use tower_lsp::lsp_types::{Location, Position, Range, Url};

use crate::{
    navigation::{NavigationAnalysis, NavigationTarget},
    state::ServerState,
};

#[derive(Clone, Debug)]
pub(crate) struct IndexedSymbol {
    pub name: String,
    pub normalized_name: String,
    pub kind: LspSymbolKind,
    pub location: Location,
    pub container_name: Option<String>,
}

pub(crate) struct WorkspaceIndexSnapshot {
    symbols: Arc<[IndexedSymbol]>,
    references: HashMap<NavigationTarget, Arc<[Location]>>,
}

impl WorkspaceIndexSnapshot {
    pub fn symbols(&self) -> &[IndexedSymbol] {
        &self.symbols
    }

    pub fn reference_locations(&self, targets: &[NavigationTarget]) -> Vec<Location> {
        let mut locations = targets
            .iter()
            .filter_map(|target| self.references.get(target))
            .flat_map(|locations| locations.iter().cloned())
            .collect::<Vec<_>>();
        sort_locations(&mut locations);
        locations.dedup();
        locations
    }
}

struct CachedWorkspaceIndex {
    revision: u64,
    files: Vec<(Url, SourceFile)>,
    snapshot: Arc<WorkspaceIndexSnapshot>,
}

/// One immutable semantic workspace index per query-database source revision.
///
/// Server request handlers hold the analysis read lease while calling
/// [`WorkspaceIndex::snapshot`], so the database revision and open-document set
/// cannot change during construction. The file-set comparison additionally
/// handles multiple URIs that resolve to the same query input.
#[derive(Default)]
pub(crate) struct WorkspaceIndex {
    cached: Mutex<Option<CachedWorkspaceIndex>>,
}

impl WorkspaceIndex {
    pub fn snapshot(&self, state: &ServerState) -> Arc<WorkspaceIndexSnapshot> {
        let files = state.project_files();
        let mut cached = self
            .cached
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        loop {
            let revision = state.db.workspace_revision();
            if let Some(existing) = cached.as_ref()
                && existing.revision == revision
                && existing.files == files
            {
                return Arc::clone(&existing.snapshot);
            }

            let snapshot = Arc::new(build_snapshot(state, &files));
            if state.db.workspace_revision() != revision {
                // HIR resolution can lazily register imported or bundled
                // modules. Rebuild once that source set has stabilized so no
                // snapshot combines entries from different generations.
                continue;
            }
            *cached = Some(CachedWorkspaceIndex {
                revision,
                files,
                snapshot: Arc::clone(&snapshot),
            });
            return snapshot;
        }
    }
}

fn build_snapshot(state: &ServerState, files: &[(Url, SourceFile)]) -> WorkspaceIndexSnapshot {
    let mut symbols = Vec::new();
    let mut references: HashMap<NavigationTarget, Vec<Location>> = HashMap::new();

    for (uri, file) in files {
        let hir = aivi_query::hir_module(&state.db, *file);
        flatten_symbols(uri, hir.source(), hir.symbols(), &mut symbols);

        let navigation = NavigationAnalysis::load(&state.db, *file);
        for (target, location) in navigation.reference_entries(&state.db) {
            let locations = references.entry(target).or_default();
            if !locations.contains(&location) {
                locations.push(location);
            }
        }
    }

    let references = references
        .into_iter()
        .map(|(target, mut locations)| {
            sort_locations(&mut locations);
            locations.dedup();
            (target, Arc::<[Location]>::from(locations))
        })
        .collect();

    WorkspaceIndexSnapshot {
        symbols: Arc::from(symbols),
        references,
    }
}

fn flatten_symbols(
    uri: &Url,
    source: &aivi_base::SourceFile,
    roots: &[LspSymbol],
    out: &mut Vec<IndexedSymbol>,
) {
    let mut stack = roots
        .iter()
        .rev()
        .map(|symbol| (symbol, None))
        .collect::<Vec<_>>();
    while let Some((symbol, container_name)) = stack.pop() {
        let range = source.span_to_lsp_range(symbol.span.span());
        out.push(IndexedSymbol {
            name: symbol.name.clone(),
            normalized_name: symbol.name.to_ascii_lowercase(),
            kind: symbol.kind,
            location: Location {
                uri: uri.clone(),
                range: Range {
                    start: Position {
                        line: range.start.line,
                        character: range.start.character,
                    },
                    end: Position {
                        line: range.end.line,
                        character: range.end.character,
                    },
                },
            },
            container_name,
        });
        stack.extend(
            symbol
                .children
                .iter()
                .rev()
                .map(|child| (child, Some(symbol.name.clone()))),
        );
    }
}

fn sort_locations(locations: &mut [Location]) {
    locations.sort_by(|left, right| {
        left.uri
            .as_str()
            .cmp(right.uri.as_str())
            .then_with(|| compare_position(left.range.start, right.range.start))
            .then_with(|| compare_position(left.range.end, right.range.end))
    });
}

fn compare_position(left: Position, right: Position) -> Ordering {
    left.line
        .cmp(&right.line)
        .then_with(|| left.character.cmp(&right.character))
}

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, sync::Arc};

    use aivi_base::LspPosition;
    use tower_lsp::lsp_types::Url;

    use crate::{
        documents,
        navigation::{NavigationAnalysis, NavigationLookup},
        state::ServerState,
    };

    #[test]
    fn reuses_snapshot_until_a_real_source_mutation() {
        let state = ServerState::new();
        let uri = Url::from_file_path(PathBuf::from("/workspace-index/main.aivi"))
            .expect("test URI should be valid");
        documents::open_document(&state, &uri, 1, "value answer = 42".to_owned());

        let first = state.workspace_index.snapshot(&state);
        let first_revision = state.db.workspace_revision();
        let second = state.workspace_index.snapshot(&state);
        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(first.symbols()[0].name, "answer");

        documents::open_document(&state, &uri, 2, "value total = 42".to_owned());
        let third = state.workspace_index.snapshot(&state);
        assert!(!Arc::ptr_eq(&first, &third));
        assert!(state.db.workspace_revision() > first_revision);
        assert_eq!(third.symbols()[0].name, "total");
    }

    #[test]
    fn imported_file_change_invalidates_index_and_unchanged_importer_semantics() {
        let state = ServerState::new();
        let main_uri = Url::from_file_path(PathBuf::from("/workspace-index/main.aivi"))
            .expect("test URI should be valid");
        let target_uri = Url::from_file_path(PathBuf::from("/workspace-index/shared/logic.aivi"))
            .expect("test URI should be valid");
        let main = "use shared.logic (\n    liftOne\n)\n\nvalue lifted = liftOne 1\n";
        let target = "type Int -> Int\nfunc liftOne = input =>\n    input\n\nexport liftOne\n";
        documents::open_document(&state, &target_uri, 1, target.to_owned());
        documents::open_document(&state, &main_uri, 1, main.to_owned());

        let first = state.workspace_index.snapshot(&state);
        let main_file = state.file(&main_uri).expect("main document should be open");
        let before = NavigationAnalysis::load(&state.db, main_file)
            .definition_targets_at_lsp_position(
                &state.db,
                LspPosition {
                    line: 4,
                    character: 15,
                },
            );
        assert!(matches!(before, NavigationLookup::Targets(_)));

        documents::open_document(&state, &target_uri, 2, target.replace("liftOne", "liftTwo"));
        let second = state.workspace_index.snapshot(&state);
        assert!(!Arc::ptr_eq(&first, &second));
        assert!(
            second
                .symbols()
                .iter()
                .any(|symbol| symbol.name == "liftTwo")
        );
        assert!(
            !second
                .symbols()
                .iter()
                .any(|symbol| symbol.name == "liftOne")
        );

        let after = NavigationAnalysis::load(&state.db, main_file)
            .definition_targets_at_lsp_position(
                &state.db,
                LspPosition {
                    line: 4,
                    character: 15,
                },
            );
        assert!(
            !matches!(after, NavigationLookup::Targets(_)),
            "the unchanged importer must not retain a stale target after its dependency changes",
        );
    }
}
