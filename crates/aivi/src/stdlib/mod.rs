use std::path::PathBuf;

use crate::diagnostics::DiagnosticSeverity;
use crate::surface::{parse_modules, Module};

mod bigint;
mod calendar;
mod json;
mod map;
mod chronos_calendar;
mod chronos_duration;
mod chronos_instant;
mod chronos_timezone;
mod collections;
mod color;
mod complex;
mod concurrency;
mod console;
mod core;
mod crypto;
mod database;
mod database_pool;
mod decimal;
mod duration;
mod file;
mod generator;
mod geometry;
mod graph;
mod i18n;
mod linalg_facade;
mod linear_algebra;
mod logic;
mod math;
mod matrix;
mod mutable_map;
mod network_facade;
mod network_http;
mod network_http_server;
mod network_https;
mod network_sockets;
mod network_streams;
mod number_facade;
mod path;
mod prelude;
mod probability;
mod quaternion;
mod rational;
mod regex;
mod signal;
mod system;
mod testing;
mod text;
mod tree;
mod ui;
mod ui_layout;
mod ui_server_html;
mod units;
mod url;
mod vector;

struct EmbeddedModule {
    name: &'static str,
    source: &'static str,
}

const EMBEDDED_MODULES: &[EmbeddedModule] = &[
    EmbeddedModule {
        name: core::MODULE_NAME,
        source: core::SOURCE,
    },
    EmbeddedModule {
        name: prelude::MODULE_NAME,
        source: prelude::SOURCE,
    },
    EmbeddedModule {
        name: text::MODULE_NAME,
        source: text::SOURCE,
    },
    EmbeddedModule {
        name: collections::MODULE_NAME,
        source: collections::SOURCE,
    },
    EmbeddedModule {
        name: generator::MODULE_NAME,
        source: generator::SOURCE,
    },
    EmbeddedModule {
        name: logic::MODULE_NAME,
        source: logic::SOURCE,
    },
    EmbeddedModule {
        name: regex::MODULE_NAME,
        source: regex::SOURCE,
    },
    EmbeddedModule {
        name: testing::MODULE_NAME,
        source: testing::SOURCE,
    },
    EmbeddedModule {
        name: units::MODULE_NAME,
        source: units::SOURCE,
    },
    EmbeddedModule {
        name: calendar::MODULE_NAME,
        source: calendar::SOURCE,
    },
    EmbeddedModule {
        name: chronos_calendar::MODULE_NAME,
        source: chronos_calendar::SOURCE,
    },
    EmbeddedModule {
        name: chronos_duration::MODULE_NAME,
        source: chronos_duration::SOURCE,
    },
    EmbeddedModule {
        name: chronos_instant::MODULE_NAME,
        source: chronos_instant::SOURCE,
    },
    EmbeddedModule {
        name: chronos_timezone::MODULE_NAME,
        source: chronos_timezone::SOURCE,
    },
    EmbeddedModule {
        name: duration::MODULE_NAME,
        source: duration::SOURCE,
    },
    EmbeddedModule {
        name: color::MODULE_NAME,
        source: color::SOURCE,
    },
    EmbeddedModule {
        name: vector::MODULE_NAME,
        source: vector::SOURCE,
    },
    EmbeddedModule {
        name: matrix::MODULE_NAME,
        source: matrix::SOURCE,
    },
    EmbeddedModule {
        name: linear_algebra::MODULE_NAME,
        source: linear_algebra::SOURCE,
    },
    EmbeddedModule {
        name: linalg_facade::MODULE_NAME,
        source: linalg_facade::SOURCE,
    },
    EmbeddedModule {
        name: probability::MODULE_NAME,
        source: probability::SOURCE,
    },
    EmbeddedModule {
        name: signal::MODULE_NAME,
        source: signal::SOURCE,
    },
    EmbeddedModule {
        name: geometry::MODULE_NAME,
        source: geometry::SOURCE,
    },
    EmbeddedModule {
        name: graph::MODULE_NAME,
        source: graph::SOURCE,
    },
    EmbeddedModule {
        name: tree::MODULE_NAME,
        source: tree::SOURCE,
    },
    EmbeddedModule {
        name: math::MODULE_NAME,
        source: math::SOURCE,
    },
    EmbeddedModule {
        name: url::MODULE_NAME,
        source: url::SOURCE,
    },
    EmbeddedModule {
        name: path::MODULE_NAME,
        source: path::SOURCE,
    },
    EmbeddedModule {
        name: concurrency::MODULE_NAME,
        source: concurrency::SOURCE,
    },
    EmbeddedModule {
        name: console::MODULE_NAME,
        source: console::SOURCE,
    },
    EmbeddedModule {
        name: crypto::MODULE_NAME,
        source: crypto::SOURCE,
    },
    EmbeddedModule {
        name: system::MODULE_NAME,
        source: system::SOURCE,
    },
    EmbeddedModule {
        name: database::MODULE_NAME,
        source: database::SOURCE,
    },
    EmbeddedModule {
        name: database_pool::MODULE_NAME,
        source: database_pool::SOURCE,
    },
    EmbeddedModule {
        name: file::MODULE_NAME,
        source: file::SOURCE,
    },
    EmbeddedModule {
        name: i18n::MODULE_NAME,
        source: i18n::SOURCE,
    },
    EmbeddedModule {
        name: bigint::MODULE_NAME,
        source: bigint::SOURCE,
    },
    EmbeddedModule {
        name: rational::MODULE_NAME,
        source: rational::SOURCE,
    },
    EmbeddedModule {
        name: decimal::MODULE_NAME,
        source: decimal::SOURCE,
    },
    EmbeddedModule {
        name: complex::MODULE_NAME,
        source: complex::SOURCE,
    },
    EmbeddedModule {
        name: quaternion::MODULE_NAME,
        source: quaternion::SOURCE,
    },
    EmbeddedModule {
        name: number_facade::MODULE_NAME,
        source: number_facade::SOURCE,
    },
    EmbeddedModule {
        name: network_http::MODULE_NAME,
        source: network_http::SOURCE,
    },
    EmbeddedModule {
        name: network_https::MODULE_NAME,
        source: network_https::SOURCE,
    },
    EmbeddedModule {
        name: network_sockets::MODULE_NAME,
        source: network_sockets::SOURCE,
    },
    EmbeddedModule {
        name: network_streams::MODULE_NAME,
        source: network_streams::SOURCE,
    },
    EmbeddedModule {
        name: network_facade::MODULE_NAME,
        source: network_facade::SOURCE,
    },
    EmbeddedModule {
        name: network_http_server::MODULE_NAME,
        source: network_http_server::SOURCE,
    },
    EmbeddedModule {
        name: ui_layout::MODULE_NAME,
        source: ui_layout::SOURCE,
    },
    EmbeddedModule {
        name: ui::MODULE_NAME,
        source: ui::SOURCE,
    },
    EmbeddedModule {
        name: ui_server_html::MODULE_NAME,
        source: ui_server_html::SOURCE,
    },
    EmbeddedModule {
        name: mutable_map::MODULE_NAME,
        source: mutable_map::SOURCE,
    },
    EmbeddedModule {
        name: json::MODULE_NAME,
        source: json::SOURCE,
    },
    EmbeddedModule {
        name: map::MODULE_NAME,
        source: map::SOURCE,
    },
];

pub fn embedded_stdlib_modules() -> Vec<Module> {
    // The embedded stdlib is allowed to be incomplete / not typecheck-clean in early versions.
    // Tooling (like doc snippet verification) may want to run without it.
    if std::env::var("AIVI_NO_STDLIB").is_ok_and(|v| v == "1" || v.eq_ignore_ascii_case("true")) {
        return Vec::new();
    }

    let trace = std::env::var("AIVI_TRACE_STDLIB").is_ok_and(|v| v == "1");
    let mut modules = Vec::new();
    for module in EMBEDDED_MODULES {
        if trace {
            eprintln!(
                "[AIVI_TRACE_STDLIB] parsing {} ({} bytes)",
                module.name,
                module.source.len()
            );
        }
        modules.extend(parse_embedded(module.name, module.source));
        if trace {
            eprintln!("[AIVI_TRACE_STDLIB] parsed {}", module.name);
        }
    }
    modules
}

pub fn embedded_stdlib_source(module_name: &str) -> Option<&'static str> {
    EMBEDDED_MODULES
        .iter()
        .find(|module| module.name == module_name)
        .map(|module| module.source)
}

fn parse_embedded(name: &str, source: &str) -> Vec<Module> {
    let path = PathBuf::from(format!("<embedded:{name}>"));
    let (modules, diagnostics) = parse_modules(path.as_path(), source);
    let errors: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.diagnostic.severity == DiagnosticSeverity::Error)
        .collect();
    debug_assert!(
        errors.is_empty(),
        "embedded stdlib module {name} failed to parse: {errors:#?}"
    );
    modules
}
