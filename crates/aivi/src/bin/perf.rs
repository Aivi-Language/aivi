use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

use aivi::{
    check_modules, check_types, desugar_blocks, desugar_modules, file_diagnostics_have_errors,
};

#[derive(Debug, Clone)]
struct Fixture {
    name: String,
    path: PathBuf,
    contents: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct FixtureMetrics {
    bytes: usize,
    parse_ns: u128,
    resolve_ns: u128,
    typecheck_ns: u128,
    lower_ns: u128,
    total_ns: u128,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct PerfReport {
    fixtures: BTreeMap<String, FixtureMetrics>,
}

fn workspace_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root")
        .to_path_buf()
}

fn load_fixtures() -> Vec<Fixture> {
    let root = workspace_root();
    let dir = root.join("crates/aivi/perf/fixtures");
    let mut fixtures = Vec::new();
    let entries = fs::read_dir(&dir).unwrap_or_else(|err| panic!("read {}: {err}", dir.display()));
    for entry in entries {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("aivi") {
            continue;
        }
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("fixture")
            .to_string();
        let contents = fs::read_to_string(&path).expect("read fixture");
        fixtures.push(Fixture {
            name,
            path,
            contents,
        });
    }
    fixtures.sort_by(|a, b| a.name.cmp(&b.name));
    fixtures
}

fn median(samples: &mut [u128]) -> u128 {
    samples.sort_unstable();
    samples[samples.len() / 2]
}

fn measure_fixture(fixture: &Fixture, iters: usize) -> FixtureMetrics {
    let bytes = fixture.contents.len();
    let mut parse = Vec::with_capacity(iters);
    let mut resolve = Vec::with_capacity(iters);
    let mut typecheck = Vec::with_capacity(iters);
    let mut lower = Vec::with_capacity(iters);
    let mut total = Vec::with_capacity(iters);

    for i in 0..=iters {
        let t0 = Instant::now();
        let (modules, parse_diags) = aivi::parse_modules(&fixture.path, &fixture.contents);
        let t1 = Instant::now();
        if file_diagnostics_have_errors(&parse_diags) {
            // Skip invalid inputs: perf fixtures should be valid; treat this as "bad fixture".
            panic!("parse diagnostics for {}: {parse_diags:?}", fixture.name);
        }

        let mut diags = check_modules(&modules);
        let t2 = Instant::now();
        if file_diagnostics_have_errors(&diags) {
            panic!("resolver diagnostics for {}: {diags:?}", fixture.name);
        }

        diags.extend(check_types(&modules));
        let t3 = Instant::now();
        if file_diagnostics_have_errors(&diags) {
            panic!("typecheck diagnostics for {}: {diags:?}", fixture.name);
        }

        let hir = desugar_modules(&modules);
        let _kernel = desugar_blocks(hir);
        let t4 = Instant::now();

        if i == 0 {
            // Warmup: don't record the first iteration.
            continue;
        }
        parse.push((t1 - t0).as_nanos());
        resolve.push((t2 - t1).as_nanos());
        typecheck.push((t3 - t2).as_nanos());
        lower.push((t4 - t3).as_nanos());
        total.push((t4 - t0).as_nanos());
    }

    FixtureMetrics {
        bytes,
        parse_ns: median(&mut parse),
        resolve_ns: median(&mut resolve),
        typecheck_ns: median(&mut typecheck),
        lower_ns: median(&mut lower),
        total_ns: median(&mut total),
    }
}

fn build_report() -> PerfReport {
    let fixtures = load_fixtures();
    let mut out = BTreeMap::new();
    for fixture in fixtures {
        let iters = if fixture.contents.len() < 4 * 1024 {
            25
        } else {
            7
        };
        let metrics = measure_fixture(&fixture, iters);
        out.insert(fixture.name, metrics);
    }
    PerfReport { fixtures: out }
}

fn print_help() {
    eprintln!(
        "aivi perf\n\nUSAGE:\n  cargo run -p aivi --bin perf -- run\n  cargo run -p aivi --bin perf -- check --baseline <path> [--max-multiplier 2.0]\n"
    );
}

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(cmd) = args.next() else {
        print_help();
        std::process::exit(2);
    };

    match cmd.as_str() {
        "run" => {
            let report = build_report();
            let json = serde_json::to_string_pretty(&report).expect("report json");
            println!("{json}");
        }
        "check" => {
            let mut baseline_path: Option<PathBuf> = None;
            let mut max_multiplier: f64 = 2.0;

            while let Some(arg) = args.next() {
                match arg.as_str() {
                    "--baseline" => {
                        baseline_path = args.next().map(PathBuf::from);
                    }
                    "--max-multiplier" => {
                        let v = args.next().expect("value for --max-multiplier");
                        max_multiplier = v.parse().expect("parse --max-multiplier");
                    }
                    "-h" | "--help" => {
                        print_help();
                        return;
                    }
                    other => {
                        eprintln!("unknown arg: {other}");
                        print_help();
                        std::process::exit(2);
                    }
                }
            }

            let Some(baseline_path) = baseline_path else {
                eprintln!("missing --baseline <path>");
                print_help();
                std::process::exit(2);
            };
            let baseline_text = fs::read_to_string(&baseline_path)
                .unwrap_or_else(|err| panic!("read {}: {err}", baseline_path.display()));
            let baseline: PerfReport =
                serde_json::from_str(&baseline_text).expect("parse baseline json");

            let current = build_report();
            let mut failures = Vec::new();

            for (name, metrics) in &current.fixtures {
                let Some(base) = baseline.fixtures.get(name) else {
                    failures.push(format!("missing baseline for fixture '{name}'"));
                    continue;
                };
                let max = (base.total_ns as f64) * max_multiplier;
                if (metrics.total_ns as f64) > max {
                    failures.push(format!(
                        "{name}: total_ns {} > allowed {:.0} (baseline {} * {})",
                        metrics.total_ns, max, base.total_ns, max_multiplier
                    ));
                }
            }

            if !failures.is_empty() {
                eprintln!("perf regression detected:");
                for f in failures {
                    eprintln!("  - {f}");
                }
                std::process::exit(1);
            }
        }
        _ => {
            print_help();
            std::process::exit(2);
        }
    }
}
