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
    let (release_flag, cargo_args) = parse_project_args(args)?;
    let release = release_flag || cfg.build.cargo_profile == "release";

    // --native-rust opts into the legacy Rust-codegen pipeline
    let use_native_rust = cargo_args.iter().any(|a| a == "--native-rust");
    let cargo_args: Vec<String> = cargo_args
        .into_iter()
        .filter(|a| a != "--native-rust")
        .collect();

    if !use_native_rust {
        // Default: Cranelift AOT pipeline
        return cmd_project_build_cranelift(&root, &cfg, release);
    }

    // Legacy: generate Rust source and compile with cargo
    generate_project_rust(&root, &cfg)?;
    let mut cmd = Command::new("cargo");
    cmd.arg("build");
    if release {
        cmd.arg("--release");
    }
    append_native_ui_target_flags(&mut cmd, &cfg.build.native_ui_target, &root)?;
    cmd.args(cargo_args);
    let status = cmd.current_dir(&root).status()?;
    if !status.success() {
        return Err(AiviError::Cargo("cargo build failed".to_string()));
    }
    Ok(())
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
    let entry_path = resolve_project_entry(root, &cfg.project.entry);
    let entry_str = entry_path
        .to_str()
        .ok_or_else(|| AiviError::InvalidPath(entry_path.display().to_string()))?;

    let (program, cg_types, monomorph_plan) = aivi::desugar_target_with_cg_types(entry_str)?;

    // 1. Compile to object file
    eprintln!("  Compiling AIVI → Cranelift AOT...");
    let object_bytes = aivi::compile_to_object(program, cg_types, monomorph_plan)?;

    // 2. Write the object file
    let gen_dir = root.join(&cfg.build.gen_dir);
    std::fs::create_dir_all(&gen_dir)?;
    let obj_path = gen_dir.join("aivi_program.o");
    std::fs::write(&obj_path, &object_bytes)?;

    // 3. Generate a thin Rust harness that builds the runtime and calls __aivi_main
    let harness_path = gen_dir.join("src");
    std::fs::create_dir_all(&harness_path)?;
    let harness_main = harness_path.join("main.rs");
    let harness_code = generate_aot_harness("aivi_program");
    std::fs::write(&harness_main, harness_code)?;

    // 4. Compile with cargo, linking the .o file
    eprintln!("  Linking...");
    let mut cmd = Command::new("cargo");
    cmd.arg("build");
    if release {
        cmd.arg("--release");
    }
    // Tell cargo to link our object file via a build script env var
    cmd.env("AIVI_AOT_OBJECT", obj_path.display().to_string());
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

fn cmd_project_run(args: &[String]) -> Result<(), AiviError> {
    let root = env::current_dir()?;
    let cfg = aivi::read_aivi_toml(&root.join("aivi.toml"))?;
    let (release_flag, cargo_args) = parse_project_args(args)?;
    if release_flag || cfg.build.cargo_profile == "release" {
        return Err(AiviError::InvalidCommand(
            "run --release is not supported by the native runtime pipeline".to_string(),
        ));
    }
    if !cargo_args.is_empty() {
        return Err(AiviError::InvalidCommand(
            "extra cargo args are not supported by the native runtime pipeline".to_string(),
        ));
    }
    let entry_path = resolve_project_entry(&root, &cfg.project.entry);
    let entry = entry_path
        .to_str()
        .ok_or_else(|| AiviError::InvalidPath(entry_path.display().to_string()))?;
    let (program, cg_types, monomorph_plan) = aivi::desugar_target_with_cg_types(entry)?;
    aivi::run_cranelift_jit(program, cg_types, monomorph_plan)
}

fn parse_project_args(args: &[String]) -> Result<(bool, Vec<String>), AiviError> {
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
    for arg in before {
        match arg.as_str() {
            "--release" => release = true,
            _ => return Err(AiviError::InvalidCommand(format!("unknown flag {arg}"))),
        }
    }

    Ok((release, after))
}

fn append_native_ui_target_flags(
    cmd: &mut Command,
    target: &aivi::NativeUiTarget,
    project_root: &Path,
) -> Result<(), AiviError> {
    if let Some(feature) = cargo_feature_for_native_ui_target(project_root, target)? {
        cmd.arg("--features");
        cmd.arg(feature);
    }
    Ok(())
}

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
                Ok(Some("aivi_native_runtime/gtk4-libadwaita"))
            }
        }
    }
}

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

fn generate_project_rust(project_root: &Path, cfg: &aivi::AiviToml) -> Result<(), AiviError> {
    let aivi_toml_path = project_root.join("aivi.toml");
    let cargo_toml_path = project_root.join("Cargo.toml");
    if !aivi_toml_path.exists() || !cargo_toml_path.exists() {
        return Err(AiviError::Config(
            "build expects a directory containing aivi.toml and Cargo.toml".to_string(),
        ));
    }

    let entry_path = resolve_project_entry(project_root, &cfg.project.entry);
    let entry_str = entry_path
        .to_str()
        .ok_or_else(|| AiviError::InvalidPath(entry_path.display().to_string()))?;

    let _modules = load_checked_modules(entry_str)?;
    let (program, cg_types, _monomorph_plan) = aivi::desugar_target_with_cg_types(entry_str)?;

    let gen_dir = project_root.join(&cfg.build.gen_dir);
    let src_out = gen_dir.join("src");
    std::fs::create_dir_all(&src_out)?;

    let (out_path, rust) = match cfg.project.kind {
        ProjectKind::Bin => (
            src_out.join("main.rs"),
            compile_rust_native_typed(program, cg_types)?,
        ),
        ProjectKind::Lib => (
            src_out.join("lib.rs"),
            compile_rust_native_lib_typed(program, cg_types)?,
        ),
    };
    std::fs::write(&out_path, rust)?;
    write_build_stamp(project_root, cfg, &gen_dir, &entry_path)?;
    Ok(())
}

fn resolve_project_entry(project_root: &Path, entry: &str) -> PathBuf {
    let entry_path = Path::new(entry);
    if entry_path.components().count() == 1 {
        project_root.join("src").join(entry_path)
    } else {
        project_root.join(entry_path)
    }
}

fn write_build_stamp(
    project_root: &Path,
    cfg: &aivi::AiviToml,
    gen_dir: &Path,
    entry_path: &Path,
) -> Result<(), AiviError> {
    let src_dir = project_root.join("src");
    let sources = aivi::collect_aivi_sources(&src_dir)?;
    let mut inputs = Vec::new();
    for path in sources {
        let bytes = std::fs::read(&path)?;
        let hash = Sha256::digest(&bytes);
        inputs.push(serde_json::json!({
            "path": normalize_path(path.strip_prefix(project_root).unwrap_or(&path)),
            "sha256": hex_lower(&hash),
        }));
    }

    let stamp = serde_json::json!({
        "tool": { "aivi": env!("CARGO_PKG_VERSION") },
        "language_version": cfg.project.language_version.clone().unwrap_or_else(|| "unknown".to_string()),
        "kind": match cfg.project.kind { ProjectKind::Bin => "bin", ProjectKind::Lib => "lib" },
        "entry": normalize_path(entry_path.strip_prefix(project_root).unwrap_or(entry_path)),
        "rust_edition": cfg.build.rust_edition.clone(),
        "inputs": inputs,
    });

    std::fs::create_dir_all(gen_dir)?;
    std::fs::write(
        gen_dir.join("aivi.json"),
        serde_json::to_vec_pretty(&stamp).unwrap(),
    )?;
    Ok(())
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{:02x}", b));
    }
    out
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
        assert_eq!(feature, Some("aivi_native_runtime/gtk4-libadwaita"));
    }
}
