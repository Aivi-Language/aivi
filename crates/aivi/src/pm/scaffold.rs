use crate::AiviError;
use std::path::{Path, PathBuf};

use super::aivi_toml::ProjectKind;

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

    let aivi_deps = aivi_path_dependency();

    let (entry_file, cargo_toml, aivi_toml, aivi_source) = match kind {
        ProjectKind::Bin => {
            let entry_file = "main.aivi";
            let aivi_toml = format!(
                "[project]\nkind = \"bin\"\nentry = \"{entry_file}\"\nlanguage_version = \"{language_version}\"\n\n[build]\ngen_dir = \"target/aivi-gen\"\nrust_edition = \"{edition}\"\ncargo_profile = \"dev\"\nnative_ui_target = \"portable\"\n"
            );
            let cargo_toml = format!(
                "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"{edition}\"\n\n[package.metadata.aivi]\nlanguage_version = \"{language_version}\"\nkind = \"bin\"\nentry = \"src/{entry_file}\"\n\n[[bin]]\nname = \"{name}\"\npath = \"target/aivi-gen/src/main.rs\"\n\n[dependencies]\n{}\n",
                aivi_deps
            );
            let aivi_source = starter_bin_source();
            (entry_file, cargo_toml, aivi_toml, aivi_source)
        }
        ProjectKind::Lib => {
            let entry_file = "lib.aivi";
            let aivi_toml = format!(
                "[project]\nkind = \"lib\"\nentry = \"{entry_file}\"\nlanguage_version = \"{language_version}\"\n\n[build]\ngen_dir = \"target/aivi-gen\"\nrust_edition = \"{edition}\"\ncargo_profile = \"dev\"\nnative_ui_target = \"portable\"\n"
            );
            let cargo_toml = format!(
                "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"{edition}\"\n\n[package.metadata.aivi]\nlanguage_version = \"{language_version}\"\nkind = \"lib\"\nentry = \"src/{entry_file}\"\n\n[lib]\npath = \"target/aivi-gen/src/lib.rs\"\n\n[dependencies]\n{}\n",
                aivi_deps
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
    let aivi_dir = manifest_dir.join(".");
    format!("aivi = {{ path = {:?} }}", aivi_dir.display().to_string())
}

fn starter_bin_source() -> &'static str {
    r#"module app.main
main : Effect Text Unit
main = do Effect {
  _ <- print \"Hello from AIVI!\"
  pure Unit
}
"#
}

fn starter_lib_source() -> &'static str {
    r#"module app.lib
hello : Text
hello = \"Hello from AIVI!\"
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
