// ---------------------------------------------------------------------------
// AOT runtime lifecycle
// ---------------------------------------------------------------------------

use crate::hir::HirProgram;
use crate::runtime::build_runtime_from_program;

/// Initialize an AIVI runtime context from a pre-built HirProgram.
///
/// Returns a heap-allocated `JitRuntimeCtx` pointer that must be passed to
/// all `rt_*` functions. Call `aivi_rt_destroy` when done.
///
/// # Safety
/// The caller must ensure `program_ptr` points to a valid `HirProgram`.
#[no_mangle]
pub extern "C" fn aivi_rt_init(program_ptr: *mut HirProgram) -> *mut JitRuntimeCtx {
    if program_ptr.is_null() {
        eprintln!(
            "{RT_YELLOW}warning[RT]{RT_RESET} {RT_BOLD}runtime init{RT_RESET}: null program pointer"
        );
        return std::ptr::null_mut();
    }
    let program = unsafe { Box::from_raw(program_ptr) };
    let runtime = match build_runtime_from_program(&program) {
        Ok(runtime) => runtime,
        Err(err) => {
            eprintln!(
                "{RT_YELLOW}warning[RT]{RT_RESET} {RT_BOLD}runtime init{RT_RESET}: failed to build runtime: {err}"
            );
            return std::ptr::null_mut();
        }
    };
    let ctx = unsafe { JitRuntimeCtx::from_runtime_owned(runtime) };
    Box::into_raw(Box::new(ctx))
}

/// Destroy a runtime context previously created by `aivi_rt_init`.
///
/// # Safety
/// `ctx` must be a pointer returned by `aivi_rt_init`.
#[no_mangle]
pub extern "C" fn aivi_rt_destroy(ctx: *mut JitRuntimeCtx) {
    if !ctx.is_null() {
        unsafe {
            drop(Box::from_raw(ctx));
        }
    }
}

/// Initialize a minimal AIVI runtime with only builtins (no user program).
/// Used by the AOT path where compiled functions are registered via
/// `rt_register_jit_fn`.
#[no_mangle]
pub extern "C" fn aivi_rt_init_base() -> *mut JitRuntimeCtx {
    use crate::runtime::build_runtime_base;
    let runtime = build_runtime_base();
    let ctx = unsafe { JitRuntimeCtx::from_runtime_owned(runtime) };
    Box::into_raw(Box::new(ctx))
}

/// Register an AOT/JIT-compiled function as a global in the runtime.
/// Does not overwrite existing globals (e.g. builtins).
/// `is_effect`: if non-zero, wraps the function as an `EffectValue::Thunk`.
#[no_mangle]
pub extern "C" fn rt_register_jit_fn(
    ctx: *mut JitRuntimeCtx,
    name_ptr: *const u8,
    name_len: i64,
    func_ptr: i64,
    arity: i64,
    is_effect: i64,
) {
    let name = unsafe {
        std::str::from_utf8_unchecked(std::slice::from_raw_parts(name_ptr, name_len as usize))
    };
    let runtime = unsafe { &mut *(*ctx).runtime };
    // Don't overwrite builtins
    if runtime.ctx.globals.get(name).is_some() {
        return;
    }
    if is_effect != 0 {
        // Wrap zero-arity effect blocks as EffectValue::Thunk
        let def_name = name.to_string();
        let fp = func_ptr as usize;
        let effect = Value::Effect(std::sync::Arc::new(
            crate::runtime::values::EffectValue::Thunk {
                func: std::sync::Arc::new(move |rt: &mut crate::runtime::Runtime| {
                    rt.jit_pending_error = None;
                    let ctx = unsafe { JitRuntimeCtx::from_runtime(rt) };
                    let ctx_ptr = &ctx as *const JitRuntimeCtx as usize;
                    let call_args = [ctx_ptr as i64];
                    let result_ptr = unsafe { super::compile::call_jit_function(fp, &call_args) };
                    let result = if result_ptr == 0 {
                        eprintln!(
                            "{RT_YELLOW}warning[RT]{RT_RESET} {RT_BOLD}null return{RT_RESET}: AOT effect `{}` returned a null pointer (treated as unit)",
                            def_name
                        );
                        Value::Unit
                    } else {
                        unsafe { abi::unbox_value(result_ptr as *mut Value) }
                    };
                    if let Some(err) = rt.jit_pending_error.take() {
                        return Err(err);
                    }
                    Ok(result)
                }),
            },
        ));
        runtime.ctx.globals.set(name.to_string(), effect);
    } else {
        let builtin = super::compile::make_jit_builtin(name, arity as usize, func_ptr as usize);
        runtime.ctx.globals.set(name.to_string(), builtin);
    }
}

/// Evaluate a sigil literal into the correct runtime value.
/// Dispatches to the shared `eval_sigil_literal` function from the runtime.
#[no_mangle]
pub extern "C" fn rt_eval_sigil(
    ctx: *mut JitRuntimeCtx,
    tag_ptr: *const u8,
    tag_len: i64,
    body_ptr: *const u8,
    body_len: i64,
    flags_ptr: *const u8,
    flags_len: i64,
) -> *mut Value {
    let tag = unsafe {
        std::str::from_utf8_unchecked(std::slice::from_raw_parts(tag_ptr, tag_len as usize))
    };
    let body = unsafe {
        std::str::from_utf8_unchecked(std::slice::from_raw_parts(body_ptr, body_len as usize))
    };
    let flags = unsafe {
        std::str::from_utf8_unchecked(std::slice::from_raw_parts(flags_ptr, flags_len as usize))
    };
    match crate::runtime::eval_sigil_literal(tag, body, flags) {
        Ok(val) => abi::box_value(val),
        Err(e) => {
            unsafe { set_pending_error(ctx, e) };
            abi::box_value(Value::Unit)
        }
    }
}

// ---------------------------------------------------------------------------
// Snapshot mock helpers
// ---------------------------------------------------------------------------

/// Install a snapshot mock for a global binding.
///
/// - **Recording mode** (`--update-snapshots`): wraps the real function in a
///   proxy that records every Effect result to `runtime.snapshot_recordings`.
/// - **Replay mode** (default): loads recorded values from the snapshot file
///   on disk and installs a function that returns them in order.
///
/// Returns a pointer to the **old** global value (for later restoration).
///
/// # Safety
/// `ctx` must be a valid `JitRuntimeCtx` pointer.
#[no_mangle]
pub extern "C" fn rt_snapshot_mock_install(
    ctx: *mut JitRuntimeCtx,
    path_ptr: *const u8,
    path_len: usize,
) -> *mut Value {
    use crate::runtime::snapshot;
    use crate::runtime::values::{BuiltinImpl, BuiltinValue, EffectValue};

    let path =
        unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(path_ptr, path_len)) };
    let runtime = unsafe { (*ctx).runtime_mut() };

    // Save old value
    let old_val = match runtime.ctx.globals.get(path) {
        Some(v) => match runtime.force_value(v) {
            Ok(forced) => forced,
            Err(e) => {
                unsafe { set_pending_error(ctx, e) };
                return abi::box_value(Value::Unit);
            }
        },
        None => Value::Unit,
    };

    let path_owned = path.to_string();

    if runtime.update_snapshots {
        // --- Recording mode ---
        runtime
            .snapshot_recordings
            .entry(path_owned.clone())
            .or_default();

        let original = old_val.clone();
        let rec_path = path_owned.clone();
        let wrapper = Value::Builtin(BuiltinValue {
            imp: Arc::new(BuiltinImpl {
                name: format!("__snapshot_record_{rec_path}"),
                arity: 1,
                func: Arc::new(move |args, rt| {
                    let result = rt.apply(original.clone(), args[0].clone())?;
                    match result {
                        Value::Effect(eff) => {
                            let rp = rec_path.clone();
                            Ok(Value::Effect(Arc::new(EffectValue::Thunk {
                                func: Arc::new(move |rt2| {
                                    let val = rt2.run_effect_value(Value::Effect(eff.clone()))?;
                                    let json = snapshot::value_to_snapshot_json(&val)?;
                                    rt2.snapshot_recordings
                                        .entry(rp.clone())
                                        .or_default()
                                        .push(json.to_string());
                                    Ok(val)
                                }),
                            })))
                        }
                        other => {
                            let json = snapshot::value_to_snapshot_json(&other)?;
                            rt.snapshot_recordings
                                .entry(rec_path.clone())
                                .or_default()
                                .push(json.to_string());
                            Ok(other)
                        }
                    }
                }),
            }),
            args: vec![],
            tagged_args: None,
        });

        runtime.ctx.globals.set(path_owned, wrapper);
    } else {
        // --- Replay mode ---
        let test_name = runtime.current_test_name.clone().unwrap_or_default();
        let project_root = runtime.project_root.clone();

        let Some(root) = project_root else {
            unsafe {
                set_pending_error(
                    ctx,
                    RuntimeError::Message("snapshot: project root not set".to_string()),
                )
            };
            return abi::box_value(old_val);
        };

        let snap_dir = snapshot::snapshot_dir(&root, &test_name);
        let snap_path = snap_dir.join(format!("{}.snap", path_owned.replace('.', "_")));

        if !snap_path.exists() {
            unsafe {
                set_pending_error(
                    ctx,
                    RuntimeError::Message(format!(
                        "snapshot file not found: {} — run with --update-snapshots to create it",
                        snap_path.display()
                    )),
                )
            };
            return abi::box_value(old_val);
        }

        let contents = match std::fs::read_to_string(&snap_path) {
            Ok(c) => c,
            Err(e) => {
                unsafe {
                    set_pending_error(
                        ctx,
                        RuntimeError::Message(format!(
                            "snapshot: failed to read {}: {e}",
                            snap_path.display()
                        )),
                    )
                };
                return abi::box_value(old_val);
            }
        };

        let entries: Vec<serde_json::Value> = match serde_json::from_str(&contents) {
            Ok(v) => v,
            Err(e) => {
                unsafe {
                    set_pending_error(
                        ctx,
                        RuntimeError::Message(format!(
                            "snapshot: failed to parse {}: {e}",
                            snap_path.display()
                        )),
                    )
                };
                return abi::box_value(old_val);
            }
        };

        let values: Result<Vec<Value>, _> = entries
            .iter()
            .map(snapshot::snapshot_json_to_value)
            .collect();
        let values = match values {
            Ok(v) => v,
            Err(e) => {
                unsafe { set_pending_error(ctx, e) };
                return abi::box_value(old_val);
            }
        };

        let replay_values = Arc::new(std::sync::Mutex::new(values));
        let rp = path_owned.clone();
        let wrapper = Value::Builtin(BuiltinValue {
            imp: Arc::new(BuiltinImpl {
                name: format!("__snapshot_replay_{rp}"),
                arity: 1,
                func: Arc::new(move |_args, _rt| {
                    let mut vals = replay_values.lock().unwrap();
                    if vals.is_empty() {
                        return Err(RuntimeError::Message(format!(
                            "snapshot replay exhausted for `{rp}` — run with --update-snapshots to re-record"
                        )));
                    }
                    let val = vals.remove(0);
                    Ok(Value::Effect(Arc::new(EffectValue::Thunk {
                        func: Arc::new(move |_| Ok(val.clone())),
                    })))
                }),
            }),
            args: vec![],
            tagged_args: None,
        });

        runtime.ctx.globals.set(path_owned, wrapper);
    }

    abi::box_value(old_val)
}

/// Flush snapshot recordings to disk for a mock binding.
/// Only writes in recording mode; no-op in replay mode.
///
/// # Safety
/// `ctx` must be a valid `JitRuntimeCtx` pointer.
#[no_mangle]
pub extern "C" fn rt_snapshot_mock_flush(
    ctx: *mut JitRuntimeCtx,
    path_ptr: *const u8,
    path_len: usize,
) {
    use crate::runtime::snapshot;

    let path =
        unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(path_ptr, path_len)) };
    let runtime = unsafe { (*ctx).runtime_mut() };

    if !runtime.update_snapshots {
        return;
    }

    let test_name = runtime.current_test_name.clone().unwrap_or_default();
    let project_root = runtime.project_root.clone();

    let Some(root) = project_root else {
        return;
    };

    if let Some(recordings) = runtime.snapshot_recordings.remove(path) {
        let snap_dir = snapshot::snapshot_dir(&root, &test_name);
        let snap_path = snap_dir.join(format!("{}.snap", path.replace('.', "_")));

        if let Err(e) = std::fs::create_dir_all(&snap_dir) {
            unsafe {
                set_pending_error(
                    ctx,
                    RuntimeError::Message(format!(
                        "snapshot: failed to create directory {}: {e}",
                        snap_dir.display()
                    )),
                )
            };
            return;
        }

        let json_entries: Vec<serde_json::Value> = recordings
            .iter()
            .filter_map(|s| serde_json::from_str(s).ok())
            .collect();
        let json_str = serde_json::to_string_pretty(&json_entries).unwrap_or_default();

        if let Err(e) = std::fs::write(&snap_path, json_str) {
            unsafe {
                set_pending_error(
                    ctx,
                    RuntimeError::Message(format!(
                        "snapshot: failed to write {}: {e}",
                        snap_path.display()
                    )),
                )
            };
        }
    }
}

// ---------------------------------------------------------------------------
// Symbol table for JITBuilder registration
// ---------------------------------------------------------------------------

/// All runtime helper symbols that need to be registered with the Cranelift
/// `JITBuilder` so JIT-compiled code can call them.
///
/// Returns pairs of `(name, fn_pointer_as_usize)`.
pub(crate) fn runtime_helper_symbols() -> Vec<(&'static str, *const u8)> {
    vec![
        // Call-depth guard
        ("rt_check_call_depth", rt_check_call_depth as *const u8),
        ("rt_dec_call_depth", rt_dec_call_depth as *const u8),
        // Match failure signaling
        ("rt_signal_match_fail", rt_signal_match_fail as *const u8),
        ("rt_box_int", rt_box_int as *const u8),
        ("rt_box_float", rt_box_float as *const u8),
        ("rt_box_bool", rt_box_bool as *const u8),
        ("rt_unbox_int", rt_unbox_int as *const u8),
        ("rt_unbox_float", rt_unbox_float as *const u8),
        ("rt_unbox_bool", rt_unbox_bool as *const u8),
        ("rt_alloc_unit", rt_alloc_unit as *const u8),
        ("rt_alloc_string", rt_alloc_string as *const u8),
        ("rt_alloc_datetime", rt_alloc_datetime as *const u8),
        ("rt_alloc_list", rt_alloc_list as *const u8),
        ("rt_alloc_tuple", rt_alloc_tuple as *const u8),
        ("rt_alloc_record", rt_alloc_record as *const u8),
        ("rt_alloc_constructor", rt_alloc_constructor as *const u8),
        ("rt_record_field", rt_record_field as *const u8),
        ("rt_list_index", rt_list_index as *const u8),
        ("rt_clone_value", rt_clone_value as *const u8),
        ("rt_drop_value", rt_drop_value as *const u8),
        // Perceus reuse helpers
        ("rt_try_reuse", rt_try_reuse as *const u8),
        ("rt_reuse_constructor", rt_reuse_constructor as *const u8),
        ("rt_reuse_record", rt_reuse_record as *const u8),
        ("rt_reuse_list", rt_reuse_list as *const u8),
        ("rt_reuse_tuple", rt_reuse_tuple as *const u8),
        ("rt_set_global", rt_set_global as *const u8),
        ("rt_get_global", rt_get_global as *const u8),
        ("rt_apply", rt_apply as *const u8),
        ("rt_force_thunk", rt_force_thunk as *const u8),
        ("rt_run_effect", rt_run_effect as *const u8),
        ("rt_bind_effect", rt_bind_effect as *const u8),
        ("rt_wrap_effect", rt_wrap_effect as *const u8),
        (
            "rt_push_resource_scope",
            rt_push_resource_scope as *const u8,
        ),
        ("rt_pop_resource_scope", rt_pop_resource_scope as *const u8),
        ("rt_binary_op", rt_binary_op as *const u8),
        // Pattern matching helpers
        (
            "rt_constructor_name_eq",
            rt_constructor_name_eq as *const u8,
        ),
        ("rt_constructor_arity", rt_constructor_arity as *const u8),
        ("rt_constructor_arg", rt_constructor_arg as *const u8),
        ("rt_tuple_len", rt_tuple_len as *const u8),
        ("rt_tuple_item", rt_tuple_item as *const u8),
        ("rt_list_len", rt_list_len as *const u8),
        ("rt_list_tail", rt_list_tail as *const u8),
        ("rt_list_concat", rt_list_concat as *const u8),
        ("rt_value_equals", rt_value_equals as *const u8),
        // Record patching
        ("rt_patch_record", rt_patch_record as *const u8),
        (
            "rt_patch_record_inplace",
            rt_patch_record_inplace as *const u8,
        ),
        // Closure creation
        ("rt_make_closure", rt_make_closure as *const u8),
        // Native generate helpers
        ("rt_generator_to_list", rt_generator_to_list as *const u8),
        ("rt_gen_vec_new", rt_gen_vec_new as *const u8),
        ("rt_gen_vec_push", rt_gen_vec_push as *const u8),
        (
            "rt_gen_vec_extend_generator",
            rt_gen_vec_extend_generator as *const u8,
        ),
        (
            "rt_gen_vec_into_generator",
            rt_gen_vec_into_generator as *const u8,
        ),
        // AOT function registration
        ("rt_register_jit_fn", rt_register_jit_fn as *const u8),
        // Sigil evaluation
        ("rt_eval_sigil", rt_eval_sigil as *const u8),
        // Function entry tracking for diagnostics
        ("rt_enter_fn", rt_enter_fn as *const u8),
        ("rt_leave_fn", rt_leave_fn as *const u8),
        // Source location tracking for diagnostics
        ("rt_set_location", rt_set_location as *const u8),
        (
            "rt_prepare_call_location",
            rt_prepare_call_location as *const u8,
        ),
        // Snapshot mock helpers
        (
            "rt_snapshot_mock_install",
            rt_snapshot_mock_install as *const u8,
        ),
        (
            "rt_snapshot_mock_flush",
            rt_snapshot_mock_flush as *const u8,
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression: no interpreter-delegation helpers should be registered.
    /// These were removed during the Cranelift migration.
    #[test]
    fn no_interpreter_delegation_symbols() {
        let forbidden = [
            "rt_env_new",
            "rt_env_set",
            "rt_eval_generate",
            "rt_make_resource",
        ];
        let symbols = runtime_helper_symbols();
        for (name, _) in &symbols {
            assert!(
                !forbidden.contains(name),
                "interpreter delegation helper '{name}' should not be in Cranelift symbol table"
            );
        }
    }

    /// Guard that `build_runtime_base()` (used by AOT) registers every global that
    /// `build_runtime_from_program()` (used by JIT) registers for a minimal program.
    ///
    /// If someone adds something to `build_runtime_from_program` that is missing from
    /// `build_runtime_base`, this test will name the missing globals so they can be
    /// backfilled into `build_runtime_base` (or its AOT equivalent).
    #[test]
    fn aot_base_runtime_globals_subset_of_jit_program_runtime() {
        use crate::hir::{HirDef, HirExpr, HirModule, HirProgram};
        use crate::runtime::{build_runtime_base, build_runtime_from_program};

        // Minimal program: one module, one dummy def — just enough to pass the
        // "no modules" guard inside build_runtime_from_program.
        let program = HirProgram {
            modules: vec![HirModule {
                name: "test".to_string(),
                defs: vec![HirDef {
                    name: "main".to_string(),
                    expr: HirExpr::LitBool { id: 0, value: true },
                }],
            }],
        };

        let jit_runtime =
            build_runtime_from_program(&program).expect("build_runtime_from_program failed");
        let aot_runtime = build_runtime_base();

        let jit_keys: std::collections::HashSet<String> =
            jit_runtime.ctx.globals.keys().into_iter().collect();
        let aot_keys: std::collections::HashSet<String> =
            aot_runtime.ctx.globals.keys().into_iter().collect();

        let missing_from_aot: Vec<&String> = jit_keys
            .iter()
            .filter(|k| !aot_keys.contains(*k))
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();

        assert!(
            missing_from_aot.is_empty(),
            "build_runtime_base() is missing globals that build_runtime_from_program() registers.\n\
             These will be absent in AOT binaries: {missing_from_aot:?}\n\
             Add them to build_runtime_base().",
        );
    }
}
