#[test]
fn unit_literals_reject_malformed_non_unit_backend_layouts() {
    let mut backend = lower_text(
        "malformed-unit.aivi",
        "value unit : Unit = ()\nvalue number = 7\n",
    );
    let int_layout = backend
        .layouts()
        .iter()
        .find_map(|(id, layout)| {
            matches!(
                layout.kind,
                LayoutKind::Primitive(aivi_backend::PrimitiveType::Int)
            )
            .then_some(id)
        })
        .unwrap();
    let (kernel_id, expr_id) = backend
        .kernels()
        .iter()
        .find_map(|(id, kernel)| {
            kernel.exprs().iter().find_map(|(expr_id, expr)| {
                matches!(
                    expr.kind,
                    KernelExprKind::Builtin(aivi_backend::BuiltinTerm::Unit)
                )
                .then_some((id, expr_id))
            })
        })
        .unwrap();
    backend
        .kernels_mut()
        .get_mut(kernel_id)
        .unwrap()
        .exprs_mut()
        .get_mut(expr_id)
        .unwrap()
        .layout = int_layout;
    assert!(
        validate_program(&backend)
            .unwrap_err()
            .errors()
            .iter()
            .any(|error| matches!(
                error,
                aivi_backend::ValidationError::UnitLiteralLayoutMismatch { .. }
            ))
    );
    assert!(compile_program(&backend).is_err());
}

#[test]
fn option_pure_uses_constructor_native_contracts() {
    let backend = lower_text(
        "option-pure.aivi",
        r#"
value unit : Option Unit = pure ()
value number : Option Int = pure 7
value decimal : Option Float = pure 2.5
value flag : Option Bool = pure True
value text : Option Text = pure "done"
"#,
    );
    let executable = aivi_backend::BackendExecutableProgram::interpreted(&backend);
    let mut engine = executable.create_engine();
    let mut evaluator = KernelEvaluator::new(&backend);
    for (name, payload) in [
        ("unit", RuntimeValue::Unit),
        ("number", RuntimeValue::Int(7)),
        (
            "decimal",
            RuntimeValue::Float(RuntimeFloat::parse_literal("2.5").unwrap()),
        ),
        ("flag", RuntimeValue::Bool(true)),
        ("text", RuntimeValue::Text("done".into())),
    ] {
        let expected = RuntimeValue::OptionSome(Box::new(payload));
        let item = find_item(&backend, name);
        assert_eq!(
            evaluator.evaluate_item(item, &BTreeMap::new()).unwrap(),
            expected,
            "interpreted {name}"
        );
        assert_eq!(
            engine.evaluate_item(item, &BTreeMap::new()).unwrap(),
            expected,
            "native {name}"
        );
    }
    compile_program(&backend).expect("Option pure kernels compile to native object code");
}

#[test]
fn unit_literals_patterns_and_aggregates_execute_in_both_engines() {
    let backend = lower_text(
        "unit.aivi",
        r#"
value literal : Unit = ()
type Unit -> Int
func inspect = unit => unit ||> () -> 7
type A -> A
func identity = value => value
value applied : Unit = identity ()
value observed : Int = inspect ()
value aggregate : (Unit, Int) = ((), 9)
value record : { done: Unit, number: Int } = { done: (), number: 9 }
value optional : Option Unit = Some ()
type Option Unit -> Option Unit
func keepOption = option => option
value optionalCall : Option Unit = keepOption (Some ())
type Option Unit -> Int
func countOption = option => option
 ||> Some () -> 1
 ||> None -> 0
value presentCount : Int = countOption (Some ())
value absentCount : Int = countOption None
value units : List Unit = [(), ()]
value singleton : Bool = () == ()
value unequal : Bool = () != ()
value rendered : Text = "unit={()}"
"#,
    );
    let executable = aivi_backend::BackendExecutableProgram::interpreted(&backend);
    let mut engine = executable.create_engine();
    let mut evaluator = KernelEvaluator::new(&backend);
    for (name, expected) in [
        ("literal", RuntimeValue::Unit),
        ("applied", RuntimeValue::Unit),
        ("observed", RuntimeValue::Int(7)),
        (
            "aggregate",
            RuntimeValue::Tuple(vec![RuntimeValue::Unit, RuntimeValue::Int(9)]),
        ),
        (
            "optional",
            RuntimeValue::OptionSome(Box::new(RuntimeValue::Unit)),
        ),
        (
            "optionalCall",
            RuntimeValue::OptionSome(Box::new(RuntimeValue::Unit)),
        ),
        ("presentCount", RuntimeValue::Int(1)),
        ("absentCount", RuntimeValue::Int(0)),
        (
            "units",
            RuntimeValue::List(vec![RuntimeValue::Unit, RuntimeValue::Unit]),
        ),
        ("singleton", RuntimeValue::Bool(true)),
        ("unequal", RuntimeValue::Bool(false)),
        ("rendered", RuntimeValue::Text("unit=()".into())),
    ] {
        let item = find_item(&backend, name);
        assert_eq!(
            evaluator.evaluate_item(item, &BTreeMap::new()).unwrap(),
            expected,
            "interpreted {name}"
        );
        assert_eq!(
            engine.evaluate_item(item, &BTreeMap::new()).unwrap(),
            expected,
            "native {name}"
        );
    }
    assert_eq!(
        evaluator
            .evaluate_item(find_item(&backend, "record"), &BTreeMap::new())
            .unwrap(),
        engine
            .evaluate_item(find_item(&backend, "record"), &BTreeMap::new())
            .unwrap()
    );
    compile_program(&backend).expect("Unit kernels compile to native object code");
}

#[test]
fn cranelift_codegen_compiles_scalar_gate_kernels() {
    let core = manual_core_gate_stage(
        CoreType::Primitive(BuiltinType::Int),
        CoreType::Primitive(BuiltinType::Bool),
        |module, span| {
            let subject = module
                .exprs_mut()
                .alloc(CoreExpr {
                    span,
                    ty: CoreType::Primitive(BuiltinType::Int),
                    kind: CoreExprKind::AmbientSubject,
                })
                .expect("subject allocation should fit");
            let one = module
                .exprs_mut()
                .alloc(CoreExpr {
                    span,
                    ty: CoreType::Primitive(BuiltinType::Int),
                    kind: CoreExprKind::Integer(IntegerLiteral { raw: "1".into() }),
                })
                .expect("integer allocation should fit");
            module
                .exprs_mut()
                .alloc(CoreExpr {
                    span,
                    ty: CoreType::Primitive(BuiltinType::Bool),
                    kind: CoreExprKind::Binary {
                        left: subject,
                        operator: HirBinaryOperator::GreaterThan,
                        right: one,
                    },
                })
                .expect("comparison allocation should fit")
        },
        |module, span| {
            module
                .exprs_mut()
                .alloc(CoreExpr {
                    span,
                    ty: CoreType::Primitive(BuiltinType::Bool),
                    kind: CoreExprKind::Reference(CoreReference::Builtin(HirBuiltinTerm::False)),
                })
                .expect("builtin allocation should fit")
        },
    );
    validate_core_module(&core).expect("manual core module should validate");
    let lambda = lower_lambda_module(&core).expect("typed lambda lowering should succeed");
    validate_lambda_module(&lambda).expect("typed lambda should validate");
    let backend = lower_backend_module(&lambda).expect("backend lowering should succeed");
    validate_program(&backend).expect("backend program should validate");

    let item = find_item(&backend, "captured");
    let pipeline = &backend.pipelines()[first_pipeline(&backend, item)];
    let BackendStageKind::Gate(BackendGateStage::Ordinary { when_true, .. }) =
        &pipeline.stages[0].kind
    else {
        panic!("expected ordinary gate stage");
    };

    let compiled = compile_program(&backend).expect("Cranelift codegen should succeed");
    let artifact = compiled
        .kernel(*when_true)
        .expect("compiled program should retain per-kernel metadata");
    assert!(artifact.code_size > 0);
    assert!(artifact.symbol.contains("gate_true"));
    assert!(artifact.clif.contains("icmp sgt"));
    assert!(artifact.clif.contains("(i64) -> i8"));
    assert!(!compiled.object().is_empty());
}

#[test]
fn cranelift_codegen_compiles_real_gate_carrier_kernels() {
    let ptr = clif_pointer_ty();

    let user_type = CoreType::Record(vec![
        CoreRecordField {
            name: "active".into(),
            ty: CoreType::Primitive(BuiltinType::Bool),
        },
        CoreRecordField {
            name: "email".into(),
            ty: CoreType::Primitive(BuiltinType::Text),
        },
    ]);
    let option_user = CoreType::Option(Box::new(user_type.clone()));
    let ordinary_core = manual_core_gate_stage(
        user_type.clone(),
        option_user.clone(),
        {
            let option_user = option_user.clone();
            let user_type = user_type.clone();
            move |module, span| {
                let subject = module
                    .exprs_mut()
                    .alloc(CoreExpr {
                        span,
                        ty: user_type.clone(),
                        kind: CoreExprKind::AmbientSubject,
                    })
                    .expect("subject allocation should fit");
                module
                    .exprs_mut()
                    .alloc(CoreExpr {
                        span,
                        ty: option_user.clone(),
                        kind: CoreExprKind::OptionSome { payload: subject },
                    })
                    .expect("some allocation should fit")
            }
        },
        {
            let option_user = option_user.clone();
            move |module, span| {
                module
                    .exprs_mut()
                    .alloc(CoreExpr {
                        span,
                        ty: option_user.clone(),
                        kind: CoreExprKind::OptionNone,
                    })
                    .expect("none allocation should fit")
            }
        },
    );
    validate_core_module(&ordinary_core).expect("manual core module should validate");
    let ordinary_lambda =
        lower_lambda_module(&ordinary_core).expect("typed lambda lowering should succeed");
    validate_lambda_module(&ordinary_lambda).expect("typed lambda should validate");
    let ordinary_backend =
        lower_backend_module(&ordinary_lambda).expect("backend lowering should succeed");
    validate_program(&ordinary_backend).expect("backend program should validate");

    let ordinary_item = find_item(&ordinary_backend, "captured");
    let (when_true, when_false) = match &ordinary_backend.pipelines()
        [first_pipeline(&ordinary_backend, ordinary_item)]
    .stages[0]
        .kind
    {
        BackendStageKind::Gate(BackendGateStage::Ordinary {
            when_true,
            when_false,
        }) => (*when_true, *when_false),
        other => panic!("expected ordinary gate stage, found {other:?}"),
    };
    let ordinary_compiled = compile_program(&ordinary_backend)
        .expect("record projection and Option carriers should compile");

    let gate_true = ordinary_compiled
        .kernel(when_true)
        .expect("gate-true artifact should exist");
    assert!(gate_true.code_size > 0);
    assert!(gate_true.clif.contains(&format!("({ptr}) -> {ptr}")));

    let gate_false = ordinary_compiled
        .kernel(when_false)
        .expect("gate-false artifact should exist");
    assert!(gate_false.code_size > 0);
    assert!(gate_false.clif.contains(&format!("iconst.{ptr} 0")));

    let signal_filter_core = manual_core_signal_filter_stage(
        user_type.clone(),
        user_type.clone(),
        user_type.clone(),
        move |module, span| {
            module
                .exprs_mut()
                .alloc(CoreExpr {
                    span,
                    ty: CoreType::Primitive(BuiltinType::Bool),
                    kind: CoreExprKind::Projection {
                        base: CoreProjectionBase::AmbientSubject,
                        path: vec!["active".into()],
                    },
                })
                .expect("projection allocation should fit")
        },
    );
    validate_core_module(&signal_filter_core).expect("manual core module should validate");
    let signal_filter_lambda =
        lower_lambda_module(&signal_filter_core).expect("typed lambda lowering should succeed");
    validate_lambda_module(&signal_filter_lambda).expect("typed lambda should validate");
    let signal_filter_backend =
        lower_backend_module(&signal_filter_lambda).expect("backend lowering should succeed");
    validate_program(&signal_filter_backend).expect("backend program should validate");

    let filtered = find_item(&signal_filter_backend, "filtered");
    let predicate = match &signal_filter_backend.pipelines()
        [first_pipeline(&signal_filter_backend, filtered)]
    .stages[0]
        .kind
    {
        BackendStageKind::Gate(BackendGateStage::SignalFilter { predicate, .. }) => *predicate,
        other => panic!("expected signal-filter gate stage, found {other:?}"),
    };
    let signal_filter_compiled =
        compile_program(&signal_filter_backend).expect("signal-filter predicate should compile");

    let predicate_artifact = signal_filter_compiled
        .kernel(predicate)
        .expect("predicate artifact should exist");
    assert!(predicate_artifact.code_size > 0);
    assert!(predicate_artifact.clif.contains(&format!("({ptr}) -> i8")));
    assert!(predicate_artifact.clif.contains("load.i8"));
    assert!(!ordinary_compiled.object().is_empty());
    assert!(!signal_filter_compiled.object().is_empty());
}

#[test]
fn cranelift_codegen_compiles_environment_slots() {
    let core = manual_core_gate_stage(
        CoreType::Primitive(BuiltinType::Int),
        CoreType::Primitive(BuiltinType::Int),
        |module, span| {
            let captured = module
                .exprs_mut()
                .alloc(CoreExpr {
                    span,
                    ty: CoreType::Primitive(BuiltinType::Int),
                    kind: CoreExprKind::Reference(CoreReference::Local(HirBindingId::from_raw(7))),
                })
                .expect("capture allocation should fit");
            let one = module
                .exprs_mut()
                .alloc(CoreExpr {
                    span,
                    ty: CoreType::Primitive(BuiltinType::Int),
                    kind: CoreExprKind::Integer(IntegerLiteral { raw: "1".into() }),
                })
                .expect("integer allocation should fit");
            module
                .exprs_mut()
                .alloc(CoreExpr {
                    span,
                    ty: CoreType::Primitive(BuiltinType::Int),
                    kind: CoreExprKind::Binary {
                        left: captured,
                        operator: HirBinaryOperator::Add,
                        right: one,
                    },
                })
                .expect("add allocation should fit")
        },
        |module, span| {
            module
                .exprs_mut()
                .alloc(CoreExpr {
                    span,
                    ty: CoreType::Primitive(BuiltinType::Int),
                    kind: CoreExprKind::Integer(IntegerLiteral { raw: "0".into() }),
                })
                .expect("integer allocation should fit")
        },
    );
    validate_core_module(&core).expect("manual core module should validate");
    let lambda = lower_lambda_module(&core).expect("typed lambda lowering should succeed");
    validate_lambda_module(&lambda).expect("typed lambda should validate");
    let backend = lower_backend_module(&lambda).expect("backend lowering should succeed");
    validate_program(&backend).expect("backend program should validate");

    let item = find_item(&backend, "captured");
    let pipeline = &backend.pipelines()[first_pipeline(&backend, item)];
    let BackendStageKind::Gate(BackendGateStage::Ordinary { when_true, .. }) =
        &pipeline.stages[0].kind
    else {
        panic!("expected ordinary gate stage");
    };

    let compiled = compile_program(&backend).expect("Cranelift codegen should succeed");
    let artifact = compiled
        .kernel(*when_true)
        .expect("compiled program should retain per-kernel metadata");
    assert!(artifact.code_size > 0);
    assert!(artifact.clif.contains("iadd"));
    assert!(artifact.clif.contains("(i64) -> i64"));
}

#[test]
fn cranelift_codegen_compiles_noninteger_literal_gate_kernels() {
    fn compile_literal_gate(
        result_type: BuiltinType,
        when_true: CoreExprKind,
        when_false: CoreExprKind,
        expected_value: RuntimeValue,
        expected_signature: &str,
        expected_clif_fragment: Option<&str>,
    ) {
        let result_ty = CoreType::Primitive(result_type);
        let core = manual_core_gate_stage(
            CoreType::Primitive(BuiltinType::Bool),
            result_ty.clone(),
            {
                let result_ty = result_ty.clone();
                move |module, span| {
                    module
                        .exprs_mut()
                        .alloc(CoreExpr {
                            span,
                            ty: result_ty.clone(),
                            kind: when_true,
                        })
                        .expect("literal allocation should fit")
                }
            },
            move |module, span| {
                module
                    .exprs_mut()
                    .alloc(CoreExpr {
                        span,
                        ty: result_ty.clone(),
                        kind: when_false,
                    })
                    .expect("fallback literal allocation should fit")
            },
        );
        validate_core_module(&core).expect("manual core module should validate");
        let lambda = lower_lambda_module(&core).expect("typed lambda lowering should succeed");
        validate_lambda_module(&lambda).expect("typed lambda should validate");
        let backend = lower_backend_module(&lambda).expect("backend lowering should succeed");
        validate_program(&backend).expect("backend program should validate");

        let item = find_item(&backend, "captured");
        let pipeline = &backend.pipelines()[first_pipeline(&backend, item)];
        let BackendStageKind::Gate(BackendGateStage::Ordinary { when_true, .. }) =
            &pipeline.stages[0].kind
        else {
            panic!("expected ordinary gate stage");
        };

        let mut evaluator = KernelEvaluator::new(&backend);
        assert_eq!(
            evaluator
                .evaluate_kernel(*when_true, None, &[], &BTreeMap::new())
                .expect("literal kernel should evaluate"),
            expected_value
        );

        let compiled = compile_program(&backend).expect("literal gate kernels should compile");
        let artifact = compiled
            .kernel(*when_true)
            .expect("compiled program should retain per-kernel metadata");
        assert!(artifact.code_size > 0);
        assert!(artifact.clif.contains(expected_signature));
        if let Some(fragment) = expected_clif_fragment {
            assert!(artifact.clif.contains(fragment));
        }
        assert!(!compiled.object().is_empty());
    }

    compile_literal_gate(
        BuiltinType::Float,
        CoreExprKind::Float(FloatLiteral { raw: "3.14".into() }),
        CoreExprKind::Float(FloatLiteral { raw: "2.5".into() }),
        RuntimeValue::Float(RuntimeFloat::parse_literal("3.14").expect("literal should parse")),
        "() -> f64",
        Some("f64const"),
    );
    compile_literal_gate(
        BuiltinType::Decimal,
        CoreExprKind::Decimal(DecimalLiteral {
            raw: "19.25d".into(),
        }),
        CoreExprKind::Decimal(DecimalLiteral { raw: "7d".into() }),
        RuntimeValue::Decimal(
            RuntimeDecimal::parse_literal("19.25d").expect("literal should parse"),
        ),
        &format!("() -> {}", clif_pointer_ty()),
        Some("symbol_value"),
    );
    compile_literal_gate(
        BuiltinType::BigInt,
        CoreExprKind::BigInt(BigIntLiteral { raw: "123n".into() }),
        CoreExprKind::BigInt(BigIntLiteral { raw: "456n".into() }),
        RuntimeValue::BigInt(RuntimeBigInt::parse_literal("123n").expect("literal should parse")),
        &format!("() -> {}", clif_pointer_ty()),
        Some("symbol_value"),
    );
}

#[test]
fn cranelift_codegen_compiles_static_text_item_bodies() {
    let backend = lower_text(
        "backend-static-text-codegen.aivi",
        r#"
value greeting:Text = "hello"
"#,
    );

    let greeting = find_item(&backend, "greeting");
    let body = backend.items()[greeting]
        .body
        .expect("greeting should carry a body kernel");

    let compiled = compile_program(&backend).expect("static text item bodies should compile");
    let artifact = compiled
        .kernel(body)
        .expect("compiled program should retain greeting kernel metadata");
    assert!(artifact.code_size > 0);
    assert!(
        artifact
            .clif
            .contains(&format!("() -> {}", clif_pointer_ty()))
    );
    assert!(artifact.clif.contains("symbol_value"));
    assert!(!compiled.object().is_empty());
}

#[test]
fn cranelift_codegen_compiles_static_interpolated_text_item_bodies() {
    let backend = lower_text(
        "backend-static-interpolated-text-codegen.aivi",
        r#"
domain Duration over Int
    suffix ms : Int = value => Duration value

type Status =
  | Idle
  | Ready Int

value folded:Text =
    "count={7} ok={True} ratio={3.5} cost={19.25d} big={123n} dur={15ms} pair={(7, False)} list={[7, 8]} maybe={Some 7} status={Ready 9} not={not False} cmp={3 < 5} fcmp={3.5 >= 2.0} same={(Some 7) == (Some 7)} diff={(Ready 9) != (Ready 8)}"
"#,
    );

    let folded = find_item(&backend, "folded");
    let body = backend.items()[folded]
        .body
        .expect("folded should carry a body kernel");

    let mut evaluator = KernelEvaluator::new(&backend);
    assert_eq!(
        evaluator
            .evaluate_item(folded, &BTreeMap::new())
            .expect("static interpolation should evaluate"),
        RuntimeValue::Text(
            "count=7 ok=True ratio=3.5 cost=19.25d big=123n dur=15 pair=(7, False) list=[7, 8] maybe=Some 7 status=Ready 9 not=True cmp=True fcmp=True same=True diff=True".into()
        )
    );

    let compiled =
        compile_program(&backend).expect("static interpolated text item bodies should compile");
    let artifact = compiled
        .kernel(body)
        .expect("compiled program should retain folded kernel metadata");
    assert!(artifact.code_size > 0);
    assert!(
        artifact
            .clif
            .contains(&format!("() -> {}", clif_pointer_ty()))
    );
    assert!(artifact.clif.contains("symbol_value"));
    assert!(!compiled.object().is_empty());
}

#[test]
fn cranelift_codegen_compiles_static_interpolated_text_with_bytes_intrinsics() {
    let backend = lower_workspace_text(
        "milestone-2/valid/workspace-type-imports/main.aivi",
        r#"
use aivi.core.bytes (
    append,
    empty,
    get,
    length,
    repeat,
    slice,
    toText
)

value folded:Text =
    "empty={empty} len={length (append (repeat 65 1) (repeat 66 2))} get={get 1 (repeat 67 3)} slice={slice 1 3 (repeat 68 4)} text={toText (repeat 69 2)} repeat={repeat 65 3} raw={append (repeat 65 1) (repeat 66 2)}"
"#,
    );

    let folded = find_item(&backend, "folded");
    let body = backend.items()[folded]
        .body
        .expect("folded should carry a body kernel");

    let mut evaluator = KernelEvaluator::new(&backend);
    assert_eq!(
        evaluator
            .evaluate_item(folded, &BTreeMap::new())
            .expect("bytes interpolation should evaluate"),
        RuntimeValue::Text(
            "empty=<bytes:0> len=3 get=Some 67 slice=<bytes:2> text=Some EE repeat=<bytes:3> raw=<bytes:3>".into()
        )
    );

    let compiled = compile_program(&backend)
        .expect("static bytes interpolation should fold into a native text literal");
    let artifact = compiled
        .kernel(body)
        .expect("compiled program should retain folded bytes kernel metadata");
    assert!(artifact.code_size > 0);
    assert!(
        artifact
            .clif
            .contains(&format!("() -> {}", clif_pointer_ty()))
    );
    assert!(artifact.clif.contains("symbol_value"));
    assert!(!compiled.object().is_empty());
}

#[test]
fn cranelift_codegen_compiles_interpolated_text() {
    let backend = lower_text(
        "backend-interpolated-text-codegen.aivi",
        r#"
value host:Text = "api.example.com"
value url:Text = "https://{host}/users"
"#,
    );

    compile_program(&backend)
        .expect("interpolated text should now compile via runtime text concat");
}

#[test]
fn cranelift_codegen_compiles_by_reference_environment_projection() {
    let captured_user = CoreType::Record(vec![
        CoreRecordField {
            name: "active".into(),
            ty: CoreType::Primitive(BuiltinType::Bool),
        },
        CoreRecordField {
            name: "email".into(),
            ty: CoreType::Primitive(BuiltinType::Text),
        },
    ]);
    let core = manual_core_gate_stage(
        CoreType::Primitive(BuiltinType::Int),
        CoreType::Primitive(BuiltinType::Bool),
        {
            let captured_user = captured_user.clone();
            move |module, span| {
                let captured = module
                    .exprs_mut()
                    .alloc(CoreExpr {
                        span,
                        ty: captured_user.clone(),
                        kind: CoreExprKind::Reference(CoreReference::Local(
                            HirBindingId::from_raw(7),
                        )),
                    })
                    .expect("capture allocation should fit");
                module
                    .exprs_mut()
                    .alloc(CoreExpr {
                        span,
                        ty: CoreType::Primitive(BuiltinType::Bool),
                        kind: CoreExprKind::Projection {
                            base: CoreProjectionBase::Expr(captured),
                            path: vec!["active".into()],
                        },
                    })
                    .expect("projection allocation should fit")
            }
        },
        |module, span| {
            module
                .exprs_mut()
                .alloc(CoreExpr {
                    span,
                    ty: CoreType::Primitive(BuiltinType::Bool),
                    kind: CoreExprKind::Reference(CoreReference::Builtin(HirBuiltinTerm::False)),
                })
                .expect("builtin allocation should fit")
        },
    );
    validate_core_module(&core).expect("manual core module should validate");
    let lambda = lower_lambda_module(&core).expect("typed lambda lowering should succeed");
    validate_lambda_module(&lambda).expect("typed lambda should validate");
    let backend = lower_backend_module(&lambda).expect("backend lowering should succeed");
    validate_program(&backend).expect("backend program should validate");

    let item = find_item(&backend, "captured");
    let pipeline = &backend.pipelines()[first_pipeline(&backend, item)];
    let BackendStageKind::Gate(BackendGateStage::Ordinary { when_true, .. }) =
        &pipeline.stages[0].kind
    else {
        panic!("expected ordinary gate stage");
    };
    assert_eq!(backend.kernels()[*when_true].environment.len(), 1);
    assert_eq!(backend.kernels()[*when_true].input_subject, None);

    let compiled =
        compile_program(&backend).expect("by-reference environment projection should compile");
    let artifact = compiled
        .kernel(*when_true)
        .expect("compiled program should retain per-kernel metadata");
    assert!(artifact.code_size > 0);
    assert!(
        artifact
            .clif
            .contains(&format!("({}) -> i8", clif_pointer_ty()))
    );
    assert!(artifact.clif.contains("load.i8"));
}

#[test]
fn cranelift_codegen_compiles_inline_pipe_memos() {
    let before_binding = HirBindingId::from_raw(101);
    let after_binding = HirBindingId::from_raw(102);
    let core = manual_core_gate_stage(
        CoreType::Primitive(BuiltinType::Int),
        CoreType::Primitive(BuiltinType::Bool),
        move |module, span| {
            let head = module
                .exprs_mut()
                .alloc(CoreExpr {
                    span,
                    ty: CoreType::Primitive(BuiltinType::Int),
                    kind: CoreExprKind::AmbientSubject,
                })
                .expect("pipe head allocation should fit");
            let stage_subject = module
                .exprs_mut()
                .alloc(CoreExpr {
                    span,
                    ty: CoreType::Primitive(BuiltinType::Int),
                    kind: CoreExprKind::AmbientSubject,
                })
                .expect("stage subject allocation should fit");
            let one = module
                .exprs_mut()
                .alloc(CoreExpr {
                    span,
                    ty: CoreType::Primitive(BuiltinType::Int),
                    kind: CoreExprKind::Integer(IntegerLiteral { raw: "1".into() }),
                })
                .expect("increment allocation should fit");
            let incremented = module
                .exprs_mut()
                .alloc(CoreExpr {
                    span,
                    ty: CoreType::Primitive(BuiltinType::Int),
                    kind: CoreExprKind::Binary {
                        left: stage_subject,
                        operator: HirBinaryOperator::Add,
                        right: one,
                    },
                })
                .expect("increment expression allocation should fit");
            let before = module
                .exprs_mut()
                .alloc(CoreExpr {
                    span,
                    ty: CoreType::Primitive(BuiltinType::Int),
                    kind: CoreExprKind::Reference(CoreReference::Local(before_binding)),
                })
                .expect("memo reference allocation should fit");
            let after = module
                .exprs_mut()
                .alloc(CoreExpr {
                    span,
                    ty: CoreType::Primitive(BuiltinType::Int),
                    kind: CoreExprKind::Reference(CoreReference::Local(after_binding)),
                })
                .expect("memo reference allocation should fit");
            let compared = module
                .exprs_mut()
                .alloc(CoreExpr {
                    span,
                    ty: CoreType::Primitive(BuiltinType::Bool),
                    kind: CoreExprKind::Binary {
                        left: before,
                        operator: HirBinaryOperator::LessThan,
                        right: after,
                    },
                })
                .expect("comparison allocation should fit");
            module
                .exprs_mut()
                .alloc(CoreExpr {
                    span,
                    ty: CoreType::Primitive(BuiltinType::Bool),
                    kind: CoreExprKind::Pipe(CoreInlinePipeExpr {
                        head,
                        stages: vec![
                            CoreInlinePipeStage {
                                span,
                                subject_memo: Some(before_binding),
                                result_memo: Some(after_binding),
                                input_subject: CoreType::Primitive(BuiltinType::Int),
                                result_subject: CoreType::Primitive(BuiltinType::Int),
                                kind: CoreInlinePipeStageKind::Transform {
                                    mode: PipeTransformMode::Apply,
                                    expr: incremented,
                                },
                            },
                            CoreInlinePipeStage {
                                span,
                                subject_memo: None,
                                result_memo: None,
                                input_subject: CoreType::Primitive(BuiltinType::Int),
                                result_subject: CoreType::Primitive(BuiltinType::Bool),
                                kind: CoreInlinePipeStageKind::Transform {
                                    mode: PipeTransformMode::Replace,
                                    expr: compared,
                                },
                            },
                        ],
                    }),
                })
                .expect("pipe allocation should fit")
        },
        |module, span| {
            module
                .exprs_mut()
                .alloc(CoreExpr {
                    span,
                    ty: CoreType::Primitive(BuiltinType::Bool),
                    kind: CoreExprKind::Reference(CoreReference::Builtin(HirBuiltinTerm::False)),
                })
                .expect("fallback allocation should fit")
        },
    );
    validate_core_module(&core).expect("manual core module should validate");
    let lambda = lower_lambda_module(&core).expect("typed lambda lowering should succeed");
    validate_lambda_module(&lambda).expect("typed lambda should validate");
    let backend = lower_backend_module(&lambda).expect("backend lowering should succeed");
    validate_program(&backend).expect("backend program should validate");

    let item = find_item(&backend, "captured");
    let pipeline = &backend.pipelines()[first_pipeline(&backend, item)];
    let BackendStageKind::Gate(BackendGateStage::Ordinary { when_true, .. }) =
        &pipeline.stages[0].kind
    else {
        panic!("expected ordinary gate stage");
    };

    let kernel = &backend.kernels()[*when_true];
    let KernelExprKind::Pipe(pipe) = &kernel.exprs()[kernel.root].kind else {
        panic!("expected gate kernel root to stay an inline pipe");
    };
    assert_eq!(pipe.stages.len(), 2);
    assert!(pipe.stages[0].subject_memo.is_some());
    assert!(pipe.stages[0].result_memo.is_some());

    let mut evaluator = KernelEvaluator::new(&backend);
    assert_eq!(
        evaluator
            .evaluate_kernel(
                *when_true,
                Some(&RuntimeValue::Int(3)),
                &[],
                &BTreeMap::new()
            )
            .expect("inline pipe memos should evaluate"),
        RuntimeValue::Bool(true)
    );

    let compiled = compile_program(&backend).expect("inline pipe memo kernels should compile");
    let artifact = compiled
        .kernel(*when_true)
        .expect("compiled program should retain memo kernel metadata");
    assert!(artifact.code_size > 0);
    assert!(artifact.clif.contains("iadd"));
    assert!(artifact.clif.contains("icmp slt"));
    assert!(artifact.clif.contains("(i64) -> i8"));
    assert!(!compiled.object().is_empty());
}

#[test]
fn cranelift_codegen_compiles_inline_pipe_gate_option_carriers() {
    let backend = lower_text(
        "backend-inline-pipe-gate-carriers.aivi",
        r#"
value maybePositive : Option Int = 2
 ?|> True

value missingNumber : Option Int = 2
 ?|> False

value maybeGreeting : Option Text = "hello"
 ?|> True

value missingGreeting : Option Text = "hello"
 ?|> False
"#,
    );

    let maybe_positive = find_item(&backend, "maybePositive");
    let maybe_positive_body = backend.items()[maybe_positive]
        .body
        .expect("maybePositive should carry a body kernel");
    let kernel = &backend.kernels()[maybe_positive_body];
    let KernelExprKind::Pipe(pipe) = &kernel.exprs()[kernel.root].kind else {
        panic!("expected inline pipe body for maybePositive");
    };
    assert!(matches!(
        pipe.stages[0].kind,
        InlinePipeStageKind::Gate { .. }
    ));

    let mut evaluator = KernelEvaluator::new(&backend);
    let globals = BTreeMap::new();
    assert_eq!(
        evaluator
            .evaluate_item(maybe_positive, &globals)
            .expect("inline scalar gate should evaluate"),
        RuntimeValue::OptionSome(Box::new(RuntimeValue::Int(2)))
    );
    assert_eq!(
        evaluator
            .evaluate_item(find_item(&backend, "missingNumber"), &globals)
            .expect("inline scalar false gate should evaluate"),
        RuntimeValue::OptionNone
    );
    assert_eq!(
        evaluator
            .evaluate_item(find_item(&backend, "maybeGreeting"), &globals)
            .expect("inline niche gate should evaluate"),
        RuntimeValue::OptionSome(Box::new(RuntimeValue::Text("hello".into())))
    );
    assert_eq!(
        evaluator
            .evaluate_item(find_item(&backend, "missingGreeting"), &globals)
            .expect("inline niche false gate should evaluate"),
        RuntimeValue::OptionNone
    );

    let compiled = compile_program(&backend)
        .expect("inline pipe gate carriers should compile through Cranelift");

    let maybe_positive_artifact = compiled
        .kernel(maybe_positive_body)
        .expect("compiled program should retain maybePositive metadata");
    assert!(maybe_positive_artifact.code_size > 0);
    assert!(maybe_positive_artifact.clif.contains("brif"));
    assert!(maybe_positive_artifact.clif.contains("() -> i128"));
    assert!(
        maybe_positive_artifact.clif.contains("ishl"),
        "option packing should shift the payload into its i128 carrier; CLIF was:\n{}",
        maybe_positive_artifact.clif
    );

    let missing_number_body = backend.items()[find_item(&backend, "missingNumber")]
        .body
        .expect("missingNumber should carry a body kernel");
    let missing_number_artifact = compiled
        .kernel(missing_number_body)
        .expect("compiled program should retain missingNumber metadata");
    assert!(missing_number_artifact.clif.contains("iconst.i64 0"));
    assert!(missing_number_artifact.clif.contains("uextend.i128"));

    let maybe_greeting_body = backend.items()[find_item(&backend, "maybeGreeting")]
        .body
        .expect("maybeGreeting should carry a body kernel");
    let maybe_greeting_artifact = compiled
        .kernel(maybe_greeting_body)
        .expect("compiled program should retain maybeGreeting metadata");
    let ptr = clif_pointer_ty();
    assert!(maybe_greeting_artifact.code_size > 0);
    assert!(maybe_greeting_artifact.clif.contains("brif"));
    assert!(
        maybe_greeting_artifact
            .clif
            .contains(&format!("() -> {ptr}"))
    );
    assert!(maybe_greeting_artifact.clif.contains("symbol_value"));

    let missing_greeting_body = backend.items()[find_item(&backend, "missingGreeting")]
        .body
        .expect("missingGreeting should carry a body kernel");
    let missing_greeting_artifact = compiled
        .kernel(missing_greeting_body)
        .expect("compiled program should retain missingGreeting metadata");
    assert!(
        missing_greeting_artifact
            .clif
            .contains(&format!("iconst.{ptr} 0"))
    );
    assert!(!compiled.object().is_empty());
}

#[test]
fn runtime_passes_functor_evidence_to_generic_functions() {
    let backend = lower_text(
        "generic-functor.aivi",
        r#"
type Functor F => (A -> B) -> F A -> F B
func transform = f xs => xs |> map f

type Int -> Int
func increment = n => n + 1
type Functor F => (A -> B) -> F A -> F B
func nested = f xs => transform f xs
value mapped : Option Int = nested increment (Some 2)
"#,
    );
    let mut evaluator = KernelEvaluator::new(&backend);
    assert_eq!(
        evaluator
            .evaluate_item(find_item(&backend, "mapped"), &BTreeMap::new())
            .unwrap(),
        RuntimeValue::OptionSome(Box::new(RuntimeValue::Int(3)))
    );
}

#[test]
fn runtime_folds_authored_carriers_with_polymorphic_accumulators() {
    let backend = lower_text(
        "generic-foldable.aivi",
        r#"
type Box A = { values: List A }
instance Foldable Box = {
    reduce = step seed box => reduce step seed box.values
}
type Int -> Int -> Int
func accumulate = total n => total * 10 + n
value box : Box Int = { values: [1, 2, 3] }
value folded : Int = reduce accumulate 0 box
"#,
    );
    let mut evaluator = KernelEvaluator::new(&backend);
    assert_eq!(
        evaluator
            .evaluate_item(find_item(&backend, "folded"), &BTreeMap::new())
            .unwrap(),
        RuntimeValue::Int(123)
    );
}

#[test]
fn runtime_maps_imported_either_instance() {
    let backend = lower_workspace_text(
        "stdlib-either-instance.aivi",
        r#"
use aivi.core.either (Either Left Right)
type Int -> Int
func increment = n => n + 1
value right : Either Text Int = Right 2
value mapped : Either Text Int = map increment right
"#,
    );
    let mut evaluator = KernelEvaluator::new(&backend);
    let result = evaluator.evaluate_item(find_item(&backend, "mapped"), &BTreeMap::new());
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn runtime_folds_imported_dictionary_instance() {
    let backend = lower_workspace_text(
        "stdlib-dict-instance.aivi",
        r#"
use aivi.core.dict (Dict fromList)
type Int -> Int -> Int
func accumulate = total n => total * 10 + n
value dictionary : Dict Text Int = fromList [("b", 2), ("a", 1)]
value folded : Int = reduce accumulate 0 dictionary
"#,
    );
    let mut evaluator = KernelEvaluator::new(&backend);
    assert_eq!(
        evaluator
            .evaluate_item(find_item(&backend, "folded"), &BTreeMap::new())
            .unwrap(),
        RuntimeValue::Int(12)
    );
}

#[test]
fn runtime_folds_imported_non_empty_instance() {
    let backend = lower_workspace_text(
        "stdlib-non-empty-instance.aivi",
        r#"
use aivi.nonEmpty (NonEmptyList fromHeadTail)
type Int -> Int -> Int
func accumulate = total n => total * 10 + n
value items : NonEmptyList Int = fromHeadTail 1 [2, 3]
value folded : Int = reduce accumulate 0 items
"#,
    );
    let mut evaluator = KernelEvaluator::new(&backend);
    assert_eq!(
        evaluator
            .evaluate_item(find_item(&backend, "folded"), &BTreeMap::new())
            .unwrap(),
        RuntimeValue::Int(123)
    );
}

#[test]
fn runtime_folds_imported_non_empty_values_without_importing_the_domain() {
    let backend = lower_workspace_text(
        "stdlib-non-empty-inferred-instance.aivi",
        r#"
use aivi.nonEmpty (fromHeadTail)
type Int -> Int -> Int
func accumulate = total n => total * 10 + n
value folded : Int = reduce accumulate 0 (append (fromHeadTail 1 [2, 3]) (fromHeadTail 4 [5]))
"#,
    );
    let mut evaluator = KernelEvaluator::new(&backend);
    assert_eq!(
        evaluator
            .evaluate_item(find_item(&backend, "folded"), &BTreeMap::new())
            .unwrap(),
        RuntimeValue::Int(12345)
    );
}

#[test]
fn runtime_executes_authored_method_local_applicative_evidence() {
    let backend = lower_text(
        "method-local-evidence.aivi",
        r#"
type Box A = Box A
instance Functor Box = { map = f box => box ||> Box a -> Box (f a) }
instance Foldable Box = { reduce = f seed box => box ||> Box a -> f seed a }
instance Traversable Box = { traverse = f box => box ||> Box a -> map Box (f a) }
type Int -> Option Int
func increment = n => Some (n + 1)
type Int -> List Int
func expand = n => [n, n + 1]
type Traversable F => F Int -> Option (F Int)
func advance = box => traverse increment box
type Traversable F => F Int -> Option (F Int)
func forwarded = box => advance box
type (Traversable F, Applicative G) => (Int -> G Int) -> F Int -> G (F Int)
func advanceWith = f box => traverse f box
value partial : Box Int -> Option (Box Int) = traverse increment
value checked : Bool = traverse increment (Box 2) == Some (Box 3)
value generic : Bool = forwarded (Box 2) == Some (Box 3)
value genericApplicative : Bool = advanceWith increment (Box 2) == Some (Box 3)
value genericList : Bool = advanceWith expand (Box 2) == [Box 2, Box 3]
value partiallyApplied : Bool = partial (Box 2) == Some (Box 3)
value differentApplicative : Bool = traverse expand (Box 2) == [Box 2, Box 3]
value piped : Bool = (Box 2 |> traverse increment) == Some (Box 3)
"#,
    );
    let mut interpreter = KernelEvaluator::new(&backend);
    let executable = aivi_backend::BackendExecutableProgram::interpreted(&backend);
    assert_eq!(
        executable.engine_kind(),
        aivi_backend::BackendExecutionEngineKind::Jit
    );
    let mut engine = executable.create_engine();
    for name in [
        "checked",
        "generic",
        "genericApplicative",
        "genericList",
        "partiallyApplied",
        "differentApplicative",
        "piped",
    ] {
        let item = find_item(&backend, name);
        assert_eq!(
            interpreter.evaluate_item(item, &BTreeMap::new()).unwrap(),
            RuntimeValue::Bool(true),
            "{name}"
        );
        assert_eq!(
            engine.evaluate_item(item, &BTreeMap::new()).unwrap(),
            RuntimeValue::Bool(true),
            "{name}"
        );
    }
}

#[test]
fn runtime_executes_builtin_traversal_with_abstract_and_authored_applicatives() {
    let backend = lower_text(
        "builtin-traversal-evidence.aivi",
        r#"
type Logged A = Logged Text A
instance Functor Logged = { map = f logged => logged ||> Logged log value -> Logged log (f value) }
instance Apply Logged = { apply = functions values => functions ||> Logged first f -> values ||> Logged second value -> Logged (append first second) (f value) }
instance Applicative Logged = { pure = value => Logged "" value }
type Int -> Logged Int
func loggedIncrement = n => n
 ||> 2 -> Logged "first" 3
 ||> _ -> Logged "second" 4
type Int -> Option Int
func increment = n => Some (n + 1)
type Int -> List Int
func expand = n => [n, n + 1]
type Traversable F => F Int -> Option (F Int)
func advance = values => traverse increment values
type Traversable F => F Int -> Option (F Int)
func forward = values => advance values
type (Traversable F, Applicative G) => (Int -> G Int) -> F Int -> G (F Int)
func traverseWith = f values => traverse f values
value sourceError : Result Text Int = Err "source"
value sourceInvalid : Validation Text Int = Invalid "source"
value optionGeneric : Bool = forward (Some 2) == Some (Some 3)
value listGeneric : Bool = advance [2, 3] == Some [3, 4]
value emptyListGeneric : Bool = advance [] == Some []
value emptyOptionGeneric : Bool = advance None == Some None
value errorGeneric : Bool = advance sourceError == Some sourceError
value invalidGeneric : Bool = advance sourceInvalid == Some sourceInvalid
value cartesian : Bool = traverseWith expand [2, 3] == [[2, 3], [2, 4], [3, 3], [3, 4]]
value logged : Bool = traverseWith loggedIncrement [2, 3] == Logged "firstsecond" [3, 4]
value loggedSome : Bool = traverseWith loggedIncrement (Some 2) == Logged "first" (Some 3)
value loggedEmpty : Bool = traverseWith loggedIncrement [] == Logged "" []
value loggedNone : Bool = traverseWith loggedIncrement None == Logged "" None
value loggedError : Bool = traverseWith loggedIncrement sourceError == Logged "" sourceError
value loggedInvalid : Bool = traverseWith loggedIncrement sourceInvalid == Logged "" sourceInvalid
value partial : List Int -> Logged (List Int) = traverse loggedIncrement
value partiallyApplied : Bool = partial [2, 3] == Logged "firstsecond" [3, 4]
value piped : Bool = ([2, 3] |> traverse loggedIncrement) == Logged "firstsecond" [3, 4]
"#,
    );
    let mut interpreter = KernelEvaluator::new(&backend);
    let executable = aivi_backend::BackendExecutableProgram::interpreted(&backend);
    let mut engine = executable.create_engine();
    for name in [
        "optionGeneric",
        "listGeneric",
        "emptyListGeneric",
        "emptyOptionGeneric",
        "errorGeneric",
        "invalidGeneric",
        "cartesian",
        "logged",
        "loggedSome",
        "loggedEmpty",
        "loggedNone",
        "loggedError",
        "loggedInvalid",
        "partiallyApplied",
        "piped",
    ] {
        let item = find_item(&backend, name);
        assert_eq!(
            interpreter.evaluate_item(item, &BTreeMap::new()).unwrap(),
            RuntimeValue::Bool(true),
            "{name}"
        );
        assert_eq!(
            engine.evaluate_item(item, &BTreeMap::new()).unwrap(),
            RuntimeValue::Bool(true),
            "{name}"
        );
    }
}

#[test]
fn authored_standard_class_spellings_keep_every_dictionary_member() {
    for class in ["Eq", "Ord", "Setoid"] {
        let source = format!(
            r#"
class {class} A = {{
    proof : A -> A -> Bool
    label : A -> Text
}}
instance {class} Int = {{
    proof = left right => True
    label = value => "authored"
}}
type {class} A => A -> A -> Bool
func accepts = left right => proof left right
type {class} A => A -> Text
func describe = value => label value
value direct : Bool = accepts 1 2
value description : Text = describe 1
value partial : Int -> Bool = accepts 1
value partiallyApplied : Bool = partial 2
value piped : Bool = 2 |> accepts 1
"#
        );
        let backend = lower_text("authored-standard-spelling.aivi", &source);
        let mut interpreter = KernelEvaluator::new(&backend);
        let executable = aivi_backend::BackendExecutableProgram::interpreted(&backend);
        let mut engine = executable.create_engine();
        for (name, expected) in [
            ("direct", RuntimeValue::Bool(true)),
            ("description", RuntimeValue::Text("authored".into())),
            ("partiallyApplied", RuntimeValue::Bool(true)),
            ("piped", RuntimeValue::Bool(true)),
        ] {
            let item = find_item(&backend, name);
            assert_eq!(
                interpreter.evaluate_item(item, &BTreeMap::new()).unwrap(),
                expected,
                "{class}.{name}"
            );
            assert_eq!(
                engine.evaluate_item(item, &BTreeMap::new()).unwrap(),
                expected,
                "{class}.{name}"
            );
        }
    }
}

#[test]
fn comparison_operators_use_typed_members_of_authored_classes() {
    let backend = lower_text(
        "typed-comparison-members.aivi",
        r#"
class Same A = { (==) : A -> A -> Bool }
class Ranking A = {
    with Same A
    compare : A -> A -> Ordering
}
type Tag = Tag Int
instance Same Tag = { (==) = left right => True }
instance Ranking Tag = { compare = left right => Equal }
type Same A => A -> A -> Bool
func equal = left right => left == right
type Ranking A => A -> A -> Bool
func ascending = left right => left < right
value direct : Bool = Tag 1 == Tag 2
value negated : Bool = (Tag 1 != Tag 2) == False
value generic : Bool = equal (Tag 1) (Tag 2)
value ranked : Bool = (Tag 1 < Tag 2) == False
value nonStrict : Bool = Tag 1 <= Tag 2
value genericRanked : Bool = ascending (Tag 1) (Tag 2) == False
"#,
    );
    let mut interpreter = KernelEvaluator::new(&backend);
    let executable = aivi_backend::BackendExecutableProgram::interpreted(&backend);
    let mut engine = executable.create_engine();
    for name in [
        "direct",
        "negated",
        "generic",
        "ranked",
        "nonStrict",
        "genericRanked",
    ] {
        let item = find_item(&backend, name);
        assert_eq!(
            interpreter.evaluate_item(item, &BTreeMap::new()).unwrap(),
            RuntimeValue::Bool(true),
            "{name}"
        );
        assert_eq!(
            engine.evaluate_item(item, &BTreeMap::new()).unwrap(),
            RuntimeValue::Bool(true),
            "{name}"
        );
    }
}

#[test]
fn comparison_members_receive_their_method_local_dictionaries() {
    let backend = lower_text(
        "comparison-method-evidence.aivi",
        r#"
class Same A = { (==) : Setoid A => A -> A -> Bool }
type Tag = Tag Int
instance Setoid Tag = { equals = left right => True }
instance Same Tag = { (==) = left right => equals left right }
type (Same A, Setoid A) => A -> A -> Bool
func equal = left right => left == right
value direct : Bool = Tag 1 == Tag 2
value generic : Bool = equal (Tag 1) (Tag 2)
"#,
    );
    let mut interpreter = KernelEvaluator::new(&backend);
    let executable = aivi_backend::BackendExecutableProgram::interpreted(&backend);
    let mut engine = executable.create_engine();
    for name in ["direct", "generic"] {
        let item = find_item(&backend, name);
        assert_eq!(
            interpreter.evaluate_item(item, &BTreeMap::new()).unwrap(),
            RuntimeValue::Bool(true),
            "{name}"
        );
        assert_eq!(
            engine.evaluate_item(item, &BTreeMap::new()).unwrap(),
            RuntimeValue::Bool(true),
            "{name}"
        );
    }
}

#[test]
fn authored_eq_operator_shadowing_keeps_its_generic_evidence() {
    let backend = lower_text(
        "authored-eq-operator.aivi",
        r#"
class Eq A = {
    (==) : A -> A -> Bool
    witness : A -> Bool
}
type Tag = Tag Int
instance Eq Tag = {
    (==) = left right => True
    witness = value => True
}
type Eq A => A -> A -> Bool
func equal = left right => left == right
type Eq A => A -> A -> Bool
func unequal = left right => left != right
type Eq A => A -> Bool
func witnessed = value => witness value
value equalTags : Bool = equal (Tag 1) (Tag 2)
value unequalTags : Bool = unequal (Tag 1) (Tag 2)
value witnessedTag : Bool = witnessed (Tag 1)
"#,
    );
    let mut interpreter = KernelEvaluator::new(&backend);
    for (name, expected) in [
        ("equalTags", true),
        ("unequalTags", false),
        ("witnessedTag", true),
    ] {
        assert_eq!(
            interpreter
                .evaluate_item(find_item(&backend, name), &BTreeMap::new())
                .unwrap(),
            RuntimeValue::Bool(expected),
            "{name}"
        );
    }
}

#[test]
fn task_traversal_builds_and_abandons_deep_plans_without_recursion() {
    let backend = lower_text(
        "task-traversal-depth.aivi",
        r#"
type Int -> Task Text Int
func step = n => pure (100 / n)
type List Int -> Task Text (List Int)
func scheduled = values => traverse step values
"#,
    );
    let item = find_item(&backend, "scheduled");
    let kernel = backend.items()[item].body.unwrap();
    let globals = BTreeMap::new();
    let mut evaluator = KernelEvaluator::new(&backend);
    let callable = evaluator.evaluate_item(item, &globals).unwrap();
    let plan = evaluator
        .apply_runtime_callable(
            kernel,
            callable.clone(),
            vec![RuntimeValue::List(vec![RuntimeValue::Int(1); 20_000])],
            &globals,
        )
        .unwrap();
    assert!(matches!(&plan, RuntimeValue::Task(_)));
    plan.discard();
    let mut values = vec![RuntimeValue::Int(1); 20_000];
    values.push(RuntimeValue::Int(0));
    let error = evaluator
        .apply_runtime_callable(kernel, callable, vec![RuntimeValue::List(values)], &globals)
        .unwrap_err();
    assert!(error.to_string().contains("division by zero"), "{error}");
}

#[test]
fn runtime_passes_conditional_equality_evidence_to_authored_members() {
    let backend = lower_text(
        "conditional-equality-evidence.aivi",
        r#"
type Box A = { item: A }
instance Eq A => Eq (Box A) = {
    (==) = left right => left.item == right.item
    (!=) = left right => left.item != right.item
}
instance Ord A => Ord (Box A) = { compare = left right => compare left.item right.item }
type Ord A => Box A -> Box A -> Bool
func before = left right => left < right
type Eq A => Box A -> Box A -> Bool
func same = left right => left == right
type Eq A => Box A -> Box A -> Bool
func forwarded = left right => same left right
value leftBox : Box Int = { item: 1 }
value rightBox : Box Int = { item: 2 }
value equal : Bool = forwarded leftBox leftBox
value unequal : Bool = leftBox != rightBox
value ordered : Bool = before leftBox rightBox
value directOrder : Bool = rightBox > leftBox
"#,
    );
    let mut interpreter = KernelEvaluator::new(&backend);
    for name in ["equal", "unequal", "ordered", "directOrder"] {
        assert_eq!(interpreter.evaluate_item(find_item(&backend, name), &BTreeMap::new()).unwrap(), RuntimeValue::Bool(true), "{name}");
    }
}
