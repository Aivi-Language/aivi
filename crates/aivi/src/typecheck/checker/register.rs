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
            ctx.type_vars.insert(param.name.clone(), var);
            params.push(var);
        }
        let body = self.type_from_expr(&alias.aliased, &mut ctx);
        AliasInfo { params, body }
    }

    pub(super) fn collect_type_sigs(&mut self, module: &Module) -> HashMap<String, Scheme> {
        let mut sigs = HashMap::new();
        for item in &module.items {
            if let ModuleItem::TypeSig(sig) = item {
                let scheme = self.scheme_from_sig(sig);
                sigs.insert(sig.name.name.clone(), scheme);
            }
            if let ModuleItem::DomainDecl(domain) = item {
                for domain_item in &domain.items {
                    if let DomainItem::TypeSig(sig) = domain_item {
                        let scheme = self.scheme_from_sig(sig);
                        sigs.insert(sig.name.name.clone(), scheme);
                    }
                }
            }
        }
        sigs
    }

    fn scheme_from_sig(&mut self, sig: &TypeSig) -> Scheme {
        let mut ctx = TypeContext::new(&self.type_constructors);
        let ty = self.type_from_expr(&sig.ty, &mut ctx);
        let vars: Vec<TypeVarId> = ctx.type_vars.values().cloned().collect();
        Scheme { vars, ty }
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
            };
            env.insert(ctor.name.name.clone(), scheme);
        }
    }

    pub(super) fn register_imports(
        &mut self,
        module: &Module,
        module_exports: &HashMap<String, HashMap<String, Scheme>>,
        module_domain_exports: &HashMap<String, HashMap<String, Vec<String>>>,
        env: &mut TypeEnv,
    ) {
        for use_decl in &module.uses {
            if let Some(exports) = module_exports.get(&use_decl.module.name) {
                let qualify = use_decl.alias.is_some();
                if use_decl.wildcard {
                    for (name, scheme) in exports {
                        env.insert(name.clone(), scheme.clone());
                        if qualify {
                            env.insert(
                                format!("{}.{}", use_decl.module.name, name),
                                scheme.clone(),
                            );
                        }
                    }
                } else {
                    for item in &use_decl.items {
                        match item.kind {
                            crate::surface::ScopeItemKind::Value => {
                                if let Some(scheme) = exports.get(&item.name.name) {
                                    env.insert(item.name.name.clone(), scheme.clone());
                                    if qualify {
                                        env.insert(
                                            format!("{}.{}", use_decl.module.name, item.name.name),
                                            scheme.clone(),
                                        );
                                    }
                                }
                            }
                            crate::surface::ScopeItemKind::Domain => {
                                let Some(domains) = module_domain_exports.get(&use_decl.module.name)
                                else {
                                    continue;
                                };
                                let Some(members) = domains.get(&item.name.name) else {
                                    continue;
                                };
                                for member in members {
                                    if let Some(scheme) = exports.get(member) {
                                        env.insert(member.clone(), scheme.clone());
                                        if qualify {
                                            env.insert(
                                                format!("{}.{}", use_decl.module.name, member),
                                                scheme.clone(),
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    pub(super) fn register_module_defs(
        &mut self,
        module: &Module,
        sigs: &HashMap<String, Scheme>,
        env: &mut TypeEnv,
    ) {
        for item in &module.items {
            match item {
                ModuleItem::Def(def) => {
                    let scheme = sigs
                        .get(&def.name.name)
                        .cloned()
                        .unwrap_or_else(|| Scheme::mono(self.fresh_var()));
                    env.insert(def.name.name.clone(), scheme);
                }
                ModuleItem::DomainDecl(domain) => {
                    for domain_item in &domain.items {
                        match domain_item {
                            DomainItem::Def(def) | DomainItem::LiteralDef(def) => {
                                let scheme = sigs
                                    .get(&def.name.name)
                                    .cloned()
                                    .unwrap_or_else(|| Scheme::mono(self.fresh_var()));
                                env.insert(def.name.name.clone(), scheme);
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
        sigs: &HashMap<String, Scheme>,
        env: &mut TypeEnv,
    ) -> Vec<FileDiagnostic> {
        let mut diagnostics = Vec::new();
        for item in &module.items {
            match item {
                ModuleItem::Def(def) => {
                    self.check_def(def, sigs, env, module, &mut diagnostics);
                }
                ModuleItem::InstanceDecl(instance) => {
                    self.check_instance_decl(instance, env, module, &mut diagnostics);
                }
                ModuleItem::DomainDecl(domain) => {
                    for domain_item in &domain.items {
                        match domain_item {
                            DomainItem::Def(def) | DomainItem::LiteralDef(def) => {
                                self.check_def(def, sigs, env, module, &mut diagnostics);
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
