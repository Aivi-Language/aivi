use std::{
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

use aivi_backend::{RuntimeSumValue, RuntimeValue};
use aivi_base::SourceDatabase;
use aivi_hir::{Item, lower_module as lower_hir_module};
use aivi_lambda::lower_module as lower_lambda_module;
use aivi_runtime::{
    SourceProviderManager, assemble_hir_runtime, link_backend_runtime, providers::WindowKeyEvent,
};
use aivi_syntax::parse_module;

struct LoweredStack {
    hir: aivi_hir::LoweringResult,
    core: aivi_core::Module,
    backend: aivi_backend::Program,
}

fn lower_text(path: &str, text: &str) -> LoweredStack {
    let mut sources = SourceDatabase::new();
    let file_id = sources.add_file(path, text);
    let parsed = parse_module(&sources[file_id]);
    assert!(
        !parsed.has_errors(),
        "fixture {path} should parse: {:?}",
        parsed.all_diagnostics().collect::<Vec<_>>()
    );
    let hir = lower_hir_module(&parsed.module);
    assert!(
        !hir.has_errors(),
        "fixture {path} should lower to HIR: {:?}",
        hir.diagnostics()
    );
    let core = aivi_core::lower_module(hir.module()).expect("typed-core lowering should succeed");
    let lambda = lower_lambda_module(&core).expect("lambda lowering should succeed");
    let backend = aivi_backend::lower_module(&lambda).expect("backend lowering should succeed");
    LoweredStack { hir, core, backend }
}

fn item_id(module: &aivi_hir::Module, name: &str) -> aivi_hir::ItemId {
    module
        .items()
        .iter()
        .find_map(|(item_id, item)| match item {
            Item::Value(item) if item.name.text() == name => Some(item_id),
            Item::Function(item) if item.name.text() == name => Some(item_id),
            Item::Signal(item) if item.name.text() == name => Some(item_id),
            Item::Type(item) if item.name.text() == name => Some(item_id),
            Item::Class(item) if item.name.text() == name => Some(item_id),
            Item::Domain(item) if item.name.text() == name => Some(item_id),
            _ => None,
        })
        .unwrap_or_else(|| panic!("expected item named {name}"))
}

fn spin_until(
    linked: &mut aivi_runtime::BackendLinkedRuntime,
    signal: aivi_runtime::SignalHandle,
    timeout: Duration,
) -> Option<RuntimeValue> {
    let start = Instant::now();
    while start.elapsed() < timeout {
        linked.tick().expect("runtime tick should succeed");
        if let Some(value) = linked.runtime().current_value(signal).unwrap() {
            return Some(value.clone());
        }
        thread::sleep(Duration::from_millis(10));
    }
    None
}

#[test]
fn window_key_scan_updates_direction_signal() {
    let lowered = lower_text(
        "runtime-window-key-direction.aivi",
        r#"
type Key =
  | Key Text

type Direction =
  | Up
  | Down
  | Left
  | Right

fun arrowKey:(Option Direction) key:Key =>
    key
     ||> Key "ArrowUp"    => Some Up
     ||> Key "ArrowDown"  => Some Down
     ||> Key "ArrowLeft"  => Some Left
     ||> Key "ArrowRight" => Some Right
     ||> _                => None

fun filterDirection:Direction current:Direction opt:(Option Direction) =>
    opt
     ||> Some dir => dir
     ||> None     => current

fun updateDirection:Direction key:Key current:Direction =>
    arrowKey key
     |> filterDirection current

@source window.keyDown with {
    repeat: False
    focusOnly: True
}
sig keyDown : Signal Key

sig direction : Signal Direction =
    keyDown
     |> scan Right updateDirection
"#,
    );
    let assembly =
        assemble_hir_runtime(lowered.hir.module()).expect("runtime assembly should build");
    let mut linked = link_backend_runtime(assembly, &lowered.core, Arc::new(lowered.backend))
        .expect("startup link should succeed");
    let actions = linked
        .tick_with_source_lifecycle()
        .expect("linked runtime tick should succeed");
    let mut providers = SourceProviderManager::new();
    providers
        .apply_actions(actions.source_actions())
        .expect("window key source should execute");
    providers.dispatch_window_key_event(WindowKeyEvent {
        name: "ArrowDown".into(),
        repeated: false,
    });

    let direction_signal = linked
        .assembly()
        .signal(item_id(lowered.hir.module(), "direction"))
        .expect("direction signal binding should exist")
        .signal();
    let constructor = lowered
        .hir
        .module()
        .sum_constructor_handle(item_id(lowered.hir.module(), "Direction"), "Down")
        .expect("Direction.Down constructor should resolve");
    let value = spin_until(&mut linked, direction_signal, Duration::from_millis(200))
        .expect("window key source should update direction");
    assert_eq!(
        value,
        RuntimeValue::Sum(RuntimeSumValue {
            item: constructor.item,
            type_name: constructor.type_name.clone(),
            variant_name: constructor.variant_name.clone(),
            fields: Vec::new(),
        })
    );
}
