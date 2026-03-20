#[derive(Clone, Debug)]
struct FlowCarrierInfo {
    full_ty: Type,
    value_ty: Type,
    effect_like: bool,
    supports_functor: bool,
    supports_chain: bool,
}

#[derive(Clone, Debug)]
enum FlowExprKind {
    Pure(Type),
    Carrier(FlowCarrierInfo),
}

struct FlowBuiltExpr {
    expr: Expr,
    kind: FlowExprKind,
}

#[derive(Default)]
struct FlowDesugarCtx {
    next_temp: usize,
}

impl FlowDesugarCtx {
    fn fresh_name(&mut self, prefix: &str) -> String {
        let name = format!("__flow_{prefix}_{}", self.next_temp);
        self.next_temp += 1;
        name
    }

    fn fresh_ident(&mut self, prefix: &str, span: &Span) -> SpannedName {
        SpannedName {
            name: self.fresh_name(prefix),
            span: span.clone(),
        }
    }
}

#[derive(Clone)]
struct FlowBindingInfo {
    name: SpannedName,
    ty: Type,
}

struct FlowSubject {
    expr: Expr,
    ty: Type,
}

struct FlowTail<'a> {
    env: &'a TypeEnv,
    remaining: &'a [FlowLine],
    ctx: &'a mut FlowDesugarCtx,
}

const FLOW_STATE_SUBJECT_FIELD: &str = "__subject";

impl TypeChecker {
    pub(super) fn desugar_flow_expr(
        &mut self,
        expr: Expr,
        _expected: Option<Type>,
        env: &TypeEnv,
    ) -> Result<Expr, TypeError> {
        let Expr::Flow { root, lines, .. } = expr else {
            return Ok(expr);
        };
        let mut ctx = FlowDesugarCtx::default();
        self.desugar_flow_from_root(*root, &lines, env, &mut ctx)
    }

    fn desugar_flow_from_root(
        &mut self,
        root: Expr,
        lines: &[FlowLine],
        env: &TypeEnv,
        ctx: &mut FlowDesugarCtx,
    ) -> Result<Expr, TypeError> {
        if lines.is_empty() {
            return Ok(root);
        }
        let root_ty = self.infer_expr_ephemeral(&root, env)?;
        match self.classify_flow_type(root_ty.clone()) {
            FlowExprKind::Carrier(info) if info.effect_like => {
                let root_span = expr_span(&root);
                let subject = ctx.fresh_ident("root", &root_span);
                let mut effect_env = env.clone();
                effect_env.insert(subject.name.clone(), Scheme::mono(info.value_ty.clone()));
                let mut items = vec![BlockItem::Bind {
                    pattern: Pattern::Ident(subject.clone()),
                    expr: root,
                    span: root_span.clone(),
                }];
                items.extend(self.build_effect_tail(
                    Expr::Ident(subject),
                    info.value_ty.clone(),
                    &effect_env,
                    lines,
                    ctx,
                )?);
                Ok(self.effect_block(items, root_span))
            }
            FlowExprKind::Carrier(info) => {
                let param = ctx.fresh_ident("subject", &expr_span(&root));
                let mut body_env = env.clone();
                body_env.insert(param.name.clone(), Scheme::mono(info.value_ty.clone()));
                let body = self.build_generic_body(
                    &param,
                    FlowSubject {
                        expr: Expr::Ident(param.clone()),
                        ty: info.value_ty.clone(),
                    },
                    FlowTail {
                        env: &body_env,
                        remaining: lines,
                        ctx,
                    },
                    Vec::new(),
                )?;
                self.build_generic_continuation_expr(root, &info, param, body, &body_env)
            }
            FlowExprKind::Pure(_) => Ok(self.build_pure_flow(root, root_ty, env, lines, ctx)?.expr),
        }
    }

    fn build_pure_flow(
        &mut self,
        current_expr: Expr,
        current_ty: Type,
        env: &TypeEnv,
        lines: &[FlowLine],
        ctx: &mut FlowDesugarCtx,
    ) -> Result<FlowBuiltExpr, TypeError> {
        if lines.is_empty() {
            return Ok(FlowBuiltExpr {
                expr: current_expr,
                kind: FlowExprKind::Pure(current_ty),
            });
        }

        match &lines[0] {
            FlowLine::Anchor(_) => {
                self.build_pure_flow(current_expr, current_ty, env, &lines[1..], ctx)
            }
            FlowLine::Recover(arm) => Err(TypeError {
                span: arm.span.clone(),
                message: "`!|>` must immediately follow `?|>`".to_string(),
                expected: None,
                found: None,
            }),
            FlowLine::Branch(_) => {
                let (branch_expr, consumed) =
                    self.build_branch_group_expr(current_expr.clone(), env, lines, ctx)?;
                self.continue_pure_with_expr(branch_expr, env, &lines[consumed..], ctx)
            }
            FlowLine::Guard(guard) => {
                if guard.fail_expr.is_none() {
                    return Err(TypeError {
                        span: guard.span.clone(),
                        message: "guard lines without `or fail ...` are only supported inside `*|>` fan-out bodies".to_string(),
                        expected: None,
                        found: None,
                    });
                }
                let fail_value = self.desugar_nested_flow_expr(
                    guard.fail_expr.clone().expect("checked guard fail expr"),
                    env,
                )?;
                let mut items = vec![BlockItem::Given {
                    cond: self.build_guard_predicate_expr(
                        current_expr.clone(),
                        &guard.predicate,
                        env,
                    )?,
                    fail_expr: self.fail_call(fail_value, guard.span.clone()),
                    span: guard.span.clone(),
                }];
                items.extend(self.build_effect_tail(
                    current_expr,
                    current_ty,
                    env,
                    &lines[1..],
                    ctx,
                )?);
                let block = self.effect_block(items, guard.span.clone());
                self.classify_flow_expr(&block, env)
            }
            FlowLine::Step(step) => match step.kind {
                FlowStepKind::Flow => {
                    let step_expr =
                        self.build_step_value_expr(current_expr.clone(), &step.expr, env)?;
                    let step_expr = self.apply_line_modifiers(
                        step_expr,
                        &step.modifiers,
                        env,
                        step.span.clone(),
                    )?;
                    match self.classify_flow_expr(&step_expr, env)? {
                        FlowBuiltExpr {
                            expr,
                            kind: FlowExprKind::Pure(ty),
                        } => self.continue_pure_flow_step(
                            expr,
                            ty,
                            step.binding.as_ref(),
                            FlowTail {
                                env,
                                remaining: &lines[1..],
                                ctx,
                            },
                            step.span.clone(),
                        ),
                        FlowBuiltExpr {
                            expr,
                            kind: FlowExprKind::Carrier(info),
                        } => {
                            if info.effect_like {
                                self.wrap_effect_from_pure(
                                    expr,
                                    Some(&info),
                                    step.binding.as_ref(),
                                    FlowSubject {
                                        expr: Expr::default_unit(step.span.clone()),
                                        ty: Type::con("Unit"),
                                    },
                                    FlowTail {
                                        env,
                                        remaining: &lines[1..],
                                        ctx,
                                    },
                                    step.span.clone(),
                                )
                            } else {
                                self.wrap_generic_from_pure(
                                    expr,
                                    &info,
                                    step.binding.as_ref(),
                                    FlowTail {
                                        env,
                                        remaining: &lines[1..],
                                        ctx,
                                    },
                                    Vec::new(),
                                    None,
                                )
                            }
                        }
                    }
                }
                FlowStepKind::Tap => {
                    let tap_expr =
                        self.build_step_value_expr(current_expr.clone(), &step.expr, env)?;
                    let tap_expr = self.apply_line_modifiers(
                        tap_expr,
                        &step.modifiers,
                        env,
                        step.span.clone(),
                    )?;
                    match self.classify_flow_expr(&tap_expr, env)? {
                        FlowBuiltExpr {
                            expr,
                            kind: FlowExprKind::Pure(ty),
                        } => {
                            let mut items = Vec::new();
                            if let Some(binding) = &step.binding {
                                items.push(BlockItem::Let {
                                    pattern: Pattern::Ident(binding.name.clone()),
                                    expr,
                                    span: binding.span.clone(),
                                });
                            } else {
                                items.push(BlockItem::Let {
                                    pattern: Pattern::Wildcard(step.span.clone()),
                                    expr,
                                    span: step.span.clone(),
                                });
                            }
                            let mut next_env = env.clone();
                            if let Some(binding) = &step.binding {
                                next_env.insert(binding.name.name.clone(), Scheme::mono(ty));
                            }
                            let rest = self.build_pure_flow(
                                current_expr,
                                current_ty,
                                &next_env,
                                &lines[1..],
                                ctx,
                            )?;
                            items.push(BlockItem::Expr {
                                expr: rest.expr,
                                span: step.span.clone(),
                            });
                            let block = Expr::Block {
                                kind: BlockKind::Plain,
                                items,
                                span: step.span.clone(),
                            };
                            self.classify_flow_expr(&block, env)
                        }
                        FlowBuiltExpr {
                            expr,
                            kind: FlowExprKind::Carrier(info),
                        } => {
                            if info.effect_like {
                                let mut next_env = env.clone();
                                let pattern = if let Some(binding) = &step.binding {
                                    next_env.insert(
                                        binding.name.name.clone(),
                                        Scheme::mono(info.value_ty.clone()),
                                    );
                                    Pattern::Ident(binding.name.clone())
                                } else {
                                    Pattern::Wildcard(step.span.clone())
                                };
                                let mut items = vec![BlockItem::Bind {
                                    pattern,
                                    expr,
                                    span: step.span.clone(),
                                }];
                                items.extend(self.build_effect_tail(
                                    current_expr,
                                    current_ty,
                                    &next_env,
                                    &lines[1..],
                                    ctx,
                                )?);
                                let block = self.effect_block(items, step.span.clone());
                                self.classify_flow_expr(&block, env)
                            } else {
                                let binding_infos = step
                                    .binding
                                    .as_ref()
                                    .map(|binding| {
                                        vec![FlowBindingInfo {
                                            name: binding.name.clone(),
                                            ty: info.value_ty.clone(),
                                        }]
                                    })
                                    .unwrap_or_default();
                                self.wrap_generic_from_pure(
                                    expr,
                                    &info,
                                    step.binding.as_ref(),
                                    FlowTail {
                                        env,
                                        remaining: &lines[1..],
                                        ctx,
                                    },
                                    binding_infos,
                                    Some(FlowSubject {
                                        expr: current_expr,
                                        ty: current_ty,
                                    }),
                                )
                            }
                        }
                    }
                }
                FlowStepKind::Attempt => {
                    let (attempt_expr, consumed) =
                        self.build_attempt_region_expr(current_expr.clone(), env, lines, ctx)?;
                    self.wrap_effect_from_pure(
                        attempt_expr,
                        None,
                        step.binding.as_ref(),
                        FlowSubject {
                            expr: Expr::default_unit(step.span.clone()),
                            ty: Type::con("Unit"),
                        },
                        FlowTail {
                            env,
                            remaining: &lines[consumed..],
                            ctx,
                        },
                        step.span.clone(),
                    )
                }
                FlowStepKind::Applicative => {
                    let (expr, kind, bindings, consumed) = self.build_applicative_group_expr(
                        current_expr.clone(),
                        current_ty.clone(),
                        env,
                        lines,
                        ctx,
                    )?;
                    match kind {
                        FlowExprKind::Carrier(info) if info.effect_like => self
                            .wrap_effect_state_from_pure(
                                expr,
                                bindings,
                                FlowSubject {
                                    expr: current_expr,
                                    ty: current_ty,
                                },
                                FlowTail {
                                    env,
                                    remaining: &lines[consumed..],
                                    ctx,
                                },
                                step.span.clone(),
                            ),
                        FlowExprKind::Carrier(info) => self.wrap_generic_from_pure(
                            expr,
                            &info,
                            None,
                            FlowTail {
                                env,
                                remaining: &lines[consumed..],
                                ctx,
                            },
                            bindings,
                            Some(FlowSubject {
                                expr: current_expr,
                                ty: current_ty,
                            }),
                        ),
                        FlowExprKind::Pure(_) => {
                            unreachable!("applicative blocks always produce a carrier")
                        }
                    }
                }
                FlowStepKind::FanOut => {
                    let fanout_expr =
                        self.build_fanout_expr(current_expr.clone(), current_ty, step, env, ctx)?;
                    self.continue_pure_with_expr(fanout_expr, env, &lines[1..], ctx)
                }
            },
        }
    }

    fn continue_pure_with_expr(
        &mut self,
        expr: Expr,
        env: &TypeEnv,
        remaining: &[FlowLine],
        ctx: &mut FlowDesugarCtx,
    ) -> Result<FlowBuiltExpr, TypeError> {
        let ty = self.infer_expr_ephemeral(&expr, env)?;
        let expr_span_value = expr_span(&expr);
        match self.classify_flow_type(ty.clone()) {
            FlowExprKind::Pure(_) => self.build_pure_flow(expr, ty, env, remaining, ctx),
            FlowExprKind::Carrier(info) if info.effect_like => {
                if remaining.is_empty() {
                    Ok(FlowBuiltExpr {
                        expr,
                        kind: FlowExprKind::Carrier(info),
                    })
                } else {
                    let subject = ctx.fresh_ident("subject", &expr_span(&expr));
                    let mut effect_env = env.clone();
                    effect_env.insert(subject.name.clone(), Scheme::mono(info.value_ty.clone()));
                    let mut items = vec![BlockItem::Bind {
                        pattern: Pattern::Ident(subject.clone()),
                        expr,
                        span: subject.span.clone(),
                    }];
                    items.extend(self.build_effect_tail(
                        Expr::Ident(subject),
                        info.value_ty,
                        &effect_env,
                        remaining,
                        ctx,
                    )?);
                    let block = self.effect_block(items, expr_span_value);
                    self.classify_flow_expr(&block, env)
                }
            }
            FlowExprKind::Carrier(info) => {
                let param = ctx.fresh_ident("subject", &expr_span(&expr));
                let mut body_env = env.clone();
                body_env.insert(param.name.clone(), Scheme::mono(info.value_ty.clone()));
                let body = self.build_generic_body(
                    &param,
                    FlowSubject {
                        expr: Expr::Ident(param.clone()),
                        ty: info.value_ty.clone(),
                    },
                    FlowTail {
                        env: &body_env,
                        remaining,
                        ctx,
                    },
                    Vec::new(),
                )?;
                let continued =
                    self.build_generic_continuation_expr(expr, &info, param, body, &body_env)?;
                self.classify_flow_expr(&continued, env)
            }
        }
    }

    fn continue_pure_flow_step(
        &mut self,
        expr: Expr,
        ty: Type,
        binding: Option<&FlowBinding>,
        tail: FlowTail<'_>,
        span: Span,
    ) -> Result<FlowBuiltExpr, TypeError> {
        let FlowTail {
            env,
            remaining,
            ctx,
        } = tail;
        if let Some(binding) = binding {
            let mut next_env = env.clone();
            next_env.insert(binding.name.name.clone(), Scheme::mono(ty.clone()));
            let rest = self.build_pure_flow(
                Expr::Ident(binding.name.clone()),
                ty.clone(),
                &next_env,
                remaining,
                ctx,
            )?;
            let block = Expr::Block {
                kind: BlockKind::Plain,
                items: vec![
                    BlockItem::Let {
                        pattern: Pattern::Ident(binding.name.clone()),
                        expr,
                        span: binding.span.clone(),
                    },
                    BlockItem::Expr {
                        expr: rest.expr,
                        span: span.clone(),
                    },
                ],
                span: span.clone(),
            };
            self.classify_flow_expr(&block, env)
        } else {
            self.build_pure_flow(expr, ty, env, remaining, ctx)
        }
    }

    fn build_effect_tail(
        &mut self,
        current_expr: Expr,
        current_ty: Type,
        env: &TypeEnv,
        lines: &[FlowLine],
        ctx: &mut FlowDesugarCtx,
    ) -> Result<Vec<BlockItem>, TypeError> {
        if lines.is_empty() {
            let current_span = expr_span(&current_expr);
            return Ok(vec![BlockItem::Expr {
                expr: self.pure_call(current_expr, current_span.clone()),
                span: current_span,
            }]);
        }

        match &lines[0] {
            FlowLine::Anchor(_) => {
                self.build_effect_tail(current_expr, current_ty, env, &lines[1..], ctx)
            }
            FlowLine::Recover(arm) => Err(TypeError {
                span: arm.span.clone(),
                message: "`!|>` must immediately follow `?|>`".to_string(),
                expected: None,
                found: None,
            }),
            FlowLine::Branch(_) => {
                let (branch_expr, consumed) =
                    self.build_branch_group_expr(current_expr.clone(), env, lines, ctx)?;
                self.append_effect_step(branch_expr, env, &lines[consumed..], ctx)
            }
            FlowLine::Guard(guard) => {
                if guard.fail_expr.is_none() {
                    return Err(TypeError {
                        span: guard.span.clone(),
                        message: "guard lines without `or fail ...` are only supported inside `*|>` fan-out bodies".to_string(),
                        expected: None,
                        found: None,
                    });
                }
                let given = BlockItem::Given {
                    cond: self.build_guard_predicate_expr(
                        current_expr.clone(),
                        &guard.predicate,
                        env,
                    )?,
                    fail_expr: {
                        let fail_value = self.desugar_nested_flow_expr(
                            guard.fail_expr.clone().expect("checked guard fail expr"),
                            env,
                        )?;
                        self.fail_call(fail_value, guard.span.clone())
                    },
                    span: guard.span.clone(),
                };
                let mut rest = vec![given];
                rest.extend(self.build_effect_tail(
                    current_expr,
                    current_ty,
                    env,
                    &lines[1..],
                    ctx,
                )?);
                Ok(rest)
            }
            FlowLine::Step(step) => match step.kind {
                FlowStepKind::Flow => {
                    let step_expr =
                        self.build_step_value_expr(current_expr.clone(), &step.expr, env)?;
                    let step_expr = self.apply_line_modifiers(
                        step_expr,
                        &step.modifiers,
                        env,
                        step.span.clone(),
                    )?;
                    self.append_effect_flow_step(
                        step_expr,
                        step.binding.as_ref(),
                        env,
                        &lines[1..],
                        ctx,
                        step.span.clone(),
                    )
                }
                FlowStepKind::Tap => {
                    let tap_expr =
                        self.build_step_value_expr(current_expr.clone(), &step.expr, env)?;
                    let tap_expr = self.apply_line_modifiers(
                        tap_expr,
                        &step.modifiers,
                        env,
                        step.span.clone(),
                    )?;
                    self.append_effect_tap_step(
                        tap_expr,
                        FlowSubject {
                            expr: current_expr,
                            ty: current_ty,
                        },
                        step.binding.as_ref(),
                        FlowTail {
                            env,
                            remaining: &lines[1..],
                            ctx,
                        },
                        step.span.clone(),
                    )
                }
                FlowStepKind::Attempt => {
                    let (attempt_expr, consumed) =
                        self.build_attempt_region_expr(current_expr.clone(), env, lines, ctx)?;
                    self.append_effect_flow_step(
                        attempt_expr,
                        step.binding.as_ref(),
                        env,
                        &lines[consumed..],
                        ctx,
                        step.span.clone(),
                    )
                }
                FlowStepKind::Applicative => {
                    let (expr, kind, bindings, consumed) = self.build_applicative_group_expr(
                        current_expr.clone(),
                        current_ty.clone(),
                        env,
                        lines,
                        ctx,
                    )?;
                    match kind {
                        FlowExprKind::Carrier(info) if info.effect_like => {
                            self.append_effect_state_step(
                                expr,
                                bindings,
                                FlowSubject {
                                    expr: current_expr,
                                    ty: current_ty,
                                },
                                FlowTail {
                                    env,
                                    remaining: &lines[consumed..],
                                    ctx,
                                },
                                step.span.clone(),
                            )
                        }
                        FlowExprKind::Carrier(_) => Err(TypeError {
                            span: step.span.clone(),
                            message: "mixing generic applicative carriers into an effect-derived flow is not supported yet".to_string(),
                            expected: None,
                            found: None,
                        }),
                        FlowExprKind::Pure(_) => unreachable!("applicative blocks always produce a carrier"),
                    }
                }
                FlowStepKind::FanOut => {
                    let fanout_expr =
                        self.build_fanout_expr(current_expr.clone(), current_ty, step, env, ctx)?;
                    self.append_effect_step(fanout_expr, env, &lines[1..], ctx)
                }
            },
        }
    }

    fn append_effect_step(
        &mut self,
        expr: Expr,
        env: &TypeEnv,
        remaining: &[FlowLine],
        ctx: &mut FlowDesugarCtx,
    ) -> Result<Vec<BlockItem>, TypeError> {
        let ty = self.infer_expr_ephemeral(&expr, env)?;
        let expr_span_value = expr_span(&expr);
        match self.classify_flow_type(ty.clone()) {
            FlowExprKind::Pure(ty) => {
                let name = ctx.fresh_ident("value", &expr_span(&expr));
                let mut next_env = env.clone();
                next_env.insert(name.name.clone(), Scheme::mono(ty.clone()));
                let mut items = vec![BlockItem::Let {
                    pattern: Pattern::Ident(name.clone()),
                    expr,
                    span: name.span.clone(),
                }];
                items.extend(self.build_effect_tail(
                    Expr::Ident(name),
                    ty,
                    &next_env,
                    remaining,
                    ctx,
                )?);
                Ok(items)
            }
            FlowExprKind::Carrier(info) if info.effect_like => {
                if remaining.is_empty() {
                    Ok(vec![BlockItem::Expr {
                        expr,
                        span: expr_span_value,
                    }])
                } else {
                    let name = ctx.fresh_ident("value", &expr_span(&expr));
                    let mut next_env = env.clone();
                    next_env.insert(name.name.clone(), Scheme::mono(info.value_ty.clone()));
                    let mut items = vec![BlockItem::Bind {
                        pattern: Pattern::Ident(name.clone()),
                        expr,
                        span: name.span.clone(),
                    }];
                    items.extend(self.build_effect_tail(
                        Expr::Ident(name),
                        info.value_ty,
                        &next_env,
                        remaining,
                        ctx,
                    )?);
                    Ok(items)
                }
            }
            FlowExprKind::Carrier(_) => Err(TypeError {
                span: expr_span(&expr),
                message:
                    "mixing non-effect carriers into an effect-derived flow is not supported yet"
                        .to_string(),
                expected: None,
                found: None,
            }),
        }
    }

    fn append_effect_flow_step(
        &mut self,
        expr: Expr,
        binding: Option<&FlowBinding>,
        env: &TypeEnv,
        remaining: &[FlowLine],
        ctx: &mut FlowDesugarCtx,
        span: Span,
    ) -> Result<Vec<BlockItem>, TypeError> {
        let ty = self.infer_expr_ephemeral(&expr, env)?;
        let kind = self.classify_flow_type(ty.clone());
        match kind {
            FlowExprKind::Pure(ty) => {
                if remaining.is_empty() {
                    return Ok(vec![BlockItem::Expr {
                        expr: self.pure_call(expr, span.clone()),
                        span,
                    }]);
                }
                let binder = binding
                    .map(|binding| binding.name.clone())
                    .unwrap_or_else(|| ctx.fresh_ident("value", &span));
                let mut next_env = env.clone();
                next_env.insert(binder.name.clone(), Scheme::mono(ty.clone()));
                let mut items = vec![BlockItem::Let {
                    pattern: Pattern::Ident(binder.clone()),
                    expr,
                    span: binder.span.clone(),
                }];
                items.extend(self.build_effect_tail(
                    Expr::Ident(binder),
                    ty,
                    &next_env,
                    remaining,
                    ctx,
                )?);
                Ok(items)
            }
            FlowExprKind::Carrier(info) if info.effect_like => {
                if remaining.is_empty() {
                    Ok(vec![BlockItem::Expr { expr, span }])
                } else {
                    let binder = binding
                        .map(|binding| binding.name.clone())
                        .unwrap_or_else(|| ctx.fresh_ident("value", &span));
                    let mut next_env = env.clone();
                    next_env.insert(binder.name.clone(), Scheme::mono(info.value_ty.clone()));
                    let mut items = vec![BlockItem::Bind {
                        pattern: Pattern::Ident(binder.clone()),
                        expr,
                        span: binder.span.clone(),
                    }];
                    items.extend(self.build_effect_tail(
                        Expr::Ident(binder),
                        info.value_ty,
                        &next_env,
                        remaining,
                        ctx,
                    )?);
                    Ok(items)
                }
            }
            FlowExprKind::Carrier(_) => Err(TypeError {
                span,
                message:
                    "mixing non-effect carriers into an effect-derived flow is not supported yet"
                        .to_string(),
                expected: None,
                found: None,
            }),
        }
    }

    fn append_effect_tap_step(
        &mut self,
        expr: Expr,
        current_subject: FlowSubject,
        binding: Option<&FlowBinding>,
        tail: FlowTail<'_>,
        span: Span,
    ) -> Result<Vec<BlockItem>, TypeError> {
        let FlowTail {
            env,
            remaining,
            ctx,
        } = tail;
        let FlowSubject {
            expr: current_expr,
            ty: current_ty,
        } = current_subject;
        let ty = self.infer_expr_ephemeral(&expr, env)?;
        match self.classify_flow_type(ty.clone()) {
            FlowExprKind::Pure(ty) => {
                let pattern = binding
                    .map(|binding| Pattern::Ident(binding.name.clone()))
                    .unwrap_or_else(|| Pattern::Wildcard(span.clone()));
                let mut next_env = env.clone();
                if let Some(binding) = binding {
                    next_env.insert(binding.name.name.clone(), Scheme::mono(ty));
                }
                let mut items = vec![BlockItem::Let {
                    pattern,
                    expr,
                    span: span.clone(),
                }];
                items.extend(self.build_effect_tail(
                    current_expr,
                    current_ty,
                    &next_env,
                    remaining,
                    ctx,
                )?);
                Ok(items)
            }
            FlowExprKind::Carrier(info) if info.effect_like => {
                let pattern = binding
                    .map(|binding| Pattern::Ident(binding.name.clone()))
                    .unwrap_or_else(|| Pattern::Wildcard(span.clone()));
                let mut next_env = env.clone();
                if let Some(binding) = binding {
                    next_env.insert(binding.name.name.clone(), Scheme::mono(info.value_ty));
                }
                let mut items = vec![BlockItem::Bind {
                    pattern,
                    expr,
                    span: span.clone(),
                }];
                items.extend(self.build_effect_tail(
                    current_expr,
                    current_ty,
                    &next_env,
                    remaining,
                    ctx,
                )?);
                Ok(items)
            }
            FlowExprKind::Carrier(_) => Err(TypeError {
                span,
                message: "generic tap carriers are not supported inside an effect-derived flow"
                    .to_string(),
                expected: None,
                found: None,
            }),
        }
    }

    fn append_effect_state_step(
        &mut self,
        expr: Expr,
        bindings: Vec<FlowBindingInfo>,
        original_subject: FlowSubject,
        tail: FlowTail<'_>,
        span: Span,
    ) -> Result<Vec<BlockItem>, TypeError> {
        let FlowTail {
            env,
            remaining,
            ctx,
        } = tail;
        let FlowSubject {
            ty: original_subject_ty,
            ..
        } = original_subject;
        let expr_ty = self.infer_expr_ephemeral(&expr, env)?;
        let FlowExprKind::Carrier(info) = self.classify_flow_type(expr_ty) else {
            unreachable!("state block should be effectlike carrier");
        };
        let state = ctx.fresh_ident("state", &span);
        let mut state_env = env.clone();
        state_env.insert(state.name.clone(), Scheme::mono(info.value_ty.clone()));
        let subject_expr = self.state_field_expr(&state, FLOW_STATE_SUBJECT_FIELD);
        let mut items = vec![BlockItem::Bind {
            pattern: Pattern::Ident(state.clone()),
            expr,
            span: span.clone(),
        }];
        let mut next_env = state_env.clone();
        for binding in bindings {
            let field_expr = self.state_field_expr(&state, &binding.name.name);
            next_env.insert(binding.name.name.clone(), Scheme::mono(binding.ty));
            items.push(BlockItem::Let {
                pattern: Pattern::Ident(binding.name),
                expr: field_expr,
                span: span.clone(),
            });
        }
        let subject_ty = self
            .infer_expr_ephemeral(&subject_expr, &state_env)
            .unwrap_or(original_subject_ty);
        let fallback_subject = if remaining.is_empty() {
            subject_expr
        } else {
            self.state_field_expr(&state, FLOW_STATE_SUBJECT_FIELD)
        };
        items.extend(self.build_effect_tail(
            fallback_subject,
            subject_ty,
            &next_env,
            remaining,
            ctx,
        )?);
        Ok(items)
    }

    fn wrap_effect_from_pure(
        &mut self,
        expr: Expr,
        info: Option<&FlowCarrierInfo>,
        binding: Option<&FlowBinding>,
        fallback_subject: FlowSubject,
        tail: FlowTail<'_>,
        span: Span,
    ) -> Result<FlowBuiltExpr, TypeError> {
        let FlowTail {
            env,
            remaining,
            ctx,
        } = tail;
        let FlowSubject {
            expr: fallback_subject,
            ty: fallback_subject_ty,
        } = fallback_subject;
        let expr_ty = self.infer_expr_ephemeral(&expr, env)?;
        let FlowExprKind::Carrier(info) = info
            .cloned()
            .map(FlowExprKind::Carrier)
            .unwrap_or_else(|| self.classify_flow_type(expr_ty.clone()))
        else {
            return Err(TypeError {
                span,
                message: format!(
                    "effect wrapper expected an effect-like flow step, found {:?}",
                    expr
                ),
                expected: None,
                found: Some(Box::new(expr_ty)),
            });
        };
        if remaining.is_empty()
            && binding.is_none()
            && matches!(&fallback_subject, Expr::Ident(name) if name.name == "Unit")
        {
            return Ok(FlowBuiltExpr {
                expr,
                kind: FlowExprKind::Carrier(info),
            });
        }
        let binder = binding
            .map(|binding| binding.name.clone())
            .unwrap_or_else(|| ctx.fresh_ident("value", &span));
        let mut next_env = env.clone();
        next_env.insert(binder.name.clone(), Scheme::mono(info.value_ty.clone()));
        let mut block_items = vec![BlockItem::Bind {
            pattern: Pattern::Ident(binder.clone()),
            expr,
            span: span.clone(),
        }];
        let use_bound_subject = binding.is_some()
            || matches!(&fallback_subject, Expr::Ident(name) if name.name == "Unit");
        let subject_expr = if use_bound_subject {
            Expr::Ident(binder)
        } else {
            fallback_subject
        };
        let subject_ty = if use_bound_subject {
            info.value_ty
        } else {
            fallback_subject_ty
        };
        block_items.extend(self.build_effect_tail(
            subject_expr,
            subject_ty,
            &next_env,
            remaining,
            ctx,
        )?);
        let block = self.effect_block(block_items, span);
        self.classify_flow_expr(&block, env)
    }

    fn wrap_effect_state_from_pure(
        &mut self,
        expr: Expr,
        bindings: Vec<FlowBindingInfo>,
        original_subject: FlowSubject,
        tail: FlowTail<'_>,
        span: Span,
    ) -> Result<FlowBuiltExpr, TypeError> {
        let env = tail.env;
        let items = self.append_effect_state_step(
            expr,
            bindings,
            original_subject,
            tail,
            span.clone(),
        )?;
        let block = self.effect_block(items, span);
        self.classify_flow_expr(&block, env)
    }

    fn wrap_generic_from_pure(
        &mut self,
        expr: Expr,
        info: &FlowCarrierInfo,
        binding: Option<&FlowBinding>,
        tail: FlowTail<'_>,
        extra_bindings: Vec<FlowBindingInfo>,
        fallback_subject: Option<FlowSubject>,
    ) -> Result<FlowBuiltExpr, TypeError> {
        let FlowTail {
            env,
            remaining,
            ctx,
        } = tail;
        let param = binding
            .map(|binding| binding.name.clone())
            .unwrap_or_else(|| ctx.fresh_ident("value", &expr_span(&expr)));
        let mut body_env = env.clone();
        body_env.insert(param.name.clone(), Scheme::mono(info.value_ty.clone()));
        let subject = fallback_subject.unwrap_or_else(|| FlowSubject {
            expr: Expr::Ident(param.clone()),
            ty: info.value_ty.clone(),
        });
        let body = self.build_generic_body(
            &param,
            subject,
            FlowTail {
                env: &body_env,
                remaining,
                ctx,
            },
            extra_bindings,
        )?;
        let continued = self.build_generic_continuation_expr(expr, info, param, body, &body_env)?;
        self.classify_flow_expr(&continued, env)
    }

    fn build_generic_body(
        &mut self,
        param: &SpannedName,
        subject: FlowSubject,
        tail: FlowTail<'_>,
        extra_bindings: Vec<FlowBindingInfo>,
    ) -> Result<Expr, TypeError> {
        let FlowTail {
            env,
            remaining,
            ctx,
        } = tail;
        let FlowSubject {
            expr: subject_expr,
            ty: current_ty,
        } = subject;
        let mut next_env = env.clone();
        let mut items = Vec::new();
        for binding in extra_bindings {
            next_env.insert(binding.name.name.clone(), Scheme::mono(binding.ty));
            items.push(BlockItem::Let {
                pattern: Pattern::Ident(binding.name.clone()),
                expr: self.state_field_expr(param, &binding.name.name),
                span: binding.name.span.clone(),
            });
        }
        let subject_name = ctx.fresh_ident("subject", &param.span);
        next_env.insert(subject_name.name.clone(), Scheme::mono(current_ty.clone()));
        items.push(BlockItem::Let {
            pattern: Pattern::Ident(subject_name.clone()),
            expr: subject_expr,
            span: subject_name.span.clone(),
        });
        let rest = self.build_pure_flow(
            Expr::Ident(subject_name),
            current_ty,
            &next_env,
            remaining,
            ctx,
        )?;
        items.push(BlockItem::Expr {
            expr: rest.expr,
            span: param.span.clone(),
        });
        Ok(Expr::Block {
            kind: BlockKind::Plain,
            items,
            span: param.span.clone(),
        })
    }

    fn build_generic_continuation_expr(
        &mut self,
        carrier_expr: Expr,
        info: &FlowCarrierInfo,
        param: SpannedName,
        body: Expr,
        env: &TypeEnv,
    ) -> Result<Expr, TypeError> {
        let body_kind = self.classify_flow_expr(&body, env)?.kind;
        let lambda = Expr::Lambda {
            params: vec![Pattern::Ident(param.clone())],
            body: Box::new(body),
            span: param.span.clone(),
        };
        let call = match body_kind {
            FlowExprKind::Pure(_) => {
                if !info.supports_functor {
                    return Err(TypeError {
                        span: param.span.clone(),
                        message: "this flow step needs a `map`/Functor instance to continue with pure lines".to_string(),
                        expected: None,
                        found: None,
                    });
                }
                self.named_call_expr("map", vec![lambda, carrier_expr], param.span.clone())
            }
            FlowExprKind::Carrier(_) => {
                if !info.supports_chain {
                    return Err(TypeError {
                        span: param.span.clone(),
                        message: "this flow step needs a `chain`/Monad instance to continue into another carrier-producing line".to_string(),
                        expected: None,
                        found: None,
                    });
                }
                self.named_call_expr("chain", vec![lambda, carrier_expr], param.span.clone())
            }
        };
        Ok(call)
    }

    fn build_step_value_expr(
        &mut self,
        current_expr: Expr,
        step_expr: &Expr,
        env: &TypeEnv,
    ) -> Result<Expr, TypeError> {
        let inner = self.desugar_nested_flow_expr(step_expr.clone(), env)?;
        Ok(Expr::Binary {
            op: "|>".to_string(),
            left: Box::new(current_expr),
            right: Box::new(self.normalize_pipe_transformer(&inner, env)),
            span: expr_span(&inner),
        })
    }

    fn build_guard_predicate_expr(
        &mut self,
        current_expr: Expr,
        predicate: &Expr,
        env: &TypeEnv,
    ) -> Result<Expr, TypeError> {
        let predicate = self.desugar_nested_flow_expr(predicate.clone(), env)?;
        Ok(Expr::Binary {
            op: "|>".to_string(),
            left: Box::new(current_expr),
            right: Box::new(self.normalize_pipe_transformer(&predicate, env)),
            span: expr_span(&predicate),
        })
    }

    fn build_branch_group_expr(
        &mut self,
        current_expr: Expr,
        env: &TypeEnv,
        lines: &[FlowLine],
        _ctx: &mut FlowDesugarCtx,
    ) -> Result<(Expr, usize), TypeError> {
        let mut arms = Vec::new();
        let mut consumed = 0;
        while let Some(FlowLine::Branch(arm)) = lines.get(consumed) {
            let arm_env = self.env_with_pattern_binders(env, &arm.pattern);
            let body =
                self.prepare_inline_arm_body(arm.body.clone(), &arm_env, current_expr.clone())?;
            let guard = match arm.guard.clone() {
                Some(guard) => Some(self.desugar_nested_flow_expr(guard, &arm_env)?),
                None => None,
            };
            arms.push(MatchArm {
                pattern: arm.pattern.clone(),
                guard,
                guard_negated: arm.guard_negated,
                body,
                span: arm.span.clone(),
            });
            consumed += 1;
        }
        if arms.is_empty() {
            return Err(TypeError {
                span: expr_span(&current_expr),
                message: "expected one or more `||>` branch arms".to_string(),
                expected: None,
                found: None,
            });
        }
        Ok((
            Expr::Match {
                scrutinee: Some(Box::new(current_expr)),
                arms,
                span: lines[0].branch_span(),
            },
            consumed,
        ))
    }

    fn build_attempt_region_expr(
        &mut self,
        current_expr: Expr,
        env: &TypeEnv,
        lines: &[FlowLine],
        ctx: &mut FlowDesugarCtx,
    ) -> Result<(Expr, usize), TypeError> {
        let FlowLine::Step(step) = &lines[0] else {
            unreachable!("attempt region must start with a step");
        };
        let raw_expr = self.build_step_value_expr(current_expr, &step.expr, env)?;
        let raw_effect_expr =
            self.apply_line_modifiers(raw_expr, &step.modifiers, env, step.span.clone())?;
        let effect_expr = self.reify_effect_candidate_expr(raw_effect_expr, step.span.clone());
        let attempt_expr = self.named_call_expr("attempt", vec![effect_expr], step.span.clone());
        let result_name = ctx.fresh_ident("attempt", &step.span);
        let value_name = ctx.fresh_ident("ok", &step.span);
        let err_name = ctx.fresh_ident("err", &step.span);
        let mut recover_arms = Vec::new();
        let mut consumed = 1;
        while let Some(FlowLine::Recover(arm)) = lines.get(consumed) {
            let arm_env = self.env_with_pattern_binders(env, &arm.pattern);
            let body = self.prepare_inline_arm_body(
                arm.body.clone(),
                &arm_env,
                Expr::Ident(err_name.clone()),
            )?;
            let body = match self.classify_flow_expr(&body, &arm_env)? {
                FlowBuiltExpr {
                    expr,
                    kind: FlowExprKind::Carrier(carrier),
                } if carrier.effect_like => {
                    self.ensure_effect_expr(expr, &carrier, arm.span.clone())?
                }
                FlowBuiltExpr { expr, .. } => self.pure_call(expr, arm.span.clone()),
            };
            recover_arms.push(MatchArm {
                pattern: arm.pattern.clone(),
                guard: arm.guard.clone(),
                guard_negated: arm.guard_negated,
                body,
                span: arm.span.clone(),
            });
            consumed += 1;
        }
        recover_arms.push(MatchArm {
            pattern: Pattern::Wildcard(step.span.clone()),
            guard: None,
            guard_negated: false,
            body: self.fail_call(Expr::Ident(err_name.clone()), step.span.clone()),
            span: step.span.clone(),
        });
        let match_expr = Expr::Match {
            scrutinee: Some(Box::new(Expr::Ident(result_name.clone()))),
            arms: vec![
                MatchArm {
                    pattern: Pattern::Constructor {
                        name: SpannedName {
                            name: "Ok".to_string(),
                            span: step.span.clone(),
                        },
                        args: vec![Pattern::Ident(value_name.clone())],
                        span: step.span.clone(),
                    },
                    guard: None,
                    guard_negated: false,
                    body: self.pure_call(Expr::Ident(value_name), step.span.clone()),
                    span: step.span.clone(),
                },
                MatchArm {
                    pattern: Pattern::Constructor {
                        name: SpannedName {
                            name: "Err".to_string(),
                            span: step.span.clone(),
                        },
                        args: vec![Pattern::Ident(err_name.clone())],
                        span: step.span.clone(),
                    },
                    guard: None,
                    guard_negated: false,
                    body: Expr::Match {
                        scrutinee: Some(Box::new(Expr::Ident(err_name))),
                        arms: recover_arms,
                        span: step.span.clone(),
                    },
                    span: step.span.clone(),
                },
            ],
            span: step.span.clone(),
        };
        Ok((
            self.effect_block(
                vec![
                    BlockItem::Bind {
                        pattern: Pattern::Ident(result_name),
                        expr: attempt_expr,
                        span: step.span.clone(),
                    },
                    BlockItem::Expr {
                        expr: match_expr,
                        span: step.span.clone(),
                    },
                ],
                step.span.clone(),
            ),
            consumed,
        ))
    }

    fn build_applicative_group_expr(
        &mut self,
        current_expr: Expr,
        current_ty: Type,
        env: &TypeEnv,
        lines: &[FlowLine],
        ctx: &mut FlowDesugarCtx,
    ) -> Result<(Expr, FlowExprKind, Vec<FlowBindingInfo>, usize), TypeError> {
        let mut steps = Vec::new();
        let mut bindings = Vec::new();
        let mut consumed = 0;
        while let Some(FlowLine::Step(step)) = lines.get(consumed) {
            if step.kind != FlowStepKind::Applicative {
                break;
            }
            if step
                .modifiers
                .iter()
                .any(|modifier| matches!(modifier, FlowModifier::Concurrent { .. }))
                && consumed > 0
            {
                return Err(TypeError {
                    span: step.span.clone(),
                    message: "`@concurrent` is only allowed on the first line of a contiguous `&|>` block".to_string(),
                    expected: None,
                    found: None,
                });
            }
            let step_expr = self.build_step_value_expr(current_expr.clone(), &step.expr, env)?;
            let step_expr =
                self.apply_line_modifiers(step_expr, &step.modifiers, env, step.span.clone())?;
            let kind = self.classify_flow_expr(&step_expr, env)?.kind;
            if let FlowExprKind::Carrier(info) = &kind {
                if info.effect_like {
                    return self
                        .build_effect_sibling_group_expr(
                            current_expr,
                            current_ty,
                            env,
                            lines,
                            ctx,
                            FlowStepKind::Applicative,
                        )
                        .map(|(expr, info, bindings, consumed)| {
                            (expr, FlowExprKind::Carrier(info), bindings, consumed)
                        });
                }
            }
            let binder = step
                .binding
                .as_ref()
                .map(|binding| binding.name.clone())
                .unwrap_or_else(|| ctx.fresh_ident("app", &step.span));
            let info = match kind {
                FlowExprKind::Carrier(info) => info,
                FlowExprKind::Pure(_) => FlowCarrierInfo {
                    full_ty: self.fresh_var(),
                    value_ty: self.fresh_var(),
                    effect_like: false,
                    supports_functor: true,
                    supports_chain: false,
                },
            };
            if step.binding.is_some() {
                bindings.push(FlowBindingInfo {
                    name: binder.clone(),
                    ty: info.value_ty.clone(),
                });
            }
            steps.push(BlockItem::Bind {
                pattern: Pattern::Ident(binder),
                expr: step_expr,
                span: step.span.clone(),
            });
            consumed += 1;
        }
        let subject_expr = current_expr;
        let mut fields = vec![self.record_field(
            FLOW_STATE_SUBJECT_FIELD,
            subject_expr.clone(),
            expr_span(&subject_expr),
        )];
        for binding in &bindings {
            fields.push(self.record_field(
                &binding.name.name,
                Expr::Ident(binding.name.clone()),
                binding.name.span.clone(),
            ));
        }
        steps.push(BlockItem::Expr {
            expr: Expr::Record {
                fields,
                span: expr_span(&subject_expr),
            },
            span: expr_span(&subject_expr),
        });
        let expr = Expr::Block {
            kind: BlockKind::Do {
                monad: SpannedName {
                    name: "Applicative".to_string(),
                    span: expr_span(&subject_expr),
                },
            },
            items: steps,
            span: expr_span(&subject_expr),
        };
        let kind = self.classify_flow_expr(&expr, env)?.kind;
        Ok((expr, kind, bindings, consumed))
    }

    fn build_effect_sibling_group_expr(
        &mut self,
        current_expr: Expr,
        _current_ty: Type,
        env: &TypeEnv,
        lines: &[FlowLine],
        ctx: &mut FlowDesugarCtx,
        kind: FlowStepKind,
    ) -> Result<(Expr, FlowCarrierInfo, Vec<FlowBindingInfo>, usize), TypeError> {
        let mut sibling_exprs = Vec::new();
        let mut tuple_patterns = Vec::new();
        let mut bindings = Vec::new();
        let mut concurrency_limit: Option<usize> = None;
        let mut consumed = 0;
        while let Some(FlowLine::Step(step)) = lines.get(consumed) {
            if step.kind != kind {
                break;
            }
            for modifier in &step.modifiers {
                if matches!(modifier, FlowModifier::Concurrent { .. }) && consumed > 0 {
                    return Err(TypeError {
                        span: step.span.clone(),
                        message: "`@concurrent` is only allowed on the first line of a contiguous sibling block".to_string(),
                        expected: None,
                        found: None,
                    });
                }
            }
            if consumed == 0 {
                concurrency_limit = self.flow_concurrency_limit(&step.modifiers)?;
            }
            let base = self.build_step_value_expr(current_expr.clone(), &step.expr, env)?;
            let raw_effect_expr =
                self.apply_line_modifiers(base, &step.modifiers, env, step.span.clone())?;
            let effect_expr = self.reify_effect_candidate_expr(raw_effect_expr, step.span.clone());
            let effect_ty = self.infer_expr_ephemeral(&effect_expr, env)?;
            let FlowExprKind::Carrier(effect_info) = self.classify_flow_type(effect_ty) else {
                return Err(TypeError {
                    span: step.span.clone(),
                    message: format!(
                        "`{}` sibling lines must yield an effect-like carrier",
                        self.flow_step_text(kind)
                    ),
                    expected: None,
                    found: None,
                });
            };
            let binder = step
                .binding
                .as_ref()
                .map(|binding| binding.name.clone())
                .unwrap_or_else(|| ctx.fresh_ident("sibling", &step.span));
            if step.binding.is_some() {
                let info = FlowBindingInfo {
                    name: binder.clone(),
                    ty: effect_info.value_ty.clone(),
                };
                bindings.push(info);
            }
            tuple_patterns.push(Pattern::Ident(binder));
            sibling_exprs.push(effect_expr);
            consumed += 1;
        }
        if sibling_exprs.is_empty() {
            return Err(TypeError {
                span: expr_span(&current_expr),
                message: format!(
                    "expected one or more `{}` sibling lines",
                    self.flow_step_text(kind)
                ),
                expected: None,
                found: None,
            });
        }
        let subject_expr = current_expr;
        let mut fields = vec![self.record_field(
            FLOW_STATE_SUBJECT_FIELD,
            subject_expr.clone(),
            expr_span(&subject_expr),
        )];
        for binding in &bindings {
            fields.push(self.record_field(
                &binding.name.name,
                Expr::Ident(binding.name.clone()),
                binding.name.span.clone(),
            ));
        }
        let result_record = Expr::Record {
            fields,
            span: expr_span(&subject_expr),
        };
        let body = if concurrency_limit == Some(1) || sibling_exprs.len() == 1 {
            let mut items = Vec::new();
            for (pattern, expr) in tuple_patterns.into_iter().zip(sibling_exprs) {
                items.push(BlockItem::Bind {
                    pattern,
                    expr,
                    span: expr_span(&result_record),
                });
            }
            items.push(BlockItem::Expr {
                expr: self.pure_call(result_record, expr_span(&subject_expr)),
                span: expr_span(&subject_expr),
            });
            self.effect_block(items, expr_span(&subject_expr))
        } else {
            if let Some(limit) = concurrency_limit {
                if limit < sibling_exprs.len() {
                    return Err(TypeError {
                        span: expr_span(&subject_expr),
                        message: format!(
                            "bounded `@concurrent {limit}` for `{}` blocks is not implemented yet; use `@concurrent 1` or a limit at least as large as the block",
                            self.flow_step_text(kind)
                        ),
                        expected: None,
                        found: None,
                    });
                }
            }
            let par_expr = self.nested_par_expr(sibling_exprs, expr_span(&subject_expr));
            self.effect_block(
                vec![
                    BlockItem::Bind {
                        pattern: self
                            .nested_tuple_pattern(tuple_patterns, expr_span(&subject_expr)),
                        expr: par_expr,
                        span: expr_span(&subject_expr),
                    },
                    BlockItem::Expr {
                        expr: self.pure_call(result_record, expr_span(&subject_expr)),
                        span: expr_span(&subject_expr),
                    },
                ],
                expr_span(&subject_expr),
            )
        };
        let body_ty = self.infer_expr_ephemeral(&body, env)?;
        let FlowExprKind::Carrier(info) = self.classify_flow_type(body_ty) else {
            unreachable!("effect sibling block must be effect-like");
        };
        Ok((body, info, bindings, consumed))
    }

    fn build_fanout_expr(
        &mut self,
        current_expr: Expr,
        _current_ty: Type,
        step: &FlowStep,
        env: &TypeEnv,
        ctx: &mut FlowDesugarCtx,
    ) -> Result<Expr, TypeError> {
        if step
            .modifiers
            .iter()
            .any(|modifier| !matches!(modifier, FlowModifier::Concurrent { .. }))
        {
            return Err(TypeError {
                span: step.span.clone(),
                message: "`*|>` currently supports only `@concurrent` as a line modifier"
                    .to_string(),
                expected: None,
                found: None,
            });
        }
        let source_expr = self.build_step_value_expr(current_expr, &step.expr, env)?;
        let source_ty = self.infer_expr_ephemeral(&source_expr, env)?;
        let item_ty = self.with_ephemeral_state_rollback(|checker| {
            checker.generate_source_elem(source_ty.clone(), step.span.clone())
        })?;
        let item = ctx.fresh_ident("item", &step.span);
        let mut item_env = env.clone();
        item_env.insert(item.name.clone(), Scheme::mono(item_ty.clone()));
        let silent_guard = self.flow_subflow_has_silent_guard(&step.subflow);
        if silent_guard {
            let mut items = vec![BlockItem::Bind {
                pattern: Pattern::Ident(item.clone()),
                expr: source_expr,
                span: step.span.clone(),
            }];
            items.extend(self.build_fanout_generate_items(
                Expr::Ident(item),
                &item_env,
                &step.subflow,
                ctx,
            )?);
            return Ok(self.named_call_expr(
                "aivi.generator.toList",
                vec![Expr::Block {
                    kind: BlockKind::Generate,
                    items,
                    span: step.span.clone(),
                }],
                step.span.clone(),
            ));
        }
        let body = self.build_pure_flow(
            Expr::Ident(item.clone()),
            item_ty.clone(),
            &item_env,
            &step.subflow,
            ctx,
        )?;
        match body.kind {
            FlowExprKind::Pure(_) => {
                let lambda = Expr::Lambda {
                    params: vec![Pattern::Ident(item.clone())],
                    body: Box::new(body.expr),
                    span: step.span.clone(),
                };
                if self.is_generator_type(&source_ty) {
                    Ok(self.named_call_expr(
                        "aivi.generator.toList",
                        vec![self.named_call_expr(
                            "aivi.generator.map",
                            vec![lambda, source_expr],
                            step.span.clone(),
                        )],
                        step.span.clone(),
                    ))
                } else {
                    Ok(self.named_call_expr("map", vec![lambda, source_expr], step.span.clone()))
                }
            }
            FlowExprKind::Carrier(carrier) if carrier.effect_like => {
                if silent_guard {
                    return Err(TypeError {
                        span: step.span.clone(),
                        message: "silent `>|>` item-skips inside effectful `*|>` fan-out bodies are not supported yet".to_string(),
                        expected: None,
                        found: None,
                    });
                }
                if self
                    .flow_concurrency_limit(&step.modifiers)?
                    .is_some_and(|limit| limit > 1)
                {
                    return Err(TypeError {
                        span: step.span.clone(),
                        message: "parallel `@concurrent` fan-out is not implemented yet; use `@concurrent 1` or omit it".to_string(),
                        expected: None,
                        found: None,
                    });
                }
                let lambda = Expr::Lambda {
                    params: vec![Pattern::Ident(item)],
                    body: Box::new(self.ensure_effect_expr(
                        body.expr,
                        &carrier,
                        step.span.clone(),
                    )?),
                    span: step.span.clone(),
                };
                let list_source =
                    self.ensure_list_source_expr(source_expr, &source_ty, step.span.clone())?;
                Ok(self.named_call_expr("traverse", vec![lambda, list_source], step.span.clone()))
            }
            FlowExprKind::Carrier(_) => Err(TypeError {
                span: step.span.clone(),
                message:
                    "`*|>` currently supports only pure or effect-like per-item fan-out bodies"
                        .to_string(),
                expected: None,
                found: None,
            }),
        }
    }

    fn build_fanout_generate_items(
        &mut self,
        current_expr: Expr,
        env: &TypeEnv,
        lines: &[FlowLine],
        ctx: &mut FlowDesugarCtx,
    ) -> Result<Vec<BlockItem>, TypeError> {
        if lines.is_empty() {
            let yield_span = expr_span(&current_expr);
            return Ok(vec![BlockItem::Yield {
                expr: current_expr,
                span: yield_span,
            }]);
        }
        match &lines[0] {
            FlowLine::Anchor(_) => {
                self.build_fanout_generate_items(current_expr, env, &lines[1..], ctx)
            }
            FlowLine::Guard(guard) if guard.fail_expr.is_none() => {
                let mut items = vec![BlockItem::Filter {
                    expr: self.build_guard_predicate_expr(
                        current_expr.clone(),
                        &guard.predicate,
                        env,
                    )?,
                    span: guard.span.clone(),
                }];
                items.extend(self.build_fanout_generate_items(
                    current_expr,
                    env,
                    &lines[1..],
                    ctx,
                )?);
                Ok(items)
            }
            FlowLine::Step(step) if step.kind == FlowStepKind::Flow => {
                if !step.modifiers.is_empty() {
                    return Err(TypeError {
                        span: step.span.clone(),
                        message: "line modifiers are not supported inside guarded pure `*|>` fan-out bodies".to_string(),
                        expected: None,
                        found: None,
                    });
                }
                let step_expr = self.build_step_value_expr(current_expr, &step.expr, env)?;
                let step_ty = self.infer_expr_ephemeral(&step_expr, env)?;
                let FlowExprKind::Pure(step_ty) = self.classify_flow_type(step_ty.clone()) else {
                    return Err(TypeError {
                        span: step.span.clone(),
                        message: "guarded pure `*|>` fan-out bodies can only use pure `|>` lines"
                            .to_string(),
                        expected: None,
                        found: None,
                    });
                };
                let binder = step
                    .binding
                    .as_ref()
                    .map(|binding| binding.name.clone())
                    .unwrap_or_else(|| ctx.fresh_ident("item", &step.span));
                let mut next_env = env.clone();
                next_env.insert(binder.name.clone(), Scheme::mono(step_ty.clone()));
                let mut items = vec![BlockItem::Let {
                    pattern: Pattern::Ident(binder.clone()),
                    expr: step_expr,
                    span: step.span.clone(),
                }];
                items.extend(self.build_fanout_generate_items(
                    Expr::Ident(binder),
                    &next_env,
                    &lines[1..],
                    ctx,
                )?);
                Ok(items)
            }
            other => Err(TypeError {
                span: other.span_clone(),
                message: "this guarded `*|>` fan-out body shape is not supported yet".to_string(),
                expected: None,
                found: None,
            }),
        }
    }

    fn apply_line_modifiers(
        &mut self,
        expr: Expr,
        modifiers: &[FlowModifier],
        env: &TypeEnv,
        span: Span,
    ) -> Result<Expr, TypeError> {
        let has_cleanup = modifiers
            .iter()
            .any(|modifier| matches!(modifier, FlowModifier::Cleanup { .. }));
        let mut base = expr;
        if modifiers
            .iter()
            .any(|modifier| matches!(modifier, FlowModifier::Concurrent { .. }))
        {
            return Err(TypeError {
                span,
                message: "`@concurrent` is only valid on `*|>`, or on the first line of a contiguous `&|>` block".to_string(),
                expected: None,
                found: None,
            });
        }
        let expr_ty = self.infer_expr_ephemeral(&base, env)?;
        let kind = self.classify_flow_type(expr_ty.clone());
        let carrier = match &kind {
            FlowExprKind::Carrier(carrier) => Some(carrier.clone()),
            FlowExprKind::Pure(_) => None,
        };
        let needs_effect = modifiers.iter().any(|modifier| {
            matches!(
                modifier,
                FlowModifier::Timeout { .. }
                    | FlowModifier::Delay { .. }
                    | FlowModifier::Retry { .. }
            )
        });
        if needs_effect {
            let Some(carrier) = carrier.clone() else {
                return Err(TypeError {
                    span,
                    message: "`@timeout`, `@delay`, and `@retry` require an effect-like line"
                        .to_string(),
                    expected: None,
                    found: None,
                });
            };
            if !carrier.effect_like {
                return Err(TypeError {
                    span,
                    message: "`@timeout`, `@delay`, and `@retry` currently support only effect-like lines".to_string(),
                    expected: None,
                    found: None,
                });
            }
            base = self.ensure_effect_expr(base, &carrier, span.clone())?;
            if !self.effect_error_accepts_text(&base, env)? {
                return Err(TypeError {
                    span,
                    message: "flow `@timeout`, `@delay`, and `@retry` currently require an effect error type compatible with Text".to_string(),
                    expected: None,
                    found: None,
                });
            }
            if let Some(modifier) = modifiers.iter().find_map(|modifier| match modifier {
                FlowModifier::Timeout { duration, .. } => Some(duration.clone()),
                _ => None,
            }) {
                let millis = self.duration_to_millis_expr(modifier, env)?;
                base = self.call_expr(
                    self.concurrent_builtin_field("timeoutWith", span.clone()),
                    vec![millis, self.string_lit("timeout", span.clone()), base],
                    span.clone(),
                );
            }
            if let Some(retry) = modifiers.iter().find_map(|modifier| match modifier {
                FlowModifier::Retry {
                    attempts,
                    interval,
                    exponential,
                    ..
                } => Some((*attempts, interval.clone(), *exponential)),
                _ => None,
            }) {
                base = self.build_retry_expr(base, retry.0, retry.1, retry.2, env, span.clone())?;
            }
            if let Some(delay) = modifiers.iter().find_map(|modifier| match modifier {
                FlowModifier::Delay { duration, .. } => Some(duration.clone()),
                _ => None,
            }) {
                let delay_millis = self.duration_to_millis_expr(delay, env)?;
                let sleep = self.call_expr(
                    self.concurrent_builtin_field("sleep", span.clone()),
                    vec![delay_millis],
                    span.clone(),
                );
                base = self.effect_block(
                    vec![
                        BlockItem::Bind {
                            pattern: Pattern::Wildcard(span.clone()),
                            expr: sleep,
                            span: span.clone(),
                        },
                        BlockItem::Expr {
                            expr: base,
                            span: span.clone(),
                        },
                    ],
                    span.clone(),
                );
            }
        }
        if has_cleanup {
            let mut cleanup_items = Vec::new();
            let resource_name = self.env_bound_name_for_cleanup(&base, &span);
            cleanup_items.push(BlockItem::Bind {
                pattern: Pattern::Ident(resource_name.clone()),
                expr: base,
                span: span.clone(),
            });
            cleanup_items.push(BlockItem::Yield {
                expr: Expr::Ident(resource_name.clone()),
                span: span.clone(),
            });
            for modifier in modifiers {
                if let FlowModifier::Cleanup { expr, .. } = modifier {
                    let cleanup_expr =
                        self.build_step_value_expr(Expr::Ident(resource_name.clone()), expr, env)?;
                    cleanup_items.push(BlockItem::Expr {
                        expr: cleanup_expr,
                        span: span.clone(),
                    });
                }
            }
            base = Expr::Block {
                kind: BlockKind::Resource,
                items: cleanup_items,
                span: span.clone(),
            };
        }
        Ok(base)
    }

    fn build_retry_expr(
        &mut self,
        effect_expr: Expr,
        attempts: u32,
        interval: Expr,
        exponential: bool,
        env: &TypeEnv,
        span: Span,
    ) -> Result<Expr, TypeError> {
        if attempts == 0 {
            return Err(TypeError {
                span,
                message: "`@retry` expects a positive attempt count".to_string(),
                expected: None,
                found: None,
            });
        }
        let interval_millis = self.duration_to_millis_expr(interval, env)?;
        self.build_retry_expr_inner(effect_expr, attempts, interval_millis, exponential, 1, span)
    }

    fn build_retry_expr_inner(
        &mut self,
        effect_expr: Expr,
        attempts: u32,
        interval_millis: Expr,
        exponential: bool,
        multiplier: i64,
        span: Span,
    ) -> Result<Expr, TypeError> {
        if attempts <= 1 {
            return Ok(effect_expr);
        }
        let result_name = SpannedName {
            name: format!("__flow_retry_result_{attempts}"),
            span: span.clone(),
        };
        let value_name = SpannedName {
            name: format!("__flow_retry_ok_{attempts}"),
            span: span.clone(),
        };
        let err_name = SpannedName {
            name: format!("__flow_retry_err_{attempts}"),
            span: span.clone(),
        };
        let sleep_duration = if multiplier == 1 {
            interval_millis.clone()
        } else {
            Expr::Binary {
                op: "*".to_string(),
                left: Box::new(interval_millis.clone()),
                right: Box::new(self.int_lit(multiplier, span.clone())),
                span: span.clone(),
            }
        };
        let sleep_expr = self.call_expr(
            self.concurrent_builtin_field("sleep", span.clone()),
            vec![sleep_duration],
            span.clone(),
        );
        let next_multiplier = if exponential {
            multiplier * 2
        } else {
            multiplier
        };
        let retry_rest = self.build_retry_expr_inner(
            effect_expr.clone(),
            attempts - 1,
            interval_millis,
            exponential,
            next_multiplier,
            span.clone(),
        )?;
        let block_span = result_name.span.clone();
        Ok(self.effect_block(
            vec![
                BlockItem::Bind {
                    pattern: Pattern::Ident(result_name.clone()),
                    expr: self.named_call_expr("attempt", vec![effect_expr], span.clone()),
                    span: span.clone(),
                },
                BlockItem::Expr {
                    expr: Expr::Match {
                        scrutinee: Some(Box::new(Expr::Ident(result_name.clone()))),
                        arms: vec![
                            MatchArm {
                                pattern: Pattern::Constructor {
                                    name: SpannedName {
                                        name: "Ok".to_string(),
                                        span: span.clone(),
                                    },
                                    args: vec![Pattern::Ident(value_name.clone())],
                                    span: span.clone(),
                                },
                                guard: None,
                                guard_negated: false,
                                body: self.pure_call(Expr::Ident(value_name), span.clone()),
                                span: span.clone(),
                            },
                            MatchArm {
                                pattern: Pattern::Constructor {
                                    name: SpannedName {
                                        name: "Err".to_string(),
                                        span: span.clone(),
                                    },
                                    args: vec![Pattern::Ident(err_name.clone())],
                                    span: span.clone(),
                                },
                                guard: None,
                                guard_negated: false,
                                body: self.effect_block(
                                    vec![
                                        BlockItem::Bind {
                                            pattern: Pattern::Wildcard(span.clone()),
                                            expr: sleep_expr,
                                            span: span.clone(),
                                        },
                                        BlockItem::Expr {
                                            expr: retry_rest,
                                            span: span.clone(),
                                        },
                                    ],
                                    span.clone(),
                                ),
                                span: span.clone(),
                            },
                        ],
                        span: span.clone(),
                    },
                    span: span.clone(),
                },
            ],
            block_span,
        ))
    }

    fn ensure_effect_expr(
        &mut self,
        expr: Expr,
        info: &FlowCarrierInfo,
        span: Span,
    ) -> Result<Expr, TypeError> {
        if self.is_explicit_effect_type(&info.full_ty) {
            return Ok(expr);
        }
        let binder = SpannedName {
            name: "__flow_effect_value".to_string(),
            span: span.clone(),
        };
        Ok(self.effect_block(
            vec![
                BlockItem::Bind {
                    pattern: Pattern::Ident(binder.clone()),
                    expr,
                    span: span.clone(),
                },
                BlockItem::Expr {
                    expr: self.pure_call(Expr::Ident(binder), span.clone()),
                    span: span.clone(),
                },
            ],
            span,
        ))
    }

    fn reify_effect_candidate_expr(&self, expr: Expr, span: Span) -> Expr {
        let binder = SpannedName {
            name: "__flow_effect_value".to_string(),
            span: span.clone(),
        };
        self.effect_block(
            vec![
                BlockItem::Bind {
                    pattern: Pattern::Ident(binder.clone()),
                    expr,
                    span: span.clone(),
                },
                BlockItem::Expr {
                    expr: self.pure_call(Expr::Ident(binder), span.clone()),
                    span: span.clone(),
                },
            ],
            span,
        )
    }

    fn effect_error_accepts_text(&mut self, expr: &Expr, env: &TypeEnv) -> Result<bool, TypeError> {
        let ty = self.infer_expr_ephemeral(expr, env)?;
        Ok(self.with_ephemeral_state_rollback(|checker| {
            let err_ty = checker.extract_effect_error_type(ty);
            err_ty.is_none_or(|err_ty| {
                checker
                    .unify(err_ty, Type::con("Text"), expr_span(expr))
                    .is_ok()
            })
        }))
    }

    fn duration_to_millis_expr(&mut self, expr: Expr, env: &TypeEnv) -> Result<Expr, TypeError> {
        let ty = self.infer_expr_ephemeral(&expr, env)?;
        let is_int = self.with_ephemeral_state_rollback(|checker| {
            checker
                .unify(ty.clone(), Type::con("Int"), expr_span(&expr))
                .is_ok()
        });
        if is_int {
            return Ok(expr);
        }
        let span = expr_span(&expr);
        Ok(self.field_expr(
            Expr::Binary {
                op: "+".to_string(),
                left: Box::new(self.zero_span_expr(span.clone())),
                right: Box::new(expr),
                span: span.clone(),
            },
            "millis",
            span,
        ))
    }

    fn env_with_pattern_binders(&mut self, env: &TypeEnv, pattern: &Pattern) -> TypeEnv {
        let mut out = env.clone();
        let mut binders = Vec::new();
        Self::collect_applicative_pattern_binders(pattern, &mut binders);
        for binder in binders {
            out.insert(binder, Scheme::mono(self.fresh_var()));
        }
        out
    }

    fn with_ephemeral_pattern_env<T>(
        &mut self,
        env: &TypeEnv,
        patterns: &[Pattern],
        f: impl FnOnce(&mut Self, &TypeEnv) -> T,
    ) -> T {
        self.with_ephemeral_state_rollback(|checker| {
            let mut scoped_env = env.clone();
            for pattern in patterns {
                scoped_env = checker.env_with_pattern_binders(&scoped_env, pattern);
            }
            f(checker, &scoped_env)
        })
    }

    fn prepare_inline_arm_body(
        &mut self,
        body: Expr,
        env: &TypeEnv,
        subject_expr: Expr,
    ) -> Result<Expr, TypeError> {
        let body = self.desugar_nested_flow_expr(body, env)?;
        if matches!(body, Expr::FieldSection { .. }) || expr_contains_placeholder(&body) {
            return Ok(self.call_expr(
                self.rewrite_placeholder_lambda(&body, "__flowArm"),
                vec![subject_expr],
                expr_span(&body),
            ));
        }
        if let Some(lifted) =
            lift_implicit_field_expr(&body, env, &self.method_to_classes, "__flowArm")
        {
            return Ok(self.call_expr(lifted, vec![subject_expr], expr_span(&body)));
        }
        Ok(body)
    }

    fn desugar_nested_path_segment(
        &mut self,
        segment: PathSegment,
        env: &TypeEnv,
    ) -> Result<PathSegment, TypeError> {
        match segment {
            PathSegment::Field(name) => Ok(PathSegment::Field(name)),
            PathSegment::Index(expr, span) => Ok(PathSegment::Index(
                self.desugar_nested_flow_expr(expr, env)?,
                span,
            )),
            PathSegment::All(span) => Ok(PathSegment::All(span)),
        }
    }

    fn desugar_nested_flow_expr(&mut self, expr: Expr, env: &TypeEnv) -> Result<Expr, TypeError> {
        match expr {
            Expr::Flow { .. } => self.desugar_flow_expr(expr, None, env),
            Expr::UnaryNeg { expr, span } => Ok(Expr::UnaryNeg {
                expr: Box::new(self.desugar_nested_flow_expr(*expr, env)?),
                span,
            }),
            Expr::Suffixed { base, suffix, span } => Ok(Expr::Suffixed {
                base: Box::new(self.desugar_nested_flow_expr(*base, env)?),
                suffix,
                span,
            }),
            Expr::TextInterpolate { parts, span } => Ok(Expr::TextInterpolate {
                parts: parts
                    .into_iter()
                    .map(|part| match part {
                        TextPart::Text { .. } => Ok(part),
                        TextPart::Expr { expr, span } => Ok(TextPart::Expr {
                            expr: Box::new(self.desugar_nested_flow_expr(*expr, env)?),
                            span,
                        }),
                    })
                    .collect::<Result<Vec<_>, TypeError>>()?,
                span,
            }),
            Expr::List { items, span } => Ok(Expr::List {
                items: items
                    .into_iter()
                    .map(|item| {
                        Ok(ListItem {
                            expr: self.desugar_nested_flow_expr(item.expr, env)?,
                            spread: item.spread,
                            span: item.span,
                        })
                    })
                    .collect::<Result<Vec<_>, TypeError>>()?,
                span,
            }),
            Expr::Tuple { items, span } => Ok(Expr::Tuple {
                items: items
                    .into_iter()
                    .map(|item| self.desugar_nested_flow_expr(item, env))
                    .collect::<Result<Vec<_>, _>>()?,
                span,
            }),
            Expr::Record { fields, span } => Ok(Expr::Record {
                fields: fields
                    .into_iter()
                    .map(|field| {
                        Ok(RecordField {
                            spread: field.spread,
                            path: field
                                .path
                                .into_iter()
                                .map(|segment| self.desugar_nested_path_segment(segment, env))
                                .collect::<Result<Vec<_>, _>>()?,
                            value: self.desugar_nested_flow_expr(field.value, env)?,
                            span: field.span,
                        })
                    })
                    .collect::<Result<Vec<_>, TypeError>>()?,
                span,
            }),
            Expr::PatchLit { fields, span } => Ok(Expr::PatchLit {
                fields: fields
                    .into_iter()
                    .map(|field| {
                        Ok(RecordField {
                            spread: field.spread,
                            path: field
                                .path
                                .into_iter()
                                .map(|segment| self.desugar_nested_path_segment(segment, env))
                                .collect::<Result<Vec<_>, _>>()?,
                            value: self.desugar_nested_flow_expr(field.value, env)?,
                            span: field.span,
                        })
                    })
                    .collect::<Result<Vec<_>, TypeError>>()?,
                span,
            }),
            Expr::FieldAccess { base, field, span } => Ok(Expr::FieldAccess {
                base: Box::new(self.desugar_nested_flow_expr(*base, env)?),
                field,
                span,
            }),
            other @ Expr::FieldSection { .. } => Ok(other),
            Expr::Index { base, index, span } => Ok(Expr::Index {
                base: Box::new(self.desugar_nested_flow_expr(*base, env)?),
                index: Box::new(self.desugar_nested_flow_expr(*index, env)?),
                span,
            }),
            Expr::Call { func, args, span } => Ok(Expr::Call {
                func: Box::new(self.desugar_nested_flow_expr(*func, env)?),
                args: args
                    .into_iter()
                    .map(|arg| self.desugar_nested_flow_expr(arg, env))
                    .collect::<Result<Vec<_>, _>>()?,
                span,
            }),
            Expr::Lambda { params, body, span } => {
                let body_patterns = params.clone();
                let body = self.with_ephemeral_pattern_env(
                    env,
                    &body_patterns,
                    move |checker, body_env| checker.desugar_nested_flow_expr(*body, body_env),
                )?;
                Ok(Expr::Lambda {
                    params,
                    body: Box::new(body),
                    span,
                })
            }
            Expr::Match {
                scrutinee,
                arms,
                span,
            } => Ok(Expr::Match {
                scrutinee: scrutinee
                    .map(|expr| self.desugar_nested_flow_expr(*expr, env).map(Box::new))
                    .transpose()?,
                arms: arms
                    .into_iter()
                    .map(|mut arm| {
                        let arm_patterns = vec![arm.pattern.clone()];
                        self.with_ephemeral_pattern_env(env, &arm_patterns, move |checker, arm_env| {
                            arm.guard = arm
                                .guard
                                .map(|guard| checker.desugar_nested_flow_expr(guard, arm_env))
                                .transpose()?;
                            arm.body = checker.desugar_nested_flow_expr(arm.body, arm_env)?;
                            Ok(arm)
                        })
                    })
                    .collect::<Result<Vec<_>, TypeError>>()?,
                span,
            }),
            Expr::If {
                cond,
                then_branch,
                else_branch,
                span,
            } => Ok(Expr::If {
                cond: Box::new(self.desugar_nested_flow_expr(*cond, env)?),
                then_branch: Box::new(self.desugar_nested_flow_expr(*then_branch, env)?),
                else_branch: Box::new(self.desugar_nested_flow_expr(*else_branch, env)?),
                span,
            }),
            Expr::Binary {
                op,
                left,
                right,
                span,
            } => Ok(Expr::Binary {
                op,
                left: Box::new(self.desugar_nested_flow_expr(*left, env)?),
                right: Box::new(self.desugar_nested_flow_expr(*right, env)?),
                span,
            }),
            Expr::Block { kind, items, span } => {
                let mut visible_patterns = Vec::new();
                let mut rewritten_items = Vec::with_capacity(items.len());
                for item in items {
                    let rewritten = match item {
                        BlockItem::Bind {
                            pattern,
                            expr,
                            span,
                        } => {
                            let expr = self.with_ephemeral_pattern_env(
                                env,
                                &visible_patterns,
                                |checker, scoped_env| checker.desugar_nested_flow_expr(expr, scoped_env),
                            )?;
                            visible_patterns.push(pattern.clone());
                            BlockItem::Bind { pattern, expr, span }
                        }
                        BlockItem::Let {
                            pattern,
                            expr,
                            span,
                        } => {
                            let expr = self.with_ephemeral_pattern_env(
                                env,
                                &visible_patterns,
                                |checker, scoped_env| checker.desugar_nested_flow_expr(expr, scoped_env),
                            )?;
                            visible_patterns.push(pattern.clone());
                            BlockItem::Let { pattern, expr, span }
                        }
                        BlockItem::Filter { expr, span } => BlockItem::Filter {
                            expr: self.with_ephemeral_pattern_env(
                                env,
                                &visible_patterns,
                                |checker, scoped_env| checker.desugar_nested_flow_expr(expr, scoped_env),
                            )?,
                            span,
                        },
                        BlockItem::Yield { expr, span } => BlockItem::Yield {
                            expr: self.with_ephemeral_pattern_env(
                                env,
                                &visible_patterns,
                                |checker, scoped_env| checker.desugar_nested_flow_expr(expr, scoped_env),
                            )?,
                            span,
                        },
                        BlockItem::Recurse { expr, span } => BlockItem::Recurse {
                            expr: self.with_ephemeral_pattern_env(
                                env,
                                &visible_patterns,
                                |checker, scoped_env| checker.desugar_nested_flow_expr(expr, scoped_env),
                            )?,
                            span,
                        },
                        BlockItem::Expr { expr, span } => BlockItem::Expr {
                            expr: self.with_ephemeral_pattern_env(
                                env,
                                &visible_patterns,
                                |checker, scoped_env| checker.desugar_nested_flow_expr(expr, scoped_env),
                            )?,
                            span,
                        },
                        BlockItem::When { cond, effect, span } => BlockItem::When {
                            cond: self.with_ephemeral_pattern_env(
                                env,
                                &visible_patterns,
                                |checker, scoped_env| checker.desugar_nested_flow_expr(cond, scoped_env),
                            )?,
                            effect: self.with_ephemeral_pattern_env(
                                env,
                                &visible_patterns,
                                |checker, scoped_env| checker.desugar_nested_flow_expr(effect, scoped_env),
                            )?,
                            span,
                        },
                        BlockItem::Unless { cond, effect, span } => BlockItem::Unless {
                            cond: self.with_ephemeral_pattern_env(
                                env,
                                &visible_patterns,
                                |checker, scoped_env| checker.desugar_nested_flow_expr(cond, scoped_env),
                            )?,
                            effect: self.with_ephemeral_pattern_env(
                                env,
                                &visible_patterns,
                                |checker, scoped_env| checker.desugar_nested_flow_expr(effect, scoped_env),
                            )?,
                            span,
                        },
                        BlockItem::Given {
                            cond,
                            fail_expr,
                            span,
                        } => BlockItem::Given {
                            cond: self.with_ephemeral_pattern_env(
                                env,
                                &visible_patterns,
                                |checker, scoped_env| checker.desugar_nested_flow_expr(cond, scoped_env),
                            )?,
                            fail_expr: self.with_ephemeral_pattern_env(
                                env,
                                &visible_patterns,
                                |checker, scoped_env| checker.desugar_nested_flow_expr(fail_expr, scoped_env),
                            )?,
                            span,
                        },
                    };
                    rewritten_items.push(rewritten);
                }
                Ok(Expr::Block {
                    kind,
                    items: rewritten_items,
                    span,
                })
            }
            other @ Expr::Raw { .. } | other @ Expr::Ident(_) | other @ Expr::Literal(_) => {
                Ok(other)
            }
            Expr::Mock {
                substitutions,
                body,
                span,
            } => Ok(Expr::Mock {
                substitutions: substitutions
                    .into_iter()
                    .map(|mut substitution| {
                        substitution.value = substitution
                            .value
                            .map(|value| self.desugar_nested_flow_expr(value, env))
                            .transpose()?;
                        Ok(substitution)
                    })
                    .collect::<Result<Vec<_>, TypeError>>()?,
                body: Box::new(self.desugar_nested_flow_expr(*body, env)?),
                span,
            }),
        }
    }

    fn infer_expr_ephemeral(&mut self, expr: &Expr, env: &TypeEnv) -> Result<Type, TypeError> {
        self.with_ephemeral_state_rollback(|checker| {
            let mut local_env = env.clone();
            let ty = checker.infer_expr(expr, &mut local_env)?;
            Ok(checker.apply(ty))
        })
    }

    fn classify_flow_expr(
        &mut self,
        expr: &Expr,
        env: &TypeEnv,
    ) -> Result<FlowBuiltExpr, TypeError> {
        let ty = self.infer_expr_ephemeral(expr, env)?;
        Ok(FlowBuiltExpr {
            expr: expr.clone(),
            kind: self.classify_flow_type(ty),
        })
    }

    fn classify_flow_type(&mut self, ty: Type) -> FlowExprKind {
        if let Some(info) = self.effect_like_flow_info(ty.clone()) {
            return FlowExprKind::Carrier(info);
        }
        if let Some(info) = self.generic_flow_info(ty.clone()) {
            return FlowExprKind::Carrier(info);
        }
        FlowExprKind::Pure(ty)
    }

    fn effect_like_flow_info(&mut self, ty: Type) -> Option<FlowCarrierInfo> {
        let applied = self.apply(ty.clone());
        let expanded = self.expand_alias(applied.clone());
        if let Some(value_ty) = self.infallible_effect_value_ty(ty.clone()) {
            return Some(FlowCarrierInfo {
                full_ty: applied,
                value_ty,
                effect_like: true,
                supports_functor: false,
                supports_chain: false,
            });
        }
        if let Some((full_ty, value_ty)) = self.explicit_effect_like_parts(expanded) {
            return Some(FlowCarrierInfo {
                full_ty,
                value_ty,
                effect_like: true,
                supports_functor: false,
                supports_chain: false,
            });
        }
        None
    }

    fn generic_flow_info(&mut self, ty: Type) -> Option<FlowCarrierInfo> {
        let applied = self.apply(ty.clone());
        let expanded = self.expand_alias(applied.clone());
        let value_ty = self.last_type_arg(&expanded)?;
        let instance_ty = self.rebuild_flat_type(&expanded);
        let supports_functor = self
            .find_instance_member_body("map", &instance_ty)
            .is_some();
        if !supports_functor {
            return None;
        }
        let supports_chain = self
            .find_instance_member_body("chain", &instance_ty)
            .is_some();
        Some(FlowCarrierInfo {
            full_ty: applied,
            value_ty,
            effect_like: false,
            supports_functor,
            supports_chain,
        })
    }

    fn explicit_effect_like_parts(&mut self, ty: Type) -> Option<(Type, Type)> {
        let (name, args) = self.flatten_type_constructor(&ty)?;
        if (self.type_name_matches(&name, "Effect")
            || self.type_name_matches(&name, "Resource")
            || self.type_name_matches(&name, "Source"))
            && args.len() == 2
        {
            Some((Type::Con(name, args.clone()), args[1].clone()))
        } else {
            None
        }
    }

    fn extract_effect_error_type(&mut self, ty: Type) -> Option<Type> {
        let applied = self.apply(ty);
        let (name, args) = self.flatten_type_constructor(&applied)?;
        if args.len() != 2 {
            return None;
        }
        if self.type_name_matches(&name, "Source") {
            Some(Type::con("Text"))
        } else if self.type_name_matches(&name, "Effect")
            || self.type_name_matches(&name, "Resource")
        {
            Some(args[0].clone())
        } else {
            None
        }
    }

    fn last_type_arg(&self, ty: &Type) -> Option<Type> {
        self.flatten_type_constructor(ty)
            .and_then(|(_, args)| args.last().cloned())
    }

    fn is_explicit_effect_type(&mut self, ty: &Type) -> bool {
        let applied = self.apply(ty.clone());
        self.flatten_type_constructor(&applied)
            .is_some_and(|(name, args)| {
                self.type_name_matches(&name, "Effect") && (args.len() == 1 || args.len() == 2)
            })
    }

    fn is_generator_type(&mut self, ty: &Type) -> bool {
        let applied = self.apply(ty.clone());
        let expanded = self.expand_alias(applied);
        self.flatten_type_constructor(&expanded)
            .is_some_and(|(name, args)| name == "aivi.generator.Generator" && args.len() == 1)
    }

    fn ensure_list_source_expr(
        &mut self,
        source_expr: Expr,
        source_ty: &Type,
        span: Span,
    ) -> Result<Expr, TypeError> {
        if self.is_generator_type(source_ty) {
            Ok(self.named_call_expr("aivi.generator.toList", vec![source_expr], span))
        } else {
            Ok(source_expr)
        }
    }

    fn flow_subflow_has_silent_guard(&self, lines: &[FlowLine]) -> bool {
        lines.iter().any(|line| match line {
            FlowLine::Guard(guard) => guard.fail_expr.is_none(),
            _ => false,
        })
    }

    fn flow_concurrency_limit(
        &self,
        modifiers: &[FlowModifier],
    ) -> Result<Option<usize>, TypeError> {
        let Some(limit_expr) = modifiers.iter().find_map(|modifier| match modifier {
            FlowModifier::Concurrent { limit, .. } => Some(limit),
            _ => None,
        }) else {
            return Ok(None);
        };
        let Expr::Literal(Literal::Number { text, span }) = limit_expr else {
            return Err(TypeError {
                span: expr_span(limit_expr),
                message: "flow `@concurrent` currently requires an integer literal".to_string(),
                expected: None,
                found: None,
            });
        };
        let value = text.parse::<usize>().map_err(|_| TypeError {
            span: span.clone(),
            message: "flow `@concurrent` expects a positive integer literal".to_string(),
            expected: None,
            found: None,
        })?;
        if value == 0 {
            return Err(TypeError {
                span: span.clone(),
                message: "flow `@concurrent` expects a positive integer literal".to_string(),
                expected: None,
                found: None,
            });
        }
        Ok(Some(value))
    }

    fn flow_step_text(&self, kind: FlowStepKind) -> &'static str {
        match kind {
            FlowStepKind::Flow => "|>",
            FlowStepKind::Tap => "~|>",
            FlowStepKind::Attempt => "?|>",
            FlowStepKind::FanOut => "*|>",
            FlowStepKind::Applicative => "&|>",
        }
    }

    fn nested_par_expr(&self, mut exprs: Vec<Expr>, span: Span) -> Expr {
        if exprs.len() == 1 {
            return exprs.pop().expect("single expr");
        }
        let right = exprs.pop().expect("right expr");
        let left = self.nested_par_expr(exprs, span.clone());
        self.call_expr(
            self.concurrent_builtin_field("par", span.clone()),
            vec![left, right],
            span,
        )
    }

    fn nested_tuple_pattern(&self, mut patterns: Vec<Pattern>, span: Span) -> Pattern {
        if patterns.len() == 1 {
            return patterns.pop().expect("single pattern");
        }
        let right = patterns.pop().expect("right pattern");
        let left = self.nested_tuple_pattern(patterns, span.clone());
        Pattern::Tuple {
            items: vec![left, right],
            span,
        }
    }

    fn env_bound_name_for_cleanup(&self, _expr: &Expr, span: &Span) -> SpannedName {
        SpannedName {
            name: "__flow_resource".to_string(),
            span: span.clone(),
        }
    }

    fn effect_block(&self, items: Vec<BlockItem>, span: Span) -> Expr {
        Expr::Block {
            kind: BlockKind::Do {
                monad: SpannedName {
                    name: "Effect".to_string(),
                    span: span.clone(),
                },
            },
            items,
            span,
        }
    }

    fn pure_call(&self, expr: Expr, span: Span) -> Expr {
        self.named_call_expr("pure", vec![expr], span)
    }

    fn fail_call(&self, expr: Expr, span: Span) -> Expr {
        self.named_call_expr("fail", vec![expr], span)
    }

    fn named_call_expr(&self, name: &str, args: Vec<Expr>, span: Span) -> Expr {
        self.call_expr(
            Expr::Ident(SpannedName {
                name: name.to_string(),
                span: span.clone(),
            }),
            args,
            span,
        )
    }

    fn concurrent_builtin_field(&self, field: &str, span: Span) -> Expr {
        self.field_expr(
            Expr::Ident(SpannedName {
                name: "aivi.concurrent".to_string(),
                span: span.clone(),
            }),
            field,
            span,
        )
    }

    fn call_expr(&self, func: Expr, args: Vec<Expr>, span: Span) -> Expr {
        Expr::Call {
            func: Box::new(func),
            args,
            span,
        }
    }

    fn field_expr(&self, base: Expr, field: &str, span: Span) -> Expr {
        Expr::FieldAccess {
            base: Box::new(base),
            field: SpannedName {
                name: field.to_string(),
                span: span.clone(),
            },
            span,
        }
    }

    fn state_field_expr(&self, state: &SpannedName, field: &str) -> Expr {
        self.field_expr(Expr::Ident(state.clone()), field, state.span.clone())
    }

    fn record_field(&self, name: &str, value: Expr, span: Span) -> RecordField {
        RecordField {
            spread: false,
            path: vec![PathSegment::Field(SpannedName {
                name: name.to_string(),
                span: span.clone(),
            })],
            value,
            span,
        }
    }

    fn zero_span_expr(&self, span: Span) -> Expr {
        Expr::Record {
            fields: vec![self.record_field("millis", self.int_lit(0, span.clone()), span.clone())],
            span,
        }
    }

    fn int_lit(&self, value: i64, span: Span) -> Expr {
        Expr::Literal(Literal::Number {
            text: value.to_string(),
            span,
        })
    }

    fn string_lit(&self, value: &str, span: Span) -> Expr {
        Expr::Literal(Literal::String {
            text: value.to_string(),
            span,
        })
    }

    fn flatten_type_constructor(&self, ty: &Type) -> Option<(String, Vec<Type>)> {
        match ty {
            Type::Con(name, args) => Some((name.clone(), args.clone())),
            Type::App(base, args) => {
                let (name, mut all_args) = self.flatten_type_constructor(base)?;
                all_args.extend(args.clone());
                Some((name, all_args))
            }
            _ => None,
        }
    }

    fn rebuild_flat_type(&self, ty: &Type) -> Type {
        self.flatten_type_constructor(ty)
            .map(|(name, args)| Type::Con(name, args))
            .unwrap_or_else(|| ty.clone())
    }
}

trait FlowLineExt {
    fn span_clone(&self) -> Span;
    fn branch_span(&self) -> Span;
}

impl FlowLineExt for FlowLine {
    fn span_clone(&self) -> Span {
        match self {
            FlowLine::Step(step) => step.span.clone(),
            FlowLine::Guard(guard) => guard.span.clone(),
            FlowLine::Branch(arm) | FlowLine::Recover(arm) => arm.span.clone(),
            FlowLine::Anchor(anchor) => anchor.span.clone(),
        }
    }

    fn branch_span(&self) -> Span {
        self.span_clone()
    }
}

impl Expr {
    fn default_unit(span: Span) -> Expr {
        Expr::Ident(SpannedName {
            name: "Unit".to_string(),
            span,
        })
    }
}
