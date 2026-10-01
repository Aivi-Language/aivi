/// Comparisons require an executable member with two matching operands and the
/// canonical result type. The class name is not a capability witness.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ComparisonKind {
    Equality,
    Ordering,
}

#[derive(Clone, Debug)]
enum ComparisonError {
    Missing(String),
    Ambiguous {
        member: &'static str,
        subject: GateType,
        candidates: Vec<String>,
    },
}

impl From<String> for ComparisonError {
    fn from(reason: String) -> Self {
        Self::Missing(reason)
    }
}

impl std::fmt::Display for ComparisonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing(reason) => f.write_str(reason),
            Self::Ambiguous {
                member,
                subject,
                candidates,
            } => write!(
                f,
                "comparison member `{member}` is ambiguous for `{subject}`; candidates: {}",
                candidates.join(", ")
            ),
        }
    }
}

impl ComparisonKind {
    fn member_names(self) -> &'static [&'static str] {
        match self {
            Self::Equality => &["==", "equals"],
            Self::Ordering => &["compare"],
        }
    }

    fn result_type(self, module: &Module) -> Option<GateType> {
        match self {
            Self::Equality => Some(GateType::Primitive(BuiltinType::Bool)),
            Self::Ordering => module.ambient_items().iter().find_map(|id| {
                matches!(&module.items()[*id], Item::Type(item) if item.name.text() == "Ordering")
                    .then(|| GateType::OpaqueItem {
                        item: *id,
                        name: "Ordering".to_owned(),
                        arguments: Vec::new(),
                    })
            }),
        }
    }
}

pub(crate) fn resolve_comparison_member_in_scope(
    module: &Module,
    kind: ComparisonKind,
    subject: &GateType,
    env: &GateExprEnv,
) -> Option<ClassMemberCallMatch> {
    let mut checker = TypeChecker::new(module);
    checker.with_class_constraint_scope(evidence_scope_constraints(env), |checker| {
        checker
            .select_comparison_member(kind, subject, &mut Vec::new())
            .ok()
    })
}

impl TypeChecker<'_> {
    fn emit_comparison_error(
        &mut self,
        span: SourceSpan,
        kind: ComparisonKind,
        subject: &GateType,
        error: &ComparisonError,
    ) {
        let diagnostic = match error {
            ComparisonError::Ambiguous { .. } => Diagnostic::error(
                "comparison evidence is ambiguous",
            )
            .with_code(code("ambiguous-class-member"))
            .with_primary_label(
                span,
                "more than one matching comparison dictionary is available",
            )
            .with_help(
                "use the desired class member explicitly or narrow this function's constraints",
            ),
            ComparisonError::Missing(_) => match kind {
                ComparisonKind::Equality => {
                    Diagnostic::error(format!("this expression requires `Eq` for `{subject}`"))
                        .with_code(code("missing-eq-instance"))
                        .with_primary_label(
                            span,
                            format!(
                                "`{subject}` does not currently have matching equality evidence"
                            ),
                        )
                }
                ComparisonKind::Ordering => Diagnostic::error(format!(
                    "this comparison requires an executable `compare` member for `{subject}`"
                ))
                .with_code(code("invalid-binary-operator"))
                .with_primary_label(
                    span,
                    format!("expected `{subject} -> {subject} -> Ordering` evidence"),
                ),
            },
        };
        self.diagnostics
            .push(diagnostic.with_note(error.to_string()));
    }

    fn match_comparison_member(
        &mut self,
        kind: ComparisonKind,
        member: ClassMemberResolution,
        subject: &GateType,
    ) -> Option<ClassMemberCallMatch> {
        let result = kind.result_type(self.module)?;
        self.typing.match_class_member_call_candidate_with_hints(
            member,
            [Some(subject), Some(subject)].into_iter(),
            Some(&result),
        )
    }

    fn prove_comparison_member(
        &mut self,
        kind: ComparisonKind,
        matched: &ClassMemberCallMatch,
        item_stack: &mut Vec<ItemId>,
    ) -> Result<(), ComparisonError> {
        let standard_equality = kind == ComparisonKind::Equality
            && matches!(&self.module.items()[matched.resolution.class], Item::Class(class) if matches!(&class.identity, crate::ClassIdentity::Standard(name) if matches!(name.as_ref(), "Eq" | "Setoid")));
        if standard_equality
            && !self.in_scope_class_constraints.contains(&matched.evidence)
            && self
                .resolve_same_module_instance_binding_with_id(
                    matched.evidence.class_item,
                    &matched.evidence.subject,
                )?
                .is_none()
            && self
                .resolve_imported_instance_binding(
                    matched.evidence.class_item,
                    &matched.evidence.subject,
                )?
                .is_none()
            && let TypeBinding::Type(subject) = &matched.evidence.subject
        {
            // Preserve the same recursive-type stack through structural proof.
            // Starting another class search here would lose its cycle boundary.
            let scope = self.current_eq_constraint_scope();
            self.require_compiler_derived_eq_with_scope(subject, &scope, item_stack)?;
            for requirement in &matched.constraints {
                self.require_class_binding(requirement)?;
            }
            return Ok(());
        }
        self.require_class_binding(&matched.evidence)?;
        for requirement in &matched.constraints {
            self.require_class_binding(requirement)?;
        }
        Ok(())
    }

    fn select_comparison_member(
        &mut self,
        kind: ComparisonKind,
        subject: &GateType,
        item_stack: &mut Vec<ItemId>,
    ) -> Result<ClassMemberCallMatch, ComparisonError> {
        // A dictionary already in scope carries its original lexical identity,
        // including the compiler-owned helpers checked under local shadowing.
        let scoped_classes = self
            .in_scope_class_constraints
            .iter()
            .map(|binding| binding.class_item)
            .fold(Vec::new(), |mut classes, id| {
                if !classes.contains(&id) {
                    classes.push(id);
                }
                classes
            });
        for name in kind.member_names() {
            let candidates = scoped_classes
                .iter()
                .flat_map(|id| {
                    let Item::Class(class) = &self.module.items()[*id] else {
                        return Vec::new();
                    };
                    class
                        .members
                        .iter()
                        .enumerate()
                        .filter_map(|(member_index, member)| {
                            (member.name.text() == *name).then_some(ClassMemberResolution {
                                class: *id,
                                member_index,
                            })
                        })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            let mut matches = Vec::new();
            let mut failure = None;
            for candidate in candidates {
                if let Some(matched) = self.match_comparison_member(kind, candidate, subject)
                    && self.in_scope_class_constraints.contains(&matched.evidence)
                {
                    match self.prove_comparison_member(kind, &matched, item_stack) {
                        Ok(()) => matches.push(matched),
                        Err(reason) => failure = Some(reason),
                    }
                }
            }
            match matches.len() {
                0 if failure.is_some() => return Err(failure.expect("failed scoped comparison")),
                0 => {}
                1 => return Ok(matches.pop().expect("one scoped comparison")),
                _ => {
                    return Err(self.ambiguous_comparison(name, subject, &matches));
                }
            }
        }
        let mut reasons = Vec::new();
        for name in kind.member_names() {
            let mut matches = Vec::new();
            let mut selected_instance_failure = None;
            let candidates = self.module.class_members_in_scope(name);
            for candidate in candidates {
                let Some(matched) = self.match_comparison_member(kind, candidate, subject) else {
                    continue;
                };
                match self.prove_comparison_member(kind, &matched, item_stack) {
                    Ok(()) => matches.push(matched),
                    Err(reason) => {
                        if self
                            .resolve_same_module_instance_binding_with_id(
                                matched.evidence.class_item,
                                &matched.evidence.subject,
                            )?
                            .is_some()
                            || self
                                .resolve_imported_instance_binding(
                                    matched.evidence.class_item,
                                    &matched.evidence.subject,
                                )?
                                .is_some()
                        {
                            selected_instance_failure = Some(reason.clone());
                        }
                        reasons.push(reason);
                    }
                }
            }
            match matches.len() {
                0 if selected_instance_failure.is_some() => {
                    return Err(selected_instance_failure.expect("failed instance comparison"));
                }
                0 => {}
                1 => return Ok(matches.pop().expect("one concrete comparison")),
                _ => {
                    return Err(self.ambiguous_comparison(name, subject, &matches));
                }
            }
        }
        Err(ComparisonError::Missing(format!(
            "no executable {} member matches `{subject} -> {subject} -> {}`{}",
            kind.member_names().join(" / "),
            kind.result_type(self.module)
                .map_or_else(|| "<unresolved result>".to_owned(), |ty| ty.to_string()),
            reasons
                .first()
                .map_or_else(String::new, |reason| format!(": {reason}"))
        )))
    }

    fn ambiguous_comparison(
        &self,
        member: &'static str,
        subject: &GateType,
        matches: &[ClassMemberCallMatch],
    ) -> ComparisonError {
        let candidates = matches
            .iter()
            .filter_map(|matched| {
                let Item::Class(class) = &self.module.items()[matched.resolution.class] else {
                    return None;
                };
                let local = self
                    .module
                    .imports()
                    .iter()
                    .find_map(|(_, import)| match &import.metadata {
                        ImportBindingMetadata::Class { identity }
                            if identity == &class.identity =>
                        {
                            Some(import.local_name.text())
                        }
                        _ => None,
                    })
                    .unwrap_or_else(|| class.name.text());
                Some(format!("{local}.{member}"))
            })
            .collect();
        ComparisonError::Ambiguous {
            member,
            subject: subject.clone(),
            candidates,
        }
    }
}
