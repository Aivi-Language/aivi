use std::{fmt::Write as _, hint::black_box, path::PathBuf, sync::Arc, time::Duration};

use aivi_query::{RootDatabase, SourceFile, whole_program_backend_unit};
use aivi_runtime::{
    DependencyValues, InputHandle, Publication, Scheduler, SignalGraphBuilder,
    assemble_hir_runtime, derive_backend_runtime_link_seed, link_backend_runtime_with_seed,
};
use aivi_runtime::{
    derive_backend_linked_runtime_tables_with_seed_and_native_kernels_from_payload,
    hir_adapter::BackendRuntimePayload,
};
use criterion::{BatchSize, Criterion, Throughput, criterion_group, criterion_main};

const GRAPH_WIDTH: usize = 256;
const REQUIREMENT_WIDTH: usize = 16;

fn chain_scheduler() -> (Scheduler<i64>, InputHandle) {
    let mut builder = SignalGraphBuilder::new();
    let input = builder
        .add_input("input", None)
        .expect("benchmark input should be valid");
    let mut dependency = input.as_signal();
    for index in 0..GRAPH_WIDTH {
        let derived = builder
            .add_derived(format!("chain_{index}"), None)
            .expect("benchmark derived signal should be valid");
        builder
            .define_derived(derived, [dependency])
            .expect("benchmark chain should stay acyclic");
        dependency = derived.as_signal();
    }
    let mut scheduler = Scheduler::new(
        builder
            .build()
            .expect("benchmark chain graph should validate"),
    );
    let stamp = scheduler
        .current_stamp(input)
        .expect("benchmark input stamp should exist");
    scheduler
        .queue_publication(Publication::new(stamp, 0))
        .expect("benchmark publication should queue");
    scheduler.tick(&mut |_, values: DependencyValues<'_, i64>| {
        values.value(0).copied().map(|value| value + 1)
    });
    (scheduler, input)
}

fn fanout_scheduler() -> (Scheduler<i64>, InputHandle) {
    let mut builder = SignalGraphBuilder::new();
    let input = builder
        .add_input("input", None)
        .expect("benchmark input should be valid");
    for index in 0..GRAPH_WIDTH {
        let derived = builder
            .add_derived(format!("fanout_{index}"), None)
            .expect("benchmark derived signal should be valid");
        builder
            .define_derived(derived, [input.as_signal()])
            .expect("benchmark fanout should stay acyclic");
    }
    let mut scheduler = Scheduler::new(
        builder
            .build()
            .expect("benchmark fanout graph should validate"),
    );
    let stamp = scheduler
        .current_stamp(input)
        .expect("benchmark input stamp should exist");
    scheduler
        .queue_publication(Publication::new(stamp, 0))
        .expect("benchmark publication should queue");
    scheduler.tick(&mut |_, values: DependencyValues<'_, i64>| values.value(0).copied());
    (scheduler, input)
}

fn open_runtime_link_program() -> (RootDatabase, SourceFile) {
    let mut source = String::new();
    for index in 0..REQUIREMENT_WIDTH {
        writeln!(source, "signal base{index} = {index}")
            .expect("writing startup benchmark source should succeed");
    }
    for signal in 0..GRAPH_WIDTH {
        write!(source, "signal signal{signal} = ")
            .expect("writing startup benchmark source should succeed");
        for dependency in 0..REQUIREMENT_WIDTH {
            if dependency > 0 {
                source.push_str(" + ");
            }
            write!(source, "base{dependency}")
                .expect("writing startup benchmark source should succeed");
        }
        writeln!(source, " + {signal}").expect("writing startup benchmark source should succeed");
    }
    let db = RootDatabase::new();
    let file = SourceFile::new(
        &db,
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("runtime-link-benchmark.aivi"),
        source,
    );
    (db, file)
}

fn bench_scheduler(c: &mut Criterion) {
    let mut group = c.benchmark_group("scheduler");
    group.throughput(Throughput::Elements(GRAPH_WIDTH as u64));

    group.bench_function("build_chain_256", |b| {
        b.iter(|| black_box(chain_scheduler()));
    });

    let (mut chain, chain_input) = chain_scheduler();
    let mut chain_value = 1_i64;
    group.bench_function("propagate_chain_256", |b| {
        b.iter(|| {
            let stamp = chain
                .current_stamp(chain_input)
                .expect("benchmark input stamp should exist");
            chain
                .queue_publication(Publication::new(stamp, chain_value))
                .expect("benchmark publication should queue");
            chain_value += 1;
            black_box(chain.tick(&mut |_, values: DependencyValues<'_, i64>| {
                values.value(0).copied().map(|value| value + 1)
            }));
        });
    });

    let (mut fanout, fanout_input) = fanout_scheduler();
    let mut fanout_value = 1_i64;
    group.bench_function("propagate_fanout_256", |b| {
        b.iter(|| {
            let stamp = fanout
                .current_stamp(fanout_input)
                .expect("benchmark input stamp should exist");
            fanout
                .queue_publication(Publication::new(stamp, fanout_value))
                .expect("benchmark publication should queue");
            fanout_value += 1;
            black_box(
                fanout.tick(&mut |_, values: DependencyValues<'_, i64>| values.value(0).copied()),
            );
        });
    });

    group.finish();
}

fn bench_runtime_startup(c: &mut Criterion) {
    let (db, file) = open_runtime_link_program();
    let unit = whole_program_backend_unit(&db, file)
        .expect("startup benchmark should lower through the compiler pipeline");
    let assembly = assemble_hir_runtime(unit.entry_hir().module())
        .expect("startup benchmark should assemble a runtime graph");
    let seed = derive_backend_runtime_link_seed(unit.core(), unit.backend())
        .expect("startup benchmark should derive a backend link seed");
    let backend = unit.backend_arc();
    let backend_payload = BackendRuntimePayload::Program(Arc::clone(&backend));
    let native_kernels = Arc::new(aivi_backend::NativeKernelArtifactSet::default());

    let mut group = c.benchmark_group("runtime_startup");
    group.throughput(Throughput::Elements(GRAPH_WIDTH as u64));
    group.bench_function("derive_link_seed_256_signals", |b| {
        b.iter(|| {
            black_box(
                derive_backend_runtime_link_seed(black_box(unit.core()), black_box(unit.backend()))
                    .expect("startup benchmark should derive a backend link seed"),
            );
        });
    });
    group.bench_function("derive_link_tables_256_by_16_signals", |b| {
        b.iter(|| {
            black_box(
                derive_backend_linked_runtime_tables_with_seed_and_native_kernels_from_payload(
                    black_box(&assembly),
                    black_box(&backend_payload),
                    black_box(&native_kernels),
                    black_box(&seed),
                )
                .expect("startup benchmark should derive backend link tables"),
            );
        });
    });
    group.bench_function("link_256_signals", |b| {
        b.iter_batched(
            || assembly.clone(),
            |assembly| {
                black_box(
                    link_backend_runtime_with_seed(
                        assembly,
                        Arc::clone(black_box(&backend)),
                        black_box(&seed),
                    )
                    .expect("startup benchmark should link a runtime"),
                );
            },
            BatchSize::SmallInput,
        );
    });
    group.finish();
}

criterion_group! {
    name = runtime;
    config = Criterion::default()
        .sample_size(40)
        .warm_up_time(Duration::from_secs(2))
        .measurement_time(Duration::from_secs(5));
    targets = bench_scheduler, bench_runtime_startup
}
criterion_main!(runtime);
