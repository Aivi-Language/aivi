#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ConstraintClass {
    Eq,
    Default,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ConstraintOrigin {
    Expression,
    RecordOmittedField {
        field_name: String,
        available_fields: Vec<String>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DefaultEvidence {
    BuiltinOptionNone,
    ImportedBinding(ImportId),
    SameModuleMemberBody(ExprId),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct ConstraintSolveReport {
    default_record_fields: Vec<SolvedDefaultRecordField>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SolvedDefaultRecordField {
    field_name: String,
    evidence: DefaultEvidence,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DefaultRecordElision {
    record_expr: ExprId,
    fields: Vec<SolvedDefaultRecordField>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct EqConstraintScope {
    constrained_parameters: HashSet<TypeParameterId>,
    class_constraints: Vec<ClassConstraintBinding>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PendingEqConstraint {
    constraint: TypeConstraint,
    scope: EqConstraintScope,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ImportedDefaultValue {
    builtin: BuiltinType,
    import: ImportId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypeConstraint {
    span: SourceSpan,
    class: ConstraintClass,
    subject: GateType,
    origin: ConstraintOrigin,
}

impl TypeConstraint {
    pub(crate) fn eq(span: SourceSpan, subject: GateType) -> Self {
        Self {
            span,
            class: ConstraintClass::Eq,
            subject,
            origin: ConstraintOrigin::Expression,
        }
    }

    pub(crate) fn default_record_field(
        span: SourceSpan,
        field_name: impl Into<String>,
        subject: GateType,
        available_fields: Vec<String>,
    ) -> Self {
        Self {
            span,
            class: ConstraintClass::Default,
            subject,
            origin: ConstraintOrigin::RecordOmittedField {
                field_name: field_name.into(),
                available_fields,
            },
        }
    }

    pub fn span(&self) -> SourceSpan {
        self.span
    }

    pub fn class(&self) -> &ConstraintClass {
        &self.class
    }

    pub fn subject(&self) -> &GateType {
        &self.subject
    }

    fn omitted_field_name(&self) -> Option<&str> {
        match &self.origin {
            ConstraintOrigin::Expression => None,
            ConstraintOrigin::RecordOmittedField { field_name, .. } => Some(field_name),
        }
    }

    fn available_field_names(&self) -> Option<&[String]> {
        match &self.origin {
            ConstraintOrigin::Expression => None,
            ConstraintOrigin::RecordOmittedField {
                available_fields, ..
            } => Some(available_fields),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TypeCheckReport {
    diagnostics: Vec<Diagnostic>,
    elisions: Vec<DefaultRecordElision>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ClassMemberImplementation {
    Builtin,
    SameModuleInstance {
        instance: ItemId,
        member_index: usize,
    },
    ImportedInstance {
        import: ImportId,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ResolvedClassMemberDispatch {
    pub member: ClassMemberResolution,
    pub subject: TypeBinding,
    pub implementation: ClassMemberImplementation,
}

impl TypeCheckReport {
    fn new(diagnostics: Vec<Diagnostic>, elisions: Vec<DefaultRecordElision>) -> Self {
        Self {
            diagnostics,
            elisions,
        }
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    pub fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }

    pub fn is_ok(&self) -> bool {
        self.diagnostics.is_empty()
    }
}

pub fn typecheck_module(module: &Module) -> TypeCheckReport {
    let mut checker = TypeChecker::new(module);
    checker.run();
    TypeCheckReport::new(checker.diagnostics, checker.default_record_elisions)
}

/// Applies the default-record-field elisions computed by [`typecheck_module`] to `module`,
/// returning the elaborated module with synthesized fields injected.
pub fn apply_defaults(module: &Module, report: &TypeCheckReport) -> Module {
    apply_default_record_elisions(module, &report.elisions)
}

pub fn elaborate_default_record_fields(module: &Module) -> Module {
    apply_defaults(module, &typecheck_module(module))
}

pub(crate) fn expression_matches(
    module: &Module,
    expr_id: ExprId,
    env: &GateExprEnv,
    expected: &GateType,
) -> bool {
    expression_matches_with_typing(module, expr_id, env, expected, GateTypeContext::new(module))
}

pub(crate) fn expression_matches_with_typing<'a>(
    module: &'a Module,
    expr_id: ExprId,
    env: &GateExprEnv,
    expected: &GateType,
    typing: GateTypeContext<'a>,
) -> bool {
    expression_signature_evidence_with_typing(module, expr_id, env, expected, typing).is_some()
}

pub(crate) fn expression_signature_evidence_with_typing<'a>(
    module: &'a Module,
    expr_id: ExprId,
    env: &GateExprEnv,
    expected: &GateType,
    typing: GateTypeContext<'a>,
) -> Option<Vec<FunctionSignatureEvidence>> {
    let mut checker = TypeChecker::with_typing(module, typing);
    let constraints = evidence_scope_constraints(env);
    let matched = checker.with_class_constraint_scope(constraints, |checker| {
        let matched = checker.check_expr(expr_id, env, Some(expected), &mut Vec::new());
        checker.solve_pending_eq_constraints();
        matched
    });
    (matched && checker.diagnostics.is_empty())
        .then(|| checker.typing.take_function_signature_evidence())
}

fn signal_name_payload_type<'a>(
    module: &Module,
    expr_id: ExprId,
    actual: &'a GateType,
) -> Option<&'a GateType> {
    matches!(module.exprs()[expr_id].kind, ExprKind::Name(_))
        .then_some(actual)
        .and_then(|actual| match actual {
            GateType::Signal(payload) => Some(payload.as_ref()),
            _ => None,
        })
}

fn signal_annotation_payload(annotation: Option<&GateType>) -> Option<&GateType> {
    match annotation {
        Some(GateType::Signal(payload)) => Some(payload.as_ref()),
        _ => None,
    }
}

pub fn signal_payload_type(module: &Module, item: &SignalItem) -> Option<GateType> {
    let mut typing = GateTypeContext::new(module);
    let expected = item
        .annotation
        .and_then(|annotation| typing.lower_annotation(annotation));
    signal_annotation_payload(expected.as_ref())
        .cloned()
        .or_else(|| {
            item.body
                .and_then(|body| typing.infer_expr(body, &GateExprEnv::default(), None).ty)
        })
}

fn evidence_scope_constraints(env: &GateExprEnv) -> Vec<ClassConstraintBinding> {
    let mut constraints = Vec::new();
    for evidence in &env.class_evidence {
        let binding = ClassConstraintBinding {
            class_item: evidence.member.class,
            subject: evidence.subject.clone(),
        };
        if !constraints.contains(&binding) {
            constraints.push(binding);
        }
    }
    constraints
}

#[cfg(test)]
pub(crate) fn resolve_class_member_dispatch(
    module: &Module,
    reference: &TermReference,
    argument_types: &[GateType],
    expected_result: Option<&GateType>,
) -> Option<ResolvedClassMemberDispatch> {
    resolve_class_member_dispatch_in_scope(
        module,
        reference,
        argument_types,
        expected_result,
        &GateExprEnv::default(),
    )
}

pub(crate) fn resolve_class_member_dispatch_in_scope(
    module: &Module,
    reference: &TermReference,
    argument_types: &[GateType],
    expected_result: Option<&GateType>,
    env: &GateExprEnv,
) -> Option<ResolvedClassMemberDispatch> {
    let mut checker = TypeChecker::new(module);
    checker.with_class_constraint_scope(evidence_scope_constraints(env), |checker| {
        match checker
            .typing
            .select_class_member_call(reference, argument_types, expected_result)?
        {
            DomainMemberSelection::Unique(matched) => checker
                .solve_class_constraint_bindings(
                    reference.span(),
                    &matched.evidence,
                    &matched.constraints,
                )
                .ok()
                .and_then(|()| checker.class_member_dispatch(&matched)),
            DomainMemberSelection::Ambiguous | DomainMemberSelection::NoMatch => None,
        }
    })
}

pub(crate) fn resolve_class_member_dispatch_for_binding(
    module: &Module,
    member: ClassMemberResolution,
    subject: &TypeBinding,
    env: &GateExprEnv,
) -> Option<ResolvedClassMemberDispatch> {
    let mut checker = TypeChecker::new(module);
    checker.with_class_constraint_scope(evidence_scope_constraints(env), |checker| {
        checker
            .require_class_binding(&ClassConstraintBinding {
                class_item: member.class,
                subject: subject.clone(),
            })
            .ok()?;
        let implementation = checker.class_member_implementation(member, subject)?;
        Some(ResolvedClassMemberDispatch {
            member,
            subject: subject.clone(),
            implementation,
        })
    })
}

fn resolve_named_class_member_dispatch_in_scope(
    module: &Module,
    subject: &GateType,
    class_name: &str,
    member_name: &str,
    env: &GateExprEnv,
) -> Option<ResolvedClassMemberDispatch> {
    let checker = TypeChecker::new(module);
    let class = checker.class_item_id_by_name(class_name)?;
    let Item::Class(class_item) = &module.items()[class] else {
        return None;
    };
    let member_index = class_item
        .members
        .iter()
        .position(|member| member.name.text() == member_name)?;
    let member = ClassMemberResolution {
        class,
        member_index,
    };
    let subject = checker
        .typing
        .class_member_subject_binding(member, subject)?;
    resolve_class_member_dispatch_for_binding(module, member, &subject, env)
}

pub(crate) fn resolve_equality_dispatch_in_scope(
    module: &Module,
    subject: &GateType,
    env: &GateExprEnv,
) -> Option<ResolvedClassMemberDispatch> {
    for (class_name, member_name) in [("Eq", "=="), ("Setoid", "equals")] {
        if let Some(dispatch) = resolve_named_class_member_dispatch_in_scope(
            module,
            subject,
            class_name,
            member_name,
            env,
        ) {
            return Some(dispatch);
        }
    }
    None
}

pub(crate) fn resolve_ordering_dispatch(
    module: &Module,
    subject: &GateType,
) -> Option<ResolvedClassMemberDispatch> {
    resolve_ordering_dispatch_in_scope(module, subject, &GateExprEnv::default())
}

pub(crate) fn resolve_ordering_dispatch_in_scope(
    module: &Module,
    subject: &GateType,
    env: &GateExprEnv,
) -> Option<ResolvedClassMemberDispatch> {
    resolve_named_class_member_dispatch_in_scope(module, subject, "Ord", "compare", env)
}
