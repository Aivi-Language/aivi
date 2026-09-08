use std::{hint::black_box, time::Duration};

use aivi_base::SourceDatabase;
use aivi_syntax::{lex_module, parse_module};
use criterion::{Criterion, criterion_group, criterion_main};

fn bench_parse_snake(c: &mut Criterion) {
    let mut sources = SourceDatabase::new();
    let file_id = sources.add_file("demos/snake.aivi", include_str!("../../demos/snake.aivi"));
    let source_file = &sources[file_id];
    c.bench_function("frontend_parse_snake", |b| {
        b.iter(|| {
            black_box(parse_module(black_box(source_file)));
        })
    });
}

fn bench_parse_reversi(c: &mut Criterion) {
    let mut sources = SourceDatabase::new();
    let file_id = sources.add_file(
        "demos/reversi.aivi",
        include_str!("../../demos/reversi.aivi"),
    );
    let source_file = &sources[file_id];
    c.bench_function("frontend_parse_reversi", |b| {
        b.iter(|| {
            black_box(parse_module(black_box(source_file)));
        })
    });
}

fn bench_lex_large_source(c: &mut Criterion) {
    let mut sources = SourceDatabase::new();
    let source = include_str!("../../demos/snake.aivi").repeat(10);
    let file_id = sources.add_file("demos/snake_10x.aivi", source);
    let source_file = &sources[file_id];
    c.bench_function("frontend_lex_10x_snake", |b| {
        b.iter(|| {
            black_box(lex_module(black_box(source_file)));
        })
    });
}

criterion_group! {
    name = parser;
    config = Criterion::default()
        .sample_size(40)
        .warm_up_time(Duration::from_secs(2))
        .measurement_time(Duration::from_secs(5));
    targets = bench_parse_snake, bench_parse_reversi, bench_lex_large_source
}
criterion_main!(parser);
