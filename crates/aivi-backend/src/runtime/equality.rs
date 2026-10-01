pub(crate) fn normalize_signal_kernel_result(
    program: &Program,
    kernel: KernelId,
    raw_result: RuntimeValue,
    expected: LayoutId,
) -> Result<RuntimeValue, EvaluationError> {
    let result = match (&program.layouts()[expected].kind, raw_result) {
        (LayoutKind::Signal { element }, value) if value_matches_layout(program, &value, *element) => {
            value
        }
        (_, RuntimeValue::Signal(value)) if value_matches_layout(program, value.as_ref(), expected) => {
            *value
        }
        (_, value) => value,
    };
    if !value_matches_layout(program, &result, expected) {
        return Err(EvaluationError::KernelResultLayoutMismatch {
            kernel,
            expected,
            found: Box::new(result),
        });
    }
    Ok(result)
}

pub(crate) fn value_matches_layout_with_signal_current(
    program: &Program,
    value: &RuntimeValue,
    layout: LayoutId,
) -> bool {
    value_matches_layout(program, value, layout)
        || matches!(value, RuntimeValue::Signal(inner) if value_matches_layout(program, inner, layout))
}

pub(crate) fn value_matches_layout(program: &Program, value: &RuntimeValue, layout: LayoutId) -> bool {
    let Some(layout) = program.layouts().get(layout) else {
        return false;
    };
    match (&layout.kind, value) {
        (LayoutKind::Primitive(PrimitiveType::Unit), RuntimeValue::Unit) => true,
        (LayoutKind::Primitive(PrimitiveType::Bool), RuntimeValue::Bool(_)) => true,
        (LayoutKind::Primitive(PrimitiveType::Int), RuntimeValue::Int(_)) => true,
        (LayoutKind::Primitive(PrimitiveType::Float), RuntimeValue::Float(_)) => true,
        (LayoutKind::Primitive(PrimitiveType::Decimal), RuntimeValue::Decimal(_)) => true,
        (LayoutKind::Primitive(PrimitiveType::BigInt), RuntimeValue::BigInt(_)) => true,
        (LayoutKind::Primitive(PrimitiveType::Text), RuntimeValue::Text(_)) => true,
        (LayoutKind::Primitive(PrimitiveType::Bytes), RuntimeValue::Bytes(_)) => true,
        (LayoutKind::Primitive(PrimitiveType::Task), RuntimeValue::Task(_))
        | (LayoutKind::Task { .. }, RuntimeValue::Task(_)) => true,
        (LayoutKind::Tuple(expected), RuntimeValue::Tuple(elements)) => {
            expected.len() == elements.len()
                && expected
                    .iter()
                    .zip(elements.iter())
                    .all(|(layout, value)| value_matches_layout(program, value, *layout))
        }
        // Shallow tag checks: the typechecker guarantees element types are correct, so
        // walking every element on every kernel call would be O(N) per check and catastrophic
        // for large collections (e.g. Matrix = List(List(...))).
        (LayoutKind::List { .. }, RuntimeValue::List(_))
        | (LayoutKind::Set { .. }, RuntimeValue::Set(_)) => true,
        (LayoutKind::Map { .. }, RuntimeValue::Map(_)) => true,
        (LayoutKind::Record(expected), RuntimeValue::Record(fields)) => {
            expected.len() == fields.len()
                && expected.iter().zip(fields.iter()).all(|(layout, field)| {
                    layout.name.as_ref() == field.label.as_ref()
                        && value_matches_layout(program, &field.value, layout.layout)
                })
        }
        (LayoutKind::Sum(variants), RuntimeValue::Sum(value)) => variants
            .iter()
            .find(|variant| variant.name.as_ref() == value.variant_name.as_ref())
            .is_some_and(|variant| {
                sum_fields_match_layout(program, &value.fields, variant.payload)
            }),
        (LayoutKind::Option { element }, RuntimeValue::OptionNone) => {
            let _ = element;
            true
        }
        (LayoutKind::Option { element }, RuntimeValue::OptionSome(value)) => {
            value_matches_layout(program, value, *element)
        }
        (LayoutKind::Result { value, .. }, RuntimeValue::ResultOk(result)) => {
            value_matches_layout(program, result, *value)
        }
        (LayoutKind::Result { error, .. }, RuntimeValue::ResultErr(result)) => {
            value_matches_layout(program, result, *error)
        }
        (LayoutKind::Validation { value, .. }, RuntimeValue::ValidationValid(result)) => {
            value_matches_layout(program, result, *value)
        }
        (LayoutKind::Validation { error, .. }, RuntimeValue::ValidationInvalid(result)) => {
            value_matches_layout(program, result, *error)
        }
        (LayoutKind::Signal { element }, RuntimeValue::Signal(value)) => {
            value_matches_layout(program, value, *element)
        }
        (LayoutKind::Signal { element }, value) => value_matches_layout(program, value, *element),
        (LayoutKind::Arrow { .. }, RuntimeValue::Callable(_)) => true,
        (LayoutKind::AnonymousDomain { .. }, RuntimeValue::SuffixedInteger { .. }) => true,
        (LayoutKind::Domain { .. }, RuntimeValue::Signal(_)) => false,
        // Named-domain layouts erase their carrier shape in backend IR. Runtime evaluation relies
        // on earlier typed lowering to keep those carrier values sound and only preserves the
        // outer signal/non-signal distinction here.
        (LayoutKind::Domain { .. }, _) => true,
        (LayoutKind::Opaque { name, variants, .. }, RuntimeValue::Sum(value)) => {
            name.as_ref() == value.type_name.as_ref()
                && (variants.is_empty()
                    || variants
                        .iter()
                        .find(|variant| variant.name.as_ref() == value.variant_name.as_ref())
                        .is_some_and(|variant| {
                            sum_fields_match_layout(program, &value.fields, variant.payload)
                        }))
        }
        _ => false,
    }
}

fn sum_fields_match_layout(
    program: &Program,
    fields: &[RuntimeValue],
    payload: Option<LayoutId>,
) -> bool {
    match (payload, fields) {
        (None, []) => true,
        (Some(layout), [field]) => value_matches_layout(program, field, layout),
        (Some(layout), fields) if fields.len() > 1 => {
            let Some(layout) = program.layouts().get(layout) else {
                return false;
            };
            let LayoutKind::Tuple(expected) = &layout.kind else {
                return false;
            };
            expected.len() == fields.len()
                && expected
                    .iter()
                    .zip(fields.iter())
                    .all(|(layout, field)| value_matches_layout(program, field, *layout))
        }
        _ => false,
    }
}

impl KernelEvaluator<'_> {
    fn evaluate_derived_equality(
        &mut self,
        kernel: KernelId,
        expr: KernelExprId,
        shape: &aivi_hir::EqualityShape,
        arguments: Vec<RuntimeValue>,
        globals: &BTreeMap<ItemId, RuntimeValue>,
    ) -> Result<RuntimeValue, EvaluationError> {
        use aivi_hir::EqualityShapeNode as Node;
        let invalid = |reason| EvaluationError::UnsupportedBuiltinClassMember {
            kernel,
            expr,
            intrinsic: BuiltinClassMemberIntrinsic::DerivedStructuralEq(std::sync::Arc::new(
                shape.clone(),
            )),
            reason,
        };
        shape
            .validate()
            .map_err(|_| invalid("invalid derived equality shape"))?;
        if arguments.len() != shape.evidence_count() + 2 {
            return Err(invalid(
                "derived equality received the wrong argument count",
            ));
        }
        // These owners release arbitrarily deep value trees iteratively, even
        // on a short circuit or a callback error. Work entries borrow owners.
        let arguments = arguments
            .into_iter()
            .map(DetachedRuntimeValue::from_runtime_owned)
            .collect::<Vec<_>>();
        let operands = &arguments[shape.evidence_count()..];
        let mut pending = vec![(
            shape.root(),
            operands[0].as_runtime(),
            operands[1].as_runtime(),
        )];
        while let Some((id, left, right)) = pending.pop() {
            match shape
                .node(id)
                .ok_or_else(|| invalid("invalid derived equality edge"))?
            {
                Node::Structural => {
                    if !structural_eq(kernel, expr, left, right)? {
                        return Ok(RuntimeValue::Bool(false));
                    }
                }
                Node::Evidence(slot) => {
                    let callable = arguments[slot.as_raw() as usize].as_runtime().clone();
                    let result = self.apply_callable(
                        kernel,
                        expr,
                        callable,
                        vec![left.clone(), right.clone()],
                        globals,
                    )?;
                    match result {
                        RuntimeValue::Bool(true) => {}
                        RuntimeValue::Bool(false) => return Ok(RuntimeValue::Bool(false)),
                        _ => return Err(invalid("equality evidence did not return Bool")),
                    }
                }
                Node::Carrier(child) => pending.push((*child, left, right)),
                Node::Tuple(fields) => {
                    let (RuntimeValue::Tuple(left), RuntimeValue::Tuple(right)) = (left, right)
                    else {
                        return Err(invalid(
                            "derived tuple equality received a different representation",
                        ));
                    };
                    if left.len() != fields.len() || right.len() != fields.len() {
                        return Err(invalid("derived tuple equality received a different arity"));
                    }
                    pending.extend(
                        fields
                            .iter()
                            .zip(left.iter().zip(right))
                            .rev()
                            .map(|(id, (left, right))| (*id, left, right)),
                    );
                }
                Node::Record(fields) => {
                    let (RuntimeValue::Record(left), RuntimeValue::Record(right)) = (left, right)
                    else {
                        return Err(invalid(
                            "derived record equality received a different representation",
                        ));
                    };
                    if left.len() != fields.len() || right.len() != fields.len() {
                        return Err(invalid(
                            "derived record equality received a different field count",
                        ));
                    }
                    for field in fields.iter().rev() {
                        let left = left
                            .iter()
                            .find(|actual| actual.label == field.name)
                            .ok_or_else(|| invalid("derived equality record field is absent"))?;
                        let right = right
                            .iter()
                            .find(|actual| actual.label == field.name)
                            .ok_or_else(|| invalid("derived equality record field is absent"))?;
                        pending.push((field.node, &left.value, &right.value));
                    }
                }
                Node::Sum(variants) => {
                    let (RuntimeValue::Sum(left), RuntimeValue::Sum(right)) = (left, right) else {
                        return Err(invalid(
                            "derived sum equality received a different representation",
                        ));
                    };
                    if left.item != right.item || left.variant_name != right.variant_name {
                        return Ok(RuntimeValue::Bool(false));
                    }
                    let variant = variants
                        .iter()
                        .find(|variant| variant.name == left.variant_name)
                        .ok_or_else(|| invalid("derived equality constructor is absent"))?;
                    if left.fields.len() != variant.fields.len()
                        || right.fields.len() != variant.fields.len()
                    {
                        return Err(invalid("derived equality constructor arity differs"));
                    }
                    pending.extend(
                        variant
                            .fields
                            .iter()
                            .zip(left.fields.iter().zip(&right.fields))
                            .rev()
                            .map(|(id, (left, right))| (*id, left, right)),
                    );
                }
                Node::List(child) => {
                    let (RuntimeValue::List(left), RuntimeValue::List(right)) = (left, right)
                    else {
                        return Err(invalid(
                            "derived list equality received a different representation",
                        ));
                    };
                    if left.len() != right.len() {
                        return Ok(RuntimeValue::Bool(false));
                    }
                    pending.extend(
                        left.iter()
                            .zip(right)
                            .rev()
                            .map(|(left, right)| (*child, left, right)),
                    );
                }
                Node::Option(child) => match (left, right) {
                    (RuntimeValue::OptionNone, RuntimeValue::OptionNone) => {}
                    (RuntimeValue::OptionSome(left), RuntimeValue::OptionSome(right)) => {
                        pending.push((*child, left, right))
                    }
                    (RuntimeValue::OptionNone, RuntimeValue::OptionSome(_))
                    | (RuntimeValue::OptionSome(_), RuntimeValue::OptionNone) => {
                        return Ok(RuntimeValue::Bool(false));
                    }
                    _ => {
                        return Err(invalid(
                            "derived option equality received a different representation",
                        ));
                    }
                },
                Node::Result { error, value } => match (left, right) {
                    (RuntimeValue::ResultErr(left), RuntimeValue::ResultErr(right)) => {
                        pending.push((*error, left, right))
                    }
                    (RuntimeValue::ResultOk(left), RuntimeValue::ResultOk(right)) => {
                        pending.push((*value, left, right))
                    }
                    (RuntimeValue::ResultErr(_), RuntimeValue::ResultOk(_))
                    | (RuntimeValue::ResultOk(_), RuntimeValue::ResultErr(_)) => {
                        return Ok(RuntimeValue::Bool(false));
                    }
                    _ => {
                        return Err(invalid(
                            "derived result equality received a different representation",
                        ));
                    }
                },
                Node::Validation { error, value } => match (left, right) {
                    (
                        RuntimeValue::ValidationInvalid(left),
                        RuntimeValue::ValidationInvalid(right),
                    ) => pending.push((*error, left, right)),
                    (RuntimeValue::ValidationValid(left), RuntimeValue::ValidationValid(right)) => {
                        pending.push((*value, left, right))
                    }
                    (RuntimeValue::ValidationInvalid(_), RuntimeValue::ValidationValid(_))
                    | (RuntimeValue::ValidationValid(_), RuntimeValue::ValidationInvalid(_)) => {
                        return Ok(RuntimeValue::Bool(false));
                    }
                    _ => {
                        return Err(invalid(
                            "derived validation equality received a different representation",
                        ));
                    }
                },
            }
        }
        Ok(RuntimeValue::Bool(true))
    }
}

fn structural_eq(
    kernel: KernelId,
    expr: KernelExprId,
    left: &RuntimeValue,
    right: &RuntimeValue,
) -> Result<bool, EvaluationError> {
    if let RuntimeValue::Signal(inner) = left {
        return structural_eq(kernel, expr, inner, right);
    }
    if let RuntimeValue::Signal(inner) = right {
        return structural_eq(kernel, expr, left, inner);
    }
    let equal = match (left, right) {
        (RuntimeValue::Unit, RuntimeValue::Unit) => true,
        (RuntimeValue::Bool(left), RuntimeValue::Bool(right)) => left == right,
        (RuntimeValue::Int(left), RuntimeValue::Int(right)) => left == right,
        (RuntimeValue::Float(left), RuntimeValue::Float(right)) => left == right,
        (RuntimeValue::Decimal(left), RuntimeValue::Decimal(right)) => left == right,
        (RuntimeValue::BigInt(left), RuntimeValue::BigInt(right)) => left == right,
        (RuntimeValue::Text(left), RuntimeValue::Text(right)) => left == right,
        (RuntimeValue::Bytes(left), RuntimeValue::Bytes(right)) => left == right,
        (RuntimeValue::Int(left), RuntimeValue::SuffixedInteger { raw, .. })
        | (RuntimeValue::SuffixedInteger { raw, .. }, RuntimeValue::Int(left)) => {
            raw.parse::<i64>().ok() == Some(*left)
        }
        (
            RuntimeValue::SuffixedInteger {
                raw: left_raw,
                suffix: left_suffix,
            },
            RuntimeValue::SuffixedInteger {
                raw: right_raw,
                suffix: right_suffix,
            },
        ) => left_raw == right_raw && left_suffix == right_suffix,
        (RuntimeValue::Tuple(left), RuntimeValue::Tuple(right))
        | (RuntimeValue::List(left), RuntimeValue::List(right)) => {
            if left.len() != right.len() {
                false
            } else {
                for (left, right) in left.iter().zip(right.iter()) {
                    if !structural_eq(kernel, expr, left, right)? {
                        return Ok(false);
                    }
                }
                true
            }
        }
        (RuntimeValue::Set(left), RuntimeValue::Set(right)) => {
            unordered_runtime_values_eq(kernel, expr, left, right)?
        }
        (RuntimeValue::Map(left), RuntimeValue::Map(right)) => {
            unordered_runtime_map_eq(kernel, expr, left, right)?
        }
        (RuntimeValue::Record(left), RuntimeValue::Record(right)) => {
            if left.len() != right.len() {
                false
            } else {
                for (left, right) in left.iter().zip(right.iter()) {
                    if left.label != right.label
                        || !structural_eq(kernel, expr, &left.value, &right.value)?
                    {
                        return Ok(false);
                    }
                }
                true
            }
        }
        (RuntimeValue::Sum(left), RuntimeValue::Sum(right)) => {
            if left.item != right.item
                || left.variant_name != right.variant_name
                || left.fields.len() != right.fields.len()
            {
                false
            } else {
                for (left, right) in left.fields.iter().zip(right.fields.iter()) {
                    if !structural_eq(kernel, expr, left, right)? {
                        return Ok(false);
                    }
                }
                true
            }
        }
        (RuntimeValue::OptionNone, RuntimeValue::OptionNone) => true,
        (RuntimeValue::OptionSome(left), RuntimeValue::OptionSome(right))
        | (RuntimeValue::ResultOk(left), RuntimeValue::ResultOk(right))
        | (RuntimeValue::ResultErr(left), RuntimeValue::ResultErr(right))
        | (RuntimeValue::ValidationValid(left), RuntimeValue::ValidationValid(right))
        | (RuntimeValue::ValidationInvalid(left), RuntimeValue::ValidationInvalid(right))
        | (RuntimeValue::Signal(left), RuntimeValue::Signal(right)) => {
            structural_eq(kernel, expr, left, right)?
        }
        (RuntimeValue::Callable(_), _)
        | (_, RuntimeValue::Callable(_))
        | (RuntimeValue::Task(_), _)
        | (_, RuntimeValue::Task(_)) => {
            return Err(EvaluationError::UnsupportedStructuralEquality {
                kernel,
                expr,
                left: Box::new(left.clone()),
                right: Box::new(right.clone()),
            });
        }
        _ => false,
    };
    Ok(equal)
}

fn unordered_runtime_values_eq(
    kernel: KernelId,
    expr: KernelExprId,
    left: &[RuntimeValue],
    right: &[RuntimeValue],
) -> Result<bool, EvaluationError> {
    if left.len() != right.len() {
        return Ok(false);
    }
    let mut matched = vec![false; right.len()];
    'left_values: for left_value in left {
        for (index, right_value) in right.iter().enumerate() {
            if matched[index] {
                continue;
            }
            if !runtime_values_may_match(left_value, right_value) {
                continue;
            }
            if structural_eq(kernel, expr, left_value, right_value)? {
                matched[index] = true;
                continue 'left_values;
            }
        }
        return Ok(false);
    }
    Ok(true)
}

#[cfg(test)]
mod derived_dictionary_tests {
    use super::*;
    use aivi_hir::{EqualityEvidenceId, EqualityNodeId, EqualityShape, EqualityShapeNode as Node, EqualitySumVariant};

    #[test]
    fn derived_equality_walks_and_releases_deep_recursive_values_on_a_small_stack() {
        std::thread::Builder::new()
            .stack_size(256 * 1024)
            .spawn(|| {
                let root = EqualityNodeId::from_raw(0);
                let shape = EqualityShape::new(
                    root,
                    vec![
                        Node::Sum(vec![
                            EqualitySumVariant {
                                name: "End".into(),
                                fields: vec![EqualityNodeId::from_raw(1)],
                            },
                            EqualitySumVariant {
                                name: "Next".into(),
                                fields: vec![root],
                            },
                        ]),
                        Node::Evidence(EqualityEvidenceId::from_raw(0)),
                    ],
                    1,
                )
                .unwrap();
                let build = |last| {
                    let sum = |name: &str, value| {
                        RuntimeValue::Sum(RuntimeSumValue {
                            item: HirItemId::from_raw(0),
                            type_name: "Chain".into(),
                            variant_name: name.into(),
                            fields: vec![value],
                        })
                    };
                    let mut value = sum("End", RuntimeValue::Int(last));
                    for _ in 0..100_000 {
                        value = sum("Next", value);
                    }
                    value
                };
                let program = Program::new();
                let mut evaluator = KernelEvaluator::new(&program);
                for (right, expected) in [(3, true), (4, false)] {
                    let result = evaluator
                        .evaluate_derived_equality(
                            KernelId::from_raw(0),
                            KernelExprId::from_raw(0),
                            &shape,
                            vec![
                                runtime_class_member_value(
                                    BuiltinClassMemberIntrinsic::StructuralEq,
                                ),
                                build(3),
                                build(right),
                            ],
                            &BTreeMap::new(),
                        )
                        .unwrap();
                    assert_eq!(result, RuntimeValue::Bool(expected));
                }
            })
            .unwrap()
            .join()
            .unwrap();
    }
}

fn unordered_runtime_map_eq(
    kernel: KernelId,
    expr: KernelExprId,
    left: &RuntimeMap,
    right: &RuntimeMap,
) -> Result<bool, EvaluationError> {
    if left.len() != right.len() {
        return Ok(false);
    }
    // Use O(1) key lookup on `right` to drive the comparison in O(n) rather
    // than the previous O(n²) linear scan.  Both sides must agree on every
    // key, and the associated values must be structurally equal.
    for (left_key, left_value) in left {
        let Some(right_value) = right.get(left_key) else {
            return Ok(false);
        };
        if !structural_eq(kernel, expr, left_value, right_value)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn runtime_values_may_match(left: &RuntimeValue, right: &RuntimeValue) -> bool {
    match (left, right) {
        (RuntimeValue::Signal(left), right) => runtime_values_may_match(left, right),
        (left, RuntimeValue::Signal(right)) => runtime_values_may_match(left, right),
        (RuntimeValue::Unit, RuntimeValue::Unit)
        | (RuntimeValue::Bool(_), RuntimeValue::Bool(_))
        | (RuntimeValue::Int(_), RuntimeValue::Int(_))
        | (RuntimeValue::Float(_), RuntimeValue::Float(_))
        | (RuntimeValue::Decimal(_), RuntimeValue::Decimal(_))
        | (RuntimeValue::BigInt(_), RuntimeValue::BigInt(_))
        | (RuntimeValue::Text(_), RuntimeValue::Text(_))
        | (RuntimeValue::Bytes(_), RuntimeValue::Bytes(_))
        | (RuntimeValue::Tuple(_), RuntimeValue::Tuple(_))
        | (RuntimeValue::List(_), RuntimeValue::List(_))
        | (RuntimeValue::Set(_), RuntimeValue::Set(_))
        | (RuntimeValue::Map(_), RuntimeValue::Map(_))
        | (RuntimeValue::Record(_), RuntimeValue::Record(_))
        | (RuntimeValue::Sum(_), RuntimeValue::Sum(_))
        | (RuntimeValue::OptionNone, RuntimeValue::OptionNone)
        | (RuntimeValue::OptionSome(_), RuntimeValue::OptionSome(_))
        | (RuntimeValue::ResultOk(_), RuntimeValue::ResultOk(_))
        | (RuntimeValue::ResultErr(_), RuntimeValue::ResultErr(_))
        | (RuntimeValue::ValidationValid(_), RuntimeValue::ValidationValid(_))
        | (RuntimeValue::ValidationInvalid(_), RuntimeValue::ValidationInvalid(_))
        | (RuntimeValue::Task(_), RuntimeValue::Task(_))
        | (RuntimeValue::Callable(_), RuntimeValue::Callable(_))
        | (RuntimeValue::SuffixedInteger { .. }, RuntimeValue::SuffixedInteger { .. }) => true,
        (RuntimeValue::Int(_), RuntimeValue::SuffixedInteger { .. })
        | (RuntimeValue::SuffixedInteger { .. }, RuntimeValue::Int(_)) => true,
        _ => false,
    }
}

fn project_field(
    kernel: KernelId,
    expr: KernelExprId,
    value: RuntimeValue,
    label: &str,
) -> Result<RuntimeValue, EvaluationError> {
    let value = strip_signal(value);
    let RuntimeValue::Record(fields) = value else {
        return Err(EvaluationError::InvalidProjectionBase {
            kernel,
            expr,
            found: Box::new(value),
        });
    };
    fields
        .into_iter()
        .find(|field| field.label.as_ref() == label)
        .map(|field| field.value)
        .ok_or_else(|| EvaluationError::UnknownProjectionField {
            kernel,
            expr,
            label: label.into(),
        })
}

fn pop_value(values: &mut Vec<RuntimeValue>) -> RuntimeValue {
    values
        .pop()
        .expect("backend runtime evaluation should keep task/value stacks aligned")
}

fn drain_tail<T>(values: &mut Vec<T>, len: usize) -> Vec<T> {
    let split = values
        .len()
        .checked_sub(len)
        .expect("backend runtime evaluation should not underflow its value stack");
    values.split_off(split)
}

fn truthy_falsy_payload(
    value: &RuntimeValue,
    constructor: BuiltinTerm,
) -> Option<Option<RuntimeValue>> {
    match (constructor, value) {
        (BuiltinTerm::True, RuntimeValue::Bool(true))
        | (BuiltinTerm::False, RuntimeValue::Bool(false))
        | (BuiltinTerm::None, RuntimeValue::OptionNone) => Some(None),
        (BuiltinTerm::Some, RuntimeValue::OptionSome(payload))
        | (BuiltinTerm::Ok, RuntimeValue::ResultOk(payload))
        | (BuiltinTerm::Err, RuntimeValue::ResultErr(payload))
        | (BuiltinTerm::Valid, RuntimeValue::ValidationValid(payload))
        | (BuiltinTerm::Invalid, RuntimeValue::ValidationInvalid(payload)) => {
            Some(Some((**payload).clone()))
        }
        _ => None,
    }
}

pub fn coerce_runtime_value(
    program: &Program,
    mut value: RuntimeValue,
    layout: LayoutId,
) -> Result<RuntimeValue, RuntimeValue> {
    let Some(layout_def) = program.layouts().get(layout) else {
        return Err(value);
    };
    if let LayoutKind::Signal { element } = &layout_def.kind {
        let inner = match value {
            RuntimeValue::Signal(inner) => *inner,
            other => other,
        };
        return coerce_runtime_value(program, inner, *element)
            .map(|inner| RuntimeValue::Signal(Box::new(inner)));
    }
    if value_matches_layout(program, &value, layout) {
        // Decoded/imported sums may carry an ItemId from a different module's
        // arena. Name, variant, and payload have passed the destination layout's
        // checks; use that layout's identity for subsequent constructor dispatch.
        if let (LayoutKind::Opaque { item: Some(item), .. }, RuntimeValue::Sum(sum)) =
            (&layout_def.kind, &mut value)
        {
            sum.item = *item;
        }
        return Ok(value);
    }
    if let RuntimeValue::Signal(inner) = &value {
        let payload = inner.as_ref().clone();
        if value_matches_layout(program, &payload, layout) {
            return Ok(payload);
        }
    }
    match &layout_def.kind {
        LayoutKind::Option { element } => {
            if value_matches_layout(program, &value, *element) {
                Ok(RuntimeValue::OptionSome(Box::new(value)))
            } else {
                Err(value)
            }
        }
        LayoutKind::Result { value: ok, error } => {
            let matches_ok = value_matches_layout(program, &value, *ok);
            let matches_err = value_matches_layout(program, &value, *error);
            match (matches_ok, matches_err) {
                (true, false) => Ok(RuntimeValue::ResultOk(Box::new(value))),
                (false, true) => Ok(RuntimeValue::ResultErr(Box::new(value))),
                _ => Err(value),
            }
        }
        LayoutKind::Validation {
            value: valid,
            error: invalid,
        } => {
            let matches_valid = value_matches_layout(program, &value, *valid);
            let matches_invalid = value_matches_layout(program, &value, *invalid);
            match (matches_valid, matches_invalid) {
                (true, false) => Ok(RuntimeValue::ValidationValid(Box::new(value))),
                (false, true) => Ok(RuntimeValue::ValidationInvalid(Box::new(value))),
                _ => Err(value),
            }
        }
        _ => Err(value),
    }
}

fn coerce_inline_pipe_value(
    program: &Program,
    value: RuntimeValue,
    layout: LayoutId,
) -> Option<RuntimeValue> {
    coerce_runtime_value(program, value, layout).ok()
}

pub(crate) fn strip_signal(value: RuntimeValue) -> RuntimeValue {
    match value {
        RuntimeValue::Signal(value) => *value,
        other => other,
    }
}

fn append_validation_errors(
    left: RuntimeValue,
    right: RuntimeValue,
) -> Result<RuntimeValue, &'static str> {
    let RuntimeValue::Sum(left) = left else {
        return Err(
            "Validation apply only accumulates Invalid payloads shaped as `NonEmpty`/`NonEmptyList`",
        );
    };
    let RuntimeValue::Sum(right) = right else {
        return Err(
            "Validation apply only accumulates Invalid payloads shaped as `NonEmpty`/`NonEmptyList`",
        );
    };
    if !matches_non_empty_runtime(&left) || !matches_non_empty_runtime(&right) {
        return Err(
            "Validation apply only accumulates Invalid payloads shaped as `NonEmpty`/`NonEmptyList`",
        );
    }

    let RuntimeSumValue {
        item,
        type_name,
        variant_name,
        fields: left_fields,
    } = left;
    let mut left_fields = left_fields;
    let head = left_fields.remove(0);
    let left_tail = match left_fields.remove(0) {
        RuntimeValue::List(values) => values,
        _ => {
            return Err(
                "Validation apply only accumulates Invalid payloads shaped as `NonEmpty`/`NonEmptyList`",
            );
        }
    };

    let RuntimeSumValue {
        fields: right_fields,
        ..
    } = right;
    let mut right_fields = right_fields;
    let right_head = right_fields.remove(0);
    let right_tail = match right_fields.remove(0) {
        RuntimeValue::List(values) => values,
        _ => {
            return Err(
                "Validation apply only accumulates Invalid payloads shaped as `NonEmpty`/`NonEmptyList`",
            );
        }
    };

    let mut tail = left_tail;
    tail.push(right_head);
    tail.extend(right_tail);

    Ok(RuntimeValue::Sum(RuntimeSumValue {
        item,
        type_name,
        variant_name,
        fields: vec![head, RuntimeValue::List(tail)],
    }))
}

fn matches_non_empty_runtime(value: &RuntimeSumValue) -> bool {
    matches!(value.type_name.as_ref(), "NonEmpty" | "NonEmptyList")
        && matches!(value.variant_name.as_ref(), "NonEmpty" | "NonEmptyList")
        && value.fields.len() == 2
        && matches!(value.fields.get(1), Some(RuntimeValue::List(_)))
}
