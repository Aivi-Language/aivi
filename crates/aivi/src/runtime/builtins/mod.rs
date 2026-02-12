mod calendar;
mod collections;
mod color;
mod concurrency;
mod core;
mod crypto;
mod database;
mod graph;
mod i18n;
mod linalg;
mod log;
mod math;
mod number;
mod regex;
mod signal;
mod sockets;
mod streams;
mod system;
mod text;
mod url_http;
mod util;

pub(crate) use core::register_builtins;
pub(crate) use util::builtin;

#[cfg(test)]
pub(crate) fn build_concurrent_record() -> crate::runtime::Value {
    concurrency::build_concurrent_record()
}
