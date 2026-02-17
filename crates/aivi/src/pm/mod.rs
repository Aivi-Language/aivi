mod aivi_toml;
mod cargo_edit;
mod cargo_metadata;
mod dep_spec;
mod ensure_dep;
mod publish;
mod scaffold;
mod sources;

pub use aivi_toml::{read_aivi_toml, AiviToml, ProjectKind};
pub use cargo_edit::{edit_cargo_toml_dependencies, CargoManifestEdits};
pub use cargo_metadata::AiviCargoMetadata;
pub use dep_spec::{CargoDepSpec, CargoDepSpecParseError};
pub use ensure_dep::ensure_aivi_dependency;
pub use publish::validate_publish_preflight;
pub use scaffold::write_scaffold;
pub use sources::collect_aivi_sources;

pub type AiviTomlBuild = aivi_toml::AiviTomlBuild;
pub type AiviTomlProject = aivi_toml::AiviTomlProject;

pub fn parse_aivi_cargo_metadata(value: &serde_json::Value) -> Option<AiviCargoMetadata> {
    cargo_metadata::parse_aivi_cargo_metadata(value)
}
