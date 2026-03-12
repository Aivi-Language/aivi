use std::collections::{BTreeMap, HashMap, HashSet};

use crate::diagnostics::{Diagnostic, FileDiagnostic, Span};
use crate::surface::{
    BlockItem, BlockKind, Def, DomainItem, Expr, ListItem, Literal, Module, ModuleItem,
    PathSegment, Pattern, RecordField, RecordPatternField, SpannedName, TextPart, TypeAlias,
    TypeDecl, TypeExpr, TypeSig,
};

use super::types::{
    number_kind, split_suffixed_number, AliasInfo, Kind, NumberKind, Scheme, SchemeOrigin, Type,
    TypeContext, TypeEnv, TypeError, TypePrinter, TypeVarId,
};
use super::{constraints::ConstraintState, query_engine::TypeQueryCache};
use super::{ClassDeclInfo, InstanceDeclInfo};

pub(super) struct TypeChecker {
    next_var: u32,
    subst: HashMap<TypeVarId, Type>,
    pub(super) var_names: HashMap<TypeVarId, String>,
    pub(super) type_constructors: HashMap<String, Kind>,
    aliases: HashMap<String, AliasInfo>,
    type_name_bindings: HashMap<String, String>,
    type_display_names: HashMap<String, String>,
    pub(super) builtin_types: HashMap<String, Kind>,
    pub(super) builtins: TypeEnv,
    global_type_constructors: HashMap<String, Kind>,
    global_aliases: HashMap<String, AliasInfo>,
    checked_defs: HashSet<String>,
    pub(super) classes: HashMap<String, ClassDeclInfo>,
    pub(super) instances: Vec<InstanceDeclInfo>,
    method_to_classes: HashMap<String, Vec<String>>,
    assumed_class_constraints: Vec<(String, TypeVarId)>,
    current_module_path: String,
    current_module_name: String,
    extra_diagnostics: Vec<FileDiagnostic>,
    adt_constructors: HashMap<String, Vec<String>>,
    /// Maps opaque type names to the module that defined them.
    /// Used to reject construction/access from outside the defining module.
    opaque_types: HashMap<String, String>,
    global_opaque_types: HashMap<String, String>,
    enabled_record_default_types: HashSet<String>,
    /// Set to `true` while elaborating a function call argument, so that
    /// `elab_record` can enforce field completeness for call-site records.
    in_call_arg: bool,
    pub(super) constraints: ConstraintState,
    pub(super) query_cache: TypeQueryCache,
    /// Records `(qualified_callee_name, resolved_type)` for polymorphic call sites.
    /// Used to build the monomorphization plan.
    poly_instantiations: Vec<(String, Type)>,
    /// Records `(span, type)` for every successfully inferred expression.
    /// Used by the LSP to provide hover information at arbitrary positions.
    pub(super) span_types: Vec<(Span, Type)>,
    /// When true, clear the substitution map between function definitions.
    /// This prevents quadratic blow-up from accumulating dead type variables across defs.
    /// Only safe when span_types are not needed (i.e. `aivi run`, not LSP).
    pub(super) compact_subst_between_defs: bool,
    /// Name of the definition currently being type-checked (for source schema tracking).
    pub(super) current_def_name: String,
    /// Records `(module, def_name, inner_cg_type)` for each `load` call site where
    /// the inner type `A` of `Source K A` is concrete. Used to inject JSON validation
    /// schemas at source boundaries.
    load_source_schemas: Vec<(String, String, CgType)>,
    /// Maps bare type names to their unique qualified names from the global type universe.
    /// Built from `global_type_constructors` keys: e.g. `"DateTime"` → `"aivi.calendar.DateTime"`.
    /// Seeded into `type_name_bindings` on every `reset_module_context` so that bare type names
    /// used in stdlib definitions and sigil literals consistently resolve to qualified names
    /// even when the module does not explicitly import the type's defining module.
    bare_to_global_qualified: HashMap<String, String>,
}

impl TypeChecker {
    pub(super) fn new() -> Self {
        let mut checker = Self {
            next_var: 0,
            subst: HashMap::new(),
            var_names: HashMap::new(),
            type_constructors: HashMap::new(),
            aliases: HashMap::new(),
            type_name_bindings: HashMap::new(),
            type_display_names: HashMap::new(),
            builtin_types: HashMap::new(),
            builtins: TypeEnv::default(),
            global_type_constructors: HashMap::new(),
            global_aliases: HashMap::new(),
            checked_defs: HashSet::new(),
            classes: HashMap::new(),
            instances: Vec::new(),
            method_to_classes: HashMap::new(),
            assumed_class_constraints: Vec::new(),
            current_module_path: String::new(),
            current_module_name: String::new(),
            extra_diagnostics: Vec::new(),
            adt_constructors: HashMap::new(),
            opaque_types: HashMap::new(),
            global_opaque_types: HashMap::new(),
            enabled_record_default_types: HashSet::new(),
            in_call_arg: false,
            constraints: ConstraintState::default(),
            query_cache: TypeQueryCache::default(),
            poly_instantiations: Vec::new(),
            span_types: Vec::new(),
            compact_subst_between_defs: false,
            current_def_name: String::new(),
            load_source_schemas: Vec::new(),
            bare_to_global_qualified: HashMap::new(),
        };
        checker.register_builtin_types();
        checker.register_builtin_aliases();
        checker.register_builtin_values();
        checker
    }

    pub(super) fn set_global_type_info(
        &mut self,
        type_constructors: HashMap<String, Kind>,
        aliases: HashMap<String, AliasInfo>,
        opaque_types: HashMap<String, String>,
    ) {
        self.global_type_constructors = type_constructors;
        self.global_aliases = aliases;
        self.global_opaque_types = opaque_types;

        // Build bare→qualified fallback map from all qualified names in the global type universe.
        // Only maps bare names that are UNIQUE across all modules so we never silently pick the
        // wrong qualified name for an ambiguous bare identifier.
        // Skips builtin type names to avoid user-defined types with the same name (e.g. a custom
        // `Unit` type) accidentally overriding the builtin `Unit`, `Bool`, `Int`, etc.
        let mut counts: HashMap<&str, usize> = HashMap::new();
        for key in self.global_type_constructors.keys() {
            if key.contains('.') {
                if let Some(bare) = key.rsplit('.').next() {
                    if !self.builtin_types.contains_key(bare) {
                        *counts.entry(bare).or_insert(0) += 1;
                    }
                }
            }
        }
        let mut bare_to_qualified: HashMap<String, String> = HashMap::new();
        for key in self.global_type_constructors.keys() {
            if key.contains('.') {
                if let Some(bare) = key.rsplit('.').next() {
                    if !self.builtin_types.contains_key(bare)
                        && counts.get(bare).copied().unwrap_or(0) == 1
                    {
                        bare_to_qualified.insert(bare.to_string(), key.clone());
                    }
                }
            }
        }
        self.bare_to_global_qualified = bare_to_qualified;
    }

    pub(super) fn reset_module_context(&mut self, _module: &Module) {
        self.subst.clear();
        self.var_names.clear();
        self.type_constructors = self.builtin_type_constructors();
        self.aliases.clear();
        self.type_name_bindings.clear();
        self.type_display_names.clear();
        for name in self.builtin_types.keys() {
            self.type_name_bindings.insert(name.clone(), name.clone());
            self.type_display_names.insert(name.clone(), name.clone());
        }
        // Override bare→bare entries for types that have a unique qualified name in the global
        // type universe (e.g. "ZonedDateTime" → "aivi.chronos.timezone.ZonedDateTime").  This
        // ensures module-declared type names consistently resolve to their canonical qualified
        // form even without an explicit import.  Per-module imports registered by
        // `register_imported_type_names` may further override these defaults.
        for (bare, qualified) in &self.bare_to_global_qualified.clone() {
            self.type_name_bindings.insert(bare.clone(), qualified.clone());
            // Keep the short bare name as the display name for error messages.
            self.type_display_names.insert(qualified.clone(), bare.clone());
        }
        self.register_builtin_aliases();
        self.type_constructors
            .extend(self.global_type_constructors.clone());
        self.aliases.extend(self.global_aliases.clone());
        self.checked_defs.clear();
        self.classes.clear();
        self.instances.clear();
        self.method_to_classes.clear();
        self.assumed_class_constraints.clear();
        self.extra_diagnostics.clear();
        self.adt_constructors.clear();
        self.opaque_types = self.global_opaque_types.clone();
        self.enabled_record_default_types = Self::collect_enabled_record_default_types(_module);
        self.current_module_path = _module.path.clone();
        self.current_module_name = _module.name.name.clone();
        self.constraints = ConstraintState::default();
        self.query_cache.clear_module(&_module.name.name);
        self.poly_instantiations.clear();
        self.span_types.clear();
    }

    fn collect_enabled_record_default_types(module: &Module) -> HashSet<String> {
        let mut enabled = HashSet::new();
        for use_decl in &module.uses {
            if use_decl.module.name != "aivi.defaults" {
                continue;
            }
            if use_decl.wildcard {
                enabled.extend(
                    [
                        "Bool",
                        "Int",
                        "Float",
                        "Text",
                        "List",
                        "Option",
                        "ToDefault",
                    ]
                    .into_iter()
                    .map(|name| name.to_string()),
                );
                continue;
            }
            for item in &use_decl.items {
                if item.kind == crate::surface::ScopeItemKind::Value {
                    enabled.insert(item.name.name.clone());
                }
            }
        }
        enabled
    }

    fn record_default_enabled(&self, marker: &str) -> bool {
        self.enabled_record_default_types.contains(marker)
    }

    /// Returns `Some(defining_module)` if `type_name` is opaque and the current module
    /// is **not** the defining module. Returns `None` if the type is not opaque or if
    /// the current module *is* the defining module (transparent access).
    pub(super) fn is_opaque_from_here(&self, type_name: &str) -> Option<&String> {
        self.opaque_types
            .get(type_name)
            .filter(|defining_module| *defining_module != &self.current_module_name)
    }

    /// Extract the top-level type constructor name from a (possibly applied) type,
    /// following type variables through substitution.
    pub(super) fn opaque_con_name(&self, ty: &Type) -> Option<String> {
        match ty {
            Type::Con(name, _) => Some(name.clone()),
            Type::Var(id) => {
                if let Some(resolved) = self.subst.get(id) {
                    self.opaque_con_name(resolved)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Returns true if the pattern attempts to destructure the value's internal structure
    /// (record fields, constructor args, tuple elements, list elements).
    /// Wildcards and plain ident binders are non-destructuring.
    pub(super) fn pattern_destructures(&self, pat: &Pattern) -> bool {
        match pat {
            Pattern::Wildcard(_) | Pattern::Ident(_) | Pattern::SubjectIdent(_) => false,
            Pattern::Literal(_) => false,
            Pattern::At { pattern, .. } => self.pattern_destructures(pattern),
            Pattern::Constructor { .. }
            | Pattern::Tuple { .. }
            | Pattern::List { .. }
            | Pattern::Record { .. } => true,
        }
    }

    /// Drain the recorded polymorphic call-site instantiations for the current module.
    pub(super) fn take_poly_instantiations(&mut self) -> Vec<(String, Type)> {
        std::mem::take(&mut self.poly_instantiations)
    }

    /// Drain `load` call-site source schemas for the current module.
    pub(super) fn take_load_source_schemas(&mut self) -> Vec<(String, String, CgType)> {
        std::mem::take(&mut self.load_source_schemas)
    }

    /// Drain the recorded span→type pairs for the current module, applying final substitutions
    /// and rendering types as strings.
    pub(super) fn take_span_types(&mut self) -> Vec<(Span, String)> {
        let entries = std::mem::take(&mut self.span_types);
        entries
            .into_iter()
            .map(|(span, ty)| {
                let applied = self.apply(ty);
                let rendered = self.type_to_string(&applied);
                (span, rendered)
            })
            .collect()
    }

    pub(super) fn resolve_type_binding(&self, name: &str) -> Option<&str> {
        self.type_name_bindings.get(name).map(|s| s.as_str())
    }

    pub(super) fn type_kind(&self, name: &str) -> Option<&Kind> {
        self.type_constructors.get(name)
    }

    pub(super) fn resolved_type_name<'a>(&'a self, type_name: &'a str) -> &'a str {
        self.resolve_type_binding(type_name).unwrap_or(type_name)
    }

    pub(super) fn type_name_matches(&self, actual: &str, expected_source_name: &str) -> bool {
        actual == expected_source_name
            || self
                .resolve_type_binding(expected_source_name)
                .is_some_and(|resolved| actual == resolved)
            || self
                .type_display_names
                .get(actual)
                .is_some_and(|display| display == expected_source_name)
    }

    pub(super) fn named_type(&self, type_name: &str) -> Type {
        Type::con(self.resolved_type_name(type_name))
    }

    fn rewrite_type_names(&self, ty: &Type) -> Type {
        match ty {
            Type::Var(var) => Type::Var(*var),
            Type::Con(name, args) => {
                let rewritten_args = args.iter().map(|arg| self.rewrite_type_names(arg)).collect();
                let resolved = if name.contains('.') {
                    name.as_str()
                } else {
                    self.resolved_type_name(name)
                };
                Type::Con(resolved.to_string(), rewritten_args)
            }
            Type::App(base, args) => Type::App(
                Box::new(self.rewrite_type_names(base)),
                args.iter().map(|arg| self.rewrite_type_names(arg)).collect(),
            ),
            Type::Func(arg, ret) => Type::Func(
                Box::new(self.rewrite_type_names(arg)),
                Box::new(self.rewrite_type_names(ret)),
            ),
            Type::Tuple(items) => {
                Type::Tuple(items.iter().map(|item| self.rewrite_type_names(item)).collect())
            }
            Type::Record { fields } => Type::Record {
                fields: fields
                    .iter()
                    .map(|(name, ty)| (name.clone(), self.rewrite_type_names(ty)))
                    .collect(),
            },
        }
    }

    pub(super) fn rewrite_env_type_names(&self, env: &mut TypeEnv) {
        env.map_schemes(|scheme| {
            let new_ty = self.rewrite_type_names(&scheme.ty);
            Scheme { vars: scheme.vars.clone(), ty: new_ty, origin: scheme.origin.clone() }
        });
    }

    pub(super) fn alias_info_for_name(&self, name: &str) -> Option<&AliasInfo> {
        self.aliases.get(name)
    }

    pub(super) fn opaque_origin_for_name(&self, name: &str) -> Option<&String> {
        self.opaque_types.get(name)
    }

    pub(super) fn scheme_to_string(&mut self, scheme: &Scheme) -> String {
        self.type_to_string(&scheme.ty)
    }

    /// Clear accumulated type-variable state after checking a single def.
    ///
    /// After generalization, all type variables generated for that def are dead: they are either
    /// bound (quantified) in the resulting scheme or resolved to concrete types. Clearing the
    /// substitution map here prevents quadratic blow-up in `env.free_vars` / `apply` calls when
    /// many defs are checked sequentially in a large module.
    ///
    /// Only call this when span_types are NOT needed (i.e. the `aivi run` path, not LSP).
    pub(super) fn compact_after_def(&mut self) {
        self.subst.clear();
        self.span_types.clear();
    }

    fn emit_extra_diag(
        &mut self,
        code: &str,
        severity: crate::diagnostics::DiagnosticSeverity,
        message: String,
        span: Span,
    ) {
        self.extra_diagnostics.push(FileDiagnostic {
            path: self.current_module_path.clone(),
            diagnostic: Diagnostic {
                code: code.to_string(),
                severity,
                message,
                span,
                labels: Vec::new(),
                hints: Vec::new(),
                suggestion: None,
            },
        });
    }

    pub(super) fn set_class_env(
        &mut self,
        classes: HashMap<String, ClassDeclInfo>,
        instances: Vec<InstanceDeclInfo>,
    ) {
        self.classes = classes;
        self.instances = instances;
        self.method_to_classes.clear();
        for (class_name, class_info) in &self.classes {
            for member_name in class_info.direct_members.keys() {
                self.method_to_classes
                    .entry(member_name.clone())
                    .or_default()
                    .push(class_name.clone());
            }
        }
    }

    #[cfg(any())]
    fn register_builtin_types(&mut self) {
        let star = Kind::Star;
        let arrow = |a, b| Kind::Arrow(Box::new(a), Box::new(b));

        for name in [
            "Unit",
            "Bool",
            "Int",
            "Float",
            "Text",
            "Html",
            "DateTime",
            "FileHandle",
            "Send",
            "Recv",
            "Closed",
            "Date",
            "Time",
            "Duration",
            "Decimal",
            "BigInt",
            "TimeZone",
            "ZonedDateTime",
            "Generator", // Generator might be higher kinded? treating as Star for now or check spec.
        ] {
            self.builtin_types.insert(name.to_string(), star.clone());
        }

        // Higher kinded types
        // List: * -> *
        self.builtin_types
            .insert("List".to_string(), arrow(star.clone(), star.clone()));
        // Option: * -> *
        self.builtin_types
            .insert("Option".to_string(), arrow(star.clone(), star.clone()));
        // Resource: * -> *
        self.builtin_types
            .insert("Resource".to_string(), arrow(star.clone(), star.clone()));

        // Result: * -> * -> *
        self.builtin_types.insert(
            "Result".to_string(),
            arrow(star.clone(), arrow(star.clone(), star.clone())),
        );
        // Effect: * -> * -> *
        self.builtin_types.insert(
            "Effect".to_string(),
            arrow(star.clone(), arrow(star.clone(), star.clone())),
        );

        // Validation: * -> * -> *
        self.builtin_types.insert(
            "Validation".to_string(),
            arrow(star.clone(), arrow(star.clone(), star.clone())),
        );

        self.type_constructors = self.builtin_types.clone();
    }

    #[cfg(any())]
    fn builtin_type_constructors(&self) -> HashMap<String, Kind> {
        self.builtin_types.clone()
    }

    #[cfg(any())]
    fn register_builtin_values(&mut self) {
        let mut env = TypeEnv::default();
        env.insert("Unit".to_string(), Scheme::mono(Type::con("Unit")));
        env.insert("True".to_string(), Scheme::mono(Type::con("Bool")));
        env.insert("False".to_string(), Scheme::mono(Type::con("Bool")));

        let a = self.fresh_var_id();
        env.insert(
            "None".to_string(),
            Scheme {
                vars: vec![a],
                ty: Type::con("Option").app(vec![Type::Var(a)]),
            },
        );
        let a = self.fresh_var_id();
        env.insert(
            "Some".to_string(),
            Scheme {
                vars: vec![a],
                ty: Type::Func(
                    Box::new(Type::Var(a)),
                    Box::new(Type::con("Option").app(vec![Type::Var(a)])),
                ),
            },
        );

        let e = self.fresh_var_id();
        let a = self.fresh_var_id();
        env.insert(
            "Ok".to_string(),
            Scheme {
                vars: vec![e, a],
                ty: Type::Func(
                    Box::new(Type::Var(a)),
                    Box::new(Type::con("Result").app(vec![Type::Var(e), Type::Var(a)])),
                ),
            },
        );
        let e = self.fresh_var_id();
        let a = self.fresh_var_id();
        env.insert(
            "Err".to_string(),
            Scheme {
                vars: vec![e, a],
                ty: Type::Func(
                    Box::new(Type::Var(e)),
                    Box::new(Type::con("Result").app(vec![Type::Var(e), Type::Var(a)])),
                ),
            },
        );
        env.insert("Closed".to_string(), Scheme::mono(Type::con("Closed")));
        let a = self.fresh_var_id();
        env.insert(
            "constructorName".to_string(),
            Scheme {
                vars: vec![a],
                ty: Type::Func(Box::new(Type::Var(a)), Box::new(Type::con("Text"))),
            },
        );
        let a = self.fresh_var_id();
        env.insert(
            "constructorOrdinal".to_string(),
            Scheme {
                vars: vec![a],
                ty: Type::Func(Box::new(Type::Var(a)), Box::new(Type::con("Int"))),
            },
        );

        let a = self.fresh_var_id();
        let e = self.fresh_var_id();
        env.insert(
            "pure".to_string(),
            Scheme {
                vars: vec![e, a],
                ty: Type::Func(
                    Box::new(Type::Var(a)),
                    Box::new(Type::con("Effect").app(vec![Type::Var(e), Type::Var(a)])),
                ),
            },
        );
        let a = self.fresh_var_id();
        let e = self.fresh_var_id();
        env.insert(
            "fail".to_string(),
            Scheme {
                vars: vec![e, a],
                ty: Type::Func(
                    Box::new(Type::Var(e)),
                    Box::new(Type::con("Effect").app(vec![Type::Var(e), Type::Var(a)])),
                ),
            },
        );
        let a = self.fresh_var_id();
        let e = self.fresh_var_id();
        let f = self.fresh_var_id();
        env.insert(
            "attempt".to_string(),
            Scheme {
                vars: vec![e, f, a],
                ty: Type::Func(
                    Box::new(Type::con("Effect").app(vec![Type::Var(e), Type::Var(a)])),
                    Box::new(Type::con("Effect").app(vec![
                        Type::Var(f),
                        Type::con("Result").app(vec![Type::Var(e), Type::Var(a)]),
                    ])),
                ),
            },
        );

        env.insert(
            "print".to_string(),
            Scheme::mono(Type::Func(
                Box::new(Type::con("Text")),
                Box::new(Type::con("Effect").app(vec![Type::con("Text"), Type::con("Unit")])),
            )),
        );
        env.insert(
            "println".to_string(),
            Scheme::mono(Type::Func(
                Box::new(Type::con("Text")),
                Box::new(Type::con("Effect").app(vec![Type::con("Text"), Type::con("Unit")])),
            )),
        );

        let e = self.fresh_var_id();
        let a = self.fresh_var_id();
        env.insert(
            "load".to_string(),
            Scheme {
                vars: vec![e, a],
                ty: Type::Func(
                    Box::new(Type::con("Effect").app(vec![Type::Var(e), Type::Var(a)])),
                    Box::new(Type::con("Effect").app(vec![Type::Var(e), Type::Var(a)])),
                ),
            },
        );

        let file_record = Type::Record {
            fields: vec![
                (
                    "read".to_string(),
                    Type::Func(
                        Box::new(Type::con("Text")),
                        Box::new(
                            Type::con("Effect").app(vec![Type::con("Text"), Type::con("Text")]),
                        ),
                    ),
                ),
                (
                    "open".to_string(),
                    Type::Func(
                        Box::new(Type::con("Text")),
                        Box::new(
                            Type::con("Effect")
                                .app(vec![Type::con("Text"), Type::con("FileHandle")]),
                        ),
                    ),
                ),
                (
                    "close".to_string(),
                    Type::Func(
                        Box::new(Type::con("FileHandle")),
                        Box::new(
                            Type::con("Effect").app(vec![Type::con("Text"), Type::con("Unit")]),
                        ),
                    ),
                ),
                (
                    "readAll".to_string(),
                    Type::Func(
                        Box::new(Type::con("FileHandle")),
                        Box::new(
                            Type::con("Effect").app(vec![Type::con("Text"), Type::con("Text")]),
                        ),
                    ),
                ),
            ]
            .into_iter()
            .collect(),
        };
        env.insert("file".to_string(), Scheme::mono(file_record));

        let a = self.fresh_var_id();
        let send_ty = Type::con("Send").app(vec![Type::Var(a)]);
        let recv_ty = Type::con("Recv").app(vec![Type::Var(a)]);
        let channel_record = Type::Record {
            fields: vec![
                (
                    "make".to_string(),
                    Type::Func(
                        Box::new(Type::con("Unit")),
                        Box::new(Type::con("Effect").app(vec![
                            Type::con("Closed"),
                            Type::Tuple(vec![send_ty.clone(), recv_ty.clone()]),
                        ])),
                    ),
                ),
                (
                    "send".to_string(),
                    Type::Func(
                        Box::new(send_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(Type::Var(a)),
                            Box::new(
                                Type::con("Effect")
                                    .app(vec![Type::con("Closed"), Type::con("Unit")]),
                            ),
                        )),
                    ),
                ),
                (
                    "recv".to_string(),
                    Type::Func(
                        Box::new(recv_ty.clone()),
                        Box::new(Type::con("Effect").app(vec![
                            Type::con("Closed"),
                            Type::con("Result").app(vec![Type::con("Closed"), Type::Var(a)]),
                        ])),
                    ),
                ),
                (
                    "close".to_string(),
                    Type::Func(
                        Box::new(send_ty),
                        Box::new(
                            Type::con("Effect").app(vec![Type::con("Closed"), Type::con("Unit")]),
                        ),
                    ),
                ),
            ]
            .into_iter()
            .collect(),
        };
        env.insert("channel".to_string(), Scheme::mono(channel_record));

        let e = self.fresh_var_id();
        let a = self.fresh_var_id();
        let b = self.fresh_var_id();
        let concurrent_record = Type::Record {
            fields: vec![
                (
                    "scope".to_string(),
                    Type::Func(
                        Box::new(Type::con("Effect").app(vec![Type::Var(e), Type::Var(a)])),
                        Box::new(Type::con("Effect").app(vec![Type::Var(e), Type::Var(a)])),
                    ),
                ),
                (
                    "par".to_string(),
                    Type::Func(
                        Box::new(Type::con("Effect").app(vec![Type::Var(e), Type::Var(a)])),
                        Box::new(Type::Func(
                            Box::new(Type::con("Effect").app(vec![Type::Var(e), Type::Var(b)])),
                            Box::new(Type::con("Effect").app(vec![
                                Type::Var(e),
                                Type::Tuple(vec![Type::Var(a), Type::Var(b)]),
                            ])),
                        )),
                    ),
                ),
                (
                    "race".to_string(),
                    Type::Func(
                        Box::new(Type::con("Effect").app(vec![Type::Var(e), Type::Var(a)])),
                        Box::new(Type::Func(
                            Box::new(Type::con("Effect").app(vec![Type::Var(e), Type::Var(a)])),
                            Box::new(Type::con("Effect").app(vec![Type::Var(e), Type::Var(a)])),
                        )),
                    ),
                ),
                (
                    "spawnDetached".to_string(),
                    Type::Func(
                        Box::new(Type::con("Effect").app(vec![Type::Var(e), Type::Var(a)])),
                        Box::new(Type::con("Effect").app(vec![Type::Var(e), Type::con("Unit")])),
                    ),
                ),
                (
                    "fork".to_string(),
                    Type::Func(
                        Box::new(Type::con("Effect").app(vec![Type::Var(e), Type::Var(a)])),
                        Box::new(Type::con("Effect").app(vec![
                            Type::Var(e),
                            Type::Record {
                                fields: vec![
                                    (
                                        "join".to_string(),
                                        Type::con("Effect")
                                            .app(vec![Type::Var(e), Type::Var(a)]),
                                    ),
                                    (
                                        "cancel".to_string(),
                                        Type::con("Effect")
                                            .app(vec![Type::Var(e), Type::con("Unit")]),
                                    ),
                                    (
                                        "isCancelled".to_string(),
                                        Type::con("Effect")
                                            .app(vec![Type::Var(e), Type::con("Bool")]),
                                    ),
                                ]
                                .into_iter()
                                .collect(),
                            },
                        ])),
                    ),
                ),
                (
                    "retry".to_string(),
                    Type::Func(
                        Box::new(Type::con("Int")),
                        Box::new(Type::Func(
                            Box::new(Type::con("Effect").app(vec![Type::Var(e), Type::Var(a)])),
                            Box::new(Type::con("Effect").app(vec![Type::Var(e), Type::Var(a)])),
                        )),
                    ),
                ),
            ]
            .into_iter()
            .collect(),
        };
        env.insert("concurrent".to_string(), Scheme::mono(concurrent_record));

        let clock_record = Type::Record {
            fields: vec![(
                "now".to_string(),
                Type::Func(
                    Box::new(Type::con("Unit")),
                    Box::new(
                        Type::con("Effect").app(vec![Type::con("Text"), Type::con("DateTime")]),
                    ),
                ),
            )]
            .into_iter()
            .collect(),
        };
        env.insert("clock".to_string(), Scheme::mono(clock_record));

        let random_record = Type::Record {
            fields: vec![(
                "int".to_string(),
                Type::Func(
                    Box::new(Type::con("Int")),
                    Box::new(Type::Func(
                        Box::new(Type::con("Int")),
                        Box::new(
                            Type::con("Effect").app(vec![Type::con("Text"), Type::con("Int")]),
                        ),
                    )),
                ),
            )]
            .into_iter()
            .collect(),
        };
        env.insert("random".to_string(), Scheme::mono(random_record));

        let html_record = Type::Record {
            fields: vec![(
                "render".to_string(),
                Type::Func(Box::new(Type::con("Html")), Box::new(Type::con("Text"))),
            )]
            .into_iter()
            .collect(),
        };
        env.insert("html".to_string(), Scheme::mono(html_record));

        self.builtins = env;
    }
}
