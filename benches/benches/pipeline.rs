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

fn bench_hir_elaboration(c: &mut Criterion) {
    let (db, file) = open_program(PROGRAM);
    let hir = hir_module(&db, file);
    let module = hir.module();
    let mut group = c.benchmark_group("hir_elaboration");
    group.bench_function("prepare_each_pass", |b| {
        b.iter(|| {
            black_box(aivi_hir::elaborate_general_expressions(module));
            black_box(aivi_hir::elaborate_ambient_items(module));
            black_box(aivi_hir::elaborate_gates(module));
            black_box(aivi_hir::elaborate_truthy_falsy(module));
            black_box(aivi_hir::elaborate_fanouts(module));
            black_box(aivi_hir::elaborate_temporal_stages(module));
            black_box(aivi_hir::elaborate_recurrences(module));
            black_box(aivi_hir::elaborate_source_lifecycles(module));
            black_box(aivi_hir::generate_source_decode_programs(module));
        })
    });
    group.bench_function("shared_preparation", |b| {
        b.iter(|| {
            let session = aivi_hir::ElaborationSession::new(module);
            black_box(session.elaborate_general_expressions());
            black_box(session.elaborate_ambient_items());
            black_box(session.elaborate_gates());
            black_box(session.elaborate_truthy_falsy());
            black_box(session.elaborate_fanouts());
            black_box(session.elaborate_temporal_stages());
            black_box(session.elaborate_recurrences());
            black_box(session.elaborate_source_lifecycles());
            black_box(session.generate_source_decode_programs());
        })
    });
    group.finish();
}

fn bench_native_flat_map_input(c: &mut Criterion) {
    let (db, file) = open_program(
        "type Int -> List Int\nfunc singleton = x => [x]\ntype List Int -> List Int\nfunc expand = xs => __aivi_list_flatMap singleton xs\n",
    );
    let unit = whole_program_backend_unit(&db, file).expect("flatMap input benchmark must lower");
    let program = unit.backend();
    let kernel = program.items()[find_item(program, "expand")].body.unwrap();
    aivi_backend::compile_native_kernel_artifact(program, kernel)
        .expect("native compilation must succeed")
        .expect("flatMap must produce a native artifact");
    let executable = BackendExecutableProgram::interpreted(program);
    let mut engine = executable.create_engine();
    let globals = BTreeMap::new();
    let mut group = c.benchmark_group("native_flat_map_input");
    for count in [256_i64, 1024, 4096] {
        let environments = [0, 1].map(|extra| {
            [RuntimeValue::List(
                (0..count + extra).map(RuntimeValue::Int).collect(),
            )]
        });
        let value = engine
            .evaluate_kernel(kernel, None, &environments[0], &globals)
            .unwrap();
        assert_eq!(value, environments[0][0]);
        let mut iteration = 0;
        group.bench_function(format!("singleton_{count}"), |b| {
            b.iter(|| {
                iteration ^= 1;
                black_box(
                    engine
                        .evaluate_kernel(kernel, None, &environments[iteration], &globals)
                        .unwrap(),
                );
            })
        });
    }
    group.finish();
}

fn bench_native_flat_map(c: &mut Criterion) {
    let (db, file) = open_program(
        "type Int -> List Int\nfunc singleton = x => [x]\ntype Int -> List Int\nfunc expand = n => __aivi_list_flatMap singleton (__aivi_list_range n)\n",
    );
    let unit = whole_program_backend_unit(&db, file).expect("flatMap benchmark must lower");
    let program = unit.backend();
    let kernel = program.items()[find_item(program, "expand")].body.unwrap();
    let executable = BackendExecutableProgram::interpreted(program);
    let mut engine = executable.create_engine();
    let globals = BTreeMap::new();
    let mut group = c.benchmark_group("native_flat_map");
    for count in [256_i64, 1024, 4096] {
        let mut iteration = 0;
        let value = engine
            .evaluate_kernel(kernel, None, &[RuntimeValue::Int(count)], &globals)
            .unwrap();
        let RuntimeValue::List(values) = value else {
            panic!("expected list")
        };
        assert_eq!(
            values,
            (0..count).map(RuntimeValue::Int).collect::<Vec<_>>()
        );
        group.bench_function(format!("singleton_{count}"), |b| {
            b.iter(|| {
                iteration ^= 1;
                black_box(
                    engine
                        .evaluate_kernel(
                            kernel,
                            None,
                            &[RuntimeValue::Int(count + iteration)],
                            &globals,
                        )
                        .unwrap(),
                );
            })
        });
    }
    group.finish();
}

criterion_group! {
    name = pipeline;
    config = Criterion::default()
        .sample_size(30)
        .warm_up_time(Duration::from_secs(2))
        .measurement_time(Duration::from_secs(5));
    targets = bench_native_flat_map_input, bench_hir_elaboration, bench_native_flat_map, bench_incremental_queries, bench_lowering_and_codegen
}
criterion_main!(pipeline);
