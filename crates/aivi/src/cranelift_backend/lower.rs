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
use cranelift_frontend::{FunctionBuilder, Variable};
use cranelift_module::{DataDescription, Linkage, Module};

use crate::cg_type::CgType;
use crate::rust_ir::{
    RustIrBlockItem, RustIrBlockKind, RustIrExpr, RustIrListItem, RustIrLiteral, RustIrMatchArm,
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
    /// Return `true` when the value is an unboxed scalar in a register.
    #[allow(dead_code)]
    fn is_unboxed_scalar(&self) -> bool {
        matches!(self.ty, Some(CgType::Int | CgType::Float | CgType::Bool))
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
    /// Pre-compiled inner lambda functions, keyed by `*const RustIrExpr`
    /// pointer identity.
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
    pub(crate) rt_gen_vec_into_generator: FuncRef,
    // AOT function registration
    pub(crate) rt_register_jit_fn: FuncRef,
    // AOT machine registration
    pub(crate) rt_register_machines_from_data: FuncRef,
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
    // Function entry tracking for diagnostics
    pub(crate) rt_enter_fn: FuncRef,
    // Source location tracking for diagnostics
    pub(crate) rt_set_location: FuncRef,
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
        // (ctx, vec_ptr) -> ptr
        rt_gen_vec_into_generator: decl!("rt_gen_vec_into_generator", [PTR, PTR], [PTR]),
        // AOT function registration
        // (ctx, name_ptr, name_len, func_ptr, arity)
        rt_register_jit_fn: decl!("rt_register_jit_fn", [PTR, PTR, PTR, PTR, PTR, PTR], []),
        // AOT machine registration: (ctx, data_ptr, data_len) -> void
        rt_register_machines_from_data: decl!(
            "rt_register_machines_from_data",
            [PTR, PTR, PTR],
            []
        ),
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
    pub(crate) rt_gen_vec_into_generator: cranelift_module::FuncId,
    // AOT function registration
    pub(crate) rt_register_jit_fn: cranelift_module::FuncId,
    // AOT machine registration
    pub(crate) rt_register_machines_from_data: cranelift_module::FuncId,
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
            rt_gen_vec_into_generator: imp!(rt_gen_vec_into_generator),
            rt_register_jit_fn: imp!(rt_register_jit_fn),
            rt_register_machines_from_data: imp!(rt_register_machines_from_data),
            rt_alloc_datetime: imp!(rt_alloc_datetime),
            rt_eval_sigil: imp!(rt_eval_sigil),
            rt_try_reuse: imp!(rt_try_reuse),
            rt_reuse_constructor: imp!(rt_reuse_constructor),
            rt_reuse_record: imp!(rt_reuse_record),
            rt_reuse_list: imp!(rt_reuse_list),
            rt_reuse_tuple: imp!(rt_reuse_tuple),
            rt_enter_fn: imp!(rt_enter_fn),
            rt_set_location: imp!(rt_set_location),
        }
    }
}

impl<'a, M: Module> LowerCtx<'a, M> {
    pub(crate) fn new(
        ctx_param: Value,
        helpers: &'a HelperRefs,
        compiled_lambdas: &'a HashMap<usize, CompiledLambda>,
        jit_funcs: &'a HashMap<String, JitFuncInfo>,
        spec_map: &'a HashMap<String, Vec<String>>,
        module: &'a mut M,
        str_counter: &'a mut usize,
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

    /// Lower a `RustIrExpr` to a typed Cranelift value.
    pub(crate) fn lower_expr(
        &mut self,
        builder: &mut FunctionBuilder<'_>,
        expr: &RustIrExpr,
    ) -> TypedValue {
        match expr {
            // ----- Literals -----
            RustIrExpr::LitNumber { text, .. } => self.lower_lit_number(builder, text),
            RustIrExpr::LitString { text, .. } => self.lower_lit_string(builder, text),
            RustIrExpr::LitBool { value, .. } => self.lower_lit_bool(builder, *value),
            RustIrExpr::LitDateTime { text, .. } => {
                let (str_ptr, str_len) = self.embed_str(builder, text.as_bytes());
                let inst = builder.ins().call(
                    self.helpers.rt_alloc_datetime,
                    &[self.ctx_param, str_ptr, str_len],
                );
                TypedValue::boxed(builder.inst_results(inst)[0])
            }
            RustIrExpr::LitSigil {
                tag, body, flags, ..
            } => {
                let (tag_ptr, tag_len) = self.embed_str(builder, tag.as_bytes());
                let (body_ptr, body_len) = self.embed_str(builder, body.as_bytes());
                let (flags_ptr, flags_len) = self.embed_str(builder, flags.as_bytes());
                let inst = builder.ins().call(
                    self.helpers.rt_eval_sigil,
                    &[
                        self.ctx_param,
                        tag_ptr,
                        tag_len,
                        body_ptr,
                        body_len,
                        flags_ptr,
                        flags_len,
                    ],
                );
                TypedValue::boxed(builder.inst_results(inst)[0])
            }
            RustIrExpr::TextInterpolate { parts, .. } => {
                self.lower_text_interpolate(builder, parts)
            }

            // ----- Variables -----
            RustIrExpr::Local { name, .. } => self.lower_local(builder, name),
            RustIrExpr::Global { name, .. } => self.lower_global(builder, name),
            RustIrExpr::Builtin { builtin, .. } => self.lower_global(builder, builtin),
            RustIrExpr::ConstructorValue { name, .. } => {
                self.lower_constructor_value(builder, name)
            }

            // ----- Functions -----
            RustIrExpr::Lambda { .. } => self.lower_lambda_expr(builder, expr),
            RustIrExpr::App { func, arg, .. } => self.lower_app(builder, func, arg),
            RustIrExpr::Call { func, args, .. } => self.lower_call(builder, func, args),

            // ----- Data structures -----
            RustIrExpr::List { items, .. } => self.lower_list(builder, items),
            RustIrExpr::Tuple { items, .. } => self.lower_tuple(builder, items),
            RustIrExpr::Record { fields, .. } => self.lower_record(builder, fields),
            RustIrExpr::Patch { target, fields, .. } => self.lower_patch(builder, target, fields),

            // ----- Access -----
            RustIrExpr::FieldAccess { base, field, .. } => {
                self.lower_field_access(builder, base, field)
            }
            RustIrExpr::Index {
                base,
                index,
                location,
                ..
            } => self.lower_index(builder, base, index, location.as_deref()),

            // ----- Control flow -----
            RustIrExpr::If {
                cond,
                then_branch,
                else_branch,
                ..
            } => self.lower_if(builder, cond, then_branch, else_branch),
            RustIrExpr::Match {
                scrutinee, arms, ..
            } => self.lower_match(builder, scrutinee, arms),
            RustIrExpr::Binary {
                op, left, right, ..
            } => self.lower_binary(builder, op, left, right),

            // ----- Blocks -----
            RustIrExpr::Block {
                block_kind, items, ..
            } => self.lower_block(builder, block_kind, items),
            RustIrExpr::Pipe { func, arg, .. } => self.lower_app(builder, func, arg),

            // ----- Special -----
            RustIrExpr::DebugFn { body, .. } => self.lower_expr(builder, body),
            RustIrExpr::Raw { text, .. } => self.lower_lit_string(builder, text),
        }
    }

    // -----------------------------------------------------------------------
    // Literal lowering
    // -----------------------------------------------------------------------

    fn lower_lit_number(&mut self, builder: &mut FunctionBuilder<'_>, text: &str) -> TypedValue {
        if let Ok(int_val) = text.parse::<i64>() {
            // Unboxed: keep the i64 in a register
            let v = builder.ins().iconst(PTR, int_val);
            TypedValue::typed(v, CgType::Int)
        } else if let Ok(float_val) = text.parse::<f64>() {
            let v = builder.ins().f64const(float_val);
            TypedValue::typed(v, CgType::Float)
        } else {
            // Fallback: treat as string (for BigInt, Rational, Decimal, etc.)
            self.lower_lit_string(builder, text)
        }
    }

    fn lower_lit_string(&mut self, builder: &mut FunctionBuilder<'_>, text: &str) -> TypedValue {
        let (ptr_val, len_val) = self.embed_str(builder, text.as_bytes());
        let call = builder.ins().call(
            self.helpers.rt_alloc_string,
            &[self.ctx_param, ptr_val, len_val],
        );
        TypedValue::boxed(builder.inst_results(call)[0])
    }

    fn lower_lit_bool(&mut self, builder: &mut FunctionBuilder<'_>, value: bool) -> TypedValue {
        // Unboxed: keep the bool as i64 0/1 in a register
        let v = builder.ins().iconst(PTR, i64::from(value));
        TypedValue::typed(v, CgType::Bool)
    }

    fn lower_text_interpolate(
        &mut self,
        builder: &mut FunctionBuilder<'_>,
        parts: &[RustIrTextPart],
    ) -> TypedValue {
        // Build interpolated string by concatenating parts via rt_binary_op "++".
        if parts.is_empty() {
            return self.lower_lit_string(builder, "");
        }
        let mut result = match &parts[0] {
            RustIrTextPart::Text { text } => self.lower_lit_string(builder, text),
            RustIrTextPart::Expr { expr } => self.lower_expr(builder, expr),
        };
        let (op_ptr, op_len) = self.embed_str(builder, b"++");
        for part in &parts[1..] {
            let part_tv = match part {
                RustIrTextPart::Text { text } => self.lower_lit_string(builder, text),
                RustIrTextPart::Expr { expr } => self.lower_expr(builder, expr),
            };
            let lhs = self.ensure_boxed(builder, result);
            let rhs = self.ensure_boxed(builder, part_tv);
            let call = builder.ins().call(
                self.helpers.rt_binary_op,
                &[self.ctx_param, op_ptr, op_len, lhs, rhs],
            );
            result = TypedValue::boxed(builder.inst_results(call)[0]);
        }
        result
    }

    // -----------------------------------------------------------------------
    // Variable lowering
    // -----------------------------------------------------------------------

    fn lower_local(&mut self, builder: &mut FunctionBuilder<'_>, name: &str) -> TypedValue {
        if let Some(tv) = self.locals.get(name) {
            tv.clone()
        } else {
            // Fallback: treat as global lookup
            self.lower_global(builder, name)
        }
    }

    fn lower_global(&mut self, builder: &mut FunctionBuilder<'_>, name: &str) -> TypedValue {
        let (name_ptr, name_len) = self.embed_str(builder, name.as_bytes());
        let call = builder.ins().call(
            self.helpers.rt_get_global,
            &[self.ctx_param, name_ptr, name_len],
        );
        TypedValue::boxed(builder.inst_results(call)[0])
    }

    pub(crate) fn emit_set_global(
        &mut self,
        builder: &mut FunctionBuilder<'_>,
        name: &str,
        value: cranelift_codegen::ir::Value,
    ) {
        let (name_ptr, name_len) = self.embed_str(builder, name.as_bytes());
        builder.ins().call(
            self.helpers.rt_set_global,
            &[self.ctx_param, name_ptr, name_len, value],
        );
    }

    /// Lower a bare constructor reference (e.g. `Sqlite`, `GtkAttribute`) by
    /// allocating a zero-arg `Value::Constructor` directly instead of looking
    /// it up in the globals map.  When applied to arguments later, `apply()`
    /// accumulates them naturally.
    fn lower_constructor_value(
        &mut self,
        builder: &mut FunctionBuilder<'_>,
        name: &str,
    ) -> TypedValue {
        let (name_ptr, name_len) = self.embed_str(builder, name.as_bytes());
        let null = builder.ins().iconst(PTR, 0);
        let zero = builder.ins().iconst(PTR, 0);
        // Perceus: use reuse token if available
        if let Some(token) = self.take_reuse_token() {
            let call = builder.ins().call(
                self.helpers.rt_reuse_constructor,
                &[self.ctx_param, token, name_ptr, name_len, null, zero],
            );
            return TypedValue::boxed(builder.inst_results(call)[0]);
        }
        let call = builder.ins().call(
            self.helpers.rt_alloc_constructor,
            &[self.ctx_param, name_ptr, name_len, null, zero],
        );
        TypedValue::boxed(builder.inst_results(call)[0])
    }

    // -----------------------------------------------------------------------
    // Function lowering
    // -----------------------------------------------------------------------

    /// Lower a Lambda expression by looking up its pre-compiled global Builtin
    /// and partially applying captured variables.
    fn lower_lambda_expr(
        &mut self,
        builder: &mut FunctionBuilder<'_>,
        expr: &RustIrExpr,
    ) -> TypedValue {
        let key = expr as *const RustIrExpr as usize;
        if let Some(cl) = self.compiled_lambdas.get(&key) {
            // Look up the pre-compiled function from globals
            let mut result = self.lower_global(builder, cl.global_name);
            // Partially apply captured values one by one via rt_apply
            for var_name in &cl.captured_vars {
                let tv = if let Some(v) = self.locals.get(var_name) {
                    v.clone()
                } else {
                    self.lower_global(builder, var_name)
                };
                let func_ptr = self.ensure_boxed(builder, result);
                let arg_ptr = self.ensure_boxed(builder, tv);
                let call = builder
                    .ins()
                    .call(self.helpers.rt_apply, &[self.ctx_param, func_ptr, arg_ptr]);
                result = TypedValue::boxed(builder.inst_results(call)[0]);
            }
            return result;
        }

        // Fallback: look up from globals or return unit
        let call = builder
            .ins()
            .call(self.helpers.rt_alloc_unit, &[self.ctx_param]);
        TypedValue::boxed(builder.inst_results(call)[0])
    }

    fn lower_app(
        &mut self,
        builder: &mut FunctionBuilder<'_>,
        func: &RustIrExpr,
        arg: &RustIrExpr,
    ) -> TypedValue {
        // Check for direct call: App(Global(name), arg) where name is a JIT function with arity 1
        if let RustIrExpr::Global { name, .. } = func {
            let maybe_info = self.jit_funcs.get(name.as_str()).cloned();
            if let Some(info) = maybe_info {
                if info.arity == 1 {
                    // Try specialization routing
                    let maybe_specs = self.spec_map.get(name.as_str()).cloned();
                    if let Some(spec_names) = maybe_specs {
                        let arg_tv = self.lower_expr(builder, arg);
                        let arg_tvs = [arg_tv];
                        if let Some(spec_name) = self.find_matching_spec(&spec_names, &arg_tvs) {
                            if let Some(si) = self.jit_funcs.get(spec_name.as_str()).cloned() {
                                return self.emit_direct_call_typed(builder, &si, &arg_tvs);
                            }
                        }
                        return self.emit_direct_call_typed(builder, &info, &arg_tvs);
                    }
                    let args_vec = vec![arg.clone()];
                    return self.emit_direct_call(builder, &info, &args_vec);
                }
            }
        }

        let func_tv = self.lower_expr(builder, func);
        let arg_tv = self.lower_expr(builder, arg);
        let func_val = self.ensure_boxed(builder, func_tv);
        let arg_val = self.ensure_boxed(builder, arg_tv);
        let call = builder
            .ins()
            .call(self.helpers.rt_apply, &[self.ctx_param, func_val, arg_val]);
        TypedValue::boxed(builder.inst_results(call)[0])
    }

    fn lower_call(
        &mut self,
        builder: &mut FunctionBuilder<'_>,
        func: &RustIrExpr,
        args: &[RustIrExpr],
    ) -> TypedValue {
        // Check for direct call: Call(Global(name), args) where name is a
        // JIT function with matching arity.
        if let RustIrExpr::Global { name, .. } = func {
            let maybe_info = self.jit_funcs.get(name.as_str()).cloned();
            if let Some(info) = maybe_info {
                if info.arity == args.len() {
                    // Try specialization routing: lower args first to get types,
                    // then check for a matching specialization.
                    let maybe_specs = self.spec_map.get(name.as_str()).cloned();
                    if let Some(spec_names) = maybe_specs {
                        let arg_tvs: Vec<TypedValue> =
                            args.iter().map(|a| self.lower_expr(builder, a)).collect();
                        if let Some(spec_name) = self.find_matching_spec(&spec_names, &arg_tvs) {
                            if let Some(si) = self.jit_funcs.get(spec_name.as_str()).cloned() {
                                return self.emit_direct_call_typed(builder, &si, &arg_tvs);
                            }
                        }
                        // No matching specialization; use the args we already lowered
                        return self.emit_direct_call_typed(builder, &info, &arg_tvs);
                    }
                    return self.emit_direct_call(builder, &info, args);
                }
            }
        }

        // Fallback: chained rt_apply
        let mut result = self.lower_expr(builder, func);
        for arg in args {
            let arg_tv = self.lower_expr(builder, arg);
            let func_val = self.ensure_boxed(builder, result);
            let arg_val = self.ensure_boxed(builder, arg_tv);
            let call = builder
                .ins()
                .call(self.helpers.rt_apply, &[self.ctx_param, func_val, arg_val]);
            result = TypedValue::boxed(builder.inst_results(call)[0]);
        }
        result
    }

    /// Find a specialization whose param types match the given arg types.
    /// Returns the specialization name rather than a reference, to avoid
    /// keeping an immutable borrow on `self`.
    fn find_matching_spec(&self, spec_names: &[String], arg_tvs: &[TypedValue]) -> Option<String> {
        for spec_name in spec_names {
            if let Some(spec_info) = self.jit_funcs.get(spec_name.as_str()) {
                if spec_info.arity == arg_tvs.len() && Self::params_match_args(spec_info, arg_tvs) {
                    return Some(spec_name.clone());
                }
            }
        }
        None
    }

    /// Check if a specialization's declared param types match the actual arg types.
    fn params_match_args(info: &JitFuncInfo, arg_tvs: &[TypedValue]) -> bool {
        for (i, arg_tv) in arg_tvs.iter().enumerate() {
            let param_ty = info.param_types.get(i).and_then(|t| t.as_ref());
            match (param_ty, &arg_tv.ty) {
                (Some(p), Some(a)) if p == a => continue,
                (None, None) => continue,
                _ => return false,
            }
        }
        true
    }

    /// Emit a direct Cranelift `call` to a JIT-compiled function, bypassing
    /// `rt_get_global` + `rt_apply`. Arguments are boxed at the call site
    /// since the callee's ABI is still all-`PTR`.
    fn emit_direct_call(
        &mut self,
        builder: &mut FunctionBuilder<'_>,
        info: &JitFuncInfo,
        args: &[RustIrExpr],
    ) -> TypedValue {
        let mut call_args = vec![self.ctx_param];
        for arg_expr in args.iter() {
            let arg_tv = self.lower_expr(builder, arg_expr);
            // Always box for the all-PTR ABI
            call_args.push(self.ensure_boxed(builder, arg_tv));
        }
        let call = builder.ins().call(info.func_ref, &call_args);
        let raw = builder.inst_results(call)[0];
        match &info.return_type {
            Some(ret_ty) => {
                // Callee returned a boxed value; we know the type so unbox it
                self.try_unbox(builder, raw, ret_ty)
                    .unwrap_or_else(|| TypedValue::boxed(raw))
            }
            None => TypedValue::boxed(raw),
        }
    }

    /// Like `emit_direct_call` but takes pre-lowered TypedValues instead of
    /// unevaluated expressions. Used for specialization routing where args
    /// are already lowered to determine their types.
    fn emit_direct_call_typed(
        &mut self,
        builder: &mut FunctionBuilder<'_>,
        info: &JitFuncInfo,
        arg_tvs: &[TypedValue],
    ) -> TypedValue {
        let mut call_args = vec![self.ctx_param];
        for arg_tv in arg_tvs {
            call_args.push(self.ensure_boxed(builder, arg_tv.clone()));
        }
        let call = builder.ins().call(info.func_ref, &call_args);
        let raw = builder.inst_results(call)[0];
        match &info.return_type {
            Some(ret_ty) => self
                .try_unbox(builder, raw, ret_ty)
                .unwrap_or_else(|| TypedValue::boxed(raw)),
            None => TypedValue::boxed(raw),
        }
    }

    // -----------------------------------------------------------------------
    // Data structure lowering
    // -----------------------------------------------------------------------

    fn lower_list(
        &mut self,
        builder: &mut FunctionBuilder<'_>,
        items: &[RustIrListItem],
    ) -> TypedValue {
        // Check if any items use spread
        let has_spread = items.iter().any(|i| i.spread);

        if !has_spread {
            // Fast path: no spreads, allocate a flat list
            let count = items.len();
            if count == 0 {
                let null = builder.ins().iconst(PTR, 0);
                let zero = builder.ins().iconst(PTR, 0);
                let call = builder
                    .ins()
                    .call(self.helpers.rt_alloc_list, &[self.ctx_param, null, zero]);
                return TypedValue::boxed(builder.inst_results(call)[0]);
            }

            let slot = builder.create_sized_stack_slot(cranelift_codegen::ir::StackSlotData::new(
                cranelift_codegen::ir::StackSlotKind::ExplicitSlot,
                (count * 8) as u32,
                0,
            ));
            for (i, item) in items.iter().enumerate() {
                let tv = self.lower_expr(builder, &item.expr);
                let boxed = self.ensure_boxed(builder, tv);
                builder.ins().stack_store(boxed, slot, (i * 8) as i32);
            }
            let arr_ptr = builder.ins().stack_addr(PTR, slot, 0);
            let len = builder.ins().iconst(PTR, count as i64);
            // Perceus: use reuse token if available
            if let Some(token) = self.take_reuse_token() {
                let call = builder.ins().call(
                    self.helpers.rt_reuse_list,
                    &[self.ctx_param, token, arr_ptr, len],
                );
                return TypedValue::boxed(builder.inst_results(call)[0]);
            }
            let call = builder
                .ins()
                .call(self.helpers.rt_alloc_list, &[self.ctx_param, arr_ptr, len]);
            TypedValue::boxed(builder.inst_results(call)[0])
        } else {
            // Spread path: group items into chunks of non-spread items and spread items,
            // build each chunk as a list, then concatenate with rt_list_concat.
            let null = builder.ins().iconst(PTR, 0);
            let zero = builder.ins().iconst(PTR, 0);
            let empty_call = builder
                .ins()
                .call(self.helpers.rt_alloc_list, &[self.ctx_param, null, zero]);
            let mut result = builder.inst_results(empty_call)[0];

            // Collect consecutive non-spread items into a chunk, flush when we hit a spread
            let mut chunk: Vec<cranelift_codegen::ir::Value> = Vec::new();
            for item in items {
                if item.spread {
                    // Flush any accumulated non-spread chunk
                    if !chunk.is_empty() {
                        let chunk_list = self.build_list_from_values(builder, &chunk);
                        let call = builder.ins().call(
                            self.helpers.rt_list_concat,
                            &[self.ctx_param, result, chunk_list],
                        );
                        result = builder.inst_results(call)[0];
                        chunk.clear();
                    }
                    // Concat the spread expression (which should evaluate to a list)
                    let spread_val = self.lower_expr(builder, &item.expr);
                    let spread_boxed = self.ensure_boxed(builder, spread_val);
                    let call = builder.ins().call(
                        self.helpers.rt_list_concat,
                        &[self.ctx_param, result, spread_boxed],
                    );
                    result = builder.inst_results(call)[0];
                } else {
                    let tv = self.lower_expr(builder, &item.expr);
                    let boxed = self.ensure_boxed(builder, tv);
                    chunk.push(boxed);
                }
            }
            // Flush any remaining non-spread chunk
            if !chunk.is_empty() {
                let chunk_list = self.build_list_from_values(builder, &chunk);
                let call = builder.ins().call(
                    self.helpers.rt_list_concat,
                    &[self.ctx_param, result, chunk_list],
                );
                result = builder.inst_results(call)[0];
            }
            TypedValue::boxed(result)
        }
    }

    fn build_list_from_values(
        &mut self,
        builder: &mut FunctionBuilder<'_>,
        values: &[cranelift_codegen::ir::Value],
    ) -> cranelift_codegen::ir::Value {
        let count = values.len();
        let slot = builder.create_sized_stack_slot(cranelift_codegen::ir::StackSlotData::new(
            cranelift_codegen::ir::StackSlotKind::ExplicitSlot,
            (count * 8) as u32,
            0,
        ));
        for (i, &val) in values.iter().enumerate() {
            builder.ins().stack_store(val, slot, (i * 8) as i32);
        }
        let arr_ptr = builder.ins().stack_addr(PTR, slot, 0);
        let len = builder.ins().iconst(PTR, count as i64);
        let call = builder
            .ins()
            .call(self.helpers.rt_alloc_list, &[self.ctx_param, arr_ptr, len]);
        builder.inst_results(call)[0]
    }

    fn lower_tuple(
        &mut self,
        builder: &mut FunctionBuilder<'_>,
        items: &[RustIrExpr],
    ) -> TypedValue {
        let count = items.len();
        if count == 0 {
            let call = builder
                .ins()
                .call(self.helpers.rt_alloc_unit, &[self.ctx_param]);
            return TypedValue::boxed(builder.inst_results(call)[0]);
        }
        let slot = builder.create_sized_stack_slot(cranelift_codegen::ir::StackSlotData::new(
            cranelift_codegen::ir::StackSlotKind::ExplicitSlot,
            (count * 8) as u32,
            0,
        ));
        for (i, item) in items.iter().enumerate() {
            let tv = self.lower_expr(builder, item);
            let boxed = self.ensure_boxed(builder, tv);
            builder.ins().stack_store(boxed, slot, (i * 8) as i32);
        }
        let arr_ptr = builder.ins().stack_addr(PTR, slot, 0);
        let len = builder.ins().iconst(PTR, count as i64);
        // Perceus: use reuse token if available
        if let Some(token) = self.take_reuse_token() {
            let call = builder.ins().call(
                self.helpers.rt_reuse_tuple,
                &[self.ctx_param, token, arr_ptr, len],
            );
            return TypedValue::boxed(builder.inst_results(call)[0]);
        }
        let call = builder
            .ins()
            .call(self.helpers.rt_alloc_tuple, &[self.ctx_param, arr_ptr, len]);
        TypedValue::boxed(builder.inst_results(call)[0])
    }

    fn lower_record(
        &mut self,
        builder: &mut FunctionBuilder<'_>,
        fields: &[RustIrRecordField],
    ) -> TypedValue {
        let count = fields.len();
        if count == 0 {
            let null = builder.ins().iconst(PTR, 0);
            let zero = builder.ins().iconst(PTR, 0);
            let call = builder.ins().call(
                self.helpers.rt_alloc_record,
                &[self.ctx_param, null, null, null, zero],
            );
            return TypedValue::boxed(builder.inst_results(call)[0]);
        }

        // Stack slots for name pointers, name lengths, and value pointers
        let names_slot =
            builder.create_sized_stack_slot(cranelift_codegen::ir::StackSlotData::new(
                cranelift_codegen::ir::StackSlotKind::ExplicitSlot,
                (count * 8) as u32,
                0,
            ));
        let lens_slot = builder.create_sized_stack_slot(cranelift_codegen::ir::StackSlotData::new(
            cranelift_codegen::ir::StackSlotKind::ExplicitSlot,
            (count * 8) as u32,
            0,
        ));
        let vals_slot = builder.create_sized_stack_slot(cranelift_codegen::ir::StackSlotData::new(
            cranelift_codegen::ir::StackSlotKind::ExplicitSlot,
            (count * 8) as u32,
            0,
        ));

        for (i, field) in fields.iter().enumerate() {
            let field_name = field
                .path
                .first()
                .map(|seg| match seg {
                    crate::rust_ir::RustIrPathSegment::Field(name) => name.as_str(),
                    _ => "_",
                })
                .unwrap_or("_");
            let (name_ptr, name_len) = self.embed_str(builder, field_name.as_bytes());
            builder
                .ins()
                .stack_store(name_ptr, names_slot, (i * 8) as i32);
            builder
                .ins()
                .stack_store(name_len, lens_slot, (i * 8) as i32);
            let tv = self.lower_expr(builder, &field.value);
            let boxed = self.ensure_boxed(builder, tv);
            builder.ins().stack_store(boxed, vals_slot, (i * 8) as i32);
        }

        let names_ptr = builder.ins().stack_addr(PTR, names_slot, 0);
        let lens_ptr = builder.ins().stack_addr(PTR, lens_slot, 0);
        let vals_ptr = builder.ins().stack_addr(PTR, vals_slot, 0);
        let len = builder.ins().iconst(PTR, count as i64);
        // Perceus: use reuse token if available
        if let Some(token) = self.take_reuse_token() {
            let call = builder.ins().call(
                self.helpers.rt_reuse_record,
                &[self.ctx_param, token, names_ptr, lens_ptr, vals_ptr, len],
            );
            return TypedValue::boxed(builder.inst_results(call)[0]);
        }
        let call = builder.ins().call(
            self.helpers.rt_alloc_record,
            &[self.ctx_param, names_ptr, lens_ptr, vals_ptr, len],
        );
        TypedValue::boxed(builder.inst_results(call)[0])
    }

    fn lower_patch(
        &mut self,
        builder: &mut FunctionBuilder<'_>,
        target: &RustIrExpr,
        fields: &[RustIrRecordField],
    ) -> TypedValue {
        // Perceus: check if target is a last-use local before lowering it
        let target_is_last_use = self.is_last_use_local(target);

        let base_tv = self.lower_expr(builder, target);
        let base = self.ensure_boxed(builder, base_tv);
        let count = fields.len();
        if count == 0 {
            return TypedValue::boxed(base);
        }

        // Build overlay: same layout as lower_record but call rt_patch_record
        let names_slot =
            builder.create_sized_stack_slot(cranelift_codegen::ir::StackSlotData::new(
                cranelift_codegen::ir::StackSlotKind::ExplicitSlot,
                (count * 8) as u32,
                0,
            ));
        let lens_slot = builder.create_sized_stack_slot(cranelift_codegen::ir::StackSlotData::new(
            cranelift_codegen::ir::StackSlotKind::ExplicitSlot,
            (count * 8) as u32,
            0,
        ));
        let vals_slot = builder.create_sized_stack_slot(cranelift_codegen::ir::StackSlotData::new(
            cranelift_codegen::ir::StackSlotKind::ExplicitSlot,
            (count * 8) as u32,
            0,
        ));

        for (i, field) in fields.iter().enumerate() {
            let field_name = field
                .path
                .first()
                .map(|seg| match seg {
                    crate::rust_ir::RustIrPathSegment::Field(name) => name.as_str(),
                    _ => "_",
                })
                .unwrap_or("_");
            let (name_ptr, name_len) = self.embed_str(builder, field_name.as_bytes());
            builder
                .ins()
                .stack_store(name_ptr, names_slot, (i * 8) as i32);
            builder
                .ins()
                .stack_store(name_len, lens_slot, (i * 8) as i32);
            let tv = self.lower_expr(builder, &field.value);
            let boxed = self.ensure_boxed(builder, tv);
            builder.ins().stack_store(boxed, vals_slot, (i * 8) as i32);
        }

        let names_ptr = builder.ins().stack_addr(PTR, names_slot, 0);
        let lens_ptr = builder.ins().stack_addr(PTR, lens_slot, 0);
        let vals_ptr = builder.ins().stack_addr(PTR, vals_slot, 0);
        let len = builder.ins().iconst(PTR, count as i64);
        // Perceus: use in-place patching when target is consumed
        let helper = if target_is_last_use {
            self.helpers.rt_patch_record_inplace
        } else {
            self.helpers.rt_patch_record
        };
        let call = builder.ins().call(
            helper,
            &[self.ctx_param, base, names_ptr, lens_ptr, vals_ptr, len],
        );
        TypedValue::boxed(builder.inst_results(call)[0])
    }

    // -----------------------------------------------------------------------
    // Access lowering
    // -----------------------------------------------------------------------

    fn lower_field_access(
        &mut self,
        builder: &mut FunctionBuilder<'_>,
        base: &RustIrExpr,
        field: &str,
    ) -> TypedValue {
        let base_tv = self.lower_expr(builder, base);
        let base_val = self.ensure_boxed(builder, base_tv);
        let (name_ptr, name_len) = self.embed_str(builder, field.as_bytes());
        let call = builder.ins().call(
            self.helpers.rt_record_field,
            &[self.ctx_param, base_val, name_ptr, name_len],
        );
        TypedValue::boxed(builder.inst_results(call)[0])
    }

    fn lower_index(
        &mut self,
        builder: &mut FunctionBuilder<'_>,
        base: &RustIrExpr,
        index: &RustIrExpr,
        location: Option<&str>,
    ) -> TypedValue {
        if let Some(loc) = location {
            self.emit_set_location(builder, loc);
        }
        let base_tv = self.lower_expr(builder, base);
        let base_val = self.ensure_boxed(builder, base_tv);
        let idx_tv = self.lower_expr(builder, index);
        // Unbox the index to get an i64 — if already unboxed Int, use directly
        let idx_int = if matches!(idx_tv.ty, Some(CgType::Int)) {
            idx_tv.val
        } else {
            let idx_boxed = self.ensure_boxed(builder, idx_tv);
            let call = builder
                .ins()
                .call(self.helpers.rt_unbox_int, &[self.ctx_param, idx_boxed]);
            builder.inst_results(call)[0]
        };
        let call = builder.ins().call(
            self.helpers.rt_list_index,
            &[self.ctx_param, base_val, idx_int],
        );
        TypedValue::boxed(builder.inst_results(call)[0])
    }

    // -----------------------------------------------------------------------
    // Control flow lowering
    // -----------------------------------------------------------------------

    fn lower_if(
        &mut self,
        builder: &mut FunctionBuilder<'_>,
        cond: &RustIrExpr,
        then_branch: &RustIrExpr,
        else_branch: &RustIrExpr,
    ) -> TypedValue {
        let cond_tv = self.lower_expr(builder, cond);
        // Get a bool condition — if already unboxed Bool, use directly
        let cond_int = if matches!(cond_tv.ty, Some(CgType::Bool)) {
            cond_tv.val
        } else {
            let cond_boxed = self.ensure_boxed(builder, cond_tv);
            let call = builder
                .ins()
                .call(self.helpers.rt_unbox_bool, &[self.ctx_param, cond_boxed]);
            builder.inst_results(call)[0]
        };
        let cond_bool = builder.ins().icmp_imm(
            cranelift_codegen::ir::condcodes::IntCC::NotEqual,
            cond_int,
            0,
        );

        let then_block = builder.create_block();
        let else_block = builder.create_block();
        let merge_block = builder.create_block();

        // Use a Cranelift variable to communicate the result across blocks.
        // Both branches return boxed values to ensure uniform type in the merge.
        let result_var = builder.declare_var(PTR);

        builder
            .ins()
            .brif(cond_bool, then_block, &[], else_block, &[]);

        builder.switch_to_block(then_block);
        builder.seal_block(then_block);
        let then_tv = self.lower_expr(builder, then_branch);
        let then_boxed = self.ensure_boxed(builder, then_tv);
        builder.def_var(result_var, then_boxed);
        builder.ins().jump(merge_block, &[]);

        builder.switch_to_block(else_block);
        builder.seal_block(else_block);
        let else_tv = self.lower_expr(builder, else_branch);
        let else_boxed = self.ensure_boxed(builder, else_tv);
        builder.def_var(result_var, else_boxed);
        builder.ins().jump(merge_block, &[]);

        builder.switch_to_block(merge_block);
        builder.seal_block(merge_block);
        TypedValue::boxed(builder.use_var(result_var))
    }

    fn lower_match(
        &mut self,
        builder: &mut FunctionBuilder<'_>,
        scrutinee: &RustIrExpr,
        arms: &[RustIrMatchArm],
    ) -> TypedValue {
        // Perceus: check if the scrutinee is a last-used local variable.
        // If so, we can reuse its box allocation for the arm body's result.
        let scrut_is_last_use = self.is_last_use_local(scrutinee);

        let scrut_tv = self.lower_expr(builder, scrutinee);
        let scrut_val = self.ensure_boxed(builder, scrut_tv);

        if arms.is_empty() {
            let call = builder
                .ins()
                .call(self.helpers.rt_alloc_unit, &[self.ctx_param]);
            return TypedValue::boxed(builder.inst_results(call)[0]);
        }

        // Chained if-else: for each arm, test pattern → body, else → next arm
        let merge_block = builder.create_block();
        let result_var = builder.declare_var(PTR);

        // Initialize result to unit (for safety)
        let unit = {
            let call = builder
                .ins()
                .call(self.helpers.rt_alloc_unit, &[self.ctx_param]);
            builder.inst_results(call)[0]
        };
        builder.def_var(result_var, unit);

        for arm in arms {
            let arm_body_block = builder.create_block();
            let arm_next_block = builder.create_block();

            // Test the pattern — emits a boolean (i64: 0 or 1)
            let matched = self.emit_pattern_test(builder, &arm.pattern, scrut_val);
            let matched_bool = builder.ins().icmp_imm(
                cranelift_codegen::ir::condcodes::IntCC::NotEqual,
                matched,
                0,
            );
            builder
                .ins()
                .brif(matched_bool, arm_body_block, &[], arm_next_block, &[]);

            // Arm body block
            builder.switch_to_block(arm_body_block);
            builder.seal_block(arm_body_block);
            // Bind pattern variables
            self.bind_pattern(builder, &arm.pattern, scrut_val);

            // Perceus: if the scrutinee is consumed, generate a reuse token.
            // The pattern has already extracted all needed fields, so the
            // scrutinee's box can be recycled for the next allocation.
            if scrut_is_last_use {
                let call = builder
                    .ins()
                    .call(self.helpers.rt_try_reuse, &[self.ctx_param, scrut_val]);
                self.reuse_token = Some(builder.inst_results(call)[0]);
            }

            // Guard check (if present)
            if let Some(guard) = &arm.guard {
                let guard_tv = self.lower_expr(builder, guard);
                let guard_int = if matches!(guard_tv.ty, Some(CgType::Bool)) {
                    guard_tv.val
                } else {
                    let guard_boxed = self.ensure_boxed(builder, guard_tv);
                    let call = builder
                        .ins()
                        .call(self.helpers.rt_unbox_bool, &[self.ctx_param, guard_boxed]);
                    builder.inst_results(call)[0]
                };
                let guard_bool = builder.ins().icmp_imm(
                    cranelift_codegen::ir::condcodes::IntCC::NotEqual,
                    guard_int,
                    0,
                );
                let guard_pass_block = builder.create_block();
                builder
                    .ins()
                    .brif(guard_bool, guard_pass_block, &[], arm_next_block, &[]);
                builder.switch_to_block(guard_pass_block);
                builder.seal_block(guard_pass_block);
            }

            let body_tv = self.lower_expr(builder, &arm.body);
            let body_boxed = self.ensure_boxed(builder, body_tv);
            // Clear any unconsumed reuse token (it won't survive the branch)
            self.reuse_token = None;
            builder.def_var(result_var, body_boxed);
            builder.ins().jump(merge_block, &[]);

            // Next arm block
            builder.switch_to_block(arm_next_block);
            builder.seal_block(arm_next_block);
        }

        // Fallthrough: non-exhaustive match → return unit
        builder.ins().jump(merge_block, &[]);

        builder.switch_to_block(merge_block);
        builder.seal_block(merge_block);
        TypedValue::boxed(builder.use_var(result_var))
    }

    /// Check if a RustIrExpr is a Local variable reference at its last use.
    fn is_last_use_local(&self, expr: &RustIrExpr) -> bool {
        if let RustIrExpr::Local { id, name, .. } = expr {
            if let Some(ref use_map) = self.use_map {
                return use_map.is_last_use(*id, name);
            }
        }
        false
    }

    /// Emit a pattern test that returns i64: 1 if matched, 0 if not.
    /// Does NOT bind variables — that's done separately by `bind_pattern`.
    fn emit_pattern_test(
        &mut self,
        builder: &mut FunctionBuilder<'_>,
        pattern: &RustIrPattern,
        value: Value,
    ) -> Value {
        match pattern {
            RustIrPattern::Wildcard { .. } => builder.ins().iconst(PTR, 1),
            RustIrPattern::Var { .. } => builder.ins().iconst(PTR, 1),
            RustIrPattern::At { pattern, .. } => self.emit_pattern_test(builder, pattern, value),
            RustIrPattern::Literal { value: lit, .. } => {
                self.emit_literal_test(builder, lit, value)
            }
            RustIrPattern::Constructor { name, args, .. } => {
                // Check name matches
                let (name_ptr, name_len) = self.embed_str(builder, name.as_bytes());
                let name_match = {
                    let call = builder.ins().call(
                        self.helpers.rt_constructor_name_eq,
                        &[self.ctx_param, value, name_ptr, name_len],
                    );
                    builder.inst_results(call)[0]
                };

                if args.is_empty() {
                    return name_match;
                }

                // Check arity
                let arity = {
                    let call = builder
                        .ins()
                        .call(self.helpers.rt_constructor_arity, &[self.ctx_param, value]);
                    builder.inst_results(call)[0]
                };
                let expected = builder.ins().iconst(PTR, args.len() as i64);
                let arity_ok = builder.ins().icmp(
                    cranelift_codegen::ir::condcodes::IntCC::Equal,
                    arity,
                    expected,
                );
                let arity_i64 = builder.ins().uextend(PTR, arity_ok);

                // AND name + arity checks
                let base_ok = builder.ins().band(name_match, arity_i64);

                // Short-circuit: skip arg extraction if name/arity don't match
                let args_block = builder.create_block();
                let pat_merge_block = builder.create_block();
                builder.append_block_param(pat_merge_block, PTR);

                let base_bool = builder.ins().icmp_imm(
                    cranelift_codegen::ir::condcodes::IntCC::NotEqual,
                    base_ok,
                    0,
                );
                let zero = builder.ins().iconst(PTR, 0);
                builder.ins().brif(
                    base_bool,
                    args_block,
                    &[],
                    pat_merge_block,
                    &[BlockArg::Value(zero)],
                );

                // Check each arg pattern (only reached when name + arity matched)
                builder.switch_to_block(args_block);
                builder.seal_block(args_block);

                let mut result = base_ok;
                for (i, arg_pat) in args.iter().enumerate() {
                    let idx = builder.ins().iconst(PTR, i as i64);
                    let arg_val = {
                        let call = builder.ins().call(
                            self.helpers.rt_constructor_arg,
                            &[self.ctx_param, value, idx],
                        );
                        builder.inst_results(call)[0]
                    };
                    let arg_ok = self.emit_pattern_test(builder, arg_pat, arg_val);
                    result = builder.ins().band(result, arg_ok);
                }
                builder
                    .ins()
                    .jump(pat_merge_block, &[BlockArg::Value(result)]);

                builder.switch_to_block(pat_merge_block);
                builder.seal_block(pat_merge_block);
                builder.block_params(pat_merge_block)[0]
            }
            RustIrPattern::Tuple { items, .. } => {
                // Check length
                let len = {
                    let call = builder
                        .ins()
                        .call(self.helpers.rt_tuple_len, &[self.ctx_param, value]);
                    builder.inst_results(call)[0]
                };
                let expected = builder.ins().iconst(PTR, items.len() as i64);
                let len_ok = builder.ins().icmp(
                    cranelift_codegen::ir::condcodes::IntCC::Equal,
                    len,
                    expected,
                );
                let mut result = builder.ins().uextend(PTR, len_ok);

                for (i, item_pat) in items.iter().enumerate() {
                    let idx = builder.ins().iconst(PTR, i as i64);
                    let item_val = {
                        let call = builder
                            .ins()
                            .call(self.helpers.rt_tuple_item, &[self.ctx_param, value, idx]);
                        builder.inst_results(call)[0]
                    };
                    let item_ok = self.emit_pattern_test(builder, item_pat, item_val);
                    result = builder.ins().band(result, item_ok);
                }
                result
            }
            RustIrPattern::List { items, rest, .. } => {
                // Check minimum length
                let len = {
                    let call = builder
                        .ins()
                        .call(self.helpers.rt_list_len, &[self.ctx_param, value]);
                    builder.inst_results(call)[0]
                };
                let min_len = builder.ins().iconst(PTR, items.len() as i64);

                let len_ok = if rest.is_some() {
                    // With rest: len >= items.len()
                    let cmp = builder.ins().icmp(
                        cranelift_codegen::ir::condcodes::IntCC::SignedGreaterThanOrEqual,
                        len,
                        min_len,
                    );
                    builder.ins().uextend(PTR, cmp)
                } else {
                    // Without rest: len == items.len()
                    let cmp = builder.ins().icmp(
                        cranelift_codegen::ir::condcodes::IntCC::Equal,
                        len,
                        min_len,
                    );
                    builder.ins().uextend(PTR, cmp)
                };

                if items.is_empty() && rest.is_none() {
                    return len_ok;
                }

                // Short-circuit: skip element access if length doesn't match
                let list_items_block = builder.create_block();
                let list_merge_block = builder.create_block();
                builder.append_block_param(list_merge_block, PTR);

                let len_bool = builder.ins().icmp_imm(
                    cranelift_codegen::ir::condcodes::IntCC::NotEqual,
                    len_ok,
                    0,
                );
                let zero = builder.ins().iconst(PTR, 0);
                builder.ins().brif(
                    len_bool,
                    list_items_block,
                    &[],
                    list_merge_block,
                    &[BlockArg::Value(zero)],
                );

                builder.switch_to_block(list_items_block);
                builder.seal_block(list_items_block);

                let mut result = len_ok;

                for (i, item_pat) in items.iter().enumerate() {
                    let idx = builder.ins().iconst(PTR, i as i64);
                    let item_val = {
                        let call = builder
                            .ins()
                            .call(self.helpers.rt_list_index, &[self.ctx_param, value, idx]);
                        builder.inst_results(call)[0]
                    };
                    let item_ok = self.emit_pattern_test(builder, item_pat, item_val);
                    result = builder.ins().band(result, item_ok);
                }

                if let Some(rest_pat) = rest.as_deref() {
                    let start = builder.ins().iconst(PTR, items.len() as i64);
                    let tail_val = {
                        let call = builder
                            .ins()
                            .call(self.helpers.rt_list_tail, &[self.ctx_param, value, start]);
                        builder.inst_results(call)[0]
                    };
                    let rest_ok = self.emit_pattern_test(builder, rest_pat, tail_val);
                    result = builder.ins().band(result, rest_ok);
                }
                builder
                    .ins()
                    .jump(list_merge_block, &[BlockArg::Value(result)]);

                builder.switch_to_block(list_merge_block);
                builder.seal_block(list_merge_block);
                builder.block_params(list_merge_block)[0]
            }
            RustIrPattern::Record { fields, .. } => {
                let mut result = builder.ins().iconst(PTR, 1);
                for field in fields {
                    // Navigate the path to get the nested value
                    let mut current = value;
                    for seg in &field.path {
                        let (name_ptr, name_len) = self.embed_str(builder, seg.as_bytes());
                        let call = builder.ins().call(
                            self.helpers.rt_record_field,
                            &[self.ctx_param, current, name_ptr, name_len],
                        );
                        current = builder.inst_results(call)[0];
                    }
                    let field_ok = self.emit_pattern_test(builder, &field.pattern, current);
                    result = builder.ins().band(result, field_ok);
                }
                result
            }
        }
    }

    /// Emit a literal equality test.
    fn emit_literal_test(
        &mut self,
        builder: &mut FunctionBuilder<'_>,
        lit: &RustIrLiteral,
        value: Value,
    ) -> Value {
        match lit {
            RustIrLiteral::Bool(b) => {
                let expected_tv = self.lower_lit_bool(builder, *b);
                let expected = self.ensure_boxed(builder, expected_tv);
                let call = builder.ins().call(
                    self.helpers.rt_value_equals,
                    &[self.ctx_param, value, expected],
                );
                builder.inst_results(call)[0]
            }
            RustIrLiteral::String(s) => {
                let expected_tv = self.lower_lit_string(builder, s);
                let expected = self.ensure_boxed(builder, expected_tv);
                let call = builder.ins().call(
                    self.helpers.rt_value_equals,
                    &[self.ctx_param, value, expected],
                );
                builder.inst_results(call)[0]
            }
            RustIrLiteral::DateTime(s) => {
                let expected_tv = self.lower_lit_string(builder, s);
                let expected = self.ensure_boxed(builder, expected_tv);
                let call = builder.ins().call(
                    self.helpers.rt_value_equals,
                    &[self.ctx_param, value, expected],
                );
                builder.inst_results(call)[0]
            }
            RustIrLiteral::Number(text) => {
                let expected_tv = self.lower_lit_number(builder, text);
                let expected = self.ensure_boxed(builder, expected_tv);
                let call = builder.ins().call(
                    self.helpers.rt_value_equals,
                    &[self.ctx_param, value, expected],
                );
                builder.inst_results(call)[0]
            }
            RustIrLiteral::Sigil { tag, body, flags } => {
                let repr = format!(
                    "#{}{}{}",
                    tag,
                    body,
                    if flags.is_empty() {
                        String::new()
                    } else {
                        format!("/{}", flags)
                    }
                );
                let expected_tv = self.lower_lit_string(builder, &repr);
                let expected = self.ensure_boxed(builder, expected_tv);
                let call = builder.ins().call(
                    self.helpers.rt_value_equals,
                    &[self.ctx_param, value, expected],
                );
                builder.inst_results(call)[0]
            }
        }
    }

    fn bind_pattern(
        &mut self,
        builder: &mut FunctionBuilder<'_>,
        pattern: &RustIrPattern,
        value: Value,
    ) {
        match pattern {
            RustIrPattern::Var { name, .. } => {
                self.locals.insert(name.clone(), TypedValue::boxed(value));
            }
            RustIrPattern::Wildcard { .. } => {}
            RustIrPattern::At { name, pattern, .. } => {
                self.locals.insert(name.clone(), TypedValue::boxed(value));
                self.bind_pattern(builder, pattern, value);
            }
            RustIrPattern::Literal { .. } => {}
            RustIrPattern::Constructor { args, .. } => {
                for (i, arg_pat) in args.iter().enumerate() {
                    let idx = builder.ins().iconst(PTR, i as i64);
                    let arg_val = {
                        let call = builder.ins().call(
                            self.helpers.rt_constructor_arg,
                            &[self.ctx_param, value, idx],
                        );
                        builder.inst_results(call)[0]
                    };
                    self.bind_pattern(builder, arg_pat, arg_val);
                }
            }
            RustIrPattern::Tuple { items, .. } => {
                for (i, item_pat) in items.iter().enumerate() {
                    let idx = builder.ins().iconst(PTR, i as i64);
                    let item_val = {
                        let call = builder
                            .ins()
                            .call(self.helpers.rt_tuple_item, &[self.ctx_param, value, idx]);
                        builder.inst_results(call)[0]
                    };
                    self.bind_pattern(builder, item_pat, item_val);
                }
            }
            RustIrPattern::List { items, rest, .. } => {
                for (i, item_pat) in items.iter().enumerate() {
                    let idx = builder.ins().iconst(PTR, i as i64);
                    let item_val = {
                        let call = builder
                            .ins()
                            .call(self.helpers.rt_list_index, &[self.ctx_param, value, idx]);
                        builder.inst_results(call)[0]
                    };
                    self.bind_pattern(builder, item_pat, item_val);
                }
                if let Some(rest_pat) = rest.as_deref() {
                    let start = builder.ins().iconst(PTR, items.len() as i64);
                    let tail_val = {
                        let call = builder
                            .ins()
                            .call(self.helpers.rt_list_tail, &[self.ctx_param, value, start]);
                        builder.inst_results(call)[0]
                    };
                    self.bind_pattern(builder, rest_pat, tail_val);
                }
            }
            RustIrPattern::Record { fields, .. } => {
                for field in fields {
                    let mut current = value;
                    for seg in &field.path {
                        let (name_ptr, name_len) = self.embed_str(builder, seg.as_bytes());
                        let call = builder.ins().call(
                            self.helpers.rt_record_field,
                            &[self.ctx_param, current, name_ptr, name_len],
                        );
                        current = builder.inst_results(call)[0];
                    }
                    self.bind_pattern(builder, &field.pattern, current);
                }
            }
        }
    }

    fn lower_binary(
        &mut self,
        builder: &mut FunctionBuilder<'_>,
        op: &str,
        left: &RustIrExpr,
        right: &RustIrExpr,
    ) -> TypedValue {
        let lhs = self.lower_expr(builder, left);
        let rhs = self.lower_expr(builder, right);

        // Native integer arithmetic when both sides are known Int
        if matches!(lhs.ty, Some(CgType::Int)) && matches!(rhs.ty, Some(CgType::Int)) {
            if let Some(tv) = self.try_native_int_op(builder, op, lhs.val, rhs.val) {
                return tv;
            }
        }

        // Native float arithmetic when both sides are known Float
        if matches!(lhs.ty, Some(CgType::Float)) && matches!(rhs.ty, Some(CgType::Float)) {
            if let Some(tv) = self.try_native_float_op(builder, op, lhs.val, rhs.val) {
                return tv;
            }
        }

        // Mixed Int/Float: promote Int to Float and do float op
        if matches!(lhs.ty, Some(CgType::Int)) && matches!(rhs.ty, Some(CgType::Float)) {
            let lf = builder.ins().fcvt_from_sint(F64, lhs.val);
            if let Some(tv) = self.try_native_float_op(builder, op, lf, rhs.val) {
                return tv;
            }
        }
        if matches!(lhs.ty, Some(CgType::Float)) && matches!(rhs.ty, Some(CgType::Int)) {
            let rf = builder.ins().fcvt_from_sint(F64, rhs.val);
            if let Some(tv) = self.try_native_float_op(builder, op, lhs.val, rf) {
                return tv;
            }
        }

        // Fallback: box both and call rt_binary_op
        let lhs_boxed = self.ensure_boxed(builder, lhs);
        let rhs_boxed = self.ensure_boxed(builder, rhs);
        let (op_ptr, op_len) = self.embed_str(builder, op.as_bytes());
        let call = builder.ins().call(
            self.helpers.rt_binary_op,
            &[self.ctx_param, op_ptr, op_len, lhs_boxed, rhs_boxed],
        );
        TypedValue::boxed(builder.inst_results(call)[0])
    }

    /// Try to emit a native i64 integer operation. Returns None if op is unknown.
    fn try_native_int_op(
        &self,
        builder: &mut FunctionBuilder<'_>,
        op: &str,
        l: Value,
        r: Value,
    ) -> Option<TypedValue> {
        match op {
            "+" => Some(TypedValue::typed(builder.ins().iadd(l, r), CgType::Int)),
            "-" => Some(TypedValue::typed(builder.ins().isub(l, r), CgType::Int)),
            "*" => Some(TypedValue::typed(builder.ins().imul(l, r), CgType::Int)),
            "/" => Some(TypedValue::typed(builder.ins().sdiv(l, r), CgType::Int)),
            "%" => Some(TypedValue::typed(builder.ins().srem(l, r), CgType::Int)),
            "==" => {
                let c = builder
                    .ins()
                    .icmp(cranelift_codegen::ir::condcodes::IntCC::Equal, l, r);
                let v = builder.ins().uextend(PTR, c);
                Some(TypedValue::typed(v, CgType::Bool))
            }
            "!=" => {
                let c = builder
                    .ins()
                    .icmp(cranelift_codegen::ir::condcodes::IntCC::NotEqual, l, r);
                let v = builder.ins().uextend(PTR, c);
                Some(TypedValue::typed(v, CgType::Bool))
            }
            "<" => {
                let c = builder.ins().icmp(
                    cranelift_codegen::ir::condcodes::IntCC::SignedLessThan,
                    l,
                    r,
                );
                let v = builder.ins().uextend(PTR, c);
                Some(TypedValue::typed(v, CgType::Bool))
            }
            "<=" => {
                let c = builder.ins().icmp(
                    cranelift_codegen::ir::condcodes::IntCC::SignedLessThanOrEqual,
                    l,
                    r,
                );
                let v = builder.ins().uextend(PTR, c);
                Some(TypedValue::typed(v, CgType::Bool))
            }
            ">" => {
                let c = builder.ins().icmp(
                    cranelift_codegen::ir::condcodes::IntCC::SignedGreaterThan,
                    l,
                    r,
                );
                let v = builder.ins().uextend(PTR, c);
                Some(TypedValue::typed(v, CgType::Bool))
            }
            ">=" => {
                let c = builder.ins().icmp(
                    cranelift_codegen::ir::condcodes::IntCC::SignedGreaterThanOrEqual,
                    l,
                    r,
                );
                let v = builder.ins().uextend(PTR, c);
                Some(TypedValue::typed(v, CgType::Bool))
            }
            _ => None,
        }
    }

    /// Try to emit a native f64 float operation. Returns None if op is unknown.
    fn try_native_float_op(
        &self,
        builder: &mut FunctionBuilder<'_>,
        op: &str,
        l: Value,
        r: Value,
    ) -> Option<TypedValue> {
        match op {
            "+" => Some(TypedValue::typed(builder.ins().fadd(l, r), CgType::Float)),
            "-" => Some(TypedValue::typed(builder.ins().fsub(l, r), CgType::Float)),
            "*" => Some(TypedValue::typed(builder.ins().fmul(l, r), CgType::Float)),
            "/" => Some(TypedValue::typed(builder.ins().fdiv(l, r), CgType::Float)),
            "==" => {
                let c = builder
                    .ins()
                    .fcmp(cranelift_codegen::ir::condcodes::FloatCC::Equal, l, r);
                let v = builder.ins().uextend(PTR, c);
                Some(TypedValue::typed(v, CgType::Bool))
            }
            "!=" => {
                let c =
                    builder
                        .ins()
                        .fcmp(cranelift_codegen::ir::condcodes::FloatCC::NotEqual, l, r);
                let v = builder.ins().uextend(PTR, c);
                Some(TypedValue::typed(v, CgType::Bool))
            }
            "<" => {
                let c =
                    builder
                        .ins()
                        .fcmp(cranelift_codegen::ir::condcodes::FloatCC::LessThan, l, r);
                let v = builder.ins().uextend(PTR, c);
                Some(TypedValue::typed(v, CgType::Bool))
            }
            "<=" => {
                let c = builder.ins().fcmp(
                    cranelift_codegen::ir::condcodes::FloatCC::LessThanOrEqual,
                    l,
                    r,
                );
                let v = builder.ins().uextend(PTR, c);
                Some(TypedValue::typed(v, CgType::Bool))
            }
            ">" => {
                let c = builder.ins().fcmp(
                    cranelift_codegen::ir::condcodes::FloatCC::GreaterThan,
                    l,
                    r,
                );
                let v = builder.ins().uextend(PTR, c);
                Some(TypedValue::typed(v, CgType::Bool))
            }
            ">=" => {
                let c = builder.ins().fcmp(
                    cranelift_codegen::ir::condcodes::FloatCC::GreaterThanOrEqual,
                    l,
                    r,
                );
                let v = builder.ins().uextend(PTR, c);
                Some(TypedValue::typed(v, CgType::Bool))
            }
            _ => None,
        }
    }

    // -----------------------------------------------------------------------
    // Block lowering
    // -----------------------------------------------------------------------

    fn lower_block(
        &mut self,
        builder: &mut FunctionBuilder<'_>,
        block_kind: &RustIrBlockKind,
        items: &[RustIrBlockItem],
    ) -> TypedValue {
        match block_kind {
            RustIrBlockKind::Plain => self.lower_plain_block(builder, items),
            RustIrBlockKind::Do { .. } => self.lower_do_block(builder, items),
            RustIrBlockKind::Generate => self.lower_native_generate(builder, items),
            RustIrBlockKind::Resource => self.lower_resource_block(builder, items),
        }
    }

    fn lower_plain_block(
        &mut self,
        builder: &mut FunctionBuilder<'_>,
        items: &[RustIrBlockItem],
    ) -> TypedValue {
        let mut last = {
            let call = builder
                .ins()
                .call(self.helpers.rt_alloc_unit, &[self.ctx_param]);
            TypedValue::boxed(builder.inst_results(call)[0])
        };
        for item in items {
            last = match item {
                RustIrBlockItem::Bind { pattern, expr } => {
                    let tv = self.lower_expr(builder, expr);
                    let boxed = self.ensure_boxed(builder, tv.clone());
                    self.bind_pattern(builder, pattern, boxed);
                    tv
                }
                RustIrBlockItem::Expr { expr } => self.lower_expr(builder, expr),
                RustIrBlockItem::Yield { expr } => self.lower_expr(builder, expr),
                RustIrBlockItem::Recurse { expr } => self.lower_expr(builder, expr),
                RustIrBlockItem::Filter { expr } => self.lower_expr(builder, expr),
            };
        }
        last
    }

    fn lower_do_block(
        &mut self,
        builder: &mut FunctionBuilder<'_>,
        items: &[RustIrBlockItem],
    ) -> TypedValue {
        // Effect block: each `<- expr` binds the result of running the effect,
        // each bare `expr` is a sequenced effect.
        let mut current_effect = {
            let call = builder
                .ins()
                .call(self.helpers.rt_alloc_unit, &[self.ctx_param]);
            TypedValue::boxed(builder.inst_results(call)[0])
        };

        for item in items {
            match item {
                RustIrBlockItem::Bind { pattern, expr } => {
                    let effect_tv = self.lower_expr(builder, expr);
                    let effect_boxed = self.ensure_boxed(builder, effect_tv);
                    // Run the effect and bind the result
                    let result = {
                        let call = builder
                            .ins()
                            .call(self.helpers.rt_run_effect, &[self.ctx_param, effect_boxed]);
                        builder.inst_results(call)[0]
                    };
                    self.bind_pattern(builder, pattern, result);
                    // For loop bindings (e.g. `__loop1`), register the closure as
                    // a runtime global so that recursive calls inside the loop body
                    // can find it via rt_get_global at each iteration.
                    if let RustIrPattern::Var { name, .. } = pattern {
                        if name.starts_with("__loop") {
                            self.emit_set_global(builder, name, result);
                        }
                    }
                    current_effect = TypedValue::boxed(result);
                }
                RustIrBlockItem::Expr { expr } => {
                    let effect_tv = self.lower_expr(builder, expr);
                    let effect_boxed = self.ensure_boxed(builder, effect_tv);
                    let call = builder
                        .ins()
                        .call(self.helpers.rt_run_effect, &[self.ctx_param, effect_boxed]);
                    current_effect = TypedValue::boxed(builder.inst_results(call)[0]);
                }
                _ => {
                    current_effect = self.lower_expr(
                        builder,
                        match item {
                            RustIrBlockItem::Yield { expr } => expr,
                            RustIrBlockItem::Recurse { expr } => expr,
                            RustIrBlockItem::Filter { expr } => expr,
                            _ => unreachable!(),
                        },
                    );
                }
            }
        }
        // Wrap the result in an Effect thunk so callers that use
        // `rt_run_effect` on the block's return value see a proper Effect.
        let wrapped = self.ensure_boxed(builder, current_effect);
        let call = builder
            .ins()
            .call(self.helpers.rt_wrap_effect, &[self.ctx_param, wrapped]);
        TypedValue::boxed(builder.inst_results(call)[0])
    }

    /// Compile a generate block natively in Cranelift.
    ///
    /// Supports all item types including Bind (generator binding via loops).
    fn lower_native_generate(
        &mut self,
        builder: &mut FunctionBuilder<'_>,
        items: &[RustIrBlockItem],
    ) -> TypedValue {
        // 1. Allocate an empty Vec<Value>
        let vec = {
            let call = builder
                .ins()
                .call(self.helpers.rt_gen_vec_new, &[self.ctx_param]);
            builder.inst_results(call)[0]
        };

        let vec_var = builder.declare_var(PTR);
        builder.def_var(vec_var, vec);

        // Create the "done" block — all paths converge here.
        let done_block = builder.create_block();

        // Process all items (may create nested loops for Bind)
        self.lower_generate_items(builder, items, vec_var, done_block);

        // Jump from the end of item processing to done
        builder.ins().jump(done_block, &[]);

        // 2. Wrap Vec into generator fold function (done block)
        builder.switch_to_block(done_block);
        builder.seal_block(done_block);

        let current_vec = builder.use_var(vec_var);
        let gen = {
            let call = builder.ins().call(
                self.helpers.rt_gen_vec_into_generator,
                &[self.ctx_param, current_vec],
            );
            builder.inst_results(call)[0]
        };
        TypedValue::boxed(gen)
    }

    /// Process generate block items, creating loops for Bind items.
    ///
    /// `vec_var` — Cranelift variable holding the `Vec<Value>*` accumulator.
    /// `skip_block` — block to jump to when a filter fails or we need to
    ///   skip remaining items (done block at top level, loop-continue inside
    ///   a Bind loop).
    fn lower_generate_items(
        &mut self,
        builder: &mut FunctionBuilder<'_>,
        items: &[RustIrBlockItem],
        vec_var: Variable,
        skip_block: cranelift_codegen::ir::Block,
    ) {
        for (i, item) in items.iter().enumerate() {
            match item {
                RustIrBlockItem::Yield { expr } => {
                    let tv = self.lower_expr(builder, expr);
                    let boxed = self.ensure_boxed(builder, tv);
                    let current_vec = builder.use_var(vec_var);
                    builder.ins().call(
                        self.helpers.rt_gen_vec_push,
                        &[self.ctx_param, current_vec, boxed],
                    );
                }
                RustIrBlockItem::Filter { expr } => {
                    let tv = self.lower_expr(builder, expr);
                    let cond = self.ensure_boxed(builder, tv);
                    let unboxed = {
                        let call = builder
                            .ins()
                            .call(self.helpers.rt_unbox_bool, &[self.ctx_param, cond]);
                        builder.inst_results(call)[0]
                    };
                    // If false, skip remaining items. If true, continue.
                    let continue_block = builder.create_block();
                    builder
                        .ins()
                        .brif(unboxed, continue_block, &[], skip_block, &[]);
                    builder.switch_to_block(continue_block);
                    builder.seal_block(continue_block);
                }
                RustIrBlockItem::Expr { expr } => {
                    let _tv = self.lower_expr(builder, expr);
                }
                RustIrBlockItem::Recurse { expr } => {
                    let _tv = self.lower_expr(builder, expr);
                }
                RustIrBlockItem::Bind { pattern, expr } => {
                    // 1. Evaluate the source expression (a generator)
                    let source_tv = self.lower_expr(builder, expr);
                    let source_boxed = self.ensure_boxed(builder, source_tv);

                    // 2. Convert generator to list via runtime helper
                    let list = {
                        let call = builder.ins().call(
                            self.helpers.rt_generator_to_list,
                            &[self.ctx_param, source_boxed],
                        );
                        builder.inst_results(call)[0]
                    };

                    // 3. Get list length
                    let len = {
                        let call = builder
                            .ins()
                            .call(self.helpers.rt_list_len, &[self.ctx_param, list]);
                        builder.inst_results(call)[0]
                    };

                    // 4. Set up loop: counter starts at 0
                    let counter_var = builder.declare_var(PTR);
                    let zero = builder.ins().iconst(PTR, 0);
                    builder.def_var(counter_var, zero);

                    let loop_header = builder.create_block();
                    let loop_body = builder.create_block();
                    let loop_exit = builder.create_block();
                    let loop_continue = builder.create_block();

                    builder.ins().jump(loop_header, &[]);

                    // Loop header: check counter < length
                    builder.switch_to_block(loop_header);
                    // Don't seal yet (back-edge from loop_continue)
                    let counter = builder.use_var(counter_var);
                    let cond = builder.ins().icmp(
                        cranelift_codegen::ir::condcodes::IntCC::SignedLessThan,
                        counter,
                        len,
                    );
                    builder.ins().brif(cond, loop_body, &[], loop_exit, &[]);

                    // Loop body: get element, bind pattern
                    builder.switch_to_block(loop_body);
                    builder.seal_block(loop_body);

                    let elem = {
                        let call = builder
                            .ins()
                            .call(self.helpers.rt_list_index, &[self.ctx_param, list, counter]);
                        builder.inst_results(call)[0]
                    };
                    self.bind_pattern(builder, pattern, elem);

                    // Process remaining items inside the loop body.
                    // Filters inside the loop jump to loop_continue (skip iteration).
                    let remaining = &items[i + 1..];
                    self.lower_generate_items(builder, remaining, vec_var, loop_continue);

                    // After remaining items: jump to loop_continue
                    builder.ins().jump(loop_continue, &[]);

                    // Loop continue: increment counter, jump back to header
                    builder.switch_to_block(loop_continue);
                    builder.seal_block(loop_continue);
                    let cur = builder.use_var(counter_var);
                    let next = builder.ins().iadd_imm(cur, 1);
                    builder.def_var(counter_var, next);
                    builder.ins().jump(loop_header, &[]);

                    // Seal loop header (two predecessors: entry + back-edge)
                    builder.seal_block(loop_header);

                    // Continue after the loop
                    builder.switch_to_block(loop_exit);
                    builder.seal_block(loop_exit);

                    // All remaining items already processed inside loop — return
                    return;
                }
            }
        }
        // No more items — falls through to caller (which adds jump to done)
    }

    fn lower_resource_block(
        &mut self,
        builder: &mut FunctionBuilder<'_>,
        items: &[RustIrBlockItem],
    ) -> TypedValue {
        // Pre-convert RustIR → HIR at compile time to avoid runtime bridge
        let hir_items = match crate::runtime::lower_runtime_rust_ir_block_items(items) {
            Ok(items) => items,
            Err(_) => {
                let call = builder
                    .ins()
                    .call(self.helpers.rt_alloc_unit, &[self.ctx_param]);
                return TypedValue::boxed(builder.inst_results(call)[0]);
            }
        };

        // Construct the Resource value at compile time and leak it
        let resource = crate::runtime::values::Value::Resource(std::sync::Arc::new(
            crate::runtime::values::ResourceValue {
                items: std::sync::Arc::new(hir_items),
            },
        ));
        let leaked_ptr = Box::into_raw(Box::new(resource));

        // At runtime: clone the pre-built resource value
        let ptr_const = builder.ins().iconst(PTR, leaked_ptr as i64);
        let call = builder
            .ins()
            .call(self.helpers.rt_clone_value, &[self.ctx_param, ptr_const]);
        TypedValue::boxed(builder.inst_results(call)[0])
    }
}
