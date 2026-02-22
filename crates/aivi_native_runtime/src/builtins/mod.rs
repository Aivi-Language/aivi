mod calendar;
mod collections;
mod color;
mod concurrency;
mod core;
mod crypto;
mod database;
mod graph;
mod gtk4;
mod gtk4_real;
mod http_server;
mod i18n;
mod instant;
mod linalg;
mod list;
mod log;
mod math;
mod mutable_map;
mod number;
mod regex;
mod signal;
mod sockets;
mod streams;
mod system;
mod text;
mod ui;
mod url_http;
mod util;

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::values::Value;

pub fn get_builtin(name: &str) -> Option<Value> {
    BUILTINS.get_or_init(build_all).get(name).cloned()
}

static BUILTINS: OnceLock<HashMap<String, Value>> = OnceLock::new();

fn build_all() -> HashMap<String, Value> {
    let mut env = HashMap::new();
    core::register_builtins(&mut env);
    env
}
