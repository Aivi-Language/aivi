//! Converts runtime errors into structured [`Diagnostic`]s using the
//! [`RuntimeSourceMap`] so that errors can be rendered with source context.

use aivi_backend::{
    EvaluationError, KernelExprKind, KernelId, Program as BackendProgram, describe_expr_kind,
};
use aivi_base::{Diagnostic, DiagnosticCode};

use crate::{
    SourceProviderExecutionError, graph::SignalGraph, source_map::RuntimeSourceMap,
    startup::BackendRuntimeError,
};

const RUNTIME_DOMAIN: &str = "runtime";

const SIGNAL_EVAL_FAILED: DiagnosticCode =
    DiagnosticCode::new(RUNTIME_DOMAIN, "SIGNAL_EVAL_FAILED");
const SOURCE_EVAL_FAILED: DiagnosticCode =
    DiagnosticCode::new(RUNTIME_DOMAIN, "SOURCE_EVAL_FAILED");
const SOURCE_PROVIDER_FAILED: DiagnosticCode =
    DiagnosticCode::new(RUNTIME_DOMAIN, "SOURCE_PROVIDER_FAILED");
const SOURCE_DECODE_FAILED: DiagnosticCode =
    DiagnosticCode::new(RUNTIME_DOMAIN, "SOURCE_DECODE_FAILED");
const TASK_EVAL_FAILED: DiagnosticCode = DiagnosticCode::new(RUNTIME_DOMAIN, "TASK_EVAL_FAILED");
const RUNTIME_INTERNAL: DiagnosticCode = DiagnosticCode::new(RUNTIME_DOMAIN, "INTERNAL");

/// Identify which pipeline stage (if any) references the given kernel.
///
/// Returns `(stage_label, stage_span, stage_index)` for the first match.
fn find_pipe_stage_for_kernel(
    backend: &BackendProgram,
    pipeline_ids: &[aivi_backend::PipelineId],
    kernel: KernelId,
) -> Option<(&'static str, aivi_base::SourceSpan, usize)> {
    for &pid in pipeline_ids {
        let pipeline = &backend.pipelines()[pid];
        for stage in &pipeline.stages {
            let references_kernel = match &stage.kind {
                aivi_backend::StageKind::Gate(aivi_backend::GateStage::SignalFilter {
                    predicate,
                    ..
                }) => *predicate == kernel,
                aivi_backend::StageKind::Gate(aivi_backend::GateStage::Ordinary {
                    when_true,
                    when_false,
                }) => *when_true == kernel || *when_false == kernel,
                aivi_backend::StageKind::Fanout(f) => {
                    f.map == kernel
                        || f.filters.iter().any(|ff| ff.predicate == kernel)
                        || f.join.as_ref().is_some_and(|j| j.kernel == kernel)
                }
                aivi_backend::StageKind::Temporal(t) => match t {
                    aivi_backend::TemporalStage::Previous { seed, .. } => *seed == kernel,
                    aivi_backend::TemporalStage::DiffFunction { diff, .. } => *diff == kernel,
                    aivi_backend::TemporalStage::DiffSeed { seed, .. } => *seed == kernel,
                    aivi_backend::TemporalStage::Delay { .. }
                    | aivi_backend::TemporalStage::Burst { .. } => false,
                },
                aivi_backend::StageKind::TruthyFalsy(_) => false,
            };
            if references_kernel {
                return Some((stage.kind.label(), stage.span, stage.index));
            }
        }
    }
    None
}

/// Extract the `KernelId` from an `EvaluationError` if it carries one.
fn eval_error_kernel(error: &EvaluationError) -> Option<KernelId> {
    // Most EvaluationError variants carry a kernel field.
    match error {
        EvaluationError::UnknownKernel { kernel }
        | EvaluationError::MissingInputSubject { kernel }
        | EvaluationError::UnexpectedInputSubject { kernel }
        | EvaluationError::KernelEnvironmentCountMismatch { kernel, .. }
        | EvaluationError::KernelInputLayoutMismatch { kernel, .. }
        | EvaluationError::KernelEnvironmentLayoutMismatch { kernel, .. }
        | EvaluationError::KernelResultLayoutMismatch { kernel, .. }
        | EvaluationError::UnknownEnvironmentSlot { kernel, .. }
        | EvaluationError::UnknownInlineSubject { kernel, .. }
        | EvaluationError::UnknownProjectionField { kernel, .. }
        | EvaluationError::InvalidProjectionBase { kernel, .. }
        | EvaluationError::InvalidCallee { kernel, .. }
        | EvaluationError::InvalidIntrinsicArgument { kernel, .. }
        | EvaluationError::IntrinsicFailed { kernel, .. }
        | EvaluationError::UnsupportedDomainMemberCall { kernel, .. }
        | EvaluationError::UnsupportedBuiltinClassMember { kernel, .. }
        | EvaluationError::UnsupportedInlinePipe { kernel, .. }
        | EvaluationError::UnsupportedInlinePipeSignalSubject { kernel, .. }
        | EvaluationError::UnsupportedInlinePipePattern { kernel, .. }
        | EvaluationError::InlinePipeCaseNoMatch { kernel, .. }
        | EvaluationError::UnsupportedUnary { kernel, .. }
        | EvaluationError::UnsupportedBinary { kernel, .. }
        | EvaluationError::InvalidBinaryArithmetic { kernel, .. }
        | EvaluationError::InvalidInterpolationValue { kernel, .. }
        | EvaluationError::InvalidIntegerLiteral { kernel, .. }
        | EvaluationError::InvalidFloatLiteral { kernel, .. }
        | EvaluationError::InvalidDecimalLiteral { kernel, .. }
        | EvaluationError::InvalidBigIntLiteral { kernel, .. }
        | EvaluationError::UnsupportedStructuralEquality { kernel, .. } => Some(*kernel),
        EvaluationError::UnknownItem { .. }
        | EvaluationError::MissingItemBody { .. }
        | EvaluationError::MissingItemValue { .. }
        | EvaluationError::RecursiveItemEvaluation { .. }
        | EvaluationError::UnsupportedNativeOnlyRuntimeOperation { .. } => None,
    }
}

fn push_eval_error_layout_notes(
    mut diag: Diagnostic,
    backend: &BackendProgram,
    error: &EvaluationError,
) -> Diagnostic {
    match error {
        EvaluationError::KernelInputLayoutMismatch { expected, .. }
        | EvaluationError::KernelResultLayoutMismatch { expected, .. } => {
            diag = diag.with_note(format!(
                "layout{expected} = {}",
                backend.layouts()[*expected]
            ));
        }
        EvaluationError::KernelEnvironmentLayoutMismatch { expected, slot, .. } => {
            diag = diag.with_note(format!(
                "environment slot {slot} expects layout{expected} = {}",
                backend.layouts()[*expected]
            ));
        }
        _ => {}
    }
    diag
}

/// Convert a [`BackendRuntimeError`] into one or more [`Diagnostic`]s,
/// using the source map, signal graph, and backend program for rich
/// source-level context including pipe stage tracking.
pub fn render_runtime_error(
    error: &BackendRuntimeError,
    source_map: &RuntimeSourceMap,
    graph: &SignalGraph,
    backend: Option<&BackendProgram>,
) -> Vec<Diagnostic> {
    match error {
        BackendRuntimeError::EvaluateDerivedSignal {
            signal,
            item,
            error: eval_error,
        } => {
            let name = source_map
                .derived_name(*signal)
                .unwrap_or("(unknown signal)");
            let mut diag = Diagnostic::error(format!(
                "derived signal `{name}` failed to evaluate: {eval_error}"
            ))
            .with_code(SIGNAL_EVAL_FAILED);

            if let Some(span) = source_map.item_span(*item) {
                diag = diag.with_primary_label(span, "signal evaluation failed here");
            }

            // Try to identify which pipe stage failed.
            if let (Some(backend), Some(kernel)) = (backend, eval_error_kernel(eval_error)) {
                diag = push_eval_error_layout_notes(diag, backend, eval_error);
                if let Some(pipeline_ids) = source_map.signal_pipeline_ids(signal.as_signal())
                    && let Some((label, stage_span, index)) =
                        find_pipe_stage_for_kernel(backend, pipeline_ids, kernel)
                {
                    diag = diag.with_secondary_label(
                        stage_span,
                        format!("pipe stage {index} ({label}) failed"),
                    );
                }
            }

            // Trace dependency chain.
            let chains = source_map.trace_signal_dependencies(graph, signal.as_signal());
            if let Some(chain) = chains.first() {
                let trace = source_map.format_dependency_chain(chain);
                diag = diag.with_note(format!("dependency chain: {trace}"));
            }

            vec![diag]
        }

        BackendRuntimeError::EvaluateReactiveSeed {
            signal,
            item,
            error: eval_error,
        } => {
            let name = source_map.signal_name(*signal).unwrap_or("(unknown)");
            let mut diag = Diagnostic::error(format!(
                "reactive signal `{name}` seed evaluation failed: {eval_error}"
            ))
            .with_code(SIGNAL_EVAL_FAILED);

            if let Some(span) = source_map.item_span(*item) {
                diag = diag.with_primary_label(span, "reactive seed failed here");
            }

            if let Some(backend) = backend {
                diag = push_eval_error_layout_notes(diag, backend, eval_error);
            }

            let chains = source_map.trace_signal_dependencies(graph, *signal);
            if let Some(chain) = chains.first() {
                let trace = source_map.format_dependency_chain(chain);
                diag = diag.with_note(format!("dependency chain: {trace}"));
            }

            vec![diag]
        }

        BackendRuntimeError::EvaluateReactiveGuard {
            signal,
            item,
            error: eval_error,
            ..
        } => {
            let name = source_map.signal_name(*signal).unwrap_or("(unknown)");
            let mut diag = Diagnostic::error(format!(
                "reactive signal `{name}` guard evaluation failed: {eval_error}"
            ))
            .with_code(SIGNAL_EVAL_FAILED);

            if let Some(span) = source_map.item_span(*item) {
                diag = diag.with_primary_label(span, "reactive guard failed here");
            }
            diag = diag.with_help("the guard expression must return a Bool value");

            vec![diag]
        }

        BackendRuntimeError::EvaluateReactiveBody {
            signal,
            item,
            error: eval_error,
            ..
        } => {
            let name = source_map.signal_name(*signal).unwrap_or("(unknown)");
            let mut diag = Diagnostic::error(format!(
                "reactive signal `{name}` body evaluation failed: {eval_error}"
            ))
            .with_code(SIGNAL_EVAL_FAILED);

            if let Some(span) = source_map.item_span(*item) {
                diag = diag.with_primary_label(span, "reactive body failed here");
            }

            if let Some(backend) = backend {
                diag = push_eval_error_layout_notes(diag, backend, eval_error);
            }

            vec![diag]
        }

        BackendRuntimeError::ReactiveGuardReturnedNonBool {
            signal,
            item,
            value,
            ..
        } => {
            let name = source_map.signal_name(*signal).unwrap_or("(unknown)");
            let mut diag = Diagnostic::error(format!(
                "reactive signal `{name}` guard must return Bool, got `{value:?}`"
            ))
            .with_code(SIGNAL_EVAL_FAILED);

            if let Some(span) = source_map.item_span(*item) {
                diag = diag.with_primary_label(span, "this guard expression");
            }
            diag = diag.with_help("ensure the `when` clause returns a Bool value");

            vec![diag]
        }

        BackendRuntimeError::ReactiveBodyReturnedNonOption {
            signal,
            item,
            value,
            ..
        } => {
            let name = source_map.signal_name(*signal).unwrap_or("(unknown)");
            let mut diag = Diagnostic::error(format!(
                "reactive signal `{name}` body must return Option, got `{value:?}`"
            ))
            .with_code(SIGNAL_EVAL_FAILED);

            if let Some(span) = source_map.item_span(*item) {
                diag = diag.with_primary_label(span, "this reactive body");
            }
            diag = diag.with_help(
                "the body of an optional reactive update must return `Some value` or `None`",
            );

            vec![diag]
        }

        BackendRuntimeError::EvaluateRecurrenceSignal {
            signal,
            item,
            error: eval_error,
        } => {
            let name = source_map.derived_name(*signal).unwrap_or("(unknown)");
            let mut diag = Diagnostic::error(format!(
                "recurrence signal `{name}` evaluation failed: {eval_error}"
            ))
            .with_code(SIGNAL_EVAL_FAILED);

            if let Some(span) = source_map.item_span(*item) {
                diag = diag.with_primary_label(span, "recurrence step failed here");
            }

            if let (Some(backend), Some(kernel)) = (backend, eval_error_kernel(eval_error)) {
                diag = push_eval_error_layout_notes(diag, backend, eval_error);
                let backend_kernel = &backend.kernels()[kernel];
                let root_expr = &backend_kernel.exprs()[backend_kernel.root];
                let root_kind = describe_expr_kind(&root_expr.kind);
                let root_item_note = match &root_expr.kind {
                    KernelExprKind::Item(item) => {
                        let backend_item = &backend.items()[*item];

                        backend_item.body.map_or_else(
                            || "; item body = <none>".to_owned(),
                            |body| {
                                let body_kernel = &backend.kernels()[body];
                                let body_root = &body_kernel.exprs()[body_kernel.root];
                                let body_root_kind = describe_expr_kind(&body_root.kind);
                                format!(
                                    "; root item{} = {}; item body = kernel{} ({}) root {} layout{}",
                                    item,
                                    backend.item_name(*item),
                                    body,
                                    body_kernel.origin.kind,
                                    body_root_kind,
                                    body_kernel.result_layout
                                )
                            },
                        )
                    }
                    _ => String::new(),
                };
                diag = diag.with_note(format!(
                    "kernel{kernel} origin: {} in item{} ({}); root expr: {root_kind}{root_item_note}; expr layout{}; result layout{}",
                    backend_kernel.origin.kind,
                    backend_kernel.origin.item,
                    backend.item_name(backend_kernel.origin.item),
                    root_expr.layout,
                    backend_kernel.result_layout
                ));
                if let Some(pipeline_ids) = source_map.signal_pipeline_ids(signal.as_signal())
                    && let Some((label, stage_span, index)) =
                        find_pipe_stage_for_kernel(backend, pipeline_ids, kernel)
                {
                    diag = diag
                        .with_secondary_label(
                            stage_span,
                            format!("recurrence pipe stage {index} ({label}) failed"),
                        )
                        .with_note(format!(
                            "failing recurrence pipe stage: stage {index} ({label})"
                        ));
                }
            }

            let chains = source_map.trace_signal_dependencies(graph, signal.as_signal());
            if let Some(chain) = chains.first() {
                let trace = source_map.format_dependency_chain(chain);
                diag = diag.with_note(format!("dependency chain: {trace}"));
            }

            vec![diag]
        }

        BackendRuntimeError::EvaluateSourceArgument {
            instance,
            index,
            error: eval_error,
        } => {
            let name = source_map
                .source_name(*instance)
                .unwrap_or("(unknown source)");
            let mut diag = Diagnostic::error(format!(
                "source `{name}` argument {index} evaluation failed: {eval_error}"
            ))
            .with_code(SOURCE_EVAL_FAILED);

            if let Some(span) = source_map.source_span(*instance) {
                diag = diag.with_primary_label(span, format!("argument {index} failed"));
            }

            vec![diag]
        }

        BackendRuntimeError::EvaluateSourceOption {
            instance,
            option_name,
            error: eval_error,
        } => {
            let name = source_map
                .source_name(*instance)
                .unwrap_or("(unknown source)");
            let mut diag = Diagnostic::error(format!(
                "source `{name}` option `{option_name}` evaluation failed: {eval_error}"
            ))
            .with_code(SOURCE_EVAL_FAILED);

            if let Some(span) = source_map.source_span(*instance) {
                diag = diag.with_primary_label(span, format!("option `{option_name}` failed"));
            }

            vec![diag]
        }

        BackendRuntimeError::InvalidActiveWhenValue { instance, value } => {
            let name = source_map
                .source_name(*instance)
                .unwrap_or("(unknown source)");
            let mut diag = Diagnostic::error(format!(
                "source `{name}` activeWhen expression must produce Bool, got `{value:?}`"
            ))
            .with_code(SOURCE_EVAL_FAILED);

            if let Some(span) = source_map.source_span(*instance) {
                diag = diag.with_primary_label(span, "this source declaration");
            }
            diag = diag.with_help("the `activeWhen` option must evaluate to a Bool value");

            vec![diag]
        }

        BackendRuntimeError::EvaluateTaskBody {
            owner,
            error: eval_error,
            ..
        } => {
            let name = source_map.item_name(*owner).unwrap_or("(unknown task)");
            let mut diag = Diagnostic::error(format!(
                "task `{name}` body evaluation failed: {eval_error}"
            ))
            .with_code(TASK_EVAL_FAILED);

            if let Some(span) = source_map.item_span(*owner) {
                diag = diag.with_primary_label(span, "task body evaluation failed here");
            }

            vec![diag]
        }

        BackendRuntimeError::DerivedDependencyArityMismatch {
            signal,
            expected,
            found,
        } => {
            let name = source_map.derived_name(*signal).unwrap_or("(unknown)");
            let diag = Diagnostic::error(format!(
                "derived signal `{name}` expected {expected} dependencies, found {found}"
            ))
            .with_code(RUNTIME_INTERNAL)
            .with_note("this is likely a compiler bug — the signal graph and backend disagree on dependency count".to_owned());

            vec![diag]
        }

        BackendRuntimeError::MissingNativeDerivedPlan {
            signal,
            item,
            kernel,
        } => {
            let name = source_map.derived_name(*signal).unwrap_or("(unknown)");
            let mut diag = Diagnostic::error(format!(
                "derived signal `{name}` could not materialize native kernel{kernel} execution"
            ))
            .with_code(RUNTIME_INTERNAL)
            .with_note(
                "the signal was linked as natively executable, but the backend no longer produced a native plan"
                    .to_owned(),
            );
            if let Some(span) = source_map.item_span(*item) {
                diag = diag.with_primary_label(span, "signal declared here");
            }
            vec![diag]
        }

        BackendRuntimeError::MissingNativeReactiveSeedPlan {
            signal,
            item,
            kernel,
        } => {
            let name = source_map.signal_name(*signal).unwrap_or("(unknown)");
            let mut diag = Diagnostic::error(format!(
                "reactive seed for `{name}` could not materialize native kernel{kernel} execution"
            ))
            .with_code(RUNTIME_INTERNAL)
            .with_note(
                "the reactive seed was linked as natively executable, but the backend no longer produced a native plan"
                    .to_owned(),
            );
            if let Some(span) = source_map.item_span(*item) {
                diag = diag.with_primary_label(span, "reactive signal declared here");
            }
            vec![diag]
        }

        BackendRuntimeError::MissingNativeReactiveGuardPlan {
            signal,
            clause,
            item,
            kernel,
        } => {
            let name = source_map.signal_name(*signal).unwrap_or("(unknown)");
            let mut diag = Diagnostic::error(format!(
                "reactive guard {:?} for `{name}` could not materialize native kernel{kernel} execution",
                clause
            ))
            .with_code(RUNTIME_INTERNAL)
            .with_note(
                "the reactive guard was linked as natively executable, but the backend no longer produced a native plan"
                    .to_owned(),
            );
            if let Some(span) = source_map.item_span(*item) {
                diag = diag.with_primary_label(span, "reactive signal declared here");
            }
            vec![diag]
        }

        BackendRuntimeError::MissingNativeReactiveBodyPlan {
            signal,
            clause,
            item,
            kernel,
        } => {
            let name = source_map.signal_name(*signal).unwrap_or("(unknown)");
            let mut diag = Diagnostic::error(format!(
                "reactive body {:?} for `{name}` could not materialize native kernel{kernel} execution",
                clause
            ))
            .with_code(RUNTIME_INTERNAL)
            .with_note(
                "the reactive body was linked as natively executable, but the backend no longer produced a native plan"
                    .to_owned(),
            );
            if let Some(span) = source_map.item_span(*item) {
                diag = diag.with_primary_label(span, "reactive signal declared here");
            }
            vec![diag]
        }

        BackendRuntimeError::InvalidTemporalDelayDuration {
            signal,
            item,
            pipeline,
            stage_index,
            value,
        } => {
            let name = source_map.derived_name(*signal).unwrap_or("(unknown)");
            let mut diag = Diagnostic::error(format!(
                "derived signal `{name}` produced an invalid delay duration: {value:?}"
            ))
            .with_code(SIGNAL_EVAL_FAILED)
            .with_note(format!(
                "pipe stage {:?}/{} must evaluate to a positive Duration value such as `200ms`",
                pipeline, stage_index
            ));
            if let Some(span) = source_map.item_span(*item) {
                diag = diag.with_primary_label(span, "delay stage is configured here");
            }
            vec![diag]
        }

        BackendRuntimeError::InvalidTemporalBurstInterval {
            signal,
            item,
            pipeline,
            stage_index,
            value,
        } => {
            let name = source_map.derived_name(*signal).unwrap_or("(unknown)");
            let mut diag = Diagnostic::error(format!(
                "derived signal `{name}` produced an invalid burst interval: {value:?}"
            ))
            .with_code(SIGNAL_EVAL_FAILED)
            .with_note(format!(
                "pipe stage {:?}/{} must evaluate to a positive Duration value such as `200ms`",
                pipeline, stage_index
            ));
            if let Some(span) = source_map.item_span(*item) {
                diag = diag.with_primary_label(span, "burst stage is configured here");
            }
            vec![diag]
        }

        BackendRuntimeError::InvalidTemporalBurstCount {
            signal,
            item,
            pipeline,
            stage_index,
            value,
        } => {
            let name = source_map.derived_name(*signal).unwrap_or("(unknown)");
            let mut diag = Diagnostic::error(format!(
                "derived signal `{name}` produced an invalid burst count: {value:?}"
            ))
            .with_code(SIGNAL_EVAL_FAILED)
            .with_note(format!(
                "pipe stage {:?}/{} must evaluate to a positive burst count such as `3times`",
                pipeline, stage_index
            ));
            if let Some(span) = source_map.item_span(*item) {
                diag = diag.with_primary_label(span, "burst stage is configured here");
            }
            vec![diag]
        }

        // Internal/infrastructure errors — render with whatever info we have.
        BackendRuntimeError::MissingTemporalHelper {
            signal,
            item,
            pipeline,
            stage_index,
        } => {
            let name = source_map.derived_name(*signal).unwrap_or("(unknown)");
            let mut diag = Diagnostic::error(format!(
                "derived signal `{name}` is missing runtime state for temporal stage {:?}/{}",
                pipeline, stage_index
            ))
            .with_code(RUNTIME_INTERNAL)
            .with_note("this is likely a compiler/runtime linking bug".to_owned());
            if let Some(span) = source_map.item_span(*item) {
                diag = diag.with_primary_label(span, "signal declared here");
            }
            vec![diag]
        }
        BackendRuntimeError::SpawnTemporalWorker { message, .. } => {
            vec![
                Diagnostic::error(format!("failed to spawn temporal worker thread: {message}"))
                    .with_code(RUNTIME_INTERNAL),
            ]
        }
        BackendRuntimeError::UnknownDerivedSignal { signal } => {
            let name = source_map.derived_name(*signal).unwrap_or("(unknown)");
            vec![
                Diagnostic::error(format!(
                    "unknown derived signal `{name}` (internal ID: {:?})",
                    signal
                ))
                .with_code(RUNTIME_INTERNAL),
            ]
        }
        BackendRuntimeError::UnknownReactiveSignal { signal } => {
            let name = source_map.signal_name(*signal).unwrap_or("(unknown)");
            vec![
                Diagnostic::error(format!(
                    "unknown reactive signal `{name}` (internal ID: {:?})",
                    signal
                ))
                .with_code(RUNTIME_INTERNAL),
            ]
        }
        BackendRuntimeError::UnknownReactiveClause { signal, clause } => {
            let name = source_map.signal_name(*signal).unwrap_or("(unknown)");
            vec![
                Diagnostic::error(format!(
                    "unknown reactive clause {:?} for signal `{name}`",
                    clause
                ))
                .with_code(RUNTIME_INTERNAL),
            ]
        }
        BackendRuntimeError::UnknownSourceInstance { instance } => {
            let name = source_map.source_name(*instance).unwrap_or("(unknown)");
            vec![
                Diagnostic::error(format!(
                    "unknown source instance `{name}` (ID: {})",
                    instance.as_raw()
                ))
                .with_code(RUNTIME_INTERNAL),
            ]
        }
        BackendRuntimeError::UnknownTaskInstance { instance } => {
            vec![
                Diagnostic::error(format!("unknown task instance (ID: {})", instance.as_raw()))
                    .with_code(RUNTIME_INTERNAL),
            ]
        }
        BackendRuntimeError::UnknownTaskOwner { owner } => {
            let name = source_map.item_name(*owner).unwrap_or("(unknown)");
            vec![
                Diagnostic::error(format!("unknown task owner `{name}`"))
                    .with_code(RUNTIME_INTERNAL),
            ]
        }

        BackendRuntimeError::MissingCommittedSignalSnapshot {
            instance, signal, ..
        } => {
            let source = source_map.source_name(*instance).unwrap_or("(unknown)");
            let sig_name = source_map.signal_name(*signal).unwrap_or("(unknown)");
            vec![Diagnostic::error(format!(
                "source `{source}` requires snapshot for signal `{sig_name}` which is not yet committed"
            ))
            .with_code(RUNTIME_INTERNAL)
            .with_help("this may indicate a dependency ordering issue in the signal graph".to_owned())]
        }

        BackendRuntimeError::MissingSignalItemMapping { instance, item, .. } => {
            let source = source_map.source_name(*instance).unwrap_or("(unknown)");
            vec![
                Diagnostic::error(format!(
                    "source `{source}` could not map backend item {item} to a runtime signal"
                ))
                .with_code(RUNTIME_INTERNAL),
            ]
        }

        BackendRuntimeError::MissingCommittedTaskSignalSnapshot {
            instance, signal, ..
        } => {
            let sig_name = source_map.signal_name(*signal).unwrap_or("(unknown)");
            vec![Diagnostic::error(format!(
                "task instance {} requires snapshot for signal `{sig_name}` which is not yet committed",
                instance.as_raw()
            ))
            .with_code(RUNTIME_INTERNAL)]
        }

        BackendRuntimeError::MissingTaskSignalItemMapping { instance, item, .. } => {
            vec![
                Diagnostic::error(format!(
                    "task instance {} could not map backend item {item} to a runtime signal",
                    instance.as_raw()
                ))
                .with_code(RUNTIME_INTERNAL),
            ]
        }

        BackendRuntimeError::TaskExecutionBlocked { owner, blocker, .. } => {
            let name = source_map.item_name(*owner).unwrap_or("(unknown task)");
            let mut diag =
                Diagnostic::error(format!("task `{name}` execution is blocked: {blocker}"))
                    .with_code(TASK_EVAL_FAILED);

            if let Some(span) = source_map.item_span(*owner) {
                diag = diag.with_primary_label(span, "this task");
            }

            vec![diag]
        }

        BackendRuntimeError::SpawnTaskWorker { message, .. } => {
            vec![
                Diagnostic::error(format!("failed to spawn task worker thread: {message}"))
                    .with_code(RUNTIME_INTERNAL),
            ]
        }

        BackendRuntimeError::Runtime(error) => {
            vec![
                Diagnostic::error(format!("runtime scheduler error: {error:?}"))
                    .with_code(RUNTIME_INTERNAL),
            ]
        }
    }
}

/// Convert a [`SourceProviderExecutionError`] into one or more [`Diagnostic`]s.
pub fn render_provider_execution_error(
    error: &SourceProviderExecutionError,
    source_map: &RuntimeSourceMap,
) -> Vec<Diagnostic> {
    let (instance, provider_key, message, help) = match error {
        SourceProviderExecutionError::MissingDecodeProgram { instance, provider } => (
            *instance,
            provider.key(),
            format!("source provider `{}` is missing its runtime decode program", provider.key()),
            Some("this is likely a compiler/runtime wiring bug rather than an application error".to_owned()),
        ),
        SourceProviderExecutionError::UnsupportedDecodeProgram {
            instance,
            provider,
            detail,
        } => (
            *instance,
            provider.key(),
            format!(
                "source provider `{}` cannot execute its runtime decode program: {detail}",
                provider.key()
            ),
            Some(
                "simplify the source payload annotation or runtime shape until this provider path is implemented"
                    .to_owned(),
            ),
        ),
        SourceProviderExecutionError::UnsupportedProviderShape {
            instance,
            provider,
            detail,
        } => (
            *instance,
            provider.key(),
            format!(
                "source provider `{}` cannot execute this source shape: {detail}",
                provider.key()
            ),
            Some("adjust the @source declaration to a supported provider shape".to_owned()),
        ),
        SourceProviderExecutionError::InvalidArgumentCount {
            instance,
            provider,
            expected,
            found,
        } => (
            *instance,
            provider.key(),
            format!(
                "source provider `{}` expected {expected} argument(s), found {found}",
                provider.key()
            ),
            Some("check the @source arguments against the provider contract".to_owned()),
        ),
        SourceProviderExecutionError::InvalidArgument {
            instance,
            provider,
            index,
            expected,
            value,
        } => (
            *instance,
            provider.key(),
            format!(
                "source provider `{}` received invalid argument {index}: expected {expected}, found {value}",
                provider.key()
            ),
            Some("adjust the argument expression so it matches the provider's expected runtime type".to_owned()),
        ),
        SourceProviderExecutionError::InvalidOption {
            instance,
            provider,
            option_name,
            expected,
            value,
        } => (
            *instance,
            provider.key(),
            format!(
                "source provider `{}` received invalid `{option_name}` option: expected {expected}, found {value}",
                provider.key()
            ),
            Some("change the option value to the provider's supported shape".to_owned()),
        ),
        SourceProviderExecutionError::UnsupportedOption {
            instance,
            provider,
            option_name,
        } => (
            *instance,
            provider.key(),
            format!(
                "source provider `{}` does not support the `{option_name}` option at runtime yet",
                provider.key()
            ),
            Some("remove that option or switch to a provider/runtime path that supports it".to_owned()),
        ),
        SourceProviderExecutionError::ZeroTimerInterval { instance } => (
            *instance,
            "timer",
            "timer sources must use a strictly positive duration".to_owned(),
            Some("replace `0ms` or a negative duration with a positive interval such as `100ms`".to_owned()),
        ),
        SourceProviderExecutionError::StartFailed {
            instance,
            provider,
            detail,
        } => (
            *instance,
            provider.key(),
            format!("source provider `{}` failed to start: {detail}", provider.key()),
            None,
        ),
    };

    let source_name = source_map
        .source_name(instance)
        .unwrap_or("(unknown source)");
    let mut diag = Diagnostic::error(format!(
        "source `{source_name}` failed during provider startup: {message}"
    ))
    .with_code(SOURCE_PROVIDER_FAILED)
    .with_note(format!("provider key: `{provider_key}`"));

    if let Some(span) = source_map.source_span(instance) {
        diag = diag.with_primary_label(span, "source declared here");
    }

    if let Some(help) = help {
        diag = diag.with_help(help);
    }

    vec![diag]
}

/// Convert a structured source-boundary decode failure into one or more [`Diagnostic`]s.
pub fn render_source_decode_error(
    instance: crate::SourceInstanceId,
    provider: aivi_typing::BuiltinSourceProvider,
    error: &crate::SourceDecodeErrorWithPath,
    source_map: &RuntimeSourceMap,
) -> Vec<Diagnostic> {
    let source_name = source_map
        .source_name(instance)
        .unwrap_or("(unknown source)");
    let mut diag = match &error.error {
        crate::SourceDecodeError::InvalidJson { detail } => Diagnostic::error(format!(
            "source `{source_name}` received malformed JSON at the app boundary: {detail}"
        )),
        crate::SourceDecodeError::UnsupportedNumber { value } => Diagnostic::error(format!(
            "source `{source_name}` received unsupported JSON number `{value}`"
        )),
        crate::SourceDecodeError::InvalidScalarLiteral { scalar, value } => Diagnostic::error(
            format!(
                "source `{source_name}` could not decode {scalar} literal from external value `{value}`"
            ),
        ),
        crate::SourceDecodeError::InvalidBytesElementKind { index, found } => Diagnostic::error(
            format!(
                "source `{source_name}` expected byte array element {index} to be an integer, found {found}"
            ),
        ),
        crate::SourceDecodeError::InvalidByteValue { index, value } => Diagnostic::error(format!(
            "source `{source_name}` expected byte array element {index} to be within 0..=255, found {value}"
        )),
        crate::SourceDecodeError::TypeMismatch { expected, found } => Diagnostic::error(format!(
            "source `{source_name}` expected {expected} at `{}`, found {found}",
            error.path_string()
        )),
        crate::SourceDecodeError::InvalidTupleLength { expected, found } => Diagnostic::error(
            format!(
                "source `{source_name}` expected tuple/list length {expected} at `{}`, found {found}",
                error.path_string()
            ),
        ),
        crate::SourceDecodeError::MissingField { field } => Diagnostic::error(format!(
            "source `{source_name}` is missing required field `{field}` at `{}`",
            error.path_string()
        )),
        crate::SourceDecodeError::UnexpectedFields { fields } => Diagnostic::error(format!(
            "source `{source_name}` received unexpected field(s) {} at `{}`",
            fields.join(", "),
            error.path_string()
        )),
        crate::SourceDecodeError::UnknownVariant { found, expected } => Diagnostic::error(format!(
            "source `{source_name}` received unknown variant `{found}` at `{}`; expected one of {}",
            error.path_string(),
            expected.join(", ")
        )),
        crate::SourceDecodeError::MissingVariantPayload { variant } => Diagnostic::error(format!(
            "source `{source_name}` expected variant `{variant}` to carry a payload at `{}`",
            error.path_string()
        )),
        crate::SourceDecodeError::UnexpectedVariantPayload { variant } => Diagnostic::error(
            format!(
                "source `{source_name}` expected variant `{variant}` to be payloadless at `{}`",
                error.path_string()
            ),
        ),
        crate::SourceDecodeError::UnsupportedProgram(detail) => Diagnostic::error(format!(
            "source `{source_name}` uses a decode shape that runtime source decoding cannot execute yet: {detail}"
        )),
    }
    .with_code(SOURCE_DECODE_FAILED)
    .with_note(format!("provider key: `{}`", provider.key()))
    .with_note(format!("decode path: {}", error.path_string()));

    if let Some(span) = source_map.source_span(instance) {
        diag = diag.with_primary_label(span, "source payload expectation declared here");
    }

    if let Some(actual) = &error.actual {
        diag = diag.with_note(format!("actual {}: {}", actual.kind, actual.preview));
    }

    diag = match &error.error {
        crate::SourceDecodeError::InvalidJson { .. } => diag.with_help(
            "ensure the external payload is valid JSON and matches the declared @source output type",
        ),
        crate::SourceDecodeError::TypeMismatch { .. }
        | crate::SourceDecodeError::InvalidTupleLength { .. }
        | crate::SourceDecodeError::MissingField { .. }
        | crate::SourceDecodeError::UnexpectedFields { .. }
        | crate::SourceDecodeError::UnknownVariant { .. }
        | crate::SourceDecodeError::MissingVariantPayload { .. }
        | crate::SourceDecodeError::UnexpectedVariantPayload { .. } => diag.with_help(
            "adjust the incoming payload shape or relax the @source output annotation so they agree",
        ),
        crate::SourceDecodeError::InvalidScalarLiteral { .. }
        | crate::SourceDecodeError::InvalidBytesElementKind { .. }
        | crate::SourceDecodeError::InvalidByteValue { .. }
        | crate::SourceDecodeError::UnsupportedNumber { .. } => diag.with_help(
            "normalize the boundary value into the exact scalar form that the @source annotation expects",
        ),
        crate::SourceDecodeError::UnsupportedProgram(_) => diag.with_help(
            "simplify the @source payload type or extend runtime decoding support for this shape",
        ),
    };

    vec![diag]
}
