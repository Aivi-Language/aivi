use aivi::{
    check_modules, check_types, compile_wasm, desugar_target, format_target,
    load_module_diagnostics, load_modules, parse_target, render_diagnostics, run_native, run_wasm,
    AiviError,
};
use std::env;
use std::path::PathBuf;
use std::process::{Command, ExitCode};

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

    match command.as_str() {
        "-h" | "--help" => {
            print_help();
            Ok(())
        }
        "parse" => {
            let Some(target) = args.next() else {
                print_help();
                return Ok(());
            };
            let bundle = parse_target(&target)?;
            let output = serde_json::to_string_pretty(&bundle)
                .map_err(|err| AiviError::Io(std::io::Error::new(std::io::ErrorKind::Other, err)))?;
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
            let Some(target) = args.next() else {
                print_help();
                return Ok(());
            };
            let mut diagnostics = load_module_diagnostics(&target)?;
            let modules = load_modules(&target)?;
            diagnostics.extend(check_modules(&modules));
            if diagnostics.is_empty() {
                diagnostics.extend(check_types(&modules));
            }
            if diagnostics.is_empty() {
                return Ok(());
            }
            for diag in diagnostics {
                let rendered = render_diagnostics(&diag.path, std::slice::from_ref(&diag.diagnostic));
                if !rendered.is_empty() {
                    eprintln!("{rendered}");
                }
            }
            Err(AiviError::Diagnostics)
        }
        "fmt" => {
            let Some(target) = args.next() else {
                print_help();
                return Ok(());
            };
            let formatted = format_target(&target)?;
            print!("{formatted}");
            Ok(())
        }
        "desugar" => {
            let Some(target) = args.next() else {
                print_help();
                return Ok(());
            };
            let diagnostics = load_module_diagnostics(&target)?;
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
            let program = desugar_target(&target)?;
            let output = serde_json::to_string_pretty(&program)
                .map_err(|err| AiviError::Io(std::io::Error::new(std::io::ErrorKind::Other, err)))?;
            println!("{output}");
            Ok(())
        }
        "lsp" | "build" | "run" => {
            match command.as_str() {
                "lsp" => {
                    let status = Command::new("aivi-lsp").args(args).status()?;
                    if !status.success() {
                        return Err(AiviError::Io(std::io::Error::new(
                            std::io::ErrorKind::Other,
                            "aivi-lsp exited with an error",
                        )));
                    }
                    Ok(())
                }
                "build" => {
                    let Some(opts) = parse_build_args(args, true)? else {
                        print_help();
                        return Ok(());
                    };
                    if opts.wasm_target != "wasm32-wasi" {
                        return Err(AiviError::InvalidCommand(format!(
                            "unsupported target {}",
                            opts.wasm_target
                        )));
                    }
                    let _modules = load_checked_modules(&opts.input)?;
                    let program = desugar_target(&opts.input)?;
                    let wasm = compile_wasm(program)?;
                    let out_path = opts
                        .output
                        .unwrap_or_else(|| PathBuf::from("target/aivi.wasm"));
                    if let Some(parent) = out_path.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    std::fs::write(&out_path, wasm)?;
                    println!("{}", out_path.display());
                    Ok(())
                }
                "run" => {
                    let Some(opts) = parse_build_args(args, false)? else {
                        print_help();
                        return Ok(());
                    };
                    if opts.wasm_target == "native" {
                        let _modules = load_checked_modules(&opts.input)?;
                        let program = desugar_target(&opts.input)?;
                        run_native(program)?;
                        return Ok(());
                    }
                    if opts.wasm_target != "wasm32-wasi" {
                        return Err(AiviError::InvalidCommand(format!(
                            "unsupported target {}",
                            opts.wasm_target
                        )));
                    }
                    let _modules = load_checked_modules(&opts.input)?;
                    let program = desugar_target(&opts.input)?;
                    let wasm = compile_wasm(program)?;
                    run_wasm(&wasm)?;
                    Ok(())
                }
                _ => Ok(()),
            }
        }
        _ => {
            print_help();
            Err(AiviError::InvalidCommand(command))
        }
    }
}

fn print_help() {
    println!(
        "aivi\n\nUSAGE:\n  aivi <COMMAND>\n\nCOMMANDS:\n  parse <path|dir/...>\n  check <path|dir/...>\n  fmt <path>\n  desugar <path|dir/...>\n  lsp\n  build <path|dir/...> [--target wasm32-wasi] [--out <file>]\n  run <path|dir/...> [--target wasm32-wasi|native]\n  -h, --help"
    );
}

struct BuildArgs {
    input: String,
    output: Option<PathBuf>,
    wasm_target: String,
}

fn parse_build_args(
    mut args: impl Iterator<Item = String>,
    allow_out: bool,
) -> Result<Option<BuildArgs>, AiviError> {
    let mut input = None;
    let mut output = None;
    let mut wasm_target = "wasm32-wasi".to_string();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--target" => {
                let Some(value) = args.next() else {
                    return Err(AiviError::InvalidCommand(
                        "--target expects a value".to_string(),
                    ));
                };
                wasm_target = value;
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
                return Err(AiviError::InvalidCommand(format!(
                    "unknown flag {arg}"
                )));
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
        wasm_target,
    }))
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
