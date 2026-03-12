use aivi::{
    embedded_stdlib_source, ensure_aivi_dependency, format_target, kernel_target,
    parse_target, render_diagnostics,
    rust_ir_target, serve_mcp_stdio_with_policy, validate_publish_preflight, write_scaffold,
    AiviError, CargoDepSpec, McpPolicy, Pipeline, ProjectKind,
};
use std::env;
use std::io;
use std::io::{IsTerminal, Write};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;

const AIVI_LANGUAGE_VERSION: &str = "0.1";

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
    let use_color = io::stderr().is_terminal();
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
        "-V" | "--version" | "version" => {
            print_version();
            Ok(())
        }
        "init" | "new" => cmd_init(&rest),
        "clean" => cmd_clean(&rest),
        "install" => cmd_install(&rest),
        "search" => cmd_search(&rest),
        "package" => cmd_package(&rest),
        "publish" => cmd_publish(&rest),
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
                    let rendered =
                        render_diagnostics(&file.path, &file.diagnostics, use_color);
                    if !rendered.is_empty() {
                        eprintln!("{rendered}");
                    }
                    had_errors = had_errors
                        || file
                            .diagnostics
                            .iter()
                            .any(|d| d.severity == aivi::DiagnosticSeverity::Error);
                }
            }
            if had_errors {
                return Err(AiviError::Diagnostics);
            }
            Ok(())
        }
        "check" => {
            let (debug_trace, rest) = consume_debug_trace_flag(&rest);
            let (check_stdlib, rest) = consume_check_stdlib_flag(&rest);
            maybe_enable_debug_trace(debug_trace);
            let Some(target_ctx) = resolve_check_or_test_target(&rest) else {
                print_help();
                return Ok(());
            };
            let mut session = aivi::WorkspaceSession::new();
            let assembly =
                session.assemble_target(&target_ctx.target, aivi::FrontendAssemblyMode::Check)?;
            let mut diagnostics = assembly.all_diagnostics();
            if !check_stdlib {
                diagnostics.retain(|diag| !diag.path.starts_with("<embedded:"));
            }
            let has_errors = aivi::file_diagnostics_have_errors(&diagnostics);
            for diag in &diagnostics {
                let rendered = render_diagnostics(
                    &diag.path,
                    std::slice::from_ref(&diag.diagnostic),
                    use_color,
                );
                if !rendered.is_empty() {
                    eprintln!("{rendered}");
                }
            }
            if has_errors {
                Err(AiviError::Diagnostics)
            } else {
                Ok(())
            }
        }
        "fmt" => {
            let (write, rest) = consume_flag("--write", &rest);
            let Some(target) = rest.first() else {
                print_help();
                return Ok(());
            };
            if write {
                use rayon::prelude::*;
                let paths: Vec<_> = aivi::resolve_target(target)?
                    .into_iter()
                    .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("aivi"))
                    .collect();
                paths.par_iter().try_for_each(|path| -> Result<(), AiviError> {
                    let content = std::fs::read_to_string(path)?;
                    let formatted = aivi::format_text(&content);
                    if formatted != content {
                        std::fs::write(path, formatted)?;
                    }
                    Ok(())
                })?;
            } else {
                let formatted = format_target(target)?;
                print!("{formatted}");
            }
            Ok(())
        }
        "test" => {
            let (check_stdlib, rest) = consume_check_stdlib_flag(&rest);
            let (only_tests, rest) = consume_multi_value_flag("--only", &rest)?;
            let (update_snapshots, rest) = consume_flag("--update-snapshots", &rest);
            let Some(target_ctx) = resolve_check_or_test_target(&rest) else {
                print_help();
                return Ok(());
            };
            let target = target_ctx.target.as_str();

            // Format all target files in-place before running tests so the suite is stable and
            // editor tooling doesn't surface spurious formatter diffs.
            let paths = aivi::resolve_target(target)?;
            let test_paths = collect_candidate_test_paths(&paths)?;
            if test_paths.is_empty() {
                return Err(AiviError::InvalidCommand(format!(
                    "no @test definitions found under {target}"
                )));
            }
            format_test_paths_in_place(&test_paths)?;

            let mut pipeline = Pipeline::from_target(target)?;
            let parse_diagnostics = pipeline.parse_diagnostics().to_vec();
            for diag in &parse_diagnostics {
                let rendered =
                    render_diagnostics(&diag.path, std::slice::from_ref(&diag.diagnostic), use_color);
                if !rendered.is_empty() {
                    eprintln!("{rendered}");
                }
            }
            if aivi::file_diagnostics_have_errors(&parse_diagnostics) {
                return Err(AiviError::Diagnostics);
            }

            let (mut test_entries, test_name_to_path) = collect_test_entries_with_paths(pipeline.modules());
            if test_entries.is_empty() {
                return Err(AiviError::InvalidCommand(format!(
                    "no @test definitions found under {target}"
                )));
            }

            if !only_tests.is_empty() {
                let mut filtered = Vec::new();
                let mut missing = Vec::new();
                for wanted in &only_tests {
                    if test_entries.iter().any(|(n, _)| n == wanted) {
                        if let Some(entry) = test_entries.iter().find(|(n, _)| n == wanted) {
                            filtered.push(entry.clone());
                        }
                        continue;
                    }
                    // Convenience: allow passing an unqualified def name (suffix match).
                    let suffix = format!(".{wanted}");
                    if let Some(entry) = test_entries.iter().find(|(n, _)| n.ends_with(&suffix)) {
                        filtered.push(entry.clone());
                    } else {
                        missing.push(wanted.clone());
                    }
                }
                if !missing.is_empty() {
                    return Err(AiviError::InvalidCommand(format!(
                        "unknown test(s): {}",
                        missing.join(", ")
                    )));
                }
                filtered.sort();
                filtered.dedup();
                test_entries = filtered;
            }

            let mut check_diags = pipeline.typecheck();
            if !check_stdlib {
                check_diags.retain(|diag| !diag.path.starts_with("<embedded:"));
            }
            for diag in &check_diags {
                let rendered =
                    render_diagnostics(&diag.path, std::slice::from_ref(&diag.diagnostic), use_color);
                if !rendered.is_empty() {
                    eprintln!("{rendered}");
                }
            }
            if aivi::file_diagnostics_have_errors(&check_diags) {
                return Err(AiviError::Diagnostics);
            }

            let program = pipeline.desugar();
            let modules = pipeline.into_modules();
            let report = aivi::run_test_suite(
                program,
                &test_entries,
                &modules,
                update_snapshots,
                target_ctx.project_root,
            )?;

            // Write out deterministic report files for CI and tooling:
            // - passed files: all tests in file passed
            // - failed files: at least one test in file failed
            let mut failed_names = HashSet::<String>::new();
            for failure in &report.failures {
                failed_names.insert(failure.name.clone());
            }
            let mut file_to_tests = BTreeMap::<PathBuf, Vec<String>>::new();
            for (name, _) in &test_entries {
                if let Some(path) = test_name_to_path.get(name) {
                    file_to_tests.entry(path.clone()).or_default().push(name.clone());
                }
            }
            let mut passed_files = BTreeSet::<PathBuf>::new();
            let mut failed_files = BTreeSet::<PathBuf>::new();
            for (path, names) in file_to_tests {
                if names.iter().any(|n| failed_names.contains(n)) {
                    failed_files.insert(path);
                } else {
                    passed_files.insert(path);
                }
            }
            std::fs::create_dir_all("target")?;
            let passed_text = passed_files
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join("\n");
            let failed_text = failed_files
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join("\n");
            std::fs::write("target/aivi-test-passed-files.txt", passed_text)?;
            std::fs::write("target/aivi-test-failed-files.txt", failed_text)?;

            // Pretty per-file summary (keeps CI/tooling stable while improving UX).
            // ASCII tag + ANSI color so terminals show green/red.
            if !passed_files.is_empty() || !failed_files.is_empty() {
                println!("\nfiles:");
                for p in &passed_files {
                    println!("\x1b[32m[OK]\x1b[0m {}", p.display());
                }
                for p in &failed_files {
                    println!("\x1b[31m[FAIL]\x1b[0m {}", p.display());
                }
            }

            if report.failed == 0 {
                for success in &report.successes {
                    println!("\x1b[32m\u{2714}\x1b[0m {}", success.description);
                }
                println!("\x1b[32m\u{2714}\x1b[0m ok: {} passed", report.passed);
                Ok(())
            } else {
                for success in &report.successes {
                    println!("\x1b[32m\u{2714}\x1b[0m {}", success.description);
                }
                for failure in &report.failures {
                    eprintln!(
                        "\x1b[31m\u{2718}\x1b[0m {}: {}",
                        failure.description, failure.message
                    );
                }
                eprintln!(
                    "\x1b[31m\u{2718}\x1b[0m FAILED: {} failed, {} passed",
                    report.failed, report.passed
                );
                Err(AiviError::Diagnostics)
            }
        }
        "desugar" => {
            let (debug_trace, rest) = consume_debug_trace_flag(&rest);
            maybe_enable_debug_trace(debug_trace);
            let Some(target) = rest.first() else {
                print_help();
                return Ok(());
            };
            let pipeline = Pipeline::from_target(target)?;
            if pipeline.has_parse_errors() {
                for diag in pipeline.parse_diagnostics() {
                    let rendered =
                        render_diagnostics(&diag.path, std::slice::from_ref(&diag.diagnostic), use_color);
                    if !rendered.is_empty() {
                        eprintln!("{rendered}");
                    }
                }
                return Err(AiviError::Diagnostics);
            }
            let program = pipeline.desugar();
            let output = serde_json::to_string_pretty(&program)
                .map_err(|err| AiviError::Io(std::io::Error::other(err)))?;
            println!("{output}");
            Ok(())
        }
        "kernel" => {
            let (debug_trace, rest) = consume_debug_trace_flag(&rest);
            maybe_enable_debug_trace(debug_trace);
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
            let (debug_trace, rest) = consume_debug_trace_flag(&rest);
            maybe_enable_debug_trace(debug_trace);
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
                let status = spawn_aivi_lsp(&rest)?;
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
                    maybe_enable_debug_trace(opts.debug_trace);
                    let mut spinner = Spinner::new("assembling frontend".to_string());
                    let result = daemon::compile_target_in_fresh_session(&opts.input, &[], use_color);
                    spinner.stop();
                    let (artifacts, summary) = match result {
                        Ok(result) => result,
                        Err(daemon::PrepareCompileFailure::Diagnostics { rendered, summary }) => {
                            daemon::print_compile_summary(&summary);
                            if !rendered.is_empty() {
                                eprint!("{rendered}");
                            }
                            return Err(AiviError::Diagnostics);
                        }
                        Err(daemon::PrepareCompileFailure::Error(err)) => return Err(err),
                    };
                    daemon::print_compile_summary(&summary);
                    let object_bytes = aivi::compile_to_object(
                        artifacts.program,
                        artifacts.cg_types,
                        artifacts.monomorph_plan,
                        &[],
                    )?;
                    let out_dir = opts
                        .output
                        .unwrap_or_else(|| PathBuf::from("target/aivi-gen"));
                    std::fs::create_dir_all(&out_dir)?;
                    let obj_path = out_dir.join("aivi_program.o");
                    std::fs::write(&obj_path, &object_bytes)?;
                    println!("{}", obj_path.display());
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
                    maybe_enable_debug_trace(opts.debug_trace);
                    if opts.target != "native" {
                        return Err(AiviError::InvalidCommand(format!(
                            "unsupported target {}",
                            opts.target
                        )));
                    }
                    if opts.watch {
                        let input_path = Path::new(&opts.input);
                        let watch_dir = input_path
                            .parent()
                            .unwrap_or(Path::new("."))
                            .to_path_buf();
                        return watch::run_watch(&opts.input, &[], &watch_dir);
                    }
                    let mut spinner = Spinner::new("assembling frontend".to_string());
                    let result = daemon::compile_target_in_fresh_session(&opts.input, &[], use_color);
                    spinner.stop();
                    let (artifacts, summary) = match result {
                        Ok(result) => result,
                        Err(daemon::PrepareCompileFailure::Diagnostics { rendered, summary }) => {
                            daemon::print_compile_summary(&summary);
                            if !rendered.is_empty() {
                                eprint!("{rendered}");
                            }
                            return Err(AiviError::Diagnostics);
                        }
                        Err(daemon::PrepareCompileFailure::Error(err)) => return Err(err),
                    };
                    daemon::print_compile_summary(&summary);
                    aivi::run_cranelift_jit_prepared(
                        artifacts.program,
                        artifacts.cg_types,
                        artifacts.monomorph_plan,
                        artifacts.source_schemas,
                        artifacts.constructor_ordinals,
                        artifacts
                            .crate_natives
                            .iter()
                            .map(|binding| binding.aivi_name.clone())
                            .collect(),
                    )
                }
            }
            _ => Ok(()),
        },
        "repl" => repl::cmd_repl(&rest),
        "daemon" => daemon::cmd_daemon(&rest),
        "mcp" => cmd_mcp(&rest),
        "i18n" => cmd_i18n(&rest),
        _ => {
            print_help();
            Err(AiviError::InvalidCommand(command))
        }
    }
}

fn spawn_aivi_lsp(args: &[String]) -> Result<std::process::ExitStatus, AiviError> {
    let mut tried = Vec::<String>::new();
    let mut candidates = Vec::<PathBuf>::new();

    // First try a sibling binary next to the current `aivi` executable (works for
    // workspace builds and `cargo install` when both binaries are installed).
    if let Ok(exe) = env::current_exe() {
        if let Some(dir) = exe.parent() {
            let name = if cfg!(windows) {
                "aivi-lsp.exe"
            } else {
                "aivi-lsp"
            };
            candidates.push(dir.join(name));
        }
    }

    // Convenience for working in a repo with a globally-installed `aivi`.
    if let Ok(cwd) = env::current_dir() {
        let name = if cfg!(windows) {
            "aivi-lsp.exe"
        } else {
            "aivi-lsp"
        };
        candidates.push(cwd.join("target").join("debug").join(name));
        candidates.push(cwd.join("target").join("release").join(name));
    }

    for candidate in candidates {
        if !candidate.is_file() {
            continue;
        }
        tried.push(candidate.display().to_string());
        match Command::new(&candidate).args(args).status() {
            Ok(status) => return Ok(status),
            Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
            Err(err) => return Err(AiviError::Io(err)),
        }
    }

    tried.push("aivi-lsp (on PATH)".to_string());
    match Command::new("aivi-lsp").args(args).status() {
        Ok(status) => Ok(status),
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            let msg = format!(
                "could not find `aivi-lsp`.\n\
Tried: {}\n\
\n\
Fix:\n\
- If you're in the repo: `cargo build -p aivi-lsp` (then rerun `aivi lsp`)\n\
- Or install it: `cargo install --path crates/aivi_lsp`",
                tried.join(", ")
            );
            Err(AiviError::Io(io::Error::new(io::ErrorKind::NotFound, msg)))
        }
        Err(err) => Err(AiviError::Io(err)),
    }
}

fn print_help() {
    println!(
        "aivi {} (language {})\n\nUSAGE:\n  aivi <COMMAND>\n\nCOMMANDS:\n  version\n  init <name> [--bin|--lib] [--edition 2024] [--language-version 0.1] [--force]\n  new <name> ... (alias of init)\n  search <query>\n  install <spec> [--no-fetch]\n  package [--allow-dirty] [--no-verify] [-- <cargo args...>]\n  publish [--dry-run] [--allow-dirty] [--no-verify] [-- <cargo args...>]\n  build [--release] [-- <cargo args...>]\n  run [--release] [--watch|-w] [-- <cargo args...>]\n  clean [--all]\n  daemon <start|status|stop>\n\n  parse <path|dir/...>\n  check [--debug-trace] [--check-stdlib] <path|dir/...>\n  fmt [--write] <path|dir/...>\n  desugar [--debug-trace] <path|dir/...>\n  kernel [--debug-trace] <path|dir/...>\n  rust-ir [--debug-trace] <path|dir/...>\n  test [--check-stdlib] <path|dir/...>\n  lsp\n  build <path|dir/...> [--debug-trace] [--out <dir|path>]\n  run <path|dir/...> [--debug-trace] [--watch|-w]\n  repl [--color] [--no-color] [--plain]\n  mcp serve <path|dir/...> [--allow-effects] [--ui]\n  i18n gen <catalog.properties> --locale <tag> --module <name> --out <file>\n\n  -h, --help\n  -V, --version",
        env!("CARGO_PKG_VERSION"),
        AIVI_LANGUAGE_VERSION
    );
}

fn print_version() {
    println!("{}", version_text());
}

fn version_text() -> String {
    format!(
        "aivi {} (language {})",
        env!("CARGO_PKG_VERSION"),
        AIVI_LANGUAGE_VERSION
    )
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
            let mut ui = false;
            for arg in args.iter().skip(1) {
                match arg.as_str() {
                    "--allow-effects" => allow_effects = true,
                    "--ui" => ui = true,
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
            cmd_mcp_serve(target, allow_effects, ui)
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
    let module_source =
        aivi::generate_i18n_module_from_properties(&module_name, &locale, &properties_text)
            .map_err(AiviError::InvalidCommand)?;

    let out_path = PathBuf::from(out_path);
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&out_path, module_source)?;
    println!("{}", out_path.display());
    Ok(())
}

fn cmd_mcp_serve(target: &str, allow_effects: bool, ui: bool) -> Result<(), AiviError> {
    // `aivi mcp serve` is meant to work even outside of a project checkout. In v0.1 it exposes
    // the bundled language specifications and does not depend on project code.
    let _ = target;
    let manifest = if ui {
        aivi::bundled_specs_manifest_with_ui()
    } else {
        aivi::bundled_specs_manifest()
    };
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
    debug_trace: bool,
    watch: bool,
}

fn parse_build_args(
    mut args: impl Iterator<Item = String>,
    allow_out: bool,
    default_target: &str,
) -> Result<Option<BuildArgs>, AiviError> {
    let mut input = None;
    let mut output = None;
    let mut target = default_target.to_string();
    let mut debug_trace = false;
    let mut watch = false;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--debug-trace" => {
                debug_trace = true;
            }
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
            "--watch" | "-w" => {
                watch = true;
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
        debug_trace,
        watch,
    }))
}

fn maybe_enable_debug_trace(enabled: bool) {
    if enabled {
        std::env::set_var("AIVI_DEBUG_TRACE", "1");
    }
}

fn consume_debug_trace_flag(args: &[String]) -> (bool, Vec<String>) {
    let mut enabled = false;
    let mut out = Vec::new();
    for arg in args {
        if arg == "--debug-trace" {
            enabled = true;
        } else {
            out.push(arg.clone());
        }
    }
    (enabled, out)
}

fn consume_check_stdlib_flag(args: &[String]) -> (bool, Vec<String>) {
    let mut enabled = false;
    let mut out = Vec::new();
    for arg in args {
        if arg == "--check-stdlib" {
            enabled = true;
        } else {
            out.push(arg.clone());
        }
    }
    (enabled, out)
}

fn consume_flag(flag: &str, args: &[String]) -> (bool, Vec<String>) {
    let mut enabled = false;
    let mut out = Vec::new();
    for arg in args {
        if arg == flag {
            enabled = true;
        } else {
            out.push(arg.clone());
        }
    }
    (enabled, out)
}

fn consume_multi_value_flag(flag: &str, args: &[String]) -> Result<(Vec<String>, Vec<String>), AiviError> {
    let mut values = Vec::new();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < args.len() {
        let arg = &args[i];
        if arg == flag {
            let Some(value) = args.get(i + 1) else {
                return Err(AiviError::InvalidCommand(format!("{flag} expects a value")));
            };
            if value.starts_with('-') {
                return Err(AiviError::InvalidCommand(format!("{flag} expects a value")));
            }
            values.push(value.clone());
            i += 2;
            continue;
        }
        out.push(arg.clone());
        i += 1;
    }
    Ok((values, out))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TargetContext {
    target: String,
    project_root: Option<PathBuf>,
}

fn resolve_check_or_test_target(args: &[String]) -> Option<TargetContext> {
    if let Ok(root) = env::current_dir() {
        let toml_path = root.join("aivi.toml");
        if toml_path.exists() {
            if let Ok(cfg) = aivi::read_aivi_toml(&toml_path) {
                return Some(TargetContext {
                    target: resolve_project_source_target(&root, &cfg.project.entry),
                    project_root: Some(root),
                });
            }
        }
    }

    let target = args.first()?.clone();
    Some(TargetContext {
        project_root: explicit_target_project_root(&target),
        target,
    })
}

fn explicit_target_project_root(target: &str) -> Option<PathBuf> {
    let base = target
        .strip_suffix("/...")
        .or_else(|| target.strip_suffix("/**"))
        .or_else(|| target.strip_suffix("\\..."))
        .or_else(|| target.strip_suffix("\\**"))
        .unwrap_or(target);
    let base = if base.is_empty() { "." } else { base };
    let path = Path::new(base).canonicalize().ok()?;
    if path.is_file() {
        path.parent().map(|parent| parent.to_path_buf())
    } else {
        Some(path)
    }
}

fn collect_candidate_test_paths(paths: &[PathBuf]) -> Result<Vec<PathBuf>, AiviError> {
    let mut test_paths = Vec::new();
    for path in paths {
        if path.extension().and_then(|s| s.to_str()) != Some("aivi") {
            continue;
        }
        let content = std::fs::read_to_string(path)?;
        if content.contains("@test") {
            test_paths.push(path.clone());
        }
    }
    Ok(test_paths)
}

fn format_test_paths_in_place(paths: &[PathBuf]) -> Result<(), AiviError> {
    for path in paths {
        let content = std::fs::read_to_string(path)?;
        let formatted = aivi::format_text(&content);
        if formatted != content {
            std::fs::write(path, formatted)?;
        }
    }
    Ok(())
}

fn collect_test_entries_with_paths(
    modules: &[aivi::Module],
) -> (Vec<(String, String)>, HashMap<String, PathBuf>) {
    let mut test_entries = Vec::new();
    let mut test_name_to_path = HashMap::<String, PathBuf>::new();
    for module in modules {
        if module.path.starts_with("<embedded:") {
            continue;
        }
        let path = PathBuf::from(module.path.clone());
        for item in &module.items {
            let aivi::ModuleItem::Def(def) = item else {
                continue;
            };
            if let Some(dec) = def.decorators.iter().find(|d| d.name.name == "test") {
                let name = format!("{}.{}", module.name.name, def.name.name);
                let description = match &dec.arg {
                    Some(aivi::Expr::Literal(aivi::Literal::String { text, .. })) => text.clone(),
                    _ => name.clone(),
                };
                test_name_to_path.insert(name.clone(), path.clone());
                test_entries.push((name, description));
            }
        }
    }
    test_entries.sort();
    test_entries.dedup();
    (test_entries, test_name_to_path)
}

struct Spinner {
    stop: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl Spinner {
    fn new(message: String) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let stop_clone = Arc::clone(&stop);
        let handle = std::thread::spawn(move || {
            let frames = ["|", "/", "-", "\\"];
            let mut idx = 0usize;
            while !stop_clone.load(Ordering::Relaxed) {
                eprint!("\r{} {}", frames[idx], message);
                let _ = std::io::stderr().flush();
                idx = (idx + 1) % frames.len();
                std::thread::sleep(Duration::from_millis(80));
            }
            eprint!("\rdone {}\n", message);
            let _ = std::io::stderr().flush();
        });
        Self { stop, handle: Some(handle) }
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

#[cfg(test)]
mod cli_tests {
    use super::*;

    #[test]
    fn version_text_contains_cli_and_language_versions() {
        let text = version_text();
        assert!(text.contains(env!("CARGO_PKG_VERSION")), "missing CLI version");
        assert!(text.contains(AIVI_LANGUAGE_VERSION), "missing language version");
        // Version text should contain both labels, not just the numbers
        assert!(
            text.lines().count() >= 1,
            "version text should have at least one line, got: {text:?}"
        );
    }

    #[test]
    fn explicit_target_project_root_handles_recursive_suffixes() {
        let dir = tempfile::tempdir().expect("tempdir");
        let src = dir.path().join("src");
        std::fs::create_dir_all(&src).expect("create src");

        let recursive = format!("{}/**", src.display());
        assert_eq!(
            explicit_target_project_root(&recursive),
            Some(src.canonicalize().expect("canonicalize src"))
        );
    }
}
