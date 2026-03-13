//! Lower `RustIrExpr` to Cranelift IR.
//!
//! This is the main expression-lowering engine for the full Cranelift backend.
//! Values are represented in one of two ways:
//!
//! - **Boxed** (`*mut Value`): heap-allocated tagged union, used for compound
//!   types, unknown types, and at runtime-helper call boundaries.
//! - **Unboxed** (native scalar): `i64` for Int, `f64` for Float, `i8` for Bool.
//!   Kept in CPU registers — no heap allocation or tag dispatch.
//!
//! The `TypedValue` wrapper tracks which representation each SSA value uses.
//! Every emitted function currently has the signature:
//!     `(ctx: i64, ...args: i64) -> i64`
//! where `ctx` is a `*mut JitRuntimeCtx` and each arg/return is a `*mut Value`.

use std::collections::HashMap;

use cranelift_codegen::ir::FuncRef;
use cranelift_codegen::ir::{types, AbiParam, BlockArg, Function, InstBuilder, Value};
use cranelift_frontend::FunctionBuilder;
use cranelift_jit::JITModule;
use cranelift_module::{DataDescription, Linkage, Module};

use crate::cg_type::CgType;
use crate::rust_ir::{
    RustIrExpr, RustIrListItem, RustIrLiteral, RustIrMatchArm, RustIrMockSubstitution,
    RustIrPattern, RustIrRecordField, RustIrTextPart,
};

/// Pointer-sized integer type used for boxed `*mut Value` pointers.
const PTR: cranelift_codegen::ir::Type = types::I64;
/// Native float type for unboxed `Float` values.
const F64: cranelift_codegen::ir::Type = types::F64;

/// A Cranelift SSA value paired with its known `CgType`.
///
/// This allows the lowering engine to keep scalars in registers (unboxed)
/// and only box them when crossing a boundary that expects `*mut Value`.
#[derive(Clone)]
pub(crate) struct TypedValue {
    /// The Cranelift SSA value.  For `CgType::Int` this is an `i64`,
    /// for `CgType::Float` an `f64`, for `CgType::Bool` an `i64` (0/1).
    /// For everything else it is a `*mut Value` pointer.
    pub(crate) val: Value,
    /// The compile-time type. `None` means "boxed / unknown".
    pub(crate) ty: Option<CgType>,
}

impl TypedValue {
    /// Create a typed value with a known scalar type.
    pub(crate) fn typed(val: Value, ty: CgType) -> Self {
        Self { val, ty: Some(ty) }
    }
    /// Create a boxed value (`*mut Value` pointer, type unknown or compound).
    pub(crate) fn boxed(val: Value) -> Self {
        Self { val, ty: None }
    }
}

/// Information about a pre-compiled inner lambda function.
pub(crate) struct CompiledLambda {
    /// Unique global name where the lambda Builtin is stored in the runtime.
    /// This is a leaked `&'static str` because `lower_global` embeds the
    /// string pointer as a constant in the JIT code, which outlives the
    /// `compile_definition` scope.
    pub(crate) global_name: &'static str,
    /// Names of the captured (free) variables, in the order they appear as
    /// leading parameters of the compiled function.
    pub(crate) captured_vars: Vec<String>,
}

/// Context for lowering a single function body.
///
/// Generic over `M: Module` so that string constants can be embedded as
/// relocatable data sections — required for AOT where compile-time `&str`
/// pointers are invalid in the final binary.
pub(crate) struct LowerCtx<'a, M: Module> {
    /// Maps local variable names to typed Cranelift SSA values.
    pub(crate) locals: HashMap<String, TypedValue>,
    /// The `ctx` (JitRuntimeCtx) parameter — first arg of every function.
    ctx_param: Value,
    /// Declared runtime helper function references in this module.
    helpers: &'a HelperRefs,
    /// Pre-compiled inner lambda functions, keyed by stable Rust IR expr id.
    pub(crate) compiled_lambdas: &'a HashMap<usize, CompiledLambda>,
    /// Registry of JIT-compiled functions available for direct calls.
    /// Maps qualified name → (FuncRef, param count, param types, return type).
    jit_funcs: &'a HashMap<String, JitFuncInfo>,
    /// Maps original short name → list of specialization short names.
    /// Used for call-site routing to monomorphized versions.
    spec_map: &'a HashMap<String, Vec<String>>,
    /// Module handle for embedding string data sections.
    module: &'a mut M,
    /// Monotonic counter for unique data section names (shared across functions).
    str_counter: &'a mut usize,
    /// Per-function cache of already-embedded strings → GlobalValue.
    str_cache: HashMap<Vec<u8>, cranelift_codegen::ir::GlobalValue>,
    /// Perceus use analysis: tracks last-use of each variable.
    use_map: Option<super::use_analysis::UseMap>,
    /// Perceus reuse token: a `*mut Value`-sized allocation that can be
    /// recycled by the next constructor/record/list allocation.
    /// Set by `lower_match` when the scrutinee is consumed.
    reuse_token: Option<Value>,
    /// The module name for the function being compiled, used to qualify bare
    /// global references so cross-module name collisions are resolved correctly.
    module_name: String,
}

/// Metadata about a JIT-compiled function, used for direct calls.
#[allow(dead_code)]
#[derive(Clone)]
pub(crate) struct JitFuncInfo {
    pub(crate) func_ref: FuncRef,
    pub(crate) arity: usize,
    pub(crate) param_types: Vec<Option<CgType>>,
    pub(crate) return_type: Option<CgType>,
}

/// Module-level metadata for a JIT function (stores FuncId, not FuncRef).
#[allow(dead_code)]
pub(crate) struct JitFuncDecl {
    pub(crate) func_id: cranelift_module::FuncId,
    pub(crate) arity: usize,
    pub(crate) param_types: Vec<Option<CgType>>,
    pub(crate) return_type: Option<CgType>,
}

/// Decompose a function CgType into parameter types and return type.
/// `Func(A, Func(B, C))` with arity 2 → `([Some(A), Some(B)], Some(C))`
pub(crate) fn decompose_func_type(
    ty: &CgType,
    arity: usize,
) -> (Vec<Option<CgType>>, Option<CgType>) {
    let mut params = Vec::new();
    let mut current = ty;
    for _ in 0..arity {
        match current {
            CgType::Func(param, ret) => {
                params.push(scalar_type(param));
                current = ret;
            }
            _ => {
                // Ran out of Func nesting before arity — fill remaining as None
                while params.len() < arity {
                    params.push(None);
                }
                return (params, None);
            }
        }
    }
    (params, scalar_type(current))
}

/// Return `Some(ty)` for types we can represent unboxed, `None` otherwise.
fn scalar_type(ty: &CgType) -> Option<CgType> {
    match ty {
        CgType::Int | CgType::Float | CgType::Bool => Some(ty.clone()),
        _ => None,
    }
}

/// Returns `true` when the pattern directly aliases the scrutinee pointer
/// (i.e., `bind_pattern` stores the scrutinee value itself rather than
/// extracting fields). In that case the Perceus reuse optimisation must be
/// skipped, because `rt_try_reuse` would overwrite the memory that the
/// pattern-bound variable still references.
fn pattern_aliases_scrutinee(pattern: &RustIrPattern) -> bool {
    matches!(
        pattern,
        RustIrPattern::Var { .. } | RustIrPattern::At { .. }
    )
}

/// Pre-declared `FuncRef`s for all runtime helpers in a JIT module.
#[allow(dead_code)]
pub(crate) struct HelperRefs {
    // Call-depth guard
    pub(crate) rt_check_call_depth: FuncRef,
    pub(crate) rt_dec_call_depth: FuncRef,
    // Match failure signaling
    pub(crate) rt_signal_match_fail: FuncRef,
    // Boxing/unboxing
    pub(crate) rt_box_int: FuncRef,
    pub(crate) rt_box_float: FuncRef,
    pub(crate) rt_box_bool: FuncRef,
    pub(crate) rt_unbox_int: FuncRef,
    pub(crate) rt_unbox_float: FuncRef,
    pub(crate) rt_unbox_bool: FuncRef,
    pub(crate) rt_alloc_unit: FuncRef,
    pub(crate) rt_alloc_string: FuncRef,
    pub(crate) rt_alloc_list: FuncRef,
    pub(crate) rt_alloc_tuple: FuncRef,
    pub(crate) rt_alloc_record: FuncRef,
    pub(crate) rt_alloc_constructor: FuncRef,
    pub(crate) rt_record_field: FuncRef,
    pub(crate) rt_list_index: FuncRef,
    pub(crate) rt_clone_value: FuncRef,
    pub(crate) rt_drop_value: FuncRef,
    pub(crate) rt_get_global: FuncRef,
    pub(crate) rt_set_global: FuncRef,
    pub(crate) rt_apply: FuncRef,
    pub(crate) rt_force_thunk: FuncRef,
    pub(crate) rt_run_effect: FuncRef,
    pub(crate) rt_bind_effect: FuncRef,
    pub(crate) rt_wrap_effect: FuncRef,
    // Resource scope management
    pub(crate) rt_push_resource_scope: FuncRef,
    pub(crate) rt_pop_resource_scope: FuncRef,
    pub(crate) rt_binary_op: FuncRef,
    // Pattern matching helpers
    pub(crate) rt_constructor_name_eq: FuncRef,
    pub(crate) rt_constructor_arity: FuncRef,
    pub(crate) rt_constructor_arg: FuncRef,
    pub(crate) rt_tuple_len: FuncRef,
    pub(crate) rt_tuple_item: FuncRef,
    pub(crate) rt_list_len: FuncRef,
    pub(crate) rt_list_tail: FuncRef,
    pub(crate) rt_list_concat: FuncRef,
    pub(crate) rt_value_equals: FuncRef,
    // Record patching
    pub(crate) rt_patch_record: FuncRef,
    pub(crate) rt_patch_record_inplace: FuncRef,
    // Closure creation
    pub(crate) rt_make_closure: FuncRef,
    // Native generate helpers
    pub(crate) rt_generator_to_list: FuncRef,
    pub(crate) rt_gen_vec_new: FuncRef,
    pub(crate) rt_gen_vec_push: FuncRef,
    pub(crate) rt_gen_vec_extend_generator: FuncRef,
    pub(crate) rt_gen_vec_into_generator: FuncRef,
    // AOT function registration
    pub(crate) rt_register_jit_fn: FuncRef,
    // DateTime allocation
    pub(crate) rt_alloc_datetime: FuncRef,
    // Sigil evaluation
    pub(crate) rt_eval_sigil: FuncRef,
    // Perceus reuse: (ctx, ptr) -> ptr (returns reuse token or null)
    pub(crate) rt_try_reuse: FuncRef,
    // Perceus reuse-aware allocation
    pub(crate) rt_reuse_constructor: FuncRef,
    pub(crate) rt_reuse_record: FuncRef,
    pub(crate) rt_reuse_list: FuncRef,
    pub(crate) rt_reuse_tuple: FuncRef,
    // Source location tracking for diagnostics
    pub(crate) rt_enter_fn: FuncRef,
    // Source location tracking for diagnostics
    pub(crate) rt_set_location: FuncRef,
    // Snapshot mock helpers
    pub(crate) rt_snapshot_mock_install: FuncRef,
    pub(crate) rt_snapshot_mock_flush: FuncRef,
}

fn define_jit_helper_thunk(
    module: &mut JITModule,
    func_id: cranelift_module::FuncId,
    sig: &cranelift_codegen::ir::Signature,
    host_ptr: *const u8,
    helper_name: &str,
) -> Result<(), String> {
    let mut function = Function::with_name_signature(
        cranelift_codegen::ir::UserFuncName::user(0, func_id.as_u32()),
        sig.clone(),
    );
    let call_sig = function.import_signature(sig.clone());
    let mut fb_ctx = cranelift_frontend::FunctionBuilderContext::new();
    {
        let mut builder = FunctionBuilder::new(&mut function, &mut fb_ctx);
        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        builder.seal_block(entry);

        let args = builder.block_params(entry).to_vec();
        let target = builder.ins().iconst(PTR, host_ptr as usize as i64);
        let call = builder.ins().call_indirect(call_sig, target, &args);
        let results = builder.inst_results(call).to_vec();
        builder.ins().return_(&results);
        builder.finalize();
    }

    let mut ctx = module.make_context();
    ctx.func = function;
    module
        .define_function(func_id, &mut ctx)
        .map_err(|e| format!("define JIT helper thunk {helper_name}: {e}"))?;
    module.clear_context(&mut ctx);
    Ok(())
}

/// Declare helper thunks for JIT mode.
///
/// Direct imported host calls can overflow x86 rel32 relocations when the JIT
/// code cache lands far from the Rust/runtime text segments. These local thunks
/// keep JIT-to-thunk calls module-local and use an indirect call for the final
/// host jump.
pub(crate) fn declare_jit_helpers(module: &mut JITModule) -> Result<DeclaredHelpers, String> {
    let helper_symbols: std::collections::HashMap<&'static str, *const u8> =
        super::runtime_helpers::runtime_helper_symbols()
            .into_iter()
            .collect();

    macro_rules! decl {
        ($name:expr, [$($param:expr),*], [$($ret:expr),*]) => {{
            let mut sig = module.make_signature();
            $(sig.params.push(AbiParam::new($param));)*
            $(sig.returns.push(AbiParam::new($ret));)*
            let func_name = format!("__aivi_jit_helper_{}", $name);
            let func_id = module
                .declare_function(&func_name, Linkage::Local, &sig)
                .map_err(|e| format!("declare JIT helper thunk {}: {e}", $name))?;
            let host_ptr = *helper_symbols
                .get($name)
                .ok_or_else(|| format!("missing runtime helper symbol {}", $name))?;
            define_jit_helper_thunk(module, func_id, &sig, host_ptr, $name)?;
            func_id
        }};
    }

    Ok(DeclaredHelpers {
        rt_check_call_depth: decl!("rt_check_call_depth", [PTR], [PTR]),
        rt_dec_call_depth: decl!("rt_dec_call_depth", [PTR], []),
        rt_signal_match_fail: decl!("rt_signal_match_fail", [PTR], [PTR]),
        rt_box_int: decl!("rt_box_int", [PTR, PTR], [PTR]),
        rt_box_float: decl!("rt_box_float", [PTR, PTR], [PTR]),
        rt_box_bool: decl!("rt_box_bool", [PTR, PTR], [PTR]),
        rt_unbox_int: decl!("rt_unbox_int", [PTR, PTR], [PTR]),
        rt_unbox_float: decl!("rt_unbox_float", [PTR, PTR], [PTR]),
        rt_unbox_bool: decl!("rt_unbox_bool", [PTR, PTR], [PTR]),
        rt_alloc_unit: decl!("rt_alloc_unit", [PTR], [PTR]),
        rt_alloc_string: decl!("rt_alloc_string", [PTR, PTR, PTR], [PTR]),
        rt_alloc_list: decl!("rt_alloc_list", [PTR, PTR, PTR], [PTR]),
        rt_alloc_tuple: decl!("rt_alloc_tuple", [PTR, PTR, PTR], [PTR]),
        rt_alloc_record: decl!("rt_alloc_record", [PTR, PTR, PTR, PTR, PTR], [PTR]),
        rt_alloc_constructor: decl!("rt_alloc_constructor", [PTR, PTR, PTR, PTR, PTR], [PTR]),
        rt_record_field: decl!("rt_record_field", [PTR, PTR, PTR, PTR], [PTR]),
        rt_list_index: decl!("rt_list_index", [PTR, PTR, PTR], [PTR]),
        rt_clone_value: decl!("rt_clone_value", [PTR, PTR], [PTR]),
        rt_drop_value: decl!("rt_drop_value", [PTR, PTR], []),
        rt_get_global: decl!("rt_get_global", [PTR, PTR, PTR], [PTR]),
        rt_set_global: decl!("rt_set_global", [PTR, PTR, PTR, PTR], []),
        rt_apply: decl!("rt_apply", [PTR, PTR, PTR], [PTR]),
        rt_force_thunk: decl!("rt_force_thunk", [PTR, PTR], [PTR]),
        rt_run_effect: decl!("rt_run_effect", [PTR, PTR], [PTR]),
        rt_bind_effect: decl!("rt_bind_effect", [PTR, PTR, PTR], [PTR]),
        rt_wrap_effect: decl!("rt_wrap_effect", [PTR, PTR], [PTR]),
        rt_push_resource_scope: decl!("rt_push_resource_scope", [PTR], []),
        rt_pop_resource_scope: decl!("rt_pop_resource_scope", [PTR], []),
        rt_binary_op: decl!("rt_binary_op", [PTR, PTR, PTR, PTR, PTR], [PTR]),
        rt_constructor_name_eq: decl!("rt_constructor_name_eq", [PTR, PTR, PTR, PTR], [PTR]),
        rt_constructor_arity: decl!("rt_constructor_arity", [PTR, PTR], [PTR]),
        rt_constructor_arg: decl!("rt_constructor_arg", [PTR, PTR, PTR], [PTR]),
        rt_tuple_len: decl!("rt_tuple_len", [PTR, PTR], [PTR]),
        rt_tuple_item: decl!("rt_tuple_item", [PTR, PTR, PTR], [PTR]),
        rt_list_len: decl!("rt_list_len", [PTR, PTR], [PTR]),
        rt_list_tail: decl!("rt_list_tail", [PTR, PTR, PTR], [PTR]),
        rt_list_concat: decl!("rt_list_concat", [PTR, PTR, PTR], [PTR]),
        rt_value_equals: decl!("rt_value_equals", [PTR, PTR, PTR], [PTR]),
        rt_patch_record: decl!("rt_patch_record", [PTR, PTR, PTR, PTR, PTR, PTR], [PTR]),
        rt_patch_record_inplace: decl!(
            "rt_patch_record_inplace",
            [PTR, PTR, PTR, PTR, PTR, PTR],
            [PTR]
        ),
        rt_make_closure: decl!("rt_make_closure", [PTR, PTR, PTR, PTR], [PTR]),
        rt_generator_to_list: decl!("rt_generator_to_list", [PTR, PTR], [PTR]),
        rt_gen_vec_new: decl!("rt_gen_vec_new", [PTR], [PTR]),
        rt_gen_vec_push: decl!("rt_gen_vec_push", [PTR, PTR, PTR], []),
        rt_gen_vec_extend_generator: decl!("rt_gen_vec_extend_generator", [PTR, PTR, PTR], []),
        rt_gen_vec_into_generator: decl!("rt_gen_vec_into_generator", [PTR, PTR], [PTR]),
        rt_register_jit_fn: decl!("rt_register_jit_fn", [PTR, PTR, PTR, PTR, PTR, PTR], []),
        rt_alloc_datetime: decl!("rt_alloc_datetime", [PTR, PTR, PTR], [PTR]),
        rt_eval_sigil: decl!("rt_eval_sigil", [PTR, PTR, PTR, PTR, PTR, PTR, PTR], [PTR]),
        rt_try_reuse: decl!("rt_try_reuse", [PTR, PTR], [PTR]),
        rt_reuse_constructor: decl!(
            "rt_reuse_constructor",
            [PTR, PTR, PTR, PTR, PTR, PTR],
            [PTR]
        ),
        rt_reuse_record: decl!("rt_reuse_record", [PTR, PTR, PTR, PTR, PTR, PTR], [PTR]),
        rt_reuse_list: decl!("rt_reuse_list", [PTR, PTR, PTR, PTR], [PTR]),
        rt_reuse_tuple: decl!("rt_reuse_tuple", [PTR, PTR, PTR, PTR], [PTR]),
        rt_enter_fn: decl!("rt_enter_fn", [PTR, PTR, PTR], []),
        rt_set_location: decl!("rt_set_location", [PTR, PTR, PTR], []),
        rt_snapshot_mock_install: decl!("rt_snapshot_mock_install", [PTR, PTR, PTR], [PTR]),
        rt_snapshot_mock_flush: decl!("rt_snapshot_mock_flush", [PTR, PTR, PTR], []),
    })
}

/// Declare all runtime helper signatures in the module and return FuncRefs
/// that can be imported into individual functions via `module.declare_func_in_func`.
pub(crate) fn declare_helpers(module: &mut impl Module) -> Result<DeclaredHelpers, String> {
    // Helper macro: declare an imported function with the given signature
    macro_rules! decl {
        ($name:expr, [$($param:expr),*], [$($ret:expr),*]) => {{
            let mut sig = module.make_signature();
            $(sig.params.push(AbiParam::new($param));)*
            $(sig.returns.push(AbiParam::new($ret));)*
            module
                .declare_function($name, Linkage::Import, &sig)
                .map_err(|e| format!("declare {}: {e}", $name))?
        }};
    }

    Ok(DeclaredHelpers {
        // (ctx) -> i64  (0 = ok, 1 = depth exceeded)
        rt_check_call_depth: decl!("rt_check_call_depth", [PTR], [PTR]),
        // (ctx) -> void
        rt_dec_call_depth: decl!("rt_dec_call_depth", [PTR], []),
        // (ctx) -> ptr  (signal non-exhaustive match)
        rt_signal_match_fail: decl!("rt_signal_match_fail", [PTR], [PTR]),
        // (ctx, i64) -> ptr
        rt_box_int: decl!("rt_box_int", [PTR, PTR], [PTR]),
        rt_box_float: decl!("rt_box_float", [PTR, PTR], [PTR]),
        rt_box_bool: decl!("rt_box_bool", [PTR, PTR], [PTR]),
        // (ctx, ptr) -> i64
        rt_unbox_int: decl!("rt_unbox_int", [PTR, PTR], [PTR]),
        rt_unbox_float: decl!("rt_unbox_float", [PTR, PTR], [PTR]),
        rt_unbox_bool: decl!("rt_unbox_bool", [PTR, PTR], [PTR]),
        // (ctx) -> ptr
        rt_alloc_unit: decl!("rt_alloc_unit", [PTR], [PTR]),
        // (ctx, str_ptr, str_len) -> ptr
        rt_alloc_string: decl!("rt_alloc_string", [PTR, PTR, PTR], [PTR]),
        // (ctx, items_ptr, len) -> ptr
        rt_alloc_list: decl!("rt_alloc_list", [PTR, PTR, PTR], [PTR]),
        rt_alloc_tuple: decl!("rt_alloc_tuple", [PTR, PTR, PTR], [PTR]),
        // (ctx, names_ptr, name_lens_ptr, values_ptr, len) -> ptr
        rt_alloc_record: decl!("rt_alloc_record", [PTR, PTR, PTR, PTR, PTR], [PTR]),
        // (ctx, name_ptr, name_len, args_ptr, args_len) -> ptr
        rt_alloc_constructor: decl!("rt_alloc_constructor", [PTR, PTR, PTR, PTR, PTR], [PTR]),
        // (ctx, value_ptr, name_ptr, name_len) -> ptr
        rt_record_field: decl!("rt_record_field", [PTR, PTR, PTR, PTR], [PTR]),
        // (ctx, value_ptr, index) -> ptr
        rt_list_index: decl!("rt_list_index", [PTR, PTR, PTR], [PTR]),
        // (ctx, ptr) -> ptr
        rt_clone_value: decl!("rt_clone_value", [PTR, PTR], [PTR]),
        // (ctx, ptr) -> void
        rt_drop_value: decl!("rt_drop_value", [PTR, PTR], []),
        // (ctx, name_ptr, name_len) -> ptr
        rt_get_global: decl!("rt_get_global", [PTR, PTR, PTR], [PTR]),
        // (ctx, name_ptr, name_len, value_ptr) -> void
        rt_set_global: decl!("rt_set_global", [PTR, PTR, PTR, PTR], []),
        // (ctx, func_ptr, arg_ptr) -> ptr
        rt_apply: decl!("rt_apply", [PTR, PTR, PTR], [PTR]),
        // (ctx, ptr) -> ptr
        rt_force_thunk: decl!("rt_force_thunk", [PTR, PTR], [PTR]),
        rt_run_effect: decl!("rt_run_effect", [PTR, PTR], [PTR]),
        // (ctx, effect_ptr, cont_ptr) -> ptr
        rt_bind_effect: decl!("rt_bind_effect", [PTR, PTR, PTR], [PTR]),
        // (ctx, value_ptr) -> ptr (wrap value in Effect thunk)
        rt_wrap_effect: decl!("rt_wrap_effect", [PTR, PTR], [PTR]),
        // (ctx) -> void — push resource scope marker
        rt_push_resource_scope: decl!("rt_push_resource_scope", [PTR], []),
        // (ctx) -> void — pop resource scope, run cleanups
        rt_pop_resource_scope: decl!("rt_pop_resource_scope", [PTR], []),
        // (ctx, op_ptr, op_len, lhs_ptr, rhs_ptr) -> ptr
        rt_binary_op: decl!("rt_binary_op", [PTR, PTR, PTR, PTR, PTR], [PTR]),
        // Pattern matching: (ctx, value_ptr, name_ptr, name_len) -> i64
        rt_constructor_name_eq: decl!("rt_constructor_name_eq", [PTR, PTR, PTR, PTR], [PTR]),
        // (ctx, value_ptr) -> i64
        rt_constructor_arity: decl!("rt_constructor_arity", [PTR, PTR], [PTR]),
        // (ctx, value_ptr, index) -> ptr
        rt_constructor_arg: decl!("rt_constructor_arg", [PTR, PTR, PTR], [PTR]),
        // (ctx, value_ptr) -> i64
        rt_tuple_len: decl!("rt_tuple_len", [PTR, PTR], [PTR]),
        // (ctx, value_ptr, index) -> ptr
        rt_tuple_item: decl!("rt_tuple_item", [PTR, PTR, PTR], [PTR]),
        // (ctx, value_ptr) -> i64
        rt_list_len: decl!("rt_list_len", [PTR, PTR], [PTR]),
        // (ctx, value_ptr, start) -> ptr
        rt_list_tail: decl!("rt_list_tail", [PTR, PTR, PTR], [PTR]),
        // (ctx, list_a, list_b) -> ptr
        rt_list_concat: decl!("rt_list_concat", [PTR, PTR, PTR], [PTR]),
        // (ctx, a, b) -> i64
        rt_value_equals: decl!("rt_value_equals", [PTR, PTR, PTR], [PTR]),
        // (ctx, base, names, name_lens, values, len) -> ptr
        rt_patch_record: decl!("rt_patch_record", [PTR, PTR, PTR, PTR, PTR, PTR], [PTR]),
        // Perceus in-place record patch: same signature as rt_patch_record
        rt_patch_record_inplace: decl!(
            "rt_patch_record_inplace",
            [PTR, PTR, PTR, PTR, PTR, PTR],
            [PTR]
        ),
        // (ctx, func_ptr, captured, count) -> ptr
        rt_make_closure: decl!("rt_make_closure", [PTR, PTR, PTR, PTR], [PTR]),
        // Native generate helpers
        // (ctx, gen_ptr) -> ptr
        rt_generator_to_list: decl!("rt_generator_to_list", [PTR, PTR], [PTR]),
        // (ctx) -> ptr
        rt_gen_vec_new: decl!("rt_gen_vec_new", [PTR], [PTR]),
        // (ctx, vec_ptr, value_ptr)
        rt_gen_vec_push: decl!("rt_gen_vec_push", [PTR, PTR, PTR], []),
        // (ctx, vec_ptr, value_ptr) — flatten generator into vec
        rt_gen_vec_extend_generator: decl!("rt_gen_vec_extend_generator", [PTR, PTR, PTR], []),
        // (ctx, vec_ptr) -> ptr
        rt_gen_vec_into_generator: decl!("rt_gen_vec_into_generator", [PTR, PTR], [PTR]),
        // AOT function registration
        // (ctx, name_ptr, name_len, func_ptr, arity)
        rt_register_jit_fn: decl!("rt_register_jit_fn", [PTR, PTR, PTR, PTR, PTR, PTR], []),
        // (ctx, str_ptr, str_len) -> ptr
        rt_alloc_datetime: decl!("rt_alloc_datetime", [PTR, PTR, PTR], [PTR]),
        // (ctx, tag_ptr, tag_len, body_ptr, body_len, flags_ptr, flags_len) -> ptr
        rt_eval_sigil: decl!("rt_eval_sigil", [PTR, PTR, PTR, PTR, PTR, PTR, PTR], [PTR]),
        // Perceus: (ctx, ptr) -> ptr (reuse token or null)
        rt_try_reuse: decl!("rt_try_reuse", [PTR, PTR], [PTR]),
        // Perceus reuse-aware allocation
        // (ctx, token, name_ptr, name_len, args_ptr, args_len) -> ptr
        rt_reuse_constructor: decl!(
            "rt_reuse_constructor",
            [PTR, PTR, PTR, PTR, PTR, PTR],
            [PTR]
        ),
        // (ctx, token, names_ptr, name_lens_ptr, values_ptr, len) -> ptr
        rt_reuse_record: decl!("rt_reuse_record", [PTR, PTR, PTR, PTR, PTR, PTR], [PTR]),
        // (ctx, token, items_ptr, len) -> ptr
        rt_reuse_list: decl!("rt_reuse_list", [PTR, PTR, PTR, PTR], [PTR]),
        // (ctx, token, items_ptr, len) -> ptr
        rt_reuse_tuple: decl!("rt_reuse_tuple", [PTR, PTR, PTR, PTR], [PTR]),
        // (ctx, name_ptr, name_len) -> void
        rt_enter_fn: decl!("rt_enter_fn", [PTR, PTR, PTR], []),
        // (ctx, loc_ptr, loc_len) -> void
        rt_set_location: decl!("rt_set_location", [PTR, PTR, PTR], []),
        // (ctx, path_ptr, path_len) -> ptr (old global value)
        rt_snapshot_mock_install: decl!("rt_snapshot_mock_install", [PTR, PTR, PTR], [PTR]),
        // (ctx, path_ptr, path_len) -> void
        rt_snapshot_mock_flush: decl!("rt_snapshot_mock_flush", [PTR, PTR, PTR], []),
    })
}

/// Module-level function IDs for all runtime helpers.
pub(crate) struct DeclaredHelpers {
    // Call-depth guard
    pub(crate) rt_check_call_depth: cranelift_module::FuncId,
    pub(crate) rt_dec_call_depth: cranelift_module::FuncId,
    // Match failure signaling
    pub(crate) rt_signal_match_fail: cranelift_module::FuncId,
    // Boxing/unboxing
    pub(crate) rt_box_int: cranelift_module::FuncId,
    pub(crate) rt_box_float: cranelift_module::FuncId,
    pub(crate) rt_box_bool: cranelift_module::FuncId,
    pub(crate) rt_unbox_int: cranelift_module::FuncId,
    pub(crate) rt_unbox_float: cranelift_module::FuncId,
    pub(crate) rt_unbox_bool: cranelift_module::FuncId,
    pub(crate) rt_alloc_unit: cranelift_module::FuncId,
    pub(crate) rt_alloc_string: cranelift_module::FuncId,
    pub(crate) rt_alloc_list: cranelift_module::FuncId,
    pub(crate) rt_alloc_tuple: cranelift_module::FuncId,
    pub(crate) rt_alloc_record: cranelift_module::FuncId,
    pub(crate) rt_alloc_constructor: cranelift_module::FuncId,
    pub(crate) rt_record_field: cranelift_module::FuncId,
    pub(crate) rt_list_index: cranelift_module::FuncId,
    pub(crate) rt_clone_value: cranelift_module::FuncId,
    pub(crate) rt_drop_value: cranelift_module::FuncId,
    pub(crate) rt_get_global: cranelift_module::FuncId,
    pub(crate) rt_set_global: cranelift_module::FuncId,
    pub(crate) rt_apply: cranelift_module::FuncId,
    pub(crate) rt_force_thunk: cranelift_module::FuncId,
    pub(crate) rt_run_effect: cranelift_module::FuncId,
    pub(crate) rt_bind_effect: cranelift_module::FuncId,
    pub(crate) rt_wrap_effect: cranelift_module::FuncId,
    // Resource scope management
    pub(crate) rt_push_resource_scope: cranelift_module::FuncId,
    pub(crate) rt_pop_resource_scope: cranelift_module::FuncId,
    pub(crate) rt_binary_op: cranelift_module::FuncId,
    pub(crate) rt_constructor_name_eq: cranelift_module::FuncId,
    pub(crate) rt_constructor_arity: cranelift_module::FuncId,
    pub(crate) rt_constructor_arg: cranelift_module::FuncId,
    pub(crate) rt_tuple_len: cranelift_module::FuncId,
    pub(crate) rt_tuple_item: cranelift_module::FuncId,
    pub(crate) rt_list_len: cranelift_module::FuncId,
    pub(crate) rt_list_tail: cranelift_module::FuncId,
    pub(crate) rt_list_concat: cranelift_module::FuncId,
    pub(crate) rt_value_equals: cranelift_module::FuncId,
    pub(crate) rt_patch_record: cranelift_module::FuncId,
    pub(crate) rt_patch_record_inplace: cranelift_module::FuncId,
    pub(crate) rt_make_closure: cranelift_module::FuncId,
    // Native generate helpers
    pub(crate) rt_generator_to_list: cranelift_module::FuncId,
    pub(crate) rt_gen_vec_new: cranelift_module::FuncId,
    pub(crate) rt_gen_vec_push: cranelift_module::FuncId,
    pub(crate) rt_gen_vec_extend_generator: cranelift_module::FuncId,
    pub(crate) rt_gen_vec_into_generator: cranelift_module::FuncId,
    // AOT function registration
    pub(crate) rt_register_jit_fn: cranelift_module::FuncId,
    // DateTime allocation
    pub(crate) rt_alloc_datetime: cranelift_module::FuncId,
    // Sigil evaluation
    pub(crate) rt_eval_sigil: cranelift_module::FuncId,
    // Perceus reuse
    pub(crate) rt_try_reuse: cranelift_module::FuncId,
    pub(crate) rt_reuse_constructor: cranelift_module::FuncId,
    pub(crate) rt_reuse_record: cranelift_module::FuncId,
    pub(crate) rt_reuse_list: cranelift_module::FuncId,
    pub(crate) rt_reuse_tuple: cranelift_module::FuncId,
    // Function entry tracking for diagnostics
    pub(crate) rt_enter_fn: cranelift_module::FuncId,
    // Source location tracking for diagnostics
    pub(crate) rt_set_location: cranelift_module::FuncId,
    // Snapshot mock helpers
    pub(crate) rt_snapshot_mock_install: cranelift_module::FuncId,
    pub(crate) rt_snapshot_mock_flush: cranelift_module::FuncId,
}

impl DeclaredHelpers {
    /// Import all helper FuncIds into a specific function, producing `FuncRef`s.
    pub(crate) fn import_into(&self, module: &mut impl Module, func: &mut Function) -> HelperRefs {
        macro_rules! imp {
            ($field:ident) => {
                module.declare_func_in_func(self.$field, func)
            };
        }
        HelperRefs {
            rt_check_call_depth: imp!(rt_check_call_depth),
            rt_dec_call_depth: imp!(rt_dec_call_depth),
            rt_signal_match_fail: imp!(rt_signal_match_fail),
            rt_box_int: imp!(rt_box_int),
            rt_box_float: imp!(rt_box_float),
            rt_box_bool: imp!(rt_box_bool),
            rt_unbox_int: imp!(rt_unbox_int),
            rt_unbox_float: imp!(rt_unbox_float),
            rt_unbox_bool: imp!(rt_unbox_bool),
            rt_alloc_unit: imp!(rt_alloc_unit),
            rt_alloc_string: imp!(rt_alloc_string),
            rt_alloc_list: imp!(rt_alloc_list),
            rt_alloc_tuple: imp!(rt_alloc_tuple),
            rt_alloc_record: imp!(rt_alloc_record),
            rt_alloc_constructor: imp!(rt_alloc_constructor),
            rt_record_field: imp!(rt_record_field),
            rt_list_index: imp!(rt_list_index),
            rt_clone_value: imp!(rt_clone_value),
            rt_drop_value: imp!(rt_drop_value),
            rt_get_global: imp!(rt_get_global),
            rt_set_global: imp!(rt_set_global),
            rt_apply: imp!(rt_apply),
            rt_force_thunk: imp!(rt_force_thunk),
            rt_run_effect: imp!(rt_run_effect),
            rt_bind_effect: imp!(rt_bind_effect),
            rt_wrap_effect: imp!(rt_wrap_effect),
            rt_push_resource_scope: imp!(rt_push_resource_scope),
            rt_pop_resource_scope: imp!(rt_pop_resource_scope),
            rt_binary_op: imp!(rt_binary_op),
            rt_constructor_name_eq: imp!(rt_constructor_name_eq),
            rt_constructor_arity: imp!(rt_constructor_arity),
            rt_constructor_arg: imp!(rt_constructor_arg),
            rt_tuple_len: imp!(rt_tuple_len),
            rt_tuple_item: imp!(rt_tuple_item),
            rt_list_len: imp!(rt_list_len),
            rt_list_tail: imp!(rt_list_tail),
            rt_list_concat: imp!(rt_list_concat),
            rt_value_equals: imp!(rt_value_equals),
            rt_patch_record: imp!(rt_patch_record),
            rt_patch_record_inplace: imp!(rt_patch_record_inplace),
            rt_make_closure: imp!(rt_make_closure),
            rt_generator_to_list: imp!(rt_generator_to_list),
            rt_gen_vec_new: imp!(rt_gen_vec_new),
            rt_gen_vec_push: imp!(rt_gen_vec_push),
            rt_gen_vec_extend_generator: imp!(rt_gen_vec_extend_generator),
            rt_gen_vec_into_generator: imp!(rt_gen_vec_into_generator),
            rt_register_jit_fn: imp!(rt_register_jit_fn),
            rt_alloc_datetime: imp!(rt_alloc_datetime),
            rt_eval_sigil: imp!(rt_eval_sigil),
            rt_try_reuse: imp!(rt_try_reuse),
            rt_reuse_constructor: imp!(rt_reuse_constructor),
            rt_reuse_record: imp!(rt_reuse_record),
            rt_reuse_list: imp!(rt_reuse_list),
            rt_reuse_tuple: imp!(rt_reuse_tuple),
            rt_enter_fn: imp!(rt_enter_fn),
            rt_set_location: imp!(rt_set_location),
            rt_snapshot_mock_install: imp!(rt_snapshot_mock_install),
            rt_snapshot_mock_flush: imp!(rt_snapshot_mock_flush),
        }
    }
}

impl<'a, M: Module> LowerCtx<'a, M> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        ctx_param: Value,
        helpers: &'a HelperRefs,
        compiled_lambdas: &'a HashMap<usize, CompiledLambda>,
        jit_funcs: &'a HashMap<String, JitFuncInfo>,
        spec_map: &'a HashMap<String, Vec<String>>,
        module: &'a mut M,
        str_counter: &'a mut usize,
        module_name: &str,
    ) -> Self {
        Self {
            locals: HashMap::new(),
            ctx_param,
            helpers,
            compiled_lambdas,
            jit_funcs,
            spec_map,
            module,
            str_counter,
            str_cache: HashMap::new(),
            use_map: None,
            reuse_token: None,
            module_name: module_name.to_string(),
        }
    }

    /// Attach a Perceus use-analysis map to this lowering context.
    pub(crate) fn set_use_map(&mut self, map: super::use_analysis::UseMap) {
        self.use_map = Some(map);
    }

    /// Take the current reuse token (if any), clearing it so it can only be
    /// consumed once.
    fn take_reuse_token(&mut self) -> Option<Value> {
        self.reuse_token.take()
    }

    /// Insert function params into `locals`, eagerly unboxing scalars when
    /// their types are known from the `CgType` annotation.
    pub(crate) fn bind_typed_params(
        &mut self,
        builder: &mut FunctionBuilder<'_>,
        param_names: &[String],
        block_params: &[Value], // block_params[0] = ctx, [1..] = args
        param_types: &[Option<CgType>],
    ) {
        for (i, name) in param_names.iter().enumerate() {
            let raw = block_params[i + 1]; // +1 to skip ctx
            let tv = if let Some(Some(ty)) = param_types.get(i) {
                self.try_unbox(builder, raw, ty)
                    .unwrap_or_else(|| TypedValue::boxed(raw))
            } else {
                TypedValue::boxed(raw)
            };
            self.locals.insert(name.clone(), tv);
        }
    }

    // -------------------------------------------------------------------
    // String embedding
    // -------------------------------------------------------------------

    /// Embed a string as a data section and return `(ptr_val, len_val)`.
    /// Within a single function, identical strings are deduplicated.
    fn embed_str(&mut self, builder: &mut FunctionBuilder<'_>, s: &[u8]) -> (Value, Value) {
        let len = s.len();
        if let Some(&gv) = self.str_cache.get(s) {
            let ptr = builder.ins().global_value(PTR, gv);
            let len_val = builder.ins().iconst(PTR, len as i64);
            return (ptr, len_val);
        }
        let name = format!("__str_{}", *self.str_counter);
        *self.str_counter += 1;
        let data_id = self
            .module
            .declare_data(&name, Linkage::Local, false, false)
            .expect("declare string data");
        let mut dd = DataDescription::new();
        dd.define(s.to_vec().into_boxed_slice());
        self.module
            .define_data(data_id, &dd)
            .expect("define string data");
        let gv = self.module.declare_data_in_func(data_id, builder.func);
        self.str_cache.insert(s.to_vec(), gv);
        let ptr = builder.ins().global_value(PTR, gv);
        let len_val = builder.ins().iconst(PTR, len as i64);
        (ptr, len_val)
    }

    // -------------------------------------------------------------------
    // Boxing / unboxing helpers
    // -------------------------------------------------------------------

    /// Ensure a `TypedValue` is in boxed (`*mut Value`) representation.
    /// If it is already boxed, return the pointer directly.
    /// If it is an unboxed scalar, emit a boxing call.
    pub(crate) fn ensure_boxed(&self, builder: &mut FunctionBuilder<'_>, tv: TypedValue) -> Value {
        match tv.ty {
            Some(CgType::Int) => {
                let call = builder
                    .ins()
                    .call(self.helpers.rt_box_int, &[self.ctx_param, tv.val]);
                builder.inst_results(call)[0]
            }
            Some(CgType::Float) => {
                // Bitcast f64 → i64 for the C ABI call
                let bits =
                    builder
                        .ins()
                        .bitcast(PTR, cranelift_codegen::ir::MemFlags::new(), tv.val);
                let call = builder
                    .ins()
                    .call(self.helpers.rt_box_float, &[self.ctx_param, bits]);
                builder.inst_results(call)[0]
            }
            Some(CgType::Bool) => {
                let call = builder
                    .ins()
                    .call(self.helpers.rt_box_bool, &[self.ctx_param, tv.val]);
                builder.inst_results(call)[0]
            }
            _ => tv.val, // already boxed
        }
    }

    /// Emit a call to `rt_set_location` to record the current source location for diagnostics.
    pub(crate) fn emit_set_location(&mut self, builder: &mut FunctionBuilder<'_>, location: &str) {
        let (ptr, len) = self.embed_str(builder, location.as_bytes());
        builder
            .ins()
            .call(self.helpers.rt_set_location, &[self.ctx_param, ptr, len]);
    }

    /// Emit a call to `rt_enter_fn` to record the current function name for diagnostics.
    pub(crate) fn emit_enter_fn(&mut self, builder: &mut FunctionBuilder<'_>, fn_name: &str) {
        let (ptr, len) = self.embed_str(builder, fn_name.as_bytes());
        builder
            .ins()
            .call(self.helpers.rt_enter_fn, &[self.ctx_param, ptr, len]);
    }

    /// Try to unbox a boxed `*mut Value` to an unboxed scalar if the target
    /// type is a known scalar.  Returns `None` if the target isn't a scalar.
    fn try_unbox(
        &self,
        builder: &mut FunctionBuilder<'_>,
        ptr: Value,
        target: &CgType,
    ) -> Option<TypedValue> {
        match target {
            CgType::Int => {
                let call = builder
                    .ins()
                    .call(self.helpers.rt_unbox_int, &[self.ctx_param, ptr]);
                Some(TypedValue::typed(
                    builder.inst_results(call)[0],
                    CgType::Int,
                ))
            }
            CgType::Float => {
                let call = builder
                    .ins()
                    .call(self.helpers.rt_unbox_float, &[self.ctx_param, ptr]);
                let bits = builder.inst_results(call)[0];
                let fval = builder
                    .ins()
                    .bitcast(F64, cranelift_codegen::ir::MemFlags::new(), bits);
                Some(TypedValue::typed(fval, CgType::Float))
            }
            CgType::Bool => {
                let call = builder
                    .ins()
                    .call(self.helpers.rt_unbox_bool, &[self.ctx_param, ptr]);
                Some(TypedValue::typed(
                    builder.inst_results(call)[0],
                    CgType::Bool,
                ))
            }
            _ => None,
        }
    }
}

include!("lower/expr.rs");
include!("lower/globals.rs");
