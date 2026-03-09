use std::collections::HashMap;
use std::sync::Arc;

use super::util::builtin;
use crate::runtime::Value;

pub(super) fn build_reactive_record() -> Value {
    let mut fields = HashMap::new();
    fields.insert(
        "signal".to_string(),
        builtin("reactive.signal", 1, |mut args, runtime| {
            runtime.reactive_create_signal(args.remove(0))
        }),
    );
    fields.insert(
        "get".to_string(),
        builtin("reactive.get", 1, |mut args, runtime| {
            runtime.reactive_get_signal(args.remove(0))
        }),
    );
    fields.insert(
        "peek".to_string(),
        builtin("reactive.peek", 1, |mut args, runtime| {
            runtime.reactive_peek_signal(args.remove(0))
        }),
    );
    fields.insert(
        "set".to_string(),
        builtin("reactive.set", 2, |mut args, runtime| {
            let value = args.pop().unwrap();
            let signal = args.pop().unwrap();
            runtime.reactive_set_signal(signal, value)
        }),
    );
    fields.insert(
        "update".to_string(),
        builtin("reactive.update", 2, |mut args, runtime| {
            let updater = args.pop().unwrap();
            let signal = args.pop().unwrap();
            runtime.reactive_update_signal(signal, updater)
        }),
    );
    fields.insert(
        "map".to_string(),
        builtin("reactive.map", 2, |mut args, runtime| {
            let mapper = args.pop().unwrap();
            let signal = args.pop().unwrap();
            runtime.reactive_map_signal(signal, mapper)
        }),
    );
    fields.insert(
        "combine2".to_string(),
        builtin("reactive.combine2", 3, |mut args, runtime| {
            let combine = args.pop().unwrap();
            let right = args.pop().unwrap();
            let left = args.pop().unwrap();
            runtime.reactive_combine2_signals(left, right, combine)
        }),
    );
    fields.insert(
        "watch".to_string(),
        builtin("reactive.watch", 2, |mut args, runtime| {
            let callback = args.pop().unwrap();
            let signal = args.pop().unwrap();
            runtime.reactive_watch_signal(signal, callback)
        }),
    );
    fields.insert(
        "batch".to_string(),
        builtin("reactive.batch", 1, |mut args, runtime| {
            runtime.reactive_batch(args.remove(0))
        }),
    );
    fields.insert(
        "event".to_string(),
        builtin("reactive.event", 1, |mut args, runtime| {
            runtime.reactive_create_event(args.remove(0))
        }),
    );
    Value::Record(Arc::new(fields))
}
