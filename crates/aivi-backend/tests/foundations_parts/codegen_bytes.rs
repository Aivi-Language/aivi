#[test]
fn native_text_search_normalization_and_ordering_survive_artifact_roundtrip() {
    use aivi_backend::{
        NativeKernelPlan, compile_native_kernel_artifact, decode_native_kernel_artifact_binary,
        encode_native_kernel_artifact_binary,
    };
    let backend = lower_workspace_text(
        "milestone-2/valid/workspace-type-imports/main.aivi",
        r#"
use aivi.text (trim, toLower, contains as textContains)
use aivi.list (all, any)
type Int -> Int -> Bool
func above = threshold item => item > threshold
type Int -> Bool
func quotientPositive = item => 10 / item > 0
signal minimum : Signal Int
signal items : Signal (List Int)
signal every : Signal Bool = all (above minimum) items
signal some : Signal Bool = any (above minimum) items
signal shortAll : Signal Bool = all quotientPositive items
signal shortAny : Signal Bool = any quotientPositive items
signal left : Signal Text
signal right : Signal Text
signal normalized : Signal Text = toLower (trim left)
signal found : Signal Bool = textContains left right
type Text -> Text -> Bool
func isLess = a b => a < b
type Text -> Text -> Bool
func isGreater = a b => a > b
type Text -> Text -> Bool
func isAtMost = a b => a <= b
type Text -> Text -> Bool
func isAtLeast = a b => a >= b
signal less : Signal Bool = isLess left right
signal greater : Signal Bool = isGreater left right
signal atMost : Signal Bool = isAtMost left right
signal atLeast : Signal Bool = isAtLeast left right
"#,
    );
    let text = |value: &str| RuntimeValue::Text(value.into());
    let ints = |values: &[i64]| {
        RuntimeValue::List(values.iter().copied().map(RuntimeValue::Int).collect())
    };
    for (name, inputs, expected) in [
        (
            "every",
            vec![RuntimeValue::Int(2), ints(&[])],
            RuntimeValue::Bool(true),
        ),
        (
            "every",
            vec![RuntimeValue::Int(2), ints(&[3, 4])],
            RuntimeValue::Bool(true),
        ),
        (
            "every",
            vec![RuntimeValue::Int(2), ints(&[3, 2])],
            RuntimeValue::Bool(false),
        ),
        (
            "some",
            vec![RuntimeValue::Int(2), ints(&[])],
            RuntimeValue::Bool(false),
        ),
        (
            "some",
            vec![RuntimeValue::Int(2), ints(&[1, 3])],
            RuntimeValue::Bool(true),
        ),
        ("shortAll", vec![ints(&[-1, 0])], RuntimeValue::Bool(false)),
        ("shortAny", vec![ints(&[1, 0])], RuntimeValue::Bool(true)),
        (
            "normalized",
            vec![text("\u{2003}İΣ Ä\n")],
            text("i\u{307}ς ä"),
        ),
        ("normalized", vec![text(" \t\n")], text("")),
        (
            "found",
            vec![text(""), text("ä\0雪")],
            RuntimeValue::Bool(true),
        ),
        (
            "found",
            vec![text("\0雪"), text("ä\0雪")],
            RuntimeValue::Bool(true),
        ),
        (
            "found",
            vec![text("Ä"), text("ä")],
            RuntimeValue::Bool(false),
        ),
        (
            "less",
            vec![text("a"), text("aa")],
            RuntimeValue::Bool(true),
        ),
        (
            "less",
            vec![text("雪"), text("ä")],
            RuntimeValue::Bool(false),
        ),
        (
            "greater",
            vec![text("雪"), text("ä")],
            RuntimeValue::Bool(true),
        ),
        (
            "atMost",
            vec![text("ä"), text("ä")],
            RuntimeValue::Bool(true),
        ),
        (
            "atLeast",
            vec![text("ä"), text("ä")],
            RuntimeValue::Bool(true),
        ),
    ] {
        let item = find_item(&backend, name);
        let BackendItemKind::Signal(signal) = &backend.items()[item].kind else {
            panic!("expected a derived signal")
        };
        let kernel = signal.body_kernel.unwrap();
        let artifact = compile_native_kernel_artifact(&backend, kernel)
            .expect("text operation should compile")
            .unwrap_or_else(|| {
                panic!(
                    "native {name}: {:?}",
                    aivi_backend::diagnose_native_kernel_artifact_miss(&backend, kernel)
                )
            });
        let bytes = encode_native_kernel_artifact_binary(&artifact);
        let artifact = decode_native_kernel_artifact_binary(&bytes).unwrap();
        let mut plan = NativeKernelPlan::from_native_artifact(&backend, kernel, &artifact)
            .expect("serialized text helper signatures must relink");
        assert_eq!(
            plan.execute(None, &inputs, &BTreeMap::new()).unwrap(),
            expected,
            "native {name} with {inputs:?}",
        );
    }
}

#[test]
fn inline_sum_matching_rebases_validated_module_local_identity() {
    let backend = lower_text(
        "decoded-sum-identity.aivi",
        r#"
type Failure = Failed Text
type Failure -> Text
func describe = failure => failure
 ||> Failed reason -> reason
"#,
    );
    let item = find_item(&backend, "describe");
    let kernel = backend.items()[item].body.unwrap();
    let decoded = RuntimeValue::Sum(RuntimeSumValue {
        item: HirItemId::from_raw(u32::MAX),
        type_name: "Failure".into(),
        variant_name: "Failed".into(),
        fields: vec![RuntimeValue::Text("file decode failed".into())],
    });
    let mut evaluator = KernelEvaluator::new(&backend);
    assert_eq!(
        evaluator
            .evaluate_kernel(kernel, None, &[decoded], &BTreeMap::new())
            .unwrap(),
        RuntimeValue::Text("file decode failed".into())
    );
    let invalid = RuntimeValue::Sum(RuntimeSumValue {
        item: HirItemId::from_raw(u32::MAX),
        type_name: "DifferentFailure".into(),
        variant_name: "Failed".into(),
        fields: vec![RuntimeValue::Text("wrong nominal type".into())],
    });
    assert!(
        evaluator
            .evaluate_kernel(kernel, None, &[invalid], &BTreeMap::new())
            .is_err()
    );
}

#[test]
fn native_polymorphic_signal_application_uses_payload_specialization() {
    let backend = lower_workspace_text(
        "milestone-2/valid/workspace-type-imports/main.aivi",
        r#"
use aivi.option (getOrElse)
signal configured : Signal (Option Text)
signal fallback : Signal Text
signal label : Signal Text = getOrElse "missing" configured
signal dynamic : Signal Text = getOrElse fallback configured
type Signal Text -> Text -> Text
func explicitCarrier = source suffix => suffix
signal explicit : Signal Text = explicitCarrier fallback fallback
type Text -> Text -> Text
func append = left right => "{left}{right}"
signal fanout : Signal Text =
 &|> fallback
 &|> fallback
  |> append
"#,
    );
    for (name, inputs, expected) in [
        ("label", vec![RuntimeValue::OptionNone], "missing"),
        ("explicit", vec![RuntimeValue::Text("kept".into())], "kept"),
        (
            "fanout",
            vec![RuntimeValue::Text("kept".into())],
            "keptkept",
        ),
        (
            "label",
            vec![RuntimeValue::OptionSome(Box::new(RuntimeValue::Text(
                "set".into(),
            )))],
            "set",
        ),
        (
            "dynamic",
            vec![
                RuntimeValue::OptionNone,
                RuntimeValue::Text("default".into()),
            ],
            "default",
        ),
    ] {
        let BackendItemKind::Signal(signal) = &backend.items()[find_item(&backend, name)].kind
        else {
            panic!("expected signal")
        };
        let kernel = signal.body_kernel.unwrap();
        let artifact = aivi_backend::compile_native_kernel_artifact(&backend, kernel)
            .unwrap()
            .unwrap_or_else(|| {
                panic!(
                    "{name}: {:?}",
                    aivi_backend::diagnose_native_kernel_artifact_miss(&backend, kernel)
                )
            });
        let bytes = aivi_backend::encode_native_kernel_artifact_binary(&artifact);
        let decoded = aivi_backend::decode_native_kernel_artifact_binary(&bytes).unwrap();
        let mut native =
            aivi_backend::NativeKernelPlan::from_native_artifact(&backend, kernel, &decoded)
                .unwrap();
        assert_eq!(
            native.execute(None, &inputs, &BTreeMap::new()).unwrap(),
            RuntimeValue::Text(expected.into())
        );
    }
}

#[test]
fn native_interpolation_formats_committed_signal_payloads() {
    for (ty, input, expected) in [
        ("Text", RuntimeValue::Text("雪".into()), "Value: 雪"),
        ("Int", RuntimeValue::Int(-42), "Value: -42"),
        ("Bool", RuntimeValue::Bool(true), "Value: True"),
        (
            "Float",
            RuntimeValue::Float(RuntimeFloat::parse_literal("1.5").unwrap()),
            "Value: 1.5",
        ),
        ("Unit", RuntimeValue::Unit, "Value: ()"),
    ] {
        let backend = lower_text(
            "signal-interpolation.aivi",
            &format!(
                "signal input : Signal {ty}\nsignal message : Signal Text = \"Value: {{input}}\"\ntype Signal {ty} -> Text\nfunc describe = value => \"Value: {{value}}\"\n"
            ),
        );
        let BackendItemKind::Signal(signal) = &backend.items()[find_item(&backend, "message")].kind
        else {
            panic!("expected signal")
        };
        let kernels = [
            signal.body_kernel.unwrap(),
            backend.items()[find_item(&backend, "describe")]
                .body
                .unwrap(),
        ];
        for kernel in kernels {
            let inputs = vec![input.clone()];
            let expected = RuntimeValue::Text(expected.into());
            let mut evaluator = KernelEvaluator::new(&backend);
            assert_eq!(
                evaluator
                    .evaluate_kernel(kernel, None, &inputs, &BTreeMap::new())
                    .unwrap(),
                expected
            );
            let mut fresh = aivi_backend::NativeKernelPlan::compile(&backend, kernel)
                .expect("fresh JIT compilation must support scalar signal payloads");
            assert_eq!(
                fresh.execute(None, &inputs, &BTreeMap::new()).unwrap(),
                expected
            );
            let artifact = aivi_backend::compile_native_kernel_artifact(&backend, kernel)
                .unwrap()
                .unwrap_or_else(|| {
                    panic!(
                        "{ty}: {:?}",
                        aivi_backend::diagnose_native_kernel_artifact_miss(&backend, kernel)
                    )
                });
            let bytes = aivi_backend::encode_native_kernel_artifact_binary(&artifact);
            let artifact = aivi_backend::decode_native_kernel_artifact_binary(&bytes).unwrap();
            let mut native =
                aivi_backend::NativeKernelPlan::from_native_artifact(&backend, kernel, &artifact)
                    .unwrap();
            assert_eq!(
                native
                    .execute(None, &inputs, &BTreeMap::new())
                    .unwrap_or_else(|error| panic!("{ty}: {error:?}")),
                expected
            );
        }
    }
}
