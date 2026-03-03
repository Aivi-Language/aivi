impl<'a, M: Module> LowerCtx<'a, M> {
// Global emission and helper methods for `LowerCtx`.
// Included inside `impl<'a, M: Module> LowerCtx<'a, M>` via `include!()`.

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

}
