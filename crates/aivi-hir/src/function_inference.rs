use std::collections::{HashMap, HashSet};

use crate::{
    GateType, Item, ItemId, Module,
    typecheck_context::{GateExprEnv, GateTypeContext},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FunctionCallEvidence {
    pub(crate) item_id: ItemId,
    pub(crate) argument_types: Vec<GateType>,
    pub(crate) result_type: Option<GateType>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FunctionSignatureEvidence {
    pub(crate) item_id: ItemId,
    pub(crate) parameter_types: Vec<GateType>,
    pub(crate) result_type: GateType,
}

#[derive(Clone, Debug, Default)]
struct InferenceSlot {
    ty: Option<GateType>,
    conflict: bool,
}

impl InferenceSlot {
    fn record(&mut self, candidate: GateType) -> bool {
        match self.ty.as_ref() {
            None => {
                self.ty = Some(candidate);
                true
            }
            Some(existing) if existing.same_shape(&candidate) => false,
            Some(_) => {
                self.conflict = true;
                self.ty = None;
                false
            }
        }
    }

    fn value(&self) -> Option<GateType> {
        if self.conflict {
            None
        } else {
            self.ty.clone()
        }
    }
}

#[derive(Clone, Debug)]
struct FunctionInferenceState {
    parameter_slots: Vec<InferenceSlot>,
    result_slot: InferenceSlot,
}

impl FunctionInferenceState {
    fn from_function(function: &crate::hir::FunctionItem, typing: &mut GateTypeContext<'_>) -> Self {
        let parameter_slots = function
            .parameters
            .iter()
            .map(|parameter| InferenceSlot {
                ty: parameter
                    .annotation
                    .and_then(|annotation| typing.lower_open_annotation(annotation)),
                conflict: false,
            })
            .collect();
        let result_slot = InferenceSlot {
            ty: function
                .annotation
                .and_then(|annotation| typing.lower_open_annotation(annotation)),
            conflict: false,
        };
        Self {
            parameter_slots,
            result_slot,
        }
    }

    fn parameter_types(&self) -> Option<Vec<GateType>> {
        self.parameter_slots.iter().map(InferenceSlot::value).collect()
    }

    fn arrow_type(&self) -> Option<GateType> {
        let mut result = self.result_slot.value()?;
        for parameter in self.parameter_slots.iter().rev() {
            result = GateType::Arrow {
                parameter: Box::new(parameter.value()?),
                result: Box::new(result),
            };
        }
        Some(result)
    }

    fn record_call(&mut self, argument_types: &[GateType], result_type: Option<&GateType>) -> bool {
        let mut changed = false;
        for (slot, argument_ty) in self.parameter_slots.iter_mut().zip(argument_types.iter()) {
            changed |= slot.record(argument_ty.clone());
        }
        if let Some(result_ty) = result_type {
            changed |= self.result_slot.record(result_ty.clone());
        }
        changed
    }

    fn record_signature(&mut self, parameter_types: &[GateType], result_type: &GateType) -> bool {
        if parameter_types.len() != self.parameter_slots.len() {
            return false;
        }
        let mut changed = false;
        for (slot, parameter_ty) in self.parameter_slots.iter_mut().zip(parameter_types.iter()) {
            changed |= slot.record(parameter_ty.clone());
        }
        changed |= self.result_slot.record(result_type.clone());
        changed
    }
}

pub(crate) fn supports_same_module_function_inference(function: &crate::hir::FunctionItem) -> bool {
    function.type_parameters.is_empty() && function.context.is_empty()
}

pub(crate) fn infer_same_module_function_types(module: &Module) -> HashMap<ItemId, GateType> {
    let function_ids = module
        .items()
        .iter()
        .filter_map(|(item_id, item)| match item {
            Item::Function(function) if supports_same_module_function_inference(function) => {
                Some(item_id)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    if function_ids.is_empty() {
        return HashMap::new();
    }

    let function_set = function_ids.iter().copied().collect::<HashSet<_>>();
    let mut seed_typing = GateTypeContext::new_for_function_inference(module);
    let mut states = function_ids
        .iter()
        .copied()
        .map(|item_id| {
            let Item::Function(function) = &module.items()[item_id] else {
                unreachable!("filtered above");
            };
            (
                item_id,
                FunctionInferenceState::from_function(function, &mut seed_typing),
            )
        })
        .collect::<HashMap<_, _>>();

    let mut changed = true;
    while changed {
        changed = false;

        let seeded_item_types = states
            .iter()
            .filter_map(|(item_id, state)| state.arrow_type().map(|ty| (*item_id, ty)))
            .collect::<HashMap<_, _>>();

        let call_evidence =
            collect_call_evidence(module, &function_ids, seeded_item_types.clone(), &function_set);
        for evidence in call_evidence {
            let Some(state) = states.get_mut(&evidence.item_id) else {
                continue;
            };
            changed |= state.record_call(&evidence.argument_types, evidence.result_type.as_ref());
        }

        let contextual_evidence = collect_contextual_signature_evidence(
            module,
            &function_ids,
            seeded_item_types.clone(),
            &function_set,
        );
        for evidence in contextual_evidence {
            let Some(state) = states.get_mut(&evidence.item_id) else {
                continue;
            };
            changed |=
                state.record_signature(&evidence.parameter_types, &evidence.result_type);
        }

        changed |= infer_body_results(module, &mut states, seeded_item_types);
    }

    states
        .into_iter()
        .filter_map(|(item_id, state)| state.arrow_type().map(|ty| (item_id, ty)))
        .collect()
}

fn collect_call_evidence(
    module: &Module,
    function_ids: &[ItemId],
    seeded_item_types: HashMap<ItemId, GateType>,
    function_set: &HashSet<ItemId>,
) -> Vec<FunctionCallEvidence> {
    let mut typing = GateTypeContext::with_seeded_item_types(module, seeded_item_types, true);
    let mut evidence = Vec::new();
    for item_id in function_ids {
        let Item::Function(function) = &module.items()[*item_id] else {
            continue;
        };
        evidence.extend(collect_body_evidence(
            module,
            &mut typing,
            function.body,
            &GateExprEnv::default(),
            function_set,
        ));
    }
    evidence.extend(typing.take_function_call_evidence());
    evidence.extend(
        typing
            .take_function_signature_evidence()
            .into_iter()
            .map(|evidence| FunctionCallEvidence {
                item_id: evidence.item_id,
                argument_types: evidence.parameter_types,
                result_type: Some(evidence.result_type),
            }),
    );
    evidence
}

fn collect_contextual_signature_evidence(
    module: &Module,
    function_ids: &[ItemId],
    seeded_item_types: HashMap<ItemId, GateType>,
    function_set: &HashSet<ItemId>,
) -> Vec<FunctionSignatureEvidence> {
    let typing = GateTypeContext::with_seeded_item_types(module, seeded_item_types, true);
    crate::typecheck::collect_contextual_function_signature_evidence(
        module,
        function_ids,
        typing,
        function_set,
    )
}

fn infer_body_results(
    module: &Module,
    states: &mut HashMap<ItemId, FunctionInferenceState>,
    seeded_item_types: HashMap<ItemId, GateType>,
) -> bool {
    let mut typing = GateTypeContext::with_seeded_item_types(module, seeded_item_types, true);
    let mut changed = false;
    for (item_id, state) in states.iter_mut() {
        if state.result_slot.value().is_some() {
            continue;
        }
        let Some(parameter_types) = state.parameter_types() else {
            continue;
        };
        let Item::Function(function) = &module.items()[*item_id] else {
            continue;
        };
        let mut env = GateExprEnv::default();
        for (parameter, parameter_ty) in function.parameters.iter().zip(parameter_types.iter()) {
            env.locals.insert(parameter.binding, parameter_ty.clone());
        }
        let body_info = typing.infer_expr(function.body, &env, None);
        let Some(result_ty) = body_info.actual_gate_type().or(body_info.ty) else {
            continue;
        };
        changed |= state.result_slot.record(result_ty);
    }
    changed
}

fn collect_body_evidence(
    module: &Module,
    typing: &mut GateTypeContext<'_>,
    expr_id: crate::ExprId,
    env: &GateExprEnv,
    function_set: &HashSet<ItemId>,
) -> Vec<FunctionCallEvidence> {
    let mut evidence = Vec::new();
    let expr = &module.exprs()[expr_id];
    if let crate::hir::ExprKind::Apply { callee, arguments } = &expr.kind
        && let crate::hir::ExprKind::Name(reference) = &module.exprs()[*callee].kind
        && let crate::ResolutionState::Resolved(crate::TermResolution::Item(item_id)) =
            reference.resolution.as_ref()
        && function_set.contains(item_id)
    {
        let argument_types = arguments
            .iter()
            .map(|argument| {
                let info = typing.infer_expr(*argument, env, None);
                info.actual_gate_type().or(info.ty)
            })
            .collect::<Option<Vec<_>>>();
        let result_type = typing.infer_expr(expr_id, env, None).actual_gate_type();
        if let Some(argument_types) = argument_types {
            evidence.push(FunctionCallEvidence {
                item_id: *item_id,
                argument_types,
                result_type,
            });
        }
    }

    match &expr.kind {
        crate::hir::ExprKind::Name(_)
        | crate::hir::ExprKind::Int(_)
        | crate::hir::ExprKind::Float(_)
        | crate::hir::ExprKind::Decimal(_)
        | crate::hir::ExprKind::BigInt(_)
        | crate::hir::ExprKind::Bool(_)
        | crate::hir::ExprKind::Text(_)
        | crate::hir::ExprKind::TemplateText { .. }
        | crate::hir::ExprKind::ImportValue(_)
        | crate::hir::ExprKind::Hole => {}
        crate::hir::ExprKind::Unary { expr, .. } => {
            evidence.extend(collect_body_evidence(module, typing, *expr, env, function_set));
        }
        crate::hir::ExprKind::Binary { left, right, .. } => {
            evidence.extend(collect_body_evidence(module, typing, *left, env, function_set));
            evidence.extend(collect_body_evidence(module, typing, *right, env, function_set));
        }
        crate::hir::ExprKind::Apply { callee, arguments } => {
            evidence.extend(collect_body_evidence(module, typing, *callee, env, function_set));
            for argument in arguments.iter() {
                evidence.extend(collect_body_evidence(module, typing, *argument, env, function_set));
            }
        }
        crate::hir::ExprKind::Lambda { body, .. } => {
            evidence.extend(collect_body_evidence(module, typing, *body, env, function_set));
        }
        crate::hir::ExprKind::SignalMethod { receiver, argument, .. } => {
            evidence.extend(collect_body_evidence(
                module,
                typing,
                *receiver,
                env,
                function_set,
            ));
            if let Some(argument) = argument {
                evidence.extend(collect_body_evidence(
                    module,
                    typing,
                    *argument,
                    env,
                    function_set,
                ));
            }
        }
        crate::hir::ExprKind::Record(record) => {
            for field in &record.fields {
                evidence.extend(collect_body_evidence(
                    module,
                    typing,
                    field.value,
                    env,
                    function_set,
                ));
            }
            if let Some(base) = record.base {
                evidence.extend(collect_body_evidence(module, typing, base, env, function_set));
            }
        }
        crate::hir::ExprKind::RecordFieldAccess { record, .. } => {
            evidence.extend(collect_body_evidence(module, typing, *record, env, function_set));
        }
        crate::hir::ExprKind::Tuple(items)
        | crate::hir::ExprKind::List(items)
        | crate::hir::ExprKind::Set(items) => {
            for item in items.iter() {
                evidence.extend(collect_body_evidence(module, typing, *item, env, function_set));
            }
        }
        crate::hir::ExprKind::Map(map) => {
            for entry in &map.entries {
                evidence.extend(collect_body_evidence(module, typing, entry.key, env, function_set));
                evidence.extend(collect_body_evidence(
                    module,
                    typing,
                    entry.value,
                    env,
                    function_set,
                ));
            }
        }
        crate::hir::ExprKind::Case(case) => {
            evidence.extend(collect_body_evidence(module, typing, case.subject, env, function_set));
            for arm in &case.arms {
                evidence.extend(collect_body_evidence(module, typing, arm.body, env, function_set));
            }
        }
        crate::hir::ExprKind::Let(let_expr) => {
            evidence.extend(collect_body_evidence(
                module,
                typing,
                let_expr.value,
                env,
                function_set,
            ));
            evidence.extend(collect_body_evidence(
                module,
                typing,
                let_expr.body,
                env,
                function_set,
            ));
        }
        crate::hir::ExprKind::If(if_expr) => {
            evidence.extend(collect_body_evidence(
                module,
                typing,
                if_expr.condition,
                env,
                function_set,
            ));
            evidence.extend(collect_body_evidence(
                module,
                typing,
                if_expr.then_branch,
                env,
                function_set,
            ));
            evidence.extend(collect_body_evidence(
                module,
                typing,
                if_expr.else_branch,
                env,
                function_set,
            ));
        }
        crate::hir::ExprKind::Pipe(pipe) => {
            evidence.extend(collect_body_evidence(module, typing, pipe.head, env, function_set));
            for stage in &pipe.stages {
                evidence.extend(collect_body_evidence(
                    module,
                    typing,
                    stage.body,
                    env,
                    function_set,
                ));
            }
        }
        crate::hir::ExprKind::Parenthesized(inner)
        | crate::hir::ExprKind::Group(inner)
        | crate::hir::ExprKind::Signal(inner)
        | crate::hir::ExprKind::SignalValue(inner)
        | crate::hir::ExprKind::SignalPrevious(inner) => {
            evidence.extend(collect_body_evidence(module, typing, *inner, env, function_set));
        }
        crate::hir::ExprKind::ForLoop(loop_expr) => {
            evidence.extend(collect_body_evidence(
                module,
                typing,
                loop_expr.sequence,
                env,
                function_set,
            ));
            evidence.extend(collect_body_evidence(
                module,
                typing,
                loop_expr.body,
                env,
                function_set,
            ));
        }
        crate::hir::ExprKind::WhileLoop(loop_expr) => {
            evidence.extend(collect_body_evidence(
                module,
                typing,
                loop_expr.condition,
                env,
                function_set,
            ));
            evidence.extend(collect_body_evidence(
                module,
                typing,
                loop_expr.body,
                env,
                function_set,
            ));
        }
        crate::hir::ExprKind::Assignment(assignment) => {
            evidence.extend(collect_body_evidence(
                module,
                typing,
                assignment.value,
                env,
                function_set,
            ));
        }
        crate::hir::ExprKind::DomainMemberAccess { base, .. } => {
            evidence.extend(collect_body_evidence(module, typing, *base, env, function_set));
        }
        crate::hir::ExprKind::ReactiveBlock(block) => {
            evidence.extend(collect_body_evidence(module, typing, block.subject, env, function_set));
            for arm in &block.arms {
                evidence.extend(collect_body_evidence(module, typing, arm.body, env, function_set));
            }
        }
    }

    evidence
}
