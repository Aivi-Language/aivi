use crate::AiviError;
use std::ffi::OsStr;
use std::path::Path;

use super::aivi_toml::ProjectKind;
use super::cargo_metadata::parse_aivi_cargo_metadata;
use super::dep_spec::CargoDepSpec;

pub fn ensure_aivi_dependency(
    root: &Path,
    dep: &CargoDepSpec,
    required_language_version: Option<&str>,
) -> Result<(), AiviError> {
    let output = std::process::Command::new("cargo")
        .arg("metadata")
        .arg("--format-version")
        .arg("1")
        .current_dir(root)
        .output()?;
    if !output.status.success() {
        return Err(AiviError::Cargo(format!(
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }

    #[derive(serde::Deserialize)]
    struct CargoMetadata {
        packages: Vec<CargoMetadataPackage>,
    }

    #[derive(serde::Deserialize)]
    struct CargoMetadataPackage {
        name: String,
        manifest_path: String,
        metadata: serde_json::Value,
    }

    let metadata: CargoMetadata = serde_json::from_slice(&output.stdout)
        .map_err(|err| AiviError::Cargo(format!("failed to parse cargo metadata: {err}")))?;

    let package = match dep {
        CargoDepSpec::Path { path, .. } => {
            let dep_dir = Path::new(path);
            let resolved = if dep_dir.is_absolute() {
                dep_dir.to_path_buf()
            } else {
                root.join(dep_dir)
            };
            let manifest = if resolved.file_name() == Some(OsStr::new("Cargo.toml")) {
                resolved
            } else {
                resolved.join("Cargo.toml")
            };
            let expected = manifest.canonicalize().ok();
            metadata.packages.iter().find(|pkg| {
                if let Some(expected) = &expected {
                    let got = Path::new(&pkg.manifest_path).canonicalize().ok();
                    got.as_ref() == Some(expected)
                } else {
                    Path::new(&pkg.manifest_path).ends_with(&manifest)
                }
            })
        }
        CargoDepSpec::Registry { name, .. } | CargoDepSpec::Git { name, .. } => {
            metadata.packages.iter().find(|pkg| pkg.name == *name)
        }
    }
    .ok_or_else(|| {
        AiviError::Cargo(format!(
            "dependency {} not found in cargo metadata",
            dep.name()
        ))
    })?;

    let aivi = parse_aivi_cargo_metadata(&package.metadata).ok_or_else(|| {
        AiviError::Cargo(format!(
            "dependency {} is not an AIVI package (missing [package.metadata.aivi])",
            dep.name()
        ))
    })?;

    if aivi.kind != ProjectKind::Lib {
        return Err(AiviError::Cargo(format!(
            "dependency {} is an AIVI {} package; dependencies must be kind=\"lib\"",
            dep.name(),
            match aivi.kind {
                ProjectKind::Bin => "bin",
                ProjectKind::Lib => "lib",
            }
        )));
    }

    if let Some(required) = required_language_version {
        if aivi.language_version != required {
            return Err(AiviError::Cargo(format!(
                "dependency {} requires AIVI language_version {}, but project uses {}",
                dep.name(),
                aivi.language_version,
                required
            )));
        }
    }

    Ok(())
}
