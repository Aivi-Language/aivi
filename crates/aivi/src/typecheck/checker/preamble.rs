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
    extra_diagnostics: Vec<FileDiagnostic>,
    adt_constructors: HashMap<String, Vec<String>>,
    pub(super) constraints: ConstraintState,
    pub(super) query_cache: TypeQueryCache,
}

impl TypeChecker {
    pub(super) fn new() -> Self {
        let mut checker = Self {
            next_var: 0,
            subst: HashMap::new(),
            var_names: HashMap::new(),
            type_constructors: HashMap::new(),
            aliases: HashMap::new(),
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
            extra_diagnostics: Vec::new(),
            adt_constructors: HashMap::new(),
            constraints: ConstraintState::default(),
            query_cache: TypeQueryCache::default(),
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
    ) {
        self.global_type_constructors = type_constructors;
        self.global_aliases = aliases;
    }

    pub(super) fn reset_module_context(&mut self, _module: &Module) {
        self.subst.clear();
        self.var_names.clear();
        self.type_constructors = self.builtin_type_constructors();
        self.aliases.clear();
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
        self.current_module_path = _module.path.clone();
        self.constraints = ConstraintState::default();
        self.query_cache.clear_module(&_module.name.name);
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
        env.insert(
            "attempt".to_string(),
            Scheme {
                vars: vec![e, a],
                ty: Type::Func(
                    Box::new(Type::con("Effect").app(vec![Type::Var(e), Type::Var(a)])),
                    Box::new(Type::con("Effect").app(vec![
                        Type::Var(e),
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
