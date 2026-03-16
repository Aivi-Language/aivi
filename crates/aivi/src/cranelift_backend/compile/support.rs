fn record_field_supported(field: &RustIrRecordField) -> bool {
    if field.spread {
        return false;
    }
    if field.path.len() != 1 {
        return false;
    }
    matches!(field.path[0], RustIrPathSegment::Field(_)) && expr_supported(&field.value)
}

fn list_item_supported(item: &RustIrListItem) -> bool {
    expr_supported(&item.expr)
}

fn pattern_supported(pattern: &RustIrPattern) -> bool {
    match pattern {
        RustIrPattern::Wildcard { .. }
        | RustIrPattern::Var { .. }
        | RustIrPattern::Literal { .. } => true,
        RustIrPattern::At { pattern, .. } => pattern_supported(pattern),
        RustIrPattern::Constructor { args, .. } => args.iter().all(pattern_supported),
        RustIrPattern::Tuple { items, .. } => items.iter().all(pattern_supported),
        RustIrPattern::List { items, rest, .. } => {
            items.iter().all(pattern_supported) && rest.as_deref().is_none_or(pattern_supported)
        }
        RustIrPattern::Record { fields, .. } => {
            fields.iter().all(|f| pattern_supported(&f.pattern))
        }
    }
}

fn expr_supported(expr: &RustIrExpr) -> bool {
    match expr {
        RustIrExpr::Local { .. }
        | RustIrExpr::Global { .. }
        | RustIrExpr::Builtin { .. }
        | RustIrExpr::ConstructorValue { .. }
        | RustIrExpr::LitString { .. }
        | RustIrExpr::LitBool { .. }
        | RustIrExpr::Raw { .. } => true,

        RustIrExpr::Mock {
            substitutions,
            body,
            ..
        } => {
            substitutions
                .iter()
                .all(|s| s.value.as_ref().is_none_or(expr_supported))
                && expr_supported(body)
        }

        RustIrExpr::LitNumber { text, .. } => {
            text.parse::<i64>().is_ok() || text.parse::<f64>().is_ok()
        }

        // These have non-trivial runtime semantics we haven't matched yet.
        RustIrExpr::LitSigil { .. }
        | RustIrExpr::LitDateTime { .. }
        | RustIrExpr::TextInterpolate { .. }
        | RustIrExpr::DebugFn { .. }
        | RustIrExpr::Pipe { .. } => true,

        RustIrExpr::Match {
            scrutinee, arms, ..
        } => {
            expr_supported(scrutinee)
                && arms.iter().all(|arm| {
                    pattern_supported(&arm.pattern)
                        && arm.guard.as_ref().is_none_or(expr_supported)
                        && expr_supported(&arm.body)
                })
        }

        RustIrExpr::Patch { target, fields, .. } => {
            expr_supported(target) && fields.iter().all(record_field_supported)
        }

        RustIrExpr::Lambda { body, .. } => expr_supported(body),

        RustIrExpr::App { func, arg, .. } => expr_supported(func) && expr_supported(arg),
        RustIrExpr::Call { func, args, .. } => {
            expr_supported(func) && args.iter().all(expr_supported)
        }

        RustIrExpr::List { items, .. } => items.iter().all(list_item_supported),
        RustIrExpr::Tuple { items, .. } => items.iter().all(expr_supported),
        RustIrExpr::Record { fields, .. } => fields.iter().all(record_field_supported),

        RustIrExpr::FieldAccess { base, .. } => expr_supported(base),
        RustIrExpr::Index { base, index, .. } => expr_supported(base) && expr_supported(index),

        RustIrExpr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => expr_supported(cond) && expr_supported(then_branch) && expr_supported(else_branch),

        RustIrExpr::Binary { left, right, .. } => expr_supported(left) && expr_supported(right),
    }
}

/// Peel Lambda wrappers to extract parameter names and the innermost body.
fn peel_params(expr: &RustIrExpr) -> (Vec<String>, &RustIrExpr) {
    let mut params = Vec::new();
    let mut cursor = expr;
    loop {
        match cursor {
            RustIrExpr::Lambda { param, body, .. } => {
                params.push(param.clone());
                cursor = body.as_ref();
            }
            _ => return (params, cursor),
        }
    }
}

fn is_trivial_self_alias_def(def: &RustIrDef) -> bool {
    let (params, body) = peel_params(&def.expr);
    params.is_empty()
        && matches!(
            body,
            RustIrExpr::Global { name, .. } if name == &def.name
        )
}

fn duplicate_trivial_self_alias_qualifieds(
    modules: &[crate::rust_ir::RustIrModule],
) -> HashSet<String> {
    let mut has_non_trivial = HashSet::new();
    let mut has_trivial_self_alias = HashSet::new();

    for module in modules {
        let module_dot = format!("{}.", module.name);
        for def in &module.defs {
            if def.name.starts_with(&module_dot) {
                continue;
            }
            let qualified = format!("{}.{}", module.name, def.name);
            if is_trivial_self_alias_def(def) {
                has_trivial_self_alias.insert(qualified);
            } else {
                has_non_trivial.insert(qualified);
            }
        }
    }

    has_trivial_self_alias
        .into_iter()
        .filter(|qualified| has_non_trivial.contains(qualified))
        .collect()
}

/// Collect all global names referenced in an expression (shallow, no dedup).
fn collect_called_globals(expr: &RustIrExpr, out: &mut HashSet<String>) {
    match expr {
        RustIrExpr::Global { name, .. } => {
            out.insert(name.clone());
        }
        RustIrExpr::App { func, arg, .. } => {
            collect_called_globals(func, out);
            collect_called_globals(arg, out);
        }
        RustIrExpr::Call { func, args, .. } => {
            collect_called_globals(func, out);
            for a in args {
                collect_called_globals(a, out);
            }
        }
        RustIrExpr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            collect_called_globals(cond, out);
            collect_called_globals(then_branch, out);
            collect_called_globals(else_branch, out);
        }
        RustIrExpr::Binary { left, right, .. } => {
            collect_called_globals(left, out);
            collect_called_globals(right, out);
        }
        RustIrExpr::Match {
            scrutinee, arms, ..
        } => {
            collect_called_globals(scrutinee, out);
            for arm in arms {
                collect_called_globals(&arm.body, out);
                if let Some(g) = &arm.guard {
                    collect_called_globals(g, out);
                }
            }
        }
        RustIrExpr::Lambda { body, .. } => collect_called_globals(body, out),
        RustIrExpr::List { items, .. } => {
            for item in items {
                collect_called_globals(&item.expr, out);
            }
        }
        RustIrExpr::Record { fields, .. } => {
            for f in fields {
                collect_called_globals(&f.value, out);
            }
        }
        RustIrExpr::Tuple { items, .. } => {
            for item in items {
                collect_called_globals(item, out);
            }
        }
        RustIrExpr::Patch { target, fields, .. } => {
            collect_called_globals(target, out);
            for f in fields {
                collect_called_globals(&f.value, out);
            }
        }
        RustIrExpr::FieldAccess { base, .. } => collect_called_globals(base, out),
        RustIrExpr::Pipe { func, arg, .. } => {
            collect_called_globals(func, out);
            collect_called_globals(arg, out);
        }
        RustIrExpr::TextInterpolate { parts, .. } => {
            for part in parts {
                if let RustIrTextPart::Expr { expr } = part {
                    collect_called_globals(expr, out);
                }
            }
        }
        RustIrExpr::DebugFn { body, .. } => collect_called_globals(body, out),
        _ => {}
    }
}

/// Build a human-readable suffix for a CgType, used for specialization naming.
fn cg_type_suffix(ty: &CgType) -> String {
    match ty {
        CgType::Int => "Int".into(),
        CgType::Float => "Float".into(),
        CgType::Bool => "Bool".into(),
        CgType::Text => "Text".into(),
        CgType::Unit => "Unit".into(),
        CgType::DateTime => "DateTime".into(),
        CgType::Func(a, b) => format!("{}_to_{}", cg_type_suffix(a), cg_type_suffix(b)),
        CgType::ListOf(elem) => format!("List_{}", cg_type_suffix(elem)),
        CgType::Tuple(items) => {
            let parts: Vec<_> = items.iter().map(cg_type_suffix).collect();
            format!("Tup_{}", parts.join("_"))
        }
        _ => {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            format!("{:?}", ty).hash(&mut hasher);
            format!("h{:x}", hasher.finish())
        }
    }
}

/// Monomorphize polymorphic definitions based on the monomorph plan.
///
/// Returns a `spec_map` mapping original short names to their specialization
/// short names (for call-site routing in the lowering phase).
fn monomorphize_program(
    modules: &mut [rust_ir::RustIrModule],
    monomorph_plan: &HashMap<String, Vec<CgType>>,
) -> HashMap<String, Vec<String>> {
    let mut spec_map: HashMap<String, Vec<String>> = HashMap::new();

    for module in modules.iter_mut() {
        let mut new_defs = Vec::new();
        let mut single_type_updates: Vec<(String, CgType)> = Vec::new();

        for def in module.defs.iter() {
            // Skip defs that already have a concrete type
            if def.cg_type.as_ref().is_some_and(|t| t.is_closed()) {
                continue;
            }
            let qualified = format!("{}.{}", module.name, def.name);
            let Some(instantiations) = monomorph_plan.get(&qualified) else {
                continue;
            };
            if instantiations.is_empty() {
                continue;
            }

            if instantiations.len() == 1 {
                // Single instantiation: set cg_type on the original def directly.
                single_type_updates.push((def.name.clone(), instantiations[0].clone()));
            }

            // Create specialized clones for each concrete type.
            for concrete_type in instantiations {
                let suffix = cg_type_suffix(concrete_type);
                let spec_name = format!("{}$mono_{}", def.name, suffix);
                new_defs.push(RustIrDef {
                    name: spec_name.clone(),
                    expr: def.expr.clone(),
                    cg_type: Some(concrete_type.clone()),
                });
                spec_map
                    .entry(def.name.clone())
                    .or_default()
                    .push(spec_name);
            }
        }

        // Apply single-instantiation type updates
        for (name, cg_type) in single_type_updates {
            if let Some(def) = module.defs.iter_mut().find(|d| d.name == name) {
                def.cg_type = Some(cg_type);
            }
        }

        module.defs.extend(new_defs);
    }

    spec_map
}

fn sanitize_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for ch in name.chars() {
        match ch {
            _ if ch.is_ascii_alphanumeric() => out.push(ch),
            '.' => out.push('_'),
            '+' => out.push_str("_plus_"),
            '-' => out.push_str("_minus_"),
            '*' => out.push_str("_star_"),
            '/' => out.push_str("_slash_"),
            '<' => out.push_str("_lt_"),
            '>' => out.push_str("_gt_"),
            '=' => out.push_str("_eq_"),
            '!' => out.push_str("_bang_"),
            '&' => out.push_str("_amp_"),
            '|' => out.push_str("_pipe_"),
            '^' => out.push_str("_caret_"),
            '%' => out.push_str("_pct_"),
            '~' => out.push_str("_tilde_"),
            _ => out.push_str(&format!("_x{:02x}_", ch as u32)),
        }
    }
    out
}

/// Create a runtime `Value::Builtin` that calls a JIT-compiled function.
pub(crate) fn make_jit_builtin(def_name: &str, arity: usize, func_ptr: usize) -> Value {
    use crate::runtime::values::{BuiltinImpl, BuiltinValue};
    use std::sync::Arc;

    let def_name = def_name.to_string();

    // For arity-0 defs, we call the JIT function immediately and cache the result
    if arity == 0 {
        // Arity-0 non-effect definitions can be eagerly evaluated
        let builtin = Value::Builtin(BuiltinValue {
            imp: Arc::new(BuiltinImpl {
                name: format!("__jit|cranelift|{}", def_name),
                arity: 0,
                func: Arc::new(move |_args: Vec<Value>, runtime: &mut Runtime| {
                    runtime.jit_pending_call_loc = runtime.jit_current_loc.clone();
                    let ctx = unsafe { JitRuntimeCtx::from_runtime(runtime) };
                    let ctx_ptr = &ctx as *const JitRuntimeCtx as usize;
                    let call_args = [ctx_ptr as i64];
                    let result_ptr = unsafe { call_jit_function(func_ptr, &call_args) };
                    let result = if result_ptr == 0 {
                        eprintln!("aivi: JIT function '{}' returned null pointer", def_name);
                        Value::Unit
                    } else {
                        unsafe { super::abi::unbox_value(result_ptr as *mut Value) }
                    };
                    if let Some(err) = runtime.jit_pending_error.take() {
                        let snapshot = runtime.take_snapshot_for_error(&err);
                        Err(crate::runtime::wrap_runtime_error_with_snapshot(
                            err, snapshot,
                        ))
                    } else {
                        Ok(result)
                    }
                }),
            }),
            args: Vec::new(),
            tagged_args: None,
        });
        return builtin;
    }

    // The builtin accumulates args until arity is reached, then calls the JIT code
    Value::Builtin(BuiltinValue {
        imp: Arc::new(BuiltinImpl {
            name: format!("__jit|cranelift|{}", def_name),
            arity,
            func: Arc::new(move |args: Vec<Value>, runtime: &mut Runtime| {
                // Clear any stale pending error before entering JIT code
                runtime.clear_pending_runtime_error();
                runtime.clear_match_failure();
                runtime.jit_pending_call_loc = runtime.jit_current_loc.clone();

                // Construct JitRuntimeCtx and call the compiled function
                let ctx = unsafe { JitRuntimeCtx::from_runtime(runtime) };
                let ctx_ptr = &ctx as *const JitRuntimeCtx as usize;

                // Box all arguments
                let boxed_args: Vec<*mut Value> =
                    args.into_iter().map(super::abi::box_value).collect();

                // Build call arguments: [ctx_ptr, arg0, arg1, ...]
                let mut call_args: Vec<i64> = Vec::with_capacity(1 + arity);
                call_args.push(ctx_ptr as i64);
                for arg in &boxed_args {
                    call_args.push(*arg as i64);
                }

                // Call the JIT function
                let result_ptr = unsafe { call_jit_function(func_ptr, &call_args) };

                // Check if the JIT function signalled a non-exhaustive match.
                // This lets apply_multi_clause try the next clause.
                if runtime.jit_match_failed {
                    let err = RuntimeError::NonExhaustiveMatch { scrutinee: None };
                    let snapshot = runtime.take_snapshot_for_error(&err);
                    runtime.jit_match_failed = false;
                    runtime.clear_pending_runtime_error();
                    // Clean up boxed arguments
                    for arg_ptr in boxed_args {
                        unsafe {
                            drop(Box::from_raw(arg_ptr));
                        }
                    }
                    if result_ptr != 0 && !call_args[1..].contains(&result_ptr) {
                        unsafe {
                            drop(Box::from_raw(result_ptr as *mut Value));
                        }
                    }
                    return Err(crate::runtime::wrap_runtime_error_with_snapshot(
                        err, snapshot,
                    ));
                }

                // Clone the result from the pointer (don't take ownership — the
                // pointer might alias one of the input args).
                let result = if result_ptr == 0 {
                    eprintln!("aivi: JIT function '{}' returned null pointer", def_name);
                    Value::Unit
                } else {
                    let rp = result_ptr as *const Value;
                    unsafe { (*rp).clone() }
                };

                // Drop all boxed arguments. Since we cloned the result above,
                // we won't double-free even if result_ptr == one of the arg ptrs.
                for arg_ptr in boxed_args {
                    unsafe {
                        drop(Box::from_raw(arg_ptr));
                    }
                }

                // If the result_ptr is distinct from all arg_ptrs, drop it too.
                if result_ptr != 0 && !call_args[1..].contains(&result_ptr) {
                    unsafe {
                        drop(Box::from_raw(result_ptr as *mut Value));
                    }
                }

                // Propagate any error that occurred inside JIT code
                if let Some(err) = runtime.jit_pending_error.take() {
                    let snapshot = runtime.take_snapshot_for_error(&err);
                    return Err(crate::runtime::wrap_runtime_error_with_snapshot(
                        err, snapshot,
                    ));
                }

                Ok(result)
            }),
        }),
        args: Vec::new(),
        tagged_args: None,
    })
}

/// Call a JIT-compiled function at the given address with the given arguments.
///
/// # Safety
/// `func_ptr` must point to valid JIT-compiled code with the matching signature.
#[allow(clippy::type_complexity)]
pub(crate) unsafe fn call_jit_function(func_ptr: usize, args: &[i64]) -> i64 {
    let code = func_ptr as *const u8;
    match args.len() {
        1 => {
            let f: extern "C" fn(i64) -> i64 = std::mem::transmute(code);
            f(args[0])
        }
        2 => {
            let f: extern "C" fn(i64, i64) -> i64 = std::mem::transmute(code);
            f(args[0], args[1])
        }
        3 => {
            let f: extern "C" fn(i64, i64, i64) -> i64 = std::mem::transmute(code);
            f(args[0], args[1], args[2])
        }
        4 => {
            let f: extern "C" fn(i64, i64, i64, i64) -> i64 = std::mem::transmute(code);
            f(args[0], args[1], args[2], args[3])
        }
        5 => {
            let f: extern "C" fn(i64, i64, i64, i64, i64) -> i64 = std::mem::transmute(code);
            f(args[0], args[1], args[2], args[3], args[4])
        }
        6 => {
            let f: extern "C" fn(i64, i64, i64, i64, i64, i64) -> i64 = std::mem::transmute(code);
            f(args[0], args[1], args[2], args[3], args[4], args[5])
        }
        7 => {
            let f: extern "C" fn(i64, i64, i64, i64, i64, i64, i64) -> i64 =
                std::mem::transmute(code);
            f(
                args[0], args[1], args[2], args[3], args[4], args[5], args[6],
            )
        }
        8 => {
            let f: extern "C" fn(i64, i64, i64, i64, i64, i64, i64, i64) -> i64 =
                std::mem::transmute(code);
            f(
                args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7],
            )
        }
        9 => {
            let f: extern "C" fn(i64, i64, i64, i64, i64, i64, i64, i64, i64) -> i64 =
                std::mem::transmute(code);
            f(
                args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8],
            )
        }
        10 => {
            let f: extern "C" fn(i64, i64, i64, i64, i64, i64, i64, i64, i64, i64) -> i64 =
                std::mem::transmute(code);
            f(
                args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8],
                args[9],
            )
        }
        11 => {
            let f: extern "C" fn(i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64) -> i64 =
                std::mem::transmute(code);
            f(
                args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8],
                args[9], args[10],
            )
        }
        12 => {
            let f: extern "C" fn(
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
            ) -> i64 = std::mem::transmute(code);
            f(
                args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8],
                args[9], args[10], args[11],
            )
        }
        13 => {
            let f: extern "C" fn(
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
            ) -> i64 = std::mem::transmute(code);
            f(
                args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8],
                args[9], args[10], args[11], args[12],
            )
        }
        14 => {
            let f: extern "C" fn(
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
            ) -> i64 = std::mem::transmute(code);
            f(
                args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8],
                args[9], args[10], args[11], args[12], args[13],
            )
        }
        15 => {
            let f: extern "C" fn(
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
            ) -> i64 = std::mem::transmute(code);
            f(
                args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8],
                args[9], args[10], args[11], args[12], args[13], args[14],
            )
        }
        16 => {
            let f: extern "C" fn(
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
            ) -> i64 = std::mem::transmute(code);
            f(
                args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8],
                args[9], args[10], args[11], args[12], args[13], args[14], args[15],
            )
        }
        17 => {
            let f: extern "C" fn(
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
            ) -> i64 = std::mem::transmute(code);
            f(
                args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8],
                args[9], args[10], args[11], args[12], args[13], args[14], args[15], args[16],
            )
        }
        18 => {
            let f: extern "C" fn(
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
            ) -> i64 = std::mem::transmute(code);
            f(
                args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8],
                args[9], args[10], args[11], args[12], args[13], args[14], args[15], args[16],
                args[17],
            )
        }
        19 => {
            let f: extern "C" fn(
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
            ) -> i64 = std::mem::transmute(code);
            f(
                args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8],
                args[9], args[10], args[11], args[12], args[13], args[14], args[15], args[16],
                args[17], args[18],
            )
        }
        20 => {
            let f: extern "C" fn(
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
            ) -> i64 = std::mem::transmute(code);
            f(
                args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8],
                args[9], args[10], args[11], args[12], args[13], args[14], args[15], args[16],
                args[17], args[18], args[19],
            )
        }
        21 => {
            let f: extern "C" fn(
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
            ) -> i64 = std::mem::transmute(code);
            f(
                args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8],
                args[9], args[10], args[11], args[12], args[13], args[14], args[15], args[16],
                args[17], args[18], args[19], args[20],
            )
        }
        22 => {
            let f: extern "C" fn(
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
            ) -> i64 = std::mem::transmute(code);
            f(
                args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8],
                args[9], args[10], args[11], args[12], args[13], args[14], args[15], args[16],
                args[17], args[18], args[19], args[20], args[21],
            )
        }
        23 => {
            let f: extern "C" fn(
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
            ) -> i64 = std::mem::transmute(code);
            f(
                args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8],
                args[9], args[10], args[11], args[12], args[13], args[14], args[15], args[16],
                args[17], args[18], args[19], args[20], args[21], args[22],
            )
        }
        24 => {
            let f: extern "C" fn(
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
            ) -> i64 = std::mem::transmute(code);
            f(
                args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8],
                args[9], args[10], args[11], args[12], args[13], args[14], args[15], args[16],
                args[17], args[18], args[19], args[20], args[21], args[22], args[23],
            )
        }
        25 => {
            let f: extern "C" fn(
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
            ) -> i64 = std::mem::transmute(code);
            f(
                args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8],
                args[9], args[10], args[11], args[12], args[13], args[14], args[15], args[16],
                args[17], args[18], args[19], args[20], args[21], args[22], args[23], args[24],
            )
        }
        26 => {
            let f: extern "C" fn(
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
            ) -> i64 = std::mem::transmute(code);
            f(
                args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8],
                args[9], args[10], args[11], args[12], args[13], args[14], args[15], args[16],
                args[17], args[18], args[19], args[20], args[21], args[22], args[23], args[24],
                args[25],
            )
        }
        27 => {
            let f: extern "C" fn(
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
            ) -> i64 = std::mem::transmute(code);
            f(
                args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8],
                args[9], args[10], args[11], args[12], args[13], args[14], args[15], args[16],
                args[17], args[18], args[19], args[20], args[21], args[22], args[23], args[24],
                args[25], args[26],
            )
        }
        28 => {
            let f: extern "C" fn(
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
            ) -> i64 = std::mem::transmute(code);
            f(
                args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8],
                args[9], args[10], args[11], args[12], args[13], args[14], args[15], args[16],
                args[17], args[18], args[19], args[20], args[21], args[22], args[23], args[24],
                args[25], args[26], args[27],
            )
        }
        29 => {
            let f: extern "C" fn(
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
            ) -> i64 = std::mem::transmute(code);
            f(
                args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8],
                args[9], args[10], args[11], args[12], args[13], args[14], args[15], args[16],
                args[17], args[18], args[19], args[20], args[21], args[22], args[23], args[24],
                args[25], args[26], args[27], args[28],
            )
        }
        30 => {
            let f: extern "C" fn(
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
            ) -> i64 = std::mem::transmute(code);
            f(
                args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8],
                args[9], args[10], args[11], args[12], args[13], args[14], args[15], args[16],
                args[17], args[18], args[19], args[20], args[21], args[22], args[23], args[24],
                args[25], args[26], args[27], args[28], args[29],
            )
        }
        31 => {
            let f: extern "C" fn(
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
            ) -> i64 = std::mem::transmute(code);
            f(
                args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8],
                args[9], args[10], args[11], args[12], args[13], args[14], args[15], args[16],
                args[17], args[18], args[19], args[20], args[21], args[22], args[23], args[24],
                args[25], args[26], args[27], args[28], args[29], args[30],
            )
        }
        32 => {
            let f: extern "C" fn(
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
            ) -> i64 = std::mem::transmute(code);
            f(
                args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8],
                args[9], args[10], args[11], args[12], args[13], args[14], args[15], args[16],
                args[17], args[18], args[19], args[20], args[21], args[22], args[23], args[24],
                args[25], args[26], args[27], args[28], args[29], args[30], args[31],
            )
        }
        33 => {
            let f: extern "C" fn(
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
                i64,
            ) -> i64 = std::mem::transmute(code);
            f(
                args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8],
                args[9], args[10], args[11], args[12], args[13], args[14], args[15], args[16],
                args[17], args[18], args[19], args[20], args[21], args[22], args[23], args[24],
                args[25], args[26], args[27], args[28], args[29], args[30], args[31], args[32],
            )
        }
        n => {
            eprintln!(
                "aivi: call_jit_function: unsupported arity {n} (max {} params + ctx)",
                MAX_JIT_ARITY
            );
            0
        }
    }
}
