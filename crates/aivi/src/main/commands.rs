fn cmd_init(args: &[String]) -> Result<(), AiviError> {
    let mut name = None;
    let mut kind = ProjectKind::Bin;
    let mut edition = "2024".to_string();
    let mut language_version = "0.1".to_string();
    let mut force = false;

    let mut iter = args.iter().cloned();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--bin" => kind = ProjectKind::Bin,
            "--lib" => kind = ProjectKind::Lib,
            "--edition" => {
                let Some(value) = iter.next() else {
                    return Err(AiviError::InvalidCommand(
                        "--edition expects a value".to_string(),
                    ));
                };
                edition = value;
            }
            "--language-version" => {
                let Some(value) = iter.next() else {
                    return Err(AiviError::InvalidCommand(
                        "--language-version expects a value".to_string(),
                    ));
                };
                language_version = value;
            }
            "--force" => force = true,
            _ if arg.starts_with('-') => {
                return Err(AiviError::InvalidCommand(format!("unknown flag {arg}")))
            }
            _ => {
                if name.is_some() {
                    return Err(AiviError::InvalidCommand(format!(
                        "unexpected argument {arg}"
                    )));
                }
                name = Some(arg);
            }
        }
    }

    let Some(name) = name else {
        return Err(AiviError::InvalidCommand("init expects <name>".to_string()));
    };

    let dir = PathBuf::from(&name);
    write_scaffold(&dir, &name, kind, &edition, &language_version, force)?;
    println!("{}", dir.display());
    Ok(())
}

fn cmd_clean(args: &[String]) -> Result<(), AiviError> {
    let mut all = false;
    for arg in args {
        match arg.as_str() {
            "--all" => all = true,
            _ if arg.starts_with('-') => {
                return Err(AiviError::InvalidCommand(format!("unknown flag {arg}")))
            }
            _ => {
                return Err(AiviError::InvalidCommand(format!(
                    "unexpected argument {arg}"
                )))
            }
        }
    }

    let root = env::current_dir()?;
    let gen_dir: String = if root.join("aivi.toml").exists() {
        aivi::read_aivi_toml(&root.join("aivi.toml"))?.build.gen_dir
    } else {
        "target/aivi-gen".to_string()
    };
    let gen_dir = root.join(gen_dir);
    if gen_dir.exists() {
        std::fs::remove_dir_all(&gen_dir)?;
    }
    if all {
        let status = Command::new("cargo")
            .arg("clean")
            .current_dir(&root)
            .status()?;
        if !status.success() {
            return Err(AiviError::Cargo("cargo clean failed".to_string()));
        }
    }
    Ok(())
}

fn cmd_search(args: &[String]) -> Result<(), AiviError> {
    let query = args
        .first()
        .ok_or_else(|| AiviError::InvalidCommand("search expects <query>".to_string()))?;
    let keyword_query = format!("keyword:aivi {query}");
    let output = Command::new("cargo")
        .arg("search")
        .arg(keyword_query)
        .arg("--limit")
        .arg("20")
        .output()?;
    if !output.status.success() {
        return Err(AiviError::Cargo(format!(
            "cargo search failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    print!("{}", String::from_utf8_lossy(&output.stdout));
    Ok(())
}

fn cmd_install(args: &[String]) -> Result<(), AiviError> {
    let mut fetch = true;
    let mut spec = None;

    for arg in args.iter().cloned() {
        match arg.as_str() {
            "--no-fetch" => fetch = false,
            _ if arg.starts_with('-') => {
                return Err(AiviError::InvalidCommand(format!("unknown flag {arg}")))
            }
            _ => {
                if spec.is_some() {
                    return Err(AiviError::InvalidCommand(format!(
                        "unexpected argument {arg}"
                    )));
                }
                spec = Some(arg);
            }
        }
    }

    let Some(spec) = spec else {
        return Err(AiviError::InvalidCommand(
            "install expects <spec>".to_string(),
        ));
    };

    let root = env::current_dir()?;
    if !root.join("aivi.toml").exists() || !root.join("Cargo.toml").exists() {
        return Err(AiviError::Config(
            "install expects a directory containing aivi.toml and Cargo.toml".to_string(),
        ));
    }
    let cfg = aivi::read_aivi_toml(&root.join("aivi.toml"))?;

    if install_stdlib_module(&root, &spec)? {
        return Ok(());
    }

    let dep = CargoDepSpec::parse_in(&root, &spec)
        .map_err(|err| AiviError::InvalidCommand(err.to_string()))?;

    let cargo_toml_path = root.join("Cargo.toml");
    let original = std::fs::read_to_string(&cargo_toml_path)?;
    let cargo_lock_path = root.join("Cargo.lock");
    let original_lock = std::fs::read_to_string(&cargo_lock_path).ok();
    let edits = aivi::edit_cargo_toml_dependencies(&original, &dep)?;
    if edits.changed {
        std::fs::write(&cargo_toml_path, edits.updated_manifest)?;
    }

    if fetch {
        let status = Command::new("cargo")
            .arg("fetch")
            .current_dir(&root)
            .status()?;
        if !status.success() {
            restore_install_manifest(
                &cargo_toml_path,
                &original,
                &cargo_lock_path,
                &original_lock,
            );
            return Err(AiviError::Cargo("cargo fetch failed".to_string()));
        }
    }

    if let Err(err) = ensure_aivi_dependency(&root, &dep, cfg.project.language_version.as_deref()) {
        restore_install_manifest(
            &cargo_toml_path,
            &original,
            &cargo_lock_path,
            &original_lock,
        );
        return Err(err);
    }

    Ok(())
}

fn restore_install_manifest(
    cargo_toml_path: &Path,
    original: &str,
    cargo_lock_path: &Path,
    original_lock: &Option<String>,
) {
    let _ = std::fs::write(cargo_toml_path, original);
    match original_lock {
        Some(contents) => {
            let _ = std::fs::write(cargo_lock_path, contents);
        }
        None => {
            let _ = std::fs::remove_file(cargo_lock_path);
        }
    }
}

fn cmd_package(args: &[String]) -> Result<(), AiviError> {
    let mut allow_dirty = false;
    let mut no_verify = false;
    let mut cargo_args = Vec::new();

    let mut saw_sep = false;
    for arg in args.iter().cloned() {
        if !saw_sep && arg == "--" {
            saw_sep = true;
            continue;
        }
        if saw_sep {
            cargo_args.push(arg);
            continue;
        }
        match arg.as_str() {
            "--allow-dirty" => allow_dirty = true,
            "--no-verify" => no_verify = true,
            _ if arg.starts_with('-') => {
                return Err(AiviError::InvalidCommand(format!("unknown flag {arg}")))
            }
            _ => {
                return Err(AiviError::InvalidCommand(format!(
                    "unexpected argument {arg}"
                )))
            }
        }
    }

    let root = env::current_dir()?;
    let cfg = aivi::read_aivi_toml(&root.join("aivi.toml"))?;
    validate_publish_preflight(&root, &cfg)?;

    let mut cmd = Command::new("cargo");
    cmd.arg("package");
    if allow_dirty {
        cmd.arg("--allow-dirty");
    }
    if no_verify {
        cmd.arg("--no-verify");
    }
    cmd.args(cargo_args);
    let status = cmd.current_dir(&root).status()?;
    if !status.success() {
        return Err(AiviError::Cargo("cargo package failed".to_string()));
    }
    Ok(())
}

fn cmd_publish(args: &[String]) -> Result<(), AiviError> {
    let mut dry_run = false;
    let mut allow_dirty = false;
    let mut no_verify = false;
    let mut cargo_args = Vec::new();

    let mut saw_sep = false;
    for arg in args.iter().cloned() {
        if !saw_sep && arg == "--" {
            saw_sep = true;
            continue;
        }
        if saw_sep {
            cargo_args.push(arg);
            continue;
        }
        match arg.as_str() {
            "--dry-run" => dry_run = true,
            "--allow-dirty" => allow_dirty = true,
            "--no-verify" => no_verify = true,
            _ if arg.starts_with('-') => {
                return Err(AiviError::InvalidCommand(format!("unknown flag {arg}")))
            }
            _ => {
                return Err(AiviError::InvalidCommand(format!(
                    "unexpected argument {arg}"
                )))
            }
        }
    }

    let root = env::current_dir()?;
    let cfg = aivi::read_aivi_toml(&root.join("aivi.toml"))?;
    validate_publish_preflight(&root, &cfg)?;

    let mut cmd = Command::new("cargo");
    cmd.arg("publish");
    if dry_run {
        cmd.arg("--dry-run");
    }
    if allow_dirty {
        cmd.arg("--allow-dirty");
    }
    if no_verify {
        cmd.arg("--no-verify");
    }
    cmd.args(cargo_args);
    let status = cmd.current_dir(&root).status()?;
    if !status.success() {
        return Err(AiviError::Cargo("cargo publish failed".to_string()));
    }
    Ok(())
}

fn install_stdlib_module(root: &Path, spec: &str) -> Result<bool, AiviError> {
    let module_name = if spec.starts_with("aivi.") {
        spec.to_string()
    } else if spec.starts_with("std.") {
        format!("aivi.{spec}")
    } else {
        return Ok(false);
    };

    let Some(source) = embedded_stdlib_source(&module_name) else {
        return Ok(false);
    };

    let rel_path = module_name.replace('.', "/") + ".aivi";
    let out_path = root.join("src").join(rel_path);
    if out_path.exists() {
        return Ok(true);
    }
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(out_path, source)?;
    Ok(true)
}

fn should_use_project_pipeline(args: &[String]) -> bool {
    // Always prefer the project pipeline when aivi.toml is present,
    // even if the user passes a file path (e.g. `aivi run src/main.aivi`).
    if env::current_dir()
        .map(|d| d.join("aivi.toml").exists())
        .unwrap_or(false)
    {
        return true;
    }
    if args.is_empty() {
        return true;
    }
    let first = &args[0];
    if first == "--" || first.starts_with('-') {
        return true;
    }
    false
}

fn cmd_project_build(args: &[String]) -> Result<(), AiviError> {
    let root = env::current_dir()?;
    let cfg = aivi::read_aivi_toml(&root.join("aivi.toml"))?;
    let proj_args = parse_project_args(args)?;
    let release = proj_args.release || cfg.build.cargo_profile == "release";

    cmd_project_build_cranelift(&root, &cfg, release)
}

/// Build an AIVI project using the Cranelift AOT pipeline.
///
/// 1. Compile all AIVI code to a native object file (.o)
/// 2. Write the object to target/aivi-gen/
/// 3. Link with the system linker to produce a standalone binary
fn cmd_project_build_cranelift(
    root: &Path,
    cfg: &aivi::AiviToml,
    release: bool,
) -> Result<(), AiviError> {
    let source_target = resolve_project_source_target(root, &cfg.project.entry);
    let target_str = source_target
        .to_str()
        .ok_or_else(|| AiviError::InvalidPath(source_target.display().to_string()))?;

    let (program, cg_types, monomorph_plan) = aivi::desugar_target_with_cg_types(target_str)?;

    // 1. Compile to object file
    eprintln!("  Compiling AIVI → Cranelift AOT...");
    let object_bytes = aivi::compile_to_object(program, cg_types, monomorph_plan)?;

    // 2. Write the object file
    let gen_dir = root.join(&cfg.build.gen_dir);
    std::fs::create_dir_all(&gen_dir)?;
    let obj_path = gen_dir.join("aivi_program.o");
    let obj_path_abs = std::fs::canonicalize(gen_dir.as_path())
        .unwrap_or_else(|_| gen_dir.clone())
        .join("aivi_program.o");
    std::fs::write(&obj_path, &object_bytes)?;

    // 3. Generate a thin Rust harness that builds the runtime and calls __aivi_main
    let harness_path = gen_dir.join("src");
    std::fs::create_dir_all(&harness_path)?;
    let harness_main = harness_path.join("main.rs");
    let harness_code = generate_aot_harness("aivi_program");
    std::fs::write(&harness_main, harness_code)?;

    // 4. Generate build.rs that links the compiled object file
    let build_rs_path = root.join("build.rs");
    let build_rs_code = generate_aot_build_rs(&obj_path_abs);
    std::fs::write(&build_rs_path, build_rs_code)?;

    // 5. Compile with cargo, linking the .o file
    eprintln!("  Linking...");
    let mut cmd = Command::new("cargo");
    cmd.arg("build");
    if release {
        cmd.arg("--release");
    }
    let status = cmd.current_dir(root).status()?;
    if !status.success() {
        return Err(AiviError::Cargo(
            "cargo build (cranelift aot) failed".to_string(),
        ));
    }

    eprintln!("  Done.");
    Ok(())
}

/// Generate the Rust harness that the AOT binary's main() calls.
fn generate_aot_harness(project_name: &str) -> String {
    format!(
        r#"//! Auto-generated AOT harness for {project_name}
//! This file is generated by `aivi build --cranelift`.

unsafe extern "C" {{
    fn __aivi_main(ctx: i64) -> i64;
}}

fn main() {{
    // Create a base runtime with builtins only.
    // __aivi_main registers all AOT-compiled functions before running main.
    let ctx = aivi::init_aot_runtime_base();
    let _result = unsafe {{ __aivi_main(ctx as i64) }};
    aivi::destroy_aot_runtime(ctx);
}}
"#
    )
}

/// Generate a `build.rs` that tells cargo to link the AOT object file.
fn generate_aot_build_rs(obj_path: &Path) -> String {
    let obj_display = obj_path.display();
    format!(
        r#"//! Auto-generated build script for AIVI Cranelift AOT.
//! Links the compiled object file into the final binary.

fn main() {{
    println!("cargo:rerun-if-changed={obj_display}");
    println!("cargo:rustc-link-arg={obj_display}");
}}
"#
    )
}

fn cmd_project_run(args: &[String]) -> Result<(), AiviError> {
    let root = env::current_dir()?;
    let cfg = aivi::read_aivi_toml(&root.join("aivi.toml"))?;
    let proj_args = parse_project_args(args)?;
    if proj_args.release || cfg.build.cargo_profile == "release" {
        return Err(AiviError::InvalidCommand(
            "run --release is not supported by the native runtime pipeline".to_string(),
        ));
    }
    if !proj_args.cargo_args.is_empty() {
        return Err(AiviError::InvalidCommand(
            "extra cargo args are not supported by the native runtime pipeline".to_string(),
        ));
    }
    let source_target = resolve_project_source_target(&root, &cfg.project.entry);
    let target = source_target
        .to_str()
        .ok_or_else(|| AiviError::InvalidPath(source_target.display().to_string()))?;
    if proj_args.watch {
        let watch_dir = source_target
            .parent()
            .unwrap_or(&root)
            .to_path_buf();
        return watch::run_watch(target, &watch_dir);
    }
    let (program, cg_types, monomorph_plan, surface_modules) = aivi::desugar_target_with_cg_types_and_surface(target)?;
    aivi::run_cranelift_jit(program, cg_types, monomorph_plan, &surface_modules)
}

struct ProjectArgs {
    release: bool,
    watch: bool,
    cargo_args: Vec<String>,
}

fn parse_project_args(args: &[String]) -> Result<ProjectArgs, AiviError> {
    let mut before = Vec::new();
    let mut after = Vec::new();
    let mut saw_sep = false;
    for arg in args {
        if !saw_sep && arg == "--" {
            saw_sep = true;
            continue;
        }
        if saw_sep {
            after.push(arg.clone());
        } else {
            before.push(arg.clone());
        }
    }

    let mut release = false;
    let mut watch = false;
    for arg in before {
        match arg.as_str() {
            "--release" => release = true,
            "--watch" | "-w" => watch = true,
            // Ignore positional file paths (e.g. `aivi run src/main.aivi`) —
            // the project pipeline uses aivi.toml's entry instead.
            _ if !arg.starts_with('-') => {}
            _ => return Err(AiviError::InvalidCommand(format!("unknown flag {arg}"))),
        }
    }

    Ok(ProjectArgs {
        release,
        watch,
        cargo_args: after,
    })
}

#[cfg(test)]
fn cargo_feature_for_native_ui_target(
    project_root: &Path,
    target: &aivi::NativeUiTarget,
) -> Result<Option<&'static str>, AiviError> {
    match target {
        aivi::NativeUiTarget::Portable => Ok(None),
        aivi::NativeUiTarget::GnomeGtk4Libadwaita => {
            let cargo_toml = project_root.join("Cargo.toml");
            if cargo_feature_declared(&cargo_toml, "runtime-gnome")? {
                Ok(Some("runtime-gnome"))
            } else {
                Ok(Some("aivi/gtk4-libadwaita"))
            }
        }
    }
}

#[cfg(test)]
fn cargo_feature_declared(cargo_toml_path: &Path, feature: &str) -> Result<bool, AiviError> {
    let content = std::fs::read_to_string(cargo_toml_path)?;
    let manifest: toml::Value = toml::from_str(&content).map_err(|err| {
        AiviError::Config(format!(
            "failed to parse {}: {err}",
            cargo_toml_path.display()
        ))
    })?;
    Ok(manifest
        .get("features")
        .and_then(toml::Value::as_table)
        .is_some_and(|features| features.contains_key(feature)))
}

fn resolve_project_entry(project_root: &Path, entry: &str) -> PathBuf {
    let entry_path = Path::new(entry);
    if entry_path.components().count() == 1 {
        project_root.join("src").join(entry_path)
    } else {
        project_root.join(entry_path)
    }
}

/// Derives the recursive source target from the project entry.
///
/// For an entry like `src/main.aivi` the source directory is `<root>/src/...`
/// so that all `.aivi` files under `src/` are included in compilation.
fn resolve_project_source_target(project_root: &Path, entry: &str) -> PathBuf {
    let entry_path = resolve_project_entry(project_root, entry);
    let src_dir = entry_path
        .parent()
        .unwrap_or(project_root);
    src_dir.join("...")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gnome_prefers_runtime_alias_when_declared() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            tmp.path().join("Cargo.toml"),
            "[package]\nname = \"x\"\nversion = \"0.1.0\"\n\n[features]\nruntime-gnome = []\n",
        )
        .expect("write Cargo.toml");
        let feature = cargo_feature_for_native_ui_target(
            tmp.path(),
            &aivi::NativeUiTarget::GnomeGtk4Libadwaita,
        )
        .expect("feature resolution");
        assert_eq!(feature, Some("runtime-gnome"));
    }

    #[test]
    fn gnome_falls_back_to_dependency_feature() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            tmp.path().join("Cargo.toml"),
            "[package]\nname = \"x\"\nversion = \"0.1.0\"\n",
        )
        .expect("write Cargo.toml");
        let feature = cargo_feature_for_native_ui_target(
            tmp.path(),
            &aivi::NativeUiTarget::GnomeGtk4Libadwaita,
        )
        .expect("feature resolution");
        assert_eq!(feature, Some("aivi/gtk4-libadwaita"));
    }
}
