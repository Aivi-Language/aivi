//! Lower `RustIrExpr` to Cranelift IR.
//!
//! This is the main expression-lowering engine for the full Cranelift backend.
//! All values are represented as opaque `i64` pointers to heap-boxed `Value`s,
//! except for known-scalar paths where unboxed representations are used.
//!
//! Every emitted function has the signature:
//!     `(ctx: i64, ...args: i64) -> i64`
//! where `ctx` is a `*mut JitRuntimeCtx` and each arg/return is a `*mut Value`.

use std::collections::HashMap;

use cranelift_codegen::ir::{types, AbiParam, Function, InstBuilder, Value};
use cranelift_codegen::ir::FuncRef;
use cranelift_frontend::FunctionBuilder;
use cranelift_module::{Linkage, Module};
use cranelift_jit::JITModule;

use crate::rust_ir::{
    RustIrBlockItem, RustIrBlockKind, RustIrExpr, RustIrListItem, RustIrMatchArm, RustIrPattern,
    RustIrRecordField, RustIrTextPart,
};

/// Pointer-sized integer type (all values are passed as i64 pointers).
const PTR: cranelift_codegen::ir::Type = types::I64;

/// Context for lowering a single function body.
pub(crate) struct LowerCtx<'a> {
    /// Maps local variable names to Cranelift SSA values (pointer to `Value`).
    pub(crate) locals: HashMap<String, Value>,
    /// The `ctx` (JitRuntimeCtx) parameter — first arg of every function.
    ctx_param: Value,
    /// Declared runtime helper function references in this module.
    helpers: &'a HelperRefs,
}

/// Pre-declared `FuncRef`s for all runtime helpers in a JIT module.
#[allow(dead_code)]
pub(crate) struct HelperRefs {
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
    pub(crate) rt_apply: FuncRef,
    pub(crate) rt_force_thunk: FuncRef,
    pub(crate) rt_run_effect: FuncRef,
    pub(crate) rt_bind_effect: FuncRef,
    pub(crate) rt_binary_op: FuncRef,
}

/// Declare all runtime helper signatures in the module and return FuncRefs
/// that can be imported into individual functions via `module.declare_func_in_func`.
pub(crate) fn declare_helpers(module: &mut JITModule) -> Result<DeclaredHelpers, String> {
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
        // (ctx, func_ptr, arg_ptr) -> ptr
        rt_apply: decl!("rt_apply", [PTR, PTR, PTR], [PTR]),
        // (ctx, ptr) -> ptr
        rt_force_thunk: decl!("rt_force_thunk", [PTR, PTR], [PTR]),
        rt_run_effect: decl!("rt_run_effect", [PTR, PTR], [PTR]),
        // (ctx, effect_ptr, cont_ptr) -> ptr
        rt_bind_effect: decl!("rt_bind_effect", [PTR, PTR, PTR], [PTR]),
        // (ctx, op_ptr, op_len, lhs_ptr, rhs_ptr) -> ptr
        rt_binary_op: decl!("rt_binary_op", [PTR, PTR, PTR, PTR, PTR], [PTR]),
    })
}

/// Module-level function IDs for all runtime helpers.
pub(crate) struct DeclaredHelpers {
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
    pub(crate) rt_apply: cranelift_module::FuncId,
    pub(crate) rt_force_thunk: cranelift_module::FuncId,
    pub(crate) rt_run_effect: cranelift_module::FuncId,
    pub(crate) rt_bind_effect: cranelift_module::FuncId,
    pub(crate) rt_binary_op: cranelift_module::FuncId,
}

impl DeclaredHelpers {
    /// Import all helper FuncIds into a specific function, producing `FuncRef`s.
    pub(crate) fn import_into(
        &self,
        module: &mut JITModule,
        func: &mut Function,
    ) -> HelperRefs {
        macro_rules! imp {
            ($field:ident) => {
                module.declare_func_in_func(self.$field, func)
            };
        }
        HelperRefs {
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
            rt_apply: imp!(rt_apply),
            rt_force_thunk: imp!(rt_force_thunk),
            rt_run_effect: imp!(rt_run_effect),
            rt_bind_effect: imp!(rt_bind_effect),
            rt_binary_op: imp!(rt_binary_op),
        }
    }
}

impl<'a> LowerCtx<'a> {
    pub(crate) fn new(ctx_param: Value, helpers: &'a HelperRefs) -> Self {
        Self {
            locals: HashMap::new(),
            ctx_param,
            helpers,
        }
    }

    /// Lower a `RustIrExpr` to a Cranelift `Value` (a `*mut runtime::Value`).
    pub(crate) fn lower_expr(
        &mut self,
        builder: &mut FunctionBuilder<'_>,
        expr: &RustIrExpr,
    ) -> Value {
        match expr {
            // ----- Literals -----
            RustIrExpr::LitNumber { text, .. } => self.lower_lit_number(builder, text),
            RustIrExpr::LitString { text, .. } => self.lower_lit_string(builder, text),
            RustIrExpr::LitBool { value, .. } => self.lower_lit_bool(builder, *value),
            RustIrExpr::LitDateTime { text, .. } => self.lower_lit_string(builder, text),
            RustIrExpr::LitSigil { tag, body, flags, .. } => {
                // Sigils are represented as strings for now: "#tag body flags"
                let repr = format!("#{}{}{}", tag, body, if flags.is_empty() { String::new() } else { format!("/{}", flags) });
                self.lower_lit_string(builder, &repr)
            }
            RustIrExpr::TextInterpolate { parts, .. } => self.lower_text_interpolate(builder, parts),

            // ----- Variables -----
            RustIrExpr::Local { name, .. } => self.lower_local(builder, name),
            RustIrExpr::Global { name, .. } => self.lower_global(builder, name),
            RustIrExpr::Builtin { builtin, .. } => self.lower_global(builder, builtin),
            RustIrExpr::ConstructorValue { name, .. } => self.lower_global(builder, name),

            // ----- Functions -----
            RustIrExpr::Lambda { param, body, .. } => self.lower_lambda(builder, param, body),
            RustIrExpr::App { func, arg, .. } => self.lower_app(builder, func, arg),
            RustIrExpr::Call { func, args, .. } => self.lower_call(builder, func, args),

            // ----- Data structures -----
            RustIrExpr::List { items, .. } => self.lower_list(builder, items),
            RustIrExpr::Tuple { items, .. } => self.lower_tuple(builder, items),
            RustIrExpr::Record { fields, .. } => self.lower_record(builder, fields),
            RustIrExpr::Patch { target, fields, .. } => self.lower_patch(builder, target, fields),

            // ----- Access -----
            RustIrExpr::FieldAccess { base, field, .. } => self.lower_field_access(builder, base, field),
            RustIrExpr::Index { base, index, .. } => self.lower_index(builder, base, index),

            // ----- Control flow -----
            RustIrExpr::If { cond, then_branch, else_branch, .. } => {
                self.lower_if(builder, cond, then_branch, else_branch)
            }
            RustIrExpr::Match { scrutinee, arms, .. } => self.lower_match(builder, scrutinee, arms),
            RustIrExpr::Binary { op, left, right, .. } => self.lower_binary(builder, op, left, right),

            // ----- Blocks -----
            RustIrExpr::Block { block_kind, items, .. } => self.lower_block(builder, block_kind, items),
            RustIrExpr::Pipe { func, arg, .. } => self.lower_app(builder, func, arg),

            // ----- Special -----
            RustIrExpr::DebugFn { body, .. } => self.lower_expr(builder, body),
            RustIrExpr::Raw { text, .. } => self.lower_lit_string(builder, text),
        }
    }

    // -----------------------------------------------------------------------
    // Literal lowering
    // -----------------------------------------------------------------------

    fn lower_lit_number(&mut self, builder: &mut FunctionBuilder<'_>, text: &str) -> Value {
        if let Ok(int_val) = text.parse::<i64>() {
            let v = builder.ins().iconst(PTR, int_val);
            let call = builder.ins().call(self.helpers.rt_box_int, &[self.ctx_param, v]);
            builder.inst_results(call)[0]
        } else if let Ok(float_val) = text.parse::<f64>() {
            let bits = float_val.to_bits() as i64;
            let v = builder.ins().iconst(PTR, bits);
            let call = builder.ins().call(self.helpers.rt_box_float, &[self.ctx_param, v]);
            builder.inst_results(call)[0]
        } else {
            // Fallback: treat as string (for BigInt, Rational, Decimal, etc.)
            self.lower_lit_string(builder, text)
        }
    }

    fn lower_lit_string(&mut self, builder: &mut FunctionBuilder<'_>, text: &str) -> Value {
        let ptr = text.as_ptr() as i64;
        let len = text.len() as i64;
        let ptr_val = builder.ins().iconst(PTR, ptr);
        let len_val = builder.ins().iconst(PTR, len);
        let call = builder.ins().call(
            self.helpers.rt_alloc_string,
            &[self.ctx_param, ptr_val, len_val],
        );
        builder.inst_results(call)[0]
    }

    fn lower_lit_bool(&mut self, builder: &mut FunctionBuilder<'_>, value: bool) -> Value {
        let v = builder.ins().iconst(PTR, i64::from(value));
        let call = builder.ins().call(self.helpers.rt_box_bool, &[self.ctx_param, v]);
        builder.inst_results(call)[0]
    }

    fn lower_text_interpolate(
        &mut self,
        builder: &mut FunctionBuilder<'_>,
        parts: &[RustIrTextPart],
    ) -> Value {
        // Build interpolated string by concatenating parts via rt_apply on
        // the text.append builtin. For now, simplify: emit each part as a
        // value and use rt_binary_op with "++" (string concat).
        if parts.is_empty() {
            return self.lower_lit_string(builder, "");
        }
        let mut result = match &parts[0] {
            RustIrTextPart::Text { text } => self.lower_lit_string(builder, text),
            RustIrTextPart::Expr { expr } => self.lower_expr(builder, expr),
        };
        let op = "++";
        let op_ptr = builder.ins().iconst(PTR, op.as_ptr() as i64);
        let op_len = builder.ins().iconst(PTR, op.len() as i64);
        for part in &parts[1..] {
            let part_val = match part {
                RustIrTextPart::Text { text } => self.lower_lit_string(builder, text),
                RustIrTextPart::Expr { expr } => self.lower_expr(builder, expr),
            };
            let call = builder.ins().call(
                self.helpers.rt_binary_op,
                &[self.ctx_param, op_ptr, op_len, result, part_val],
            );
            result = builder.inst_results(call)[0];
        }
        result
    }

    // -----------------------------------------------------------------------
    // Variable lowering
    // -----------------------------------------------------------------------

    fn lower_local(&self, builder: &mut FunctionBuilder<'_>, name: &str) -> Value {
        if let Some(&val) = self.locals.get(name) {
            val
        } else {
            // Fallback: treat as global lookup
            self.lower_global(builder, name)
        }
    }

    fn lower_global(&self, builder: &mut FunctionBuilder<'_>, name: &str) -> Value {
        let name_ptr = builder.ins().iconst(PTR, name.as_ptr() as i64);
        let name_len = builder.ins().iconst(PTR, name.len() as i64);
        let call = builder.ins().call(
            self.helpers.rt_get_global,
            &[self.ctx_param, name_ptr, name_len],
        );
        builder.inst_results(call)[0]
    }

    // -----------------------------------------------------------------------
    // Function lowering
    // -----------------------------------------------------------------------

    fn lower_lambda(
        &mut self,
        builder: &mut FunctionBuilder<'_>,
        _param: &str,
        _body: &RustIrExpr,
    ) -> Value {
        // Lambda lowering requires emitting a closure (code pointer + captured env).
        // For now, fall back to constructing via global lookup of the enclosing def.
        // Full closure compilation will be implemented in Phase 3.
        // TODO(cranelift-migration): emit real closure
        let call = builder.ins().call(self.helpers.rt_alloc_unit, &[self.ctx_param]);
        builder.inst_results(call)[0]
    }

    fn lower_app(
        &mut self,
        builder: &mut FunctionBuilder<'_>,
        func: &RustIrExpr,
        arg: &RustIrExpr,
    ) -> Value {
        let func_val = self.lower_expr(builder, func);
        let arg_val = self.lower_expr(builder, arg);
        let call = builder.ins().call(
            self.helpers.rt_apply,
            &[self.ctx_param, func_val, arg_val],
        );
        builder.inst_results(call)[0]
    }

    fn lower_call(
        &mut self,
        builder: &mut FunctionBuilder<'_>,
        func: &RustIrExpr,
        args: &[RustIrExpr],
    ) -> Value {
        // Multi-arg call desugars to chained application
        let mut result = self.lower_expr(builder, func);
        for arg in args {
            let arg_val = self.lower_expr(builder, arg);
            let call = builder.ins().call(
                self.helpers.rt_apply,
                &[self.ctx_param, result, arg_val],
            );
            result = builder.inst_results(call)[0];
        }
        result
    }

    // -----------------------------------------------------------------------
    // Data structure lowering
    // -----------------------------------------------------------------------

    fn lower_list(
        &mut self,
        builder: &mut FunctionBuilder<'_>,
        items: &[RustIrListItem],
    ) -> Value {
        // Allocate a stack slot for the item pointers array
        let count = items.len();
        if count == 0 {
            let null = builder.ins().iconst(PTR, 0);
            let zero = builder.ins().iconst(PTR, 0);
            let call = builder.ins().call(
                self.helpers.rt_alloc_list,
                &[self.ctx_param, null, zero],
            );
            return builder.inst_results(call)[0];
        }

        // Emit each item and store pointers in a stack slot
        let slot = builder.create_sized_stack_slot(cranelift_codegen::ir::StackSlotData::new(
            cranelift_codegen::ir::StackSlotKind::ExplicitSlot,
            (count * 8) as u32,
            0,
        ));
        for (i, item) in items.iter().enumerate() {
            let val = self.lower_expr(builder, &item.expr);
            builder.ins().stack_store(val, slot, (i * 8) as i32);
        }
        let arr_ptr = builder.ins().stack_addr(PTR, slot, 0);
        let len = builder.ins().iconst(PTR, count as i64);
        let call = builder.ins().call(
            self.helpers.rt_alloc_list,
            &[self.ctx_param, arr_ptr, len],
        );
        builder.inst_results(call)[0]
    }

    fn lower_tuple(
        &mut self,
        builder: &mut FunctionBuilder<'_>,
        items: &[RustIrExpr],
    ) -> Value {
        let count = items.len();
        if count == 0 {
            let call = builder.ins().call(self.helpers.rt_alloc_unit, &[self.ctx_param]);
            return builder.inst_results(call)[0];
        }
        let slot = builder.create_sized_stack_slot(cranelift_codegen::ir::StackSlotData::new(
            cranelift_codegen::ir::StackSlotKind::ExplicitSlot,
            (count * 8) as u32,
            0,
        ));
        for (i, item) in items.iter().enumerate() {
            let val = self.lower_expr(builder, item);
            builder.ins().stack_store(val, slot, (i * 8) as i32);
        }
        let arr_ptr = builder.ins().stack_addr(PTR, slot, 0);
        let len = builder.ins().iconst(PTR, count as i64);
        let call = builder.ins().call(
            self.helpers.rt_alloc_tuple,
            &[self.ctx_param, arr_ptr, len],
        );
        builder.inst_results(call)[0]
    }

    fn lower_record(
        &mut self,
        builder: &mut FunctionBuilder<'_>,
        fields: &[RustIrRecordField],
    ) -> Value {
        let count = fields.len();
        if count == 0 {
            let null = builder.ins().iconst(PTR, 0);
            let zero = builder.ins().iconst(PTR, 0);
            let call = builder.ins().call(
                self.helpers.rt_alloc_record,
                &[self.ctx_param, null, null, null, zero],
            );
            return builder.inst_results(call)[0];
        }

        // Stack slots for name pointers, name lengths, and value pointers
        let names_slot = builder.create_sized_stack_slot(cranelift_codegen::ir::StackSlotData::new(
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
            let field_name = field.path.first().map(|seg| match seg {
                crate::rust_ir::RustIrPathSegment::Field(name) => name.as_str(),
                _ => "_",
            }).unwrap_or("_");
            let name_ptr = builder.ins().iconst(PTR, field_name.as_ptr() as i64);
            let name_len = builder.ins().iconst(PTR, field_name.len() as i64);
            builder.ins().stack_store(name_ptr, names_slot, (i * 8) as i32);
            builder.ins().stack_store(name_len, lens_slot, (i * 8) as i32);
            let val = self.lower_expr(builder, &field.value);
            builder.ins().stack_store(val, vals_slot, (i * 8) as i32);
        }

        let names_ptr = builder.ins().stack_addr(PTR, names_slot, 0);
        let lens_ptr = builder.ins().stack_addr(PTR, lens_slot, 0);
        let vals_ptr = builder.ins().stack_addr(PTR, vals_slot, 0);
        let len = builder.ins().iconst(PTR, count as i64);
        let call = builder.ins().call(
            self.helpers.rt_alloc_record,
            &[self.ctx_param, names_ptr, lens_ptr, vals_ptr, len],
        );
        builder.inst_results(call)[0]
    }

    fn lower_patch(
        &mut self,
        builder: &mut FunctionBuilder<'_>,
        target: &RustIrExpr,
        fields: &[RustIrRecordField],
    ) -> Value {
        // Patch = get the base record, then overlay fields.
        // For now, lower as: base value, then apply field updates via rt_apply.
        // TODO(cranelift-migration): implement proper record patching
        let _base = self.lower_expr(builder, target);
        // Simplified: just emit the record with the new fields
        self.lower_record(builder, fields)
    }

    // -----------------------------------------------------------------------
    // Access lowering
    // -----------------------------------------------------------------------

    fn lower_field_access(
        &mut self,
        builder: &mut FunctionBuilder<'_>,
        base: &RustIrExpr,
        field: &str,
    ) -> Value {
        let base_val = self.lower_expr(builder, base);
        let name_ptr = builder.ins().iconst(PTR, field.as_ptr() as i64);
        let name_len = builder.ins().iconst(PTR, field.len() as i64);
        let call = builder.ins().call(
            self.helpers.rt_record_field,
            &[self.ctx_param, base_val, name_ptr, name_len],
        );
        builder.inst_results(call)[0]
    }

    fn lower_index(
        &mut self,
        builder: &mut FunctionBuilder<'_>,
        base: &RustIrExpr,
        index: &RustIrExpr,
    ) -> Value {
        let base_val = self.lower_expr(builder, base);
        let idx_val = self.lower_expr(builder, index);
        // Unbox the index to get an i64
        let idx_int = {
            let call = builder.ins().call(
                self.helpers.rt_unbox_int,
                &[self.ctx_param, idx_val],
            );
            builder.inst_results(call)[0]
        };
        let call = builder.ins().call(
            self.helpers.rt_list_index,
            &[self.ctx_param, base_val, idx_int],
        );
        builder.inst_results(call)[0]
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
    ) -> Value {
        let cond_val = self.lower_expr(builder, cond);
        // Unbox bool
        let cond_int = {
            let call = builder.ins().call(
                self.helpers.rt_unbox_bool,
                &[self.ctx_param, cond_val],
            );
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

        // Use a Cranelift variable to communicate the result across blocks
        let result_var = builder.declare_var(PTR);

        builder.ins().brif(cond_bool, then_block, &[], else_block, &[]);

        builder.switch_to_block(then_block);
        builder.seal_block(then_block);
        let then_val = self.lower_expr(builder, then_branch);
        builder.def_var(result_var, then_val);
        builder.ins().jump(merge_block, &[]);

        builder.switch_to_block(else_block);
        builder.seal_block(else_block);
        let else_val = self.lower_expr(builder, else_branch);
        builder.def_var(result_var, else_val);
        builder.ins().jump(merge_block, &[]);

        builder.switch_to_block(merge_block);
        builder.seal_block(merge_block);
        builder.use_var(result_var)
    }

    fn lower_match(
        &mut self,
        builder: &mut FunctionBuilder<'_>,
        scrutinee: &RustIrExpr,
        arms: &[RustIrMatchArm],
    ) -> Value {
        // Pattern matching compilation: for now, use chained if-else via rt_apply.
        // Each arm's pattern is tested by attempting to match and bind.
        // TODO(cranelift-migration): implement proper pattern compilation
        let scrut_val = self.lower_expr(builder, scrutinee);

        if arms.is_empty() {
            let call = builder.ins().call(self.helpers.rt_alloc_unit, &[self.ctx_param]);
            return builder.inst_results(call)[0];
        }

        // For now, just evaluate the first arm's body (wildcard-like fallback)
        // Full pattern matching will be Phase 2f+
        if let Some(arm) = arms.first() {
            self.bind_pattern(builder, &arm.pattern, scrut_val);
            return self.lower_expr(builder, &arm.body);
        }

        let call = builder.ins().call(self.helpers.rt_alloc_unit, &[self.ctx_param]);
        builder.inst_results(call)[0]
    }

    fn bind_pattern(
        &mut self,
        _builder: &mut FunctionBuilder<'_>,
        pattern: &RustIrPattern,
        value: Value,
    ) {
        match pattern {
            RustIrPattern::Var { name, .. } => {
                self.locals.insert(name.clone(), value);
            }
            RustIrPattern::Wildcard { .. } => {}
            RustIrPattern::At { name, pattern, .. } => {
                self.locals.insert(name.clone(), value);
                self.bind_pattern(_builder, pattern, value);
            }
            _ => {
                // TODO(cranelift-migration): destructuring patterns
            }
        }
    }

    fn lower_binary(
        &mut self,
        builder: &mut FunctionBuilder<'_>,
        op: &str,
        left: &RustIrExpr,
        right: &RustIrExpr,
    ) -> Value {
        let lhs = self.lower_expr(builder, left);
        let rhs = self.lower_expr(builder, right);
        let op_ptr = builder.ins().iconst(PTR, op.as_ptr() as i64);
        let op_len = builder.ins().iconst(PTR, op.len() as i64);
        let call = builder.ins().call(
            self.helpers.rt_binary_op,
            &[self.ctx_param, op_ptr, op_len, lhs, rhs],
        );
        builder.inst_results(call)[0]
    }

    // -----------------------------------------------------------------------
    // Block lowering
    // -----------------------------------------------------------------------

    fn lower_block(
        &mut self,
        builder: &mut FunctionBuilder<'_>,
        block_kind: &RustIrBlockKind,
        items: &[RustIrBlockItem],
    ) -> Value {
        match block_kind {
            RustIrBlockKind::Plain => self.lower_plain_block(builder, items),
            RustIrBlockKind::Do { .. } => self.lower_do_block(builder, items),
            RustIrBlockKind::Generate => self.lower_plain_block(builder, items),
            RustIrBlockKind::Resource => self.lower_plain_block(builder, items),
        }
    }

    fn lower_plain_block(
        &mut self,
        builder: &mut FunctionBuilder<'_>,
        items: &[RustIrBlockItem],
    ) -> Value {
        let mut last = {
            let call = builder.ins().call(self.helpers.rt_alloc_unit, &[self.ctx_param]);
            builder.inst_results(call)[0]
        };
        for item in items {
            last = match item {
                RustIrBlockItem::Bind { pattern, expr } => {
                    let val = self.lower_expr(builder, expr);
                    self.bind_pattern(builder, pattern, val);
                    val
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
    ) -> Value {
        // Effect block: each `<- expr` binds the result of running the effect,
        // each bare `expr` is a sequenced effect.
        // Lowered as chained rt_bind_effect calls.
        let mut current_effect = {
            let call = builder.ins().call(self.helpers.rt_alloc_unit, &[self.ctx_param]);
            builder.inst_results(call)[0]
        };

        for item in items {
            match item {
                RustIrBlockItem::Bind { pattern, expr } => {
                    let effect = self.lower_expr(builder, expr);
                    // Run the effect and bind the result
                    let result = {
                        let call = builder.ins().call(
                            self.helpers.rt_run_effect,
                            &[self.ctx_param, effect],
                        );
                        builder.inst_results(call)[0]
                    };
                    self.bind_pattern(builder, pattern, result);
                    current_effect = result;
                }
                RustIrBlockItem::Expr { expr } => {
                    let effect = self.lower_expr(builder, expr);
                    let call = builder.ins().call(
                        self.helpers.rt_run_effect,
                        &[self.ctx_param, effect],
                    );
                    current_effect = builder.inst_results(call)[0];
                }
                _ => {
                    current_effect = self.lower_expr(builder, match item {
                        RustIrBlockItem::Yield { expr } => expr,
                        RustIrBlockItem::Recurse { expr } => expr,
                        RustIrBlockItem::Filter { expr } => expr,
                        _ => unreachable!(),
                    });
                }
            }
        }
        current_effect
    }
}
