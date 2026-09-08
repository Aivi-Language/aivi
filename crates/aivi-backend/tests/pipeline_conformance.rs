use std::collections::BTreeMap;

use aivi_backend::{
    BackendExecutableProgram, KernelEvaluator, NativeKernelPlan, Program, RuntimeValue,
    lower_module as lower_backend_module, validate_program,
};
use aivi_base::SourceDatabase;
use aivi_core::{lower_module as lower_core_module, validate_module as validate_core_module};
use aivi_lambda::{lower_module as lower_lambda_module, validate_module as validate_lambda_module};
use aivi_syntax::parse_module;

struct FeatureCase {
    feature: &'static str,
    spec_section: &'static str,
    source: &'static str,
    item: &'static str,
    expected: RuntimeValue,
}

fn feature_cases() -> Vec<FeatureCase> {
    vec![
        FeatureCase {
            feature: "scalar arithmetic",
            spec_section: "AIVI_RFC.md §6.2 and §10",
            source: "value result:Int = 21 + 21\n",
            item: "result",
            expected: RuntimeValue::Int(42),
        },
        FeatureCase {
            feature: "ordered comparison",
            spec_section: "AIVI_RFC.md §7.3",
            source: "value result:Bool = 6 >= 5\n",
            item: "result",
            expected: RuntimeValue::Bool(true),
        },
        FeatureCase {
            feature: "closed tuple equality",
            spec_section: "AIVI_RFC.md §6.6 and §7.3",
            source: "value result:Bool = (1, 2) == (1, 2)\n",
            item: "result",
            expected: RuntimeValue::Bool(true),
        },
        FeatureCase {
            feature: "closed record equality",
            spec_section: "AIVI_RFC.md §6.4 and §7.3",
            source: concat!(
                "type Stats = { count: Int, active: Bool }\n",
                "value result:Bool = { count: 3, active: True } == { count: 3, active: True }\n",
            ),
            item: "result",
            expected: RuntimeValue::Bool(true),
        },
        FeatureCase {
            feature: "closed sum case split",
            spec_section: "AIVI_RFC.md §6.5 and §11.4",
            source: concat!(
                "type Shape = Circle Int | Point\n",
                "value result:Int = Circle 42\n",
                "    ||> Circle radius -> radius\n",
                "    ||> Point -> 0\n",
            ),
            item: "result",
            expected: RuntimeValue::Int(42),
        },
        FeatureCase {
            feature: "truthy/falsy branching",
            spec_section: "AIVI_RFC.md §11.4.1",
            source: concat!("value result:Int = True\n", "    T|> 42\n", "    F|> 0\n",),
            item: "result",
            expected: RuntimeValue::Int(42),
        },
    ]
}

fn lower_feature(case: &FeatureCase) -> Program {
    let mut sources = SourceDatabase::new();
    let file = sources.add_file(format!("conformance/{}.aivi", case.feature), case.source);
    let parsed = parse_module(&sources[file]);
    assert!(
        !parsed.has_errors(),
        "{} ({}) must parse without CST diagnostics: {:?}",
        case.feature,
        case.spec_section,
        parsed.all_diagnostics().collect::<Vec<_>>()
    );

    let hir = aivi_hir::lower_module(&parsed.module);
    assert!(
        !hir.has_errors(),
        "{} ({}) must lower and type-check without HIR diagnostics: {:?}",
        case.feature,
        case.spec_section,
        hir.diagnostics()
    );

    let core = lower_core_module(hir.module()).unwrap_or_else(|errors| {
        panic!(
            "{} ({}) must lower into typed core: {errors}",
            case.feature, case.spec_section
        )
    });
    validate_core_module(&core).unwrap_or_else(|errors| {
        panic!(
            "{} ({}) must satisfy typed-core invariants: {errors}",
            case.feature, case.spec_section
        )
    });

    let lambda = lower_lambda_module(&core).unwrap_or_else(|errors| {
        panic!(
            "{} ({}) must lower into closed lambda IR: {errors}",
            case.feature, case.spec_section
        )
    });
    validate_lambda_module(&lambda).unwrap_or_else(|errors| {
        panic!(
            "{} ({}) must satisfy lambda-IR invariants: {errors}",
            case.feature, case.spec_section
        )
    });

    let backend = lower_backend_module(&lambda).unwrap_or_else(|errors| {
        panic!(
            "{} ({}) must lower into backend IR: {errors}",
            case.feature, case.spec_section
        )
    });
    validate_program(&backend).unwrap_or_else(|errors| {
        panic!(
            "{} ({}) must satisfy backend-IR invariants: {errors}",
            case.feature, case.spec_section
        )
    });
    backend
}

fn find_item(program: &Program, name: &str) -> aivi_backend::ItemId {
    program
        .items()
        .iter()
        .find_map(|(id, item)| (item.name.as_ref() == name).then_some(id))
        .unwrap_or_else(|| panic!("conformance program should contain `{name}`"))
}

#[test]
fn representative_features_cross_owned_irs_with_interpreter_jit_and_native_parity() {
    for case in feature_cases() {
        let backend = lower_feature(&case);
        let item = find_item(&backend, case.item);
        let globals = BTreeMap::new();

        let interpreted = KernelEvaluator::new(&backend)
            .evaluate_item(item, &globals)
            .unwrap_or_else(|error| {
                panic!(
                    "{} ({}) must execute in the interpreter: {error}",
                    case.feature, case.spec_section
                )
            });

        let executable = BackendExecutableProgram::interpreted(&backend);
        let jitted = executable
            .create_engine()
            .evaluate_item(item, &globals)
            .unwrap_or_else(|error| {
                panic!(
                    "{} ({}) must execute in the JIT lane: {error}",
                    case.feature, case.spec_section
                )
            });

        let compiled = BackendExecutableProgram::compile(&backend).unwrap_or_else(|errors| {
            panic!(
                "{} ({}) must compile into the AOT object lane: {errors}",
                case.feature, case.spec_section
            )
        });
        assert!(
            compiled
                .compiled_object()
                .is_some_and(|artifact| !artifact.object().is_empty()),
            "{} ({}) must emit a non-empty object",
            case.feature,
            case.spec_section
        );

        let kernel = backend.items()[item]
            .body
            .unwrap_or_else(|| panic!("{} must carry a backend body kernel", case.feature));
        let mut native = NativeKernelPlan::compile(&backend, kernel).unwrap_or_else(|| {
            panic!(
                "{} ({}) must compile into an executable native artifact",
                case.feature, case.spec_section
            )
        });
        let native_result = native.execute(None, &[], &globals).unwrap_or_else(|error| {
            panic!(
                "{} ({}) must execute through the native artifact lane: {error:?}",
                case.feature, case.spec_section
            )
        });

        assert_eq!(interpreted, case.expected, "{} interpreter", case.feature);
        assert_eq!(jitted, interpreted, "{} JIT parity", case.feature);
        assert_eq!(native_result, interpreted, "{} native parity", case.feature);
    }
}
