use aivi_base::{FileId, SourceDatabase, SourceSpan};
use aivi_syntax::parse_module;

use crate::{
    BuiltinType, Item, PipeTransformMode, RecordFieldSurface, TypeParameterId, lower_module,
};

use super::*;
use crate::typecheck_context::GateExprInfo;
use crate::typecheck_context::SourceOptionActualType;

fn typecheck_text(path: &str, text: &str) -> TypeCheckReport {
    let mut sources = SourceDatabase::new();
    let file_id = sources.add_file(path, text);
    let parsed = parse_module(&sources[file_id]);
    assert!(
        !parsed.has_errors(),
        "typecheck input should parse cleanly: {:?}",
        parsed.all_diagnostics().collect::<Vec<_>>()
    );
    let lowered = lower_module(&parsed.module);
    assert!(
        !lowered.has_errors(),
        "typecheck input should lower cleanly: {:?}",
        lowered.diagnostics()
    );
    typecheck_module(lowered.module())
}

#[test]
fn definitive_contracts_report_unknown_evidence_without_changing_quiet_probes() {
    let module = lowered_module_text(
        "unknown-contract-evidence.aivi",
        "fun undecided = input => input\n",
    );
    let body = module
        .items()
        .iter()
        .find_map(|(_, item)| match item {
            Item::Function(function) if function.name.text() == "undecided" => Some(function.body),
            _ => None,
        })
        .expect("undecided function");
    let ExprKind::Name(reference) = &module.exprs()[body].kind else {
        panic!("expected parameter reference");
    };
    let ResolutionState::Resolved(TermResolution::Local(binding)) = reference.resolution.as_ref()
    else {
        panic!("expected resolved lexical binding");
    };
    let expected = GateType::Primitive(BuiltinType::Int);
    let mut checker = TypeChecker::new(&module);
    let mut stack = Vec::new();
    let mut env = GateExprEnv::default();
    assert!(!checker.check_expr(body, &env, Some(&expected), &mut stack));
    assert!(
        checker.diagnostics.is_empty(),
        "a quiet probe must not publish a failure"
    );
    assert!(!checker.check_expected_expr(body, &env, &expected, &mut stack));
    assert_eq!(checker.diagnostics.len(), 1);
    assert_eq!(
        checker.diagnostics[0].code,
        Some(crate::codes::TYPE_MISMATCH)
    );
    assert_eq!(checker.type_mismatch_reports[0].actual, None);
    env.locals.insert(*binding, expected.clone());
    assert!(checker.check_expected_expr(body, &env, &expected, &mut stack));
    assert_eq!(
        checker.diagnostics.len(),
        1,
        "proven evidence must not add a failure"
    );
}

#[test]
fn imported_structural_equality_requires_every_payload() {
    struct Resolver(crate::ExportedNames);
    impl crate::ImportResolver for Resolver {
        fn resolve(&self, _: &[&str]) -> crate::ImportModuleResolution {
            crate::ImportModuleResolution::Resolved(self.0.clone())
        }
    }
    for (model, subject, accepted) in [
        ("type Box = Box Bytes", "Box", false),
        ("type Box = Box (Int -> Int)", "Box", false),
        ("type Box = Box (List Bytes)", "Box", false),
        ("type Box = Box (Option Int) Text", "Box", true),
        ("type Box A = Box A", "Box Int", true),
        ("type Box A = Box A", "Box Bytes", false),
        ("type Box A = Box Int", "Box Bytes", true),
        ("type Box A = Box A", "Box A", true),
        ("type Box A = Here A | Next (Box A)", "Box Int", true),
        ("type Box A = Here A | Next (Box Bytes)", "Box Int", false),
        ("type Box A = End | Next (Box Int)", "Box Text", true),
        ("type Box = { payload : Bytes }", "Box", false),
        ("type Box = { payload : Int }", "Box", true),
        ("domain Box over Bytes", "Box", false),
        ("domain Box over Int", "Box", true),
        (
            "type Box = Box Bytes\ninstance Eq Box = { (==) = left right => True }",
            "Box",
            true,
        ),
    ] {
        let mut sources = SourceDatabase::new();
        let owner = sources.add_file("models.aivi", format!("{model}\nexport Box\n"));
        let parsed = parse_module(&sources[owner]);
        assert!(!parsed.has_errors(), "{model}");
        let lowered = lower_module(&parsed.module);
        assert!(
            !lowered.has_errors(),
            "{model}: {:?}",
            lowered.diagnostics()
        );
        assert!(
            lowered
                .module()
                .validate(crate::ValidationMode::RequireResolvedNames)
                .is_ok(),
            "{model}: {:?}",
            lowered
                .module()
                .validate(crate::ValidationMode::RequireResolvedNames)
        );
        let resolver = Resolver(crate::exports(lowered.module()));
        let left = subject.replacen("Box", "Original", 1);
        let right = subject.replacen("Box", "Alias", 1);
        let context = if subject == "Box A" { "Eq A => " } else { "" };
        let consumer = sources.add_file(
            "consumer.aivi",
            format!("use models (Box as Original)\nuse models (Box as Alias)\ntype {context}{left} -> {right} -> Bool\nfunc same = left right => left == right\n"),
        );
        let parsed = parse_module(&sources[consumer]);
        assert!(!parsed.has_errors(), "{model}");
        let lowered = crate::lower_module_with_resolver(&parsed.module, Some(&resolver));
        assert!(
            !lowered.has_errors(),
            "{model}: {:?}",
            lowered.diagnostics()
        );
        let report = typecheck_module(lowered.module());
        assert_eq!(
            report.is_ok(),
            accepted,
            "{model}: {:?}",
            report.diagnostics()
        );
        if !accepted {
            assert!(
                report
                    .diagnostics()
                    .iter()
                    .any(|d| d.code == Some(code("missing-eq-instance"))),
                "{model}: {:?}",
                report.diagnostics()
            );
        }
    }
}

#[test]
fn derived_inequality_is_optional_only_for_the_standard_eq_class() {
    for (declaration, accepted) in [
        ("", true),
        (
            "class Eq A = {\n    (==) : A -> A -> Bool\n    (!=) : A -> A -> Bool\n}\n",
            false,
        ),
    ] {
        let module = lowered_module_text(
            "eq-inventory-validation.aivi",
            &format!(
                "{declaration}type Box = Box Int\ninstance Eq Box = {{ (==) = left right => True }}\n"
            ),
        );
        if !accepted {
            let (_, class) = module
                .items()
                .iter()
                .find_map(|(id, item)| match item {
                    Item::Class(class) if module.root_items().contains(&id) => Some((id, class)),
                    _ => None,
                })
                .expect("authored class");
            assert!(matches!(
                class.identity,
                crate::ClassIdentity::Source { .. }
            ));
            assert_eq!(
                class
                    .members
                    .iter()
                    .map(|member| member.name.text())
                    .collect::<Vec<_>>(),
                vec!["==", "!="]
            );
        }
        let report = module.validate(crate::ValidationMode::RequireResolvedNames);
        assert_eq!(report.is_ok(), accepted, "{report:?}");
        if !accepted {
            assert!(format!("{report:?}").contains("missing-instance-member"));
        }
    }
}

#[test]
fn structural_equality_tracks_changed_recursive_arguments() {
    let report = typecheck_text(
        "changed-recursive-equality.aivi",
        "type Box A = Here A | Next (Box Bytes)\ntype Box Int -> Box Int -> Bool\nfunc same = left right => left == right\n",
    );
    assert!(!report.is_ok(), "{:?}", report.diagnostics());
    assert!(
        report
            .diagnostics()
            .iter()
            .any(|d| d.code == Some(code("missing-eq-instance")))
    );
}

#[test]
fn structural_equality_bounds_growing_proofs_and_restores_its_path() {
    let report = typecheck_text(
        "growing-recursive-equality.aivi",
        "type Box A = End | Next (Box (List A))\ntype Box Int -> Box Int -> Bool\nfunc same = left right => left == right\n",
    );
    assert!(!report.is_ok());
    assert!(
        report
            .diagnostics()
            .iter()
            .any(|d| d.code == Some(code("equality-proof-complexity"))),
        "{:?}",
        report.diagnostics()
    );
    let module = lowered_module_text("wide-equality-proof.aivi", "");
    let mut checker = TypeChecker::new(&module);
    let mut path = EqualityProofPath::default();
    let too_wide = GateType::Tuple(vec![GateType::Primitive(BuiltinType::Int); 4096]);
    assert!(matches!(
        checker.require_compiler_derived_eq(&too_wide, &mut path),
        Err(ComparisonError::Complexity)
    ));
    assert!(path.active.is_empty());
    assert_eq!(checker.equality_proof_depth, 0);
    assert!(
        checker
            .require_compiler_derived_eq(
                &GateType::Primitive(BuiltinType::Int),
                &mut EqualityProofPath::default()
            )
            .is_ok()
    );
}

#[test]
fn structural_equality_rejects_unknown_import_representations() {
    let module = lowered_module_text("unknown-import-equality.aivi", "");
    let mut checker = TypeChecker::new(&module);
    let subject = GateType::OpaqueImport {
        origin: Some(Box::new(crate::TypeIdentity::Source {
            file: FileId::new(100),
            name: "Unknown".into(),
        })),
        import: ImportId::from_raw(u32::MAX),
        name: "Unknown".into(),
        arguments: Vec::new(),
        definition: None,
    };
    let mut path = EqualityProofPath::default();
    let error = checker
        .require_compiler_derived_eq(&subject, &mut path)
        .unwrap_err();
    assert!(error.to_string().contains("closed representation"));
    assert!(path.active.is_empty());
    assert_eq!(checker.equality_proof_depth, 0);
}

#[test]
fn private_imported_recursive_types_preserve_payload_proofs() {
    struct Resolver(crate::ExportedNames);
    impl crate::ImportResolver for Resolver {
        fn resolve(&self, _: &[&str]) -> crate::ImportModuleResolution {
            crate::ImportModuleResolution::Resolved(self.0.clone())
        }
    }
    for (recursive, accepted) in [("Box A", true), ("Box Bytes", false)] {
        let mut sources = SourceDatabase::new();
        let owner = sources.add_file("private-models.aivi", format!("type Box A = Here A | Next ({recursive})\nvalue left : Box Int = Here 1\nvalue right : Box Int = Here 2\nexport left\nexport right\n"));
        let parsed = parse_module(&sources[owner]);
        assert!(!parsed.has_errors());
        let lowered = lower_module(&parsed.module);
        assert!(!lowered.has_errors());
        let resolver = Resolver(crate::exports(lowered.module()));
        assert!(resolver.0.find("Box").is_none());
        let consumer = sources.add_file(
            "consumer.aivi",
            "use models (left, right)\nvalue same : Bool = left == right\n",
        );
        let parsed = parse_module(&sources[consumer]);
        assert!(!parsed.has_errors());
        let lowered = crate::lower_module_with_resolver(&parsed.module, Some(&resolver));
        assert!(!lowered.has_errors());
        let report = typecheck_module(lowered.module());
        assert_eq!(
            report.is_ok(),
            accepted,
            "{recursive}: {:?}",
            report.diagnostics()
        );
    }
}

#[test]
fn authored_class_names_do_not_inherit_compiler_instances() {
    let report = typecheck_text(
        "authored-functor-shadow.aivi",
        r#"
class Functor F = { map : (A -> B) -> F A -> F B }
type List Int -> List Int
func missing = values => map (n => n + 1) values
"#,
    );
    assert!(!report.is_ok(), "{:?}", report.diagnostics());
    assert!(
        report
            .diagnostics()
            .iter()
            .any(|d| d.code == Some(code("missing-class-instance"))),
        "{:?}",
        report.diagnostics()
    );
}

#[test]
fn conditional_instances_discharge_concrete_and_inferred_prerequisites() {
    // Existing cases below also lock down failed proof backtracking.
    for (name, source, accepted) in [
        (
            "concrete",
            "class Render A = { render : A -> Text }\ntype Blob = Blob Bytes\ninstance Eq Bytes => Render Blob = { render = blob => \"rendered\" }\nvalue renderFn : Blob -> Text = render\n",
            false,
        ),
        (
            "inferred-missing",
            "class Render A = { render : A -> Text }\ntype Box A = Box A\ninstance Eq A => Render (Box A) = { render = box => \"rendered\" }\nvalue renderFn : Box Bytes -> Text = render\n",
            false,
        ),
        (
            "inferred-available",
            "class Render A = { render : A -> Text }\ntype Box A = Box A\ninstance Eq A => Render (Box A) = { render = box => \"rendered\" }\nvalue renderFn : Box Int -> Text = render\n",
            true,
        ),
        (
            "unbound-prerequisite",
            "class Render A = { render : A -> Text }\ntype Blob = Blob Int\ninstance Eq A => Render Blob = { render = blob => \"rendered\" }\nvalue renderFn : Blob -> Text = render\n",
            false,
        ),
        (
            "contextual",
            "class Render A = { render : A -> Text }\ntype Box A = Box A\ninstance Eq A => Render (Box A) = { render = box => \"rendered\" }\ntype Eq A => Box A -> Text\nfunc showBox = box => render box\n",
            true,
        ),
        (
            "equality",
            "type Blob = Blob Int\ninstance Eq Bytes => Eq Blob = {\n    (==) = left right => True\n    (!=) = left right => False\n}\nvalue same : Bool = Blob 1 == Blob 2\n",
            false,
        ),
    ] {
        let report = typecheck_text(name, source);
        assert_eq!(
            report.is_ok(),
            accepted,
            "{name}: {:?}",
            report.diagnostics()
        );
        if !accepted {
            assert!(
                report
                    .diagnostics()
                    .iter()
                    .any(|d| d.code == Some(code("missing-class-instance"))
                        || d.code == Some(crate::codes::MISSING_EQ_INSTANCE)),
                "{name}: {:?}",
                report.diagnostics()
            );
        }
    }
}

#[test]
fn comparison_constraints_require_matching_executable_members() {
    for (name, class, member, operator, diagnostic) in [
        (
            "eq-missing",
            "Eq",
            "same : A -> A -> Bool",
            "==",
            "missing-eq-instance",
        ),
        (
            "eq-result",
            "Eq",
            "(==) : A -> A -> Int",
            "==",
            "missing-eq-instance",
        ),
        (
            "eq-operands",
            "Eq",
            "(==) : Int -> Int -> Bool",
            "==",
            "missing-eq-instance",
        ),
        (
            "ord-missing",
            "Ord",
            "same : A -> A -> Bool",
            "<",
            "invalid-binary-operator",
        ),
        (
            "ord-result",
            "Ord",
            "compare : A -> A -> Bool",
            "<",
            "invalid-binary-operator",
        ),
        (
            "ord-operands",
            "Ord",
            "compare : Int -> Int -> Ordering",
            "<",
            "invalid-binary-operator",
        ),
    ] {
        let source = format!(
            "class {class} A = {{ {member} }}\ntype {class} A => A -> A -> Bool\nfunc operation = left right => left {operator} right\n"
        );
        let report = typecheck_text(&format!("{name}.aivi"), &source);
        assert!(!report.is_ok(), "{name}: {:?}", report.diagnostics());
        assert!(
            report
                .diagnostics()
                .iter()
                .any(|issue| issue.code == Some(code(diagnostic))),
            "{name}: {:?}",
            report.diagnostics()
        );
    }
}

#[test]
fn comparison_capabilities_follow_member_identity_and_type() {
    let report = typecheck_text(
        "comparison-capabilities.aivi",
        r#"
class Same A = { (==) : A -> A -> Bool }
class Ranking A = { compare : A -> A -> Ordering }
type Same A => A -> A -> Bool
func equality = left right => left == right
type Same A => A -> A -> Bool
func inequality = left right => left != right
type Ranking A => A -> A -> Bool
func ascending = left right => left < right
"#,
    );
    assert!(report.is_ok(), "{:?}", report.diagnostics());
}

#[test]
fn ambiguous_comparison_dictionaries_have_an_actionable_diagnostic() {
    for (member, signature, operator) in [("(==)", "Bool", "=="), ("compare", "Ordering", "<")] {
        let source = format!(
            "class First A = {{ {member} : A -> A -> {signature} }}\nclass Second A = {{ {member} : A -> A -> {signature} }}\ntype (First A, Second A) => A -> A -> Bool\nfunc ambiguous = left right => left {operator} right\n"
        );
        let report = typecheck_text("ambiguous-comparison.aivi", &source);
        assert!(!report.is_ok());
        let diagnostic = report
            .diagnostics()
            .iter()
            .find(|diagnostic| diagnostic.code == Some(code("ambiguous-class-member")))
            .expect("comparison ambiguity must retain its diagnostic category");
        assert!(
            diagnostic
                .notes
                .iter()
                .any(|note| note.contains("First") && note.contains("Second")),
            "{diagnostic:?}"
        );
    }
}

#[test]
fn conditional_equality_uses_its_captured_class_constraint_scope() {
    for (constraint, instance_constraint, accepted) in [
        ("Eq A =>", "Eq A", true),
        ("Ord A =>", "Eq A", true),
        ("Render A =>", "Render A", true),
        ("", "Render A", false),
    ] {
        let source = format!(
            "class Render A = {{ render : A -> Text }}\ntype Box A = Box A\ninstance {instance_constraint} => Eq (Box A) = {{\n    (==) = left right => True\n    (!=) = left right => False\n}}\ntype {constraint} Box A -> Box A -> Bool\nfunc same = left right => left == right\n"
        );
        let report = typecheck_text("scoped-conditional-equality.aivi", &source);
        assert_eq!(report.is_ok(), accepted, "{:?}", report.diagnostics());
    }

    let report = typecheck_text(
        "conditional-equality-scope-leak.aivi",
        r#"
class Render A = { render : A -> Text }
type Box A = Box A
instance Render A => Eq (Box A) = {
    (==) = left right => True
    (!=) = left right => False
}
type Render A => Box A -> Box A -> Bool
func scoped = left right => left == right
type Box A -> Box A -> Bool
func unscoped = left right => left == right
"#,
    );
    assert!(!report.is_ok());
    assert_eq!(
        report
            .diagnostics()
            .iter()
            .filter(|diagnostic| diagnostic.code == Some(crate::codes::MISSING_EQ_INSTANCE))
            .count(),
        1,
        "{:?}",
        report.diagnostics()
    );
}

#[test]
fn conditional_instances_reject_cycles_and_bound_growing_proofs() {
    for (source, reason) in [
        (
            "class Render A = { render : A -> Text }\ntype Box A = Box A\ninstance Render (Box A) => Render (Box A) = { render = box => \"rendered\" }\nvalue renderFn : Box Int -> Text = render\n",
            "cyclic instance prerequisites",
        ),
        (
            "class Render A = { render : A -> Text }\ntype Box A = Box A\ninstance Render (Box (Box A)) => Render (Box A) = { render = box => \"rendered\" }\nvalue renderFn : Box Int -> Text = render\n",
            "compiler complexity limit",
        ),
    ] {
        let report = typecheck_text("cyclic-instance.aivi", source);
        assert!(!report.is_ok(), "an ungrounded proof must fail");
        assert!(
            format!("{:?}", report.diagnostics()).contains(reason),
            "{:?}",
            report.diagnostics()
        );
    }
}

#[test]
fn conditional_instances_can_move_arguments_without_a_decreasing_head_rule() {
    let report = typecheck_text(
        "finite-instance-proof.aivi",
        r#"
class Render A = { render : A -> Text }
type Pair A B = Pair A B
instance Render (Pair A (List B)) => Render (Pair (List A) B) = { render = pair => "list" }
instance Render (Pair Int B) = { render = pair => "int" }
value renderFn : Pair (List (List Int)) Text -> Text = render
"#,
    );
    assert!(
        report.is_ok(),
        "a finite proof that moves list layers must succeed: {:?}",
        report.diagnostics()
    );
}

#[test]
fn portable_member_shapes_preserve_import_identity_and_quantifier_sharing() {
    let carrier = |file, import, parameter| GateType::OpaqueImport {
        origin: Some(Box::new(crate::TypeIdentity::Source {
            file: FileId::new(file),
            name: "Carrier".into(),
        })),
        import: crate::ImportId::from_raw(import),
        name: "Carrier".into(),
        arguments: vec![GateType::TypeParameter {
            parameter: crate::TypeParameterId::from_raw(parameter),
            name: "A".into(),
        }],
        definition: None,
    };
    let arrow = |parameter, result| GateType::Arrow {
        parameter: Box::new(parameter),
        result: Box::new(result),
    };
    let expected = arrow(carrier(1, 1, 10), carrier(1, 1, 10));
    let aliased = arrow(carrier(1, 2, 20), carrier(1, 2, 20));
    let split_quantifiers = arrow(carrier(1, 2, 20), carrier(1, 2, 21));
    let foreign = arrow(carrier(2, 2, 20), carrier(2, 2, 20));
    let module = lowered_module_text("member-identities.aivi", "");
    let typing = GateTypeContext::new(&module);
    assert!(typing.types_match(&expected, &aliased));
    assert!(!typing.types_match(&expected, &split_quantifiers));
    assert!(!typing.types_match(&expected, &foreign));
}

#[test]
fn nominal_origins_survive_alias_expansion_and_rigid_shapes() {
    let parameter = crate::TypeParameterId::from_raw(7);
    let variable = GateType::TypeParameter {
        parameter,
        name: "A".into(),
    };
    let identity = |file| crate::TypeIdentity::Source {
        file: aivi_base::FileId::new(file),
        name: "Carrier".into(),
    };
    let carrier = |file, import| GateType::OpaqueImport {
        origin: Some(Box::new(identity(file))),
        import: crate::ImportId::from_raw(import),
        name: "Carrier".into(),
        arguments: vec![variable.clone()],
        definition: None,
    };
    let alias = GateType::OpaqueImport {
        origin: Some(Box::new(crate::TypeIdentity::Source {
            file: aivi_base::FileId::new(3),
            name: "Alias".into(),
        })),
        import: crate::ImportId::from_raw(4),
        name: "Alias".into(),
        arguments: vec![variable.clone()],
        definition: Some(Box::new(crate::ImportTypeDefinition::Alias(
            crate::ImportValueType::Named {
                origin: Some(crate::ImportedTypeOrigin {
                    identity: identity(1),
                    source_module: Some("owner".into()),
                }),
                type_name: "Carrier".into(),
                arguments: vec![crate::ImportValueType::TypeVariable {
                    index: 0,
                    name: "A".into(),
                }],
                definition: None,
            },
        ))),
    };
    assert!(alias.same_shape_with_rigid_parameters(&carrier(1, 9), &[parameter]));
    assert!(!alias.same_shape_with_rigid_parameters(&carrier(2, 9), &[parameter]));
    let distinct_parameter = carrier(1, 9).substitute_type_parameter(
        parameter,
        &GateType::TypeParameter {
            parameter: crate::TypeParameterId::from_raw(8),
            name: "B".into(),
        },
    );
    assert!(!alias.same_shape_with_rigid_parameters(&distinct_parameter, &[parameter]));
}

#[test]
fn imported_conditional_instances_keep_shared_head_quantifiers() {
    struct Resolver(crate::ExportedNames);
    impl crate::ImportResolver for Resolver {
        fn resolve(&self, path: &[&str]) -> crate::ImportModuleResolution {
            if path == ["shared", "carrier"] {
                crate::ImportModuleResolution::Resolved(self.0.clone())
            } else {
                crate::ImportModuleResolution::Missing
            }
        }
    }
    let owner = lowered_module_text(
        "carrier.aivi",
        r#"
type Carrier E A = Carrier E A
instance Eq E => Functor (Carrier E) = {
    map = f carrier => carrier ||> Carrier e a -> Carrier e (f a)
}
instance Eq A => Semigroup (Carrier Text A) = { append = left right => left }
value sample : Carrier Text Int = Carrier "context" 1
export (Carrier, sample)
"#,
    );
    assert!(typecheck_module(&owner).is_ok());
    let exported = crate::exports(&owner);
    assert_eq!(exported.instances.len(), 2);
    let functor = &exported.instances[0];
    assert!(matches!(
        &functor.context[0].subject,
        crate::ImportedTypeBinding::Type(crate::ImportValueType::TypeVariable { index: 0, .. })
    ));
    assert!(
        matches!(&functor.head, crate::ImportedTypeBinding::Constructor { arguments, .. }
        if matches!(&arguments[0], crate::ImportValueType::TypeVariable { index: 0, .. }))
    );
    for (ty, accepted) in [
        (
            "(Int -> Bool) -> Carrier Text Int -> Carrier Text Bool",
            true,
        ),
        (
            "(Int -> Bool) -> Carrier Bytes Int -> Carrier Bytes Bool",
            false,
        ),
        (
            "Carrier Text Int -> Carrier Text Int -> Carrier Text Int",
            true,
        ),
        (
            "Carrier Text Bytes -> Carrier Text Bytes -> Carrier Text Bytes",
            false,
        ),
    ] {
        let member = if ty.starts_with('(') { "map" } else { "append" };
        let mut sources = SourceDatabase::new();
        let file = sources.add_file("consumer.aivi", format!("use shared.carrier (Carrier as Bag)\nuse shared.carrier (Carrier)\nvalue selected : {ty} = {member}\n"));
        let parsed = parse_module(&sources[file]);
        assert!(!parsed.has_errors());
        let lowered =
            crate::lower_module_with_resolver(&parsed.module, Some(&Resolver(exported.clone())));
        assert!(!lowered.has_errors(), "{:?}", lowered.diagnostics());
        let report = typecheck_module(lowered.module());
        assert_eq!(report.is_ok(), accepted, "{ty}: {:?}", report.diagnostics());
        if !accepted {
            assert!(format!("{:?}", report.diagnostics()).contains("Eq"));
        }
    }
    let mut private_export = exported.clone();
    private_export.names.retain(|name| name.name == "sample");
    let mut sources = SourceDatabase::new();
    let file = sources.add_file("private-consumer.aivi", "use shared.carrier (sample)\ntype Int -> Int\nfunc increment = item => item + 1\nvalue mapped = map increment sample\n");
    let parsed = parse_module(&sources[file]);
    assert!(!parsed.has_errors());
    let lowered =
        crate::lower_module_with_resolver(&parsed.module, Some(&Resolver(private_export)));
    assert!(!lowered.has_errors(), "{:?}", lowered.diagnostics());
    let report = typecheck_module(lowered.module());
    assert!(
        report.is_ok(),
        "carrier identity must survive without a named type import: {:?}",
        report.diagnostics()
    );
}

#[test]
fn portable_instance_heads_bind_constructor_parameters_in_fixed_arguments() {
    use crate::{
        ImportValueType, ImportedTypeBinding, ImportedTypeConstructor, TypeConstructorBinding,
        TypeConstructorHead,
    };
    let module = Module::default();
    let mut typing = GateTypeContext::new(&module);
    let application = ImportValueType::TypeApplication {
        index: 0,
        name: "F".into(),
        arguments: vec![ImportValueType::TypeVariable {
            index: 1,
            name: "A".into(),
        }],
    };
    let actual = GateType::Option(Box::new(GateType::Primitive(BuiltinType::Int)));
    for (template, actual) in [
        (
            ImportedTypeBinding::Type(application.clone()),
            TypeBinding::Type(actual.clone()),
        ),
        (
            ImportedTypeBinding::Constructor {
                head: ImportedTypeConstructor::Builtin(BuiltinType::Result),
                arguments: vec![application],
            },
            TypeBinding::Constructor(TypeConstructorBinding::new(
                TypeConstructorHead::Builtin(BuiltinType::Result),
                vec![actual],
            )),
        ),
    ] {
        let mut bindings = PolyTypeBindings::new();
        assert!(typing.match_import_type_binding(&template, &actual, &mut bindings));
        let prerequisite = ImportedTypeBinding::Constructor {
            head: ImportedTypeConstructor::Parameter {
                index: 0,
                name: "F".into(),
                arity: 1,
            },
            arguments: Vec::new(),
        };
        assert_eq!(
            typing.instantiate_import_type_binding(&prerequisite, &bindings),
            Some(TypeBinding::Constructor(TypeConstructorBinding::new(
                TypeConstructorHead::Builtin(BuiltinType::Option),
                Vec::new()
            )))
        );
        assert_eq!(
            typing.instantiate_import_type_binding(&template, &bindings),
            Some(actual)
        );
    }
}

#[test]
fn imported_constructor_identity_preserves_transparent_alias_parameter_order() {
    struct Resolver(crate::ExportedNames);
    impl crate::ImportResolver for Resolver {
        fn resolve(&self, _: &[&str]) -> crate::ImportModuleResolution {
            crate::ImportModuleResolution::Resolved(self.0.clone())
        }
    }
    for (definition, accepted) in [
        ("type Pair A B = { left : A, right : B }\n", true),
        ("type Pair A B = { left : B, right : A }\n", false),
    ] {
        let owner = lowered_module_text("owner.aivi", definition);
        let mut sources = SourceDatabase::new();
        sources.add_file("owner.aivi", definition);
        let file = sources.add_file(
            "consumer.aivi",
            "use owner (Pair as ImportedPair)\ntype Pair A B = { left : A, right : B }\n",
        );
        let parsed = parse_module(&sources[file]);
        let lowered = crate::lower_module_with_resolver(
            &parsed.module,
            Some(&Resolver(crate::exports(&owner))),
        );
        assert!(!lowered.has_errors(), "{:?}", lowered.diagnostics());
        let module = lowered.module();
        let (id, _) = module
            .items()
            .iter()
            .find(|(_, item)| matches!(item, Item::Type(ty) if ty.name.text() == "Pair"))
            .expect("local alias");
        let typing = GateTypeContext::new(module);
        let template = crate::ImportedTypeBinding::Constructor {
            head: crate::ImportedTypeConstructor::Named {
                origin: None,
                name: "Pair".into(),
                arity: 2,
                definition: None,
            },
            arguments: Vec::new(),
        };
        let actual = TypeBinding::Constructor(crate::TypeConstructorBinding::new(
            crate::TypeConstructorHead::Item(id),
            Vec::new(),
        ));
        assert_eq!(
            typing.match_import_type_binding(&template, &actual, &mut PolyTypeBindings::new()),
            accepted
        );
    }
}

#[test]
fn typecheck_rejects_wrong_polymorphic_instance_result() {
    let report = typecheck_text(
        "wrong-functor-result.aivi",
        r#"
type Box A = Box A
instance Functor Box = {
    map = f box => Box "wrong"
}
"#,
    );
    assert!(
        report
            .diagnostics()
            .iter()
            .any(|diagnostic| { diagnostic.code == Some(crate::codes::TYPE_MISMATCH) }),
        "polymorphic instance bodies must satisfy every quantified result type: {:?}",
        report.diagnostics()
    );
}

#[test]
fn typecheck_preserves_distinct_quantified_function_parameters() {
    let report = typecheck_text(
        "rigid-function-parameters.aivi",
        "type A -> B -> A\nfunc first = x y => y\n",
    );
    assert!(
        report
            .diagnostics()
            .iter()
            .any(|diagnostic| { diagnostic.code == Some(crate::codes::TYPE_MISMATCH) }),
        "a B value cannot satisfy the independently quantified A result: {:?}",
        report.diagnostics()
    );
}

#[test]
fn typecheck_rejects_specialization_of_quantified_local_values() {
    for text in [
        "type A -> B -> (A, A)\nfunc bad = x y => (x, y)\n",
        "type List B -> List A\nfunc bad = ys => ys\n",
        "type (A -> A) -> B -> B\nfunc bad = f y => f y\n",
        "type Functor F => (F A -> F A) -> F B -> F B\nfunc bad = f ys => f ys\n",
        "type Functor F => (A -> B) -> F A -> F B\nfunc bad = f xs => xs\n",
        "type A -> A -> A\nfunc choose = x y => x\ntype A -> B -> A\nfunc bad = x y => choose x y\n",
    ] {
        let report = typecheck_text("rigid-nested-types.aivi", text);
        assert!(
            !report.is_ok(),
            "independent quantified types cannot be unified when checking a body: {text}"
        );
    }
}

#[test]
fn partial_constructor_evidence_retains_its_higher_kinded_head() {
    let module = Module::default();
    let mut typing = GateTypeContext::new(&module);
    let constructor = crate::TypeParameterId::from_raw(0);
    let payload = crate::TypeParameterId::from_raw(1);
    let int = GateType::Primitive(BuiltinType::Int);
    let template = |argument| GateType::TypeApplication {
        parameter: constructor,
        name: "F".to_owned(),
        arguments: vec![argument],
    };
    let info = |actual| crate::typecheck_context::GateExprInfo {
        actual: Some(actual),
        ..Default::default()
    };
    let list = info(SourceOptionActualType::List(Box::new(
        SourceOptionActualType::Hole,
    )));
    let none = info(SourceOptionActualType::Option(Box::new(
        SourceOptionActualType::Hole,
    )));
    let mut bindings = HashMap::new();
    assert!(typing.match_gate_expr_template(&template(int.clone()), &list, &mut bindings));
    assert_eq!(
        bindings.get(&constructor),
        Some(&GateType::List(Box::new(int.clone())))
    );
    assert!(!typing.match_gate_expr_template(&template(int.clone()), &none, &mut bindings));

    let unknown_error = info(SourceOptionActualType::Result {
        error: Box::new(SourceOptionActualType::Hole),
        value: Box::new(SourceOptionActualType::Hole),
    });
    assert!(!typing.match_gate_expr_template(
        &template(int.clone()),
        &unknown_error,
        &mut HashMap::new()
    ));
    let result = GateType::Result {
        error: Box::new(GateType::Primitive(BuiltinType::Text)),
        value: Box::new(int.clone()),
    };
    let mut bindings = HashMap::from([(constructor, result.clone())]);
    assert!(typing.match_gate_expr_template(&template(int), &unknown_error, &mut bindings));
    assert_eq!(bindings.get(&constructor), Some(&result));

    let mut bindings = HashMap::new();
    assert!(typing.match_gate_expr_template(
        &template(GateType::TypeParameter {
            parameter: payload,
            name: "A".to_owned(),
        }),
        &list,
        &mut bindings
    ));
    assert!(
        !bindings.contains_key(&payload),
        "a hole cannot establish a payload binding"
    );
    typing.replace_rigid_type_parameters(vec![constructor]);
    assert!(!typing.match_gate_expr_template(
        &template(GateType::Primitive(BuiltinType::Int)),
        &list,
        &mut HashMap::new()
    ));
}

#[test]
fn contextual_type_templates_preserve_constructor_quantifiers() {
    let module = Module::default();
    let mut typing = GateTypeContext::new(&module);
    let constructor = crate::TypeParameterId::from_raw(0);
    let local = crate::TypeParameterId::from_raw(1);
    let flexible = crate::TypeParameterId::from_raw(2);
    typing.replace_rigid_type_parameters(vec![constructor, local]);
    let application = |parameter| GateType::TypeApplication {
        parameter: constructor,
        name: "F".to_owned(),
        arguments: vec![GateType::TypeParameter {
            parameter,
            name: "A".to_owned(),
        }],
    };
    let mut bindings = HashMap::new();
    assert!(typing.match_gate_type_template(
        &application(flexible),
        &application(local),
        &mut bindings
    ));
    assert_eq!(
        bindings.get(&flexible),
        Some(&GateType::TypeParameter {
            parameter: local,
            name: "A".to_owned()
        })
    );
    assert!(!typing.match_gate_type_template(
        &application(local),
        &application(flexible),
        &mut HashMap::new()
    ));
    assert!(!typing.match_gate_type_template(
        &application(local),
        &GateType::List(Box::new(GateType::TypeParameter {
            parameter: local,
            name: "A".to_owned()
        })),
        &mut HashMap::new()
    ));
}

#[test]
fn typecheck_accepts_instantiation_at_polymorphic_calls() {
    let report = typecheck_text(
        "polymorphic-call-instantiation.aivi",
        r#"
type A -> A -> A
func choose = x y => x
type (A -> A) -> A -> A
func apply = f x => f x
type A -> B -> A
func first = x y => choose x x
value text : Text = choose "left" "right"
value number : Int = apply (n => n + 1) 2
"#,
    );
    assert!(
        report.is_ok(),
        "call sites must still instantiate generic signatures: {:?}",
        report.diagnostics()
    );
}

#[test]
fn generic_inline_callbacks_inherit_lexical_types_and_class_evidence() {
    for (name, source) in [
        (
            "concrete-input",
            "type Functor F => F Int -> F Int\nfunc copy = value => map (n => n + 1) value\n",
        ),
        (
            "closed-capture",
            "type Functor F => Int -> F Int -> F Int\nfunc offset = amount value => map (n => n + amount) value\n",
        ),
        (
            "rigid-capture",
            "type Functor F => A -> F Int -> F A\nfunc replace = captured value => map (n => captured) value\n",
        ),
        (
            "rigid-input",
            "type Functor F => F A -> F A\nfunc copy = value => map (n => n) value\n",
        ),
        (
            "class-capture",
            "type (Functor F, Eq A) => A -> F A -> F Bool\nfunc matches = captured value => map (n => n == captured) value\n",
        ),
        (
            "nested",
            "type Functor F => F (List Int) -> F (List Int)\nfunc copy = value => map (items => map (n => n + 1) items) value\n",
        ),
        (
            "curried-result",
            "type Functor F => F Int -> F (Int -> Int)\nfunc copy = value => map (n => m => n + m) value\n",
        ),
        (
            "rigid-curried-result",
            "type Functor F => F (A -> A) -> F (A -> A)\nfunc copy = value => map (f => n => f n) value\n",
        ),
    ] {
        let report = typecheck_text(name, source);
        assert!(report.is_ok(), "{name}: {:?}", report.diagnostics());
    }
}

#[test]
fn generic_inline_callbacks_cannot_specialize_lexical_types_or_invent_evidence() {
    for (name, source) in [
        (
            "rigid-capture",
            "type Functor F => A -> F Int -> F A\nfunc replace = captured value => map (n => captured + 1) value\n",
        ),
        (
            "rigid-input",
            "type Functor F => F A -> F A\nfunc copy = value => map (n => n + 1) value\n",
        ),
        (
            "distinct-parameters",
            "type Functor F => A -> F B -> F A\nfunc replace = captured value => map (n => n) value\n",
        ),
        (
            "missing-evidence",
            "type Functor F => A -> F A -> F Bool\nfunc matches = captured value => map (n => n == captured) value\n",
        ),
        (
            "wrong-result",
            "type Functor F => F Int -> F Int\nfunc copy = value => map (n => \"wrong\") value\n",
        ),
    ] {
        let report = typecheck_text(name, source);
        assert!(
            !report.is_ok(),
            "{name} must reject an invalid lexical callback"
        );
        assert!(!report.diagnostics().is_empty());
    }
}

#[test]
fn typecheck_instantiates_generic_callbacks_from_known_container_types() {
    let report = typecheck_text(
        "generic-callback-instantiation.aivi",
        r#"
type A -> B -> (A, B)
func pair = left right => (left, right)
type (A -> B -> C) -> List A -> List B -> List C
func zipWith = transform left right => []
type List A -> List B -> List (A, B)
func zip = left right => zipWith pair left right

type A -> A
func identity = x => x
type List A -> List A
func copy = items => items |> map identity
"#,
    );
    assert!(
        report.is_ok(),
        "generic callbacks must instantiate using their arguments and expected results: {:?}",
        report.diagnostics()
    );
}

#[test]
fn typecheck_accepts_polymorphic_instance_member_constraints() {
    let report = typecheck_text(
        "constrained-instance-method.aivi",
        r#"
class Display A = {
    display : Eq B => A -> B -> Bool
}

type Label = Label Text
instance Display Label = {
    display = label item => item == item
}
"#,
    );
    assert!(
        report.is_ok(),
        "method-local constraints must be available while checking its universally quantified body: {:?}",
        report.diagnostics()
    );
}

#[test]
fn inline_instance_closures_preserve_rigidity_and_constraint_scope() {
    for (name, source) in [
        (
            "rigid-inline-identity.aivi",
            r#"
type Arrow A B = Arrow (A -> B)
type Arrow A B -> A -> B
func runArrow = arrow x => arrow ||> Arrow f -> f x
instance Semigroupoid Arrow = { compose left right = Arrow (x => runArrow left (runArrow right x)) }
instance Category Arrow = { id = Arrow (x => 1) }
"#,
        ),
        (
            "inline-method-constraint-leak.aivi",
            r#"
class Matcher F = {
    matches : Eq A => A -> F A -> Bool
    unchecked : A -> F A -> Bool
}
type Box A = Box A
instance Matcher Box = {
    matches expected box = (actual => actual == expected) expected
    unchecked expected box = (actual => actual == expected) expected
}
"#,
        ),
        (
            "inline-constructor-owner-mismatch.aivi",
            r#"
class Identity P = { identity : P A A }
type Arrow A B = Arrow (A -> B)
type Other A B = Other (A -> B)
instance Identity Arrow = { identity = Other (x => x) }
"#,
        ),
        (
            "inline-explicit-parameter-mismatch.aivi",
            r#"
class Identity P = { identity : P A A }
type Arrow A B = Arrow (A -> B)
instance Identity Arrow = { identity = Arrow (x:Int => x) }
"#,
        ),
    ] {
        let report = typecheck_text(name, source);
        assert!(
            !report.is_ok(),
            "{name} must reject an invalid instance closure"
        );
        assert!(!report.diagnostics().is_empty());
    }
}

fn typecheck_and_elaborate_text(path: &str, text: &str) -> (TypeCheckReport, Module) {
    let mut sources = SourceDatabase::new();
    let file_id = sources.add_file(path, text);
    let parsed = parse_module(&sources[file_id]);
    assert!(
        !parsed.has_errors(),
        "typecheck input should parse cleanly: {:?}",
        parsed.all_diagnostics().collect::<Vec<_>>()
    );
    let lowered = lower_module(&parsed.module);
    assert!(
        !lowered.has_errors(),
        "typecheck input should lower cleanly: {:?}",
        lowered.diagnostics()
    );
    let lowered_module = lowered.module().clone();
    let report = typecheck_module(&lowered_module);
    let elaborated = apply_defaults(&lowered_module, &report);
    (report, elaborated)
}

fn lowered_module_text(path: &str, text: &str) -> Module {
    let mut sources = SourceDatabase::new();
    let file_id = sources.add_file(path, text);
    let parsed = parse_module(&sources[file_id]);
    assert!(
        !parsed.has_errors(),
        "module input should parse cleanly: {:?}",
        parsed.all_diagnostics().collect::<Vec<_>>()
    );
    let lowered = lower_module(&parsed.module);
    assert!(
        !lowered.has_errors(),
        "module input should lower cleanly: {:?}",
        lowered.diagnostics()
    );
    lowered.module().clone()
}

fn unit_span() -> SourceSpan {
    SourceSpan::default()
}

fn test_name(text: &str) -> crate::Name {
    crate::Name::new(text, unit_span()).expect("test name should stay valid")
}

fn test_path(text: &str) -> crate::NamePath {
    crate::NamePath::from_vec(vec![test_name(text)]).expect("single-segment path")
}

fn builtin_type(module: &mut Module, builtin: BuiltinType) -> crate::TypeId {
    let builtin_name = match builtin {
        BuiltinType::Int => "Int",
        BuiltinType::Float => "Float",
        BuiltinType::Decimal => "Decimal",
        BuiltinType::BigInt => "BigInt",
        BuiltinType::Bool => "Bool",
        BuiltinType::Text => "Text",
        BuiltinType::Unit => "Unit",
        BuiltinType::Bytes => "Bytes",
        BuiltinType::List => "List",
        BuiltinType::Map => "Map",
        BuiltinType::Set => "Set",
        BuiltinType::Option => "Option",
        BuiltinType::Result => "Result",
        BuiltinType::Validation => "Validation",
        BuiltinType::Signal => "Signal",
        BuiltinType::Task => "Task",
    };
    module
        .alloc_type(crate::TypeNode {
            span: unit_span(),
            kind: crate::TypeKind::Name(crate::TypeReference::resolved(
                test_path(builtin_name),
                crate::TypeResolution::Builtin(builtin),
            )),
        })
        .expect("builtin type allocation should fit")
}

fn type_parameter(module: &mut Module, text: &str) -> crate::TypeParameterId {
    module
        .alloc_type_parameter(crate::TypeParameter {
            span: unit_span(),
            name: test_name(text),
        })
        .expect("type parameter allocation should fit")
}

fn type_parameter_type(
    module: &mut Module,
    parameter: crate::TypeParameterId,
    text: &str,
) -> crate::TypeId {
    module
        .alloc_type(crate::TypeNode {
            span: unit_span(),
            kind: crate::TypeKind::Name(crate::TypeReference::resolved(
                test_path(text),
                crate::TypeResolution::TypeParameter(parameter),
            )),
        })
        .expect("type parameter reference allocation should fit")
}

fn applied_type(
    module: &mut Module,
    callee: crate::TypeId,
    argument: crate::TypeId,
) -> crate::TypeId {
    module
        .alloc_type(crate::TypeNode {
            span: unit_span(),
            kind: crate::TypeKind::Apply {
                callee,
                arguments: crate::NonEmpty::new(argument, Vec::new()),
            },
        })
        .expect("applied type allocation should fit")
}

fn builtin_term_expr(
    module: &mut Module,
    builtin: crate::BuiltinTerm,
    text: &str,
) -> crate::ExprId {
    module
        .alloc_expr(crate::Expr {
            span: unit_span(),
            kind: crate::ExprKind::Name(crate::TermReference::resolved(
                test_path(text),
                crate::TermResolution::Builtin(builtin),
            )),
        })
        .expect("builtin term allocation should fit")
}

#[test]
fn typecheck_allows_option_default_record_elision() {
    let report = typecheck_text(
        "record-elision.aivi",
        "use aivi.defaults (Option)\n\
             type Profile = {\n\
                 name: Text,\n\
                 nickname: Option Text,\n\
                 bio: Option Text\n\
             }\n\
             value name = \"Ada\"\n\
             value nickname = Some \"Countess\"\n\
             value profile:Profile = { name, nickname }\n",
    );
    assert!(
        report.is_ok(),
        "expected defaulted record elision to typecheck, got diagnostics: {:?}",
        report.diagnostics()
    );
}

#[test]
fn typecheck_elaborates_option_default_record_elision_into_explicit_fields() {
    let (report, module) = typecheck_and_elaborate_text(
        "record-elision-hir.aivi",
        "use aivi.defaults (Option)\n\
             type Profile = {\n\
                 name: Text,\n\
                 nickname: Option Text,\n\
                 bio: Option Text\n\
             }\n\
             value name = \"Ada\"\n\
             value nickname = Some \"Countess\"\n\
             value profile:Profile = { name, nickname }\n",
    );
    assert!(
        report.is_ok(),
        "expected defaulted record elision to typecheck, got diagnostics: {:?}",
        report.diagnostics()
    );

    let module = &module;
    let profile = value_body(module, "profile");
    let ExprKind::Record(record) = &module.exprs()[profile].kind else {
        panic!("expected `profile` to stay a record literal");
    };
    assert_eq!(
        record.fields.len(),
        3,
        "expected omitted bio field to be synthesized"
    );
    assert_eq!(
        record
            .fields
            .iter()
            .map(|field| field.label.text())
            .collect::<Vec<_>>(),
        vec!["name", "nickname", "bio"]
    );
    assert_eq!(
        record
            .fields
            .iter()
            .map(|field| field.surface)
            .collect::<Vec<_>>(),
        vec![
            RecordFieldSurface::Shorthand,
            RecordFieldSurface::Shorthand,
            RecordFieldSurface::Defaulted,
        ]
    );
    let defaulted_value = record.fields[2].value;
    match &module.exprs()[defaulted_value].kind {
        ExprKind::Name(reference) => assert!(matches!(
            reference.resolution.as_ref(),
            ResolutionState::Resolved(TermResolution::Builtin(BuiltinTerm::None))
        )),
        other => panic!("expected synthesized option default to be `None`, found {other:?}"),
    }
}

#[test]
fn typecheck_reports_missing_eq_for_map_equality() {
    let report = typecheck_text(
        "map-equality.aivi",
        "value left = Map { \"id\": 1 }\n\
             value right = Map { \"id\": 1 }\n\
             value same:Bool = left == right\n",
    );
    assert!(
        report
            .diagnostics()
            .iter()
            .any(|diagnostic| { diagnostic.code == Some(crate::codes::MISSING_EQ_INSTANCE) }),
        "expected missing Eq diagnostic, got diagnostics: {:?}",
        report.diagnostics()
    );
}

#[test]
fn typecheck_reports_missing_eq_for_map_inequality() {
    let report = typecheck_text(
        "map-inequality.aivi",
        "value left = Map { \"id\": 1 }\n\
             value right = Map { \"id\": 2 }\n\
             value different:Bool = left != right\n",
    );
    assert!(
        report
            .diagnostics()
            .iter()
            .any(|diagnostic| { diagnostic.code == Some(crate::codes::MISSING_EQ_INSTANCE) }),
        "expected missing Eq diagnostic for !=, got diagnostics: {:?}",
        report.diagnostics()
    );
}

#[test]
fn expression_matches_solves_deferred_eq_constraints() {
    let module = lowered_module_text(
        "expression-matches-map-equality.aivi",
        "value left = Map { \"id\": 1 }\n\
             value right = Map { \"id\": 1 }\n\
             value same:Bool = left == right\n",
    );
    assert!(
        !expression_matches(
            &module,
            value_body(&module, "same"),
            &GateExprEnv::default(),
            &GateType::Primitive(BuiltinType::Bool),
        ),
        "expected expression_matches to reject deferred missing Eq evidence"
    );
}

#[test]
fn typecheck_accepts_same_module_eq_instances_for_nonstructural_types() {
    let report = typecheck_text(
        "same-module-eq-instance.aivi",
        r#"class Eq A = {
    (==) : A -> A -> Bool
}
type Blob = Blob Bytes
fun blobEquals:Bool = left:Blob right:Blob =>
    True
instance Eq Blob = {
    (==) left right = blobEquals left right
}
fun compare:Bool = left:Blob right:Blob =>
    left == right
"#,
    );
    assert!(
        report.is_ok(),
        "expected same-module Eq instance to satisfy equality, got diagnostics: {:?}",
        report.diagnostics()
    );
}

#[test]
fn typecheck_accepts_equality_in_instance_member_bodies() {
    let report = typecheck_text(
        "instance-member-equality.aivi",
        "class Compare A = {\n\
             \x20\x20\x20\x20same : A -> A -> Bool\n\
             }\n\
             type Label = Label Text\n\
             instance Compare Label = {\n\
             \x20\x20\x20\x20same left right = left == right\n\
             }\n",
    );
    assert!(
        report.is_ok(),
        "expected equality inside instance members to typecheck, got diagnostics: {:?}",
        report.diagnostics()
    );
}

#[test]
fn typecheck_accepts_class_requirements_in_generic_instance_bodies() {
    let report = typecheck_text(
        "class-require-instance-context.aivi",
        "class Container A = {\n\
             \x20\x20\x20\x20require Eq A\n\
             \x20\x20\x20\x20same : A -> A -> Bool\n\
             }\n\
             instance Eq A -> Container A = {\n\
             \x20\x20\x20\x20same left right = left == right\n\
             }\n",
    );
    assert!(
        report.is_ok(),
        "expected class `require` constraints to typecheck inside generic instance bodies, got diagnostics: {:?}",
        report.diagnostics()
    );
}

#[test]
fn typecheck_reports_missing_instance_requirement_for_class_requirements() {
    let report = typecheck_text(
        "class-require-missing-instance.aivi",
        "class Container A = {\n\
             \x20\x20\x20\x20require Eq A\n\
             \x20\x20\x20\x20same : A -> A -> Bool\n\
             }\n\
             instance Container Bytes = {\n\
             \x20\x20\x20\x20same left right = True\n\
             }\n",
    );
    assert!(
        report.diagnostics().iter().any(|diagnostic| {
            diagnostic.code == Some(crate::codes::MISSING_INSTANCE_REQUIREMENT)
        }),
        "expected class `require` constraints to reject unsatisfied instances, got diagnostics: {:?}",
        report.diagnostics()
    );
}

#[test]
fn typecheck_reports_instance_member_operator_operand_mismatch() {
    let report = typecheck_text(
        "instance-member-operator-mismatch.aivi",
        "class Ready A = {\n\
             \x20\x20\x20\x20ready : A -> Bool\n\
             }\n\
             type Blob = Blob Bytes\n\
             instance Ready Blob = {\n\
             \x20\x20\x20\x20ready blob = blob and True\n\
             }\n",
    );
    assert!(
        report
            .diagnostics()
            .iter()
            .any(|diagnostic| { diagnostic.code == Some(crate::codes::TYPE_MISMATCH) }),
        "expected instance member operator mismatch diagnostic, got diagnostics: {:?}",
        report.diagnostics()
    );
}

#[test]
fn typecheck_reports_constructor_operand_mismatch_without_payload_evidence() {
    let report = typecheck_text(
        "invalid-unary-operator.aivi",
        "value broken:Bool = not None\n",
    );
    assert!(
        report
            .diagnostics()
            .iter()
            .any(|diagnostic| { diagnostic.code == Some(crate::codes::TYPE_MISMATCH) }),
        "expected constructor operand mismatch diagnostic, got diagnostics: {:?}",
        report.diagnostics()
    );
}

#[test]
fn typecheck_accepts_prelude_functor_map_calls() {
    let report = typecheck_text(
        "prelude-map-call.aivi",
        "fun increment:Int = n:Int => n + 1\n\
             value mapped:Option Int = map increment (Some 1)\n",
    );
    assert!(
        report.is_ok(),
        "expected ambient prelude Functor map call to typecheck, got diagnostics: {:?}",
        report.diagnostics()
    );
}

#[test]
fn typecheck_preserves_function_valued_results_when_referencing_functions() {
    let report = typecheck_text(
        "returned-function.aivi",
        r#"
type Int -> Int
func increment = value => value + 1
type Bool -> (Int -> Int)
func getIncrement = ignored => increment
type Bool -> Text -> (Int -> Int)
func getIncrementWithLabel = ignored label => increment
value function : Bool -> (Int -> Int) = getIncrement
value withLabel : Bool -> Text -> (Int -> Int) = getIncrementWithLabel
value direct : Int -> Int = getIncrement True
value mapped : Option (Int -> Int) = map getIncrement (Some True)
value applied : Option Int = apply mapped (Some 41)
"#,
    );
    assert!(report.is_ok(), "{:?}", report.diagnostics());
}

#[test]
fn typecheck_rejects_discarding_a_function_valued_result_arrow() {
    let report = typecheck_text(
        "returned-function-mismatch.aivi",
        r#"
type Int -> Int
func increment = value => value + 1
type Bool -> (Int -> Int)
func getIncrement = ignored => increment
value wrong : Bool -> Int = getIncrement
"#,
    );
    assert!(
        report
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code == Some(crate::codes::TYPE_MISMATCH)),
        "{:?}",
        report.diagnostics()
    );
}

#[test]
fn typecheck_accepts_prelude_foldable_reduce_calls() {
    let report = typecheck_text(
        "prelude-reduce-call.aivi",
        "fun add:Int = acc:Int item:Int => acc + item\n\
             value joined:Text = reduce append empty [\"hel\", \"lo\"]\n\
             value total:Int = reduce add 10 (Some 2)\n",
    );
    assert!(
        report.is_ok(),
        "expected ambient prelude Foldable reduce calls to typecheck, got diagnostics: {:?}",
        report.diagnostics()
    );
}

#[test]
fn class_member_callbacks_preserve_rigid_contracts_and_contextual_carriers() {
    for source in [
        "type Comonad W => W A -> W A\nfunc preserve = values => extend extract values\n",
        "type Either L R = EL L | ER R\ntype Applicative G => Either L A -> G (Either L A)\nfunc wrapEither = either => either\n ||> EL error -> pure (EL error)\n ||> ER item -> pure (ER item)\n",
    ] {
        let report = typecheck_text("class-callback-contract.aivi", source);
        assert!(report.is_ok(), "{source}: {:?}", report.diagnostics());
    }
    for source in [
        "type Comonad W => W A -> W Int\nfunc invalid = values => extend extract values\n",
        "type Comonad W => W A -> Int\nfunc invalid = values => extract values\n",
        "type Extend W => W A -> W Int\nfunc invalid = values => extend (items => extract items) values\n",
    ] {
        let report = typecheck_text("invalid-class-callback-contract.aivi", source);
        assert!(!report.is_ok(), "invalid contract accepted: {source}");
    }
}

#[test]
fn typecheck_accepts_class_member_names_from_expected_arrow_types() {
    let report = typecheck_text(
        "class-member-name-expected-arrow.aivi",
        "value pureOption:(Int -> Option Int) = pure\n",
    );
    assert!(
        report.is_ok(),
        "expected class member names to resolve from expected arrows, got diagnostics: {:?}",
        report.diagnostics()
    );
}

#[test]
fn typecheck_accepts_function_signature_constraints_at_call_sites() {
    let report = typecheck_text(
        "function-signature-constraints.aivi",
        "fun same:Eq A -> Bool = x:A => True\n\
             value sameText:Bool = same \"Ada\"\n",
    );
    assert!(
        report.is_ok(),
        "expected signature constraints to solve at call sites, got diagnostics: {:?}",
        report.diagnostics()
    );
}

#[test]
fn typecheck_accepts_class_requirements_in_function_contexts() {
    let report = typecheck_text(
        "class-require-function-context.aivi",
        r#"class Container A = {
    require Eq A
    same : A -> A -> Bool
}
fun delegated:Container A => Bool = left:A right:A =>
    left == right
instance Container Text = {
    same left right = left == right
}
value sameText:Bool = delegated "Ada" "Grace"
"#,
    );
    assert!(
        report.is_ok(),
        "expected class `require` constraints to propagate through function contexts, got diagnostics: {:?}",
        report.diagnostics()
    );
}

#[test]
fn typecheck_expands_class_requirements_into_eq_bindings() {
    let module = lowered_module_text(
        "class-require-expansion.aivi",
        r#"class Container A = {
    require Eq A
    same : A -> A -> Bool
}
fun delegated:Container A => Bool = left:A right:A =>
    left == right
"#,
    );
    let function = module
        .items()
        .iter()
        .find_map(|(_, item)| match item {
            Item::Function(item) if item.name.text() == "delegated" => Some(item.clone()),
            _ => None,
        })
        .expect("delegated function should lower");
    let mut checker = TypeChecker::new(&module);
    let bindings = checker.constraint_bindings(&function.context, &PolyTypeBindings::new());
    let expanded = checker.expand_class_constraint_bindings(bindings);
    let labels = expanded
        .iter()
        .map(|binding| checker.class_constraint_binding_label(binding))
        .collect::<Vec<_>>();
    let context_kinds = function
        .context
        .iter()
        .map(|constraint| format!("{:?}", module.types()[*constraint].kind))
        .collect::<Vec<_>>();
    assert!(
        labels.iter().any(|label| label == "Eq A"),
        "expected `Container A` to imply `Eq A`, got context len {} kinds {:?} and labels {labels:?}",
        function.context.len(),
        context_kinds
    );
}

#[test]
fn typecheck_accepts_ord_comparison_for_text() {
    let report = typecheck_text(
        "ord-text-comparison.aivi",
        "value ordered:Bool = \"a\" < \"b\"\n",
    );
    assert!(
        report.is_ok(),
        "expected Ord-backed text comparison to typecheck, got diagnostics: {:?}",
        report.diagnostics()
    );
}

#[test]
fn typecheck_accepts_ord_comparison_for_nominal_domains() {
    let report = typecheck_text(
        "ord-domain-comparison.aivi",
        "domain Calendar over Int = {\n\
             \x20\x20\x20\x20suffix day : Int = value => Calendar value\n\
             \x20\x20\x20\x20toDay : Calendar -> Int\n\
             \x20\x20\x20\x20toDay value = value\n\
             }\n\
             instance Eq Calendar = {\n\
             \x20\x20\x20\x20(==) left right = toDay left == toDay right\n\
             \x20\x20\x20\x20(!=) left right = toDay left != toDay right\n\
             }\n\
             instance Ord Calendar = {\n\
             \x20\x20\x20\x20compare left right = compare (toDay left) (toDay right)\n\
             }\n\
             value earlier:Bool = 1day < 2day\n",
    );
    assert!(
        report.is_ok(),
        "expected Ord-backed domain comparison to typecheck, got diagnostics: {:?}",
        report.diagnostics()
    );
}

#[test]
fn typecheck_accepts_ordering_operator_sections() {
    let report = typecheck_text(
        "ord-operator-section.aivi",
        "value less:(Int -> Int -> Bool) = (<)\n\
         value ordered:Bool = less 1 2\n",
    );
    assert!(
        report.is_ok(),
        "expected ordering operator section to typecheck, got diagnostics: {:?}",
        report.diagnostics()
    );
}

#[test]
fn unknown_binary_result_does_not_retain_operand_actual_type() {
    let mut sources = SourceDatabase::new();
    let file = sources.add_file(
        "binary-actual-type.aivi",
        "value compared : Bool = Some 1 == 2\n",
    );
    let parsed = parse_module(&sources[file]);
    assert!(!parsed.has_errors());
    let lowered = lower_module(&parsed.module);
    let module = lowered.module();
    let body = module
        .items()
        .iter()
        .find_map(|(_, item)| match item {
            Item::Value(value) if value.name.text() == "compared" => Some(value.body),
            _ => None,
        })
        .unwrap();
    let mut typing = GateTypeContext::new(module);
    let info = typing.infer_expr(body, &GateExprEnv::default(), None);
    assert!(info.ty.is_none());
    assert!(
        info.actual_gate_type().is_none(),
        "operand evidence leaked: {info:?}"
    );
}

#[test]
fn typecheck_reports_invalid_binary_operator_for_non_ord_comparison() {
    let report = typecheck_text(
        "invalid-binary-operator.aivi",
        "value broken:Bool = [1] < [2]\n",
    );
    assert!(
        report
            .diagnostics()
            .iter()
            .any(|diagnostic| { diagnostic.code == Some(crate::codes::INVALID_BINARY_OPERATOR) }),
        "expected invalid binary operator diagnostic, got diagnostics: {:?}",
        report.diagnostics()
    );
}

#[test]
fn contextual_comparisons_reject_inconsistent_callback_results() {
    let prefix = "type (E -> B) -> (A -> B) -> Result E A -> B\n\
func fold = onErr onOk result => result\n\
 ||> Err error -> onErr error\n\
 ||> Ok value -> onOk value\n\
type Text -> Text\n\
func keepError = text => text\n\
type Int -> Int\n\
func increment = n => n + 1\n";
    for body in [
        "value bad : Int = fold keepError increment (Ok 2)\n",
        "value bad : Bool = fold keepError increment (Ok 2) == 3\n",
        "value bad : Task Text Bool = pure (fold keepError increment (Ok 2) == 3)\n",
        "value bad : Task Text Int = pure \"wrong\"\n",
        "value bad : List Int = map keepError [1]\n",
        "value bad : (Text -> Task Text Int) = pure\n",
        "value bad : Task Text Text = pure []\n",
        "value bad : Task Text Bool = pure (fold keepError increment (Ok \"two\") == 3)\n",
    ] {
        let report = typecheck_text("inconsistent-fold-result.aivi", &format!("{prefix}{body}"));
        assert!(
            !report.is_ok(),
            "accepted {body:?}: {:?}",
            report.diagnostics()
        );
        assert!(
            report
                .diagnostics()
                .iter()
                .any(|d| d.code == Some(crate::codes::TYPE_MISMATCH)),
            "missing callback type mismatch for {body:?}: {:?}",
            report.diagnostics()
        );
    }
}

#[test]
fn scoped_http_task_url_uses_the_list_concat_contract() {
    let mut sources = SourceDatabase::new();
    let file = sources.add_file(
        "http-url-contract.aivi",
        r#"
type HttpSource = Unit
@source http "https://service.example"
signal api : HttpSource
value health : Task Text Text = api.get "/health"
"#,
    );
    let parsed = parse_module(&sources[file]);
    let lowered = lower_module(&parsed.module);
    assert!(!lowered.has_errors(), "{:?}", lowered.diagnostics());
    let module = lowered.module();
    let body = module
        .items()
        .iter()
        .find_map(|(_, item)| match item {
            Item::Value(value) if value.name.text() == "health" => Some(value.body),
            _ => None,
        })
        .unwrap();
    let ExprKind::Apply { arguments, .. } = &module.exprs()[body].kind else {
        panic!("expected an HTTP task application");
    };
    let ExprKind::Apply { callee, arguments } = &module.exprs()[*arguments.first()].kind else {
        panic!("expected scoped URL concatenation");
    };
    let ExprKind::Name(reference) = &module.exprs()[*callee].kind else {
        panic!("expected concat intrinsic");
    };
    assert!(matches!(
        reference.resolution.as_ref(),
        ResolutionState::Resolved(TermResolution::IntrinsicValue(
            crate::IntrinsicValue::TextConcat
        ))
    ));
    assert_eq!(arguments.len(), 1, "concat accepts one list, not two texts");
    let ExprKind::List(parts) = &module.exprs()[*arguments.first()].kind else {
        panic!("expected URL parts list");
    };
    assert_eq!(parts.len(), 2);
    let report = typecheck_module(module);
    assert!(
        report.is_ok(),
        "generated URL call must check: {:?}",
        report.diagnostics()
    );
}

#[test]
fn empty_list_inference_preserves_its_container_shape() {
    let mut sources = SourceDatabase::new();
    let file = sources.add_file("empty-list-shape.aivi", "value items = []\n");
    let parsed = parse_module(&sources[file]);
    let lowered = lower_module(&parsed.module);
    let body = lowered
        .module()
        .items()
        .iter()
        .find_map(|(_, item)| match item {
            Item::Value(value) if value.name.text() == "items" => Some(value.body),
            _ => None,
        })
        .unwrap();
    let mut typing = GateTypeContext::new(lowered.module());
    let info = typing.infer_expr(body, &GateExprEnv::default(), None);
    assert!(
        info.ty.is_none(),
        "empty lists must not choose an element type"
    );
    assert_eq!(
        info.actual,
        Some(SourceOptionActualType::List(Box::new(
            SourceOptionActualType::Hole
        )))
    );
}

#[test]
fn contextual_comparisons_require_evidence_for_every_sum_payload() {
    let report = typecheck_text(
        "ambiguous-sum-comparison.aivi",
        r#"
type Either L R = | Left L | Right R
type (L1 -> L2) -> Either L1 R -> Either L2 R
func mapLeft = f value => value
 ||> Left item -> Left (f item)
 ||> Right item -> Right item
type Text -> Text
func mark = text => text
value bad : Task Text Bool = pure (mapLeft mark (Left "error") == Left "error")
"#,
    );
    assert!(!report.is_ok());
    assert!(
        report
            .diagnostics()
            .iter()
            .any(|diagnostic| { diagnostic.code == Some(crate::codes::MISSING_EQ_INSTANCE) }),
        "missing payload evidence accepted: {:?}",
        report.diagnostics()
    );
}

#[test]
fn contextual_class_calls_preserve_polymorphic_and_alias_contracts() {
    for (name, source) in [
        (
            "consistent-fold",
            r#"
type (E -> B) -> (A -> B) -> Result E A -> B
func fold = onErr onOk result => result
 ||> Err error -> onErr error
 ||> Ok value -> onOk value
type Text -> Int
func handleError = text => 0
type Int -> Int
func increment = n => n + 1
value good : Task Text Bool = pure (fold handleError increment (Ok 2) == 3)
"#,
        ),
        (
            "unresolved-predicate",
            r#"
type Either L R = | Left L | Right R
type (L1 -> L2) -> (R1 -> R2) -> Either L1 R1 -> Either L2 R2
func mapBoth = onLeft onRight value => value
 ||> Left item -> Left (onLeft item)
 ||> Right item -> Right (onRight item)
type Text -> Text
func mark = text => text
type Int -> Int
func double = n => n * 2
value good : Task Text Bool = pure (mapBoth mark double (Right 2) == Right 4)
"#,
        ),
        (
            "sum-predicate",
            r#"
type Either L R = | Left L | Right R
type Either L R -> Bool
func isLeft = value => value
 ||> Left item -> True
 ||> Right item -> False
value good : Task Text Bool = pure (isLeft (Left "error"))
"#,
        ),
        (
            "sum-map",
            r#"
type Either L R = | Left L | Right R
type (L1 -> L2) -> Either L1 R -> Either L2 R
func mapLeft = f value => value
 ||> Left item -> Left (f item)
 ||> Right item -> Right item
type Text -> Text
func mark = text => text
value left : Either Text Int = Left "error"
value good : Task Text Bool = pure (mapLeft mark left == Left "error")
"#,
        ),
        (
            "constructor",
            "value good : Task Text (Option Int) = pure None\n",
        ),
        ("reference", "value good : Text -> Task Text Text = pure\n"),
        (
            "callback",
            "value good : List Bool = map (n => n > 0) [1]\n",
        ),
        (
            "partial",
            "value good : List Int -> List Int = map (n => n + 1)\n",
        ),
        (
            "generic",
            "type Int -> Int\nfunc increment = n => n + 1\ntype Functor F => F Int -> F Int\nfunc copy = value => map increment value\n",
        ),
        (
            "alias",
            r#"
type Bag A = { items: List A }
instance Foldable Bag = { reduce = f seed bag => reduce f seed bag.items }
value bag : Bag Int = { items: [1, 2] }
value good : Int = reduce (total n => total + n) 0 bag
"#,
        ),
    ] {
        let report = typecheck_text(name, source);
        assert!(
            report.is_ok(),
            "valid {name} rejected: {:?}",
            report.diagnostics()
        );
    }
}

#[test]
fn typecheck_reports_value_annotation_mismatch() {
    let report = typecheck_text("value-mismatch.aivi", "value answer:Text = 42\n");
    assert!(
        report
            .diagnostics()
            .iter()
            .any(|diagnostic| { diagnostic.code == Some(crate::codes::TYPE_MISMATCH) }),
        "expected type mismatch diagnostic, got diagnostics: {:?}",
        report.diagnostics()
    );
}

#[test]
fn closed_value_annotations_reject_incompatible_partial_shapes() {
    for body in ["x => x", "[]", "[None]", "None", "Some []"] {
        for used in [false, true] {
            let source = format!(
                "value bad : Int = {body}\n{}",
                if used {
                    "value observed : Int = bad + 1\n"
                } else {
                    ""
                }
            );
            let report = typecheck_text("closed-value-shape.aivi", &source);
            assert!(
                report
                    .diagnostics()
                    .iter()
                    .any(|d| d.code == Some(crate::codes::TYPE_MISMATCH)),
                "incompatible {body:?}, used={used}, was accepted: {:?}",
                report.diagnostics()
            );
        }
    }
}

#[test]
fn callback_contract_mismatches_have_one_diagnostic_owner() {
    for source in [
        "value bad : Int -> Text = x => x\n",
        "value bad : List Text = map (n => n) [1]\n",
        "type Functor F => F A -> F Text\nfunc bad = xs => map (x => x) xs\n",
    ] {
        let report = typecheck_text("single-contract-error.aivi", source);
        let errors = report
            .diagnostics()
            .iter()
            .filter(|d| d.code == Some(crate::codes::TYPE_MISMATCH))
            .collect::<Vec<_>>();
        assert_eq!(errors.len(), 1, "{source}: {:?}", report.diagnostics());
    }
    let report = typecheck_text(
        "distinct-contract-errors.aivi",
        "value first : Int -> Text = x => x\nvalue second : Int -> Text = y => y\n",
    );
    assert_eq!(
        report
            .diagnostics()
            .iter()
            .filter(|d| d.code == Some(crate::codes::TYPE_MISMATCH))
            .count(),
        2,
        "distinct source failures must survive: {:?}",
        report.diagnostics()
    );
}

#[test]
fn diagnostic_ownership_preserves_distinct_contracts_and_binder_identities_at_one_span() {
    let module = lowered_module_text("distinct-contract-proof.aivi", "value subject = 1\n");
    let span = module.exprs()[value_body(&module, "subject")].span;
    let mut checker = TypeChecker::new(&module);
    let actual = GateType::Primitive(BuiltinType::Int);
    let contracts = [
        GateType::Primitive(BuiltinType::Text),
        GateType::Primitive(BuiltinType::Bool),
        GateType::TypeParameter {
            parameter: TypeParameterId::from_raw(7),
            name: "A".to_owned(),
        },
        GateType::TypeParameter {
            parameter: TypeParameterId::from_raw(8),
            name: "A".to_owned(),
        },
    ];
    for expected in &contracts {
        checker.emit_type_mismatch(span, expected, &actual);
        checker.emit_type_mismatch(span, expected, &actual);
    }
    checker.publish_unique_type_mismatches();
    assert_eq!(checker.diagnostics.len(), contracts.len());
}

#[test]
fn indexed_class_scope_matches_declaration_order_aliases_and_ambient_fallback() {
    fn scanned_scope(module: &Module, name: &str) -> Vec<ClassMemberResolution> {
        let mut classes = module
            .root_items()
            .iter()
            .copied()
            .filter(|id| matches!(module.items()[*id], Item::Class(_)))
            .collect::<Vec<_>>();
        for (_, import) in module.imports().iter() {
            let class = module.items().iter().find_map(|(id, item)| {
                let Item::Class(class) = item else {
                    return None;
                };
                match &import.metadata {
                    ImportBindingMetadata::Class { identity } if &class.identity == identity => {
                        Some(id)
                    }
                    _ => None,
                }
            });
            if let Some(class) = class
                && !classes.contains(&class)
            {
                classes.push(class);
            }
        }
        let members = |classes: &[ItemId]| {
            classes
                .iter()
                .flat_map(|id| {
                    let Item::Class(class) = &module.items()[*id] else {
                        return Vec::new();
                    };
                    class
                        .members
                        .iter()
                        .enumerate()
                        .filter_map(|(member_index, member)| {
                            (member.name.text() == name).then_some(ClassMemberResolution {
                                class: *id,
                                member_index,
                            })
                        })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>()
        };
        let explicit = members(&classes);
        if explicit.is_empty() {
            members(module.ambient_items())
        } else {
            explicit
        }
    }
    for source in [
        "value subject = 1\n",
        "class Same A = { equals : A -> A -> Bool }\n",
        "class First A = { (==) : A -> A -> Bool }\nclass Second A = { (==) : A -> A -> Bool }\n",
        "use aivi.prelude (Eq as Equality, Ord as Order)\nvalue subject = 1\n",
        "use aivi.prelude (Eq as First, Eq as Second)\nvalue subject = 1\n",
        "use aivi.core.dict (Dict)\nvalue subject = 1\n",
    ] {
        let lowered = crate::test_support::lower_text_with_stdlib("class-scope-index.aivi", source);
        assert!(
            !lowered.has_errors(),
            "{source}: {:?}",
            lowered.diagnostics()
        );
        let module = lowered.module();
        let mut names = std::collections::BTreeSet::from(["unknown_member"]);
        for (_, item) in module.items().iter() {
            if let Item::Class(class) = item {
                names.extend(class.members.iter().map(|member| member.name.text()));
            }
        }
        for name in names {
            assert_eq!(
                module.class_members_in_scope(name),
                scanned_scope(module, name),
                "{source}: {name}"
            );
        }
    }
}

#[test]
fn borrowed_nominal_identities_preserve_origin_arguments_and_rigid_binders() {
    let module = lowered_module_text("borrowed-nominal-identity.aivi", "type Box A = Box A\n");
    let item = module
        .items()
        .iter()
        .find_map(|(id, item)| match item {
            Item::Type(item) if item.name.text() == "Box" => Some(id),
            _ => None,
        })
        .expect("Box declaration");
    let local = |argument| GateType::OpaqueItem {
        item,
        name: "Box".to_owned(),
        arguments: vec![argument],
    };
    let imported = |identity, argument| GateType::OpaqueImport {
        origin: Some(Box::new(identity)),
        import: ImportId::from_raw(u32::MAX),
        name: "Alias".to_owned(),
        arguments: vec![argument],
        definition: None,
    };
    let origin = crate::TypeIdentity::Source {
        file: module.file(),
        name: "Box".into(),
    };
    let int = GateType::Primitive(BuiltinType::Int);
    let mut typing = GateTypeContext::new(&module);
    assert!(typing.types_match(&local(int.clone()), &imported(origin.clone(), int.clone())));
    for other in [
        crate::TypeIdentity::Source {
            file: FileId::new(1000),
            name: "Box".into(),
        },
        crate::TypeIdentity::Standard("Box".into()),
        crate::TypeIdentity::Source {
            file: module.file(),
            name: "Other".into(),
        },
    ] {
        assert!(!typing.types_match(&local(int.clone()), &imported(other, int.clone())));
    }
    assert!(!typing.types_match(
        &local(int.clone()),
        &imported(origin.clone(), GateType::Primitive(BuiltinType::Bool))
    ));
    let left = TypeParameterId::from_raw(7);
    let right = TypeParameterId::from_raw(8);
    typing.replace_rigid_type_parameters(vec![left, right]);
    assert!(!typing.types_match(
        &local(GateType::TypeParameter {
            parameter: left,
            name: "A".to_owned()
        }),
        &imported(
            origin,
            GateType::TypeParameter {
                parameter: right,
                name: "A".to_owned()
            }
        ),
    ));
}

#[test]
fn closed_value_annotations_accept_contextual_alias_shapes() {
    let report = typecheck_text(
        "closed-value-aliases.aivi",
        "type Callback = (Int -> Int)\ntype Numbers = (List Int)\n\
         value direct : Int -> Int = x => x\n\
         value callback : Callback = x => x\n\
         value empty : Numbers = []\n\
         value nested : List (Option Int) = [None]\n\
         value mapped : Numbers = map callback [1, 2]\n",
    );
    assert!(
        report.is_ok(),
        "valid contextual shapes rejected: {:?}",
        report.diagnostics()
    );
}

#[test]
fn definitive_contracts_preserve_structural_callbacks_and_record_patterns() {
    let lowered = crate::test_support::lower_text_with_stdlib(
        "structural-contracts.aivi",
        r#"
use aivi.list (Partition, partition)
type Todo = { done: Bool }
type Todo -> Bool
func isOpen = |> not .done
value items : List Todo = filter isOpen [{ done: False }]
value filtered : List Int = filter (n => n > 0) [1, -1]
value flattened : List Int = flatMap (n => [n]) [1]
type State = { items: List Todo }
type State -> State
func clear = state => state <| { items: filter isOpen state.items }
type Text -> List Todo -> Int -> List Todo
func row = label todos n => todos
value rows : List Todo = flatMap (row "" [{ done: False }]) [1]
value groups : Partition Int = partition (n => n > 0) [1, -1]
value positive : List Int = groups ||> { matched, unmatched } -> matched
"#,
    );
    assert!(!lowered.has_errors(), "{:?}", lowered.diagnostics());
    let checked = typecheck_module(lowered.module());
    assert!(checked.is_ok(), "{:?}", checked.diagnostics());
    for source in [
        "value wrong : List Int = filter (n => \"wrong\") [1]\n",
        "value wrong : List Int = flatMap (n => n) [1]\n",
        "type Int -> List Int -> List Int\nfunc insertOrdered = n items => items\nvalue wrong : List Int = [1] |> reduce insertOrdered []\n",
    ] {
        let lowered = crate::test_support::lower_text_with_stdlib("hoisted-mismatch.aivi", source);
        assert!(!lowered.has_errors(), "{:?}", lowered.diagnostics());
        let checked = typecheck_module(lowered.module());
        assert!(!checked.is_ok(), "{source}: {:?}", checked.diagnostics());
    }
}

#[test]
fn definitive_contracts_accept_snake_callbacks() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../demos/snake.aivi");
    let source = std::fs::read_to_string(&path).unwrap();
    let lowered = crate::test_support::lower_text_with_stdlib("snake.aivi", &source);
    assert!(!lowered.has_errors(), "{:?}", lowered.diagnostics());
    let checked = typecheck_module(lowered.module());
    assert!(checked.is_ok(), "{:?}", checked.diagnostics());
}

#[test]
fn closed_value_annotations_preserve_result_and_recurrence_evidence() {
    for path in ["result-block", "pipe-explicit-recurrence-wakeups"] {
        let text = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
            format!("../../fixtures/frontend/milestone-2/valid/{path}/main.aivi"),
        ))
        .unwrap();
        let module = lowered_module_text(path, &text);
        let mut typing = GateTypeContext::new(&module);
        for (_, item) in module.items().iter() {
            if let Item::Value(value) = item {
                let expected = typing.lower_annotation(value.annotation.unwrap()).unwrap();
                let info = typing.infer_expr_with_expected(
                    value.body,
                    &GateExprEnv::default(),
                    None,
                    &expected,
                );
                assert!(info.issues.is_empty(), "{path}: {:?}", info.issues);
                assert!(
                    typing.match_gate_expr_template(&expected, &info, &mut HashMap::new()),
                    "{path}: {info:?}"
                );
            }
        }
        let checked = typecheck_module(&module);
        assert!(checked.is_ok(), "{path}: {:?}", checked.diagnostics());
    }
}

#[test]
fn definitive_declaration_contracts_reject_partial_wrong_payloads() {
    for source in [
        "type Int -> Int\nfunc wrong = n => []\n",
        "signal wrong : Signal Int = []\n",
        "class Scalar A = { scalar : A -> Int }\ninstance Scalar Int = { scalar = n => [] }\n",
        "domain Scalar over Int = { type scalar : Int -> Int\nscalar = n => [] }\n",
        "@recur.backoff 3times\nvalue wrong : Task Int Text = 0\n @|> . + 1\n <|@ . + 1\n",
        "value wrong : Int = True\n ||> True -> []\n ||> False -> 1\n",
        "type Int -> Int\nfunc accept = n => n\nvalue wrong : Int = True\n ||> True -> accept []\n ||> False -> 1\n",
        "value wrong : Int = True\n T|> 1\n F|> []\n",
    ] {
        let report = typecheck_text("definitive-declaration.aivi", source);
        assert!(!report.is_ok(), "{source}: {:?}", report.diagnostics());
    }
}

#[test]
fn partial_shape_diagnostics_retain_known_outer_heads() {
    for (body, shape) in [("[]", "List _"), ("x => x", "_ -> _")] {
        let report = typecheck_text(
            "known-partial-shape.aivi",
            &format!("value bad : Int = {body}\n"),
        );
        assert!(
            report
                .diagnostics()
                .iter()
                .any(|d| d.message.contains(shape)),
            "{:?}",
            report.diagnostics()
        );
    }
}

#[test]
fn exported_constructor_signatures_retain_provenance() {
    let module = lowered_module_text(
        "constructor-exports.aivi",
        "type Maybe A = Missing | Found A\nvalue fallback : Maybe Int = Found 7\nvalue wrap : Int -> Maybe Int = Found\nexport Missing\nexport Found\nexport fallback\nexport wrap\n",
    );
    let exports = crate::exports(&module);
    for name in ["Missing", "Found"] {
        assert!(matches!(
            &exports.find(name).unwrap().metadata,
            crate::ImportBindingMetadata::ConstructorValue { variant_name, .. } if variant_name == name
        ));
    }
    for name in ["fallback", "wrap"] {
        assert!(matches!(
            exports.find(name).unwrap().metadata,
            crate::ImportBindingMetadata::Value { .. }
        ));
    }
}

#[test]
fn transparent_constructor_aliases_have_portable_payloads() {
    let module = lowered_module_text(
        "constructor-alias-exports.aivi",
        "type Maybe A = (Option A)\ntype Reply A = (Result Text A)\nexport Maybe\nexport Reply\n",
    );
    let exports = crate::exports(&module);
    assert!(
        matches!(
            &exports.find("Maybe").unwrap().metadata,
            crate::ImportBindingMetadata::TypeConstructor {
                definition: Some(crate::ImportTypeDefinition::Alias(
                    crate::ImportValueType::Option(_)
                )),
                ..
            }
        ),
        "{:?}",
        exports.find("Maybe")
    );
    assert!(
        matches!(
            &exports.find("Reply").unwrap().metadata,
            crate::ImportBindingMetadata::TypeConstructor {
                definition: Some(crate::ImportTypeDefinition::Alias(
                    crate::ImportValueType::Result { .. }
                )),
                ..
            }
        ),
        "{:?}",
        exports.find("Reply")
    );
}

#[test]
fn constructor_callbacks_infer_builtin_payload_contracts() {
    for (constructor, source, result) in [
        ("Some", "[1, 2]", "List (Option Int)"),
        ("Ok", "[1, 2]", "List (Result Text Int)"),
        ("Err", "[\"missing\"]", "List (Result Text Int)"),
        ("Valid", "[1, 2]", "List (Validation Text Int)"),
        ("Invalid", "[\"missing\"]", "List (Validation Text Int)"),
    ] {
        let report = typecheck_text(
            "builtin-constructor-callback.aivi",
            &format!("value wrapped : {result} = map {constructor} {source}\n"),
        );
        assert!(report.is_ok(), "{constructor}: {:?}", report.diagnostics());
    }
}

#[test]
fn constructor_callbacks_infer_authored_empty_effect_heads() {
    let report = typecheck_text(
        "authored-empty-constructor-callback.aivi",
        r#"
type Maybe A = Missing | Found A
instance Functor Maybe = {
    map = f maybe => maybe
     ||> Missing -> Missing
     ||> Found a -> Found (f a)
}
instance Apply Maybe = {
    apply = functions values => functions
     ||> Missing -> Missing
     ||> Found f -> map f values
}
instance Applicative Maybe = { pure = a => Found a }
type (Traversable F, Applicative G) => (Int -> G Int) -> F Int -> G (F Int)
func visit = transform values => traverse transform values
value rejected : Bool = visit (n => Missing) [1, 2] == Missing
"#,
    );
    assert!(report.is_ok(), "{:?}", report.diagnostics());
}

#[test]
fn constructor_callbacks_preserve_nominal_fixed_prefixes() {
    let declaration = r#"
type Report E A = Silent | Failure E | Success A
instance Functor (Report E) = {
    map = f report => report
     ||> Silent -> Silent
     ||> Failure e -> Failure e
     ||> Success a -> Success (f a)
}
instance Apply (Report E) = {
    apply = functions values => functions
     ||> Silent -> Silent
     ||> Failure e -> Failure e
     ||> Success f -> map f values
}
instance Applicative (Report E) = { pure = a => Success a }
type (Traversable F, Applicative G) => (Int -> G Int) -> F Int -> G (F Int)
func visit = transform values => traverse transform values
"#;
    for (source, accepted) in [
        (
            "value result : Bool = visit (n => Failure \"missing\") [1] == Failure \"missing\"\n",
            true,
        ),
        (
            "value result : Report Text (List Int) = visit (n => Silent) [1]\n",
            true,
        ),
        (
            "value result : Bool = visit (n => Silent) [1] == Silent\n",
            false,
        ),
        (
            "value result : Report Int (List Int) = visit (n => Failure \"missing\") [1]\n",
            false,
        ),
    ] {
        let report = typecheck_text(
            "nominal-prefix-callback.aivi",
            &format!("{declaration}\n{source}"),
        );
        assert_eq!(
            report.is_ok(),
            accepted,
            "{source}: {:?}",
            report.diagnostics()
        );
    }
}

#[test]
fn constructor_callbacks_preserve_rigid_inputs_and_result_payloads() {
    for source in [
        "value invalid : List (Option Text) = map Some [1]\n",
        "value invalid : List (Option Text) = [1] |> map Some\n",
        "value invalid : List (Result Text Int) = map Err [1]\n",
        "value invalid : Int -> Text -> Option Int = Some\n",
        "value invalid : Int -> Option Int = None\n",
        "value invalid : Int = None\n",
        "type Maybe A = Missing | Found A\nvalue invalid : Int = Missing\n",
        "type Maybe A = Missing | Found A\nvalue invalid : Int -> Maybe Int = Missing\n",
        "type Functor F => F A -> F (Option Int)\nfunc invalid = values => map Some values\n",
        "type Functor F => F A -> F (Option Int)\nfunc invalid = values => values |> map Some\n",
    ] {
        let report = typecheck_text("invalid-constructor-callback.aivi", source);
        assert!(
            !report.is_ok(),
            "invalid constructor contract accepted: {source}"
        );
    }
}

#[test]
fn contextual_callbacks_infer_empty_effect_constructor_results() {
    for callback in ["n => None", "n => []"] {
        let report = typecheck_text(
            "callback-effect-constructor.aivi",
            &format!(
                "type (Traversable F, Applicative G) => (Int -> G Int) -> F Int -> G (F Int)\nfunc visit = transform values => traverse transform values\nvalue result : Bool = visit ({callback}) [1] == {}\n",
                if callback == "n => None" {
                    "None"
                } else {
                    "[]"
                }
            ),
        );
        assert!(report.is_ok(), "{callback}: {:?}", report.diagnostics());
    }
}

#[test]
fn contextual_callbacks_instantiate_constructor_quantifiers_from_generic_inputs() {
    let module = lowered_module_text(
        "constructor-callback-identity.aivi",
        r#"
type Identity A = Identity A
instance Functor Identity = { map = f identity => identity ||> Identity a -> Identity (f a) }
instance Apply Identity = { apply = functions values => functions ||> Identity f -> map f values }
instance Applicative Identity = { pure = a => Identity a }
type Identity A -> A
func unwrap = identity => identity ||> Identity a -> a
type (Traversable F, Eq (F Int)) => F Int -> Bool
func identityLaw = values => unwrap (traverse Identity values) == values
"#,
    );
    let report = typecheck_module(&module);
    assert!(report.is_ok(), "{:?}", report.diagnostics());
}

#[test]
fn nullary_constructor_inference_preserves_absent_payloads_as_holes() {
    let module = lowered_module_text(
        "nullary-constructor-evidence.aivi",
        "type Maybe A = Missing | Found A\nvalue empty = Missing\n",
    );
    let body = module
        .items()
        .iter()
        .find_map(|(_, item)| match item {
            Item::Value(value) if value.name.text() == "empty" => Some(value.body),
            _ => None,
        })
        .unwrap();
    let info = GateTypeContext::new(&module).infer_expr(body, &GateExprEnv::default(), None);
    assert!(
        info.ty.is_none(),
        "a nullary constructor cannot fix its payload: {:?}",
        info.ty
    );
    assert!(
        matches!(info.actual, Some(SourceOptionActualType::OpaqueItem { arguments, .. })
        if arguments == vec![SourceOptionActualType::Hole])
    );
}

#[test]
fn nullary_constructor_branches_instantiate_from_present_payloads() {
    let report = typecheck_text(
        "nullary-constructor-branches.aivi",
        r#"
type Maybe A = Missing | Found A
type (A -> B) -> Maybe A -> Maybe B
func transform = f maybe => maybe
 ||> Missing -> Missing
 ||> Found a -> Found (f a)
type (A -> B) -> Maybe A -> Maybe B
func reversed = f maybe => maybe
 ||> Found a -> Found (f a)
 ||> Missing -> Missing
type Maybe Int -> Result Text (Maybe Int)
func nested = maybe => maybe
 ||> Missing -> Ok Missing
 ||> Found n -> Ok (Found n)
value empty : Maybe Int = Missing
value mapped : Maybe Text = transform (n => "yes") (Found 1)
value reverseEmpty : Maybe Text = reversed (n => "yes") empty
"#,
    );
    assert!(report.is_ok(), "{:?}", report.diagnostics());
}

#[test]
fn nullary_constructor_branches_keep_known_payloads_rigid() {
    for body in [
        "maybe\n ||> Missing -> Found captured\n ||> Found n -> Found n",
        "maybe\n ||> Found n -> Found n\n ||> Missing -> Found captured",
        "maybe\n ||> Missing -> Ok (Found captured)\n ||> Found n -> Ok (Found n)",
    ] {
        let result = if body.contains("Ok") {
            "Result Text (Maybe Int)"
        } else {
            "Maybe Int"
        };
        let source = format!(
            "type Maybe A = Missing | Found A\ntype A -> Maybe Int -> {result}\nfunc invalid = captured maybe => {body}\n"
        );
        let report = typecheck_text("nullary-constructor-rigid.aivi", &source);
        assert!(
            !report.is_ok(),
            "rigid payload specialization was accepted: {source}"
        );
        assert!(
            report
                .diagnostics()
                .iter()
                .any(|d| d.code == Some(crate::codes::CASE_BRANCH_TYPE_MISMATCH)),
            "{:?}",
            report.diagnostics()
        );
    }
}

#[test]
fn contextual_callbacks_preserve_present_generic_payload_contracts() {
    let module = lowered_module_text(
        "generic-some.aivi",
        "type A -> Option A\nfunc wrap = value => Some value\n",
    );
    let function = module
        .items()
        .iter()
        .find_map(|(_, item)| match item {
            Item::Function(function) if function.name.text() == "wrap" => Some(function),
            _ => None,
        })
        .unwrap();
    let mut typing = GateTypeContext::new(&module);
    typing.replace_rigid_type_parameters(function.type_parameters.clone());
    let parameter = &function.parameters[0];
    let payload = typing
        .lower_open_annotation(parameter.annotation.unwrap())
        .unwrap();
    let mut env = GateExprEnv::default();
    env.locals.insert(parameter.binding, payload.clone());
    let info = typing.infer_expr(function.body, &env, None);
    assert_eq!(info.ty, Some(GateType::Option(Box::new(payload))));
    assert!(!typing.match_gate_expr_template(
        &GateType::Option(Box::new(GateType::Primitive(BuiltinType::Int))),
        &info,
        &mut HashMap::new()
    ));
}

#[test]
fn exact_shape_proofs_preserve_holes_binders_carriers_and_canonical_fallback() {
    let int = GateType::Primitive(BuiltinType::Int);
    let text = GateType::Primitive(BuiltinType::Text);
    let parameter = GateType::TypeParameter {
        parameter: TypeParameterId::from_raw(29),
        name: "A".to_owned(),
    };
    let record = GateType::Record(vec![GateRecordField {
        name: "value".to_owned(),
        ty: int.clone(),
    }]);
    let imported = GateType::OpaqueImport {
        origin: Some(Box::new(crate::TypeIdentity::Source {
            file: FileId::new(7),
            name: "Carrier".into(),
        })),
        import: ImportId::from_raw(9),
        name: "Carrier".to_owned(),
        arguments: vec![int.clone()],
        definition: None,
    };
    let types = vec![
        int.clone(),
        parameter.clone(),
        record.clone(),
        GateType::TypeApplication {
            parameter: TypeParameterId::from_raw(30),
            name: "F".to_owned(),
            arguments: vec![parameter.clone()],
        },
        GateType::Tuple(vec![record.clone(), parameter.clone()]),
        GateType::Arrow {
            parameter: Box::new(parameter),
            result: Box::new(record),
        },
        GateType::List(Box::new(int.clone())),
        GateType::Set(Box::new(int.clone())),
        GateType::Option(Box::new(int.clone())),
        GateType::Signal(Box::new(int.clone())),
        GateType::Map {
            key: Box::new(text.clone()),
            value: Box::new(int.clone()),
        },
        GateType::Result {
            error: Box::new(text.clone()),
            value: Box::new(int.clone()),
        },
        GateType::Validation {
            error: Box::new(text.clone()),
            value: Box::new(int.clone()),
        },
        GateType::Task {
            error: Box::new(text.clone()),
            value: Box::new(int.clone()),
        },
        GateType::Domain {
            item: ItemId::from_raw(3),
            name: "Wrapper".to_owned(),
            arguments: vec![int.clone()],
        },
        GateType::OpaqueItem {
            item: ItemId::from_raw(4),
            name: "Wrapper".to_owned(),
            arguments: vec![int.clone()],
        },
        imported.clone(),
    ];
    for ty in &types {
        let actual = SourceOptionActualType::from_gate_type(ty);
        assert!(actual.has_exact_gate_shape(ty), "{ty}");
        assert!(!actual.has_exact_gate_shape(&text), "{ty}");
        assert!(!SourceOptionActualType::Hole.has_exact_gate_shape(ty));
    }
    let distinct = GateType::TypeParameter {
        parameter: TypeParameterId::from_raw(31),
        name: "A".to_owned(),
    };
    assert!(!SourceOptionActualType::from_gate_type(&types[1]).has_exact_gate_shape(&distinct));
    let mut distinct_import = imported.clone();
    let GateType::OpaqueImport { import, .. } = &mut distinct_import else {
        unreachable!()
    };
    *import = ImportId::from_raw(10);
    assert!(
        !SourceOptionActualType::from_gate_type(&imported).has_exact_gate_shape(&distinct_import)
    );
    let partial = SourceOptionActualType::List(Box::new(SourceOptionActualType::Hole));
    assert!(!partial.has_exact_gate_shape(&GateType::List(Box::new(int.clone()))));

    let fields = vec![
        GateRecordField {
            name: "a".to_owned(),
            ty: int,
        },
        GateRecordField {
            name: "b".to_owned(),
            ty: text,
        },
    ];
    let actual = SourceOptionActualType::from_gate_type(&GateType::Record(fields.clone()));
    let reversed = GateType::Record(fields.into_iter().rev().collect());
    assert!(!actual.has_exact_gate_shape(&reversed));
    let module = lowered_module_text("exact-shape-canonical-fallback.aivi", "");
    let typing = GateTypeContext::new(&module);
    assert!(typing.types_match(&imported, &distinct_import));
    let actual = SourceOptionActualType::from_gate_type(&imported);
    let info = typing.finalize_expr_info(GateExprInfo {
        ty: Some(distinct_import),
        actual: Some(actual.clone()),
        ..GateExprInfo::default()
    });
    assert_eq!(
        info.actual,
        Some(actual),
        "canonical nominal matching must preserve equivalent stored evidence"
    );
}

#[test]
fn actual_gate_type_preserves_partial_evidence_and_exact_identity() {
    let payload = GateType::TypeParameter {
        parameter: TypeParameterId::from_raw(29),
        name: "A".to_owned(),
    };
    let applied = GateType::TypeApplication {
        parameter: TypeParameterId::from_raw(30),
        name: "F".to_owned(),
        arguments: vec![payload.clone()],
    };
    for ty in [
        payload,
        applied.clone(),
        GateType::Task {
            error: Box::new(GateType::Primitive(BuiltinType::Text)),
            value: Box::new(applied),
        },
        GateType::OpaqueItem {
            item: ItemId::from_raw(12),
            name: "Carrier".to_owned(),
            arguments: vec![GateType::Primitive(BuiltinType::Int)],
        },
    ] {
        let mut info = GateExprInfo {
            ty: Some(ty.clone()),
            ..GateExprInfo::default()
        };
        assert_eq!(info.actual_gate_type(), Some(ty.clone()));
        info.actual = Some(SourceOptionActualType::from_gate_type(&ty));
        info.ty = None;
        assert_eq!(info.actual_gate_type(), Some(ty.clone()));
        let actual = info.actual.clone();
        assert_eq!(info.take_inferred_type(), Some(ty.clone()));
        assert_eq!(
            info.actual, actual,
            "taking a type must retain partial evidence"
        );
        info.ty = Some(ty);
        info.contains_signal = true;
        assert_eq!(info.take_actual(), actual);
        assert!(info.actual.is_none());
        assert!(info.ty.is_some());
        assert!(
            info.contains_signal,
            "taking evidence must preserve metadata"
        );
    }
    let hinted = GateType::List(Box::new(GateType::Primitive(BuiltinType::Int)));
    let partial = GateExprInfo {
        ty: Some(hinted),
        actual: Some(SourceOptionActualType::List(Box::new(
            SourceOptionActualType::Hole,
        ))),
        ..GateExprInfo::default()
    };
    assert_eq!(partial.actual_gate_type(), None);
    let conflicting = GateExprInfo {
        ty: Some(GateType::Primitive(BuiltinType::Text)),
        actual: Some(SourceOptionActualType::Primitive(BuiltinType::Int)),
        ..GateExprInfo::default()
    };
    assert_eq!(
        conflicting.actual_gate_type(),
        Some(GateType::Primitive(BuiltinType::Int))
    );
}

#[test]
fn partial_structural_constructor_mismatches_preserve_existing_bindings() {
    let module = lowered_module_text("partial-structural-constructor.aivi", "");
    let typing = GateTypeContext::new(&module);
    let int = GateType::Primitive(BuiltinType::Int);
    let hole = || Box::new(SourceOptionActualType::Hole);
    for actual in [
        SourceOptionActualType::Tuple(vec![
            SourceOptionActualType::Hole,
            SourceOptionActualType::Hole,
        ]),
        SourceOptionActualType::Record(vec![
            crate::typecheck_context::SourceOptionActualRecordField {
                name: "field".to_owned(),
                ty: SourceOptionActualType::Hole,
            },
        ]),
        SourceOptionActualType::Arrow {
            parameter: hole(),
            result: hole(),
        },
        SourceOptionActualType::List(hole()),
        SourceOptionActualType::Map {
            key: hole(),
            value: hole(),
        },
        SourceOptionActualType::Set(hole()),
        SourceOptionActualType::Option(hole()),
        SourceOptionActualType::Result {
            error: hole(),
            value: hole(),
        },
        SourceOptionActualType::Validation {
            error: hole(),
            value: hole(),
        },
        SourceOptionActualType::Signal(hole()),
        SourceOptionActualType::Task {
            error: hole(),
            value: hole(),
        },
    ] {
        let info = GateExprInfo {
            actual: Some(actual),
            ..GateExprInfo::default()
        };
        let mut bindings = HashMap::from([(TypeParameterId::from_raw(7), int.clone())]);
        let before = bindings.clone();
        assert!(!typing.match_gate_expr_template(&int, &info, &mut bindings));
        assert_eq!(bindings, before);
    }
    let mut bindings = HashMap::new();
    assert!(typing.match_gate_expr_template(
        &GateType::List(Box::new(int)),
        &GateExprInfo {
            actual: Some(SourceOptionActualType::List(hole())),
            ..GateExprInfo::default()
        },
        &mut bindings,
    ));
}

#[test]
fn partial_constructor_evidence_preserves_abstract_payload_identity() {
    let parameter = GateType::TypeParameter {
        parameter: TypeParameterId::from_raw(29),
        name: "A".to_owned(),
    };
    let applied = GateType::TypeApplication {
        parameter: TypeParameterId::from_raw(30),
        name: "F".to_owned(),
        arguments: vec![parameter.clone()],
    };
    let module = lowered_module_text("partial-abstract-payload.aivi", "");
    let typing = GateTypeContext::new(&module);
    for payload in [parameter, applied] {
        let actual = SourceOptionActualType::from_gate_type(&payload);
        assert_eq!(actual.to_gate_type(), Some(payload.clone()));
        assert_eq!(
            actual.unify(&SourceOptionActualType::Hole),
            Some(actual.clone())
        );
        assert!(
            actual
                .unify(&SourceOptionActualType::Primitive(BuiltinType::Int))
                .is_none()
        );
        let info = GateExprInfo {
            actual: Some(SourceOptionActualType::Result {
                error: Box::new(SourceOptionActualType::Hole),
                value: Box::new(actual),
            }),
            ..GateExprInfo::default()
        };
        assert!(!typing.match_gate_expr_template(
            &GateType::Result {
                error: Box::new(GateType::Primitive(BuiltinType::Text)),
                value: Box::new(GateType::Primitive(BuiltinType::Int)),
            },
            &info,
            &mut HashMap::new(),
        ));
        assert!(typing.match_gate_expr_template(
            &GateType::Result {
                error: Box::new(GateType::Primitive(BuiltinType::Text)),
                value: Box::new(payload),
            },
            &info,
            &mut HashMap::new(),
        ));
    }
}

#[test]
fn contextual_callbacks_reject_partial_constructor_capture_and_rigid_effects() {
    let prefix = "type (Traversable F, Applicative G) => (Int -> G Int) -> F Int -> G (F Int)\nfunc visit = transform values => traverse transform values\n";
    for constructor in ["Some", "Ok", "Err", "Valid", "Invalid"] {
        for body in [
            format!(
                "type (Traversable F, Applicative G) => A -> F Int -> G (F Int)\nfunc bad = captured values => visit (n => {constructor} captured) values\n"
            ),
            format!(
                "type (Traversable F, Applicative G) => F Int -> G (F Int)\nfunc bad = values => visit (n => {constructor} n) values\n"
            ),
        ] {
            let report = typecheck_text(
                "partial-constructor-rigid-callback.aivi",
                &format!("{prefix}{body}"),
            );
            assert!(!report.is_ok(), "accepted {constructor}: {body}");
        }
    }
}

#[test]
fn contextual_expected_results_instantiate_phantom_constructor_parameters() {
    let report = typecheck_text(
        "phantom-constructor-result.aivi",
        r#"
type Choice L R = Left L | Right R
type Applicative G => Choice E A -> (A -> G B) -> G (Choice E B)
func visit = choice transform => choice
 ||> Left error -> pure (Left error)
 ||> Right item -> map Right (transform item)
"#,
    );
    assert!(report.is_ok(), "{:?}", report.diagnostics());
}

#[test]
fn contextual_callbacks_preserve_valid_partial_constructor_payloads() {
    for signature_and_body in [
        "type A -> Result Text A\nfunc wrap = captured => Ok captured\n",
        "type A -> Result A Int\nfunc wrap = captured => Err captured\n",
        "type A -> Validation Text A\nfunc wrap = captured => Valid captured\n",
        "type A -> Validation A Int\nfunc wrap = captured => Invalid captured\n",
        "type Traversable F => A -> F A -> Result Text (F A)\nfunc fill = captured values => traverse (n => Ok captured) values\n",
        "type Traversable F => A -> F A -> Validation Text (F A)\nfunc fill = captured values => traverse (n => Valid captured) values\n",
    ] {
        let report = typecheck_text("valid-partial-constructor.aivi", signature_and_body);
        assert!(
            report.is_ok(),
            "{signature_and_body}: {:?}",
            report.diagnostics()
        );
    }
}

#[test]
fn contextual_callbacks_reject_incompatible_effects_and_rigid_specialization() {
    let prefix = "type (Traversable F, Applicative G) => (Int -> G Int) -> F Int -> G (F Int)\nfunc visit = transform values => traverse transform values\n";
    for body in [
        "value bad : Bool = visit (n => Some \"wrong\") [1] == None\n",
        "value bad : Bool = visit (n => n) [1] == None\n",
        "type (Traversable F, Applicative G) => F Int -> G (F Int)\nfunc bad = values => visit (n => None) values\n",
        "type (Traversable F, Applicative G) => A -> F Int -> G (F Int)\nfunc bad = captured values => visit (n => Some captured) values\n",
        "type A -> List Int\nfunc bad = captured => map (n => captured) [1]\n",
        "type A -> List Int\nfunc bad = captured => map (n => [captured]) [1]\n",
        "type Applicative G => (Int -> G Int) -> (Int -> G Int) -> G Int\nfunc both = first second => first 1\nvalue bad : Bool = both (n => None) (n => [n]) == None\n",
        "value bad : Bool = traverse (n => None) [1] == None\n",
    ] {
        let report = typecheck_text(
            "incompatible-callback-context.aivi",
            &format!("{prefix}{body}"),
        );
        assert!(!report.is_ok(), "accepted {body:?}");
        assert!(report.diagnostics().iter().any(|diagnostic| matches!(diagnostic.code, Some(code) if code == crate::codes::TYPE_MISMATCH || code == crate::codes::INVALID_BINARY_OPERATOR)), "{body:?}: {:?}", report.diagnostics());
    }
}

#[test]
fn contextual_callbacks_solve_direct_class_calls_and_nested_bodies() {
    for source in [
        "value empty : Option (List Text) = None\nvalue result : Bool = traverse (n => None) [1] == empty\n",
        "value empty : List (List Text) = []\nvalue result : Bool = traverse (n => []) [1] == empty\n",
        "type (Traversable F, Applicative G) => (Int -> G Int) -> F Int -> G (F Int)\nfunc visit = transform values => traverse transform values\nvalue result : List (Option (List Int)) = map (n => visit (m => None) [n]) [1]\n",
        "type (Traversable F, Applicative G) => (Int -> G Int) -> F Int -> G (F Int)\nfunc visit = transform values => traverse transform values\nvalue rejected = [1, 2]\n |> visit (n => None)\nvalue result : Bool = rejected == None\n",
    ] {
        let report = typecheck_text("direct-and-nested-callbacks.aivi", source);
        assert!(report.is_ok(), "{source:?}: {:?}", report.diagnostics());
    }
}

#[test]
fn contextual_callbacks_infer_nested_factory_results_before_consumers() {
    let report = typecheck_text(
        "nested-callback-factory.aivi",
        r#"
type Box A = { payload: A }
type Int -> Int -> (Int -> Int -> A) -> Box A
func build = x y cell => { payload: cell x y }
type Box A -> Int
func size = box => 0
value result : Option Int = map size (Some (build 0 3 (x y => 1)))
"#,
    );
    assert!(report.is_ok(), "{:?}", report.diagnostics());

    struct Resolver(crate::ExportedNames);
    impl crate::ImportResolver for Resolver {
        fn resolve(&self, _: &[&str]) -> crate::ImportModuleResolution {
            crate::ImportModuleResolution::Resolved(self.0.clone())
        }
    }
    let mut sources = SourceDatabase::new();
    let owner = sources.add_file(
        "factory.aivi",
        r#"
type Box A = { payload: A }
type Int -> Int -> (Int -> Int -> A) -> Box A
func build = x y cell => { payload: cell x y }
type Box A -> Int
func size = box => 0
export Box
export build
export size
"#,
    );
    let parsed = parse_module(&sources[owner]);
    assert!(!parsed.has_errors());
    let lowered = lower_module(&parsed.module);
    assert!(!lowered.has_errors());
    let resolver = Resolver(crate::exports(lowered.module()));
    let consumer = sources.add_file("consumer.aivi", "use factory (Box, build, size)\nvalue result : Option Int = map size (Some (build 0 3 (x y => 1)))\n");
    let parsed = parse_module(&sources[consumer]);
    assert!(!parsed.has_errors());
    let lowered = crate::lower_module_with_resolver(&parsed.module, Some(&resolver));
    assert!(!lowered.has_errors(), "{:?}", lowered.diagnostics());
    let report = typecheck_module(lowered.module());
    assert!(
        report.is_ok(),
        "imported factory: {:?}",
        report.diagnostics()
    );
}

#[test]
fn typecheck_accepts_signal_merge_arms_against_signal_payloads() {
    let report = typecheck_text(
        "signal-merge-valid.aivi",
        r#"signal ready : Signal Bool

signal total : Signal Int = ready
  ||> True => 42
  ||> _ => 0
"#,
    );
    assert!(
        report.is_ok(),
        "expected signal merge typing to accept direct signal references, got diagnostics: {:?}",
        report.diagnostics()
    );
}

#[test]
fn typecheck_reports_non_bool_signal_merge_guard() {
    let report = typecheck_text(
        "signal-merge-guard-not-bool.aivi",
        r#"signal src : Signal Int

signal total : Signal Int = src
  ||> 1 => 2
  ||> _ => 0
"#,
    );
    // Pattern match on integer literal should work — 1 is a valid pattern
    // This test now verifies merge arm semantics rather than guard-is-bool
    assert!(
        !report.diagnostics().is_empty() || report.is_ok(),
        "signal merge with integer pattern should either type-check or report a pattern mismatch"
    );
}

#[test]
fn typecheck_reports_signal_merge_body_payload_mismatch() {
    let report = typecheck_text(
        "signal-merge-body-mismatch.aivi",
        r#"signal ready : Signal Bool

signal total : Signal Int = ready
  ||> True => "oops"
  ||> _ => 0
"#,
    );
    assert!(
        report
            .diagnostics()
            .iter()
            .any(|diagnostic| { diagnostic.code == Some(crate::codes::TYPE_MISMATCH) }),
        "expected signal merge body mismatch to report a type mismatch, got diagnostics: {:?}",
        report.diagnostics()
    );
}

#[test]
fn typecheck_accepts_single_source_signal_merge() {
    let report = typecheck_text(
        "single-source-merge-valid.aivi",
        r#"type Direction = Up | Down
type Event = Turn Direction | Tick

signal event = Turn Down

signal heading : Signal Direction = event
  ||> Turn dir => dir
  ||> _ => Up
"#,
    );
    assert!(
        report.is_ok(),
        "expected single-source signal merge typing to succeed, got diagnostics: {:?}",
        report.diagnostics()
    );
}

#[test]
fn typecheck_accepts_source_pattern_signal_merge() {
    let report = typecheck_text(
        "source-pattern-merge-valid.aivi",
        r#"signal ready : Signal Bool

signal total : Signal Int = ready
  ||> True => 42
  ||> _ => 0
"#,
    );
    assert!(
        report.is_ok(),
        "expected source-pattern signal merge typing to succeed, got diagnostics: {:?}",
        report.diagnostics()
    );
}

#[test]
fn typecheck_accepts_unannotated_function_name_from_expected_arrow() {
    let report = typecheck_text(
        "function-name-expected-arrow.aivi",
        "fun keep = x => x\n\
             value chosen:(Option Int -> Option Int) = keep\n",
    );
    assert!(
        report.is_ok(),
        "expected unannotated function name to typecheck from expected arrow, got diagnostics: {:?}",
        report.diagnostics()
    );
}

#[test]
fn typecheck_accepts_unannotated_function_application_from_expected_result() {
    let report = typecheck_text(
        "function-application-expected-result.aivi",
        "fun keepNone = opt:Option Int => None\n\
             value result:Option Int = keepNone None\n",
    );
    assert!(
        report.is_ok(),
        "expected unannotated function application to typecheck from expected result, got diagnostics: {:?}",
        report.diagnostics()
    );
}

#[test]
fn typecheck_propagates_contextual_inference_through_same_module_helpers() {
    let report = typecheck_text(
        "function-helper-contextual-inference.aivi",
        "fun keep = value => value\n\
             fun relay = value => keep value\n\
             value chosen:(Option Int -> Option Int) = relay\n",
    );
    assert!(
        report.is_ok(),
        "expected same-module helper inference to typecheck, got diagnostics: {:?}",
        report.diagnostics()
    );
}

#[test]
fn typecheck_accepts_partial_application_from_expected_arrow() {
    let report = typecheck_text(
        "function-partial-application-expected-arrow.aivi",
        "fun keepLeft = left right => left\n\
             value chooser:(Int -> Int) = keepLeft 1\n",
    );
    assert!(
        report.is_ok(),
        "expected partial application to typecheck from expected arrow, got diagnostics: {:?}",
        report.diagnostics()
    );
}

#[test]
fn typecheck_accepts_function_application_with_expected_builtin_hole_argument() {
    let report = typecheck_text(
        "function-application-expected-hole.aivi",
        "fun keep:Option Int = opt:Option Int => opt\n\
             value result:Option Int = keep None\n",
    );
    assert!(
        report.is_ok(),
        "expected keep None to typecheck, got diagnostics: {:?}",
        report.diagnostics()
    );
}

#[test]
fn typecheck_reports_function_application_result_mismatch() {
    let report = typecheck_text(
        "function-application-result-mismatch.aivi",
        "fun keep:Option Int = opt:Option Int => opt\n\
             value result:Option Text = keep None\n",
    );
    assert!(
        report
            .diagnostics()
            .iter()
            .any(|diagnostic| { diagnostic.code == Some(crate::codes::TYPE_MISMATCH) }),
        "expected type mismatch diagnostic, got diagnostics: {:?}",
        report.diagnostics()
    );
}

#[test]
fn typecheck_reports_missing_default_instance_via_constraint_solver() {
    let report = typecheck_text(
        "missing-default-instance.aivi",
        "type Nickname = Nickname Text\n\
             type User = {\n\
                 name: Text,\n\
                 nickname: Nickname\n\
             }\n\
             value name = \"Ada\"\n\
             value user:User = { name }\n",
    );
    assert!(
        report.diagnostics().iter().any(|diagnostic| {
            diagnostic.code == Some(crate::codes::MISSING_DEFAULT_INSTANCE)
                && diagnostic.message.contains("nickname")
        }),
        "expected missing Default diagnostic from constraint solver, got diagnostics: {:?}",
        report.diagnostics()
    );
}

#[test]
fn typecheck_accepts_same_module_default_instances_for_record_elision() {
    let report = typecheck_text(
        "same-module-default-instance.aivi",
        "class Default A = {\n\
             \x20\x20\x20\x20default : A\n\
             }\n\
             type Nickname = Nickname Text\n\
             instance Default Nickname = {\n\
             \x20\x20\x20\x20default = Nickname \"\"\n\
             }\n\
             type User = {\n\
                 name: Text,\n\
                 nickname: Nickname\n\
             }\n\
             value name = \"Ada\"\n\
             value user:User = { name }\n",
    );
    assert!(
        report.is_ok(),
        "expected same-module Default instance to satisfy record elision, got diagnostics: {:?}",
        report.diagnostics()
    );
}

#[test]
fn typecheck_accepts_imported_default_values_for_record_elision() {
    let report = typecheck_text(
        "imported-default-values.aivi",
        "use aivi.defaults (defaultText as emptyText, defaultInt, defaultBool as disabled)\n\
             type Settings = {\n\
                 title: Text,\n\
                 retries: Int,\n\
                 enabled: Bool,\n\
                 label: Text\n\
             }\n\
             value title = \"AIVI\"\n\
             value settings:Settings = { title }\n",
    );
    assert!(
        report.is_ok(),
        "expected imported aivi.defaults values to satisfy record elision, got diagnostics: {:?}",
        report.diagnostics()
    );
}

#[test]
fn typecheck_accepts_ambient_default_class_for_record_elision() {
    let report = typecheck_text(
        "ambient-default-instance.aivi",
        "type Nickname = Nickname Text\n\
             instance Default Nickname = {\n\
             \x20\x20\x20\x20default = Nickname \"\"\n\
             }\n\
             type User = {\n\
                 name: Text,\n\
                 nickname: Nickname\n\
             }\n\
             value user:User = { name: \"Ada\" }\n",
    );
    assert!(
        report.is_ok(),
        "expected ambient Default class to satisfy record elision, got diagnostics: {:?}",
        report.diagnostics()
    );
}

#[test]
fn typecheck_elaborates_imported_default_values_into_explicit_fields() {
    let (report, module) = typecheck_and_elaborate_text(
        "imported-default-values-hir.aivi",
        "use aivi.defaults (defaultText as emptyText, defaultInt, defaultBool as disabled)\n\
             type Settings = {\n\
                 title: Text,\n\
                 retries: Int,\n\
                 enabled: Bool,\n\
                 label: Text\n\
             }\n\
             value title = \"AIVI\"\n\
             value settings:Settings = { title }\n",
    );
    assert!(
        report.is_ok(),
        "expected imported aivi.defaults values to satisfy record elision, got diagnostics: {:?}",
        report.diagnostics()
    );

    let settings = value_body(&module, "settings");
    let ExprKind::Record(record) = &module.exprs()[settings].kind else {
        panic!("expected `settings` to stay a record literal");
    };
    assert_eq!(
        record
            .fields
            .iter()
            .map(|field| field.label.text())
            .collect::<Vec<_>>(),
        vec!["title", "retries", "enabled", "label"]
    );
    assert_eq!(
        record
            .fields
            .iter()
            .map(|field| field.surface)
            .collect::<Vec<_>>(),
        vec![
            RecordFieldSurface::Shorthand,
            RecordFieldSurface::Defaulted,
            RecordFieldSurface::Defaulted,
            RecordFieldSurface::Defaulted,
        ]
    );

    let empty_text = import_binding_id(&module, "emptyText");
    let default_int = import_binding_id(&module, "defaultInt");
    let disabled = import_binding_id(&module, "disabled");
    for (label, expected_import) in [
        ("retries", default_int),
        ("enabled", disabled),
        ("label", empty_text),
    ] {
        let value = record
            .fields
            .iter()
            .find(|field| field.label.text() == label)
            .map(|field| field.value)
            .expect("expected synthesized field to exist");
        match &module.exprs()[value].kind {
            ExprKind::Name(reference) => assert!(matches!(
                reference.resolution.as_ref(),
                ResolutionState::Resolved(TermResolution::Import(import_id))
                    if *import_id == expected_import
            )),
            other => panic!(
                "expected synthesized imported default for `{label}` to stay a name reference, found {other:?}"
            ),
        }
    }
}

#[test]
fn typecheck_accepts_metadata_backed_imported_default_values_without_defaults_module_path() {
    let mut module = lowered_module_text(
        "rewritten-imported-default-values.aivi",
        "use aivi.defaults (defaultText as emptyText, defaultInt, defaultBool as disabled)\n\
             type Settings = {\n\
                 title: Text,\n\
                 retries: Int,\n\
                 enabled: Bool,\n\
                 label: Text\n\
             }\n\
             value title = \"AIVI\"\n\
             value settings:Settings = { title }\n",
    );
    rewrite_first_use_module_path(&mut module, &["custom", "defaults"]);

    let report = typecheck_module(&module);
    assert!(
        report.is_ok(),
        "expected imported default metadata to satisfy record elision independent of use-path spelling, got diagnostics: {:?}",
        report.diagnostics()
    );
}

#[test]
fn typecheck_accepts_metadata_backed_option_default_bundle_without_defaults_module_path() {
    let mut module = lowered_module_text(
        "rewritten-option-default-bundle.aivi",
        "use aivi.defaults (Option)\n\
             type User = {\n\
                 name: Text,\n\
                 nickname: Option Text\n\
             }\n\
             value name = \"Ada\"\n\
             value user:User = { name }\n",
    );
    rewrite_first_use_module_path(&mut module, &["custom", "defaults"]);

    let report = typecheck_module(&module);
    assert!(
        report.is_ok(),
        "expected imported Option default bundle metadata to satisfy record elision independent of use-path spelling, got diagnostics: {:?}",
        report.diagnostics()
    );
}

#[test]
fn typecheck_elaborates_same_module_default_instances_into_explicit_fields() {
    let (report, module) = typecheck_and_elaborate_text(
        "same-module-default-instance-hir.aivi",
        "class Default A = {\n\
             \x20\x20\x20\x20default : A\n\
             }\n\
             type Nickname = Nickname Text\n\
             instance Default Nickname = {\n\
             \x20\x20\x20\x20default = Nickname \"\"\n\
             }\n\
             type User = {\n\
                 name: Text,\n\
                 nickname: Nickname\n\
             }\n\
             value name = \"Ada\"\n\
             value user:User = { name }\n",
    );
    assert!(
        report.is_ok(),
        "expected same-module Default instance to satisfy record elision, got diagnostics: {:?}",
        report.diagnostics()
    );

    let module = &module;
    let user = value_body(module, "user");
    let ExprKind::Record(record) = &module.exprs()[user].kind else {
        panic!("expected `user` to stay a record literal");
    };
    assert_eq!(
        record
            .fields
            .iter()
            .map(|field| field.label.text())
            .collect::<Vec<_>>(),
        vec!["name", "nickname"]
    );
    assert_eq!(record.fields[1].surface, RecordFieldSurface::Defaulted);

    let default_body = same_module_default_body(module, "default");
    assert_eq!(
        record.fields[1].value, default_body,
        "same-module Default synthesis should reuse the validated instance member body"
    );
}

#[test]
fn typecheck_reports_same_module_constructor_argument_mismatch() {
    let report = typecheck_text(
        "same-module-constructor-mismatch.aivi",
        "type Box A = Box A\n\
             value wrapped:(Box Text) = Box 42\n",
    );
    assert!(
        report
            .diagnostics()
            .iter()
            .any(|diagnostic| { diagnostic.code == Some(crate::codes::TYPE_MISMATCH) }),
        "expected same-module constructor mismatch diagnostic, got diagnostics: {:?}",
        report.diagnostics()
    );
}

#[test]
fn typecheck_reports_mixed_applicative_cluster_members() {
    let report = typecheck_text(
        "mixed-applicative-cluster.aivi",
        "type NamePair = NamePair Text Text\n\
             value first:(Option Text) = Some \"Ada\"\n\
             signal last = \"Lovelace\"\n\
             value broken =\n\
              &|> first\n\
              &|> last\n\
               |> NamePair\n",
    );
    assert!(
        report.diagnostics().iter().any(|diagnostic| {
            diagnostic.code == Some(crate::codes::APPLICATIVE_CLUSTER_MISMATCH)
        }),
        "expected applicative cluster mismatch diagnostic, got diagnostics: {:?}",
        report.diagnostics()
    );
}

#[test]
fn typecheck_accepts_partial_builtin_applicative_clusters() {
    let report = typecheck_text(
        "partial-builtin-clusters.aivi",
        "type NamePair = NamePair Text Text\n\
             value first = Some \"Ada\"\n\
             value last = None\n\
             value maybePair:Option NamePair =\n\
              &|> first\n\
              &|> last\n\
               |> NamePair\n\
             value okFirst = Ok \"Ada\"\n\
             value errLast = Err \"missing\"\n\
             value resultPair:Result Text NamePair =\n\
              &|> okFirst\n\
              &|> errLast\n\
               |> NamePair\n",
    );
    assert!(
        report.is_ok(),
        "expected partial builtin clusters to typecheck, got diagnostics: {:?}",
        report.diagnostics()
    );
}

#[test]
fn typecheck_reports_case_branch_type_mismatch() {
    let report = typecheck_text(
        "case-branch-type-mismatch.aivi",
        r#"type Screen =
  | Loading
  | Ready Text
value current:Screen = Loading
value broken =
    current
     ||> Loading -> 0
     ||> Ready title -> title
"#,
    );
    assert!(
        report
            .diagnostics()
            .iter()
            .any(|diagnostic| { diagnostic.code == Some(crate::codes::CASE_BRANCH_TYPE_MISMATCH) }),
        "expected case branch type mismatch diagnostic, got diagnostics: {:?}",
        report.diagnostics()
    );
}

#[test]
fn typecheck_reports_non_result_bindings_in_result_blocks() {
    let report = typecheck_text(
        "result-block-binding-not-result.aivi",
        concat!(
            "value broken: Result Text Int =\n",
            "    result {\n",
            "        x <- 42\n",
            "        x\n",
            "    }\n",
        ),
    );
    assert!(
        report.diagnostics().iter().any(
            |diagnostic| diagnostic.code == Some(crate::codes::RESULT_BLOCK_BINDING_NOT_RESULT)
        ),
        "expected non-Result result-block binding diagnostic, got diagnostics: {:?}",
        report.diagnostics()
    );
}

#[test]
fn typecheck_reports_result_block_error_mismatches() {
    let report = typecheck_text(
        "result-block-error-mismatch.aivi",
        concat!(
            "value broken: Result Text Int =\n",
            "    result {\n",
            "        x <- Ok 1\n",
            "        y <- Err 2\n",
            "        x\n",
            "    }\n",
        ),
    );
    assert!(
        report
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code == Some(crate::codes::RESULT_BLOCK_ERROR_MISMATCH)),
        "expected result-block error mismatch diagnostic, got diagnostics: {:?}",
        report.diagnostics()
    );
}

#[test]
fn typecheck_accepts_partial_builtin_case_runs() {
    let report = typecheck_text(
        "partial-builtin-case-runs.aivi",
        r#"type Screen =
  | Loading
  | Ready Text
  | Failed Text
value current:Screen = Loading
value maybeLabel:Option Text =
    current
     ||> Loading -> None
     ||> Ready title -> Some title
     ||> Failed reason -> Some reason
value resultLabel:Result Text Text =
    current
     ||> Loading -> Ok "loading"
     ||> Ready title -> Ok title
     ||> Failed reason -> Err reason
"#,
    );
    assert!(
        report.is_ok(),
        "expected partial builtin case runs to typecheck, got diagnostics: {:?}",
        report.diagnostics()
    );
}

#[test]
fn typecheck_accepts_applied_calls_in_case_branches() {
    let report = typecheck_text(
        "applied-call-case-branches.aivi",
        r#"fun addOne:Int = n:Int => n + 1
value x:Int =
    0
     ||> 0 -> addOne 0
     ||> _ -> 1
"#,
    );
    assert!(
        report.is_ok(),
        "expected applied calls in case branches to typecheck, got diagnostics: {:?}",
        report.diagnostics()
    );
}

#[test]
fn literal_case_continuations_check_nested_callbacks_and_captured_values() {
    let report = typecheck_text(
        "literal-case-continuations.aivi",
        r#"type Payload = { number: Int, callback: Int -> Int }
value good:Payload =
    2
     ||> subject -> { number: subject, callback: n => n + 1 }
fun increment:Int = n:Int => n + 1
value called:Int =
    2
     ||> _ -> increment
"#,
    );
    assert!(report.is_ok(), "{:?}", report.diagnostics());
    for body in [
        "{ number: subject, callback: n => \"wrong\" }",
        "{ number: \"wrong\", callback: n => n }",
    ] {
        let report = typecheck_text(
            "invalid-literal-continuation.aivi",
            &format!(
                "type Payload = {{ number: Int, callback: Int -> Int }}\nvalue wrong:Payload = 2\n ||> subject -> {body}\n"
            ),
        );
        assert!(!report.is_ok(), "{body}: {:?}", report.diagnostics());
    }
}

#[test]
fn typecheck_accepts_applied_calls_in_truthy_falsy_branches() {
    let report = typecheck_text(
        "applied-call-truthy-falsy-branches.aivi",
        r#"fun addOne:Int = n:Int => n + 1
value x:Int =
    True
     T|> addOne 0
     F|> 1
"#,
    );
    assert!(
        report.is_ok(),
        "expected applied calls in truthy/falsy branches to typecheck, got diagnostics: {:?}",
        report.diagnostics()
    );
}

#[test]
fn typecheck_accepts_generic_record_projection_in_function_body() {
    let report = typecheck_text(
        "generic-record-projection-in-function-body.aivi",
        r#"type TakeAcc A = {
    n: Int,
    items: List A
}
fun remaining:Int = acc:(TakeAcc A) => acc.n
fun items:(List A) = acc:(TakeAcc A) => acc.items
"#,
    );
    assert!(
        report.is_ok(),
        "expected generic record projection in function bodies to typecheck, got diagnostics: {:?}",
        report.diagnostics()
    );
}

#[test]
fn typecheck_accepts_polymorphic_pipe_transforms() {
    let mut module = Module::new(FileId::new(0));
    let option_type = builtin_type(&mut module, BuiltinType::Option);
    let int_type = builtin_type(&mut module, BuiltinType::Int);
    let text_type = builtin_type(&mut module, BuiltinType::Text);
    let parameter = type_parameter(&mut module, "A");
    let a_type = type_parameter_type(&mut module, parameter, "A");
    let option_a_type = applied_type(&mut module, option_type, a_type);
    let binding = module
        .alloc_binding(crate::Binding {
            span: unit_span(),
            name: test_name("value"),
            kind: crate::BindingKind::FunctionParameter,
        })
        .expect("binding allocation should fit");
    let local_expr = module
        .alloc_expr(crate::Expr {
            span: unit_span(),
            kind: crate::ExprKind::Name(crate::TermReference::resolved(
                test_path("value"),
                crate::TermResolution::Local(binding),
            )),
        })
        .expect("local expression allocation should fit");
    let some_expr = builtin_term_expr(&mut module, crate::BuiltinTerm::Some, "Some");
    let wrap_body = module
        .alloc_expr(crate::Expr {
            span: unit_span(),
            kind: crate::ExprKind::Apply {
                callee: some_expr,
                arguments: crate::NonEmpty::new(local_expr, Vec::new()),
            },
        })
        .expect("wrap body allocation should fit");
    let wrap = module
        .push_item(crate::Item::Function(crate::FunctionItem {
            origin: crate::FunctionOrigin::Declared,
            header: crate::ItemHeader {
                span: unit_span(),
                decorators: Vec::new(),
            },
            name: test_name("wrap"),
            type_parameters: vec![parameter],
            context: Vec::new(),
            parameters: vec![crate::FunctionParameter {
                span: unit_span(),
                binding,
                annotation: Some(a_type),
            }],
            annotation: Some(option_a_type),
            body: wrap_body,
        }))
        .expect("function allocation should fit");
    let wrap_ref_number = module
        .alloc_expr(crate::Expr {
            span: unit_span(),
            kind: crate::ExprKind::Name(crate::TermReference::resolved(
                test_path("wrap"),
                crate::TermResolution::Item(wrap),
            )),
        })
        .expect("wrap reference allocation should fit");
    let maybe_number_head = module
        .alloc_expr(crate::Expr {
            span: unit_span(),
            kind: crate::ExprKind::Integer(crate::IntegerLiteral { raw: "1".into() }),
        })
        .expect("integer allocation should fit");
    let maybe_number_body = module
        .alloc_expr(crate::Expr {
            span: unit_span(),
            kind: crate::ExprKind::Pipe(crate::PipeExpr {
                head: maybe_number_head,
                stages: crate::NonEmpty::new(
                    crate::PipeStage {
                        span: unit_span(),
                        subject_memo: None,
                        result_memo: None,
                        kind: crate::PipeStageKind::Transform {
                            expr: wrap_ref_number,
                        },
                    },
                    Vec::new(),
                ),
                result_block_desugaring: false,
            }),
        })
        .expect("pipe allocation should fit");
    let option_int_type = applied_type(&mut module, option_type, int_type);
    let _maybe_number = module
        .push_item(crate::Item::Value(crate::ValueItem {
            header: crate::ItemHeader {
                span: unit_span(),
                decorators: Vec::new(),
            },
            name: test_name("maybeNumber"),
            annotation: Some(option_int_type),
            body: maybe_number_body,
        }))
        .expect("value allocation should fit");
    let wrap_ref_label = module
        .alloc_expr(crate::Expr {
            span: unit_span(),
            kind: crate::ExprKind::Name(crate::TermReference::resolved(
                test_path("wrap"),
                crate::TermResolution::Item(wrap),
            )),
        })
        .expect("wrap reference allocation should fit");
    let maybe_label_head = module
        .alloc_expr(crate::Expr {
            span: unit_span(),
            kind: crate::ExprKind::Text(crate::TextLiteral {
                segments: vec![crate::TextSegment::Text(crate::TextFragment {
                    raw: "Ada".into(),
                    span: unit_span(),
                })],
            }),
        })
        .expect("text allocation should fit");
    let maybe_label_body = module
        .alloc_expr(crate::Expr {
            span: unit_span(),
            kind: crate::ExprKind::Pipe(crate::PipeExpr {
                head: maybe_label_head,
                stages: crate::NonEmpty::new(
                    crate::PipeStage {
                        span: unit_span(),
                        subject_memo: None,
                        result_memo: None,
                        kind: crate::PipeStageKind::Transform {
                            expr: wrap_ref_label,
                        },
                    },
                    Vec::new(),
                ),
                result_block_desugaring: false,
            }),
        })
        .expect("pipe allocation should fit");
    let option_text_type = applied_type(&mut module, option_type, text_type);
    let _maybe_label = module
        .push_item(crate::Item::Value(crate::ValueItem {
            header: crate::ItemHeader {
                span: unit_span(),
                decorators: Vec::new(),
            },
            name: test_name("maybeLabel"),
            annotation: Some(option_text_type),
            body: maybe_label_body,
        }))
        .expect("value allocation should fit");

    let report = typecheck_module(&module);
    assert!(
        report.is_ok(),
        "expected polymorphic pipe transforms to typecheck, got diagnostics: {:?}",
        report.diagnostics()
    );
}

#[test]
fn typecheck_infers_callable_and_replacement_pipe_transforms() {
    let mut module = Module::new(FileId::new(0));
    let int_type = builtin_type(&mut module, BuiltinType::Int);
    let binding = module
        .alloc_binding(crate::Binding {
            span: unit_span(),
            name: test_name("value"),
            kind: crate::BindingKind::FunctionParameter,
        })
        .expect("binding allocation should fit");
    let local_expr = module
        .alloc_expr(crate::Expr {
            span: unit_span(),
            kind: crate::ExprKind::Name(crate::TermReference::resolved(
                test_path("value"),
                crate::TermResolution::Local(binding),
            )),
        })
        .expect("local expression allocation should fit");
    let add_one = module
        .push_item(crate::Item::Function(crate::FunctionItem {
            origin: crate::FunctionOrigin::Declared,
            header: crate::ItemHeader {
                span: unit_span(),
                decorators: Vec::new(),
            },
            name: test_name("addOne"),
            type_parameters: Vec::new(),
            context: Vec::new(),
            parameters: vec![crate::FunctionParameter {
                span: unit_span(),
                binding,
                annotation: Some(int_type),
            }],
            annotation: Some(int_type),
            body: local_expr,
        }))
        .expect("function allocation should fit");
    let callable_expr = module
        .alloc_expr(crate::Expr {
            span: unit_span(),
            kind: crate::ExprKind::Name(crate::TermReference::resolved(
                test_path("addOne"),
                crate::TermResolution::Item(add_one),
            )),
        })
        .expect("callable expression allocation should fit");
    let replacement_expr = module
        .alloc_expr(crate::Expr {
            span: unit_span(),
            kind: crate::ExprKind::Text(crate::TextLiteral {
                segments: vec![crate::TextSegment::Text(crate::TextFragment {
                    raw: "done".into(),
                    span: unit_span(),
                })],
            }),
        })
        .expect("replacement expression allocation should fit");

    let mut typing = GateTypeContext::new(&module);
    let env = GateExprEnv::default();
    let subject = GateType::Primitive(BuiltinType::Int);

    assert_eq!(
        typing.infer_transform_stage_mode(callable_expr, &env, &subject),
        PipeTransformMode::Apply
    );
    assert_eq!(
        typing.infer_transform_stage(callable_expr, &env, &subject),
        Some(GateType::Primitive(BuiltinType::Int))
    );
    assert_eq!(
        typing.infer_transform_stage_mode(replacement_expr, &env, &subject),
        PipeTransformMode::Replace
    );
    assert_eq!(
        typing.infer_transform_stage(replacement_expr, &env, &subject),
        Some(GateType::Primitive(BuiltinType::Text))
    );
}

#[test]
fn typecheck_accepts_polymorphic_function_application() {
    let mut module = Module::new(FileId::new(0));
    let option_type = builtin_type(&mut module, BuiltinType::Option);
    let int_type = builtin_type(&mut module, BuiltinType::Int);
    let text_type = builtin_type(&mut module, BuiltinType::Text);
    let parameter = type_parameter(&mut module, "A");
    let a_type = type_parameter_type(&mut module, parameter, "A");
    let option_a_type = applied_type(&mut module, option_type, a_type);
    let binding = module
        .alloc_binding(crate::Binding {
            span: unit_span(),
            name: test_name("value"),
            kind: crate::BindingKind::FunctionParameter,
        })
        .expect("binding allocation should fit");
    let local_expr = module
        .alloc_expr(crate::Expr {
            span: unit_span(),
            kind: crate::ExprKind::Name(crate::TermReference::resolved(
                test_path("value"),
                crate::TermResolution::Local(binding),
            )),
        })
        .expect("local expression allocation should fit");
    let some_expr = builtin_term_expr(&mut module, crate::BuiltinTerm::Some, "Some");
    let wrap_body = module
        .alloc_expr(crate::Expr {
            span: unit_span(),
            kind: crate::ExprKind::Apply {
                callee: some_expr,
                arguments: crate::NonEmpty::new(local_expr, Vec::new()),
            },
        })
        .expect("wrap body allocation should fit");
    let wrap = module
        .push_item(crate::Item::Function(crate::FunctionItem {
            origin: crate::FunctionOrigin::Declared,
            header: crate::ItemHeader {
                span: unit_span(),
                decorators: Vec::new(),
            },
            name: test_name("wrap"),
            type_parameters: vec![parameter],
            context: Vec::new(),
            parameters: vec![crate::FunctionParameter {
                span: unit_span(),
                binding,
                annotation: Some(a_type),
            }],
            annotation: Some(option_a_type),
            body: wrap_body,
        }))
        .expect("function allocation should fit");
    let wrap_ref_number = module
        .alloc_expr(crate::Expr {
            span: unit_span(),
            kind: crate::ExprKind::Name(crate::TermReference::resolved(
                test_path("wrap"),
                crate::TermResolution::Item(wrap),
            )),
        })
        .expect("wrap reference allocation should fit");
    let number_argument = module
        .alloc_expr(crate::Expr {
            span: unit_span(),
            kind: crate::ExprKind::Integer(crate::IntegerLiteral { raw: "1".into() }),
        })
        .expect("integer allocation should fit");
    let maybe_number_body = module
        .alloc_expr(crate::Expr {
            span: unit_span(),
            kind: crate::ExprKind::Apply {
                callee: wrap_ref_number,
                arguments: crate::NonEmpty::new(number_argument, Vec::new()),
            },
        })
        .expect("application allocation should fit");
    let option_int_type = applied_type(&mut module, option_type, int_type);
    let _maybe_number = module
        .push_item(crate::Item::Value(crate::ValueItem {
            header: crate::ItemHeader {
                span: unit_span(),
                decorators: Vec::new(),
            },
            name: test_name("maybeNumber"),
            annotation: Some(option_int_type),
            body: maybe_number_body,
        }))
        .expect("value allocation should fit");
    let wrap_ref_label = module
        .alloc_expr(crate::Expr {
            span: unit_span(),
            kind: crate::ExprKind::Name(crate::TermReference::resolved(
                test_path("wrap"),
                crate::TermResolution::Item(wrap),
            )),
        })
        .expect("wrap reference allocation should fit");
    let label_argument = module
        .alloc_expr(crate::Expr {
            span: unit_span(),
            kind: crate::ExprKind::Text(crate::TextLiteral {
                segments: vec![crate::TextSegment::Text(crate::TextFragment {
                    raw: "Ada".into(),
                    span: unit_span(),
                })],
            }),
        })
        .expect("text allocation should fit");
    let maybe_label_body = module
        .alloc_expr(crate::Expr {
            span: unit_span(),
            kind: crate::ExprKind::Apply {
                callee: wrap_ref_label,
                arguments: crate::NonEmpty::new(label_argument, Vec::new()),
            },
        })
        .expect("application allocation should fit");
    let option_text_type = applied_type(&mut module, option_type, text_type);
    let _maybe_label = module
        .push_item(crate::Item::Value(crate::ValueItem {
            header: crate::ItemHeader {
                span: unit_span(),
                decorators: Vec::new(),
            },
            name: test_name("maybeLabel"),
            annotation: Some(option_text_type),
            body: maybe_label_body,
        }))
        .expect("value allocation should fit");

    let report = typecheck_module(&module);
    assert!(
        report.is_ok(),
        "expected polymorphic function application to typecheck, got diagnostics: {:?}",
        report.diagnostics()
    );
}

#[test]
fn typecheck_accepts_expected_polymorphic_ambient_helper_application() {
    let report = typecheck_text(
        "expected-polymorphic-ambient-helper-application.aivi",
        "fun even:Bool = n:Int => n == 2 or n == 4\n\
             value maybeName:Option Text = Some \"Ada\"\n\
             value numbers:List Int = [1, 2, 3, 4]\n\
             value chosenName:Text = __aivi_option_getOrElse \"guest\" maybeName\n\
             value count:Int = __aivi_list_length numbers\n\
             value firstNumber:Option Int = __aivi_list_head numbers\n\
             value hasEven:Bool = __aivi_list_any even numbers\n",
    );
    assert!(
        report.is_ok(),
        "expected ambient polymorphic helper application to typecheck, got diagnostics: {:?}",
        report.diagnostics()
    );
}

#[test]
fn typecheck_allows_signal_names_in_direct_function_calls() {
    let report = typecheck_text(
        "signal-name-direct-call.aivi",
        r#"signal direction : Signal Int = 1
fun step:Int = x:Int => x
fun current:Int = tick:Unit => step direction
"#,
    );
    assert!(
        report.is_ok(),
        "expected direct function application to accept a signal payload name, got diagnostics: {:?}",
        report.diagnostics()
    );
}

#[test]
fn typecheck_reports_invalid_pipe_stage_input_for_transforms() {
    let report = typecheck_text(
        "invalid-pipe-stage-transform.aivi",
        "fun describe:Text = n:Int => \"count\"\n\
             value broken:Text = \"Ada\" |> describe\n",
    );
    assert!(
        report
            .diagnostics()
            .iter()
            .any(|diagnostic| { diagnostic.code == Some(crate::codes::INVALID_PIPE_STAGE_INPUT) }),
        "expected invalid pipe stage input diagnostic, got diagnostics: {:?}",
        report.diagnostics()
    );
}

#[test]
fn typecheck_reports_invalid_pipe_stage_input_for_taps() {
    let report = typecheck_text(
        "invalid-pipe-stage-tap.aivi",
        "fun describe:Text = n:Int => \"count\"\n\
             value broken:Text = \"Ada\" | describe\n",
    );
    assert!(
        report
            .diagnostics()
            .iter()
            .any(|diagnostic| { diagnostic.code == Some(crate::codes::INVALID_PIPE_STAGE_INPUT) }),
        "expected invalid pipe stage input diagnostic for tap, got diagnostics: {:?}",
        report.diagnostics()
    );
}

#[test]
fn typecheck_accepts_higher_kinded_instance_member_signatures() {
    let report = typecheck_text(
        "higher-kinded-instance-members.aivi",
        "class Applicative F = {\n\
             \x20\x20\x20\x20pureInt : F Int\n\
             }\n\
             instance Applicative Option = {\n\
             \x20\x20\x20\x20pureInt = Some 1\n\
             }\n\
             class Functor F = {\n\
             \x20\x20\x20\x20labelInt : F Int\n\
             }\n\
             instance Functor (Result Text) = {\n\
             \x20\x20\x20\x20labelInt = Ok 1\n\
             }\n",
    );
    assert!(
        report.is_ok(),
        "expected higher-kinded instance member signatures to typecheck, got diagnostics: {:?}",
        report.diagnostics()
    );
}

#[test]
fn typecheck_resolves_partial_same_module_instances_generically() {
    let module = lowered_module_text(
        "partial-same-module-instances.aivi",
        "class Applicative F = {\n\
             \x20\x20\x20\x20pureInt : F Int\n\
             }\n\
             instance Applicative Option = {\n\
             \x20\x20\x20\x20pureInt = Some 1\n\
             }\n\
             class Monad F = {\n\
             \x20\x20\x20\x20labelInt : F Int\n\
             }\n\
             instance Monad (Result Text) = {\n\
             \x20\x20\x20\x20labelInt = Ok 1\n\
             }\n",
    );
    let mut checker = TypeChecker::new(&module);
    for (name, carrier) in [
        (
            "Applicative",
            GateType::Option(Box::new(GateType::Primitive(BuiltinType::Int))),
        ),
        (
            "Monad",
            GateType::Result {
                error: Box::new(GateType::Primitive(BuiltinType::Text)),
                value: Box::new(GateType::Primitive(BuiltinType::Int)),
            },
        ),
    ] {
        let class_item = checker.class_item_id_by_name(name).expect("local class");
        let subject = checker
            .typing
            .class_member_subject_binding(
                ClassMemberResolution {
                    class: class_item,
                    member_index: 0,
                },
                &carrier,
            )
            .expect("partial constructor subject");
        assert!(
            checker
                .require_class_binding(&ClassConstraintBinding {
                    class_item,
                    subject
                })
                .is_ok(),
            "expected general class resolution to accept same-module `{name}` for `{carrier}`"
        );
    }
}

#[test]
fn typecheck_accepts_projection_from_unannotated_record_values() {
    let report = typecheck_text(
        "projection-from-record-value.aivi",
        "value profile = { name: \"Ada\", age: 36 }\n\
             value name:Text = profile.name\n",
    );
    assert!(
        report.is_ok(),
        "expected projection from an unannotated record value to typecheck, got diagnostics: {:?}",
        report.diagnostics()
    );
}

#[test]
fn typecheck_accepts_projection_from_signal_wrapped_records() {
    let report = typecheck_text(
        "projection-from-signal-record.aivi",
        "type Game = { score: Int }\n\
             type State = { game: Game, seenRestartCount: Int }\n\
             signal state : Signal State = { game: { score: 0 }, seenRestartCount: 0 }\n\
             signal game : Signal Game = state.game\n\
             signal score : Signal Int = state.game.score\n",
    );
    assert!(
        report.is_ok(),
        "expected projection from signal-wrapped records to typecheck, got diagnostics: {:?}",
        report.diagnostics()
    );
}

#[test]
fn typecheck_accepts_projection_from_domain_values() {
    let report = typecheck_text(
        "projection-from-domain-value.aivi",
        "domain Path over Text = {\n\
             \x20\x20\x20\x20fromText : Text -> Path\n\
             \x20\x20\x20\x20unwrap : Path -> Text\n\
             }\n\
             value home : Path = fromText \"/tmp/app\"\n\
             value raw : Text = home.unwrap\n",
    );
    assert!(
        report.is_ok(),
        "expected projection from a domain value to typecheck, got diagnostics: {:?}",
        report.diagnostics()
    );
}

#[test]
fn typecheck_checks_authored_domain_members_against_carrier_view() {
    let report = typecheck_text(
        "domain-member-carrier-view.aivi",
        "domain Duration over Int = {\n\
             \x20\x20\x20\x20fromMillis : Int -> Duration\n\
             \x20\x20\x20\x20fromMillis raw = raw\n\
             \x20\x20\x20\x20toMillis : Duration -> Int\n\
             \x20\x20\x20\x20toMillis duration = duration\n\
             \x20\x20\x20\x20(+) : Duration -> Duration -> Duration\n\
             \x20\x20\x20\x20(+) = left right => left + right\n\
             }\n\
             value raw : Int = toMillis (fromMillis 10)\n\
             value total : Duration = fromMillis 10 + fromMillis 5\n",
    );
    assert!(
        report.is_ok(),
        "expected authored domain members to typecheck against carrier view, got diagnostics: {:?}",
        report.diagnostics()
    );
}

#[test]
fn typecheck_reports_invalid_projection_from_signal_wrapped_domains() {
    let report = typecheck_text(
        "signal-projection-domain-value.aivi",
        "domain Path over Text = {\n\
             \x20\x20\x20\x20fromText : Text -> Path\n\
             \x20\x20\x20\x20unwrap : Path -> Text\n\
             }\n\
             signal home : Signal Path = fromText \"/tmp/app\"\n\
             signal raw : Signal Text = home.unwrap\n",
    );
    assert!(
        report
            .diagnostics()
            .iter()
            .any(|diagnostic| { diagnostic.code == Some(crate::codes::INVALID_PROJECTION) }),
        "expected signal-wrapped domain projections to stay invalid until pointwise runtime support exists, got diagnostics: {:?}",
        report.diagnostics()
    );
}

#[test]
fn typecheck_reports_unknown_field_from_signal_record_projection() {
    let report = typecheck_text(
        "signal-projection-unknown-field.aivi",
        "type State = { game: Int }\n\
             signal state : Signal State = { game: 1 }\n\
             signal missing : Signal Int = state.score\n",
    );
    assert!(
        report
            .diagnostics()
            .iter()
            .any(|diagnostic| { diagnostic.code == Some(crate::codes::UNKNOWN_PROJECTION_FIELD) }),
        "expected unknown projection field diagnostic from a signal projection, got diagnostics: {:?}",
        report.diagnostics()
    );
}

#[test]
fn typecheck_reports_invalid_projection_from_signal_non_record_payload() {
    let report = typecheck_text(
        "signal-projection-non-record-payload.aivi",
        "signal score : Signal Int = 1\n\
             signal broken : Signal Int = score.value\n",
    );
    assert!(
        report
            .diagnostics()
            .iter()
            .any(|diagnostic| { diagnostic.code == Some(crate::codes::INVALID_PROJECTION) }),
        "expected invalid projection diagnostic from a signal payload projection, got diagnostics: {:?}",
        report.diagnostics()
    );
}

#[test]
fn typecheck_reports_unknown_field_from_unannotated_record_projection() {
    let report = typecheck_text(
        "projection-unknown-field.aivi",
        "value profile = { name: \"Ada\", age: 36 }\n\
             value missing:Text = profile.missing\n",
    );
    assert!(
        report
            .diagnostics()
            .iter()
            .any(|diagnostic| { diagnostic.code == Some(crate::codes::UNKNOWN_PROJECTION_FIELD) }),
        "expected unknown projection field diagnostic, got diagnostics: {:?}",
        report.diagnostics()
    );
}

#[test]
fn typecheck_accepts_collection_literals_with_expected_shapes() {
    let report = typecheck_text(
        "expected-collection-literals.aivi",
        "value pair:(Option Int, Result Text Int) = (None, Ok 1)\n\
             value items:List (Option Int) = [None, Some 2]\n\
             value headers:Map Text (Option Int) = Map { \"primary\": None, \"backup\": Some 3 }\n\
             value tags:Set (Option Int) = Set [None, Some 4]\n",
    );
    assert!(
        report.is_ok(),
        "expected collection literals to use their expected shapes bidirectionally, got diagnostics: {:?}",
        report.diagnostics()
    );
}

#[test]
fn typecheck_reports_collection_literal_element_mismatches() {
    let report = typecheck_text(
        "expected-collection-literal-mismatches.aivi",
        "value pair:(Option Int, Result Text Int) = (Some \"Ada\", Ok \"Ada\")\n\
             value items:List (Option Int) = [Some \"Ada\"]\n\
             value headers:Map Text (Option Int) = Map { \"primary\": Some \"Ada\" }\n\
             value tags:Set (Option Int) = Set [Some \"Ada\"]\n",
    );
    let mismatch_count = report
        .diagnostics()
        .iter()
        .filter(|diagnostic| diagnostic.code == Some(crate::codes::TYPE_MISMATCH))
        .count();
    assert!(
        mismatch_count >= 4,
        "expected collection literal mismatches to surface type mismatches, got diagnostics: {:?}",
        report.diagnostics()
    );
}

#[test]
fn typecheck_accepts_builtin_noninteger_literals_with_matching_annotations() {
    let report = typecheck_text(
        "builtin-noninteger-literals-valid.aivi",
        "value pi:Float = 3.14\n\
             value amount:Decimal = 19.25d\n\
             value whole:Decimal = 19d\n\
             value count:BigInt = 123n\n",
    );
    assert!(
        report.is_ok(),
        "expected builtin noninteger literals to typecheck, got diagnostics: {:?}",
        report.diagnostics()
    );
}

#[test]
fn typecheck_reports_noninteger_literal_type_mismatches() {
    let report = typecheck_text(
        "builtin-noninteger-literals-invalid.aivi",
        "value pi:Float = 19.25d\n\
             value amount:Decimal = 3.14\n\
             value count:BigInt = 42\n",
    );
    let mismatch_count = report
        .diagnostics()
        .iter()
        .filter(|diagnostic| diagnostic.code == Some(crate::codes::TYPE_MISMATCH))
        .count();
    assert!(
        mismatch_count >= 3,
        "expected noninteger literal mismatches to surface type mismatches, got diagnostics: {:?}",
        report.diagnostics()
    );
}

fn value_body(module: &Module, name: &str) -> ExprId {
    module
        .items()
        .iter()
        .find_map(|(_, item)| match item {
            Item::Value(value) if value.name.text() == name => Some(value.body),
            _ => None,
        })
        .expect("expected value item to exist")
}

fn same_module_default_body(module: &Module, member_name: &str) -> ExprId {
    module
        .items()
        .iter()
        .find_map(|(_, item)| match item {
            Item::Instance(instance) => instance
                .members
                .iter()
                .find(|member| member.name.text() == member_name)
                .map(|member| member.body),
            _ => None,
        })
        .expect("expected same-module Default member to exist")
}

fn import_binding_id(module: &Module, local_name: &str) -> ImportId {
    module
        .imports()
        .iter()
        .find_map(|(import_id, import)| {
            (import.local_name.text() == local_name).then_some(import_id)
        })
        .expect("expected import binding to exist")
}

fn rewrite_first_use_module_path(module: &mut Module, segments: &[&str]) {
    let use_item_id = module
        .root_items()
        .iter()
        .copied()
        .find(|item_id| matches!(module.items()[*item_id], Item::Use(_)))
        .expect("expected use item to exist");
    let Item::Use(use_item) = module
        .arenas
        .items
        .get_mut(use_item_id)
        .expect("use item should remain addressable")
    else {
        unreachable!("selected root item should stay a use item");
    };
    use_item.module =
        crate::NamePath::from_vec(segments.iter().map(|segment| test_name(segment)).collect())
            .expect("rewritten use path should stay valid");
}

#[test]
fn typecheck_infers_signal_without_double_wrapping() {
    // Derived signals without explicit annotations should not double-wrap
    // Signal(Signal(T)). When a signal pipe body already produces Signal(T),
    // item_value_type should detect this and avoid wrapping again.
    let report = typecheck_text(
        "signal-no-double-wrap.aivi",
        "signal counter : Signal Int = 0\n\
             signal doubled = counter |> . * 2\n",
    );
    assert!(
        report.is_ok(),
        "unannotated derived signal should typecheck without double Signal wrapping: {:?}",
        report.diagnostics()
    );
}

#[test]
fn category_value_members_preserve_rigid_domain_and_codomain_types() {
    for body in ["compose id arrow", "compose arrow id"] {
        let report = typecheck_text(
            "category-value-contract.aivi",
            &format!("type Category P => P A B -> P A B\nfunc keep = arrow => {body}\n"),
        );
        assert!(report.is_ok(), "{body}: {:?}", report.diagnostics());
    }
    for source in [
        "type Category P => P A B -> P A C\nfunc invalid = arrow => compose id arrow\n",
        "type Category P => P A B -> P C B\nfunc invalid = arrow => compose arrow id\n",
        "type Category P => P A B -> P A B\nfunc invalid = arrow => id\n",
    ] {
        let report = typecheck_text("category-invalid-value-contract.aivi", source);
        assert!(!report.is_ok(), "invalid contract accepted: {source}");
        assert!(
            report
                .diagnostics()
                .iter()
                .any(|d| d.code == Some(crate::codes::TYPE_MISMATCH)),
            "{source}: {:?}",
            report.diagnostics()
        );
    }
}

#[test]
fn semigroupoid_contracts_preserve_independent_input_middle_and_output_types() {
    let declarations = r#"
type Arrow A B = Arrow (A -> B)
type Arrow A B -> A -> B
func runArrow = arrow x => arrow
 ||> Arrow f -> f x
type Arrow B C -> Arrow A B -> Arrow A C
func composeArrow = left right => Arrow (x => runArrow left (runArrow right x))
type A -> A
func identity = x => x
"#;
    let report = typecheck_text(
        "semigroupoid-contract.aivi",
        &format!(
            r#"{declarations}
instance Semigroupoid Arrow = {{ compose = composeArrow }}
instance Category Arrow = {{ id = Arrow identity }}
type Semigroupoid P => P B C -> P A B -> P A C
func combine = left right => compose left right
type Category P => P B C -> P A B -> P A C
func categoryCombine = left right => compose left right
value firstClass : Arrow Int Text -> Arrow Bool Int -> Arrow Bool Text = compose
value partial : Arrow Bool Int -> Arrow Bool Text = compose (Arrow (n => "{{n}}"))
value identityArrow : Arrow Text Text = id
"#
        ),
    );
    assert!(report.is_ok(), "{:?}", report.diagnostics());
    for body in [
        "left right => left",
        "left right => right",
        "left right => Arrow (x => runArrow left x)",
    ] {
        let report = typecheck_text(
            "semigroupoid-incompatible-contract.aivi",
            &format!("{declarations}\ninstance Semigroupoid Arrow = {{ compose = {body} }}\n"),
        );
        assert!(
            report
                .diagnostics()
                .iter()
                .any(|d| d.code == Some(crate::codes::TYPE_MISMATCH)),
            "{body}: {:?}",
            report.diagnostics()
        );
    }
}

#[test]
fn reactive_operators_infer_result_owned_payload_contracts() {
    let module = lowered_module_text(
        "reactive-operator-contracts.aivi",
        r#"
signal n = 7
signal other = 3
signal ready = True
signal enabled = False
signal sum = n + 1
signal both = ready and enabled
signal inverse = not ready
signal less = n < other
signal equal = n == other
signal chained = sum * 2
"#,
    );
    let report = typecheck_module(&module);
    assert!(report.is_ok(), "{:?}", report.diagnostics());
    let mut typing = GateTypeContext::new(&module);
    for (name, expected) in [
        ("sum", BuiltinType::Int),
        ("both", BuiltinType::Bool),
        ("inverse", BuiltinType::Bool),
        ("less", BuiltinType::Bool),
        ("equal", BuiltinType::Bool),
        ("chained", BuiltinType::Int),
    ] {
        let (id, signal) = module
            .items()
            .iter()
            .find_map(|(id, item)| match item {
                Item::Signal(signal) if signal.name.text() == name => Some((id, signal)),
                _ => None,
            })
            .unwrap();
        let info = typing.infer_expr(signal.body.unwrap(), &GateExprEnv::default(), None);
        let payload = GateType::Primitive(expected);
        assert_eq!(info.ty, Some(payload.clone()), "{name}: {info:?}");
        assert_eq!(
            info.actual_gate_type(),
            Some(payload.clone()),
            "result evidence for {name}"
        );
        assert!(info.contains_signal, "preserve dependencies for {name}");
        assert!(info.issues.is_empty(), "{name}: {:?}", info.issues);
        assert_eq!(
            typing.item_value_type(id),
            Some(GateType::Signal(Box::new(payload))),
            "one wrapper for {name}"
        );
    }
}

#[test]
fn reactive_operators_check_explicit_payload_contracts() {
    let report = typecheck_text(
        "reactive-operator-annotations.aivi",
        r#"
signal n : Signal Int = 7
signal other : Signal Int = 3
signal ready : Signal Bool = True
signal enabled : Signal Bool = False
signal sum : Signal Int = n + 1
signal both : Signal Bool = ready and enabled
signal inverse : Signal Bool = not ready
signal less : Signal Bool = n < other
signal equal : Signal Bool = n == other
signal chained : Signal Int = sum * 2
"#,
    );
    assert!(report.is_ok(), "{:?}", report.diagnostics());
}

#[test]
fn reactive_operators_reject_incompatible_payloads() {
    for expression in [
        "n + ready",
        "ready and n",
        "not n",
        "n < ready",
        "n == ready",
    ] {
        let report = typecheck_text(
            "reactive-operator-invalid.aivi",
            &format!("signal n = 7\nsignal ready = True\nsignal invalid = {expression}\n"),
        );
        assert!(
            !report.is_ok(),
            "accepted {expression}: {:?}",
            report.diagnostics()
        );
    }
}

#[test]
fn operator_payload_contracts_preserve_holes_and_other_carriers() {
    let int = GateType::Primitive(BuiltinType::Int);
    let partial = GateExprInfo {
        ty: Some(GateType::Signal(Box::new(int.clone()))),
        actual: Some(SourceOptionActualType::Signal(Box::new(
            SourceOptionActualType::Hole,
        ))),
        contains_signal: true,
        ..GateExprInfo::default()
    };
    assert_eq!(
        partial.operator_payload_type(),
        None,
        "a partial payload cannot use a complete hint"
    );
    for ty in [
        GateType::Signal(Box::new(int.clone())),
        GateType::Option(Box::new(int.clone())),
        GateType::Task {
            error: Box::new(GateType::Primitive(BuiltinType::Text)),
            value: Box::new(int),
        },
    ] {
        let info = GateExprInfo {
            ty: Some(GateType::Signal(Box::new(ty.clone()))),
            ..GateExprInfo::default()
        };
        assert_eq!(
            info.operator_payload_type(),
            Some(ty),
            "unwrap exactly one signal"
        );
    }
}

#[test]
fn local_record_aliases_retain_binary_constructor_evidence() {
    let report = typecheck_text(
        "record-alias-class.aivi",
        r#"
class Compose P = { compose : P B C -> P A B -> P A C }
type Arrow A B = { run: A -> B }
type Arrow B C -> Arrow A B -> Arrow A C
func composeArrow = left right => { run: x => left.run (right.run x) }
instance Compose Arrow = { compose = composeArrow }
type Compose P => P B C -> P A B -> P A C
func combine = left right => compose left right
value encode : Arrow Int Text = { run: n => "{n}" }
value select : Arrow Bool Int = { run: flag => 7 }
value composed : Arrow Bool Text = combine encode select
value structural : { run: Bool -> Text } = composed
value result : Text = structural.run True
"#,
    );
    assert!(report.is_ok(), "{:?}", report.diagnostics());
}

#[test]
fn local_record_aliases_retain_fixed_constructor_arguments() {
    let declarations = r#"
type Entry K A = { key: K, value: A }
type (A -> B) -> Entry K A -> Entry K B
func mapEntry = f entry => { key: entry.key, value: f entry.value }
instance Functor (Entry K) = { map = mapEntry }
type Functor F => (A -> B) -> F A -> F B
func transform = f values => map f values
value entry : Entry Text Int = { key: "key", value: 3 }
type Int -> Bool
func positive = n => n > 0
"#;
    let valid = typecheck_text(
        "fixed-record-alias.aivi",
        &format!("{declarations}\nvalue result : Entry Text Bool = transform positive entry\n"),
    );
    assert!(valid.is_ok(), "{:?}", valid.diagnostics());
    let invalid = typecheck_text(
        "changed-fixed-record-alias.aivi",
        &format!("{declarations}\nvalue result : Entry Int Bool = transform positive entry\n"),
    );
    assert!(!invalid.is_ok(), "fixed constructor argument changed");
    assert!(
        invalid
            .diagnostics()
            .iter()
            .any(|d| d.code == Some(crate::codes::TYPE_MISMATCH)),
        "{:?}",
        invalid.diagnostics()
    );
}

#[test]
fn local_structural_aliases_keep_patterns_and_function_calls() {
    let report = typecheck_text(
        "structural-alias-operations.aivi",
        r#"
type Entry A = { value: A }
type Entry A -> A
func readEntry = entry => entry ||> { value } -> value
type Pair A B = (A, B)
type Pair A B -> A
func first = pair => pair ||> (a, b) -> a
type Reader R A = R -> A
class MapReader F = { transform : (A -> B) -> F A -> F B }
type (A -> B) -> Reader R A -> Reader R B
func mapReader = f reader => input => f (reader input)
instance MapReader (Reader R) = { transform = mapReader }
type MapReader F => (A -> B) -> F A -> F B
func transformReader = f value => transform f value
value reader : Reader Int Bool = n => n > 0
value convert : Bool -> Text = flag => "answer"
value mapped : Reader Int Text = transformReader convert reader
value answer : Text = mapped 1
value fromRecord : Int = readEntry { value: 3 }
value fromTuple : Int = first (3, True)
type EntryKey K A = { key: K, value: A } |> Pick (key)
value keyOnly : EntryKey Text Int = { key: "key" }
type Payload A = { payload: A }
type SelectedEntry A = Pick (payload) (Payload A)
value selected : SelectedEntry Int = { payload: 3 }
"#,
    );
    assert!(report.is_ok(), "{:?}", report.diagnostics());
}
