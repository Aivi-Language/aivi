mod aivi_toml;
mod cargo_edit;
mod cargo_metadata;
mod dep_spec;
mod ensure_dep;
mod publish;
mod scaffold;
mod sources;

pub use aivi_toml::{AiviToml, AiviTomlBuild, AiviTomlProject, ProjectKind, read_aivi_toml};
pub use cargo_edit::{CargoManifestEdits, edit_cargo_toml_dependencies};
pub use cargo_metadata::{AiviCargoMetadata, parse_aivi_cargo_metadata};
pub use dep_spec::{CargoDepSpec, CargoDepSpecParseError};
pub use ensure_dep::ensure_aivi_dependency;
pub use publish::validate_publish_preflight;
pub use scaffold::write_scaffold;
pub use sources::collect_aivi_sources;
