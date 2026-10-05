//! Closed callable adaptation at the runtime-aware backend boundary.
//!
//! A concrete callable retains its declaration ABI even when its use contract
//! is generic. Helpers explicitly repack arguments/results through ordinary
//! direct calls. The original expression remains the interpreter's value.

use std::collections::{BTreeSet, HashMap};

use aivi_core::{Arena, ArenaOverflow};

use crate::{
    AbiParameter, AbiResult, CallingConvention, CallingConventionKind, EnvSlotId, Item, ItemId,
    ItemKind, Kernel, KernelExpr, KernelExprId, KernelExprKind, KernelId, KernelOrigin,
    KernelOriginKind, KernelSignature, Layout, LayoutId, LayoutKind, LoweringError, ParameterRole,
    Program,
    native_abi::{LayoutCompatibility, layouts_compatible, runtime_payload_layout},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum ClosedReference {
    Item(ItemId),
    Evidence(ItemId),
}

impl ClosedReference {
    fn item(self) -> ItemId {
        match self {
            Self::Item(item) | Self::Evidence(item) => item,
        }
    }

    fn kind(self) -> KernelExprKind {
        match self {
            Self::Item(item) => KernelExprKind::Item(item),
            Self::Evidence(item) => KernelExprKind::ExecutableEvidence(item),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ClosedCallable {
    pub target: ClosedReference,
    pub arguments: Vec<ClosedReference>,
}

/// Resolve only closed references and their static partial applications. An
/// explicit visited set rejects malformed expression cycles without recursion.
pub(crate) fn closed_callable(kernel: &Kernel, root: KernelExprId) -> Option<ClosedCallable> {
    fn reference(kind: &KernelExprKind) -> Option<ClosedReference> {
        match kind {
            KernelExprKind::Item(item) => Some(ClosedReference::Item(*item)),
            KernelExprKind::ExecutableEvidence(item) => Some(ClosedReference::Evidence(*item)),
            _ => None,
        }
    }
    let mut cursor = root;
    let mut visited = BTreeSet::new();
    let mut prefixes = Vec::new();
    loop {
        if !visited.insert(cursor) {
            return None;
        }
        let expr = kernel.exprs().get(cursor)?;
        if let Some(target) = reference(&expr.kind) {
            let mut arguments = Vec::new();
            for prefix in prefixes.into_iter().rev() {
                arguments.extend(prefix);
            }
            return Some(ClosedCallable { target, arguments });
        }
        match &expr.kind {
            KernelExprKind::CallableAdapter { value, .. } => cursor = *value,
            KernelExprKind::Apply { callee, arguments } => {
                let prefix = arguments
                    .iter()
                    .map(|argument| reference(&kernel.exprs().get(*argument)?.kind))
                    .collect::<Option<Vec<_>>>()?;
                prefixes.push(prefix);
                cursor = *callee;
            }
            _ => return None,
        }
    }
}

pub(crate) fn callable_signature(
    program: &Program,
    layout: LayoutId,
) -> Option<(Vec<LayoutId>, LayoutId)> {
    let mut current = layout;
    let mut parameters = Vec::new();
    let mut visited = BTreeSet::new();
    loop {
        current = runtime_payload_layout(program, current)?;
        if !visited.insert(current) {
            return None;
        }
        match program.layouts().get(current)?.kind {
            LayoutKind::Arrow { parameter, result } => {
                parameters.push(parameter);
                current = result;
            }
            _ => return Some((parameters, current)),
        }
    }
}

/// Only a forwarding intrinsic wrapper qualifies for native specialization.
pub(crate) fn builtin_wrapper_intrinsic(
    kernel: &Kernel,
) -> Option<crate::BuiltinClassMemberIntrinsic> {
    match &kernel.exprs()[kernel.root].kind {
        KernelExprKind::BuiltinClassMember(intrinsic) => Some(intrinsic.clone()),
        KernelExprKind::Apply { callee, arguments } => {
            let KernelExprKind::BuiltinClassMember(intrinsic) = &kernel.exprs()[*callee].kind
            else {
                return None;
            };
            let forwards = arguments.len() == kernel.environment.len()
                && arguments.iter().enumerate().all(|(index, expr)| {
                    matches!(kernel.exprs()[*expr].kind, KernelExprKind::Environment(slot) if slot.index() == index)
                });
            forwards.then_some(intrinsic.clone())
        }
        _ => None,
    }
}

fn overflow(family: &'static str, error: ArenaOverflow) -> LoweringError {
    LoweringError::ArenaOverflow {
        family,
        attempted_len: error.attempted_len(),
    }
}

struct AdapterBuilder<'a> {
    program: &'a mut Program,
    arrows: HashMap<(LayoutId, LayoutId), LayoutId>,
    helpers: HashMap<(ClosedCallable, LayoutId, LayoutId), ItemId>,
}

impl AdapterBuilder<'_> {
    fn declaration_layout(&mut self, item: ItemId) -> Result<Option<LayoutId>, LoweringError> {
        let Some(decl) = self.program.items().get(item) else {
            return Ok(None);
        };
        let Some(body) = decl.body.and_then(|body| self.program.kernels().get(body)) else {
            return Ok(None);
        };
        let mut layout = body.result_layout;
        let parameters = decl.parameters.clone();
        for parameter in parameters.into_iter().rev() {
            layout = if let Some(layout) = self.arrows.get(&(parameter, layout)) {
                *layout
            } else {
                let arrow = self
                    .program
                    .layouts_mut()
                    .alloc(Layout::new(LayoutKind::Arrow {
                        parameter,
                        result: layout,
                    }))
                    .map_err(|error| overflow("callable layouts", error))?;
                self.arrows.insert((parameter, layout), arrow);
                arrow
            };
        }
        Ok(Some(layout))
    }

    fn adapt(
        &mut self,
        kernel_id: KernelId,
        value: KernelExprId,
        expected: LayoutId,
    ) -> Result<KernelExprId, LoweringError> {
        if runtime_payload_layout(self.program, expected).is_none_or(|layout| {
            !matches!(
                self.program.layouts()[layout].kind,
                LayoutKind::Arrow { .. }
            )
        }) {
            return Ok(value);
        }
        let Some(source) = closed_callable(&self.program.kernels()[kernel_id], value) else {
            return Ok(value);
        };
        let Some(actual) = self.declaration_layout(source.target.item())? else {
            return Ok(value);
        };
        let Some((actual_parameters, _)) = callable_signature(self.program, actual) else {
            return Ok(value);
        };
        if source.arguments.len() >= actual_parameters.len() {
            return Ok(value);
        }
        let original = self.program.kernels()[kernel_id].exprs()[value].clone();
        if source.arguments.is_empty()
            && layouts_compatible(
                self.program,
                expected,
                actual,
                LayoutCompatibility::CallableRepresentation,
            )
        {
            if original.layout == actual {
                return Ok(value);
            }
            // A generic function's use-site type can be concrete while its
            // descriptor still has the declaration ABI. Normalize that view.
            return self
                .program
                .kernels_mut()
                .get_mut(kernel_id)
                .expect("adapter kernel exists")
                .exprs_mut()
                .alloc(KernelExpr {
                    layout: actual,
                    ..original
                })
                .map_err(|error| overflow("callable expressions", error));
        }
        let helper = if let Some(helper) =
            self.helpers
                .get(&(source.clone(), expected, original.layout))
        {
            *helper
        } else {
            let Some(helper) = self.helper(&source, expected, original.layout, original.span)?
            else {
                return Ok(value);
            };
            self.helpers
                .insert((source, expected, original.layout), helper);
            helper
        };
        let kernel = &mut self
            .program
            .kernels_mut()
            .get_mut(kernel_id)
            .expect("adapter kernel exists");
        if !kernel.global_items.contains(&helper) {
            kernel.global_items.push(helper);
            kernel.global_items.sort_unstable();
        }
        kernel
            .exprs_mut()
            .alloc(KernelExpr {
                span: original.span,
                layout: expected,
                kind: KernelExprKind::CallableAdapter {
                    value,
                    adapter: helper,
                },
            })
            .map_err(|error| overflow("callable expressions", error))
    }

    fn helper(
        &mut self,
        source: &ClosedCallable,
        layout: LayoutId,
        use_layout: LayoutId,
        span: aivi_base::SourceSpan,
    ) -> Result<Option<ItemId>, LoweringError> {
        let Some((parameters, result)) = callable_signature(self.program, layout) else {
            return Ok(None);
        };
        let Some((use_parameters, use_result)) = callable_signature(self.program, use_layout)
        else {
            return Ok(None);
        };
        if use_parameters.len() != parameters.len() {
            return Ok(None);
        }
        let target = source.target.item();
        let decl = self.program.items()[target].clone();
        let Some(body) = decl.body else {
            return Ok(None);
        };
        if parameters.len() + source.arguments.len() != decl.parameters.len() {
            return Ok(None);
        }
        let target_result = self.program.kernels()[body].result_layout;
        if !layouts_compatible(
            self.program,
            result,
            target_result,
            LayoutCompatibility::Repack,
        ) || !parameters
            .iter()
            .zip(&decl.parameters[source.arguments.len()..])
            .all(|(found, expected)| {
                layouts_compatible(self.program, *expected, *found, LayoutCompatibility::Repack)
            })
        {
            return Ok(None);
        }
        let bound_arguments =
            u32::try_from(source.arguments.len()).map_err(|_| LoweringError::ArenaOverflow {
                family: "callable adapter prefix",
                attempted_len: source.arguments.len(),
            })?;
        let mut prefix_layouts = Vec::with_capacity(source.arguments.len());
        for reference in &source.arguments {
            let Some(argument_layout) = self.declaration_layout(reference.item())? else {
                return Ok(None);
            };
            prefix_layouts.push(argument_layout);
        }
        let item = self
            .program
            .items_mut()
            .alloc(Item {
                origin: decl.origin,
                span,
                name: format!("callable-adapter#{}#{}", target.as_raw(), layout.as_raw()).into(),
                kind: ItemKind::CallableAdapter,
                parameters: parameters.clone(),
                body: None,
                pipelines: Vec::new(),
            })
            .map_err(|error| overflow("callable adapter items", error))?;
        let mut exprs = Arena::new();
        let mut globals = BTreeSet::from([target]);
        let callee_layout = self
            .declaration_layout(target)?
            .expect("target body checked above");
        let callee = exprs
            .alloc(KernelExpr {
                span,
                layout: callee_layout,
                kind: source.target.kind(),
            })
            .map_err(|error| overflow("callable adapter expressions", error))?;
        let mut arguments = Vec::new();
        for (reference, argument_layout) in source.arguments.iter().zip(prefix_layouts) {
            globals.insert(reference.item());
            arguments.push(
                exprs
                    .alloc(KernelExpr {
                        span,
                        layout: argument_layout,
                        kind: reference.kind(),
                    })
                    .map_err(|error| overflow("callable adapter expressions", error))?,
            );
        }
        let mut abi_parameters = Vec::new();
        for (index, parameter) in parameters.iter().enumerate() {
            let slot = EnvSlotId::from_raw(index as u32);
            let environment = exprs
                .alloc(KernelExpr {
                    span,
                    layout: *parameter,
                    kind: KernelExprKind::Environment(slot),
                })
                .map_err(|error| overflow("callable adapter expressions", error))?;
            let argument = if *parameter == use_parameters[index] {
                environment
            } else {
                exprs
                    .alloc(KernelExpr {
                        span,
                        layout: use_parameters[index],
                        kind: KernelExprKind::Repack { value: environment },
                    })
                    .map_err(|error| overflow("callable adapter expressions", error))?
            };
            arguments.push(argument);
            abi_parameters.push(AbiParameter {
                role: ParameterRole::Environment(slot),
                layout: *parameter,
                pass_mode: self.program.layouts()[*parameter].abi,
            });
        }
        let call = exprs
            .alloc(KernelExpr {
                span,
                layout: use_result,
                kind: KernelExprKind::Apply { callee, arguments },
            })
            .map_err(|error| overflow("callable adapter expressions", error))?;
        let root = if result == use_result {
            call
        } else {
            exprs
                .alloc(KernelExpr {
                    span,
                    layout: result,
                    kind: KernelExprKind::Repack { value: call },
                })
                .map_err(|error| overflow("callable adapter expressions", error))?
        };
        let result_pass_mode = self.program.layouts()[result].abi;
        let kernel = self
            .program
            .kernels_mut()
            .alloc(Kernel::new(
                KernelOrigin {
                    item,
                    span,
                    kind: KernelOriginKind::CallableAdapter {
                        target,
                        callable_layout: layout,
                        use_layout,
                        bound_arguments,
                    },
                },
                KernelSignature {
                    input_subject: None,
                    inline_subjects: Vec::new(),
                    environment: parameters,
                    result_layout: result,
                    convention: CallingConvention {
                        kind: CallingConventionKind::RuntimeKernelV1,
                        parameters: abi_parameters,
                        result: AbiResult {
                            layout: result,
                            pass_mode: result_pass_mode,
                        },
                    },
                    global_items: globals.into_iter().collect(),
                },
                root,
                exprs,
            ))
            .map_err(|error| overflow("callable adapter kernels", error))?;
        self.program
            .items_mut()
            .get_mut(item)
            .expect("adapter item exists")
            .body = Some(kernel);
        Ok(Some(item))
    }

    fn kernel(&mut self, id: KernelId) -> Result<(), LoweringError> {
        let count = self.program.kernels()[id].exprs().len();
        for index in 0..count {
            let expr_id = KernelExprId::from_raw(index as u32);
            let KernelExprKind::Apply {
                callee,
                mut arguments,
            } = self.program.kernels()[id].exprs()[expr_id].kind.clone()
            else {
                continue;
            };
            let callee_expr = &self.program.kernels()[id].exprs()[callee];
            let expected = match callee_expr.kind {
                KernelExprKind::Item(item) | KernelExprKind::ExecutableEvidence(item) => {
                    let decl = &self.program.items()[item];
                    if decl.body.is_some_and(|body| {
                        builtin_wrapper_intrinsic(&self.program.kernels()[body]).is_some()
                    }) && !matches!(
                        self.program.kernels()[id].origin.kind,
                        KernelOriginKind::CallableAdapter { .. }
                    ) {
                        continue;
                    }
                    decl.parameters.clone()
                }
                KernelExprKind::Environment(_) => {
                    let Some((parameters, _)) =
                        callable_signature(self.program, callee_expr.layout)
                    else {
                        continue;
                    };
                    parameters
                }
                _ => continue,
            };
            for (argument, expected) in arguments.iter_mut().zip(expected) {
                *argument = self.adapt(id, *argument, expected)?;
            }
            self.program
                .kernels_mut()
                .get_mut(id)
                .expect("adapter kernel exists")
                .exprs_mut()
                .get_mut(expr_id)
                .expect("adapter expression exists")
                .kind = KernelExprKind::Apply { callee, arguments };
        }
        Ok(())
    }
}

pub(crate) fn adapt_closed_callables(program: &mut Program) -> Result<(), LoweringError> {
    let arrows = program
        .layouts()
        .iter()
        .filter_map(|(id, layout)| match layout.kind {
            LayoutKind::Arrow { parameter, result } => Some(((parameter, result), id)),
            _ => None,
        })
        .collect();
    let mut builder = AdapterBuilder {
        program,
        arrows,
        helpers: HashMap::new(),
    };
    let mut index = 0;
    // Each generated helper is also scanned once, adapting static prerequisite
    // dictionaries with the same rules. Deduplication bounds repeated targets.
    while index < builder.program.kernels().len() {
        builder.kernel(KernelId::from_raw(index as u32))?;
        index += 1;
    }
    Ok(())
}

fn helper_source(kernel: &Kernel) -> Option<ClosedCallable> {
    let KernelOriginKind::CallableAdapter {
        target,
        bound_arguments,
        ..
    } = kernel.origin.kind
    else {
        return None;
    };
    let root = match kernel.exprs().get(kernel.root)?.kind {
        KernelExprKind::Repack { value } => value,
        _ => kernel.root,
    };
    let KernelExprKind::Apply { callee, arguments } = &kernel.exprs().get(root)?.kind else {
        return None;
    };
    let callee = closed_callable(kernel, *callee)?;
    if callee.target.item() != target || !callee.arguments.is_empty() {
        return None;
    }
    let mut prefix = Vec::new();
    for argument in arguments.get(..bound_arguments as usize)? {
        let source = closed_callable(kernel, *argument)?;
        if !source.arguments.is_empty() {
            return None;
        }
        prefix.push(source.target);
    }
    Some(ClosedCallable {
        target: callee.target,
        arguments: prefix,
    })
}

pub(crate) fn validate_helper(program: &Program, kernel: &Kernel) -> Result<(), &'static str> {
    let KernelOriginKind::CallableAdapter {
        target,
        callable_layout,
        use_layout,
        bound_arguments,
    } = kernel.origin.kind
    else {
        return Ok(());
    };
    let Some(source) = helper_source(kernel) else {
        return Err("helper body does not forward its declared target and closed prefix");
    };
    let Some((parameters, result)) = callable_signature(program, callable_layout) else {
        return Err("helper callable layout is missing or cyclic");
    };
    if parameters.is_empty()
        || parameters != kernel.environment
        || result != kernel.result_layout
        || kernel
            .exprs()
            .get(kernel.root)
            .is_none_or(|expr| expr.layout != result)
        || kernel.input_subject.is_some()
        || !kernel.inline_subjects.is_empty()
    {
        return Err("helper signature does not match its caller ABI");
    }
    let Some(target_decl) = program.items().get(target) else {
        return Err("helper target item is missing");
    };
    let Some(target_body) = target_decl
        .body
        .and_then(|body| program.kernels().get(body))
    else {
        return Err("helper target body is missing");
    };
    if matches!(
        target_body.origin.kind,
        KernelOriginKind::CallableAdapter { .. }
    ) {
        return Err("helper target must be an original callable");
    }
    let root = match kernel.exprs().get(kernel.root).map(|expr| &expr.kind) {
        Some(KernelExprKind::Repack { value }) => *value,
        _ => kernel.root,
    };
    let Some(root_expr) = kernel.exprs().get(root) else {
        return Err("helper call expression is missing");
    };
    let KernelExprKind::Apply { callee, arguments } = &root_expr.kind else {
        return Err("helper root must call its target");
    };
    let Some(callee_expr) = kernel.exprs().get(*callee) else {
        return Err("helper callee expression is missing");
    };
    if callable_signature(program, callee_expr.layout)
        != Some((target_decl.parameters.clone(), target_body.result_layout))
    {
        return Err("helper callee does not preserve the target declaration ABI");
    }
    let Some((use_parameters, use_result)) = callable_signature(program, use_layout) else {
        return Err("helper use contract is missing or cyclic");
    };
    if use_parameters.len() != parameters.len() || root_expr.layout != use_result {
        return Err("helper call does not preserve its concrete use contract");
    }
    let prefix_len = bound_arguments as usize;
    if arguments.len() != target_decl.parameters.len()
        || prefix_len + parameters.len() != arguments.len()
        || source.arguments.len() != prefix_len
    {
        return Err("helper prefix and environment do not saturate its target");
    }
    if !layouts_compatible(
        program,
        kernel.result_layout,
        target_body.result_layout,
        LayoutCompatibility::Repack,
    ) {
        return Err("helper result cannot be explicitly repacked");
    }
    for (index, argument) in arguments.iter().enumerate() {
        let Some(expr) = kernel.exprs().get(*argument) else {
            return Err("helper argument expression is missing");
        };
        if index >= prefix_len {
            let index = index - prefix_len;
            let environment = match expr.kind {
                KernelExprKind::Repack { value } => kernel
                    .exprs()
                    .get(value)
                    .ok_or("helper repack input is missing")?,
                _ => expr,
            };
            if expr.layout != use_parameters[index]
                || !matches!(environment.kind, KernelExprKind::Environment(slot) if slot.index() == index)
            {
                return Err(
                    "helper environment arguments must preserve their use contracts and declaration order",
                );
            }
        }
        if !layouts_compatible(
            program,
            target_decl.parameters[index],
            expr.layout,
            LayoutCompatibility::Repack,
        ) {
            return Err("helper argument cannot be explicitly repacked");
        }
    }
    Ok(())
}

pub(crate) fn validate_bridge(
    program: &Program,
    kernel: &Kernel,
    expr: KernelExprId,
    value: KernelExprId,
    adapter: ItemId,
) -> Result<(), &'static str> {
    let source = closed_callable(kernel, value).ok_or("bridge value is not a closed callable")?;
    let helper = program
        .items()
        .get(adapter)
        .and_then(|item| item.body)
        .and_then(|body| program.kernels().get(body))
        .ok_or("bridge helper item or body is missing")?;
    let KernelOriginKind::CallableAdapter {
        callable_layout,
        use_layout,
        ..
    } = helper.origin.kind
    else {
        return Err("bridge target is not an ABI helper");
    };
    if kernel
        .exprs()
        .get(value)
        .is_none_or(|expr| expr.layout != use_layout)
    {
        return Err("bridge value differs from its concrete use contract");
    }
    if kernel.exprs()[expr].layout != callable_layout {
        return Err("bridge layout differs from its helper ABI");
    }
    if helper_source(helper).as_ref() != Some(&source) {
        return Err("bridge changes the original callable target or bound arguments");
    }
    validate_helper(program, helper)
}
