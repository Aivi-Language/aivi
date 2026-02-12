use crate::AiviError;
use serde::Deserialize;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use toml_edit::{value, DocumentMut, Item, Table};

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
    #[serde(default = "default_codegen")]
    pub codegen: Codegen,
}

impl Default for AiviTomlBuild {
    fn default() -> Self {
        Self {
            gen_dir: default_gen_dir(),
            rust_edition: default_rust_edition(),
            cargo_profile: default_cargo_profile(),
            codegen: default_codegen(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Codegen {
    /// Emit Rust that embeds AIVI HIR as JSON and runs via the interpreter runtime.
    Embed,
    /// Emit standalone Rust logic (experimental; limited language/builtins support).
    Native,
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

fn default_codegen() -> Codegen {
    Codegen::Embed
}

pub fn read_aivi_toml(path: &Path) -> Result<AiviToml, AiviError> {
    let text = std::fs::read_to_string(path)?;
    toml::from_str(&text)
        .map_err(|err| AiviError::Config(format!("failed to parse {}: {err}", path.display())))
}

pub fn write_scaffold(
    dir: &Path,
    name: &str,
    kind: ProjectKind,
    edition: &str,
    language_version: &str,
    force: bool,
) -> Result<(), AiviError> {
    validate_package_name(name)?;
    if dir.exists() {
        let mut iter = std::fs::read_dir(dir)?;
        if iter.next().is_some() && !force {
            return Err(AiviError::Io(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                format!(
                    "refusing to initialize non-empty directory {}",
                    dir.display()
                ),
            )));
        }
    } else {
        std::fs::create_dir_all(dir)?;
    }

    let src_dir = dir.join("src");
    std::fs::create_dir_all(&src_dir)?;

    let (entry_file, cargo_toml, aivi_toml, aivi_source) = match kind {
        ProjectKind::Bin => {
            let entry_file = "main.aivi";
            let aivi_toml = format!(
                "[project]\nkind = \"bin\"\nentry = \"{entry_file}\"\nlanguage_version = \"{language_version}\"\n\n[build]\ngen_dir = \"target/aivi-gen\"\nrust_edition = \"{edition}\"\ncargo_profile = \"dev\"\ncodegen = \"native\"\n"
            );
            let cargo_toml = format!(
                "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"{edition}\"\n\n[package.metadata.aivi]\nlanguage_version = \"{language_version}\"\nkind = \"bin\"\nentry = \"src/{entry_file}\"\n\n[[bin]]\nname = \"{name}\"\npath = \"target/aivi-gen/src/main.rs\"\n\n[dependencies]\n{}\nserde_json = \"1.0\"\n",
                format!("{}\n{}", aivi_path_dependency(), aivi_native_runtime_path_dependency())
            );
            let aivi_source = starter_bin_source();
            (entry_file, cargo_toml, aivi_toml, aivi_source)
        }
        ProjectKind::Lib => {
            let entry_file = "lib.aivi";
            let aivi_toml = format!(
                "[project]\nkind = \"lib\"\nentry = \"{entry_file}\"\nlanguage_version = \"{language_version}\"\n\n[build]\ngen_dir = \"target/aivi-gen\"\nrust_edition = \"{edition}\"\ncargo_profile = \"dev\"\ncodegen = \"native\"\n"
            );
            let cargo_toml = format!(
                "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"{edition}\"\n\n[package.metadata.aivi]\nlanguage_version = \"{language_version}\"\nkind = \"lib\"\nentry = \"src/{entry_file}\"\n\n[lib]\npath = \"target/aivi-gen/src/lib.rs\"\n\n[dependencies]\n{}\nserde_json = \"1.0\"\n",
                format!("{}\n{}", aivi_path_dependency(), aivi_native_runtime_path_dependency())
            );
            let aivi_source = starter_lib_source();
            (entry_file, cargo_toml, aivi_toml, aivi_source)
        }
    };

    std::fs::write(dir.join("aivi.toml"), aivi_toml)?;
    std::fs::write(dir.join("Cargo.toml"), cargo_toml)?;
    std::fs::write(dir.join(".gitignore"), "/target\n**/target\n")?;
    std::fs::write(src_dir.join(entry_file), aivi_source)?;

    Ok(())
}

fn aivi_path_dependency() -> String {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    format!(
        "aivi = {{ path = {:?} }}",
        manifest_dir.display().to_string()
    )
}

fn aivi_native_runtime_path_dependency() -> String {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let rt_dir = manifest_dir.join("../aivi_native_runtime");
    format!(
        "aivi_native_runtime = {{ path = {:?} }}",
        rt_dir.display().to_string()
    )
}

fn starter_bin_source() -> &'static str {
    r#"module app.main
main : Effect Text Unit
main = effect {
  _ <- print "Hello from AIVI!"
  pure Unit
}
"#
}

fn starter_lib_source() -> &'static str {
    r#"module app.lib
hello : Text
hello = "Hello from AIVI!"
"#
}

fn validate_package_name(name: &str) -> Result<(), AiviError> {
    if name.is_empty() {
        return Err(AiviError::InvalidCommand(
            "name must not be empty".to_string(),
        ));
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(AiviError::InvalidCommand(format!(
            "invalid name {name}: use lowercase letters, digits, and '-'"
        )));
    }
    if name.starts_with('-') || name.ends_with('-') || name.contains("--") {
        return Err(AiviError::InvalidCommand(format!("invalid name {name}")));
    }
    Ok(())
}

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

        let (name, version_req) = match spec.split_once('@') {
            Some((name, "latest")) => (name, "*"),
            Some((name, version_req)) => (name, version_req),
            None => (spec, "*"),
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
            version_req: version_req.trim().to_string(),
        })
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

pub fn collect_aivi_sources(src_dir: &Path) -> Result<Vec<PathBuf>, AiviError> {
    let mut paths = Vec::new();
    if !src_dir.exists() {
        return Ok(paths);
    }
    collect_aivi_sources_inner(src_dir, &mut paths)?;
    paths.sort();
    Ok(paths)
}

fn collect_aivi_sources_inner(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), AiviError> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_aivi_sources_inner(&path, out)?;
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) == Some("aivi") {
            out.push(path);
        }
    }
    Ok(())
}
