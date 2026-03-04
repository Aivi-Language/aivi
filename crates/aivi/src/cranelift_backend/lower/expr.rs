impl<'a, M: Module> LowerCtx<'a, M> {
// Expression lowering methods for `LowerCtx`.
// Included inside `impl<'a, M: Module> LowerCtx<'a, M>` via `include!()`.

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

            RustIrExpr::Pipe { func, arg, .. } => self.lower_app(builder, func, arg),

            // ----- Special -----
            RustIrExpr::DebugFn { body, .. } => self.lower_expr(builder, body),
            RustIrExpr::Raw { text, .. } => self.lower_lit_string(builder, text),

            // ----- Mocking -----
            RustIrExpr::Mock {
                substitutions,
                body,
                ..
            } => self.lower_mock(builder, substitutions, body),
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

    /// Lower `mock ... in body`: save old globals, install mocks, eval body, restore.
    fn lower_mock(
        &mut self,
        builder: &mut FunctionBuilder<'_>,
        substitutions: &[RustIrMockSubstitution],
        body: &RustIrExpr,
    ) -> TypedValue {
        // Track which substitutions are snapshot mocks (they save via the helper).
        let mut saved: Vec<(&str, Option<TypedValue>)> = Vec::new();

        for sub in substitutions {
            if sub.snapshot {
                // Snapshot mock: rt_snapshot_mock_install saves old and installs wrapper.
                let (path_ptr, path_len) = self.embed_str(builder, sub.path.as_bytes());
                let call = builder.ins().call(
                    self.helpers.rt_snapshot_mock_install,
                    &[self.ctx_param, path_ptr, path_len],
                );
                let old_val = TypedValue::boxed(builder.inst_results(call)[0]);
                saved.push((&sub.path, Some(old_val)));
            } else {
                // Regular mock: save old, install new.
                let old_val = self.lower_global(builder, &sub.path);
                saved.push((&sub.path, Some(old_val)));
                if let Some(ref value_expr) = sub.value {
                    let mock_val = self.lower_expr(builder, value_expr);
                    self.emit_set_global(builder, &sub.path, mock_val.val);
                }
            }
        }

        // Evaluate body.
        let result = self.lower_expr(builder, body);

        // Restore original values (reverse order for nested correctness).
        for (path, old_val) in saved.into_iter().rev() {
            // For snapshot mocks, flush recordings before restore.
            if substitutions.iter().any(|s| s.path == path && s.snapshot) {
                let (path_ptr, path_len) = self.embed_str(builder, path.as_bytes());
                builder.ins().call(
                    self.helpers.rt_snapshot_mock_flush,
                    &[self.ctx_param, path_ptr, path_len],
                );
            }
            if let Some(old) = old_val {
                self.emit_set_global(builder, path, old.val);
            }
        }

        result
    }
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

        // Fallback: lambda not found in compiled_lambdas — this should not happen
        // if lambda pre-compilation is correct.
        eprintln!(
            "warning[JIT] lambda fallback: key 0x{:x} not found in compiled_lambdas",
            key
        );
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
                    let arg_tv = self.lower_expr(builder, arg);
                    return self.emit_direct_call_typed(builder, &info, &[arg_tv]);
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
                    let arg_tvs: Vec<TypedValue> =
                        args.iter().map(|a| self.lower_expr(builder, a)).collect();
                    return self.emit_direct_call_typed(builder, &info, &arg_tvs);
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
    /// `rt_get_global` + `rt_apply`. Arguments must be pre-lowered to avoid
    /// SSA value ordering issues when nested calls appear as arguments.
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

            // Save outer scope so pattern bindings don't leak between arms
            // or into code after the match.
            let saved_locals = self.locals.clone();

            // Bind pattern variables
            self.bind_pattern(builder, &arm.pattern, scrut_val);

            // Perceus: if the scrutinee is consumed, generate a reuse token.
            // The pattern has already extracted all needed fields, so the
            // scrutinee's box can be recycled for the next allocation.
            // Skip when the pattern directly aliases the scrutinee (Var/At)
            // because bind_pattern stores the same pointer — rt_try_reuse
            // would overwrite the value with Unit while the bound variable
            // still references it.
            if scrut_is_last_use && !pattern_aliases_scrutinee(&arm.pattern) {
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

            // Restore outer scope: pattern bindings must not leak into the
            // next arm or into code following the match expression.
            self.locals = saved_locals;

            // Next arm block
            builder.switch_to_block(arm_next_block);
            builder.seal_block(arm_next_block);
        }

        // Fallthrough: non-exhaustive match → signal failure so
        // make_jit_builtin / apply_multi_clause can try the next clause.
        let call = builder
            .ins()
            .call(self.helpers.rt_signal_match_fail, &[self.ctx_param]);
        let fail_val = builder.inst_results(call)[0];
        builder.def_var(result_var, fail_val);
        builder.ins().jump(merge_block, &[]);

        builder.switch_to_block(merge_block);
        builder.seal_block(merge_block);
        TypedValue::boxed(builder.use_var(result_var))
    }

}
