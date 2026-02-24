use std::ffi::OsStr;
use std::path::Path;
use toml_edit::DocumentMut;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CargoDepSpec {
    Registry {
        name: String,
        version_req: String,
    },
    Git {
        name: String,
        git: String,
        rev: Option<String>,
    },
    Path {
        name: String,
        path: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct CargoDepSpecParseError(pub String);

impl CargoDepSpec {
    pub fn parse(spec: &str) -> Result<Self, CargoDepSpecParseError> {
        if let Some(path) = spec.strip_prefix("path:") {
            let path = path.trim();
            if path.is_empty() {
                return Err(CargoDepSpecParseError(
                    "path: spec must include a path".to_string(),
                ));
            }
            let name = infer_name_from_path(Path::new(path)).ok_or_else(|| {
                CargoDepSpecParseError("failed to infer crate name from path".to_string())
            })?;
            return Ok(Self::Path {
                name,
                path: path.to_string(),
            });
        }

        if let Some(rest) = spec.strip_prefix("git+") {
            let (git, rev) = split_git_rev(rest)?;
            let name = infer_name_from_git_url(&git).ok_or_else(|| {
                CargoDepSpecParseError("failed to infer crate name from git url".to_string())
            })?;
            return Ok(Self::Git { name, git, rev });
        }

        let (name, version_req_raw) = match spec.split_once('@') {
            Some((name, version_req)) => (name, version_req),
            None => (spec, "*"),
        };
        let version_req = version_req_raw.trim();
        let version_req = if version_req == "latest" {
            "*"
        } else {
            version_req
        };
        let name = name.trim();
        if name.is_empty() {
            return Err(CargoDepSpecParseError("missing crate name".to_string()));
        }
        if !name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
        {
            return Err(CargoDepSpecParseError(format!("invalid crate name {name}")));
        }

        Ok(Self::Registry {
            name: name.to_string(),
            version_req: version_req.to_string(),
        })
    }

    pub fn parse_in(root: &Path, spec: &str) -> Result<Self, CargoDepSpecParseError> {
        if let Some(path) = spec.strip_prefix("path:") {
            let path = path.trim();
            if path.is_empty() {
                return Err(CargoDepSpecParseError(
                    "path: spec must include a path".to_string(),
                ));
            }
            let package_name = read_package_name_for_path_dep(root, Path::new(path))
                .ok()
                .flatten()
                .or_else(|| infer_name_from_path(Path::new(path)))
                .ok_or_else(|| {
                    CargoDepSpecParseError("failed to infer crate name from path".to_string())
                })?;
            return Ok(Self::Path {
                name: package_name,
                path: path.to_string(),
            });
        }

        Self::parse(spec)
    }

    pub fn name(&self) -> &str {
        match self {
            CargoDepSpec::Registry { name, .. } => name,
            CargoDepSpec::Git { name, .. } => name,
            CargoDepSpec::Path { name, .. } => name,
        }
    }
}

fn split_git_rev(input: &str) -> Result<(String, Option<String>), CargoDepSpecParseError> {
    let Some((url, fragment)) = input.split_once('#') else {
        return Ok((input.to_string(), None));
    };
    if fragment.is_empty() {
        return Ok((url.to_string(), None));
    }
    let mut rev = None;
    for pair in fragment.split('&') {
        let (key, value) = pair
            .split_once('=')
            .ok_or_else(|| CargoDepSpecParseError(format!("invalid git fragment {pair}")))?;
        if key == "rev" {
            rev = Some(value.to_string());
        }
    }
    Ok((url.to_string(), rev))
}

fn infer_name_from_git_url(url: &str) -> Option<String> {
    let url = url.trim_end_matches('/');
    let last = url.rsplit('/').next()?;
    let last = last.strip_suffix(".git").unwrap_or(last);
    (!last.is_empty()).then(|| last.replace('.', "-"))
}

fn infer_name_from_path(path: &Path) -> Option<String> {
    let base = path.file_name().and_then(OsStr::to_str)?;
    (!base.is_empty()).then(|| base.to_string())
}

fn read_package_name_for_path_dep(
    root: &Path,
    path: &Path,
) -> Result<Option<String>, CargoDepSpecParseError> {
    let resolved = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    };
    let cargo_toml_path = if resolved.file_name() == Some(OsStr::new("Cargo.toml")) {
        resolved
    } else {
        resolved.join("Cargo.toml")
    };
    let text = match std::fs::read_to_string(&cargo_toml_path) {
        Ok(text) => text,
        Err(_) => return Ok(None),
    };
    let doc = match text.parse::<DocumentMut>() {
        Ok(doc) => doc,
        Err(_) => return Ok(None),
    };
    Ok(doc
        .get("package")
        .and_then(|t| t.get("name"))
        .and_then(|i| i.as_str())
        .map(|s| s.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_registry_latest_with_whitespace_maps_to_wildcard() {
        let parsed = CargoDepSpec::parse("serde @ latest").expect("parse dep spec");
        assert_eq!(
            parsed,
            CargoDepSpec::Registry {
                name: "serde".to_string(),
                version_req: "*".to_string(),
            }
        );
    }

    #[test]
    fn parse_path_spec_trims_whitespace() {
        let parsed = CargoDepSpec::parse("path: ./dep ").expect("parse dep spec");
        assert_eq!(
            parsed,
            CargoDepSpec::Path {
                name: "dep".to_string(),
                path: "./dep".to_string(),
            }
        );
    }

    #[test]
    fn split_git_rev_extracts_rev_parameter() {
        let (git, rev) =
            split_git_rev("https://example.com/org/repo.git?x=y#rev=abc123").expect("split git");
        assert_eq!(git, "https://example.com/org/repo.git?x=y");
        assert_eq!(rev.as_deref(), Some("abc123"));
    }

    #[test]
    fn split_git_rev_rejects_invalid_fragment_pairs() {
        let err = split_git_rev("https://example.com/org/repo.git#broken").unwrap_err();
        assert!(err.0.contains("invalid git fragment"));
    }
}
