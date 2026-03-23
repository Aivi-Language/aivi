use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use aivi_typing::{BuiltinSourceProvider, RecurrenceWakeupPlan, SourceCancellationPolicy};

use crate::{
    graph::{InputHandle, OwnerHandle, SignalGraph, SignalHandle},
    scheduler::{
        DerivedNodeEvaluator, Generation, Publication, PublicationStamp, Scheduler,
        SchedulerAccessError, TickOutcome, WorkerPublicationSender,
    },
};

macro_rules! define_runtime_id {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(u32);

        impl $name {
            pub const fn from_raw(raw: u32) -> Self {
                Self(raw)
            }

            pub const fn as_raw(self) -> u32 {
                self.0
            }
        }
    };
}

define_runtime_id!(SourceInstanceId);
define_runtime_id!(TaskInstanceId);

/// Runtime-facing provider identity carried forward from source elaboration/lowering.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum RuntimeSourceProvider {
    Builtin(BuiltinSourceProvider),
    Custom(Box<str>),
}

impl RuntimeSourceProvider {
    pub const fn builtin(provider: BuiltinSourceProvider) -> Self {
        Self::Builtin(provider)
    }

    pub fn custom(provider: impl Into<Box<str>>) -> Self {
        Self::Custom(provider.into())
    }
}

/// Reconfiguration contract retained from the current source-lifecycle handoff.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SourceReplacementPolicy {
    DisposeSupersededBeforePublish,
}

/// Stale-work contract retained from the current source-lifecycle handoff.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SourceStaleWorkPolicy {
    DropStalePublications,
}

/// Runtime-facing source contract ready to accept lowered compiler handoffs.
///
/// `D` intentionally stays generic so later lowering can store either the compiler's decode
/// program directly or a lowered runtime-local decode reference without changing the scheduler
/// boundary again.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceRuntimeSpec<D = ()> {
    pub instance: SourceInstanceId,
    pub input: InputHandle,
    pub provider: RuntimeSourceProvider,
    pub reconfiguration_dependencies: Box<[SignalHandle]>,
    pub explicit_triggers: Box<[SignalHandle]>,
    pub active_when: Option<SignalHandle>,
    pub cancellation: SourceCancellationPolicy,
    pub replacement: SourceReplacementPolicy,
    pub stale_work: SourceStaleWorkPolicy,
    pub wakeup: Option<RecurrenceWakeupPlan>,
    pub decode: Option<D>,
}

impl<D> SourceRuntimeSpec<D> {
    pub fn new(
        instance: SourceInstanceId,
        input: InputHandle,
        provider: RuntimeSourceProvider,
    ) -> Self {
        Self {
            instance,
            input,
            provider,
            reconfiguration_dependencies: Vec::new().into_boxed_slice(),
            explicit_triggers: Vec::new().into_boxed_slice(),
            active_when: None,
            cancellation: SourceCancellationPolicy::ProviderManaged,
            replacement: SourceReplacementPolicy::DisposeSupersededBeforePublish,
            stale_work: SourceStaleWorkPolicy::DropStalePublications,
            wakeup: None,
            decode: None,
        }
    }
}

/// Runtime-facing task execution contract.
///
/// The current slice chooses the narrowest coherent scheduler contract: one live run per task
/// instance. Starting the same task again supersedes the older run by cancelling it and advancing
/// the scheduler generation on the task's sink input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskRuntimeSpec {
    pub instance: TaskInstanceId,
    pub input: InputHandle,
    pub dependencies: Box<[SignalHandle]>,
    pub wakeup: Option<RecurrenceWakeupPlan>,
}

impl TaskRuntimeSpec {
    pub fn new(instance: TaskInstanceId, input: InputHandle) -> Self {
        Self {
            instance,
            input,
            dependencies: Vec::new().into_boxed_slice(),
            wakeup: None,
        }
    }
}

/// Read-only worker-side cancellation view.
///
/// Workers may observe cancellation but cannot directly mutate scheduler-owned task/source state.
#[derive(Clone, Debug, Default)]
pub struct CancellationObserver {
    state: Arc<AtomicBool>,
}

impl CancellationObserver {
    pub fn is_cancelled(&self) -> bool {
        self.state.load(Ordering::Acquire)
    }
}

#[derive(Clone, Debug, Default)]
struct CancellationHandle {
    state: Arc<AtomicBool>,
}

impl CancellationHandle {
    fn new() -> Self {
        Self::default()
    }

    fn observer(&self) -> CancellationObserver {
        CancellationObserver {
            state: self.state.clone(),
        }
    }

    fn cancel(&self) {
        self.state.store(true, Ordering::Release);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PublicationPortError<V> {
    Cancelled { stamp: PublicationStamp, value: V },
    Disconnected { stamp: PublicationStamp, value: V },
}

/// Cloneable worker-side publication port for long-lived sources.
#[derive(Clone)]
pub struct SourcePublicationPort<V> {
    sender: WorkerPublicationSender<V>,
    stamp: PublicationStamp,
    cancellation: CancellationObserver,
}

impl<V> SourcePublicationPort<V> {
    pub fn stamp(&self) -> PublicationStamp {
        self.stamp
    }

    pub fn cancellation(&self) -> CancellationObserver {
        self.cancellation.clone()
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancellation.is_cancelled()
    }

    pub fn publish(&self, value: V) -> Result<(), PublicationPortError<V>> {
        publish_with_contract(&self.sender, self.stamp, &self.cancellation, value)
    }
}

/// One-shot worker-side completion port for `Task` execution.
pub struct TaskCompletionPort<V> {
    sender: WorkerPublicationSender<V>,
    stamp: PublicationStamp,
    cancellation: CancellationObserver,
}

impl<V> TaskCompletionPort<V> {
    pub fn stamp(&self) -> PublicationStamp {
        self.stamp
    }

    pub fn cancellation(&self) -> CancellationObserver {
        self.cancellation.clone()
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancellation.is_cancelled()
    }

    pub fn complete(self, value: V) -> Result<(), PublicationPortError<V>> {
        publish_with_contract(&self.sender, self.stamp, &self.cancellation, value)
    }
}

fn publish_with_contract<V>(
    sender: &WorkerPublicationSender<V>,
    stamp: PublicationStamp,
    cancellation: &CancellationObserver,
    value: V,
) -> Result<(), PublicationPortError<V>> {
    if cancellation.is_cancelled() {
        return Err(PublicationPortError::Cancelled { stamp, value });
    }

    sender
        .publish(Publication::new(stamp, value))
        .map_err(|err| {
            let (stamp, value) = err.into_publication().into_parts();
            PublicationPortError::Disconnected { stamp, value }
        })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TaskSourceRuntimeError {
    UnknownSignalHandle { signal: u32 },
    SignalIsNotInput { signal: u32 },
    DuplicateManagedInput { input: u32 },
    DuplicateSourceInstance { instance: SourceInstanceId },
    DuplicateTaskInstance { instance: TaskInstanceId },
    UnknownSourceInstance { instance: SourceInstanceId },
    UnknownTaskInstance { instance: TaskInstanceId },
    OwnerPendingDisposal { owner: OwnerHandle },
    Scheduler(SchedulerAccessError),
}

impl From<SchedulerAccessError> for TaskSourceRuntimeError {
    fn from(value: SchedulerAccessError) -> Self {
        Self::Scheduler(value)
    }
}

pub struct TaskSourceRuntime<V, D = ()> {
    scheduler: Scheduler<V>,
    sources: BTreeMap<SourceInstanceId, SourceSlot<D>>,
    tasks: BTreeMap<TaskInstanceId, TaskSlot>,
    claimed_inputs: BTreeMap<u32, ManagedInputKind>,
    pending_owner_disposals: BTreeSet<OwnerHandle>,
}

impl<V, D> TaskSourceRuntime<V, D> {
    pub fn new(graph: SignalGraph) -> Self {
        Self::from_scheduler(Scheduler::new(graph))
    }

    pub fn from_scheduler(scheduler: Scheduler<V>) -> Self {
        Self {
            scheduler,
            sources: BTreeMap::new(),
            tasks: BTreeMap::new(),
            claimed_inputs: BTreeMap::new(),
            pending_owner_disposals: BTreeSet::new(),
        }
    }

    pub fn graph(&self) -> &SignalGraph {
        self.scheduler.graph()
    }

    pub fn worker_sender(&self) -> WorkerPublicationSender<V> {
        self.scheduler.worker_sender()
    }

    pub fn tick_count(&self) -> u64 {
        self.scheduler.tick_count()
    }

    pub fn current_value(
        &self,
        signal: SignalHandle,
    ) -> Result<Option<&V>, TaskSourceRuntimeError> {
        self.scheduler.current_value(signal).map_err(Into::into)
    }

    pub fn current_generation(
        &self,
        input: InputHandle,
    ) -> Result<Generation, TaskSourceRuntimeError> {
        self.scheduler.current_generation(input).map_err(Into::into)
    }

    pub fn is_owner_active(&self, owner: OwnerHandle) -> Result<bool, TaskSourceRuntimeError> {
        self.scheduler.is_owner_active(owner).map_err(Into::into)
    }

    pub fn source_spec(&self, instance: SourceInstanceId) -> Option<&SourceRuntimeSpec<D>> {
        self.sources.get(&instance).map(|slot| &slot.spec)
    }

    pub fn task_spec(&self, instance: TaskInstanceId) -> Option<&TaskRuntimeSpec> {
        self.tasks.get(&instance).map(|slot| &slot.spec)
    }

    pub fn is_source_active(&self, instance: SourceInstanceId) -> bool {
        self.sources
            .get(&instance)
            .is_some_and(|slot| slot.active.is_some())
    }

    pub fn is_task_active(&self, instance: TaskInstanceId) -> bool {
        self.tasks
            .get(&instance)
            .is_some_and(|slot| slot.active.is_some())
    }

    pub fn register_source(
        &mut self,
        spec: SourceRuntimeSpec<D>,
    ) -> Result<(), TaskSourceRuntimeError> {
        self.validate_input_handle(spec.input)?;
        for &dependency in &spec.reconfiguration_dependencies {
            self.validate_signal_handle(dependency)?;
        }
        for &trigger in &spec.explicit_triggers {
            self.validate_signal_handle(trigger)?;
        }
        if let Some(active_when) = spec.active_when {
            self.validate_signal_handle(active_when)?;
        }
        if self.sources.contains_key(&spec.instance) {
            return Err(TaskSourceRuntimeError::DuplicateSourceInstance {
                instance: spec.instance,
            });
        }
        self.claim_input(spec.input)?;

        let owner = self.input_owner(spec.input);
        self.sources.insert(
            spec.instance,
            SourceSlot {
                spec,
                owner,
                active: None,
            },
        );
        Ok(())
    }

    pub fn register_task(&mut self, spec: TaskRuntimeSpec) -> Result<(), TaskSourceRuntimeError> {
        self.validate_input_handle(spec.input)?;
        for &dependency in &spec.dependencies {
            self.validate_signal_handle(dependency)?;
        }
        if self.tasks.contains_key(&spec.instance) {
            return Err(TaskSourceRuntimeError::DuplicateTaskInstance {
                instance: spec.instance,
            });
        }
        self.claim_input(spec.input)?;

        let owner = self.input_owner(spec.input);
        self.tasks.insert(
            spec.instance,
            TaskSlot {
                spec,
                owner,
                active: None,
            },
        );
        Ok(())
    }

    pub fn activate_source(
        &mut self,
        instance: SourceInstanceId,
    ) -> Result<SourcePublicationPort<V>, TaskSourceRuntimeError> {
        let (owner, input, active) = {
            let slot = self
                .sources
                .get(&instance)
                .ok_or(TaskSourceRuntimeError::UnknownSourceInstance { instance })?;
            (slot.owner, slot.spec.input, slot.active.clone())
        };
        self.ensure_owner_ready(owner)?;

        if let Some(active) = active {
            return Ok(active.source_port(self.scheduler.worker_sender()));
        }

        let next = ActiveLease::new(self.scheduler.current_stamp(input)?);
        let port = next.source_port(self.scheduler.worker_sender());
        self.sources
            .get_mut(&instance)
            .expect("validated source slot disappeared")
            .active = Some(next);
        Ok(port)
    }

    pub fn reconfigure_source(
        &mut self,
        instance: SourceInstanceId,
    ) -> Result<SourcePublicationPort<V>, TaskSourceRuntimeError> {
        let (owner, input, active) = {
            let slot = self
                .sources
                .get(&instance)
                .ok_or(TaskSourceRuntimeError::UnknownSourceInstance { instance })?;
            (slot.owner, slot.spec.input, slot.active.clone())
        };
        self.ensure_owner_ready(owner)?;

        let stamp = if let Some(active) = active {
            active.cancellation.cancel();
            self.scheduler.advance_generation(input)?
        } else {
            self.scheduler.current_stamp(input)?
        };
        let next = ActiveLease::new(stamp);
        let port = next.source_port(self.scheduler.worker_sender());
        self.sources
            .get_mut(&instance)
            .expect("validated source slot disappeared")
            .active = Some(next);
        Ok(port)
    }

    pub fn suspend_source(
        &mut self,
        instance: SourceInstanceId,
    ) -> Result<(), TaskSourceRuntimeError> {
        let (input, active) = {
            let slot = self
                .sources
                .get(&instance)
                .ok_or(TaskSourceRuntimeError::UnknownSourceInstance { instance })?;
            (slot.spec.input, slot.active.clone())
        };

        if let Some(active) = active {
            active.cancellation.cancel();
            self.scheduler.advance_generation(input)?;
            self.sources
                .get_mut(&instance)
                .expect("validated source slot disappeared")
                .active = None;
        }
        Ok(())
    }

    pub fn start_task(
        &mut self,
        instance: TaskInstanceId,
    ) -> Result<TaskCompletionPort<V>, TaskSourceRuntimeError> {
        let (owner, input, active) = {
            let slot = self
                .tasks
                .get(&instance)
                .ok_or(TaskSourceRuntimeError::UnknownTaskInstance { instance })?;
            (slot.owner, slot.spec.input, slot.active.clone())
        };
        self.ensure_owner_ready(owner)?;

        let stamp = if let Some(active) = active {
            active.cancellation.cancel();
            self.scheduler.advance_generation(input)?
        } else {
            self.scheduler.current_stamp(input)?
        };
        let next = ActiveLease::new(stamp);
        let port = next.task_port(self.scheduler.worker_sender());
        self.tasks
            .get_mut(&instance)
            .expect("validated task slot disappeared")
            .active = Some(next);
        Ok(port)
    }

    pub fn cancel_task(&mut self, instance: TaskInstanceId) -> Result<(), TaskSourceRuntimeError> {
        let (input, active) = {
            let slot = self
                .tasks
                .get(&instance)
                .ok_or(TaskSourceRuntimeError::UnknownTaskInstance { instance })?;
            (slot.spec.input, slot.active.clone())
        };

        if let Some(active) = active {
            active.cancellation.cancel();
            self.scheduler.advance_generation(input)?;
            self.tasks
                .get_mut(&instance)
                .expect("validated task slot disappeared")
                .active = None;
        }
        Ok(())
    }

    pub fn dispose_owner(&mut self, owner: OwnerHandle) -> Result<(), TaskSourceRuntimeError> {
        self.scheduler.is_owner_active(owner)?;
        let subtree = self.collect_owner_subtree(owner);
        self.pending_owner_disposals.extend(subtree.iter().copied());
        self.cancel_sources_in_subtree(&subtree);
        self.cancel_tasks_in_subtree(&subtree);
        self.scheduler.queue_dispose_owner(owner)?;
        Ok(())
    }

    pub fn tick<E>(&mut self, evaluator: &mut E) -> TickOutcome
    where
        E: DerivedNodeEvaluator<V>,
    {
        let outcome = self.scheduler.tick(evaluator);
        self.pending_owner_disposals.clear();
        outcome
    }

    fn claim_input(&mut self, input: InputHandle) -> Result<(), TaskSourceRuntimeError> {
        if self
            .claimed_inputs
            .insert(input.as_raw(), ManagedInputKind::TaskOrSource)
            .is_some()
        {
            return Err(TaskSourceRuntimeError::DuplicateManagedInput {
                input: input.as_raw(),
            });
        }
        Ok(())
    }

    fn validate_signal_handle(&self, signal: SignalHandle) -> Result<(), TaskSourceRuntimeError> {
        if self.scheduler.graph().signal(signal).is_some() {
            Ok(())
        } else {
            Err(TaskSourceRuntimeError::UnknownSignalHandle {
                signal: signal.as_raw(),
            })
        }
    }

    fn validate_input_handle(&self, input: InputHandle) -> Result<(), TaskSourceRuntimeError> {
        match self.scheduler.graph().signal(input.as_signal()) {
            Some(spec) if spec.is_input() => Ok(()),
            Some(_) => Err(TaskSourceRuntimeError::SignalIsNotInput {
                signal: input.as_raw(),
            }),
            None => Err(TaskSourceRuntimeError::UnknownSignalHandle {
                signal: input.as_raw(),
            }),
        }
    }

    fn input_owner(&self, input: InputHandle) -> Option<OwnerHandle> {
        self.scheduler
            .graph()
            .signal(input.as_signal())
            .and_then(|spec| spec.owner())
    }

    fn ensure_owner_ready(&self, owner: Option<OwnerHandle>) -> Result<(), TaskSourceRuntimeError> {
        let Some(owner) = owner else {
            return Ok(());
        };

        if self.owner_pending_disposal(owner) {
            return Err(TaskSourceRuntimeError::OwnerPendingDisposal { owner });
        }

        if !self.scheduler.is_owner_active(owner)? {
            return Err(TaskSourceRuntimeError::Scheduler(
                SchedulerAccessError::OwnerInactive { owner },
            ));
        }
        Ok(())
    }

    fn owner_pending_disposal(&self, owner: OwnerHandle) -> bool {
        let mut current = Some(owner);
        while let Some(owner) = current {
            if self.pending_owner_disposals.contains(&owner) {
                return true;
            }
            current = self
                .scheduler
                .graph()
                .owner(owner)
                .and_then(|spec| spec.parent());
        }
        false
    }

    fn collect_owner_subtree(&self, owner: OwnerHandle) -> BTreeSet<OwnerHandle> {
        let mut subtree = BTreeSet::new();
        let mut worklist = VecDeque::from([owner]);

        while let Some(owner) = worklist.pop_front() {
            if !subtree.insert(owner) {
                continue;
            }
            if let Some(spec) = self.scheduler.graph().owner(owner) {
                for &child in spec.children() {
                    worklist.push_back(child);
                }
            }
        }

        subtree
    }

    fn cancel_sources_in_subtree(&mut self, subtree: &BTreeSet<OwnerHandle>) {
        for slot in self.sources.values_mut() {
            if slot.owner.is_some_and(|owner| subtree.contains(&owner))
                && let Some(active) = slot.active.take()
            {
                active.cancellation.cancel();
            }
        }
    }

    fn cancel_tasks_in_subtree(&mut self, subtree: &BTreeSet<OwnerHandle>) {
        for slot in self.tasks.values_mut() {
            if slot.owner.is_some_and(|owner| subtree.contains(&owner))
                && let Some(active) = slot.active.take()
            {
                active.cancellation.cancel();
            }
        }
    }
}

struct SourceSlot<D> {
    spec: SourceRuntimeSpec<D>,
    owner: Option<OwnerHandle>,
    active: Option<ActiveLease>,
}

struct TaskSlot {
    spec: TaskRuntimeSpec,
    owner: Option<OwnerHandle>,
    active: Option<ActiveLease>,
}

#[derive(Clone)]
struct ActiveLease {
    stamp: PublicationStamp,
    cancellation: CancellationHandle,
}

impl ActiveLease {
    fn new(stamp: PublicationStamp) -> Self {
        Self {
            stamp,
            cancellation: CancellationHandle::new(),
        }
    }

    fn source_port<V>(&self, sender: WorkerPublicationSender<V>) -> SourcePublicationPort<V> {
        SourcePublicationPort {
            sender,
            stamp: self.stamp,
            cancellation: self.cancellation.observer(),
        }
    }

    fn task_port<V>(&self, sender: WorkerPublicationSender<V>) -> TaskCompletionPort<V> {
        TaskCompletionPort {
            sender,
            stamp: self.stamp,
            cancellation: self.cancellation.observer(),
        }
    }
}

enum ManagedInputKind {
    TaskOrSource,
}

#[cfg(test)]
mod tests {
    use aivi_typing::{NonSourceWakeupCause, RecurrenceWakeupEvidence};

    use crate::{
        effects::{
            PublicationPortError, RuntimeSourceProvider, SourceInstanceId, SourceReplacementPolicy,
            SourceRuntimeSpec, SourceStaleWorkPolicy, TaskInstanceId, TaskRuntimeSpec,
            TaskSourceRuntime, TaskSourceRuntimeError,
        },
        graph::SignalGraphBuilder,
        scheduler::{DependencyValues, Publication, PublicationDropReason, SchedulerAccessError},
    };

    #[test]
    fn source_reconfiguration_cancels_old_resource_and_drops_stale_publications() {
        let mut builder = SignalGraphBuilder::new();
        let input = builder.add_input("users", None).unwrap();
        let graph = builder.build().unwrap();
        let mut runtime: TaskSourceRuntime<i32, &'static str> = TaskSourceRuntime::new(graph);

        let mut source = SourceRuntimeSpec::new(
            SourceInstanceId::from_raw(7),
            input,
            RuntimeSourceProvider::builtin(aivi_typing::BuiltinSourceProvider::HttpGet),
        );
        source.cancellation = aivi_typing::SourceCancellationPolicy::CancelInFlight;
        source.replacement = SourceReplacementPolicy::DisposeSupersededBeforePublish;
        source.stale_work = SourceStaleWorkPolicy::DropStalePublications;
        source.decode = Some("users.decode");
        runtime.register_source(source).unwrap();

        assert_eq!(
            runtime
                .source_spec(SourceInstanceId::from_raw(7))
                .unwrap()
                .decode,
            Some("users.decode")
        );

        let first = runtime
            .activate_source(SourceInstanceId::from_raw(7))
            .unwrap();
        assert!(!first.is_cancelled());
        first.publish(1).unwrap();
        runtime.tick(&mut |_, _: DependencyValues<'_, i32>| None);
        assert_eq!(
            runtime.current_value(input.as_signal()).unwrap().copied(),
            Some(1)
        );

        let stale_stamp = first.stamp();
        let second = runtime
            .reconfigure_source(SourceInstanceId::from_raw(7))
            .unwrap();
        assert!(first.is_cancelled());
        assert_eq!(
            first.publish(9),
            Err(PublicationPortError::Cancelled {
                stamp: stale_stamp,
                value: 9,
            })
        );

        runtime
            .worker_sender()
            .publish(Publication::new(stale_stamp, 99))
            .unwrap();
        second.publish(2).unwrap();
        let outcome = runtime.tick(&mut |_, _: DependencyValues<'_, i32>| None);

        assert_eq!(outcome.dropped_publications().len(), 1);
        assert_eq!(
            outcome.dropped_publications()[0].reason(),
            PublicationDropReason::StaleGeneration {
                active: second.stamp().generation(),
            }
        );
        assert_eq!(
            runtime.current_value(input.as_signal()).unwrap().copied(),
            Some(2)
        );
    }

    #[test]
    fn task_cancellation_suppresses_old_completions_and_retains_wakeup_handoff() {
        let mut builder = SignalGraphBuilder::new();
        let input = builder.add_input("task-result", None).unwrap();
        let graph = builder.build().unwrap();
        let mut runtime: TaskSourceRuntime<i32> = TaskSourceRuntime::new(graph);

        let wakeup =
            aivi_typing::RecurrenceWakeupPlan::from_evidence(RecurrenceWakeupEvidence::NonSource {
                cause: NonSourceWakeupCause::ExplicitTimer,
            });
        let mut task = TaskRuntimeSpec::new(TaskInstanceId::from_raw(3), input);
        task.wakeup = Some(wakeup);
        runtime.register_task(task).unwrap();

        assert_eq!(
            runtime
                .task_spec(TaskInstanceId::from_raw(3))
                .unwrap()
                .wakeup,
            Some(wakeup)
        );

        let first = runtime.start_task(TaskInstanceId::from_raw(3)).unwrap();
        let stale_stamp = first.stamp();
        runtime.cancel_task(TaskInstanceId::from_raw(3)).unwrap();
        assert!(first.is_cancelled());
        assert_eq!(
            first.complete(5),
            Err(PublicationPortError::Cancelled {
                stamp: stale_stamp,
                value: 5,
            })
        );

        runtime
            .worker_sender()
            .publish(Publication::new(stale_stamp, 13))
            .unwrap();
        let second = runtime.start_task(TaskInstanceId::from_raw(3)).unwrap();
        let second_stamp = second.stamp();
        second.complete(21).unwrap();
        let outcome = runtime.tick(&mut |_, _: DependencyValues<'_, i32>| None);

        assert_eq!(outcome.dropped_publications().len(), 1);
        assert_eq!(
            outcome.dropped_publications()[0].reason(),
            PublicationDropReason::StaleGeneration {
                active: second_stamp.generation(),
            }
        );
        assert_eq!(
            runtime.current_value(input.as_signal()).unwrap().copied(),
            Some(21)
        );
    }

    #[test]
    fn owner_disposal_cancels_registered_work_and_blocks_new_runs_until_tick() {
        let mut builder = SignalGraphBuilder::new();
        let owner = builder.add_owner("view", None).unwrap();
        let source_input = builder.add_input("source", Some(owner)).unwrap();
        let task_input = builder.add_input("task", Some(owner)).unwrap();
        let graph = builder.build().unwrap();
        let mut runtime: TaskSourceRuntime<i32> = TaskSourceRuntime::new(graph);

        runtime
            .register_source(SourceRuntimeSpec::new(
                SourceInstanceId::from_raw(1),
                source_input,
                RuntimeSourceProvider::builtin(aivi_typing::BuiltinSourceProvider::TimerEvery),
            ))
            .unwrap();
        runtime
            .register_task(TaskRuntimeSpec::new(
                TaskInstanceId::from_raw(2),
                task_input,
            ))
            .unwrap();

        let source = runtime
            .activate_source(SourceInstanceId::from_raw(1))
            .unwrap();
        let task = runtime.start_task(TaskInstanceId::from_raw(2)).unwrap();

        runtime.dispose_owner(owner).unwrap();
        assert!(source.is_cancelled());
        assert!(task.is_cancelled());
        assert!(matches!(
            runtime.start_task(TaskInstanceId::from_raw(2)),
            Err(TaskSourceRuntimeError::OwnerPendingDisposal { owner: actual_owner })
                if actual_owner == owner
        ));
        assert!(matches!(
            runtime.activate_source(SourceInstanceId::from_raw(1)),
            Err(TaskSourceRuntimeError::OwnerPendingDisposal { owner: actual_owner })
                if actual_owner == owner
        ));

        runtime
            .worker_sender()
            .publish(Publication::new(source.stamp(), 1))
            .unwrap();
        runtime
            .worker_sender()
            .publish(Publication::new(task.stamp(), 2))
            .unwrap();
        let outcome = runtime.tick(&mut |_, _: DependencyValues<'_, i32>| None);

        assert_eq!(runtime.is_owner_active(owner).unwrap(), false);
        assert_eq!(outcome.dropped_publications().len(), 2);
        assert!(
            outcome
                .dropped_publications()
                .iter()
                .all(|publication| publication.reason()
                    == PublicationDropReason::OwnerInactive { owner })
        );
        assert!(matches!(
            runtime.start_task(TaskInstanceId::from_raw(2)),
            Err(TaskSourceRuntimeError::Scheduler(
                SchedulerAccessError::OwnerInactive { owner: actual_owner }
            )) if actual_owner == owner
        ));
    }
}
