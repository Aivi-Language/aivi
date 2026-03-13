/// Compile an AIVI program to a native object file via Cranelift AOT.
///
/// Returns the raw object file bytes. The caller is responsible for writing
/// them to disk and linking with the AIVI runtime library.
pub fn compile_to_object(
    program: HirProgram,
    cg_types: HashMap<String, HashMap<String, CgType>>,
    monomorph_plan: HashMap<String, Vec<CgType>>,
    surface_modules: &[crate::surface::Module],
) -> Result<Vec<u8>, AiviError> {
    use super::object_module::create_object_module;

    // 1. Lower HIR → desugar blocks → RustIR
    let desugared_program = kernel::desugar_blocks(program);
    let mut rust_program = rust_ir::lower_kernel(desugared_program)?;

    // 2. Annotate each def with its CgType
    for module in &mut rust_program.modules {
        if let Some(module_types) = cg_types.get(&module.name) {
            for def in &mut module.defs {
                let cg_ty = module_types.get(&def.name).or_else(|| {
                    def.name
                        .rsplit('.')
                        .next()
                        .and_then(|short| module_types.get(short))
                });
                if let Some(cg_ty) = cg_ty {
                    def.cg_type = Some(cg_ty.clone());
                }
            }
        }
    }

    // 3. Monomorphize
    let spec_map = monomorphize_program(&mut rust_program.modules, &monomorph_plan);

    // 3b. Inline small functions
    super::inline::inline_program(&mut rust_program.modules);
    let duplicate_trivial_self_aliases =
        duplicate_trivial_self_alias_qualifieds(&rust_program.modules);

    // 4. Create ObjectModule targeting the host platform
    let mut module = create_object_module("aivi_program")
        .map_err(|e| AiviError::Runtime(format!("cranelift object init: {e}")))?;

    // 5. Declare runtime helper imports
    let helpers = declare_helpers(&mut module)
        .map_err(|e| AiviError::Runtime(format!("cranelift declare helpers: {e}")))?;

    // 6. Two-pass compilation (same as JIT path)
    #[allow(dead_code)]
    struct DeclaredDef<'a> {
        def: &'a RustIrDef,
        module_name: String,
        qualified: String,
        func_name: String,
        func_id: cranelift_module::FuncId,
        arity: usize,
        param_types: Vec<Option<CgType>>,
        return_type: Option<CgType>,
        is_effect_block: bool,
    }
    let mut declared_defs: Vec<DeclaredDef> = Vec::new();
    let mut declared_names: HashSet<String> = HashSet::new();

    // Pass 1: Declare all eligible functions
    for ir_module in &rust_program.modules {
        let module_dot = format!("{}.", ir_module.name);
        for def in &ir_module.defs {
            // Skip qualified aliases emitted by the Kernel
            if def.name.starts_with(&module_dot) {
                continue;
            }
            let qualified = format!("{}.{}", ir_module.name, def.name);
            if duplicate_trivial_self_aliases.contains(&qualified)
                && is_trivial_self_alias_def(def)
            {
                continue;
            }
            let (params, body) = peel_params(&def.expr);
            let is_stdlib_module = ir_module.name.starts_with("aivi.");
            if params.len() > MAX_JIT_ARITY {
                if is_stdlib_module {
                    continue;
                }
                return Err(AiviError::Runtime(format!(
                    "cranelift aot compile {}: unsupported arity {} (max {})",
                    qualified,
                    params.len(),
                    MAX_JIT_ARITY
                )));
            }
            if !expr_supported(body) {
                if is_stdlib_module {
                    continue;
                }
                return Err(AiviError::Runtime(format!(
                    "cranelift aot compile {}: unsupported expression shape",
                    qualified
                )));
            }
            let func_name = format!("__aivi_jit_{}", sanitize_name(&qualified));
            if declared_names.contains(&func_name) {
                continue;
            }
            declared_names.insert(func_name.clone());

            let arity = params.len();
            let mut sig = module.make_signature();
            sig.params.push(AbiParam::new(PTR)); // ctx
            for _ in 0..arity {
                sig.params.push(AbiParam::new(PTR));
            }
            sig.returns.push(AbiParam::new(PTR));

            // AOT: export all functions so the runtime can find them
            let func_id = module
                .declare_function(&func_name, Linkage::Export, &sig)
                .map_err(|e| AiviError::Runtime(format!("declare {}: {e}", func_name)))?;

            let (param_types, return_type) = if let Some(cg_ty) = &def.cg_type {
                decompose_func_type(cg_ty, arity)
            } else {
                (vec![None; arity], None)
            };

            let is_effect_block = false;

            declared_defs.push(DeclaredDef {
                def,
                module_name: ir_module.name.clone(),
                qualified: qualified.clone(),
                func_name,
                func_id,
                arity,
                param_types: param_types.clone(),
                return_type: return_type.clone(),
                is_effect_block,
            });
        }
    }

    // Build JitFuncDecl registry for direct calls
    let mut compiled_decls: HashMap<String, JitFuncDecl> = HashMap::new();
    // Pre-populate with all declared functions (AOT can forward-reference)
    for dd in &declared_defs {
        compiled_decls.insert(
            dd.qualified.clone(),
            JitFuncDecl {
                func_id: dd.func_id,
                arity: dd.arity,
                param_types: dd.param_types.clone(),
                return_type: dd.return_type.clone(),
            },
        );
    }

    // Pass 2: Compile function bodies
    let mut lambda_counter: usize = 0;
    let mut compiled_func_entries: Vec<AotFuncEntry> = Vec::new();
    let mut str_counter: usize = 0;

    let mut all_lambdas: Vec<CompiledLambdaInfo> = Vec::new();

    for dd in &declared_defs {
        match compile_definition_body(
            &mut module,
            &helpers,
            dd.def,
            &dd.module_name,
            &dd.qualified,
            dd.func_id,
            dd.arity,
            &dd.param_types,
            &dd.return_type,
            &compiled_decls,
            &mut lambda_counter,
            &spec_map,
            &mut str_counter,
        ) {
            Ok(lambdas) => {
                all_lambdas.extend(lambdas);
                compiled_func_entries.push(AotFuncEntry {
                    short_name: dd.def.name.clone(),
                    qualified_name: dd.qualified.clone(),
                    func_id: dd.func_id,
                    arity: dd.arity,
                    is_effect_block: dd.is_effect_block,
                });
            }
            Err(e) => {
                return Err(AiviError::Runtime(format!(
                    "cranelift aot compile {}: {e}",
                    dd.qualified
                )))
            }
        }
    }

    // 7. Generate the entry point wrapper: __aivi_main()
    generate_aot_entry(
        &mut module,
        &helpers,
        &compiled_func_entries,
        &all_lambdas,
        surface_modules,
    )
    .map_err(|e| AiviError::Runtime(format!("aot entry point: {e}")))?;

    // 8. Emit the object file
    let product = module.finish();
    let bytes = product
        .emit()
        .map_err(|e| AiviError::Runtime(format!("emit object: {e}")))?;

    Ok(bytes)
}

/// Information about a compiled function for AOT entry-point registration.
pub(crate) struct AotFuncEntry {
    pub(crate) short_name: String,
    pub(crate) qualified_name: String,
    pub(crate) func_id: cranelift_module::FuncId,
    pub(crate) arity: usize,
    pub(crate) is_effect_block: bool,
}

/// Generate the AOT entry point `__aivi_main` that:
/// 1. Registers all compiled functions as globals via `rt_register_jit_fn`
/// 2. Looks up and runs the `main` function as an effect
/// 3. Returns the result
fn generate_aot_entry<M: Module>(
    module: &mut M,
    helpers: &DeclaredHelpers,
    compiled_funcs: &[AotFuncEntry],
    compiled_lambdas: &[CompiledLambdaInfo],
    _surface_modules: &[crate::surface::Module],
) -> Result<(), String> {
    use cranelift_module::DataDescription;

    // Declare the entry function: (ctx) -> ptr
    let mut sig = module.make_signature();
    sig.params.push(AbiParam::new(PTR)); // ctx
    sig.returns.push(AbiParam::new(PTR)); // result

    let func_id = module
        .declare_function("__aivi_main", Linkage::Export, &sig)
        .map_err(|e| format!("declare __aivi_main: {e}"))?;

    let mut function = Function::with_name_signature(
        cranelift_codegen::ir::UserFuncName::user(0, func_id.as_u32()),
        sig,
    );

    // Import helpers
    let helper_refs = helpers.import_into(module, &mut function);

    // Embed function name strings as data sections and declare func refs
    struct FuncReg {
        func_ref: cranelift_codegen::ir::FuncRef,
        short_name_gv: cranelift_codegen::ir::GlobalValue,
        short_name_len: usize,
        qual_name_gv: cranelift_codegen::ir::GlobalValue,
        qual_name_len: usize,
        arity: usize,
        is_effect_block: bool,
    }
    let mut regs = Vec::new();

    for (i, entry) in compiled_funcs.iter().enumerate() {
        let func_ref = module.declare_func_in_func(entry.func_id, &mut function);

        // Embed short name
        let short_data_id = module
            .declare_data(&format!("__nm_s_{i}"), Linkage::Local, false, false)
            .map_err(|e| format!("declare name data: {e}"))?;
        let mut dd = DataDescription::new();
        dd.define(entry.short_name.as_bytes().to_vec().into_boxed_slice());
        module
            .define_data(short_data_id, &dd)
            .map_err(|e| format!("define name data: {e}"))?;
        let short_gv = module.declare_data_in_func(short_data_id, &mut function);

        // Embed qualified name
        let qual_data_id = module
            .declare_data(&format!("__nm_q_{i}"), Linkage::Local, false, false)
            .map_err(|e| format!("declare qual name data: {e}"))?;
        let mut dd = DataDescription::new();
        dd.define(entry.qualified_name.as_bytes().to_vec().into_boxed_slice());
        module
            .define_data(qual_data_id, &dd)
            .map_err(|e| format!("define qual name data: {e}"))?;
        let qual_gv = module.declare_data_in_func(qual_data_id, &mut function);

        regs.push(FuncReg {
            func_ref,
            short_name_gv: short_gv,
            short_name_len: entry.short_name.len(),
            qual_name_gv: qual_gv,
            qual_name_len: entry.qualified_name.len(),
            arity: entry.arity,
            is_effect_block: entry.is_effect_block,
        });
    }

    // Prepare lambda registrations
    struct LambdaReg {
        func_ref: cranelift_codegen::ir::FuncRef,
        name_gv: cranelift_codegen::ir::GlobalValue,
        name_len: usize,
        arity: usize,
    }
    let mut lambda_regs = Vec::new();

    for (i, lam) in compiled_lambdas.iter().enumerate() {
        let func_ref = module.declare_func_in_func(lam.func_id, &mut function);

        let data_id = module
            .declare_data(&format!("__nm_lam_{i}"), Linkage::Local, false, false)
            .map_err(|e| format!("declare lambda name data: {e}"))?;
        let mut dd = DataDescription::new();
        dd.define(lam.global_name.as_bytes().to_vec().into_boxed_slice());
        module
            .define_data(data_id, &dd)
            .map_err(|e| format!("define lambda name data: {e}"))?;
        let name_gv = module.declare_data_in_func(data_id, &mut function);

        lambda_regs.push(LambdaReg {
            func_ref,
            name_gv,
            name_len: lam.global_name.len(),
            arity: lam.total_arity,
        });
    }

    // Embed "main" string for the final lookup
    let main_data_id = module
        .declare_data("__nm_main", Linkage::Local, false, false)
        .map_err(|e| format!("declare main name: {e}"))?;
    let mut dd = DataDescription::new();
    dd.define(b"main".to_vec().into_boxed_slice());
    module
        .define_data(main_data_id, &dd)
        .map_err(|e| format!("define main name: {e}"))?;
    let main_name_gv = module.declare_data_in_func(main_data_id, &mut function);

    // Build the function body
    let mut fb_ctx = FunctionBuilderContext::new();
    {
        let mut builder = FunctionBuilder::new(&mut function, &mut fb_ctx);
        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        builder.seal_block(entry);

        let ctx_param = builder.block_params(entry)[0];

        // Register each compiled function (short + qualified name)
        for reg in &regs {
            let func_ptr = builder.ins().func_addr(PTR, reg.func_ref);
            let arity_val = builder.ins().iconst(PTR, reg.arity as i64);
            let is_effect_val = builder
                .ins()
                .iconst(PTR, if reg.is_effect_block { 1i64 } else { 0i64 });

            let short_ptr = builder.ins().global_value(PTR, reg.short_name_gv);
            let short_len = builder.ins().iconst(PTR, reg.short_name_len as i64);
            builder.ins().call(
                helper_refs.rt_register_jit_fn,
                &[
                    ctx_param,
                    short_ptr,
                    short_len,
                    func_ptr,
                    arity_val,
                    is_effect_val,
                ],
            );

            let qual_ptr = builder.ins().global_value(PTR, reg.qual_name_gv);
            let qual_len = builder.ins().iconst(PTR, reg.qual_name_len as i64);
            builder.ins().call(
                helper_refs.rt_register_jit_fn,
                &[
                    ctx_param,
                    qual_ptr,
                    qual_len,
                    func_ptr,
                    arity_val,
                    is_effect_val,
                ],
            );
        }

        // Register each compiled lambda
        for lreg in &lambda_regs {
            let func_ptr = builder.ins().func_addr(PTR, lreg.func_ref);
            let arity_val = builder.ins().iconst(PTR, lreg.arity as i64);
            let is_effect_val = builder.ins().iconst(PTR, 0i64);

            let name_ptr = builder.ins().global_value(PTR, lreg.name_gv);
            let name_len = builder.ins().iconst(PTR, lreg.name_len as i64);
            builder.ins().call(
                helper_refs.rt_register_jit_fn,
                &[
                    ctx_param,
                    name_ptr,
                    name_len,
                    func_ptr,
                    arity_val,
                    is_effect_val,
                ],
            );
        }

        // Look up "main" and run as effect
        let main_ptr = builder.ins().global_value(PTR, main_name_gv);
        let main_len = builder.ins().iconst(PTR, 4i64);
        let main_val = builder
            .ins()
            .call(helper_refs.rt_get_global, &[ctx_param, main_ptr, main_len]);
        let main_val = builder.inst_results(main_val)[0];

        let result = builder
            .ins()
            .call(helper_refs.rt_run_effect, &[ctx_param, main_val]);
        let result = builder.inst_results(result)[0];

        builder.ins().return_(&[result]);
        builder.finalize();
    }

    let mut ctx = module.make_context();
    ctx.func = function;
    module
        .define_function(func_id, &mut ctx)
        .map_err(|e| format!("define __aivi_main: {e}"))?;
    module.clear_context(&mut ctx);

    Ok(())
}

/// Information about a compiled lambda that needs post-processing.
pub(crate) struct CompiledLambdaInfo {
    pub(crate) func_id: cranelift_module::FuncId,
    pub(crate) global_name: String,
    pub(crate) total_arity: usize,
}
