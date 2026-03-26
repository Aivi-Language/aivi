use std::{
    error::Error,
    fmt,
    path::{Path, PathBuf},
};

use crate::{discover_workspace_root, discover_workspace_root_from_directory};

/// How the entrypoint path was chosen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntrypointOrigin {
    ExplicitPath,
    ImplicitWorkspaceMain,
}

/// A v1 entrypoint selection paired with the workspace root it should compile
/// against.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedEntrypoint {
    entry_path: PathBuf,
    workspace_root: PathBuf,
    origin: EntrypointOrigin,
}

impl ResolvedEntrypoint {
    fn new(entry_path: PathBuf, workspace_root: PathBuf, origin: EntrypointOrigin) -> Self {
        Self {
            entry_path,
            workspace_root,
            origin,
        }
    }

    pub fn entry_path(&self) -> &Path {
        &self.entry_path
    }

    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    pub fn origin(&self) -> EntrypointOrigin {
        self.origin
    }
}

/// v1 entry discovery can only fail when the implicit `<workspace-root>/main.aivi`
/// target is absent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EntrypointResolutionError {
    MissingImplicitEntrypoint {
        workspace_root: PathBuf,
        expected_path: PathBuf,
    },
}

impl EntrypointResolutionError {
    pub fn workspace_root(&self) -> &Path {
        match self {
            Self::MissingImplicitEntrypoint { workspace_root, .. } => workspace_root,
        }
    }

    pub fn expected_path(&self) -> &Path {
        match self {
            Self::MissingImplicitEntrypoint { expected_path, .. } => expected_path,
        }
    }
}

impl fmt::Display for EntrypointResolutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingImplicitEntrypoint { expected_path, .. } => write!(
                f,
                "expected implicit entry file at {}; pass `--path <entry-file>` to choose a different entry file",
                expected_path.display()
            ),
        }
    }
}

impl Error for EntrypointResolutionError {}

/// Resolve the v1 entrypoint contract for tooling that starts from a current
/// working directory and an optional explicit `--path` override.
pub fn resolve_v1_entrypoint(
    current_dir: &Path,
    explicit_path: Option<&Path>,
) -> Result<ResolvedEntrypoint, EntrypointResolutionError> {
    if let Some(explicit_path) = explicit_path {
        return Ok(ResolvedEntrypoint::new(
            explicit_path.to_path_buf(),
            discover_workspace_root(explicit_path),
            EntrypointOrigin::ExplicitPath,
        ));
    }

    let workspace_root = discover_workspace_root_from_directory(current_dir);
    let entry_path = workspace_root.join("main.aivi");
    if !entry_path.is_file() {
        return Err(EntrypointResolutionError::MissingImplicitEntrypoint {
            workspace_root,
            expected_path: entry_path,
        });
    }

    Ok(ResolvedEntrypoint::new(
        entry_path,
        workspace_root,
        EntrypointOrigin::ImplicitWorkspaceMain,
    ))
}
