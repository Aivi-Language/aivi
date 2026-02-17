use crate::AiviError;
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProjectKind {
    Bin,
    Lib,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AiviToml {
    pub project: AiviTomlProject,
    #[serde(default)]
    pub build: AiviTomlBuild,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AiviTomlProject {
    pub kind: ProjectKind,
    pub entry: String,
    #[serde(default)]
    pub language_version: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AiviTomlBuild {
    #[serde(default = "default_gen_dir")]
    pub gen_dir: String,
    #[serde(default = "default_rust_edition")]
    pub rust_edition: String,
    #[serde(default = "default_cargo_profile")]
    pub cargo_profile: String,
}

impl Default for AiviTomlBuild {
    fn default() -> Self {
        Self {
            gen_dir: default_gen_dir(),
            rust_edition: default_rust_edition(),
            cargo_profile: default_cargo_profile(),
        }
    }
}

fn default_gen_dir() -> String {
    "target/aivi-gen".to_string()
}

fn default_rust_edition() -> String {
    "2024".to_string()
}

fn default_cargo_profile() -> String {
    "dev".to_string()
}

pub fn read_aivi_toml(path: &Path) -> Result<AiviToml, AiviError> {
    let text = std::fs::read_to_string(path)?;
    toml::from_str(&text)
        .map_err(|err| AiviError::Config(format!("failed to parse {}: {err}", path.display())))
}
