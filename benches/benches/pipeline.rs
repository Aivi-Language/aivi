use std::{collections::BTreeMap, fmt::Write as _, hint::black_box, path::PathBuf, time::Duration};

use aivi_backend::cache::compute_program_fingerprint;
use aivi_backend::{
    BackendExecutableProgram, Program, RuntimeValue, compile_program, compute_kernel_fingerprint,
};
use aivi_query::{
    RootDatabase, SourceFile, hir_module, parsed_file, reachable_workspace_hir_modules,
    whole_program_backend_unit,
};
use criterion::{BatchSize, Criterion, Throughput, criterion_group, criterion_main};

const PROGRAM: &str = concat!(
    "type Int -> Int\n",
    "func increment = value =>\n",
    "    value + 1\n",
    "\n",
    "value total = increment 41\n",
);

fn open_program(text: &str) -> (RootDatabase, SourceFile) {
    let db = RootDatabase::new();
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("benchmark.aivi");
    let file = SourceFile::new(&db, path, text.to_owned());
    (db, file)
}

fn open_workspace_reachability_program() -> (RootDatabase, SourceFile) {
    let db = RootDatabase::new();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("benchmark-workspace");
    SourceFile::new(
        &db,
        root.join("shared/math.aivi"),
        "type Int -> Int\nfunc inc = x =>\n    x + 1\n\nexport (inc)\n".to_owned(),
    );
    for index in 0..256 {
        SourceFile::new(
            &db,
            root.join(format!("unrelated/module{index}.aivi")),
            format!("value ignored{index}:Int = {index}\n"),
        );
    }
    let entry = SourceFile::new(
        &db,
        root.join("main.aivi"),
        "use shared.math (inc)\n\nvalue answer = inc 41\n".to_owned(),
    );
    (db, entry)
}

fn lower_program() -> Program {
    let (db, file) = open_program(PROGRAM);
    whole_program_backend_unit(&db, file)
        .expect("benchmark program should lower through the complete compiler pipeline")
        .backend()
        .clone()
}

fn fingerprint_program() -> Program {
    let mut source = String::from("value result:Int = 0");
    for value in 1..=256 {
        write!(source, " + {value}").expect("writing benchmark source should succeed");
    }
    source.push('\n');
    let (db, file) = open_program(&source);
    whole_program_backend_unit(&db, file)
        .expect("fingerprint benchmark should lower through the complete compiler pipeline")
        .backend()
        .clone()
}

fn find_item(program: &Program, name: &str) -> aivi_backend::ItemId {
    program
        .items()
        .iter()
        .find_map(|(id, item)| (item.name.as_ref() == name).then_some(id))
        .unwrap_or_else(|| panic!("benchmark program should contain `{name}`"))
}

fn bench_incremental_queries(c: &mut Criterion) {
    let mut group = c.benchmark_group("query");
    group.throughput(Throughput::Elements(1));

    let (db, file) = open_program(PROGRAM);
    black_box(parsed_file(&db, file));
    black_box(hir_module(&db, file));
    group.bench_function("cached_parse_and_hir", |b| {
        b.iter(|| {
            black_box(parsed_file(black_box(&db), black_box(file)));
            black_box(hir_module(black_box(&db), black_box(file)));
        });
    });

    group.bench_function("incremental_hir_after_edit", |b| {
        b.iter_batched(
            || {
                let (db, file) = open_program(PROGRAM);
                black_box(hir_module(&db, file));
                (db, file)
            },
            |(db, file)| {
                assert!(file.set_text(&db, PROGRAM.replace("41", "42")));
                black_box(hir_module(&db, file));
            },
            BatchSize::SmallInput,
        );
    });

    group.bench_function("workspace_reachability_cold_256_unrelated", |b| {
        b.iter_batched(
            open_workspace_reachability_program,
            |(db, entry)| {
                let modules = reachable_workspace_hir_modules(&db, entry);
                let project_modules = modules
                    .iter()
                    .filter(|module| !module.name().starts_with("aivi."))
                    .collect::<Vec<_>>();
                assert_eq!(project_modules.len(), 1);
                assert_eq!(project_modules[0].name(), "shared.math");
                black_box(modules);
            },
            BatchSize::LargeInput,
        );
    });

    group.finish();
}

fn bench_lowering_and_codegen(c: &mut Criterion) {
    let mut group = c.benchmark_group("pipeline");
    group.throughput(Throughput::Elements(1));

    group.bench_function("cold_full_lowering", |b| {
        b.iter_batched(
            || open_program(PROGRAM),
            |(db, file)| {
                black_box(
                    whole_program_backend_unit(&db, file).expect("benchmark program should lower"),
                );
            },
            BatchSize::SmallInput,
        );
    });

    let program = lower_program();
    let fingerprint_program = fingerprint_program();
    let fingerprint_item = find_item(&fingerprint_program, "result");
    let fingerprint_kernel = fingerprint_program.items()[fingerprint_item]
        .body
        .expect("fingerprint benchmark item should carry a body kernel");
    group.bench_function("kernel_fingerprint_256_exprs", |b| {
        b.iter(|| {
            black_box(compute_kernel_fingerprint(
                black_box(&fingerprint_program),
                black_box(fingerprint_kernel),
            ));
        });
    });
    group.bench_function("program_fingerprint_256_exprs", |b| {
        b.iter(|| {
            black_box(compute_program_fingerprint(black_box(&fingerprint_program)));
        });
    });

    group.bench_function("aot_object_codegen", |b| {
        b.iter(|| {
            black_box(
                compile_program(black_box(&program))
                    .expect("benchmark program should compile to an object"),
            );
        });
    });

    let total = find_item(&program, "total");
    let globals = BTreeMap::new();
    group.bench_function("jit_cold_evaluate", |b| {
        b.iter(|| {
            let executable = BackendExecutableProgram::interpreted(black_box(&program));
            let mut engine = executable.create_engine();
            let value = engine
                .evaluate_item(total, &globals)
                .expect("benchmark program should execute through the JIT");
            assert_eq!(value, RuntimeValue::Int(42));
            black_box(value);
        });
    });

    let executable = BackendExecutableProgram::interpreted(&program);
    let mut engine = executable.create_engine();
    assert_eq!(
        engine
            .evaluate_item(total, &globals)
            .expect("benchmark warmup should execute"),
        RuntimeValue::Int(42)
    );
    group.bench_function("jit_warm_evaluate", |b| {
        b.iter(|| {
            black_box(
                engine
                    .evaluate_item(total, &globals)
                    .expect("warmed benchmark program should execute"),
            );
        });
    });

    group.finish();
}

criterion_group! {
    name = pipeline;
    config = Criterion::default()
        .sample_size(30)
        .warm_up_time(Duration::from_secs(2))
        .measurement_time(Duration::from_secs(5));
    targets = bench_incremental_queries, bench_lowering_and_codegen
}
criterion_main!(pipeline);
