use std::{fs, hint::black_box, path::PathBuf, time::Duration};

use aivi_base::SourceDatabase;
use aivi_hir::{
    ImportModuleResolution, ImportResolver, exports, lower_module, lower_module_with_resolver,
    typecheck_module,
};
use aivi_syntax::parse_module;

use criterion::{Criterion, criterion_group, criterion_main};

/// Resolves `aivi.*` stdlib imports from the bundled stdlib directory.
struct StdlibResolver {
    stdlib_root: PathBuf,
}

impl StdlibResolver {
    fn new() -> Self {
        Self {
            stdlib_root: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../stdlib"),
        }
    }
}

impl ImportResolver for StdlibResolver {
    fn resolve(&self, path: &[&str]) -> ImportModuleResolution {
        if path.first() != Some(&"aivi") {
            return ImportModuleResolution::Missing;
        }
        let mut file_path = self.stdlib_root.clone();
        for segment in path {
            file_path.push(segment);
        }
        file_path.set_extension("aivi");
        let text = match fs::read_to_string(&file_path) {
            Ok(t) => t,
            Err(_) => return ImportModuleResolution::Missing,
        };
        let mut sources = SourceDatabase::new();
        let file_id = sources.add_file(file_path.to_string_lossy().as_ref(), text.as_str());
        let parsed = parse_module(&sources[file_id]);
        if parsed.has_errors() {
            return ImportModuleResolution::Missing;
        }
        let lowered = lower_module(&parsed.module);
        ImportModuleResolution::Resolved(exports(lowered.module()))
    }
}

fn bench_typecheck_snake(c: &mut Criterion) {
    let resolver = StdlibResolver::new();
    let mut sources = SourceDatabase::new();
    let file_id = sources.add_file("demos/snake.aivi", include_str!("../../demos/snake.aivi"));
    let source_file = &sources[file_id];
    c.bench_function("frontend_typecheck_snake", |b| {
        b.iter(|| {
            let parsed = parse_module(black_box(source_file));
            let lowered = lower_module_with_resolver(black_box(&parsed.module), Some(&resolver));
            black_box(typecheck_module(lowered.module()));
        })
    });
}

fn bench_typecheck_reversi(c: &mut Criterion) {
    let resolver = StdlibResolver::new();
    let mut sources = SourceDatabase::new();
    let file_id = sources.add_file(
        "demos/reversi.aivi",
        include_str!("../../demos/reversi.aivi"),
    );
    let source_file = &sources[file_id];
    c.bench_function("frontend_typecheck_reversi", |b| {
        b.iter(|| {
            let parsed = parse_module(black_box(source_file));
            let lowered = lower_module_with_resolver(black_box(&parsed.module), Some(&resolver));
            black_box(typecheck_module(lowered.module()));
        })
    });
}

fn bench_typecheck_large(c: &mut Criterion) {
    let resolver = StdlibResolver::new();
    let mut sources = SourceDatabase::new();
    let source = include_str!("../../demos/snake.aivi").repeat(10);
    let file_id = sources.add_file("demos/snake_10x.aivi", source);
    let source_file = &sources[file_id];
    c.bench_function("frontend_typecheck_10x_snake", |b| {
        b.iter(|| {
            let parsed = parse_module(black_box(source_file));
            let lowered = lower_module_with_resolver(black_box(&parsed.module), Some(&resolver));
            black_box(typecheck_module(lowered.module()));
        })
    });
}

criterion_group! {
    name = typecheck;
    config = Criterion::default()
        .sample_size(30)
        .warm_up_time(Duration::from_secs(2))
        .measurement_time(Duration::from_secs(5));
    targets = bench_typecheck_snake, bench_typecheck_reversi, bench_typecheck_large
}
criterion_main!(typecheck);
