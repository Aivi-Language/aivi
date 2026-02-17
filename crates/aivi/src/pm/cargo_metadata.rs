use super::aivi_toml::ProjectKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiviCargoMetadata {
    pub language_version: String,
    pub kind: ProjectKind,
    pub entry: Option<String>,
}

pub fn parse_aivi_cargo_metadata(value: &serde_json::Value) -> Option<AiviCargoMetadata> {
    let aivi = value.get("aivi")?.as_object()?;
    let language_version = aivi
        .get("language_version")
        .and_then(serde_json::Value::as_str)?
        .to_string();
    let kind = match aivi.get("kind").and_then(serde_json::Value::as_str)? {
        "bin" => ProjectKind::Bin,
        "lib" => ProjectKind::Lib,
        _ => return None,
    };
    let entry = aivi
        .get("entry")
        .and_then(serde_json::Value::as_str)
        .map(|s| s.to_string());
    Some(AiviCargoMetadata {
        language_version,
        kind,
        entry,
    })
}
