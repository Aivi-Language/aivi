mod util;
mod system;
mod concurrency;
mod text;
mod regex;
mod math;
mod calendar;
mod color;
mod number;
mod url_http;
mod collections;
mod linalg;
mod signal;
mod graph;
mod core;

pub(crate) use core::register_builtins;
pub(crate) use util::builtin;

#[cfg(test)]
pub(crate) fn build_concurrent_record() -> crate::runtime::Value {
    concurrency::build_concurrent_record()
}
