fn code(name: &'static str) -> DiagnosticCode {
    DiagnosticCode::new("hir", name)
}

fn is_numeric_gate_type(ty: &GateType) -> bool {
    matches!(
        ty,
        GateType::Primitive(
            BuiltinType::Int | BuiltinType::Float | BuiltinType::Decimal | BuiltinType::BigInt
        )
    )
}

fn unary_operator_text(operator: UnaryOperator) -> &'static str {
    match operator {
        UnaryOperator::Not => "not",
    }
}

fn binary_operator_text(operator: BinaryOperator) -> &'static str {
    match operator {
        BinaryOperator::Add => "+",
        BinaryOperator::Subtract => "-",
        BinaryOperator::Multiply => "*",
        BinaryOperator::Divide => "/",
        BinaryOperator::Modulo => "%",
        BinaryOperator::GreaterThan => ">",
        BinaryOperator::LessThan => "<",
        BinaryOperator::GreaterThanOrEqual => ">=",
        BinaryOperator::LessThanOrEqual => "<=",
        BinaryOperator::Equals => "==",
        BinaryOperator::NotEquals => "!=",
        BinaryOperator::And => "and",
        BinaryOperator::Or => "or",
    }
}

fn describe_inferred_type(ty: Option<&GateType>) -> String {
    ty.map(|ty| format!("`{ty}`"))
        .unwrap_or_else(|| "an unresolved expression".to_owned())
}

fn patch_map_entry_type(key: &GateType, value: &GateType) -> GateType {
    GateType::Record(vec![
        GateRecordField {
            name: "key".to_owned(),
            ty: key.clone(),
        },
        GateRecordField {
            name: "value".to_owned(),
            ty: value.clone(),
        },
    ])
}

// KNOWN ISSUE: This function mutates the module (by synthesizing and injecting new record
// fields) during the type-checking phase. Because it alters the structure of the module
// that was passed into the type checker, running type checking a second time on the
// elaborated module will observe different record expressions than the first run, making
// type checking non-idempotent. The elaboration of default record fields should be moved
// to a separate, explicit elaboration pass that runs after type checking completes, so
// that the type checker itself remains a pure read-only query over the module.
fn apply_default_record_elisions(module: &Module, elisions: &[DefaultRecordElision]) -> Module {
    if elisions.is_empty() {
        return module.clone();
    }

    let mut module = module.clone();
    for elision in elisions {
        let record_span = module.exprs()[elision.record_expr].span;
        let synthesized_fields = elision
            .fields
            .iter()
            .map(|field| synthesize_default_record_field(&mut module, record_span, field))
            .collect::<Vec<_>>();
        let Some(expr) = module.arenas.exprs.get_mut(elision.record_expr) else {
            continue;
        };
        let ExprKind::Record(record) = &mut expr.kind else {
            continue;
        };
        record.fields.extend(synthesized_fields);
    }
    module
}

fn synthesize_default_record_field(
    module: &mut Module,
    record_span: SourceSpan,
    field: &SolvedDefaultRecordField,
) -> RecordExprField {
    let label = Name::new(field.field_name.clone(), record_span)
        .expect("typechecked record field names must stay valid");
    let value = match field.evidence {
        DefaultEvidence::BuiltinOptionNone => {
            alloc_builtin_default_expr(module, record_span, BuiltinTerm::None, "None")
        }
        DefaultEvidence::ImportedBinding(import) => {
            alloc_import_default_expr(module, record_span, import)
        }
        DefaultEvidence::SameModuleMemberBody(body) => body,
    };
    RecordExprField {
        span: record_span,
        label,
        value,
        surface: RecordFieldSurface::Defaulted,
    }
}

fn alloc_builtin_default_expr(
    module: &mut Module,
    span: SourceSpan,
    builtin: BuiltinTerm,
    text: &str,
) -> ExprId {
    let path = NamePath::from_vec(vec![
        Name::new(text, span).expect("builtin default term name must stay valid"),
    ])
    .expect("builtin default term path must stay valid");
    module
        .alloc_expr(crate::Expr {
            span,
            kind: ExprKind::Name(TermReference::resolved(
                path,
                TermResolution::Builtin(builtin),
            )),
        })
        .expect("default-record elaboration should fit inside the expression arena")
}

fn alloc_import_default_expr(module: &mut Module, span: SourceSpan, import: ImportId) -> ExprId {
    let local_name = module.imports()[import].local_name.text().to_owned();
    let path = NamePath::from_vec(vec![
        Name::new(local_name, span).expect("default import local name must stay valid"),
    ])
    .expect("default import path must stay valid");
    module
        .alloc_expr(crate::Expr {
            span,
            kind: ExprKind::Name(TermReference::resolved(
                path,
                TermResolution::Import(import),
            )),
        })
        .expect("default-record elaboration should fit inside the expression arena")
}
