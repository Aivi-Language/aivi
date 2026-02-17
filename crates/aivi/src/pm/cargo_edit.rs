use crate::AiviError;
use toml_edit::{value, DocumentMut, Item, Table};

use super::dep_spec::CargoDepSpec;

pub struct CargoManifestEdits {
    pub updated_manifest: String,
    pub changed: bool,
}

pub fn edit_cargo_toml_dependencies(
    cargo_toml_text: &str,
    dep: &CargoDepSpec,
) -> Result<CargoManifestEdits, AiviError> {
    let mut doc = cargo_toml_text
        .parse::<DocumentMut>()
        .map_err(|err| AiviError::Cargo(format!("failed to parse Cargo.toml: {err}")))?;

    if !doc.as_table().contains_key("package") {
        return Err(AiviError::Cargo(
            "missing [package] in Cargo.toml".to_string(),
        ));
    }

    if doc["dependencies"].is_none() {
        doc["dependencies"] = Item::Table(Table::new());
    }

    let deps = doc["dependencies"]
        .as_table_mut()
        .ok_or_else(|| AiviError::Cargo("[dependencies] must be a table".to_string()))?;

    let name = dep.name();
    let before = deps.get(name).map(|i| i.to_string());
    let item = match dep {
        CargoDepSpec::Registry { version_req, .. } => value(version_req.as_str()),
        CargoDepSpec::Git { git, rev, .. } => {
            let mut t = Table::new();
            t.set_implicit(true);
            t["git"] = value(git.as_str());
            if let Some(rev) = rev {
                t["rev"] = value(rev.as_str());
            }
            Item::Table(t)
        }
        CargoDepSpec::Path { path, .. } => {
            let mut t = Table::new();
            t.set_implicit(true);
            t["path"] = value(path.as_str());
            Item::Table(t)
        }
    };
    deps[name] = item;

    let after = deps.get(name).map(|i| i.to_string());
    Ok(CargoManifestEdits {
        updated_manifest: doc.to_string(),
        changed: before != after,
    })
}
