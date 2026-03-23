#![forbid(unsafe_code)]

use std::{
    env,
    ffi::OsString,
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
    process::ExitCode,
};

use aivi_backend::{compile_program, lower_module as lower_backend_module, validate_program};
use aivi_base::{Diagnostic, FileId, Severity, SourceDatabase};
use aivi_core::{lower_module as lower_core_module, validate_module as validate_core_module};
use aivi_hir::{ValidationMode, lower_module as lower_hir_module};
use aivi_lambda::{lower_module as lower_lambda_module, validate_module as validate_lambda_module};
use aivi_syntax::{Formatter, ItemKind, TokenKind, lex_module, parse_module};

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<ExitCode, String> {
    let mut args = env::args_os();
    let _binary = args.next();

    let Some(first) = args.next() else {
        print_usage();
        return Ok(ExitCode::from(2));
    };

    if first == OsString::from("check") {
        return check_file(&take_path(args)?);
    }

    if first == OsString::from("compile") {
        return run_compile(args);
    }

    if first == OsString::from("lex") {
        return lex_file(&take_path(args)?);
    }

    if first == OsString::from("lsp") {
        return run_lsp(args);
    }

    if first == OsString::from("fmt") {
        return run_fmt(args);
    }

    // Default: treat the first argument as a path and run `check`.
    check_file(&PathBuf::from(first))
}

fn run_fmt(mut args: impl Iterator<Item = OsString>) -> Result<ExitCode, String> {
    let Some(next) = args.next() else {
        return Err("expected a path or --stdin/--check argument after `fmt`".to_owned());
    };

    if next == OsString::from("--stdin") {
        return format_stdin();
    }

    if next == OsString::from("--check") {
        // Collect remaining paths; if none given use no-op (no files = no changes).
        let paths: Vec<PathBuf> = args.map(PathBuf::from).collect();
        return format_check(&paths);
    }

    // Treat as a file path — format to stdout (legacy behaviour).
    format_file(&PathBuf::from(next))
}

fn take_path(mut args: impl Iterator<Item = OsString>) -> Result<PathBuf, String> {
    args.next()
        .map(PathBuf::from)
        .ok_or_else(|| "expected a path argument".to_owned())
}

fn run_compile(mut args: impl Iterator<Item = OsString>) -> Result<ExitCode, String> {
    let path = args
        .next()
        .map(PathBuf::from)
        .ok_or_else(|| "expected a path argument after `compile`".to_owned())?;
    let mut output = None;

    while let Some(argument) = args.next() {
        if argument == OsString::from("-o") || argument == OsString::from("--output") {
            let artifact = args
                .next()
                .map(PathBuf::from)
                .ok_or_else(|| "expected a path after `-o`/`--output` for `compile`".to_owned())?;
            if output.replace(artifact).is_some() {
                return Err("compile output path was provided more than once".to_owned());
            }
            continue;
        }

        return Err(format!(
            "unexpected compile argument `{}`",
            argument.to_string_lossy()
        ));
    }

    compile_file(&path, output.as_deref())
}

fn load_source(path: &Path) -> Result<(SourceDatabase, FileId), String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let mut sources = SourceDatabase::new();
    let file_id = sources.add_file(path.to_path_buf(), text);
    Ok((sources, file_id))
}

fn check_file(path: &Path) -> Result<ExitCode, String> {
    let (sources, file_id) = load_source(path)?;
    let file = &sources[file_id];
    let parsed = parse_module(file);
    let syntax_failed = print_diagnostics(&sources, parsed.all_diagnostics());
    if syntax_failed {
        Ok(ExitCode::FAILURE)
    } else {
        let lowered = lower_hir_module(&parsed.module);
        let lowering_failed = print_diagnostics(&sources, lowered.diagnostics());
        let validation_mode = if lowering_failed {
            ValidationMode::Structural
        } else {
            ValidationMode::RequireResolvedNames
        };
        let validation = lowered.module().validate(validation_mode);
        let validation_failed = print_diagnostics(&sources, validation.diagnostics());
        if lowering_failed || validation_failed {
            return Ok(ExitCode::FAILURE);
        }
        println!(
            "syntax + HIR passed: {} ({} item{})",
            path.display(),
            parsed.module.items.len(),
            if parsed.module.items.len() == 1 {
                ""
            } else {
                "s"
            }
        );
        Ok(ExitCode::SUCCESS)
    }
}

fn compile_file(path: &Path, output: Option<&Path>) -> Result<ExitCode, String> {
    let (sources, file_id) = load_source(path)?;
    let file = &sources[file_id];
    let parsed = parse_module(file);
    let syntax_failed =
        print_stage_diagnostics(CompileStage::Syntax, &sources, parsed.all_diagnostics());
    if syntax_failed {
        print_pipeline_stop(CompileStage::Syntax);
        return Ok(ExitCode::FAILURE);
    }

    let lowered = lower_hir_module(&parsed.module);
    let hir_lowering_failed =
        print_stage_diagnostics(CompileStage::HirLowering, &sources, lowered.diagnostics());
    let validation_mode = if hir_lowering_failed {
        ValidationMode::Structural
    } else {
        ValidationMode::RequireResolvedNames
    };
    let validation = lowered.module().validate(validation_mode);
    let hir_validation_failed = print_stage_diagnostics(
        CompileStage::HirValidation,
        &sources,
        validation.diagnostics(),
    );
    if hir_lowering_failed {
        print_pipeline_stop(CompileStage::HirLowering);
        return Ok(ExitCode::FAILURE);
    }
    if hir_validation_failed {
        print_pipeline_stop(CompileStage::HirValidation);
        return Ok(ExitCode::FAILURE);
    }

    let hir_module = lowered.module();
    let core = match lower_core_module(hir_module) {
        Ok(core) => core,
        Err(errors) => {
            print_stage_errors(CompileStage::TypedCoreLowering, errors.errors());
            print_pipeline_stop(CompileStage::TypedCoreLowering);
            return Ok(ExitCode::FAILURE);
        }
    };
    if let Err(errors) = validate_core_module(&core) {
        print_stage_errors(CompileStage::TypedCoreValidation, errors.errors());
        print_pipeline_stop(CompileStage::TypedCoreValidation);
        return Ok(ExitCode::FAILURE);
    }

    let lambda = match lower_lambda_module(&core) {
        Ok(lambda) => lambda,
        Err(errors) => {
            print_stage_errors(CompileStage::TypedLambdaLowering, errors.errors());
            print_pipeline_stop(CompileStage::TypedLambdaLowering);
            return Ok(ExitCode::FAILURE);
        }
    };
    if let Err(errors) = validate_lambda_module(&lambda) {
        print_stage_errors(CompileStage::TypedLambdaValidation, errors.errors());
        print_pipeline_stop(CompileStage::TypedLambdaValidation);
        return Ok(ExitCode::FAILURE);
    }

    let backend = match lower_backend_module(&lambda) {
        Ok(backend) => backend,
        Err(errors) => {
            print_stage_errors(CompileStage::BackendLowering, errors.errors());
            print_pipeline_stop(CompileStage::BackendLowering);
            return Ok(ExitCode::FAILURE);
        }
    };
    if let Err(errors) = validate_program(&backend) {
        print_stage_errors(CompileStage::BackendValidation, errors.errors());
        print_pipeline_stop(CompileStage::BackendValidation);
        return Ok(ExitCode::FAILURE);
    }

    let compiled = match compile_program(&backend) {
        Ok(compiled) => compiled,
        Err(errors) => {
            print_stage_errors(CompileStage::Codegen, errors.errors());
            print_pipeline_stop(CompileStage::Codegen);
            return Ok(ExitCode::FAILURE);
        }
    };

    if let Some(output_path) = output {
        write_object_file(output_path, compiled.object())?;
    }

    println!("compile pipeline passed: {}", path.display());
    println!(
        "  syntax: ok ({} surface item{})",
        parsed.module.items.len(),
        plural_suffix(parsed.module.items.len())
    );
    let hir_item_count = hir_module.items().iter().count();
    println!(
        "  HIR: ok ({} item{})",
        hir_item_count,
        plural_suffix(hir_item_count)
    );
    let core_item_count = core.items().iter().count();
    println!(
        "  typed core: ok ({} item{})",
        core_item_count,
        plural_suffix(core_item_count)
    );
    let lambda_item_count = lambda.items().iter().count();
    let lambda_closure_count = lambda.closures().iter().count();
    println!(
        "  typed lambda: ok ({} item{}, {} closure{})",
        lambda_item_count,
        plural_suffix(lambda_item_count),
        lambda_closure_count,
        plural_suffix(lambda_closure_count)
    );
    let backend_item_count = backend.items().iter().count();
    let pipeline_count = backend.pipelines().iter().count();
    let kernel_count = backend.kernels().iter().count();
    println!(
        "  backend: ok ({} item{}, {} pipeline{}, {} kernel{})",
        backend_item_count,
        plural_suffix(backend_item_count),
        pipeline_count,
        plural_suffix(pipeline_count),
        kernel_count,
        plural_suffix(kernel_count)
    );
    println!(
        "  codegen: ok ({} compiled kernel{}, {} byte{})",
        compiled.kernels().len(),
        plural_suffix(compiled.kernels().len()),
        compiled.object().len(),
        plural_suffix(compiled.object().len())
    );
    if let Some(output_path) = output {
        println!("  object file: {}", output_path.display());
    } else {
        println!("  object file: not written (pass -o/--output to persist it)");
    }
    println!(
        "runtime startup/link integration is not available yet; the supported CLI boundary is Cranelift object code, not a runnable GTK binary."
    );
    Ok(ExitCode::SUCCESS)
}

fn print_diagnostics<'a>(
    sources: &SourceDatabase,
    diagnostics: impl IntoIterator<Item = &'a Diagnostic>,
) -> bool {
    let mut saw_error = false;
    for diagnostic in diagnostics {
        eprintln!("{}\n", diagnostic.render(sources));
        if diagnostic.severity == Severity::Error {
            saw_error = true;
        }
    }
    saw_error
}

fn print_stage_diagnostics<'a>(
    stage: CompileStage,
    sources: &SourceDatabase,
    diagnostics: impl IntoIterator<Item = &'a Diagnostic>,
) -> bool {
    let mut saw_any = false;
    let mut saw_error = false;
    for diagnostic in diagnostics {
        if !saw_any {
            eprintln!("{} diagnostics:\n", stage.label());
            saw_any = true;
        }
        eprintln!("{}\n", diagnostic.render(sources));
        if diagnostic.severity == Severity::Error {
            saw_error = true;
        }
    }
    saw_error
}

fn print_stage_errors<E: std::fmt::Display>(stage: CompileStage, errors: &[E]) {
    eprintln!("{} failed:", stage.label());
    if errors.is_empty() {
        eprintln!("- no detailed errors were reported");
        return;
    }
    for error in errors {
        eprintln!("- {error}");
    }
}

fn print_pipeline_stop(stage: CompileStage) {
    eprintln!("compile pipeline stopped at {}.", stage.label());
}

fn write_object_file(path: &Path, object: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    fs::write(path, object).map_err(|error| format!("failed to write {}: {error}", path.display()))
}

fn plural_suffix(count: usize) -> &'static str {
    if count == 1 { "" } else { "s" }
}

#[derive(Clone, Copy, Debug)]
enum CompileStage {
    Syntax,
    HirLowering,
    HirValidation,
    TypedCoreLowering,
    TypedCoreValidation,
    TypedLambdaLowering,
    TypedLambdaValidation,
    BackendLowering,
    BackendValidation,
    Codegen,
}

impl CompileStage {
    const fn label(self) -> &'static str {
        match self {
            Self::Syntax => "syntax",
            Self::HirLowering => "HIR lowering",
            Self::HirValidation => "HIR validation",
            Self::TypedCoreLowering => "typed-core lowering",
            Self::TypedCoreValidation => "typed-core validation",
            Self::TypedLambdaLowering => "typed-lambda lowering",
            Self::TypedLambdaValidation => "typed-lambda validation",
            Self::BackendLowering => "backend lowering",
            Self::BackendValidation => "backend validation",
            Self::Codegen => "codegen",
        }
    }
}

fn lex_file(path: &Path) -> Result<ExitCode, String> {
    let (sources, file_id) = load_source(path)?;
    let file = &sources[file_id];
    let lexed = lex_module(file);

    for token in lexed
        .tokens()
        .iter()
        .filter(|token| !token.kind().is_trivia())
    {
        println!(
            "{kind:?} @{start}..{end} {text:?}{line_start}",
            kind = token.kind(),
            start = token.span().start().as_u32(),
            end = token.span().end().as_u32(),
            text = token.text(file),
            line_start = if token.line_start() {
                " [line-start]"
            } else {
                ""
            },
        );
    }

    if lexed.has_errors() {
        for diagnostic in lexed.diagnostics() {
            eprintln!("{}\n", diagnostic.render(&sources));
        }
        Ok(ExitCode::FAILURE)
    } else {
        Ok(ExitCode::SUCCESS)
    }
}

fn format_file(path: &Path) -> Result<ExitCode, String> {
    let (sources, file_id) = load_source(path)?;
    let file = &sources[file_id];
    let parsed = parse_module(file);
    if parsed.has_errors() {
        for diagnostic in parsed.all_diagnostics() {
            eprintln!("{}\n", diagnostic.render(&sources));
        }
        return Ok(ExitCode::FAILURE);
    }

    let formatter = Formatter;
    print!("{}", formatter.format(&parsed.module));
    Ok(ExitCode::SUCCESS)
}

fn format_stdin() -> Result<ExitCode, String> {
    let mut source = String::new();
    io::stdin()
        .read_to_string(&mut source)
        .map_err(|e| format!("failed to read stdin: {e}"))?;
    let mut sources = SourceDatabase::new();
    let file_id = sources.add_file("<stdin>", source);
    let file = &sources[file_id];
    let parsed = parse_module(file);
    // Per plan/02: tolerate parse errors, emit formatted output regardless.
    let formatter = Formatter;
    print!("{}", formatter.format(&parsed.module));
    Ok(ExitCode::SUCCESS)
}

fn format_check(paths: &[PathBuf]) -> Result<ExitCode, String> {
    let mut any_changed = false;
    for path in paths {
        let (sources, file_id) = load_source(path)?;
        let file = &sources[file_id];
        let parsed = parse_module(file);
        let formatter = Formatter;
        let formatted = formatter.format(&parsed.module);
        if formatted != file.text() {
            println!("{}", path.display());
            any_changed = true;
        }
    }
    if any_changed {
        Ok(ExitCode::FAILURE)
    } else {
        Ok(ExitCode::SUCCESS)
    }
}

fn run_lsp(_args: impl Iterator<Item = OsString>) -> Result<ExitCode, String> {
    tokio::runtime::Runtime::new()
        .map_err(|e| format!("failed to create tokio runtime: {e}"))?
        .block_on(aivi_lsp::run())
        .map_err(|e| format!("LSP server error: {e}"))?;
    Ok(ExitCode::SUCCESS)
}

fn print_usage() {
    eprintln!(
        "usage:\n  aivi <path>\n  aivi check <path>\n  aivi compile <path> [-o <object>]\n  aivi lex <path>\n  aivi fmt <path>\n  aivi fmt --stdin\n  aivi fmt --check [path...]\n  aivi lsp"
    );
    eprintln!(
        "commands:\n  check    Lex, parse, lower, and validate a module through HIR\n  compile  Lower through typed core, typed lambda, backend, and Cranelift codegen\n  lex      Dump the lossless token stream\n  fmt      Canonically format the supported surface subset\n  lsp      Start the language server"
    );
    eprintln!(
        "milestone-2 surface items: {:?}",
        [
            ItemKind::Type,
            ItemKind::Value,
            ItemKind::Function,
            ItemKind::Signal,
            ItemKind::Class,
            ItemKind::Instance,
            ItemKind::Domain,
            ItemKind::SourceProviderContract,
            ItemKind::Use,
            ItemKind::Export,
        ]
    );
    eprintln!(
        "core pipe operators: {:?}",
        [
            TokenKind::PipeTransform,
            TokenKind::PipeGate,
            TokenKind::PipeCase,
            TokenKind::PipeMap,
            TokenKind::PipeApply,
            TokenKind::PipeRecurStart,
            TokenKind::PipeRecurStep,
            TokenKind::PipeTap,
            TokenKind::PipeFanIn,
        ]
    );
}

#[cfg(test)]
mod tests {
    use super::check_file;
    use std::{path::PathBuf, process::ExitCode};

    fn fixture(path: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("fixtures/frontend")
            .join(path)
    }

    #[test]
    fn check_accepts_milestone_two_valid_fixtures() {
        for path in [
            "milestone-2/valid/source-decorator-signals/main.aivi",
            "milestone-2/valid/pipe-recurrence-nonsource-wakeup/main.aivi",
        ] {
            let result = check_file(&fixture(path)).expect("check should run");
            assert_eq!(result, ExitCode::SUCCESS, "expected {path} to pass");
        }
    }

    #[test]
    fn check_rejects_milestone_two_invalid_fixture() {
        let result = check_file(&fixture("milestone-2/invalid/unknown-decorator/main.aivi"))
            .expect("check should run");
        assert_eq!(result, ExitCode::FAILURE);
    }
}
