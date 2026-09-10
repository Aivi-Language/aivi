impl<'a> RuntimeFragmentLowerer<'a> {
    fn new(hir: &'a aivi_hir::Module, fragment: &'a RuntimeFragmentSpec) -> Self {
        let report = elaborate_general_expressions(hir);
        let completeness_errors = validate_general_expr_report_completeness(hir, &report, |_| true);
        let (items, domain_members, instance_members) = report.into_parts();
        let mut report_by_owner: HashMap<HirItemId, _> =
            items.into_iter().map(|item| (item.owner, item)).collect();
        // Ambient prelude items are elaborated separately; merge their results so
        // runtime fragments that reference ambient functions can lower them.
        let ambient_report = elaborate_ambient_items(hir);
        let (ambient_items, ambient_domain_members, ambient_instance_members) = ambient_report.into_parts();
        for item in ambient_items {
            report_by_owner.entry(item.owner).or_insert(item);
        }
        let mut domain_member_reports: HashMap<_, _> = domain_members
            .into_iter()
            .map(|item| {
                (
                    DomainMemberKey {
                        domain: item.domain_owner,
                        member_index: item.member_index,
                    },
                    item,
                )
            })
            .collect();
        for member in ambient_domain_members {
            domain_member_reports
                .entry(DomainMemberKey {
                    domain: member.domain_owner,
                    member_index: member.member_index,
                })
                .or_insert(member);
        }
        let instance_member_reports = instance_members
            .into_iter()
            .chain(ambient_instance_members)
            .map(|item| {
                (
                    InstanceMemberKey {
                        instance: item.instance_owner,
                        member_index: item.member_index,
                    },
                    item,
                )
            })
            .collect();
        let mut lowerer = Self {
            lowerer: ModuleLowerer::new(hir),
            fragment,
            report_by_owner,
            domain_member_reports,
            instance_member_reports,
            lowering: HashSet::new(),
            lowered: HashSet::new(),
            lowering_domain_members: HashSet::new(),
            lowered_domain_members: HashSet::new(),
            lowering_instance_members: HashSet::new(),
            lowered_instance_members: HashSet::new(),
        };
        lowerer.lowerer.errors.extend(completeness_errors);
        lowerer
    }

    fn new_with_workspace(
        hir: &'a aivi_hir::Module,
        workspace_hirs: &[(&str, &'a aivi_hir::Module)],
        fragment: &'a RuntimeFragmentSpec,
    ) -> Result<Self, LoweringErrors> {
        let mut lowerer = Self::new(hir, fragment);
        lowerer.lowerer.ws_origin_base = lowerer.lowerer.next_synthetic_item_origin_raw;
        lowerer.lowerer.next_synthetic_item_origin_raw = workspace_origin_ranges(hir, workspace_hirs)?.1;
        let roots = runtime_fragment_included_items(hir, fragment);
        let selected = select_workspace_items(hir, workspace_hirs, &roots);
        for ((name, ws_hir), items) in workspace_hirs.iter().zip(&selected[1..]) {
            lowerer.lowerer.compile_workspace_module(name, ws_hir, Some(items))?;
        }
        lowerer.lowerer.register_imported_type_origins();
        Ok(lowerer)
    }

    fn build(mut self) -> Result<LoweredRuntimeFragment, LoweringErrors> {
        // Guard: reject incomplete elaboration before walking dependencies.
        let has_completeness_errors = self.lowerer.errors.iter().any(|e| {
            matches!(
                e,
                LoweringError::MissingGeneralExprElaboration { .. }
                    | LoweringError::MissingDomainMemberElaboration { .. }
                    | LoweringError::MissingInstanceMemberElaboration { .. }
            )
        });
        if has_completeness_errors {
            return Err(LoweringErrors::new(self.lowerer.errors));
        }
        let dependencies = referenced_hir_dependencies(&self.fragment.body);
        for dependency in dependencies.items {
            self.ensure_hir_item_lowered(dependency);
        }
        for dependency in dependencies.domain_members {
            self.ensure_domain_member_lowered(dependency);
        }
        for dependency in dependencies.instance_members {
            self.ensure_instance_member_lowered(dependency);
        }

        let fragment_item = self
            .lowerer
            .module
            .items_mut()
            .alloc(Item {
                origin: self.fragment.owner,
                span: self.lowerer.hir.exprs()[self.fragment.body_expr].span,
                name: self.fragment.name.clone(),
                kind: if self.fragment.parameters.is_empty() {
                    ItemKind::Value
                } else {
                    ItemKind::Function
                },
                parameters: self
                    .fragment
                    .parameters
                    .iter()
                    .map(|parameter| ItemParameter {
                        binding: parameter.binding,
                        span: parameter.span,
                        name: parameter.name.clone(),
                        ty: Type::lower(&parameter.ty),
                    })
                    .collect(),
                body: None,
                pipes: Vec::new(),
            })
            .map_err(|overflow| LoweringErrors::new(vec![arena_overflow("items", overflow)]))?;

        match self
            .lowerer
            .lower_runtime_expr(self.fragment.owner, &self.fragment.body)
        {
            Ok(body) => {
                let item = self
                    .lowerer
                    .module
                    .items_mut()
                    .get_mut(fragment_item)
                    .expect("freshly allocated runtime fragment item should exist");
                item.body = Some(body);
            }
            Err(error) => self.lowerer.errors.push(error),
        }

        if !self.lowerer.errors.is_empty() {
            return Err(LoweringErrors::new(self.lowerer.errors));
        }
        if let Err(validation) = validate_module(&self.lowerer.module) {
            self.lowerer.errors.extend(
                validation
                    .into_errors()
                    .into_iter()
                    .map(|error| LoweringError::Validation(Box::new(error))),
            );
            return Err(LoweringErrors::new(self.lowerer.errors));
        }

        Ok(LoweredRuntimeFragment {
            entry_name: self.fragment.name.clone(),
            module: self.lowerer.module,
        })
    }

    fn ensure_hir_item_lowered(&mut self, owner: HirItemId) {
        if self.lowered.contains(&owner) || self.lowering.contains(&owner) {
            return;
        }
        match self.lowerer.hir.items().get(owner) {
            Some(HirItem::Signal(_)) => {
                if self.seed_hir_item(owner).is_some() {
                    self.lowered.insert(owner);
                }
                return;
            }
            // Domain/Type/Class/Use/Export/SourceProviderContract items do not produce
            // core-level items — only their members do (handled via
            // ensure_domain_member_lowered / ensure_instance_member_lowered). Silently
            // skip them so transitive dependency walks don't trigger UnknownOwner.
            Some(
                HirItem::Domain(_)
                | HirItem::Type(_)
                | HirItem::Class(_)
                | HirItem::SourceProviderContract(_)
                | HirItem::Use(_)
                | HirItem::Export(_),
            )
            | None => return,
            _ => {}
        }
        let Some(report) = self.report_by_owner.get(&owner).cloned() else {
            let is_ambient = self.lowerer.hir.ambient_items().contains(&owner);
            if is_ambient {
                // Ambient items are elaborated separately; just seed them without a body.
                self.seed_hir_item(owner);
                return;
            }
            self.lowerer
                .errors
                .push(LoweringError::UnknownOwner { owner });
            return;
        };
        let Some(core_item) = self.seed_hir_item(owner) else {
            return;
        };
        let body = match report.outcome {
            GeneralExprOutcome::Lowered(body) => body,
            GeneralExprOutcome::Blocked(blocked) => {
                self.lowerer.errors.push(LoweringError::BlockedGeneralExpr {
                    owner,
                    body_expr: report.body_expr,
                    span: blocked.primary_span().unwrap_or_default(),
                    blocked: Box::new(blocked),
                });
                return;
            }
        };

        self.lowering.insert(owner);
        let dependencies = referenced_hir_dependencies(&body);
        for dependency in dependencies.items {
            self.ensure_hir_item_lowered(dependency);
        }
        for dependency in dependencies.domain_members {
            self.ensure_domain_member_lowered(dependency);
        }
        for dependency in dependencies.instance_members {
            self.ensure_instance_member_lowered(dependency);
        }
        if self.lowerer.errors.is_empty() {
            match self.lowerer.lower_runtime_expr(owner, &body) {
                Ok(lowered_body) => {
                    let item = self
                        .lowerer
                        .module
                        .items_mut()
                        .get_mut(core_item)
                        .expect("seeded runtime dependency item should exist");
                    item.parameters = report
                        .parameters
                        .iter()
                        .map(|parameter| ItemParameter {
                            binding: parameter.binding,
                            span: parameter.span,
                            name: parameter.name.clone(),
                            ty: Type::lower(&parameter.ty),
                        })
                        .collect();
                    item.body = Some(lowered_body);
                }
                Err(error) => self.lowerer.errors.push(error),
            }
        }
        self.lowering.remove(&owner);
        self.lowered.insert(owner);
    }

    fn ensure_domain_member_lowered(&mut self, key: DomainMemberKey) {
        if self.lowered_domain_members.contains(&key) || self.lowering_domain_members.contains(&key)
        {
            return;
        }
        let Some(report) = self.domain_member_reports.get(&key).cloned() else {
            return;
        };
        let Some(core_item) = self
            .lowerer
            .seed_domain_member_item(key.domain, key.member_index)
        else {
            return;
        };
        let body = match report.outcome {
            GeneralExprOutcome::Lowered(body) => body,
            GeneralExprOutcome::Blocked(blocked) => {
                self.lowerer.errors.push(LoweringError::BlockedGeneralExpr {
                    owner: key.domain,
                    body_expr: report.body_expr,
                    span: blocked.primary_span().unwrap_or_default(),
                    blocked: Box::new(blocked),
                });
                return;
            }
        };

        self.lowering_domain_members.insert(key);
        let dependencies = referenced_hir_dependencies(&body);
        for dependency in dependencies.items {
            self.ensure_hir_item_lowered(dependency);
        }
        for dependency in dependencies.domain_members {
            self.ensure_domain_member_lowered(dependency);
        }
        for dependency in dependencies.instance_members {
            self.ensure_instance_member_lowered(dependency);
        }
        if self.lowerer.errors.is_empty() {
            match self.lowerer.lower_runtime_expr(key.domain, &body) {
                Ok(lowered_body) => {
                    let item = self
                        .lowerer
                        .module
                        .items_mut()
                        .get_mut(core_item)
                        .expect("seeded runtime dependency item should exist");
                    item.parameters = report
                        .parameters
                        .iter()
                        .map(|parameter| ItemParameter {
                            binding: parameter.binding,
                            span: parameter.span,
                            name: parameter.name.clone(),
                            ty: Type::lower(&parameter.ty),
                        })
                        .collect();
                    item.body = Some(lowered_body);
                }
                Err(error) => self.lowerer.errors.push(error),
            }
        }
        self.lowering_domain_members.remove(&key);
        self.lowered_domain_members.insert(key);
    }

    fn ensure_instance_member_lowered(&mut self, key: InstanceMemberKey) {
        if self.lowered_instance_members.contains(&key)
            || self.lowering_instance_members.contains(&key)
        {
            return;
        }
        let Some(report) = self.instance_member_reports.get(&key).cloned() else {
            self.lowerer.errors.push(LoweringError::UnknownOwner {
                owner: key.instance,
            });
            return;
        };
        let Some(core_item) = self
            .lowerer
            .seed_instance_member_item(key.instance, key.member_index)
        else {
            return;
        };
        let body = match report.outcome {
            GeneralExprOutcome::Lowered(body) => body,
            GeneralExprOutcome::Blocked(blocked) => {
                self.lowerer.errors.push(LoweringError::BlockedGeneralExpr {
                    owner: key.instance,
                    body_expr: report.body_expr,
                    span: blocked.primary_span().unwrap_or_default(),
                    blocked: Box::new(blocked),
                });
                return;
            }
        };

        self.lowering_instance_members.insert(key);
        let dependencies = referenced_hir_dependencies(&body);
        for dependency in dependencies.items {
            self.ensure_hir_item_lowered(dependency);
        }
        for dependency in dependencies.domain_members {
            self.ensure_domain_member_lowered(dependency);
        }
        for dependency in dependencies.instance_members {
            self.ensure_instance_member_lowered(dependency);
        }
        if self.lowerer.errors.is_empty() {
            match self.lowerer.lower_runtime_expr(key.instance, &body) {
                Ok(lowered_body) => {
                    let item = self
                        .lowerer
                        .module
                        .items_mut()
                        .get_mut(core_item)
                        .expect("seeded runtime dependency item should exist");
                    item.parameters = report
                        .parameters
                        .iter()
                        .map(|parameter| ItemParameter {
                            binding: parameter.binding,
                            span: parameter.span,
                            name: parameter.name.clone(),
                            ty: Type::lower(&parameter.ty),
                        })
                        .collect();
                    item.body = Some(lowered_body);
                }
                Err(error) => self.lowerer.errors.push(error),
            }
        }
        self.lowering_instance_members.remove(&key);
        self.lowered_instance_members.insert(key);
    }

    fn seed_hir_item(&mut self, owner: HirItemId) -> Option<ItemId> {
        if let Some(item) = self.lowerer.item_map.get(&owner).copied() {
            return Some(item);
        }
        let item = self.lowerer.hir.items().get(owner)?;
        let (span, name, kind) = match item {
            HirItem::Value(item) => (item.header.span, item.name.text().into(), ItemKind::Value),
            HirItem::Function(item) => (
                item.header.span,
                item.name.text().into(),
                ItemKind::Function,
            ),
            HirItem::Signal(item) => (
                item.header.span,
                item.name.text().into(),
                ItemKind::Signal(SignalInfo::default()),
            ),
            HirItem::Instance(item) => (
                item.header.span,
                format!("instance#{}", owner.as_raw()).into_boxed_str(),
                ItemKind::Instance,
            ),
            // These item types do not produce core-level items.
            HirItem::Type(_)
            | HirItem::Class(_)
            | HirItem::Domain(_)
            | HirItem::SourceProviderContract(_)
            | HirItem::Use(_)
            | HirItem::Export(_)
            | HirItem::Hoist(_) => return None,
        };
        let item_id = match self.lowerer.module.items_mut().alloc(Item {
            origin: owner,
            span,
            name,
            kind,
            parameters: Vec::new(),
            body: None,
            pipes: Vec::new(),
        }) {
            Ok(item_id) => item_id,
            Err(overflow) => {
                self.lowerer.errors.push(arena_overflow("items", overflow));
                return None;
            }
        };
        self.lowerer.item_map.insert(owner, item_id);
        Some(item_id)
    }
}

impl<'a> RuntimeFragmentItemCollector<'a> {
    fn new(hir: &'a aivi_hir::Module, fragment: &'a RuntimeFragmentSpec) -> Self {
        let (items, domain_members, instance_members) =
            elaborate_general_expressions(hir).into_parts();
        let mut report_by_owner: HashMap<HirItemId, _> =
            items.into_iter().map(|item| (item.owner, item)).collect();
        // Include ambient prelude items so fragment dependency collection can
        // transitively walk through ambient function bodies.
        let (ambient_items, ambient_domain_members, ambient_instance_members) = elaborate_ambient_items(hir).into_parts();
        for item in ambient_items {
            report_by_owner.entry(item.owner).or_insert(item);
        }
        let mut domain_member_reports: HashMap<_, _> = domain_members
            .into_iter()
            .map(|item| {
                (
                    DomainMemberKey {
                        domain: item.domain_owner,
                        member_index: item.member_index,
                    },
                    item,
                )
            })
            .collect();
        for member in ambient_domain_members {
            domain_member_reports
                .entry(DomainMemberKey {
                    domain: member.domain_owner,
                    member_index: member.member_index,
                })
                .or_insert(member);
        }
        let instance_member_reports = instance_members
            .into_iter()
            .chain(ambient_instance_members)
            .map(|item| {
                (
                    InstanceMemberKey {
                        instance: item.instance_owner,
                        member_index: item.member_index,
                    },
                    item,
                )
            })
            .collect();
        Self {
            hir,
            fragment,
            report_by_owner,
            domain_member_reports,
            instance_member_reports,
            included_items: HashSet::new(),
            visited_domain_members: HashSet::new(),
            visited_instance_members: HashSet::new(),
        }
    }

    fn collect(mut self) -> HashSet<HirItemId> {
        self.collect_item(self.fragment.owner);
        self.collect_mock_replacements(self.fragment.owner);
        let dependencies = referenced_hir_dependencies(&self.fragment.body);
        for dependency in dependencies.items {
            self.collect_item(dependency);
        }
        for dependency in dependencies.domain_members {
            self.collect_domain_member(dependency);
        }
        for dependency in dependencies.instance_members {
            self.collect_instance_member(dependency);
        }
        self.included_items
    }

    fn collect_item(&mut self, owner: HirItemId) {
        if !self.included_items.insert(owner) {
            return;
        }
        let Some(item) = self.hir.items().get(owner) else {
            return;
        };
        self.collect_mock_replacements(owner);
        match item {
            HirItem::Signal(signal) => {
                for dependency in &signal.signal_dependencies {
                    self.collect_item(*dependency);
                }
                if let Some(source_metadata) = &signal.source_metadata {
                    for dependency in &source_metadata.signal_dependencies {
                        self.collect_item(*dependency);
                    }
                    for dependency in source_metadata.lifecycle_dependencies.merged() {
                        self.collect_item(dependency);
                    }
                }
            }
            HirItem::Instance(instance) => {
                for member_index in 0..instance.members.len() {
                    self.collect_instance_member(InstanceMemberKey {
                        instance: owner,
                        member_index,
                    });
                }
            }
            HirItem::Value(_) | HirItem::Function(_) => {
                let Some(report) = self.report_by_owner.get(&owner) else {
                    return;
                };
                let GeneralExprOutcome::Lowered(body) = &report.outcome else {
                    return;
                };
                let dependencies = referenced_hir_dependencies(body);
                for dependency in dependencies.items {
                    self.collect_item(dependency);
                }
                for dependency in dependencies.domain_members {
                    self.collect_domain_member(dependency);
                }
                for dependency in dependencies.instance_members {
                    self.collect_instance_member(dependency);
                }
            }
            HirItem::Type(_)
            | HirItem::Class(_)
            | HirItem::SourceProviderContract(_)
            | HirItem::Use(_)
            | HirItem::Export(_)
            | HirItem::Hoist(_) => {}
            HirItem::Domain(domain) => {
                for (member_index, member) in domain.members.iter().enumerate() {
                    if member.body.is_none() {
                        continue;
                    }
                    self.collect_domain_member(DomainMemberKey {
                        domain: owner,
                        member_index,
                    });
                }
            }
        }
    }

    fn collect_domain_member(&mut self, key: DomainMemberKey) {
        if !self.visited_domain_members.insert(key) {
            return;
        }
        self.collect_item(key.domain);
        let Some(report) = self.domain_member_reports.get(&key) else {
            return;
        };
        let GeneralExprOutcome::Lowered(body) = &report.outcome else {
            return;
        };
        let dependencies = referenced_hir_dependencies(body);
        for dependency in dependencies.items {
            self.collect_item(dependency);
        }
        for dependency in dependencies.domain_members {
            self.collect_domain_member(dependency);
        }
        for dependency in dependencies.instance_members {
            self.collect_instance_member(dependency);
        }
    }

    fn collect_instance_member(&mut self, key: InstanceMemberKey) {
        if !self.visited_instance_members.insert(key) {
            return;
        }
        self.collect_item(key.instance);
        let Some(report) = self.instance_member_reports.get(&key) else {
            return;
        };
        let GeneralExprOutcome::Lowered(body) = &report.outcome else {
            return;
        };
        let dependencies = referenced_hir_dependencies(body);
        for dependency in dependencies.items {
            self.collect_item(dependency);
        }
        for dependency in dependencies.domain_members {
            self.collect_domain_member(dependency);
        }
        for dependency in dependencies.instance_members {
            self.collect_instance_member(dependency);
        }
    }

    fn collect_mock_replacements(&mut self, owner: HirItemId) {
        let Some(item) = self.hir.items().get(owner) else {
            return;
        };
        for decorator_id in item.decorators() {
            let Some(decorator) = self.hir.decorators().get(*decorator_id) else {
                continue;
            };
            let DecoratorPayload::Mock(mock) = &decorator.payload else {
                continue;
            };
            let Some(MockImportTarget::Item(item_id)) =
                mock_replacement_target(self.hir, mock.replacement)
            else {
                continue;
            };
            self.collect_item(item_id);
        }
    }
}

#[derive(Default)]
struct HirDependencies {
    imports: Vec<ImportId>,
    items: Vec<HirItemId>,
    domain_members: Vec<DomainMemberKey>,
    instance_members: Vec<InstanceMemberKey>,
}

fn referenced_hir_dependencies(root: &GateRuntimeExpr) -> HirDependencies {
    let mut seen_imports = HashSet::new();
    let mut seen_items = HashSet::new();
    let mut seen_domain_members = HashSet::new();
    let mut seen_instance_members = HashSet::new();
    let mut work = vec![root];
    while let Some(expr) = work.pop() {
        match &expr.kind {
            GateRuntimeExprKind::AmbientSubject
            | GateRuntimeExprKind::Integer(_)
            | GateRuntimeExprKind::Float(_)
            | GateRuntimeExprKind::Decimal(_)
            | GateRuntimeExprKind::BigInt(_)
            | GateRuntimeExprKind::SuffixedInteger(_)
            | GateRuntimeExprKind::Reference(GateRuntimeReference::Local(_))
            | GateRuntimeExprKind::Reference(GateRuntimeReference::Builtin(_))
            | GateRuntimeExprKind::Reference(GateRuntimeReference::IntrinsicValue(_))
            | GateRuntimeExprKind::Reference(GateRuntimeReference::SumConstructor(_)) => {}
            GateRuntimeExprKind::Reference(GateRuntimeReference::Import(import)) => { seen_imports.insert(*import); }
            GateRuntimeExprKind::Reference(GateRuntimeReference::Item(item)) => {
                seen_items.insert(*item);
            }
            GateRuntimeExprKind::Reference(GateRuntimeReference::DomainMember(handle)) => {
                seen_domain_members.insert(DomainMemberKey {
                    domain: handle.domain,
                    member_index: handle.member_index,
                });
            }
            GateRuntimeExprKind::Reference(GateRuntimeReference::ClassMember(dispatch)) => {
                match dispatch.implementation {
                    aivi_hir::ClassMemberImplementation::SameModuleInstance { instance, member_index } => {
                        seen_instance_members.insert(InstanceMemberKey { instance, member_index });
                    }
                    aivi_hir::ClassMemberImplementation::ImportedInstance { import } => { seen_imports.insert(import); }
                    aivi_hir::ClassMemberImplementation::Builtin => {}
                }
            }
            GateRuntimeExprKind::Text(text) => {
                for segment in text.segments.iter().rev() {
                    if let GateRuntimeTextSegment::Interpolation(interpolation) = segment {
                        work.push(interpolation);
                    }
                }
            }
            GateRuntimeExprKind::Tuple(elements)
            | GateRuntimeExprKind::List(elements)
            | GateRuntimeExprKind::Set(elements) => {
                for element in elements.iter().rev() {
                    work.push(element);
                }
            }
            GateRuntimeExprKind::Map(entries) => {
                for entry in entries.iter().rev() {
                    work.push(&entry.value);
                    work.push(&entry.key);
                }
            }
            GateRuntimeExprKind::Record(fields) => {
                for field in fields.iter().rev() {
                    work.push(&field.value);
                }
            }
            GateRuntimeExprKind::Projection { base, .. } => {
                if let GateRuntimeProjectionBase::Expr(base) = base {
                    work.push(base);
                }
            }
            GateRuntimeExprKind::Apply { callee, arguments } => {
                for argument in arguments.iter().rev() {
                    work.push(argument);
                }
                work.push(callee);
            }
            GateRuntimeExprKind::Unary { expr, .. } => work.push(expr),
            GateRuntimeExprKind::Binary { left, right, .. } => {
                work.push(right);
                work.push(left);
            }
            GateRuntimeExprKind::Pipe(pipe) => {
                work.push(&pipe.head);
                for stage in pipe.stages.iter().rev() {
                    match &stage.kind {
                        GateRuntimePipeStageKind::Transform { expr, .. }
                        | GateRuntimePipeStageKind::Tap { expr }
                        | GateRuntimePipeStageKind::Gate {
                            predicate: expr, ..
                        }
                        | GateRuntimePipeStageKind::FanOut { map_expr: expr } => work.push(expr),
                        GateRuntimePipeStageKind::Case { arms } => {
                            for arm in arms.iter().rev() {
                                work.push(&arm.body);
                            }
                        }
                        GateRuntimePipeStageKind::TruthyFalsy { truthy, falsy } => {
                            work.push(&falsy.body);
                            work.push(&truthy.body);
                        }
                    }
                }
            }
        }
    }
    let mut items = seen_items.into_iter().collect::<Vec<_>>();
    items.sort_by_key(|item| item.as_raw());
    let mut domain_members = seen_domain_members.into_iter().collect::<Vec<_>>();
    domain_members.sort();
    let mut instance_members = seen_instance_members.into_iter().collect::<Vec<_>>();
    instance_members.sort();
    let mut imports = seen_imports.into_iter().collect::<Vec<_>>();
    imports.sort_by_key(|import| import.as_raw());
    HirDependencies {
        imports,
        items,
        domain_members,
        instance_members,
    }
}
/// Select bundled implementations through resolved references. Hoisting makes names
/// available; it must not make every library body an execution dependency.
fn select_workspace_items(
    entry: &aivi_hir::Module,
    workspace: &[(&str, &aivi_hir::Module)],
    roots: &HashSet<HirItemId>,
) -> Vec<HashSet<HirItemId>> {
    enum Pending { Item(usize, HirItemId), Import(usize, ImportId) }
    let modules = std::iter::once(entry).chain(workspace.iter().map(|(_, hir)| *hir)).collect::<Vec<_>>();
    let module_indices = workspace.iter().enumerate().map(|(index, (name, _))| (*name, index + 1)).collect::<HashMap<_, _>>();
    let import_maps = modules.iter().map(|hir| ModuleLowerer::make_import_to_module_map(hir)).collect::<Vec<_>>();
    let mut reports = (0..modules.len()).map(|_| None).collect::<Vec<Option<HashMap<HirItemId, Vec<HirDependencies>>>>>();
    let mut names = Vec::new();
    for hir in &modules {
        names.push(hir.items().iter().filter_map(|(id, item)| {
            let name = match item {
                HirItem::Function(item) => item.name.text(),
                HirItem::Value(item) => item.name.text(),
                HirItem::Signal(item) => item.name.text(),
                HirItem::Domain(item) => item.name.text(),
                _ => return None,
            };
            Some((name, id))
        }).collect::<HashMap<_, _>>());
    }
    let mut selected = vec![HashSet::new(); modules.len()];
    let mut visited_imports = HashSet::new();
    let mut work = roots.iter().map(|id| Pending::Item(0, *id)).collect::<Vec<_>>();
    // Application modules retain their source/markup lifecycle roots.
    for (index, (name, hir)) in workspace.iter().enumerate() {
        if !name.starts_with("aivi.") && *name != "aivi" {
            work.extend(hir.items().iter().map(|(id, _)| Pending::Item(index + 1, id)));
        }
    }
    while let Some(pending) = work.pop() {
        match pending {
            Pending::Item(index, id) => {
                if !selected[index].insert(id) { continue; }
                let report = reports[index].get_or_insert_with(|| workspace_item_dependencies(modules[index]));
                if let Some(deps) = report.get(&id) {
                    for dep in deps {
                        work.extend(dep.items.iter().map(|id| Pending::Item(index, *id)));
                        work.extend(dep.domain_members.iter().map(|key| Pending::Item(index, key.domain)));
                        work.extend(dep.instance_members.iter().map(|key| Pending::Item(index, key.instance)));
                        work.extend(dep.imports.iter().map(|id| Pending::Import(index, *id)));
                    }
                }
            }
            Pending::Import(index, id) => {
                if !visited_imports.insert((index, id)) { continue; }
                let Some(binding) = modules[index].imports().get(id) else { continue; };
                let Some(source) = binding.source_module.as_deref().or_else(|| import_maps[index].get(&id).map(|name| name.as_ref())) else { continue; };
                let Some(&target) = module_indices.get(source) else { continue; };
                let name = workspace_import_cache_key(binding);
                if let Some(&item) = names[target].get(name) {
                    work.push(Pending::Item(target, item));
                } else if let ImportBindingMetadata::InstanceMember { class_name, member_name, subject, .. } = &binding.metadata {
                    for (owner, item) in modules[target].items().iter() {
                        let HirItem::Instance(instance) = item else { continue; };
                        if instance.class.path.segments().last().text() == class_name.as_ref()
                            && instance.arguments.iter().next().and_then(|ty| workspace_instance_subject_label(modules[target], *ty)).as_deref() == Some(subject.as_ref())
                            && instance.members.iter().any(|member| member.name.text() == member_name.as_ref())
                        { work.push(Pending::Item(target, owner)); }
                    }
                } else if let ImportBindingMetadata::DomainSuffix { domain_name, .. } = &binding.metadata {
                    if let Some(&item) = names[target].get(domain_name.as_ref()) { work.push(Pending::Item(target, item)); }
                } else {
                    for (import, candidate) in modules[target].imports().iter() {
                        if candidate.local_name.text() == name { work.push(Pending::Import(target, import)); }
                    }
                }
            }
        }
    }
    selected
}

fn workspace_item_dependencies(hir: &aivi_hir::Module) -> HashMap<HirItemId, Vec<HirDependencies>> {
        let mut deps: HashMap<HirItemId, Vec<HirDependencies>> = HashMap::new();
        for report in [elaborate_general_expressions(hir), elaborate_ambient_items(hir)] {
            let (items, domains, instances) = report.into_parts();
            for item in items {
                if let GeneralExprOutcome::Lowered(body) = item.outcome {
                    deps.entry(item.owner).or_default().push(referenced_hir_dependencies(&body));
                }
            }
            for item in domains {
                if let GeneralExprOutcome::Lowered(body) = item.outcome {
                    deps.entry(item.domain_owner).or_default().push(referenced_hir_dependencies(&body));
                }
            }
            for item in instances {
                if let GeneralExprOutcome::Lowered(body) = item.outcome {
                    deps.entry(item.instance_owner).or_default().push(referenced_hir_dependencies(&body));
                }
            }
        }
    deps
}
