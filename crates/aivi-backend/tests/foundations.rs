use std::{fs, path::PathBuf};

use aivi_backend::{
    DecodeStepKind, DomainDecodeSurfaceKind, GateStage as BackendGateStage,
    ItemKind as BackendItemKind, KernelExprKind, LayoutKind, LoweringError, NonSourceWakeupCause,
    RecurrenceTarget, SourceProvider, StageKind as BackendStageKind, ValidationError,
    lower_module as lower_backend_module, validate_program,
};
use aivi_base::{SourceDatabase, SourceSpan};
use aivi_core::{
    Expr as CoreExpr, ExprKind as CoreExprKind, GateStage as CoreGateStage, Item as CoreItem,
    ItemKind as CoreItemKind, Module as CoreModule, Pipe as CorePipe, PipeOrigin as CorePipeOrigin,
    Reference as CoreReference, Stage as CoreStage, StageKind as CoreStageKind, Type as CoreType,
    lower_module as lower_core_module, validate_module as validate_core_module,
};
use aivi_hir::{
    BindingId as HirBindingId, BuiltinType, ExprId as HirExprId, IntegerLiteral,
    ItemId as HirItemId,
};
use aivi_syntax::parse_module;

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join("frontend")
}

fn lower_text(path: &str, text: &str) -> aivi_backend::Program {
    let mut sources = SourceDatabase::new();
    let file_id = sources.add_file(path, text);
    let parsed = parse_module(&sources[file_id]);
    assert!(
        !parsed.has_errors(),
        "backend test input should parse: {:?}",
        parsed.all_diagnostics().collect::<Vec<_>>()
    );
    let hir = aivi_hir::lower_module(&parsed.module);
    assert!(
        !hir.has_errors(),
        "backend test input should lower to HIR: {:?}",
        hir.diagnostics()
    );
    let core = lower_core_module(hir.module()).expect("HIR should lower into typed core");
    validate_core_module(&core).expect("typed core should validate before backend lowering");
    let backend = lower_backend_module(&core).expect("backend lowering should succeed");
    validate_program(&backend).expect("backend program should validate");
    backend
}

fn lower_fixture(path: &str) -> aivi_backend::Program {
    let text = fs::read_to_string(fixture_root().join(path)).expect("fixture should be readable");
    lower_text(path, &text)
}

fn find_item(program: &aivi_backend::Program, name: &str) -> aivi_backend::ItemId {
    program
        .items()
        .iter()
        .find(|(_, item)| item.name.as_ref() == name)
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("expected backend item `{name}`"))
}

fn first_pipeline(
    program: &aivi_backend::Program,
    item: aivi_backend::ItemId,
) -> aivi_backend::PipelineId {
    program.items()[item].pipelines[0]
}

fn manual_core_gate(when_true: CoreExprKind) -> CoreModule {
    let span = SourceSpan::default();
    let mut module = CoreModule::new();
    let item_id = module
        .items_mut()
        .alloc(CoreItem {
            origin: HirItemId::from_raw(0),
            span,
            name: "captured".into(),
            kind: CoreItemKind::Value,
            pipes: Vec::new(),
        })
        .expect("item allocation should fit");
    let when_true = module
        .exprs_mut()
        .alloc(CoreExpr {
            span,
            ty: CoreType::Primitive(BuiltinType::Int),
            kind: when_true,
        })
        .expect("expression allocation should fit");
    let when_false = module
        .exprs_mut()
        .alloc(CoreExpr {
            span,
            ty: CoreType::Primitive(BuiltinType::Int),
            kind: CoreExprKind::Integer(IntegerLiteral { raw: "0".into() }),
        })
        .expect("expression allocation should fit");
    let pipe_id = module
        .pipes_mut()
        .alloc(CorePipe {
            owner: item_id,
            origin: CorePipeOrigin {
                owner: HirItemId::from_raw(0),
                pipe_expr: HirExprId::from_raw(0),
                span,
            },
            stages: Vec::new(),
            recurrence: None,
        })
        .expect("pipe allocation should fit");
    let stage_id = module
        .stages_mut()
        .alloc(CoreStage {
            pipe: pipe_id,
            index: 0,
            span,
            input_subject: CoreType::Primitive(BuiltinType::Int),
            result_subject: CoreType::Primitive(BuiltinType::Int),
            kind: CoreStageKind::Gate(CoreGateStage::Ordinary {
                when_true,
                when_false,
            }),
        })
        .expect("stage allocation should fit");
    module
        .pipes_mut()
        .get_mut(pipe_id)
        .expect("pipe should exist")
        .stages
        .push(stage_id);
    module
        .items_mut()
        .get_mut(item_id)
        .expect("item should exist")
        .pipes
        .push(pipe_id);
    module
}

#[test]
fn lowers_gate_fixture_into_backend_ir() {
    let backend = lower_fixture("milestone-2/valid/pipe-gate-carriers/main.aivi");
    let maybe_active = find_item(&backend, "maybeActive");
    let pipeline = &backend.pipelines()[first_pipeline(&backend, maybe_active)];
    let stage = &pipeline.stages[0];
    let BackendStageKind::Gate(BackendGateStage::Ordinary {
        when_true,
        when_false,
    }) = &stage.kind
    else {
        panic!("expected gate stage in maybeActive pipeline");
    };
    assert_eq!(
        backend.kernels()[*when_true].input_subject,
        Some(stage.input_layout)
    );
    assert_eq!(backend.kernels()[*when_false].input_subject, None);
    let pretty = backend.pretty();
    assert!(pretty.contains("runtime-kernel-v1"));
    assert!(pretty.contains("gate-false"));
}

#[test]
fn lowers_source_decode_into_backend_plans() {
    let backend = lower_text(
        "backend-source-decode.aivi",
        r#"
domain Duration over Int
    parse : Int -> Result Text Duration
    value : Duration -> Int

@source custom.feed
sig timeout : Signal Duration
"#,
    );

    let timeout = find_item(&backend, "timeout");
    let BackendItemKind::Signal(signal) = &backend.items()[timeout].kind else {
        panic!("timeout should remain a signal item");
    };
    let source = &backend.sources()[signal.source.expect("timeout should carry a source")];
    assert!(
        matches!(source.provider, SourceProvider::Custom(ref key) if key.as_ref() == "custom.feed")
    );
    let decode = &backend.decode_plans()[source.decode.expect("source should carry a decode plan")];
    let root = &decode.steps()[decode.root];
    match &root.kind {
        DecodeStepKind::Domain { surface, .. } => {
            assert_eq!(surface.member_name.as_ref(), "parse");
            assert_eq!(surface.kind, DomainDecodeSurfaceKind::FallibleResult);
        }
        other => panic!("expected domain decode root, found {other:?}"),
    }
    assert!(matches!(
        backend.layouts()[root.layout].kind,
        LayoutKind::AnonymousDomain { .. }
    ));
}

#[test]
fn lowers_recurrence_targets_and_witnesses() {
    let backend = lower_fixture("milestone-2/valid/pipe-recurrence-nonsource-wakeup/main.aivi");

    let polled = find_item(&backend, "polled");
    let polled_recurrence = backend.pipelines()[first_pipeline(&backend, polled)]
        .recurrence
        .as_ref()
        .expect("polled should carry a recurrence plan");
    assert_eq!(polled_recurrence.target, RecurrenceTarget::Signal);
    assert_eq!(polled_recurrence.steps.len(), 1);
    assert!(matches!(
        polled_recurrence
            .non_source_wakeup
            .as_ref()
            .map(|w| w.cause),
        Some(NonSourceWakeupCause::ExplicitTimer)
    ));

    let retried = find_item(&backend, "retried");
    let retried_recurrence = backend.pipelines()[first_pipeline(&backend, retried)]
        .recurrence
        .as_ref()
        .expect("retried should carry a recurrence plan");
    assert_eq!(retried_recurrence.target, RecurrenceTarget::Task);
    assert!(matches!(
        retried_recurrence
            .non_source_wakeup
            .as_ref()
            .map(|w| w.cause),
        Some(NonSourceWakeupCause::ExplicitBackoff)
    ));
}

#[test]
fn lowers_domain_operators_into_backend_gate_kernels() {
    let backend = lower_text(
        "backend-domain-operators.aivi",
        r#"
domain Duration over Int
    literal ms : Int -> Duration
    (+) : Duration -> Duration -> Duration
    (>) : Duration -> Duration -> Bool

type Window = {
    delay: Duration
}

sig windows : Signal Window = { delay: 10ms }

sig slowWindows : Signal Window =
    windows
     ?|> ((.delay + 5ms) > 12ms)
"#,
    );

    let slow_windows = find_item(&backend, "slowWindows");
    let pipeline = &backend.pipelines()[first_pipeline(&backend, slow_windows)];
    let BackendStageKind::Gate(BackendGateStage::SignalFilter { predicate, .. }) =
        &pipeline.stages[0].kind
    else {
        panic!("expected signal-filter gate stage for slowWindows");
    };

    let predicate_kernel = &backend.kernels()[*predicate];
    match &predicate_kernel.exprs()[predicate_kernel.root].kind {
        KernelExprKind::Apply { callee, arguments } => {
            assert_eq!(arguments.len(), 2);
            match &predicate_kernel.exprs()[*callee].kind {
                KernelExprKind::DomainMember(handle) => {
                    assert_eq!(handle.domain_name.as_ref(), "Duration");
                    assert_eq!(handle.member_name.as_ref(), ">");
                }
                other => panic!(
                    "expected explicit domain-member callee for outer comparison, found {other:?}"
                ),
            }
            match &predicate_kernel.exprs()[arguments[0]].kind {
                KernelExprKind::Apply { callee, arguments } => {
                    assert_eq!(arguments.len(), 2);
                    match &predicate_kernel.exprs()[*callee].kind {
                        KernelExprKind::DomainMember(handle) => {
                            assert_eq!(handle.domain_name.as_ref(), "Duration");
                            assert_eq!(handle.member_name.as_ref(), "+");
                        }
                        other => panic!(
                            "expected explicit domain-member callee for nested add, found {other:?}"
                        ),
                    }
                    assert!(matches!(
                        &predicate_kernel.exprs()[arguments[0]].kind,
                        KernelExprKind::Projection { .. }
                    ));
                    assert!(matches!(
                        &predicate_kernel.exprs()[arguments[1]].kind,
                        KernelExprKind::SuffixedInteger(_)
                    ));
                }
                other => panic!(
                    "expected outer comparison left operand to be a nested apply tree, found {other:?}"
                ),
            }
            assert!(matches!(
                &predicate_kernel.exprs()[arguments[1]].kind,
                KernelExprKind::SuffixedInteger(_)
            ));
        }
        other => panic!(
            "expected predicate kernel to lower into an explicit apply tree, found {other:?}"
        ),
    }
}

#[test]
fn lowering_makes_local_bindings_explicit_environment_slots() {
    let core = manual_core_gate(CoreExprKind::Reference(CoreReference::Local(
        HirBindingId::from_raw(7),
    )));
    validate_core_module(&core).expect("manual core module should validate");
    let backend = lower_backend_module(&core).expect("backend lowering should succeed");
    let item = find_item(&backend, "captured");
    let pipeline = &backend.pipelines()[first_pipeline(&backend, item)];
    let BackendStageKind::Gate(BackendGateStage::Ordinary {
        when_true,
        when_false,
    }) = &pipeline.stages[0].kind
    else {
        panic!("expected ordinary gate stage");
    };
    assert_eq!(backend.kernels()[*when_true].environment.len(), 1);
    assert_eq!(backend.kernels()[*when_true].input_subject, None);
    assert!(backend.kernels()[*when_false].environment.is_empty());
}

#[test]
fn validator_catches_missing_kernel_input_subject() {
    let mut backend = lower_fixture("milestone-2/valid/pipe-gate-carriers/main.aivi");
    let maybe_active = find_item(&backend, "maybeActive");
    let pipeline_id = first_pipeline(&backend, maybe_active);
    let when_true = match &backend.pipelines()[pipeline_id].stages[0].kind {
        BackendStageKind::Gate(BackendGateStage::Ordinary { when_true, .. }) => *when_true,
        other => panic!("expected ordinary gate stage, found {other:?}"),
    };
    backend
        .kernels_mut()
        .get_mut(when_true)
        .expect("gate kernel should exist")
        .input_subject = None;

    let errors =
        validate_program(&backend).expect_err("missing input subject should fail validation");
    assert!(errors.errors().iter().any(|error| {
        matches!(
            error,
            ValidationError::KernelMissingInputSubject { kernel, .. }
                | ValidationError::KernelConventionMismatch { kernel }
                if *kernel == when_true
        )
    }));
}

#[test]
fn lowering_rejects_unresolved_hir_item_references() {
    let core = manual_core_gate(CoreExprKind::Reference(CoreReference::HirItem(
        HirItemId::from_raw(99),
    )));
    validate_core_module(&core).expect("manual core module should validate");
    let errors =
        lower_backend_module(&core).expect_err("unresolved HIR item reference should fail");
    assert!(
        errors
            .errors()
            .iter()
            .any(|error| matches!(error, LoweringError::UnresolvedItemReference { .. }))
    );
}
