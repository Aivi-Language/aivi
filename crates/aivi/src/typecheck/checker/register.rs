impl TypeChecker {
    fn qualified_type_name(&self, module_name: &str, type_name: &str) -> String {
        format!("{module_name}.{type_name}")
    }

    fn bind_type_name(&mut self, local_name: String, internal_name: String) {
        // Never downgrade an already-qualified binding to a bare name.  This preserves the
        // global-seeded qualified names (e.g. "DateTime" → "aivi.calendar.DateTime") when a
        // re-export module (like `aivi`) ships a bare `internal_name` for the same type.
        let keep_existing = self
            .type_name_bindings
            .get(&local_name)
            .is_some_and(|existing| existing.contains('.') && !internal_name.contains('.'));
        if !keep_existing {
            self.type_name_bindings
                .insert(local_name.clone(), internal_name.clone());
            self.type_display_names.insert(internal_name, local_name);
        }
    }

    pub(super) fn register_module_types(&mut self, module: &Module) {
        for item in &module.items {
            match item {
                ModuleItem::TypeDecl(type_decl) => {
                    let mut kind = Kind::Star;
                    for _ in &type_decl.params {
                        kind = Kind::Arrow(Box::new(Kind::Star), Box::new(kind));
                    }
                    let internal_name =
                        self.qualified_type_name(&module.name.name, &type_decl.name.name);
                    self.type_constructors.insert(internal_name.clone(), kind);
                    self.bind_type_name(type_decl.name.name.clone(), internal_name.clone());
                    if type_decl.opaque {
                        self.opaque_types.insert(internal_name, module.name.name.clone());
                    }
                }
                ModuleItem::TypeAlias(alias) => {
                    let mut kind = Kind::Star;
                    for _ in &alias.params {
                        kind = Kind::Arrow(Box::new(Kind::Star), Box::new(kind));
                    }
                    let internal_name = self.qualified_type_name(&module.name.name, &alias.name.name);
                    self.type_constructors.insert(internal_name.clone(), kind);
                    self.bind_type_name(alias.name.name.clone(), internal_name.clone());
                    let alias_info = self.alias_info(alias);
                    self.aliases.insert(internal_name.clone(), alias_info);
                    if alias.opaque {
                        self.opaque_types.insert(internal_name, module.name.name.clone());
                    }
                }
                ModuleItem::DomainDecl(domain) => {
                    for domain_item in &domain.items {
                        if let DomainItem::TypeAlias(type_decl) = domain_item {
                            let mut kind = Kind::Star;
                            for _ in &type_decl.params {
                                kind = Kind::Arrow(Box::new(Kind::Star), Box::new(kind));
                            }
                            let internal_name =
                                self.qualified_type_name(&module.name.name, &type_decl.name.name);
                            self.type_constructors.insert(internal_name.clone(), kind);
                            self.bind_type_name(type_decl.name.name.clone(), internal_name.clone());
                            if type_decl.opaque {
                                self.opaque_types.insert(internal_name, module.name.name.clone());
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// Re-build alias bodies for all TypeAlias items in the module using the current
    /// `type_name_bindings` (which are only fully populated after `register_imported_type_names`).
    /// Must be called after `register_imported_type_names` so that imported type names
    /// (e.g. `Field` from `aivi.ui.forms`) are resolved to their qualified internal names.
    pub(super) fn rebuild_module_alias_bodies(&mut self, module: &Module) {
        for item in &module.items {
            if let ModuleItem::TypeAlias(alias) = item {
                let internal_name =
                    self.qualified_type_name(&module.name.name, &alias.name.name);
                let alias_info = self.alias_info(alias);
                self.aliases.insert(internal_name, alias_info);
            }
        }
    }

    pub(super) fn register_builtin_aliases(&mut self) {
        let a = self.fresh_var_id();
        self.bind_type_name("Patch".to_string(), "Patch".to_string());
        self.aliases.insert(
            "Patch".to_string(),
            AliasInfo {
                params: vec![a],
                body: Type::Func(Box::new(Type::Var(a)), Box::new(Type::Var(a))),
            },
        );

        // Unit ≅ {} — the empty record and the unit type are interchangeable.
        // `{}` as an expression parses as an empty plain-block (inferred Unit), while
        // `{}` as a type annotation produces Type::Record { fields: {} }.  Adding this
        // alias lets unification see through the distinction so callers can write
        // `render m {}` where the function expects `{}` (empty record).

        // v0.1: Source errors are currently just `Text` messages.
        // The `K` parameter exists for spec alignment and future evolution.
        let k = self.fresh_var_id();
        self.bind_type_name("SourceError".to_string(), "SourceError".to_string());
        self.aliases.insert(
            "SourceError".to_string(),
            AliasInfo {
                params: vec![k],
                body: Type::con("Text"),
            },
        );
    }

    pub(super) fn register_imported_type_names(
        &mut self,
        module: &Module,
        module_type_exports: &HashMap<String, HashMap<String, super::TypeSurface>>,
    ) {
        for use_decl in &module.uses {
            let Some(exports) = module_type_exports.get(&use_decl.module.name) else {
                continue;
            };
            for surface in exports.values() {
                self.type_constructors
                    .insert(surface.internal_name.clone(), surface.kind.clone());
                if let Some(alias) = &surface.alias {
                    self.aliases
                        .insert(surface.internal_name.clone(), alias.clone());
                }
                if let Some(origin) = &surface.opaque_origin {
                    self.opaque_types
                        .insert(surface.internal_name.clone(), origin.clone());
                }
            }
        }

        let available_names: HashMap<String, HashSet<String>> = module_type_exports
            .iter()
            .map(|(module_name, exports)| (module_name.clone(), exports.keys().cloned().collect()))
            .collect();
        let mut local_defs: HashSet<String> = HashSet::new();
        for item in &module.items {
            match item {
                ModuleItem::TypeDecl(type_decl) => {
                    local_defs.insert(type_decl.name.name.clone());
                }
                ModuleItem::TypeAlias(alias) => {
                    local_defs.insert(alias.name.name.clone());
                }
                ModuleItem::DomainDecl(domain) => {
                    for domain_item in &domain.items {
                        if let DomainItem::TypeAlias(type_decl) = domain_item {
                            local_defs.insert(type_decl.name.name.clone());
                        }
                    }
                }
                _ => {}
            }
        }
        let import_pairs =
            crate::surface::compute_import_pairs(&module.uses, &available_names, &local_defs);

        for (bare, qualified) in &import_pairs {
            let Some((module_name, original)) = qualified.rsplit_once('.') else {
                continue;
            };
            let Some(exports) = module_type_exports.get(module_name) else {
                continue;
            };
            let Some(surface) = exports.get(original) else {
                continue;
            };
            self.bind_type_name(bare.clone(), surface.internal_name.clone());
        }

        for use_decl in &module.uses {
            if use_decl.alias.is_none() {
                continue;
            }
            let Some(exports) = module_type_exports.get(&use_decl.module.name) else {
                continue;
            };
            for (name, surface) in exports {
                if local_defs.contains(name) {
                    continue;
                }
                self.bind_type_name(name.clone(), surface.internal_name.clone());
            }
        }
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
                    let Some(internal_name) = self
                        .resolve_type_binding(&type_decl.name.name)
                        .map(str::to_string)
                    else {
                        continue;
                    };
                    if !type_decl.constructors.is_empty() {
                        self.adt_constructors.insert(
                            internal_name.clone(),
                            type_decl
                                .constructors
                                .iter()
                                .map(|ctor| ctor.name.name.clone())
                                .collect(),
                        );
                    }
                    // Skip registering constructors for opaque ADTs outside their module.
                    if type_decl.opaque && self.is_opaque_from_here(&internal_name).is_some() {
                        continue;
                    }
                    self.register_adt_constructors(type_decl, env);
                }
                ModuleItem::DomainDecl(domain) => {
                    for domain_item in &domain.items {
                        if let DomainItem::TypeAlias(type_decl) = domain_item {
                            let Some(internal_name) =
                                self.resolve_type_binding(&type_decl.name.name)
                                    .map(str::to_string)
                            else {
                                continue;
                            };
                            if !type_decl.constructors.is_empty() {
                                self.adt_constructors.insert(
                                    internal_name.clone(),
                                    type_decl
                                        .constructors
                                        .iter()
                                        .map(|ctor| ctor.name.name.clone())
                                        .collect(),
                                );
                            }
                            if type_decl.opaque && self.is_opaque_from_here(&internal_name).is_some()
                            {
                                continue;
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
            self.var_names.insert(var, param.name.clone());
            ctx.type_vars.insert(param.name.clone(), var);
            params.push(var);
        }
        let result_name = self
            .resolve_type_binding(&type_decl.name.name)
            .unwrap_or(&type_decl.name.name);
        let result_type =
            Type::con(result_name).app(params.iter().map(|var| Type::Var(*var)).collect());

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

        // Qualified export names (for example `aivi.database.load`) should be
        // available everywhere without requiring a local import. Unqualified
        // spellings still follow normal import rules.
        for (mod_name, exports) in module_exports {
            for (name, schemes) in exports {
                Self::insert_schemes(env, format!("{mod_name}.{name}"), schemes);
            }
        }

        let empty_defs = HashSet::new();
        let import_pairs = crate::surface::compute_import_pairs(
            &module.uses,
            &available_names,
            &empty_defs,
        );

        // Register value imports (bare + qualified) with their type schemes.
        for (bare, qualified) in &import_pairs {
            if let Some((mod_name, original)) = qualified.rsplit_once('.') {
                if let Some(exports) = module_exports.get(mod_name) {
                    if let Some(schemes) = exports.get(original) {
                        Self::insert_schemes(env, bare.clone(), schemes);
                        Self::insert_schemes(env, qualified.clone(), schemes);
                    }
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

            if use_decl.wildcard {
                // Wildcard import: also bring in all domain members, minus any hidden names.
                if let Some(domains) = module_domain_exports.get(&use_decl.module.name) {
                    for (domain_name, members) in domains {
                        if use_decl.hides_domain(domain_name) {
                            continue;
                        }
                        for member in members {
                            if use_decl.hides_value(member) {
                                continue;
                            }
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
                continue;
            }
            for item in use_decl.imported_items() {
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
            // For suffix template names (start with a digit, e.g. "1s", "1px"), accumulate
            // instead of overwrite so that importing two domains that both define the same
            // suffix template is detected as ambiguous at the use site.
            if name.starts_with(|c: char| c.is_ascii_digit()) {
                if let Some(existing) = env.get_all(&name) {
                    // Skip if the same origin is already present (avoids double-registration
                    // when elaboration adds domain members to module_exports and the wildcard
                    // domain import loop also tries to insert them).
                    let new_origin = &schemes[0].origin;
                    let already_present = existing.iter().any(|s| s.origin == *new_origin);
                    if !already_present {
                        let mut combined = existing.to_vec();
                        combined.push(schemes[0].clone());
                        env.insert_overloads(name, combined);
                    }
                    return;
                }
            }
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
