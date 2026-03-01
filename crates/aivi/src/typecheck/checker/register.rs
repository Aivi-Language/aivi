impl TypeChecker {
    pub(super) fn register_module_types(&mut self, module: &Module) {
        for item in &module.items {
            match item {
                ModuleItem::TypeDecl(type_decl) => {
                    let mut kind = Kind::Star;
                    for _ in &type_decl.params {
                        kind = Kind::Arrow(Box::new(Kind::Star), Box::new(kind));
                    }
                    self.type_constructors
                        .insert(type_decl.name.name.clone(), kind);
                }
                ModuleItem::TypeAlias(alias) => {
                    let mut kind = Kind::Star;
                    for _ in &alias.params {
                        kind = Kind::Arrow(Box::new(Kind::Star), Box::new(kind));
                    }
                    self.type_constructors.insert(alias.name.name.clone(), kind);
                    let alias_info = self.alias_info(alias);
                    self.aliases.insert(alias.name.name.clone(), alias_info);
                }
                ModuleItem::DomainDecl(domain) => {
                    for domain_item in &domain.items {
                        if let DomainItem::TypeAlias(type_decl) = domain_item {
                            let mut kind = Kind::Star;
                            for _ in &type_decl.params {
                                kind = Kind::Arrow(Box::new(Kind::Star), Box::new(kind));
                            }
                            self.type_constructors
                                .insert(type_decl.name.name.clone(), kind);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    pub(super) fn register_builtin_aliases(&mut self) {
        let a = self.fresh_var_id();
        self.aliases.insert(
            "Patch".to_string(),
            AliasInfo {
                params: vec![a],
                body: Type::Func(Box::new(Type::Var(a)), Box::new(Type::Var(a))),
            },
        );

        // v0.1: Source errors are currently just `Text` messages.
        // The `K` parameter exists for spec alignment and future evolution.
        let k = self.fresh_var_id();
        self.aliases.insert(
            "SourceError".to_string(),
            AliasInfo {
                params: vec![k],
                body: Type::con("Text"),
            },
        );
    }

    pub(super) fn collect_type_expr_diags(&mut self, module: &Module) -> Vec<FileDiagnostic> {
        let mut errors = Vec::new();
        for item in &module.items {
            match item {
                ModuleItem::TypeSig(sig) => {
                    self.validate_type_expr(&sig.ty, &mut errors);
                }
                ModuleItem::TypeAlias(alias) => {
                    self.validate_type_expr(&alias.aliased, &mut errors);
                }
                ModuleItem::DomainDecl(domain) => {
                    for domain_item in &domain.items {
                        match domain_item {
                            DomainItem::TypeSig(sig) => {
                                self.validate_type_expr(&sig.ty, &mut errors);
                            }
                            DomainItem::TypeAlias(_) => {}
                            DomainItem::Def(_) | DomainItem::LiteralDef(_) => {}
                        }
                    }
                }
                _ => {}
            }
        }
        errors
            .into_iter()
            .map(|err| self.error_to_diag(module, err))
            .collect()
    }

    pub(super) fn alias_info(&mut self, alias: &TypeAlias) -> AliasInfo {
        let mut ctx = TypeContext::new(&self.type_constructors);
        let mut params = Vec::new();
        for param in &alias.params {
            let var = self.fresh_var_id();
            self.var_names.insert(var, param.name.clone());
            ctx.type_vars.insert(param.name.clone(), var);
            params.push(var);
        }
        let body = self.type_from_expr(&alias.aliased, &mut ctx);
        AliasInfo { params, body }
    }

    pub(super) fn collect_type_sigs(&mut self, module: &Module) -> HashMap<String, Vec<Scheme>> {
        let mut sigs: HashMap<String, Vec<Scheme>> = HashMap::new();
        for item in &module.items {
            if let ModuleItem::TypeSig(sig) = item {
                let scheme =
                    self.scheme_from_sig(sig, SchemeOrigin::new(module.name.name.clone(), None));
                sigs.entry(sig.name.name.clone()).or_default().push(scheme);
            }
            if let ModuleItem::DomainDecl(domain) = item {
                for domain_item in &domain.items {
                    if let DomainItem::TypeSig(sig) = domain_item {
                        let scheme = self.scheme_from_sig(
                            sig,
                            SchemeOrigin::new(
                                module.name.name.clone(),
                                Some(domain.name.name.clone()),
                            ),
                        );
                        sigs.entry(sig.name.name.clone()).or_default().push(scheme);
                    }
                }
            }
        }
        sigs
    }

    fn scheme_from_sig(&mut self, sig: &TypeSig, origin: SchemeOrigin) -> Scheme {
        let mut ctx = TypeContext::new(&self.type_constructors);
        let ty = self.type_from_expr(&sig.ty, &mut ctx);
        let vars: Vec<TypeVarId> = ctx.type_vars.values().cloned().collect();
        Scheme {
            vars,
            ty,
            origin: Some(origin),
        }
    }

    pub(super) fn register_module_constructors(&mut self, module: &Module, env: &mut TypeEnv) {
        for item in &module.items {
            match item {
                ModuleItem::TypeDecl(type_decl) => {
                    if !type_decl.constructors.is_empty() {
                        self.adt_constructors.insert(
                            type_decl.name.name.clone(),
                            type_decl
                                .constructors
                                .iter()
                                .map(|ctor| ctor.name.name.clone())
                                .collect(),
                        );
                    }
                    self.register_adt_constructors(type_decl, env);
                }
                ModuleItem::DomainDecl(domain) => {
                    for domain_item in &domain.items {
                        if let DomainItem::TypeAlias(type_decl) = domain_item {
                            if !type_decl.constructors.is_empty() {
                                self.adt_constructors.insert(
                                    type_decl.name.name.clone(),
                                    type_decl
                                        .constructors
                                        .iter()
                                        .map(|ctor| ctor.name.name.clone())
                                        .collect(),
                                );
                            }
                            self.register_adt_constructors(type_decl, env);
                        }
                    }
                }
                ModuleItem::MachineDecl(machine_decl) => {
                    let mut machine_scheme = Scheme::mono(self.fresh_var());
                    machine_scheme.origin = Some(SchemeOrigin::new(module.name.name.clone(), None));
                    env.insert(machine_decl.name.name.clone(), machine_scheme);

                    for state in &machine_decl.states {
                        let mut state_scheme = Scheme::mono(self.fresh_var());
                        state_scheme.origin = Some(SchemeOrigin::new(module.name.name.clone(), None));
                        env.insert(state.name.name.clone(), state_scheme);
                    }

                    for transition in &machine_decl.transitions {
                        let mut transition_scheme = Scheme::mono(self.fresh_var());
                        transition_scheme.origin =
                            Some(SchemeOrigin::new(module.name.name.clone(), None));
                        env.insert(transition.name.name.clone(), transition_scheme);
                    }
                }
                _ => {}
            }
        }
    }

    fn register_adt_constructors(&mut self, type_decl: &TypeDecl, env: &mut TypeEnv) {
        if type_decl.constructors.is_empty() {
            return;
        }
        let mut ctx = TypeContext::new(&self.type_constructors);
        let mut params = Vec::new();
        for param in &type_decl.params {
            let var = self.fresh_var_id();
            self.var_names.insert(var, param.name.clone());
            ctx.type_vars.insert(param.name.clone(), var);
            params.push(var);
        }
        let result_type =
            Type::con(&type_decl.name.name).app(params.iter().map(|var| Type::Var(*var)).collect());

        for ctor in &type_decl.constructors {
            let mut ctor_type = result_type.clone();
            for arg in ctor.args.iter().rev() {
                let arg_type = self.type_from_expr(arg, &mut ctx);
                ctor_type = Type::Func(Box::new(arg_type), Box::new(ctor_type));
            }
            let scheme = Scheme {
                vars: params.clone(),
                ty: ctor_type,
                origin: None,
            };
            env.insert(ctor.name.name.clone(), scheme);
        }
    }

    pub(super) fn register_imports(
        &mut self,
        module: &Module,
        module_exports: &HashMap<String, HashMap<String, Vec<Scheme>>>,
        module_domain_exports: &HashMap<String, HashMap<String, Vec<String>>>,
        env: &mut TypeEnv,
    ) {
        // Build available_names from module_exports for compute_import_pairs.
        let available_names: HashMap<String, HashSet<String>> = module_exports
            .iter()
            .map(|(mod_name, exports)| {
                (mod_name.clone(), exports.keys().cloned().collect())
            })
            .collect();
        let empty_defs = HashSet::new();
        let import_pairs = crate::surface::compute_import_pairs(
            &module.uses,
            &available_names,
            &empty_defs,
        );

        // Register value imports (bare + qualified) with their type schemes.
        for (bare, qualified) in &import_pairs {
            let mod_name = qualified.rsplit_once('.').map(|(m, _)| m).unwrap_or("");
            if let Some(exports) = module_exports.get(mod_name) {
                if let Some(schemes) = exports.get(bare) {
                    Self::insert_schemes(env, bare.clone(), schemes);
                    Self::insert_schemes(env, qualified.clone(), schemes);
                }
            }
        }

        // Handle domain imports and aliased imports (not covered by compute_import_pairs).
        for use_decl in &module.uses {
            let Some(exports) = module_exports.get(&use_decl.module.name) else {
                continue;
            };

            // Aliased imports (e.g. `use aivi.list as List`) still need their
            // individual exports registered under bare + qualified names so
            // alias-expanded references like `aivi.list.find` can be typed.
            if use_decl.alias.is_some() {
                for (name, schemes) in exports {
                    Self::insert_schemes(env, name.clone(), schemes);
                    Self::insert_schemes(
                        env,
                        format!("{}.{}", use_decl.module.name, name),
                        schemes,
                    );
                }
                continue;
            }

            if use_decl.items.is_empty() {
                // Wildcard import: also bring in all domain members.
                if use_decl.wildcard {
                    if let Some(domains) = module_domain_exports.get(&use_decl.module.name) {
                        for members in domains.values() {
                            for member in members {
                                if let Some(schemes) = exports.get(member) {
                                    Self::insert_schemes(env, member.clone(), schemes);
                                    Self::insert_schemes(
                                        env,
                                        format!("{}.{}", use_decl.module.name, member),
                                        schemes,
                                    );
                                }
                            }
                        }
                    }
                }
                continue;
            }
            for item in &use_decl.items {
                if item.kind != crate::surface::ScopeItemKind::Domain {
                    continue;
                }
                let Some(domains) = module_domain_exports.get(&use_decl.module.name) else {
                    continue;
                };
                let Some(members) = domains.get(&item.name.name) else {
                    continue;
                };
                for member in members {
                    if let Some(schemes) = exports.get(member) {
                        Self::insert_schemes(env, member.clone(), schemes);
                        Self::insert_schemes(
                            env,
                            format!("{}.{}", use_decl.module.name, member),
                            schemes,
                        );
                    }
                }
            }
        }
    }

    fn insert_schemes(env: &mut TypeEnv, name: String, schemes: &[Scheme]) {
        if schemes.len() == 1 {
            env.insert(name, schemes[0].clone());
        } else {
            env.insert_overloads(name, schemes.to_vec());
        }
    }

    pub(super) fn register_module_defs(
        &mut self,
        module: &Module,
        sigs: &HashMap<String, Vec<Scheme>>,
        env: &mut TypeEnv,
    ) {
        for item in &module.items {
            match item {
                ModuleItem::Def(def) => {
                    let name = def.name.name.clone();
                    match sigs.get(&name) {
                        Some(candidates) if candidates.len() == 1 => {
                            let mut scheme = candidates[0].clone();
                            scheme.origin = Some(SchemeOrigin::new(module.name.name.clone(), None));
                            env.insert(name, scheme);
                        }
                        Some(candidates) => {
                            let mut schemes = candidates.clone();
                            for scheme in &mut schemes {
                                scheme.origin =
                                    Some(SchemeOrigin::new(module.name.name.clone(), None));
                            }
                            env.insert_overloads(name, schemes);
                        }
                        None => {
                            let mut scheme = Scheme::mono(self.fresh_var());
                            scheme.origin = Some(SchemeOrigin::new(module.name.name.clone(), None));
                            env.insert(name, scheme);
                        }
                    }
                }
                ModuleItem::DomainDecl(domain) => {
                    for domain_item in &domain.items {
                        match domain_item {
                            DomainItem::Def(def) | DomainItem::LiteralDef(def) => {
                                let name = def.name.name.clone();
                                match sigs.get(&name) {
                                    Some(candidates) if candidates.len() == 1 => {
                                        let mut scheme = candidates[0].clone();
                                        scheme.origin = Some(SchemeOrigin::new(
                                            module.name.name.clone(),
                                            Some(domain.name.name.clone()),
                                        ));
                                        env.insert(name, scheme);
                                    }
                                    Some(candidates) => {
                                        let mut schemes = candidates.clone();
                                        for scheme in &mut schemes {
                                            scheme.origin = Some(SchemeOrigin::new(
                                                module.name.name.clone(),
                                                Some(domain.name.name.clone()),
                                            ));
                                        }
                                        env.insert_overloads(name, schemes);
                                    }
                                    None => {
                                        let mut scheme = Scheme::mono(self.fresh_var());
                                        scheme.origin = Some(SchemeOrigin::new(
                                            module.name.name.clone(),
                                            Some(domain.name.name.clone()),
                                        ));
                                        env.insert(name, scheme);
                                    }
                                }
                            }
                            DomainItem::TypeAlias(_) | DomainItem::TypeSig(_) => {}
                        }
                    }
                }
                _ => {}
            }
        }
    }

    pub(super) fn check_module_defs(
        &mut self,
        module: &Module,
        sigs: &HashMap<String, Vec<Scheme>>,
        env: &mut TypeEnv,
    ) -> Vec<FileDiagnostic> {
        let mut diagnostics = Vec::new();
        let trace = std::env::var("AIVI_TRACE_TIMING").is_ok_and(|v| v == "1");
        let mut def_counts: HashMap<String, usize> = HashMap::new();
        for item in &module.items {
            match item {
                ModuleItem::Def(def) => {
                    *def_counts.entry(def.name.name.clone()).or_insert(0) += 1;
                }
                ModuleItem::DomainDecl(domain) => {
                    for domain_item in &domain.items {
                        match domain_item {
                            DomainItem::Def(def) | DomainItem::LiteralDef(def) => {
                                *def_counts.entry(def.name.name.clone()).or_insert(0) += 1;
                            }
                            DomainItem::TypeAlias(_) | DomainItem::TypeSig(_) => {}
                        }
                    }
                }
                _ => {}
            }
        }
        for item in &module.items {
            match item {
                ModuleItem::Def(def) => {
                    let def_count = def_counts.get(&def.name.name).copied().unwrap_or(1);
                    let t0 = if trace { Some(std::time::Instant::now()) } else { None };
                    self.check_def(def, sigs, env, module, def_count, &mut diagnostics);
                    if let Some(t0) = t0 {
                        let ms = t0.elapsed().as_millis();
                        if ms > 10 {
                            eprintln!("[AIVI_TIMING_DEF] {}.{:<40} {:>6}ms  env={}", module.name.name, def.name.name, ms, env.len());
                        }
                    }
                    if self.compact_subst_between_defs {
                        self.compact_after_def();
                    }
                }
                ModuleItem::InstanceDecl(instance) => {
                    self.check_instance_decl(instance, env, module, &mut diagnostics);
                }
                ModuleItem::DomainDecl(domain) => {
                    for domain_item in &domain.items {
                        match domain_item {
                            DomainItem::Def(def) | DomainItem::LiteralDef(def) => {
                                let def_count =
                                    def_counts.get(&def.name.name).copied().unwrap_or(1);
                                let t0 = if trace { Some(std::time::Instant::now()) } else { None };
                                self.check_def(def, sigs, env, module, def_count, &mut diagnostics);
                                if let Some(t0) = t0 {
                                    let ms = t0.elapsed().as_millis();
                                    if ms > 10 {
                                        eprintln!("[AIVI_TIMING_DEF] {}.{:<40} {:>6}ms  env={}", module.name.name, def.name.name, ms, env.len());
                                    }
                                }
                                if self.compact_subst_between_defs {
                                    self.compact_after_def();
                                }
                            }
                            DomainItem::TypeAlias(_) | DomainItem::TypeSig(_) => {}
                        }
                    }
                }
                _ => {}
            }
        }
        diagnostics.append(&mut self.extra_diagnostics);
        diagnostics
    }
}
