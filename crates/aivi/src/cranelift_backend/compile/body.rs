/// Compile the body of a pre-declared function.
///
/// The function has already been declared via `module.declare_function`; this
/// fills in the body IR. Returns pending lambda info on success.
/// Generic over `M: Module` so it works with both JITModule and ObjectModule.
#[allow(clippy::too_many_arguments)]
fn compile_definition_body<M: Module>(
    module: &mut M,
    helpers: &DeclaredHelpers,
    def: &RustIrDef,
    module_name: &str,
    qualified_name: &str,
    func_id: cranelift_module::FuncId,
    _arity: usize,
    param_types: &[Option<CgType>],
    _return_type: &Option<CgType>,
    compiled_decls: &HashMap<String, JitFuncDecl>,
    lambda_counter: &mut usize,
    spec_map: &HashMap<String, Vec<String>>,
    str_counter: &mut usize,
    use_far_jit_calls: bool,
) -> Result<Vec<CompiledLambdaInfo>, String> {
    let (params, body) = peel_params(&def.expr);

    // --- Pre-compile inner lambdas ---
    let mut lambdas: Vec<(&RustIrExpr, Vec<String>)> = Vec::new();
    collect_inner_lambdas(body, &mut Vec::new(), &mut lambdas);

    let mut compiled_lambdas: HashMap<usize, CompiledLambda> = HashMap::new();

    // Compile each lambda as a function: (ctx, cap0, cap1, ..., param) -> result
    // Lambdas are collected bottom-up (innermost first), so nested lambdas
    // appear before their parents.  Global lookups at runtime resolve
    // forward references.
    let mut pending_lambdas: Vec<CompiledLambdaInfo> = Vec::new();

    for (lambda_expr, captured_vars) in &lambdas {
        let RustIrExpr::Lambda {
            id,
            param,
            body,
            location,
        } = lambda_expr
        else {
            continue;
        };

        let total_arity = captured_vars.len() + 1; // captures + the actual param
        if total_arity > MAX_JIT_ARITY {
            eprintln!(
                "aivi: lambda skipped: too many captures ({} captures + 1 param = {} > {})",
                captured_vars.len(),
                total_arity,
                MAX_JIT_ARITY
            );
            continue;
        }

        let global_name = format!("__jit_lambda_{}", *lambda_counter);
        *lambda_counter += 1;

        // Leak the name so the raw pointer embedded in JIT code remains valid.
        let global_name_static: &'static str = Box::leak(global_name.clone().into_boxed_str());

        // Store in compiled_lambdas so nested lambdas can reference it
        let key = *id as usize;
        compiled_lambdas.insert(
            key,
            CompiledLambda {
                global_name: global_name_static,
                captured_vars: captured_vars.clone(),
            },
        );

        // Build function signature: (ctx, cap0, cap1, ..., param) -> result
        let mut sig = module.make_signature();
        sig.params.push(AbiParam::new(PTR)); // ctx
        for _ in 0..total_arity {
            sig.params.push(AbiParam::new(PTR)); // each cap + param
        }
        sig.returns.push(AbiParam::new(PTR));

        let func_name = format!("__aivi_lambda_{}", sanitize_name(&global_name));
        let func_id = module
            .declare_function(&func_name, Linkage::Local, &sig)
            .map_err(|e| format!("declare lambda {}: {e}", func_name))?;

        let mut function = Function::with_name_signature(
            cranelift_codegen::ir::UserFuncName::user(0, func_id.as_u32()),
            sig,
        );

        let helper_refs = helpers.import_into(module, &mut function);

        let mut fb_ctx = FunctionBuilderContext::new();
        {
            let mut builder = FunctionBuilder::new(&mut function, &mut fb_ctx);
            let entry = builder.create_block();
            builder.append_block_params_for_function_params(entry);
            builder.switch_to_block(entry);
            builder.seal_block(entry);

            let block_params = builder.block_params(entry).to_vec();
            let ctx_param = block_params[0];

            // --- Call-depth guard: bail with Unit if recursion too deep ---
            let depth_exceeded = builder
                .ins()
                .call(helper_refs.rt_check_call_depth, &[ctx_param]);
            let depth_flag = builder.inst_results(depth_exceeded)[0];
            let zero = builder.ins().iconst(types::I64, 0);
            let is_exceeded = builder.ins().icmp(
                cranelift_codegen::ir::condcodes::IntCC::NotEqual,
                depth_flag,
                zero,
            );
            let body_block = builder.create_block();
            let bail_block = builder.create_block();
            builder
                .ins()
                .brif(is_exceeded, bail_block, &[], body_block, &[]);

            // Bail block: return Unit without lowering the body
            builder.switch_to_block(bail_block);
            builder.seal_block(bail_block);
            let unit_val = builder.ins().call(helper_refs.rt_alloc_unit, &[ctx_param]);
            let unit_ptr = builder.inst_results(unit_val)[0];
            builder.ins().return_(&[unit_ptr]);

            // Body block: normal execution
            builder.switch_to_block(body_block);
            builder.seal_block(body_block);

            let empty_jit_funcs: HashMap<String, JitFuncInfo> = HashMap::new();
            let empty_spec_map: HashMap<String, Vec<String>> = HashMap::new();
            let mut lower_ctx = LowerCtx::new(
                ctx_param,
                &helper_refs,
                &compiled_lambdas,
                &empty_jit_funcs,
                &empty_spec_map,
                module,
                str_counter,
                module_name,
                use_far_jit_calls,
            );

            // Emit function-entry tracking (lambda — show parent name for context).
            // Guard against double-prefixing when `def.name` is already a qualified alias.
            let def_display = if def.name.starts_with(&format!("{module_name}.")) {
                def.name.clone()
            } else {
                format!("{module_name}.{}", def.name)
            };
            let lambda_display = format!("{def_display} (lambda)");
            lower_ctx.emit_enter_fn(&mut builder, &lambda_display, location.as_ref());

            // Perceus: run use analysis on the lambda body
            let use_map = super::use_analysis::analyze_uses(body);
            lower_ctx.set_use_map(use_map);

            // Bind captured vars as leading params (boxed — received as *mut Value)
            for (i, var_name) in captured_vars.iter().enumerate() {
                lower_ctx.locals.insert(
                    var_name.clone(),
                    super::lower::TypedValue::boxed(block_params[i + 1]),
                );
            }
            // Bind the actual lambda parameter (boxed)
            lower_ctx.locals.insert(
                param.clone(),
                super::lower::TypedValue::boxed(block_params[captured_vars.len() + 1]),
            );

            // When a lambda's parameter is a compiler-generated loop name (e.g.
            // `__loop1`), the loop body references that name via `rt_get_global`.
            // Register the parameter value as a runtime global here so that
            // recursive calls inside the loop body resolve correctly.
            if param.starts_with("__loop") {
                let param_val = block_params[captured_vars.len() + 1];
                lower_ctx.emit_set_global(&mut builder, param, param_val);
            }

            let result = lower_ctx.lower_expr(&mut builder, body);
            let result_boxed = lower_ctx.ensure_boxed(&mut builder, result);
            lower_ctx.emit_leave_fn(&mut builder);
            builder
                .ins()
                .call(helper_refs.rt_dec_call_depth, &[ctx_param]);
            builder.ins().return_(&[result_boxed]);
            builder.finalize();
        }

        let mut ctx = module.make_context();
        ctx.func = function;
        module
            .define_function(func_id, &mut ctx)
            .map_err(|e| format!("define lambda {}: {e}", func_name))?;
        module.clear_context(&mut ctx);

        pending_lambdas.push(CompiledLambdaInfo {
            func_id,
            global_name: global_name.clone(),
            total_arity,
        });
    }

    // --- Compile the main body (function was pre-declared in Pass 1) ---
    let sig = module
        .declarations()
        .get_function_decl(func_id)
        .signature
        .clone();
    let mut function = Function::with_name_signature(
        cranelift_codegen::ir::UserFuncName::user(0, func_id.as_u32()),
        sig,
    );

    let helper_refs = helpers.import_into(module, &mut function);

    // Import only JIT functions that the body actually references AND that have
    // been successfully compiled. Resolve short names via the current module.
    // Also import specializations (from spec_map) when the original is referenced.
    let mut called_globals = HashSet::new();
    collect_called_globals(body, &mut called_globals);

    let mut local_jit_funcs: HashMap<String, JitFuncInfo> = HashMap::new();
    let mut local_spec_map: HashMap<String, Vec<String>> = HashMap::new();
    for name in &called_globals {
        // Try qualified name first, then resolve short name via current module
        let decl = compiled_decls.get(name).or_else(|| {
            let qualified = format!("{}.{}", module_name, name);
            compiled_decls.get(&qualified)
        });
        if let Some(decl) = decl {
            let func_ref = if use_far_jit_calls {
                super::lower::declare_module_func_in_func(
                    module,
                    decl.func_id,
                    &mut function,
                    false,
                )
            } else {
                module.declare_func_in_func(decl.func_id, &mut function)
            };
            local_jit_funcs.insert(
                name.clone(),
                JitFuncInfo {
                    func_ref,
                    arity: decl.arity,
                    param_types: decl.param_types.clone(),
                    return_type: decl.return_type.clone(),
                },
            );
        }

        // Also import any specializations of this function
        if let Some(spec_names) = spec_map.get(name.as_str()) {
            let mut imported_specs = Vec::new();
            for spec_short in spec_names {
                // Resolve the specialization's qualified name
                let spec_qualified = format!("{}.{}", module_name, spec_short);
                let spec_decl = compiled_decls
                    .get(spec_short)
                    .or_else(|| compiled_decls.get(&spec_qualified));
                if let Some(sd) = spec_decl {
                    let func_ref = if use_far_jit_calls {
                        super::lower::declare_module_func_in_func(
                            module,
                            sd.func_id,
                            &mut function,
                            false,
                        )
                    } else {
                        module.declare_func_in_func(sd.func_id, &mut function)
                    };
                    local_jit_funcs.insert(
                        spec_short.clone(),
                        JitFuncInfo {
                            func_ref,
                            arity: sd.arity,
                            param_types: sd.param_types.clone(),
                            return_type: sd.return_type.clone(),
                        },
                    );
                    imported_specs.push(spec_short.clone());
                }
            }
            if !imported_specs.is_empty() {
                local_spec_map.insert(name.clone(), imported_specs);
            }
        }
    }

    let mut fb_ctx = FunctionBuilderContext::new();
    {
        let mut builder = FunctionBuilder::new(&mut function, &mut fb_ctx);
        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        builder.seal_block(entry);

        let block_params = builder.block_params(entry).to_vec();
        let ctx_param = block_params[0];

        // --- Call-depth guard: bail with Unit if recursion too deep ---
        let depth_exceeded = builder
            .ins()
            .call(helper_refs.rt_check_call_depth, &[ctx_param]);
        let depth_flag = builder.inst_results(depth_exceeded)[0];
        let zero = builder.ins().iconst(types::I64, 0);
        let is_exceeded = builder.ins().icmp(
            cranelift_codegen::ir::condcodes::IntCC::NotEqual,
            depth_flag,
            zero,
        );
        let body_block = builder.create_block();
        let bail_block = builder.create_block();
        builder
            .ins()
            .brif(is_exceeded, bail_block, &[], body_block, &[]);

        // Bail block: return Unit without lowering the body
        builder.switch_to_block(bail_block);
        builder.seal_block(bail_block);
        let unit_val = builder.ins().call(helper_refs.rt_alloc_unit, &[ctx_param]);
        let unit_ptr = builder.inst_results(unit_val)[0];
        builder.ins().return_(&[unit_ptr]);

        // Body block: normal execution
        builder.switch_to_block(body_block);
        builder.seal_block(body_block);

        let mut lower_ctx = LowerCtx::new(
            ctx_param,
            &helper_refs,
            &compiled_lambdas,
            &local_jit_funcs,
            &local_spec_map,
            module,
            str_counter,
            module_name,
            use_far_jit_calls,
        );

        // Emit function-entry tracking so runtime warnings can include the function name.
        // Qualified alias defs already have `module_name.` baked into `def.name`; avoid
        // double-prefixing (which would produce e.g. `aivi.list.aivi.list.find`).
        let display_name = if def.name.starts_with(&format!("{module_name}.")) {
            def.name.clone()
        } else {
            format!("{module_name}.{}", def.name)
        };
        lower_ctx.emit_enter_fn(&mut builder, &display_name, def.location.as_ref());

        // Perceus: run use analysis on the function body
        let use_map = super::use_analysis::analyze_uses(body);
        lower_ctx.set_use_map(use_map);

        // Bind params with typed unboxing when types are known
        let param_names: Vec<String> = params.iter().map(|s| s.to_string()).collect();
        lower_ctx.bind_typed_params(&mut builder, &param_names, &block_params, param_types);

        let result = lower_ctx.lower_expr(&mut builder, body);
        let result_boxed = lower_ctx.ensure_boxed(&mut builder, result);
        lower_ctx.emit_leave_fn(&mut builder);
        builder
            .ins()
            .call(helper_refs.rt_dec_call_depth, &[ctx_param]);
        builder.ins().return_(&[result_boxed]);
        builder.finalize();
    }

    let mut ctx = module.make_context();
    ctx.func = function;
    module
        .define_function(func_id, &mut ctx)
        .map_err(|e| format!("define {}: {e}", qualified_name))?;
    module.clear_context(&mut ctx);

    Ok(pending_lambdas)
}

/// Collect all inner Lambda nodes in post-order (innermost first).
/// `bound` tracks variables that are in scope (parameters, let-bindings).
/// Each lambda is returned with its list of captured (free) variables.
fn collect_inner_lambdas<'a>(
    expr: &'a RustIrExpr,
    bound: &mut Vec<String>,
    out: &mut Vec<(&'a RustIrExpr, Vec<String>)>,
) {
    match expr {
        RustIrExpr::Lambda { param, body, .. } => {
            bound.push(param.clone());
            collect_inner_lambdas(body, bound, out);
            bound.pop();

            let mut free = HashSet::new();
            let mut inner_bound = vec![param.clone()];
            collect_free_locals(body, &mut inner_bound, &mut free);
            let mut captured: Vec<String> = free.into_iter().collect();
            captured.sort();

            out.push((expr, captured));
        }
        RustIrExpr::App { func, arg, .. } => {
            collect_inner_lambdas(func, bound, out);
            collect_inner_lambdas(arg, bound, out);
        }
        RustIrExpr::Call { func, args, .. } => {
            collect_inner_lambdas(func, bound, out);
            for a in args {
                collect_inner_lambdas(a, bound, out);
            }
        }
        RustIrExpr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            collect_inner_lambdas(cond, bound, out);
            collect_inner_lambdas(then_branch, bound, out);
            collect_inner_lambdas(else_branch, bound, out);
        }
        RustIrExpr::Binary { left, right, .. } => {
            collect_inner_lambdas(left, bound, out);
            collect_inner_lambdas(right, bound, out);
        }
        RustIrExpr::FieldAccess { base, .. } => {
            collect_inner_lambdas(base, bound, out);
        }
        RustIrExpr::Index { base, index, .. } => {
            collect_inner_lambdas(base, bound, out);
            collect_inner_lambdas(index, bound, out);
        }
        RustIrExpr::List { items, .. } => {
            for item in items {
                collect_inner_lambdas(&item.expr, bound, out);
            }
        }
        RustIrExpr::Tuple { items, .. } => {
            for item in items {
                collect_inner_lambdas(item, bound, out);
            }
        }
        RustIrExpr::Record { fields, .. } => {
            for f in fields {
                collect_inner_lambdas(&f.value, bound, out);
            }
        }
        RustIrExpr::Match {
            scrutinee, arms, ..
        } => {
            collect_inner_lambdas(scrutinee, bound, out);
            for arm in arms {
                let mark = bound.len();
                collect_pattern_vars(&arm.pattern, bound);
                if let Some(g) = &arm.guard {
                    collect_inner_lambdas(g, bound, out);
                }
                collect_inner_lambdas(&arm.body, bound, out);
                bound.truncate(mark);
            }
        }
        RustIrExpr::Patch { target, fields, .. } => {
            collect_inner_lambdas(target, bound, out);
            for f in fields {
                collect_inner_lambdas(&f.value, bound, out);
            }
        }
        RustIrExpr::TextInterpolate { parts, .. } => {
            for p in parts {
                if let RustIrTextPart::Expr { expr } = p {
                    collect_inner_lambdas(expr, bound, out);
                }
            }
        }
        RustIrExpr::Pipe { func, arg, .. } => {
            collect_inner_lambdas(func, bound, out);
            collect_inner_lambdas(arg, bound, out);
        }
        RustIrExpr::DebugFn { body, .. } => {
            collect_inner_lambdas(body, bound, out);
        }
        RustIrExpr::Mock {
            substitutions,
            body,
            ..
        } => {
            for sub in substitutions {
                if let Some(v) = &sub.value {
                    collect_inner_lambdas(v, bound, out);
                }
            }
            collect_inner_lambdas(body, bound, out);
        }
        // Leaf expressions don't contain lambdas
        RustIrExpr::Local { .. }
        | RustIrExpr::Global { .. }
        | RustIrExpr::Builtin { .. }
        | RustIrExpr::ConstructorValue { .. }
        | RustIrExpr::LitBool { .. }
        | RustIrExpr::LitNumber { .. }
        | RustIrExpr::LitString { .. }
        | RustIrExpr::LitSigil { .. }
        | RustIrExpr::LitDateTime { .. }
        | RustIrExpr::Raw { .. } => {}
    }
}

/// Collect variable names bound by a pattern.
fn collect_pattern_vars(pat: &RustIrPattern, bound: &mut Vec<String>) {
    match pat {
        RustIrPattern::Var { name, .. } => bound.push(name.clone()),
        RustIrPattern::At { name, pattern, .. } => {
            bound.push(name.clone());
            collect_pattern_vars(pattern, bound);
        }
        RustIrPattern::Constructor { args, .. } => {
            for a in args {
                collect_pattern_vars(a, bound);
            }
        }
        RustIrPattern::Tuple { items, .. } => {
            for i in items {
                collect_pattern_vars(i, bound);
            }
        }
        RustIrPattern::List { items, rest, .. } => {
            for i in items {
                collect_pattern_vars(i, bound);
            }
            if let Some(r) = rest {
                collect_pattern_vars(r, bound);
            }
        }
        RustIrPattern::Record { fields, .. } => {
            for f in fields {
                collect_pattern_vars(&f.pattern, bound);
            }
        }
        RustIrPattern::Literal { .. } | RustIrPattern::Wildcard { .. } => {}
    }
}

/// Collect free local variable references in an expression.
/// `bound` tracks variables currently in scope; `free` accumulates unbound locals.
fn collect_free_locals(expr: &RustIrExpr, bound: &mut Vec<String>, free: &mut HashSet<String>) {
    match expr {
        RustIrExpr::Local { name, .. } => {
            if !bound.contains(name) {
                free.insert(name.clone());
            }
        }
        RustIrExpr::Lambda { param, body, .. } => {
            bound.push(param.clone());
            collect_free_locals(body, bound, free);
            bound.pop();
        }
        RustIrExpr::App { func, arg, .. } => {
            collect_free_locals(func, bound, free);
            collect_free_locals(arg, bound, free);
        }
        RustIrExpr::Call { func, args, .. } => {
            collect_free_locals(func, bound, free);
            for a in args {
                collect_free_locals(a, bound, free);
            }
        }
        RustIrExpr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            collect_free_locals(cond, bound, free);
            collect_free_locals(then_branch, bound, free);
            collect_free_locals(else_branch, bound, free);
        }
        RustIrExpr::Binary { left, right, .. } => {
            collect_free_locals(left, bound, free);
            collect_free_locals(right, bound, free);
        }
        RustIrExpr::FieldAccess { base, .. } => {
            collect_free_locals(base, bound, free);
        }
        RustIrExpr::Index { base, index, .. } => {
            collect_free_locals(base, bound, free);
            collect_free_locals(index, bound, free);
        }
        RustIrExpr::List { items, .. } => {
            for item in items {
                collect_free_locals(&item.expr, bound, free);
            }
        }
        RustIrExpr::Tuple { items, .. } => {
            for item in items {
                collect_free_locals(item, bound, free);
            }
        }
        RustIrExpr::Record { fields, .. } => {
            for f in fields {
                collect_free_locals(&f.value, bound, free);
            }
        }
        RustIrExpr::Match {
            scrutinee, arms, ..
        } => {
            collect_free_locals(scrutinee, bound, free);
            for arm in arms {
                let mark = bound.len();
                collect_pattern_vars(&arm.pattern, bound);
                if let Some(g) = &arm.guard {
                    collect_free_locals(g, bound, free);
                }
                collect_free_locals(&arm.body, bound, free);
                bound.truncate(mark);
            }
        }
        RustIrExpr::Patch { target, fields, .. } => {
            collect_free_locals(target, bound, free);
            for f in fields {
                collect_free_locals(&f.value, bound, free);
            }
        }
        RustIrExpr::TextInterpolate { parts, .. } => {
            for p in parts {
                if let RustIrTextPart::Expr { expr } = p {
                    collect_free_locals(expr, bound, free);
                }
            }
        }
        RustIrExpr::Pipe { func, arg, .. } => {
            collect_free_locals(func, bound, free);
            collect_free_locals(arg, bound, free);
        }
        RustIrExpr::DebugFn { body, .. } => {
            collect_free_locals(body, bound, free);
        }
        RustIrExpr::Mock {
            substitutions,
            body,
            ..
        } => {
            for sub in substitutions {
                if let Some(v) = &sub.value {
                    collect_free_locals(v, bound, free);
                }
            }
            collect_free_locals(body, bound, free);
        }
        // Leaves with no free locals
        RustIrExpr::Global { .. }
        | RustIrExpr::Builtin { .. }
        | RustIrExpr::ConstructorValue { .. }
        | RustIrExpr::LitBool { .. }
        | RustIrExpr::LitNumber { .. }
        | RustIrExpr::LitString { .. }
        | RustIrExpr::LitSigil { .. }
        | RustIrExpr::LitDateTime { .. }
        | RustIrExpr::Raw { .. } => {}
    }
}
