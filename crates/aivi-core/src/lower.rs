use std::collections::{BTreeMap, HashMap};

use aivi_base::SourceSpan;
use aivi_hir::{
    BlockedFanoutSegment, BlockedGateStage, BlockedRecurrenceNode, BlockedSourceDecodeProgram,
    BlockedSourceLifecycleNode, BlockedTruthyFalsyStage, Expr as HirExpr, ExprId as HirExprId,
    ExprKind as HirExprKind, GateRuntimeExpr, GateRuntimeExprKind, GateRuntimeMapEntry,
    GateRuntimePipeExpr, GateRuntimePipeStageKind, GateRuntimeProjectionBase, GateRuntimeReference,
    GateRuntimeTextLiteral, GateRuntimeTextSegment, GateStageOutcome, Item as HirItem,
    ItemId as HirItemId, RecurrenceNodeOutcome, SourceDecodeProgram, SourceDecodeProgramOutcome,
    SourceLifecycleNodeOutcome, TruthyFalsyStageOutcome, elaborate_fanouts, elaborate_gates,
    elaborate_recurrences, elaborate_source_lifecycles, elaborate_truthy_falsy,
    generate_source_decode_programs,
};

use crate::{
    Arena, ArenaOverflow, DecodeField, DecodeProgram, DecodeProgramId, DecodeStep, DecodeStepId,
    DomainDecodeSurface, DomainDecodeSurfaceKind, Expr, ExprId, FanoutJoin, FanoutStage, GateStage,
    Item, ItemId, ItemKind, MapEntry, Module, NonSourceWakeup, Pipe, PipeOrigin, PipeRecurrence,
    PipeStage, ProjectionBase, RecordExprField, RecurrenceStage, Reference, SignalInfo, SourceId,
    SourceInstanceId, SourceNode, SourceOptionBinding, Stage, StageId, StageKind, TextLiteral,
    TextSegment, TruthyFalsyBranch, TruthyFalsyStage, Type,
    expr::{ExprKind, PipeExpr},
    validate::{ValidationError, validate_module},
};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LoweringErrors {
    errors: Vec<LoweringError>,
}

impl LoweringErrors {
    pub fn new(errors: Vec<LoweringError>) -> Self {
        Self { errors }
    }

    pub fn errors(&self) -> &[LoweringError] {
        &self.errors
    }

    pub fn into_errors(self) -> Vec<LoweringError> {
        self.errors
    }

    pub fn is_empty(&self) -> bool {
        self.errors.is_empty()
    }
}

impl std::fmt::Display for LoweringErrors {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (index, error) in self.errors.iter().enumerate() {
            if index > 0 {
                f.write_str("; ")?;
            }
            write!(f, "{error}")?;
        }
        Ok(())
    }
}

impl std::error::Error for LoweringErrors {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoweringError {
    UnknownOwner {
        owner: HirItemId,
    },
    BlockedGateStage {
        owner: HirItemId,
        pipe_expr: HirExprId,
        stage_index: usize,
        span: SourceSpan,
        blocked: BlockedGateStage,
    },
    BlockedTruthyFalsyStage {
        owner: HirItemId,
        pipe_expr: HirExprId,
        truthy_stage_index: usize,
        falsy_stage_index: usize,
        span: SourceSpan,
        blocked: BlockedTruthyFalsyStage,
    },
    BlockedFanoutStage {
        owner: HirItemId,
        pipe_expr: HirExprId,
        map_stage_index: usize,
        span: SourceSpan,
        blocked: BlockedFanoutSegment,
    },
    BlockedRecurrence {
        owner: HirItemId,
        pipe_expr: HirExprId,
        start_stage_index: usize,
        span: SourceSpan,
        blocked: BlockedRecurrenceNode,
    },
    BlockedSourceLifecycle {
        owner: HirItemId,
        span: SourceSpan,
        blocked: BlockedSourceLifecycleNode,
    },
    BlockedDecodeProgram {
        owner: HirItemId,
        span: SourceSpan,
        blocked: BlockedSourceDecodeProgram,
    },
    DuplicatePipeStage {
        owner: HirItemId,
        pipe_expr: HirExprId,
        stage_index: usize,
    },
    DuplicatePipeRecurrence {
        owner: HirItemId,
        pipe_expr: HirExprId,
    },
    DuplicateSourceOwner {
        owner: HirItemId,
    },
    DuplicateDecodeOwner {
        owner: HirItemId,
    },
    MissingSourceForDecode {
        owner: HirItemId,
    },
    DependencyOutsideCore {
        owner: HirItemId,
        dependency: HirItemId,
    },
    ArenaOverflow {
        arena: &'static str,
        attempted_len: usize,
    },
    Validation(ValidationError),
}

impl std::fmt::Display for LoweringError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownOwner { owner } => {
                write!(f, "typed-core lowering cannot find owner item {owner}")
            }
            Self::BlockedGateStage {
                owner,
                stage_index,
                blocked,
                ..
            } => write!(
                f,
                "typed-core lowering blocked on gate stage {stage_index} for item {owner}: {blocked:?}"
            ),
            Self::BlockedTruthyFalsyStage {
                owner,
                truthy_stage_index,
                falsy_stage_index,
                blocked,
                ..
            } => write!(
                f,
                "typed-core lowering blocked on truthy/falsy pair {truthy_stage_index}/{falsy_stage_index} for item {owner}: {blocked:?}"
            ),
            Self::BlockedFanoutStage {
                owner,
                map_stage_index,
                blocked,
                ..
            } => write!(
                f,
                "typed-core lowering blocked on fanout stage {map_stage_index} for item {owner}: {blocked:?}"
            ),
            Self::BlockedRecurrence {
                owner,
                start_stage_index,
                blocked,
                ..
            } => write!(
                f,
                "typed-core lowering blocked on recurrence stage {start_stage_index} for item {owner}: {blocked:?}"
            ),
            Self::BlockedSourceLifecycle { owner, blocked, .. } => write!(
                f,
                "typed-core lowering blocked on source lifecycle for item {owner}: {blocked:?}"
            ),
            Self::BlockedDecodeProgram { owner, blocked, .. } => write!(
                f,
                "typed-core lowering blocked on decode program for item {owner}: {blocked:?}"
            ),
            Self::DuplicatePipeStage {
                owner,
                pipe_expr,
                stage_index,
            } => write!(
                f,
                "typed-core lowering saw duplicate stage {stage_index} for pipe {pipe_expr} owned by item {owner}"
            ),
            Self::DuplicatePipeRecurrence { owner, pipe_expr } => write!(
                f,
                "typed-core lowering saw duplicate recurrence attachment for pipe {pipe_expr} owned by item {owner}"
            ),
            Self::DuplicateSourceOwner { owner } => {
                write!(
                    f,
                    "typed-core lowering saw more than one source for item {owner}"
                )
            }
            Self::DuplicateDecodeOwner { owner } => {
                write!(
                    f,
                    "typed-core lowering saw more than one decode program for item {owner}"
                )
            }
            Self::MissingSourceForDecode { owner } => write!(
                f,
                "typed-core lowering cannot attach decode program because item {owner} has no lowered source node"
            ),
            Self::DependencyOutsideCore { owner, dependency } => write!(
                f,
                "typed-core lowering cannot map dependency {dependency} owned by item {owner} into the current core slice"
            ),
            Self::ArenaOverflow {
                arena,
                attempted_len,
            } => write!(
                f,
                "typed-core {arena} arena overflowed after {attempted_len} entries"
            ),
            Self::Validation(error) => write!(f, "typed-core validation failed: {error}"),
        }
    }
}

pub fn lower_module(hir: &aivi_hir::Module) -> Result<Module, LoweringErrors> {
    ModuleLowerer::new(hir).build()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct PipeKey {
    owner: HirItemId,
    pipe_expr: HirExprId,
}

struct PipeBuilder {
    owner: ItemId,
    origin: PipeOrigin,
    stages: BTreeMap<usize, PendingStage>,
    recurrence: Option<PipeRecurrence>,
}

enum PendingStage {
    Lowered {
        span: SourceSpan,
        input_subject: Type,
        result_subject: Type,
        kind: StageKind,
    },
}

struct ModuleLowerer<'a> {
    hir: &'a aivi_hir::Module,
    module: Module,
    item_map: HashMap<HirItemId, ItemId>,
    pipe_builders: BTreeMap<PipeKey, PipeBuilder>,
    source_by_owner: HashMap<ItemId, SourceId>,
    decode_by_owner: HashMap<ItemId, DecodeProgramId>,
    errors: Vec<LoweringError>,
}

impl<'a> ModuleLowerer<'a> {
    fn new(hir: &'a aivi_hir::Module) -> Self {
        Self {
            hir,
            module: Module::new(),
            item_map: HashMap::new(),
            pipe_builders: BTreeMap::new(),
            source_by_owner: HashMap::new(),
            decode_by_owner: HashMap::new(),
            errors: Vec::new(),
        }
    }

    fn build(mut self) -> Result<Module, LoweringErrors> {
        self.seed_items()?;
        self.seed_signal_dependencies();
        self.lower_gate_stages();
        self.lower_truthy_falsy_stages();
        self.lower_fanout_stages();
        self.lower_recurrences();
        self.finalize_pipes()?;
        self.lower_sources()?;
        self.lower_decode_programs()?;

        if !self.errors.is_empty() {
            return Err(LoweringErrors::new(self.errors));
        }

        if let Err(validation) = validate_module(&self.module) {
            self.errors.extend(
                validation
                    .into_errors()
                    .into_iter()
                    .map(LoweringError::Validation),
            );
            return Err(LoweringErrors::new(self.errors));
        }

        Ok(self.module)
    }

    fn seed_items(&mut self) -> Result<(), LoweringErrors> {
        for (hir_id, item) in self.hir.items().iter() {
            let (span, name, kind) = match item {
                HirItem::Value(item) => {
                    (item.header.span, item.name.text().into(), ItemKind::Value)
                }
                HirItem::Function(item) => (
                    item.header.span,
                    item.name.text().into(),
                    ItemKind::Function,
                ),
                HirItem::Signal(item) => (
                    item.header.span,
                    item.name.text().into(),
                    ItemKind::Signal(SignalInfo::default()),
                ),
                HirItem::Type(_)
                | HirItem::Class(_)
                | HirItem::Domain(_)
                | HirItem::SourceProviderContract(_)
                | HirItem::Instance(_)
                | HirItem::Use(_)
                | HirItem::Export(_) => continue,
            };
            let item_id = self
                .module
                .items_mut()
                .alloc(Item {
                    origin: hir_id,
                    span,
                    name,
                    kind,
                    pipes: Vec::new(),
                })
                .map_err(|overflow| LoweringErrors::new(vec![arena_overflow("items", overflow)]))?;
            self.item_map.insert(hir_id, item_id);
        }
        Ok(())
    }

    fn seed_signal_dependencies(&mut self) {
        for (hir_id, item) in self.hir.items().iter() {
            let HirItem::Signal(signal) = item else {
                continue;
            };
            let Some(item_id) = self.item_map.get(&hir_id).copied() else {
                self.errors
                    .push(LoweringError::UnknownOwner { owner: hir_id });
                continue;
            };
            let dependencies = signal
                .signal_dependencies
                .iter()
                .filter_map(|dependency| self.map_dependency(hir_id, *dependency))
                .collect::<Vec<_>>();
            let Some(item) = self.module.items_mut().get_mut(item_id) else {
                self.errors
                    .push(LoweringError::UnknownOwner { owner: hir_id });
                continue;
            };
            let ItemKind::Signal(info) = &mut item.kind else {
                continue;
            };
            let mut dependencies = dependencies;
            dependencies.sort();
            dependencies.dedup();
            info.dependencies = dependencies;
        }
    }

    fn lower_gate_stages(&mut self) {
        for stage in elaborate_gates(self.hir).into_stages() {
            let key = PipeKey {
                owner: stage.owner,
                pipe_expr: stage.pipe_expr,
            };
            let builder = match self.pipe_builder(key) {
                Some(builder) => builder,
                None => continue,
            };
            let lowered = match stage.outcome {
                GateStageOutcome::Ordinary(plan) => {
                    let input_subject = Type::lower(&plan.input_subject);
                    let result_subject = Type::lower(&plan.result_type);
                    let ambient = match self.alloc_expr(
                        stage.owner,
                        stage.stage_span,
                        Expr {
                            span: stage.stage_span,
                            ty: input_subject.clone(),
                            kind: ExprKind::AmbientSubject,
                        },
                    ) {
                        Ok(id) => id,
                        Err(error) => {
                            self.errors.push(error);
                            continue;
                        }
                    };
                    let when_true = match self.alloc_expr(
                        stage.owner,
                        stage.stage_span,
                        Expr {
                            span: stage.stage_span,
                            ty: result_subject.clone(),
                            kind: ExprKind::OptionSome { payload: ambient },
                        },
                    ) {
                        Ok(id) => id,
                        Err(error) => {
                            self.errors.push(error);
                            continue;
                        }
                    };
                    let when_false = match self.alloc_expr(
                        stage.owner,
                        stage.stage_span,
                        Expr {
                            span: stage.stage_span,
                            ty: result_subject.clone(),
                            kind: ExprKind::OptionNone,
                        },
                    ) {
                        Ok(id) => id,
                        Err(error) => {
                            self.errors.push(error);
                            continue;
                        }
                    };
                    PendingStage::Lowered {
                        span: stage.stage_span,
                        input_subject,
                        result_subject,
                        kind: StageKind::Gate(GateStage::Ordinary {
                            when_true,
                            when_false,
                        }),
                    }
                }
                GateStageOutcome::SignalFilter(plan) => {
                    let predicate =
                        match self.lower_runtime_expr(stage.owner, &plan.runtime_predicate) {
                            Ok(expr) => expr,
                            Err(error) => {
                                self.errors.push(error);
                                continue;
                            }
                        };
                    PendingStage::Lowered {
                        span: stage.stage_span,
                        input_subject: Type::lower(&plan.input_subject),
                        result_subject: Type::lower(&plan.result_type),
                        kind: StageKind::Gate(GateStage::SignalFilter {
                            payload_type: Type::lower(&plan.payload_type),
                            predicate,
                            emits_negative_update: plan.emits_negative_update,
                        }),
                    }
                }
                GateStageOutcome::Blocked(blocked) => {
                    self.errors.push(LoweringError::BlockedGateStage {
                        owner: stage.owner,
                        pipe_expr: stage.pipe_expr,
                        stage_index: stage.stage_index,
                        span: stage.stage_span,
                        blocked,
                    });
                    continue;
                }
            };
            if builder.stages.insert(stage.stage_index, lowered).is_some() {
                self.errors.push(LoweringError::DuplicatePipeStage {
                    owner: stage.owner,
                    pipe_expr: stage.pipe_expr,
                    stage_index: stage.stage_index,
                });
            }
        }
    }

    fn lower_truthy_falsy_stages(&mut self) {
        for stage in elaborate_truthy_falsy(self.hir).into_stages() {
            let key = PipeKey {
                owner: stage.owner,
                pipe_expr: stage.pipe_expr,
            };
            let builder = match self.pipe_builder(key) {
                Some(builder) => builder,
                None => continue,
            };
            let outcome = match stage.outcome {
                TruthyFalsyStageOutcome::Planned(plan) => {
                    let span = join_spans(stage.truthy_stage_span, stage.falsy_stage_span);
                    PendingStage::Lowered {
                        span,
                        input_subject: Type::lower(&plan.input_subject),
                        result_subject: Type::lower(&plan.result_type),
                        kind: StageKind::TruthyFalsy(TruthyFalsyStage {
                            truthy_stage_index: stage.truthy_stage_index,
                            truthy_stage_span: stage.truthy_stage_span,
                            falsy_stage_index: stage.falsy_stage_index,
                            falsy_stage_span: stage.falsy_stage_span,
                            truthy: TruthyFalsyBranch {
                                constructor: plan.truthy.constructor,
                                payload_subject: plan
                                    .truthy
                                    .payload_subject
                                    .as_ref()
                                    .map(Type::lower),
                                result_type: Type::lower(&plan.truthy.result_type),
                                origin_expr: plan.truthy.expr,
                            },
                            falsy: TruthyFalsyBranch {
                                constructor: plan.falsy.constructor,
                                payload_subject: plan
                                    .falsy
                                    .payload_subject
                                    .as_ref()
                                    .map(Type::lower),
                                result_type: Type::lower(&plan.falsy.result_type),
                                origin_expr: plan.falsy.expr,
                            },
                        }),
                    }
                }
                TruthyFalsyStageOutcome::Blocked(blocked) => {
                    self.errors.push(LoweringError::BlockedTruthyFalsyStage {
                        owner: stage.owner,
                        pipe_expr: stage.pipe_expr,
                        truthy_stage_index: stage.truthy_stage_index,
                        falsy_stage_index: stage.falsy_stage_index,
                        span: join_spans(stage.truthy_stage_span, stage.falsy_stage_span),
                        blocked,
                    });
                    continue;
                }
            };
            if builder
                .stages
                .insert(stage.truthy_stage_index, outcome)
                .is_some()
            {
                self.errors.push(LoweringError::DuplicatePipeStage {
                    owner: stage.owner,
                    pipe_expr: stage.pipe_expr,
                    stage_index: stage.truthy_stage_index,
                });
            }
        }
    }

    fn lower_fanout_stages(&mut self) {
        for segment in elaborate_fanouts(self.hir).into_segments() {
            let key = PipeKey {
                owner: segment.owner,
                pipe_expr: segment.pipe_expr,
            };
            let builder = match self.pipe_builder(key) {
                Some(builder) => builder,
                None => continue,
            };
            let outcome = match segment.outcome {
                aivi_hir::FanoutSegmentOutcome::Planned(plan) => {
                    let span = plan
                        .join
                        .as_ref()
                        .map(|join| join_spans(segment.map_stage_span, join.stage_span))
                        .unwrap_or(segment.map_stage_span);
                    PendingStage::Lowered {
                        span,
                        input_subject: Type::lower(&plan.input_subject),
                        result_subject: Type::lower(&plan.result_type),
                        kind: StageKind::Fanout(FanoutStage {
                            carrier: plan.carrier,
                            element_subject: Type::lower(&plan.element_subject),
                            mapped_element_type: Type::lower(&plan.mapped_element_type),
                            mapped_collection_type: Type::lower(&plan.mapped_collection_type),
                            join: plan.join.map(|join| FanoutJoin {
                                stage_index: join.stage_index,
                                stage_span: join.stage_span,
                                origin_expr: join.expr,
                                input_subject: Type::lower(&join.input_subject),
                                collection_subject: Type::lower(&join.collection_subject),
                                result_type: Type::lower(&join.result_type),
                            }),
                        }),
                    }
                }
                aivi_hir::FanoutSegmentOutcome::Blocked(blocked) => {
                    self.errors.push(LoweringError::BlockedFanoutStage {
                        owner: segment.owner,
                        pipe_expr: segment.pipe_expr,
                        map_stage_index: segment.map_stage_index,
                        span: segment.map_stage_span,
                        blocked,
                    });
                    continue;
                }
            };
            if builder
                .stages
                .insert(segment.map_stage_index, outcome)
                .is_some()
            {
                self.errors.push(LoweringError::DuplicatePipeStage {
                    owner: segment.owner,
                    pipe_expr: segment.pipe_expr,
                    stage_index: segment.map_stage_index,
                });
            }
        }
    }

    fn lower_recurrences(&mut self) {
        for node in elaborate_recurrences(self.hir).into_nodes() {
            let key = PipeKey {
                owner: node.owner,
                pipe_expr: node.pipe_expr,
            };
            let builder = match self.pipe_builder(key) {
                Some(builder) => builder,
                None => continue,
            };
            let recurrence = match node.outcome {
                RecurrenceNodeOutcome::Planned(plan) => {
                    let start = match self.lower_recurrence_stage(node.owner, &plan.start) {
                        Ok(stage) => stage,
                        Err(error) => {
                            self.errors.push(error);
                            continue;
                        }
                    };
                    let mut steps = Vec::with_capacity(plan.steps.len());
                    let mut failed = false;
                    for step in &plan.steps {
                        match self.lower_recurrence_stage(node.owner, step) {
                            Ok(stage) => steps.push(stage),
                            Err(error) => {
                                self.errors.push(error);
                                failed = true;
                                break;
                            }
                        }
                    }
                    if failed {
                        continue;
                    }
                    let non_source_wakeup = match plan.non_source_wakeup {
                        Some(binding) => {
                            match self.lower_runtime_expr(node.owner, &binding.runtime_witness) {
                                Ok(runtime_witness) => Some(NonSourceWakeup {
                                    cause: binding.cause,
                                    witness_expr: binding.witness,
                                    runtime_witness,
                                }),
                                Err(error) => {
                                    self.errors.push(error);
                                    continue;
                                }
                            }
                        }
                        None => None,
                    };
                    PipeRecurrence {
                        target: plan.target,
                        wakeup: plan.wakeup,
                        start,
                        steps,
                        non_source_wakeup,
                    }
                }
                RecurrenceNodeOutcome::Blocked(blocked) => {
                    self.errors.push(LoweringError::BlockedRecurrence {
                        owner: node.owner,
                        pipe_expr: node.pipe_expr,
                        start_stage_index: node.start_stage_index,
                        span: node.start_stage_span,
                        blocked,
                    });
                    continue;
                }
            };
            if builder.recurrence.replace(recurrence).is_some() {
                self.errors.push(LoweringError::DuplicatePipeRecurrence {
                    owner: node.owner,
                    pipe_expr: node.pipe_expr,
                });
            }
        }
    }

    fn finalize_pipes(&mut self) -> Result<(), LoweringErrors> {
        let builders = std::mem::take(&mut self.pipe_builders);
        for (_, builder) in builders {
            let pipe_id = self
                .module
                .pipes_mut()
                .alloc(Pipe {
                    owner: builder.owner,
                    origin: builder.origin,
                    stages: Vec::new(),
                    recurrence: builder.recurrence,
                })
                .map_err(|overflow| LoweringErrors::new(vec![arena_overflow("pipes", overflow)]))?;
            let mut stage_ids = Vec::with_capacity(builder.stages.len());
            for (index, pending) in builder.stages {
                let PendingStage::Lowered {
                    span,
                    input_subject,
                    result_subject,
                    kind,
                } = pending;
                let stage_id = self
                    .module
                    .stages_mut()
                    .alloc(Stage {
                        pipe: pipe_id,
                        index,
                        span,
                        input_subject,
                        result_subject,
                        kind,
                    })
                    .map_err(|overflow| {
                        LoweringErrors::new(vec![arena_overflow("stages", overflow)])
                    })?;
                stage_ids.push(stage_id);
            }
            self.module
                .pipes_mut()
                .get_mut(pipe_id)
                .expect("pipe just allocated")
                .stages = stage_ids;
            self.module
                .items_mut()
                .get_mut(builder.owner)
                .expect("pipe owner should exist")
                .pipes
                .push(pipe_id);
        }
        Ok(())
    }

    fn lower_sources(&mut self) -> Result<(), LoweringErrors> {
        for node in elaborate_source_lifecycles(self.hir).into_nodes() {
            let Some(owner) = self.item_map.get(&node.owner).copied() else {
                self.errors
                    .push(LoweringError::UnknownOwner { owner: node.owner });
                continue;
            };
            let plan = match node.outcome {
                SourceLifecycleNodeOutcome::Planned(plan) => plan,
                SourceLifecycleNodeOutcome::Blocked(blocked) => {
                    self.errors.push(LoweringError::BlockedSourceLifecycle {
                        owner: node.owner,
                        span: node.source_span,
                        blocked,
                    });
                    continue;
                }
            };
            if self.source_by_owner.contains_key(&owner) {
                self.errors
                    .push(LoweringError::DuplicateSourceOwner { owner: node.owner });
                continue;
            }
            let reconfiguration_dependencies = plan
                .reconfiguration_dependencies
                .iter()
                .filter_map(|dependency| self.map_dependency(node.owner, *dependency))
                .collect::<Vec<_>>();
            let source_id = self
                .module
                .sources_mut()
                .alloc(SourceNode {
                    owner,
                    span: node.source_span,
                    instance: SourceInstanceId::from_raw(plan.instance.decorator().as_raw()),
                    provider: plan.provider,
                    teardown: plan.teardown,
                    replacement: plan.replacement,
                    reconfiguration_dependencies,
                    explicit_triggers: plan
                        .explicit_triggers
                        .into_iter()
                        .map(|binding| SourceOptionBinding {
                            option_span: binding.option_span,
                            option_name: binding.option_name.text().into(),
                            origin_expr: binding.expr,
                        })
                        .collect(),
                    active_when: plan.active_when.map(|binding| SourceOptionBinding {
                        option_span: binding.option_span,
                        option_name: binding.option_name.text().into(),
                        origin_expr: binding.expr,
                    }),
                    cancellation: plan.cancellation,
                    stale_work: plan.stale_work,
                    decode: None,
                })
                .map_err(|overflow| {
                    LoweringErrors::new(vec![arena_overflow("sources", overflow)])
                })?;
            self.source_by_owner.insert(owner, source_id);
            let Some(item) = self.module.items_mut().get_mut(owner) else {
                self.errors
                    .push(LoweringError::UnknownOwner { owner: node.owner });
                continue;
            };
            let ItemKind::Signal(info) = &mut item.kind else {
                self.errors
                    .push(LoweringError::UnknownOwner { owner: node.owner });
                continue;
            };
            info.source = Some(source_id);
        }
        Ok(())
    }

    fn lower_decode_programs(&mut self) -> Result<(), LoweringErrors> {
        for node in generate_source_decode_programs(self.hir).into_nodes() {
            let Some(owner) = self.item_map.get(&node.owner).copied() else {
                self.errors
                    .push(LoweringError::UnknownOwner { owner: node.owner });
                continue;
            };
            let Some(source_id) = self.source_by_owner.get(&owner).copied() else {
                match node.outcome {
                    SourceDecodeProgramOutcome::Planned(_) => {
                        self.errors
                            .push(LoweringError::MissingSourceForDecode { owner: node.owner });
                    }
                    SourceDecodeProgramOutcome::Blocked(blocked) => {
                        self.errors.push(LoweringError::BlockedDecodeProgram {
                            owner: node.owner,
                            span: node.source_span,
                            blocked,
                        });
                    }
                }
                continue;
            };
            if self.decode_by_owner.contains_key(&owner) {
                self.errors
                    .push(LoweringError::DuplicateDecodeOwner { owner: node.owner });
                continue;
            }
            let program = match node.outcome {
                SourceDecodeProgramOutcome::Planned(program) => {
                    match self.lower_decode_program(owner, &program) {
                        Ok(program) => program,
                        Err(error) => {
                            self.errors.push(error);
                            continue;
                        }
                    }
                }
                SourceDecodeProgramOutcome::Blocked(blocked) => {
                    self.errors.push(LoweringError::BlockedDecodeProgram {
                        owner: node.owner,
                        span: node.source_span,
                        blocked,
                    });
                    continue;
                }
            };
            let decode_id =
                self.module
                    .decode_programs_mut()
                    .alloc(program)
                    .map_err(|overflow| {
                        LoweringErrors::new(vec![arena_overflow("decode-programs", overflow)])
                    })?;
            self.decode_by_owner.insert(owner, decode_id);
            self.module
                .sources_mut()
                .get_mut(source_id)
                .expect("source should exist when attaching decode")
                .decode = Some(decode_id);
        }
        Ok(())
    }

    fn pipe_builder(&mut self, key: PipeKey) -> Option<&mut PipeBuilder> {
        if !self.pipe_builders.contains_key(&key) {
            let owner = self.item_map.get(&key.owner).copied();
            let Some(owner) = owner else {
                self.errors
                    .push(LoweringError::UnknownOwner { owner: key.owner });
                return None;
            };
            let span = self.hir.exprs()[key.pipe_expr].span;
            self.pipe_builders.insert(
                key,
                PipeBuilder {
                    owner,
                    origin: PipeOrigin {
                        owner: key.owner,
                        pipe_expr: key.pipe_expr,
                        span,
                    },
                    stages: BTreeMap::new(),
                    recurrence: None,
                },
            );
        }
        self.pipe_builders.get_mut(&key)
    }

    fn lower_recurrence_stage(
        &mut self,
        owner: HirItemId,
        stage: &aivi_hir::RecurrenceStagePlan,
    ) -> Result<RecurrenceStage, LoweringError> {
        Ok(RecurrenceStage {
            stage_index: stage.stage_index,
            stage_span: stage.stage_span,
            origin_expr: stage.expr,
            input_subject: Type::lower(&stage.input_subject),
            result_subject: Type::lower(&stage.result_subject),
            runtime_expr: self.lower_runtime_expr(owner, &stage.runtime_expr)?,
        })
    }

    fn map_dependency(&mut self, owner: HirItemId, dependency: HirItemId) -> Option<ItemId> {
        match self.item_map.get(&dependency).copied() {
            Some(item) => Some(item),
            None => {
                self.errors
                    .push(LoweringError::DependencyOutsideCore { owner, dependency });
                None
            }
        }
    }

    fn alloc_expr(
        &mut self,
        _owner: HirItemId,
        _span: SourceSpan,
        expr: Expr,
    ) -> Result<ExprId, LoweringError> {
        self.module
            .exprs_mut()
            .alloc(expr)
            .map_err(|overflow| LoweringError::ArenaOverflow {
                arena: "exprs",
                attempted_len: overflow.attempted_len(),
            })
    }

    fn lower_runtime_expr(
        &mut self,
        owner: HirItemId,
        root: &GateRuntimeExpr,
    ) -> Result<ExprId, LoweringError> {
        enum Task<'a> {
            Visit(&'a GateRuntimeExpr),
            BuildText {
                span: SourceSpan,
                ty: Type,
                segments: Vec<SegmentSpec>,
            },
            BuildTuple {
                span: SourceSpan,
                ty: Type,
                len: usize,
            },
            BuildList {
                span: SourceSpan,
                ty: Type,
                len: usize,
            },
            BuildMap {
                span: SourceSpan,
                ty: Type,
                entries: usize,
            },
            BuildSet {
                span: SourceSpan,
                ty: Type,
                len: usize,
            },
            BuildRecord {
                span: SourceSpan,
                ty: Type,
                labels: Vec<Box<str>>,
            },
            BuildProjection {
                span: SourceSpan,
                ty: Type,
                base_is_expr: bool,
                path: Vec<Box<str>>,
            },
            BuildApply {
                span: SourceSpan,
                ty: Type,
                arguments: usize,
            },
            BuildUnary {
                span: SourceSpan,
                ty: Type,
                operator: aivi_hir::UnaryOperator,
            },
            BuildBinary {
                span: SourceSpan,
                ty: Type,
                operator: aivi_hir::BinaryOperator,
            },
            BuildPipe {
                span: SourceSpan,
                ty: Type,
                stages: Vec<PipeStageSpec>,
            },
        }

        let mut tasks = vec![Task::Visit(root)];
        let mut values = Vec::new();

        while let Some(task) = tasks.pop() {
            match task {
                Task::Visit(expr) => {
                    let ty = Type::lower(&expr.ty);
                    match &expr.kind {
                        GateRuntimeExprKind::AmbientSubject => {
                            values.push(self.alloc_expr(
                                owner,
                                expr.span,
                                Expr {
                                    span: expr.span,
                                    ty,
                                    kind: ExprKind::AmbientSubject,
                                },
                            )?);
                        }
                        GateRuntimeExprKind::Reference(reference) => {
                            values.push(
                                self.alloc_expr(
                                    owner,
                                    expr.span,
                                    Expr {
                                        span: expr.span,
                                        ty,
                                        kind: ExprKind::Reference(match reference {
                                            GateRuntimeReference::Local(binding) => {
                                                Reference::Local(*binding)
                                            }
                                            GateRuntimeReference::Item(item) => self
                                                .item_map
                                                .get(item)
                                                .copied()
                                                .map(Reference::Item)
                                                .unwrap_or(Reference::HirItem(*item)),
                                            GateRuntimeReference::Builtin(term) => {
                                                Reference::Builtin(*term)
                                            }
                                        }),
                                    },
                                )?,
                            );
                        }
                        GateRuntimeExprKind::Integer(integer) => {
                            values.push(self.alloc_expr(
                                owner,
                                expr.span,
                                Expr {
                                    span: expr.span,
                                    ty,
                                    kind: ExprKind::Integer(integer.clone()),
                                },
                            )?);
                        }
                        GateRuntimeExprKind::SuffixedInteger(integer) => {
                            values.push(self.alloc_expr(
                                owner,
                                expr.span,
                                Expr {
                                    span: expr.span,
                                    ty,
                                    kind: ExprKind::SuffixedInteger(integer.clone()),
                                },
                            )?);
                        }
                        GateRuntimeExprKind::Text(text) => {
                            tasks.push(Task::BuildText {
                                span: expr.span,
                                ty,
                                segments: text_segment_specs(text),
                            });
                            for segment in text.segments.iter().rev() {
                                if let GateRuntimeTextSegment::Interpolation(interpolation) =
                                    segment
                                {
                                    tasks.push(Task::Visit(interpolation));
                                }
                            }
                        }
                        GateRuntimeExprKind::Tuple(elements) => {
                            tasks.push(Task::BuildTuple {
                                span: expr.span,
                                ty,
                                len: elements.len(),
                            });
                            for element in elements.iter().rev() {
                                tasks.push(Task::Visit(element));
                            }
                        }
                        GateRuntimeExprKind::List(elements) => {
                            tasks.push(Task::BuildList {
                                span: expr.span,
                                ty,
                                len: elements.len(),
                            });
                            for element in elements.iter().rev() {
                                tasks.push(Task::Visit(element));
                            }
                        }
                        GateRuntimeExprKind::Map(entries) => {
                            tasks.push(Task::BuildMap {
                                span: expr.span,
                                ty,
                                entries: entries.len(),
                            });
                            for entry in entries.iter().rev() {
                                tasks.push(Task::Visit(&entry.value));
                                tasks.push(Task::Visit(&entry.key));
                            }
                        }
                        GateRuntimeExprKind::Set(elements) => {
                            tasks.push(Task::BuildSet {
                                span: expr.span,
                                ty,
                                len: elements.len(),
                            });
                            for element in elements.iter().rev() {
                                tasks.push(Task::Visit(element));
                            }
                        }
                        GateRuntimeExprKind::Record(fields) => {
                            tasks.push(Task::BuildRecord {
                                span: expr.span,
                                ty,
                                labels: fields
                                    .iter()
                                    .map(|field| field.label.text().into())
                                    .collect(),
                            });
                            for field in fields.iter().rev() {
                                tasks.push(Task::Visit(&field.value));
                            }
                        }
                        GateRuntimeExprKind::Projection { base, path } => {
                            tasks.push(Task::BuildProjection {
                                span: expr.span,
                                ty,
                                base_is_expr: matches!(base, GateRuntimeProjectionBase::Expr(_)),
                                path: path
                                    .segments()
                                    .iter()
                                    .map(|segment| segment.text().into())
                                    .collect(),
                            });
                            if let GateRuntimeProjectionBase::Expr(base) = base {
                                tasks.push(Task::Visit(base));
                            }
                        }
                        GateRuntimeExprKind::Apply { callee, arguments } => {
                            tasks.push(Task::BuildApply {
                                span: expr.span,
                                ty,
                                arguments: arguments.len(),
                            });
                            for argument in arguments.iter().rev() {
                                tasks.push(Task::Visit(argument));
                            }
                            tasks.push(Task::Visit(callee));
                        }
                        GateRuntimeExprKind::Unary {
                            operator,
                            expr: inner,
                        } => {
                            tasks.push(Task::BuildUnary {
                                span: expr.span,
                                ty,
                                operator: *operator,
                            });
                            tasks.push(Task::Visit(inner));
                        }
                        GateRuntimeExprKind::Binary {
                            left,
                            operator,
                            right,
                        } => {
                            tasks.push(Task::BuildBinary {
                                span: expr.span,
                                ty,
                                operator: *operator,
                            });
                            tasks.push(Task::Visit(right));
                            tasks.push(Task::Visit(left));
                        }
                        GateRuntimeExprKind::Pipe(pipe) => {
                            tasks.push(Task::BuildPipe {
                                span: expr.span,
                                ty,
                                stages: pipe_stage_specs(pipe),
                            });
                            for stage in pipe.stages.iter().rev() {
                                match &stage.kind {
                                    GateRuntimePipeStageKind::Transform { expr }
                                    | GateRuntimePipeStageKind::Tap { expr } => {
                                        tasks.push(Task::Visit(expr));
                                    }
                                    GateRuntimePipeStageKind::Gate { predicate, .. } => {
                                        tasks.push(Task::Visit(predicate));
                                    }
                                }
                            }
                            tasks.push(Task::Visit(&pipe.head));
                        }
                    }
                }
                Task::BuildText { span, ty, segments } => {
                    let interpolation_count = segments
                        .iter()
                        .filter(|segment| matches!(segment, SegmentSpec::Interpolation { .. }))
                        .count();
                    let mut exprs = drain_tail(&mut values, interpolation_count).into_iter();
                    let segments = segments
                        .into_iter()
                        .map(|segment| match segment {
                            SegmentSpec::Fragment { raw, span } => {
                                TextSegment::Fragment { raw, span }
                            }
                            SegmentSpec::Interpolation { span } => TextSegment::Interpolation {
                                expr: exprs.next().expect("text interpolation count should match"),
                                span,
                            },
                        })
                        .collect();
                    values.push(self.alloc_expr(
                        owner,
                        span,
                        Expr {
                            span,
                            ty,
                            kind: ExprKind::Text(TextLiteral { segments }),
                        },
                    )?);
                }
                Task::BuildTuple { span, ty, len } => {
                    let elements = drain_tail(&mut values, len);
                    values.push(self.alloc_expr(
                        owner,
                        span,
                        Expr {
                            span,
                            ty,
                            kind: ExprKind::Tuple(elements),
                        },
                    )?);
                }
                Task::BuildList { span, ty, len } => {
                    let elements = drain_tail(&mut values, len);
                    values.push(self.alloc_expr(
                        owner,
                        span,
                        Expr {
                            span,
                            ty,
                            kind: ExprKind::List(elements),
                        },
                    )?);
                }
                Task::BuildMap { span, ty, entries } => {
                    let lowered = drain_tail(&mut values, entries * 2);
                    let mut iter = lowered.into_iter();
                    let entries = (0..entries)
                        .map(|_| MapEntry {
                            key: iter.next().expect("map key should exist"),
                            value: iter.next().expect("map value should exist"),
                        })
                        .collect();
                    values.push(self.alloc_expr(
                        owner,
                        span,
                        Expr {
                            span,
                            ty,
                            kind: ExprKind::Map(entries),
                        },
                    )?);
                }
                Task::BuildSet { span, ty, len } => {
                    let elements = drain_tail(&mut values, len);
                    values.push(self.alloc_expr(
                        owner,
                        span,
                        Expr {
                            span,
                            ty,
                            kind: ExprKind::Set(elements),
                        },
                    )?);
                }
                Task::BuildRecord { span, ty, labels } => {
                    let len = labels.len();
                    let fields = labels
                        .into_iter()
                        .zip(drain_tail(&mut values, len))
                        .map(|(label, value)| RecordExprField { label, value })
                        .collect();
                    values.push(self.alloc_expr(
                        owner,
                        span,
                        Expr {
                            span,
                            ty,
                            kind: ExprKind::Record(fields),
                        },
                    )?);
                }
                Task::BuildProjection {
                    span,
                    ty,
                    base_is_expr,
                    path,
                } => {
                    let base = if base_is_expr {
                        ProjectionBase::Expr(values.pop().expect("projection base should exist"))
                    } else {
                        ProjectionBase::AmbientSubject
                    };
                    values.push(self.alloc_expr(
                        owner,
                        span,
                        Expr {
                            span,
                            ty,
                            kind: ExprKind::Projection { base, path },
                        },
                    )?);
                }
                Task::BuildApply {
                    span,
                    ty,
                    arguments,
                } => {
                    let lowered = drain_tail(&mut values, arguments + 1);
                    let mut iter = lowered.into_iter();
                    let callee = iter.next().expect("apply callee should exist");
                    let arguments = iter.collect();
                    values.push(self.alloc_expr(
                        owner,
                        span,
                        Expr {
                            span,
                            ty,
                            kind: ExprKind::Apply { callee, arguments },
                        },
                    )?);
                }
                Task::BuildUnary { span, ty, operator } => {
                    let inner = values.pop().expect("unary child should exist");
                    values.push(self.alloc_expr(
                        owner,
                        span,
                        Expr {
                            span,
                            ty,
                            kind: ExprKind::Unary {
                                operator,
                                expr: inner,
                            },
                        },
                    )?);
                }
                Task::BuildBinary { span, ty, operator } => {
                    let lowered = drain_tail(&mut values, 2);
                    values.push(self.alloc_expr(
                        owner,
                        span,
                        Expr {
                            span,
                            ty,
                            kind: ExprKind::Binary {
                                left: lowered[0],
                                operator,
                                right: lowered[1],
                            },
                        },
                    )?);
                }
                Task::BuildPipe { span, ty, stages } => {
                    let lowered = drain_tail(&mut values, stages.len() + 1);
                    let mut iter = lowered.into_iter();
                    let head = iter.next().expect("pipe head should exist");
                    let stages = stages
                        .into_iter()
                        .map(|stage| {
                            let expr = iter.next().expect("pipe stage expr should exist");
                            PipeStage {
                                span: stage.span,
                                input_subject: stage.input_subject,
                                result_subject: stage.result_subject,
                                kind: match stage.kind {
                                    PipeStageKindSpec::Transform => {
                                        crate::expr::PipeStageKind::Transform { expr }
                                    }
                                    PipeStageKindSpec::Tap => {
                                        crate::expr::PipeStageKind::Tap { expr }
                                    }
                                    PipeStageKindSpec::Gate {
                                        emits_negative_update,
                                    } => crate::expr::PipeStageKind::Gate {
                                        predicate: expr,
                                        emits_negative_update,
                                    },
                                },
                            }
                        })
                        .collect();
                    values.push(self.alloc_expr(
                        owner,
                        span,
                        Expr {
                            span,
                            ty,
                            kind: ExprKind::Pipe(PipeExpr { head, stages }),
                        },
                    )?);
                }
            }
        }

        Ok(values
            .pop()
            .expect("runtime expression lowering should produce one expression"))
    }

    fn lower_decode_program(
        &mut self,
        owner: ItemId,
        program: &SourceDecodeProgram,
    ) -> Result<DecodeProgram, LoweringError> {
        let mut steps = Arena::new();
        let step_positions = program
            .steps()
            .iter()
            .enumerate()
            .map(|(index, step)| (step as *const _, index))
            .collect::<HashMap<_, _>>();

        let step_id_for = |program: &SourceDecodeProgram,
                           step_positions: &HashMap<*const aivi_hir::DecodeProgramStep, usize>,
                           step_id: aivi_hir::DecodeProgramStepId|
         -> DecodeStepId {
            let ptr = program.step(step_id) as *const _;
            let index = step_positions[&ptr];
            DecodeStepId::from_raw(index as u32)
        };

        for step in program.steps() {
            let lowered = match step {
                aivi_hir::DecodeProgramStep::Scalar { scalar } => {
                    DecodeStep::Scalar { scalar: *scalar }
                }
                aivi_hir::DecodeProgramStep::Tuple { elements } => DecodeStep::Tuple {
                    elements: elements
                        .iter()
                        .map(|step| step_id_for(program, &step_positions, *step))
                        .collect(),
                },
                aivi_hir::DecodeProgramStep::Record {
                    fields,
                    extra_fields,
                } => DecodeStep::Record {
                    fields: fields
                        .iter()
                        .map(|field| DecodeField {
                            name: field.name.as_str().into(),
                            requirement: field.requirement,
                            step: step_id_for(program, &step_positions, field.step),
                        })
                        .collect(),
                    extra_fields: *extra_fields,
                },
                aivi_hir::DecodeProgramStep::Sum { variants, strategy } => DecodeStep::Sum {
                    variants: variants
                        .iter()
                        .map(|variant| crate::DecodeVariant {
                            name: variant.name.as_str().into(),
                            payload: variant
                                .payload
                                .map(|payload| step_id_for(program, &step_positions, payload)),
                        })
                        .collect(),
                    strategy: *strategy,
                },
                aivi_hir::DecodeProgramStep::Domain { carrier, surface } => DecodeStep::Domain {
                    carrier: step_id_for(program, &step_positions, *carrier),
                    surface: DomainDecodeSurface {
                        domain_item: surface.domain_item,
                        member_index: surface.member_index,
                        member_name: surface.member_name.clone(),
                        kind: match surface.kind {
                            aivi_hir::DomainDecodeSurfaceKind::Direct => {
                                DomainDecodeSurfaceKind::Direct
                            }
                            aivi_hir::DomainDecodeSurfaceKind::FallibleResult => {
                                DomainDecodeSurfaceKind::FallibleResult
                            }
                        },
                        span: surface.span,
                    },
                },
                aivi_hir::DecodeProgramStep::List { element } => DecodeStep::List {
                    element: step_id_for(program, &step_positions, *element),
                },
                aivi_hir::DecodeProgramStep::Option { element } => DecodeStep::Option {
                    element: step_id_for(program, &step_positions, *element),
                },
                aivi_hir::DecodeProgramStep::Result { error, value } => DecodeStep::Result {
                    error: step_id_for(program, &step_positions, *error),
                    value: step_id_for(program, &step_positions, *value),
                },
                aivi_hir::DecodeProgramStep::Validation { error, value } => {
                    DecodeStep::Validation {
                        error: step_id_for(program, &step_positions, *error),
                        value: step_id_for(program, &step_positions, *value),
                    }
                }
            };
            let _ = steps
                .alloc(lowered)
                .map_err(|overflow| LoweringError::ArenaOverflow {
                    arena: "decode-steps",
                    attempted_len: overflow.attempted_len(),
                })?;
        }

        let root_index = step_positions[&(program.root_step() as *const _)] as u32;
        Ok(DecodeProgram {
            owner,
            mode: program.mode,
            payload_annotation: program.payload_annotation,
            root: DecodeStepId::from_raw(root_index),
            steps,
        })
    }
}

fn arena_overflow(arena: &'static str, overflow: ArenaOverflow) -> LoweringError {
    LoweringError::ArenaOverflow {
        arena,
        attempted_len: overflow.attempted_len(),
    }
}

fn join_spans(left: SourceSpan, right: SourceSpan) -> SourceSpan {
    left.join(right)
        .expect("typed-core lowering only joins spans from the same source file")
}

fn drain_tail<T>(values: &mut Vec<T>, len: usize) -> Vec<T> {
    let split = values
        .len()
        .checked_sub(len)
        .expect("requested more lowered values than available");
    values.drain(split..).collect()
}

fn text_segment_specs(text: &GateRuntimeTextLiteral) -> Vec<SegmentSpec> {
    text.segments
        .iter()
        .map(|segment| match segment {
            GateRuntimeTextSegment::Fragment(fragment) => SegmentSpec::Fragment {
                raw: fragment.raw.clone(),
                span: fragment.span,
            },
            GateRuntimeTextSegment::Interpolation(interpolation) => SegmentSpec::Interpolation {
                span: interpolation.span,
            },
        })
        .collect()
}

fn pipe_stage_specs(pipe: &GateRuntimePipeExpr) -> Vec<PipeStageSpec> {
    pipe.stages
        .iter()
        .map(|stage| PipeStageSpec {
            span: stage.span,
            input_subject: Type::lower(&stage.input_subject),
            result_subject: Type::lower(&stage.result_subject),
            kind: match &stage.kind {
                GateRuntimePipeStageKind::Transform { .. } => PipeStageKindSpec::Transform,
                GateRuntimePipeStageKind::Tap { .. } => PipeStageKindSpec::Tap,
                GateRuntimePipeStageKind::Gate {
                    emits_negative_update,
                    ..
                } => PipeStageKindSpec::Gate {
                    emits_negative_update: *emits_negative_update,
                },
            },
        })
        .collect()
}

#[derive(Clone)]
enum SegmentSpec {
    Fragment { raw: Box<str>, span: SourceSpan },
    Interpolation { span: SourceSpan },
}

#[derive(Clone)]
struct PipeStageSpec {
    span: SourceSpan,
    input_subject: Type,
    result_subject: Type,
    kind: PipeStageKindSpec,
}

#[derive(Clone)]
enum PipeStageKindSpec {
    Transform,
    Tap,
    Gate { emits_negative_update: bool },
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use aivi_base::SourceDatabase;
    use aivi_syntax::parse_module;

    use super::{LoweringError, lower_module};
    use crate::{
        DecodeStep, GateStage, ItemKind, StageKind, Type,
        validate::{ValidationError, validate_module},
    };

    fn fixture_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("fixtures")
            .join("frontend")
    }

    fn lower_text(path: &str, text: &str) -> aivi_hir::LoweringResult {
        let mut sources = SourceDatabase::new();
        let file_id = sources.add_file(path, text);
        let parsed = parse_module(&sources[file_id]);
        assert!(
            !parsed.has_errors(),
            "fixture {path} should parse before HIR lowering: {:?}",
            parsed.all_diagnostics().collect::<Vec<_>>()
        );
        aivi_hir::lower_module(&parsed.module)
    }

    fn lower_fixture(path: &str) -> aivi_hir::LoweringResult {
        let text =
            fs::read_to_string(fixture_root().join(path)).expect("fixture should be readable");
        lower_text(path, &text)
    }

    #[test]
    fn lowers_pipe_and_source_fixtures_into_core_ir() {
        let lowered = lower_fixture("milestone-2/valid/pipe-gate-carriers/main.aivi");
        assert!(
            !lowered.has_errors(),
            "gate fixture should lower cleanly before typed-core lowering: {:?}",
            lowered.diagnostics()
        );

        let core = lower_module(lowered.module()).expect("typed-core lowering should succeed");
        validate_module(&core).expect("lowered core module should validate");

        let maybe_user = core
            .items()
            .iter()
            .find(|(_, item)| item.name.as_ref() == "maybeUser")
            .map(|(id, _)| id)
            .expect("expected maybeUser item");
        let pipes = &core.items()[maybe_user].pipes;
        assert_eq!(pipes.len(), 1);
        let pipe = &core.pipes()[pipes[0]];
        let first_stage = &core.stages()[pipe.stages[0]];
        assert!(matches!(
            &first_stage.kind,
            StageKind::Gate(GateStage::Ordinary { .. })
        ));
        let pretty = core.pretty();
        assert!(
            pretty.contains("gate"),
            "pretty dump should mention gate stages: {pretty}"
        );
    }

    #[test]
    fn lowers_source_and_decode_programs_into_core_ir() {
        let lowered = lower_text(
            "typed-core-source-decode.aivi",
            r#"
domain Duration over Int
    parse : Int -> Result Text Duration
    value : Duration -> Int

@source custom.feed
sig timeout : Signal Duration
"#,
        );
        assert!(
            !lowered.has_errors(),
            "source/decode example should lower cleanly before typed-core lowering: {:?}",
            lowered.diagnostics()
        );

        let core = lower_module(lowered.module()).expect("typed-core lowering should succeed");
        let timeout = core
            .items()
            .iter()
            .find(|(_, item)| item.name.as_ref() == "timeout")
            .map(|(id, _)| id)
            .expect("expected timeout signal item");
        let ItemKind::Signal(info) = &core.items()[timeout].kind else {
            panic!("timeout should remain a signal item");
        };
        let source = info
            .source
            .expect("timeout should carry a lowered source node");
        let decode = core.sources()[source]
            .decode
            .expect("source should carry a decode program");
        match &core.decode_programs()[decode].steps()[core.decode_programs()[decode].root] {
            DecodeStep::Domain { surface, .. } => {
                assert_eq!(surface.member_name.as_ref(), "parse");
                assert_eq!(surface.kind, crate::DomainDecodeSurfaceKind::FallibleResult);
            }
            other => panic!("expected domain decode root, found {other:?}"),
        }
    }

    #[test]
    fn lowers_recurrence_reports_into_pipe_nodes() {
        let lowered = lower_fixture("milestone-2/valid/pipe-recurrence-nonsource-wakeup/main.aivi");
        assert!(
            !lowered.has_errors(),
            "recurrence fixture should lower cleanly before typed-core lowering: {:?}",
            lowered.diagnostics()
        );

        let core = lower_module(lowered.module()).expect("typed-core lowering should succeed");
        let polled = core
            .items()
            .iter()
            .find(|(_, item)| item.name.as_ref() == "polled")
            .map(|(id, _)| id)
            .expect("expected polled signal item");
        let pipe = &core.pipes()[core.items()[polled].pipes[0]];
        let recurrence = pipe
            .recurrence
            .as_ref()
            .expect("expected recurrence attachment");
        assert_eq!(recurrence.steps.len(), 1);
        assert!(recurrence.non_source_wakeup.is_some());
    }

    #[test]
    fn rejects_blocked_hir_handoffs_instead_of_guessing() {
        let lowered = lower_fixture("milestone-2/invalid/gate-predicate-not-bool/main.aivi");
        let errors = lower_module(lowered.module()).expect_err("blocked gate should stop lowering");
        assert!(
            errors
                .errors()
                .iter()
                .any(|error| matches!(error, LoweringError::BlockedGateStage { .. }))
        );
    }

    #[test]
    fn rejects_blocked_decode_programs() {
        let lowered = lower_text(
            "typed-core-blocked-decode.aivi",
            r#"
domain Duration over Int
    millis : Int -> Duration
    tryMillis : Int -> Result Text Duration
    value : Duration -> Int

@source custom.feed
sig timeout : Signal Duration
"#,
        );
        assert!(
            !lowered.has_errors(),
            "ambiguous decode example should lower cleanly before typed-core lowering: {:?}",
            lowered.diagnostics()
        );

        let errors =
            lower_module(lowered.module()).expect_err("ambiguous decode should block lowering");
        assert!(
            errors
                .errors()
                .iter()
                .any(|error| matches!(error, LoweringError::BlockedDecodeProgram { .. }))
        );
    }

    #[test]
    fn validator_catches_broken_recurrence_closure() {
        let lowered = lower_fixture("milestone-2/valid/pipe-recurrence-nonsource-wakeup/main.aivi");
        let mut core = lower_module(lowered.module()).expect("typed-core lowering should succeed");
        let pipe_id = core
            .pipes()
            .iter()
            .find(|(_, pipe)| pipe.recurrence.is_some())
            .map(|(id, _)| id)
            .expect("expected recurrence pipe");
        let pipe = core
            .pipes_mut()
            .get_mut(pipe_id)
            .expect("pipe should exist");
        let recurrence = pipe.recurrence.as_mut().expect("recurrence should exist");
        recurrence.steps[0].result_subject = Type::Primitive(aivi_hir::BuiltinType::Text);
        let errors =
            validate_module(&core).expect_err("manually broken recurrence should fail validation");
        assert!(
            errors
                .errors()
                .iter()
                .any(|error| matches!(error, ValidationError::RecurrenceDoesNotClose { .. }))
        );
    }
}
