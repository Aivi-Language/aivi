use crate::AiviError;
use std::path::Path;
use toml_edit::DocumentMut;

use super::aivi_toml::{AiviToml, ProjectKind};

pub fn validate_publish_preflight(project_root: &Path, cfg: &AiviToml) -> Result<(), AiviError> {
    let aivi_toml_path = project_root.join("aivi.toml");
    let cargo_toml_path = project_root.join("Cargo.toml");
    if !aivi_toml_path.exists() || !cargo_toml_path.exists() {
        return Err(AiviError::Config(
            "publish expects a directory containing aivi.toml and Cargo.toml".to_string(),
        ));
    }

    let cargo_text = std::fs::read_to_string(&cargo_toml_path)?;
    let doc = cargo_text
        .parse::<DocumentMut>()
        .map_err(|err| AiviError::Cargo(format!("failed to parse Cargo.toml: {err}")))?;

    let aivi = doc
        .get("package")
        .and_then(|p| p.get("metadata"))
        .and_then(|m| m.get("aivi"))
        .and_then(|t| t.as_table())
        .ok_or_else(|| {
            AiviError::Cargo(
                "missing [package.metadata.aivi] in Cargo.toml (required for AIVI packages)"
                    .to_string(),
            )
        })?;

    let language_version = aivi
        .get("language_version")
        .and_then(|i| i.as_str())
        .ok_or_else(|| {
            AiviError::Cargo("missing [package.metadata.aivi].language_version".to_string())
        })?;
    let kind = aivi
        .get("kind")
        .and_then(|i| i.as_str())
        .ok_or_else(|| AiviError::Cargo("missing [package.metadata.aivi].kind".to_string()))?;
    let entry = aivi.get("entry").and_then(|i| i.as_str());

    let expected_kind = match cfg.project.kind {
        ProjectKind::Bin => "bin",
        ProjectKind::Lib => "lib",
    };
    if kind != expected_kind {
        return Err(AiviError::Cargo(format!(
            "Cargo.toml [package.metadata.aivi].kind is {kind}, but aivi.toml project.kind is {expected_kind}"
        )));
    }

    if let Some(required) = cfg.project.language_version.as_deref() {
        if language_version != required {
            return Err(AiviError::Cargo(format!(
                "Cargo.toml [package.metadata.aivi].language_version is {language_version}, but aivi.toml project.language_version is {required}"
            )));
        }
    }

    let expected_entry = expected_cargo_entry_for_project(&cfg.project.entry);
    let Some(entry) = entry else {
        return Err(AiviError::Cargo(
            "missing [package.metadata.aivi].entry".to_string(),
        ));
    };
    if entry != expected_entry {
        return Err(AiviError::Cargo(format!(
            "Cargo.toml [package.metadata.aivi].entry is {entry}, expected {expected_entry}"
        )));
    }

    Ok(())
}

fn expected_cargo_entry_for_project(entry: &str) -> String {
    let entry_path = Path::new(entry);
    if entry_path.components().count() == 1 {
        format!("src/{entry}")
    } else {
        entry.to_string()
    }
}
