use aivi::parse_file;
use std::path::PathBuf;

fn main() {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: parse_quick <file>");
        std::process::exit(2);
    };
    let path = PathBuf::from(path);
    let file = match parse_file(&path) {
        Ok(f) => f,
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(2);
        }
    };
    if file.diagnostics.is_empty() {
        return;
    }
    for diag in &file.diagnostics {
        eprintln!(
            "{}:{}:{} {}: {}",
            path.display(),
            diag.span.start.line,
            diag.span.start.column,
            diag.code,
            diag.message
        );
    }
    std::process::exit(1);
}
