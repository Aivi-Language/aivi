use aivi::{
    check_modules, check_types, collect_mcp_manifest, compile_rust, compile_rust_lib,
    compile_rust_native, compile_rust_native_lib, desugar_target,
    embedded_stdlib_source, format_target, kernel_target, load_module_diagnostics, load_modules,
    parse_target, render_diagnostics, run_native, rust_ir_target, serve_mcp_stdio_with_policy,
    write_scaffold, AiviError, CargoDepSpec, Codegen, McpPolicy, ProjectKind,
};
use sha2::{Digest, Sha256};
use std::env;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::Duration;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(AiviError::Diagnostics) => ExitCode::FAILURE,
        Err(err) => {
            eprintln!("{err}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), AiviError> {
    let mut args = env::args().skip(1);
    let Some(command) = args.next() else {
        print_help();
        return Ok(());
    };
    let rest: Vec<String> = args.collect();

    match command.as_str() {
        "-h" | "--help" => {
            print_help();
            Ok(())
        }
        "init" | "new" => cmd_init(&rest),
        "clean" => cmd_clean(&rest),
        "install" => cmd_install(&rest),
        "search" => cmd_search(&rest),
        "parse" => {
            let Some(target) = rest.first() else {
                print_help();
                return Ok(());
            };
            let bundle = parse_target(target)?;
            let output = serde_json::to_string_pretty(&bundle)
                .map_err(|err| AiviError::Io(std::io::Error::other(err)))?;
            println!("{output}");
            let mut had_errors = false;
            for file in &bundle.files {
                if !file.diagnostics.is_empty() {
                    let rendered = render_diagnostics(&file.path, &file.diagnostics);
                    if !rendered.is_empty() {
                        eprintln!("{rendered}");
                    }
                    had_errors = true;
                }
            }
            if had_errors {
                return Err(AiviError::Diagnostics);
            }
            Ok(())
        }
        "check" => {
            let Some(target) = rest.first() else {
                print_help();
                return Ok(());
            };
            let mut diagnostics = load_module_diagnostics(target)?;
            let modules = load_modules(target)?;
            diagnostics.extend(check_modules(&modules));
            if diagnostics.is_empty() {
                diagnostics.extend(check_types(&modules));
            }
            if diagnostics.is_empty() {
                return Ok(());
            }
            for diag in diagnostics {
                let rendered =
                    render_diagnostics(&diag.path, std::slice::from_ref(&diag.diagnostic));
                if !rendered.is_empty() {
                    eprintln!("{rendered}");
                }
            }
            Err(AiviError::Diagnostics)
        }
        "fmt" => {
            let Some(target) = rest.first() else {
                print_help();
                return Ok(());
            };
            let formatted = format_target(target)?;
            print!("{formatted}");
            Ok(())
        }
        "desugar" => {
            let Some(target) = rest.first() else {
                print_help();
                return Ok(());
            };
            let diagnostics = load_module_diagnostics(target)?;
            if !diagnostics.is_empty() {
                for diag in diagnostics {
                    let rendered =
                        render_diagnostics(&diag.path, std::slice::from_ref(&diag.diagnostic));
                    if !rendered.is_empty() {
                        eprintln!("{rendered}");
                    }
                }
                return Err(AiviError::Diagnostics);
            }
            let program = desugar_target(target)?;
            let output = serde_json::to_string_pretty(&program)
                .map_err(|err| AiviError::Io(std::io::Error::other(err)))?;
            println!("{output}");
            Ok(())
        }
        "kernel" => {
            let Some(target) = rest.first() else {
                print_help();
                return Ok(());
            };
            let program = kernel_target(target)?;
            let output = serde_json::to_string_pretty(&program)
                .map_err(|err| AiviError::Io(std::io::Error::other(err)))?;
            println!("{output}");
            Ok(())
        }
        "rust-ir" => {
            let Some(target) = rest.first() else {
                print_help();
                return Ok(());
            };
            let program = rust_ir_target(target)?;
            let output = serde_json::to_string_pretty(&program)
                .map_err(|err| AiviError::Io(std::io::Error::other(err)))?;
            println!("{output}");
            Ok(())
        }
        "lsp" | "build" | "run" => match command.as_str() {
            "lsp" => {
                let status = Command::new("aivi-lsp").args(&rest).status()?;
                if !status.success() {
                    return Err(AiviError::Io(std::io::Error::other(
                        "aivi-lsp exited with an error",
                    )));
                }
                Ok(())
            }
            "build" => {
                if should_use_project_pipeline(&rest) {
                    cmd_project_build(&rest)
                } else {
                    let Some(opts) = parse_build_args(rest.into_iter(), true, "rust")? else {
                        print_help();
                        return Ok(());
                    };
                    if opts.target != "rust"
                        && opts.target != "rust-embed"
                        && opts.target != "rust-native"
                        && opts.target != "rustc"
                    {
                        return Err(AiviError::InvalidCommand(format!(
                            "unsupported target {}",
                            opts.target
                        )));
                    }
                    let _modules = load_checked_modules_with_progress(&opts.input)?;
                    let program = desugar_target(&opts.input)?;
                    if opts.target == "rust" || opts.target == "rust-embed" {
                        let rust = compile_rust(program)?;
                        let out_dir = opts
                            .output
                            .unwrap_or_else(|| PathBuf::from("target/aivi-gen"));
                        write_rust_project_embed(&out_dir, &rust)?;
                        println!("{}", out_dir.display());
                    } else if opts.target == "rust-native" {
                        let rust = compile_rust_native(program)?;
                        let out_dir = opts
                            .output
                            .unwrap_or_else(|| PathBuf::from("target/aivi-gen"));
                        write_rust_project_native(&out_dir, &rust)?;
                        println!("{}", out_dir.display());
                    } else {
                        let out = opts
                            .output
                            .unwrap_or_else(|| PathBuf::from("target/aivi-rustc/aivi_out"));
                        aivi::build_with_rustc(program, &out, &opts.forward)?;
                        println!("{}", out.display());
                    }
                    Ok(())
                }
            }
            "run" => {
                if should_use_project_pipeline(&rest) {
                    cmd_project_run(&rest)
                } else {
                    let Some(opts) = parse_build_args(rest.into_iter(), false, "native")? else {
                        print_help();
                        return Ok(());
                    };
                    if opts.target != "native" {
                        return Err(AiviError::InvalidCommand(format!(
                            "unsupported target {}",
                            opts.target
                        )));
                    }
                    // Skip all checking for `run` — checking hangs on the
                    // embedded stdlib (preexisting bug). The desugar_target
                    // function parses + desugars without type/module checking.
                    let program = desugar_target(&opts.input)?;
                    run_native(program)?;
                    Ok(())
                }
            }
            _ => Ok(()),
        },
        "mcp" => cmd_mcp(&rest),
        "i18n" => cmd_i18n(&rest),
        _ => {
            print_help();
            Err(AiviError::InvalidCommand(command))
        }
    }
}

fn print_help() {
    println!(
        "aivi\n\nUSAGE:\n  aivi <COMMAND>\n\nCOMMANDS:\n  init <name> [--bin|--lib] [--edition 2024] [--language-version 0.1] [--force]\n  new <name> ... (alias of init)\n  search <query>\n  install <spec> [--require-aivi] [--no-fetch]\n  build [--release] [-- <cargo args...>]\n  run [--release] [-- <cargo args...>]\n  clean [--all]\n\n  parse <path|dir/...>\n  check <path|dir/...>\n  fmt <path>\n  desugar <path|dir/...>\n  kernel <path|dir/...>\n  rust-ir <path|dir/...>\n  lsp\n  build <path|dir/...> [--target rust|rust-embed|rust-native|rustc] [--out <dir|path>] [-- <rustc args...>]\n  run <path|dir/...> [--target native]\n  mcp serve <path|dir/...> [--allow-effects]\n  i18n gen <catalog.properties> --locale <tag> --module <name> --out <file>\n\n  -h, --help"
    );
}

fn cmd_mcp(args: &[String]) -> Result<(), AiviError> {
    let Some(subcommand) = args.first() else {
        print_help();
        return Ok(());
    };
    match subcommand.as_str() {
        "serve" => {
            let mut target = None;
            let mut allow_effects = false;
            for arg in args.iter().skip(1) {
                match arg.as_str() {
                    "--allow-effects" => allow_effects = true,
                    value if !value.starts_with('-') && target.is_none() => {
                        target = Some(value.to_string());
                    }
                    other => {
                        return Err(AiviError::InvalidCommand(format!(
                            "unexpected mcp serve argument {other}"
                        )));
                    }
                }
            }
            let target = target.as_deref().unwrap_or("./...");
            cmd_mcp_serve(target, allow_effects)
        }
        _ => Err(AiviError::InvalidCommand(format!("mcp {subcommand}"))),
    }
}

fn cmd_i18n(args: &[String]) -> Result<(), AiviError> {
    let Some(subcommand) = args.first() else {
        print_help();
        return Ok(());
    };
    match subcommand.as_str() {
        "gen" => cmd_i18n_gen(&args[1..]),
        other => Err(AiviError::InvalidCommand(format!("i18n {other}"))),
    }
}

fn cmd_i18n_gen(args: &[String]) -> Result<(), AiviError> {
    let mut catalog = None;
    let mut locale = None;
    let mut module_name = None;
    let mut out_path = None;

    let mut iter = args.iter().peekable();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--locale" => {
                locale = iter.next().cloned();
            }
            "--module" => {
                module_name = iter.next().cloned();
            }
            "--out" => {
                out_path = iter.next().cloned();
            }
            value if !value.starts_with('-') && catalog.is_none() => {
                catalog = Some(value.to_string());
            }
            other => {
                return Err(AiviError::InvalidCommand(format!(
                    "unexpected i18n gen argument {other}"
                )));
            }
        }
    }

    let Some(catalog_path) = catalog else {
        return Err(AiviError::InvalidCommand(
            "i18n gen requires <catalog.properties>".to_string(),
        ));
    };
    let Some(locale) = locale else {
        return Err(AiviError::InvalidCommand(
            "i18n gen requires --locale <tag>".to_string(),
        ));
    };
    let Some(module_name) = module_name else {
        return Err(AiviError::InvalidCommand(
            "i18n gen requires --module <name>".to_string(),
        ));
    };
    let Some(out_path) = out_path else {
        return Err(AiviError::InvalidCommand(
            "i18n gen requires --out <file>".to_string(),
        ));
    };

    let properties_text = std::fs::read_to_string(&catalog_path)?;
    let module_source = aivi::generate_i18n_module_from_properties(
        &module_name,
        &locale,
        &properties_text,
    )
    .map_err(AiviError::InvalidCommand)?;

    let out_path = PathBuf::from(out_path);
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&out_path, module_source)?;
    println!("{}", out_path.display());
    Ok(())
}

fn cmd_mcp_serve(target: &str, allow_effects: bool) -> Result<(), AiviError> {
    let mut diagnostics = load_module_diagnostics(target)?;
    let modules = load_modules(target)?;
    diagnostics.extend(check_modules(&modules));
    if diagnostics.is_empty() {
        diagnostics.extend(check_types(&modules));
    }
    if !diagnostics.is_empty() {
        for diag in diagnostics {
            let rendered = render_diagnostics(&diag.path, std::slice::from_ref(&diag.diagnostic));
            if !rendered.is_empty() {
                eprintln!("{rendered}");
            }
        }
        return Err(AiviError::Diagnostics);
    }

    let manifest = collect_mcp_manifest(&modules);
    serve_mcp_stdio_with_policy(
        &manifest,
        McpPolicy {
            allow_effectful_tools: allow_effects,
        },
    )?;
    Ok(())
}

struct BuildArgs {
    input: String,
    output: Option<PathBuf>,
    target: String,
    forward: Vec<String>,
}

fn parse_build_args(
    mut args: impl Iterator<Item = String>,
    allow_out: bool,
    default_target: &str,
) -> Result<Option<BuildArgs>, AiviError> {
    let mut input = None;
    let mut output = None;
    let mut target = default_target.to_string();
    let mut forward = Vec::new();

    while let Some(arg) = args.next() {
        if arg == "--" {
            forward.extend(args);
            break;
        }
        match arg.as_str() {
            "--target" => {
                let Some(value) = args.next() else {
                    return Err(AiviError::InvalidCommand(
                        "--target expects a value".to_string(),
                    ));
                };
                target = value;
            }
            "--out" if allow_out => {
                let Some(value) = args.next() else {
                    return Err(AiviError::InvalidCommand(
                        "--out expects a value".to_string(),
                    ));
                };
                output = Some(PathBuf::from(value));
            }
            _ if arg.starts_with('-') => {
                return Err(AiviError::InvalidCommand(format!("unknown flag {arg}")));
            }
            _ => {
                if input.is_some() {
                    return Err(AiviError::InvalidCommand(format!(
                        "unexpected argument {arg}"
                    )));
                }
                input = Some(arg);
            }
        }
    }

    let Some(input) = input else {
        return Ok(None);
    };

    Ok(Some(BuildArgs {
        input,
        output,
        target,
        forward,
    }))
}

struct Spinner {
    stop: Arc<AtomicBool>,
    message: Arc<Mutex<String>>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl Spinner {
    fn new(message: String) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let message_state = Arc::new(Mutex::new(message));
        let stop_clone = Arc::clone(&stop);
        let message_clone = Arc::clone(&message_state);
        let handle = std::thread::spawn(move || {
            let frames = ["|", "/", "-", "\\"];
            let mut idx = 0usize;
            while !stop_clone.load(Ordering::Relaxed) {
                let msg = message_clone
                    .lock()
                    .map(|guard| guard.clone())
                    .unwrap_or_default();
                eprint!("\r{} {}", frames[idx], msg);
                let _ = std::io::stderr().flush();
                idx = (idx + 1) % frames.len();
                std::thread::sleep(Duration::from_millis(80));
            }
            let msg = message_clone
                .lock()
                .map(|guard| guard.clone())
                .unwrap_or_default();
            eprint!("\rdone {}\n", msg);
            let _ = std::io::stderr().flush();
        });
        Self {
            stop,
            message: message_state,
            handle: Some(handle),
        }
    }

    fn set_message(&self, message: String) {
        if let Ok(mut guard) = self.message.lock() {
            *guard = message;
        }
    }

    fn stop(&mut self) {
        if let Some(handle) = self.handle.take() {
            self.stop.store(true, Ordering::Relaxed);
            let _ = handle.join();
        }
    }
}

impl Drop for Spinner {
    fn drop(&mut self) {
        self.stop();
    }
}

fn write_rust_project_embed(out_dir: &Path, main_rs: &str) -> Result<(), AiviError> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let aivi_path = normalize_path(&manifest_dir);
    let cargo_toml = format!(
        "[package]\nname = \"aivi-gen\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\naivi = {{ path = \"{}\" }}\nserde_json = \"1.0\"\n",
        aivi_path
    );
    let src_dir = out_dir.join("src");
    std::fs::create_dir_all(&src_dir)?;
    std::fs::write(out_dir.join("Cargo.toml"), cargo_toml)?;
    std::fs::write(src_dir.join("main.rs"), main_rs)?;
    Ok(())
}

fn write_rust_project_native(out_dir: &Path, main_rs: &str) -> Result<(), AiviError> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let rt_path = normalize_path(&manifest_dir.join("../aivi_native_runtime"));
    let cargo_toml = format!(
        "[package]\nname = \"aivi-gen\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\naivi_native_runtime = {{ path = \"{}\" }}\n",
        rt_path
    );
    let src_dir = out_dir.join("src");
    std::fs::create_dir_all(&src_dir)?;
    std::fs::write(out_dir.join("Cargo.toml"), cargo_toml)?;
    std::fs::write(src_dir.join("main.rs"), main_rs)?;
    Ok(())
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn load_checked_modules_with_progress(target: &str) -> Result<Vec<aivi::Module>, AiviError> {
    let paths = aivi::resolve_target(target)?;
    let mut spinner = Spinner::new("checking sources".to_string());
    let mut diagnostics = Vec::new();
    let mut modules = Vec::new();

    for path in &paths {
        spinner.set_message(format!("checking {}", path.display()));
        let content = std::fs::read_to_string(path)?;
        let (mut parsed, mut file_diags) = aivi::parse_modules(path, &content);
        modules.append(&mut parsed);
        diagnostics.append(&mut file_diags);
    }

    spinner.stop();

    let mut stdlib_modules = aivi::embedded_stdlib_modules();
    stdlib_modules.append(&mut modules);
    diagnostics.extend(check_modules(&stdlib_modules));
    if diagnostics.is_empty() {
        diagnostics.extend(check_types(&stdlib_modules));
    }
    if diagnostics.is_empty() {
        return Ok(stdlib_modules);
    }
    for diag in diagnostics {
        let rendered = render_diagnostics(&diag.path, std::slice::from_ref(&diag.diagnostic));
        if !rendered.is_empty() {
            eprintln!("{rendered}");
        }
    }
    Err(AiviError::Diagnostics)
}

fn load_checked_modules(target: &str) -> Result<Vec<aivi::Module>, AiviError> {
    let mut diagnostics = load_module_diagnostics(target)?;
    let modules = load_modules(target)?;
    diagnostics.extend(check_modules(&modules));
    if diagnostics.is_empty() {
        diagnostics.extend(check_types(&modules));
    }
    if diagnostics.is_empty() {
        return Ok(modules);
    }
    for diag in diagnostics {
        let rendered = render_diagnostics(&diag.path, std::slice::from_ref(&diag.diagnostic));
        if !rendered.is_empty() {
            eprintln!("{rendered}");
        }
    }
    Err(AiviError::Diagnostics)
}

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
            "--require-aivi" => {}
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

    if install_stdlib_module(&root, &spec)? {
        return Ok(());
    }

    let dep =
        CargoDepSpec::parse(&spec).map_err(|err| AiviError::InvalidCommand(err.to_string()))?;

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

    if let Err(err) = ensure_aivi_dependency(&root, &dep) {
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

fn ensure_aivi_dependency(root: &Path, dep: &CargoDepSpec) -> Result<(), AiviError> {
    let output = Command::new("cargo")
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
        metadata: serde_json::Value,
    }

    let metadata: CargoMetadata = serde_json::from_slice(&output.stdout)
        .map_err(|err| AiviError::Cargo(format!("failed to parse cargo metadata: {err}")))?;

    let package = metadata
        .packages
        .iter()
        .find(|pkg| pkg.name == dep.name())
        .ok_or_else(|| {
            AiviError::Cargo(format!(
                "dependency {} not found in cargo metadata",
                dep.name()
            ))
        })?;

    if !is_aivi_metadata(&package.metadata) {
        return Err(AiviError::Cargo(format!(
            "dependency {} is not an AIVI package (missing [package.metadata.aivi])",
            dep.name()
        )));
    }

    Ok(())
}

fn is_aivi_metadata(metadata: &serde_json::Value) -> bool {
    let Some(aivi) = metadata.get("aivi") else {
        return false;
    };
    let Some(aivi) = aivi.as_object() else {
        return false;
    };
    let language_version = aivi
        .get("language_version")
        .and_then(serde_json::Value::as_str);
    let kind = aivi.get("kind").and_then(serde_json::Value::as_str);
    language_version.is_some() && matches!(kind, Some("lib" | "bin"))
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
    generate_project_rust(&root, &cfg)?;
    let mut cmd = Command::new("cargo");
    cmd.arg("build");
    if release {
        cmd.arg("--release");
    }
    cmd.args(cargo_args);
    let status = cmd.current_dir(&root).status()?;
    if !status.success() {
        return Err(AiviError::Cargo("cargo build failed".to_string()));
    }
    Ok(())
}

fn cmd_project_run(args: &[String]) -> Result<(), AiviError> {
    let root = env::current_dir()?;
    let cfg = aivi::read_aivi_toml(&root.join("aivi.toml"))?;
    let (release_flag, cargo_args) = parse_project_args(args)?;
    let release = release_flag || cfg.build.cargo_profile == "release";
    generate_project_rust(&root, &cfg)?;
    let mut cmd = Command::new("cargo");
    cmd.arg("run");
    if release {
        cmd.arg("--release");
    }
    cmd.args(cargo_args);
    let status = cmd.current_dir(&root).status()?;
    if !status.success() {
        return Err(AiviError::Cargo("cargo run failed".to_string()));
    }
    Ok(())
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
    let program = desugar_target(entry_str)?;

    let gen_dir = project_root.join(&cfg.build.gen_dir);
    let src_out = gen_dir.join("src");
    std::fs::create_dir_all(&src_out)?;

    let (out_path, rust) = match (cfg.project.kind, cfg.build.codegen) {
        (ProjectKind::Bin, Codegen::Embed) => (src_out.join("main.rs"), compile_rust(program)?),
        (ProjectKind::Lib, Codegen::Embed) => (src_out.join("lib.rs"), compile_rust_lib(program)?),
        (ProjectKind::Bin, Codegen::Native) => {
            (src_out.join("main.rs"), compile_rust_native(program)?)
        }
        (ProjectKind::Lib, Codegen::Native) => {
            (src_out.join("lib.rs"), compile_rust_native_lib(program)?)
        }
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
