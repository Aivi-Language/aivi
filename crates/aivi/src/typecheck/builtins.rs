use std::collections::HashSet;

use super::{Scheme, Type, TypeChecker, TypeEnv};

impl TypeChecker {
    pub(super) fn register_builtin_types(&mut self) {
        for name in [
            "Unit",
            "Bool",
            "Int",
            "Float",
            "Text",
            "List",
            "Option",
            "Result",
            "Effect",
            "Resource",
            "Generator",
            "Html",
            "DateTime",
            "FileHandle",
            "Send",
            "Recv",
            "Closed",
        ] {
            self.builtin_types.insert(name.to_string());
        }
        self.type_constructors = self.builtin_types.clone();
    }

    pub(super) fn builtin_type_constructors(&self) -> HashSet<String> {
        self.builtin_types.clone()
    }

    pub(super) fn register_builtin_values(&mut self) {
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
            open: true,
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
            open: true,
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
            open: true,
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
            open: true,
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
            open: true,
        };
        env.insert("random".to_string(), Scheme::mono(random_record));

        let html_record = Type::Record {
            fields: vec![(
                "render".to_string(),
                Type::Func(Box::new(Type::con("Html")), Box::new(Type::con("Text"))),
            )]
            .into_iter()
            .collect(),
            open: true,
        };
        env.insert("html".to_string(), Scheme::mono(html_record));

        self.builtins = env;
    }
}
