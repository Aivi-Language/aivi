use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    ffi::c_void,
    ptr,
    rc::Rc,
    time::{Duration, Instant},
};

use aivi_ffi_call::{
    AbiValue, AllocationArena, decode_len_prefixed_bytes, encode_len_prefixed_bytes,
    with_active_arena,
};

use crate::{
    BackendExecutionEngine, BackendExecutionEngineKind, BackendExecutionOptions, EvalFrame,
    EvaluationCallProfile, EvaluationError, ItemId, KernelEvaluationProfile, KernelEvaluator,
    KernelFingerprint, KernelId, LayoutId, LayoutKind, PrimitiveType, Program, RuntimeCallable,
    RuntimeFloat, RuntimeValue, TASK_COMPOSITION_KERNEL_ID, TaskFunctionApplier,
    cache::compile_kernel_jit_cached, codegen::CompiledJitKernel, compute_kernel_fingerprint,
    program::ItemKind,
};

pub(crate) struct LazyJitExecutionEngine<'a> {
    program: &'a Program,
    fallback: KernelEvaluator<'a>,
    last_kernel_call: Option<LastKernelCall>,
    item_cache: BTreeMap<ItemId, RuntimeValue>,
    item_stack: BTreeSet<ItemId>,
    eval_trace: Vec<EvalFrame>,
    kernel_plans: BTreeMap<KernelFingerprint, CachedKernelPlan>,
    jit_profile: Option<KernelEvaluationProfile>,
    combined_profile: Option<KernelEvaluationProfile>,
}

impl<'a> LazyJitExecutionEngine<'a> {
    pub(crate) fn new(program: &'a Program, options: BackendExecutionOptions) -> Self {
        Self::with_profile(program, options, false)
    }

    pub(crate) fn new_profiled(program: &'a Program, options: BackendExecutionOptions) -> Self {
        Self::with_profile(program, options, true)
    }

    fn with_profile(program: &'a Program, options: BackendExecutionOptions, profiled: bool) -> Self {
        let fallback = if profiled {
            KernelEvaluator::new_profiled(program)
        } else {
            KernelEvaluator::new(program)
        };
        let mut engine = Self {
            program,
            fallback,
            last_kernel_call: None,
            item_cache: BTreeMap::new(),
            item_stack: BTreeSet::new(),
            eval_trace: Vec::new(),
            kernel_plans: BTreeMap::new(),
            jit_profile: profiled.then(KernelEvaluationProfile::default),
            combined_profile: profiled.then(KernelEvaluationProfile::default),
        };
        if options.eagerly_compile_signals {
            engine.prepare_signal_body_plans();
        }
        engine.refresh_profile();
        engine
    }

    fn refresh_profile(&mut self) {
        let Some(jit_profile) = self.jit_profile.as_ref() else {
            self.combined_profile = None;
            return;
        };
        let mut combined = self.fallback.profile_snapshot().unwrap_or_default();
        combined.merge_from(jit_profile);
        self.combined_profile = Some(combined);
    }

    fn record_kernel_profile(&mut self, kernel: KernelId, elapsed: Duration, cache_hit: bool) {
        if let Some(profile) = &mut self.jit_profile {
            record_call(
                profile.kernels.entry(kernel).or_default(),
                elapsed,
                cache_hit,
            );
            self.refresh_profile();
        }
    }

    fn record_item_profile(&mut self, item: ItemId, elapsed: Duration, cache_hit: bool) {
        if let Some(profile) = &mut self.jit_profile {
            record_call(profile.items.entry(item).or_default(), elapsed, cache_hit);
            self.refresh_profile();
        }
    }

    fn prepare_kernel_plan(&mut self, kernel_id: KernelId) -> KernelFingerprint {
        let fingerprint = compute_kernel_fingerprint(self.program, kernel_id);
        self.kernel_plans
            .entry(fingerprint)
            .or_insert_with(|| CachedKernelPlan::build(self.program, kernel_id));
        fingerprint
    }

    fn prepare_signal_body_plans(&mut self) {
        let kernels = self
            .program
            .items()
            .iter()
            .filter_map(|(_, item)| match &item.kind {
                ItemKind::Signal(signal) => signal.body_kernel,
                _ => None,
            })
            .collect::<Vec<_>>();
        for kernel_id in kernels {
            self.prepare_kernel_plan(kernel_id);
        }
    }
}

impl BackendExecutionEngine for LazyJitExecutionEngine<'_> {
    fn kind(&self) -> BackendExecutionEngineKind {
        BackendExecutionEngineKind::Jit
    }

    fn program(&self) -> &Program {
        self.program
    }

    fn profile(&self) -> Option<&KernelEvaluationProfile> {
        self.combined_profile.as_ref()
    }

    fn profile_snapshot(&self) -> Option<KernelEvaluationProfile> {
        self.combined_profile.clone()
    }

    fn eval_trace(&self) -> &[EvalFrame] {
        &self.eval_trace
    }

    fn evaluate_kernel(
        &mut self,
        kernel_id: KernelId,
        input_subject: Option<&RuntimeValue>,
        environment: &[RuntimeValue],
        globals: &BTreeMap<ItemId, RuntimeValue>,
    ) -> Result<RuntimeValue, EvaluationError> {
        let started_at = self.jit_profile.as_ref().map(|_| Instant::now());
        let kernel = self
            .program
            .kernels()
            .get(kernel_id)
            .cloned()
            .ok_or(EvaluationError::UnknownKernel { kernel: kernel_id })?;
        if let Some((cached_result, cached_layout)) =
            self.last_kernel_call.as_ref().and_then(|last| {
                (last.kernel_id == kernel_id
                    && last.input_subject.as_ref() == input_subject
                    && last.environment.as_ref() == environment)
                    .then(|| (last.result.clone(), last.result_layout))
            })
        {
            self.record_kernel_profile(
                kernel_id,
                started_at.map_or(Duration::ZERO, |started| started.elapsed()),
                true,
            );
            if cached_layout != kernel.result_layout {
                return Err(EvaluationError::KernelResultLayoutMismatch {
                    kernel: kernel_id,
                    expected: kernel.result_layout,
                    found: cached_result,
                });
            }
            return Ok(cached_result);
        }

        let fingerprint = self.prepare_kernel_plan(kernel_id);
        let execution = {
            let plan = self
                .kernel_plans
                .get_mut(&fingerprint)
                .expect("prepared plan should remain cached");
            match plan {
                CachedKernelPlan::Compiled(compiled) => {
                    validate_compiled_inputs(
                        kernel_id,
                        &kernel,
                        compiled,
                        input_subject,
                        environment,
                    )?;
                    execute_compiled_kernel(
                        kernel_id,
                        compiled,
                        input_subject,
                        environment,
                        globals,
                    )
                }
                CachedKernelPlan::Fallback => Err(CompiledKernelFailure::Fallback),
            }
        };

        let result = match execution {
            Ok(result) => {
                self.record_kernel_profile(
                    kernel_id,
                    started_at.map_or(Duration::ZERO, |started| started.elapsed()),
                    false,
                );
                result
            }
            Err(CompiledKernelFailure::Evaluation(error)) => return Err(error),
            Err(CompiledKernelFailure::Fallback) => {
                self.kernel_plans
                    .insert(fingerprint, CachedKernelPlan::Fallback);
                let result = self.fallback.evaluate_kernel(
                    kernel_id,
                    input_subject,
                    environment,
                    globals,
                )?;
                self.refresh_profile();
                result
            }
        };

        self.last_kernel_call = Some(LastKernelCall {
            kernel_id,
            input_subject: input_subject.cloned(),
            environment: environment.to_vec().into_boxed_slice(),
            result: result.clone(),
            result_layout: kernel.result_layout,
        });
        Ok(result)
    }

    fn evaluate_signal_body_kernel(
        &mut self,
        kernel_id: KernelId,
        environment: &[RuntimeValue],
        globals: &BTreeMap<ItemId, RuntimeValue>,
    ) -> Result<RuntimeValue, EvaluationError> {
        let started_at = self.jit_profile.as_ref().map(|_| Instant::now());
        let kernel = self
            .program
            .kernels()
            .get(kernel_id)
            .cloned()
            .ok_or(EvaluationError::UnknownKernel { kernel: kernel_id })?;
        if let Some((cached_result, cached_layout)) =
            self.last_kernel_call.as_ref().and_then(|last| {
                (last.kernel_id == kernel_id
                    && last.input_subject.is_none()
                    && last.environment.as_ref() == environment)
                    .then(|| (last.result.clone(), last.result_layout))
            })
        {
            self.record_kernel_profile(
                kernel_id,
                started_at.map_or(Duration::ZERO, |started| started.elapsed()),
                true,
            );
            if cached_layout != kernel.result_layout {
                return Err(EvaluationError::KernelResultLayoutMismatch {
                    kernel: kernel_id,
                    expected: kernel.result_layout,
                    found: cached_result,
                });
            }
            return Ok(cached_result);
        }

        let fingerprint = self.prepare_kernel_plan(kernel_id);
        let execution = {
            let plan = self
                .kernel_plans
                .get_mut(&fingerprint)
                .expect("prepared plan should remain cached");
            match plan {
                CachedKernelPlan::Compiled(compiled) => {
                    validate_compiled_inputs(kernel_id, &kernel, compiled, None, environment)?;
                    execute_compiled_kernel(kernel_id, compiled, None, environment, globals)
                }
                CachedKernelPlan::Fallback => Err(CompiledKernelFailure::Fallback),
            }
        };

        let raw_result = match execution {
            Ok(result) => {
                self.record_kernel_profile(
                    kernel_id,
                    started_at.map_or(Duration::ZERO, |started| started.elapsed()),
                    false,
                );
                result
            }
            Err(CompiledKernelFailure::Evaluation(error)) => return Err(error),
            Err(CompiledKernelFailure::Fallback) => {
                self.kernel_plans
                    .insert(fingerprint, CachedKernelPlan::Fallback);
                let result = self
                    .fallback
                    .evaluate_signal_body_kernel(kernel_id, environment, globals)?;
                self.refresh_profile();
                result
            }
        };
        let result = crate::runtime::normalize_signal_kernel_result(
            self.program,
            kernel_id,
            raw_result,
            kernel.result_layout,
        )?;
        self.last_kernel_call = Some(LastKernelCall {
            kernel_id,
            input_subject: None,
            environment: environment.to_vec().into_boxed_slice(),
            result: result.clone(),
            result_layout: kernel.result_layout,
        });
        Ok(result)
    }

    fn apply_runtime_callable(
        &mut self,
        kernel_id: KernelId,
        callee: RuntimeValue,
        arguments: Vec<RuntimeValue>,
        globals: &BTreeMap<ItemId, RuntimeValue>,
    ) -> Result<RuntimeValue, EvaluationError> {
        let result = self
            .fallback
            .apply_runtime_callable(kernel_id, callee, arguments, globals);
        self.refresh_profile();
        result
    }

    fn subtract_runtime_values(
        &self,
        kernel_id: KernelId,
        left: RuntimeValue,
        right: RuntimeValue,
    ) -> Result<RuntimeValue, EvaluationError> {
        self.fallback
            .subtract_runtime_values(kernel_id, left, right)
    }

    fn evaluate_item(
        &mut self,
        item: ItemId,
        globals: &BTreeMap<ItemId, RuntimeValue>,
    ) -> Result<RuntimeValue, EvaluationError> {
        if let Some(value) = globals.get(&item) {
            return Ok(value.clone());
        }
        let started_at = self.jit_profile.as_ref().map(|_| Instant::now());
        if let Some(value) = self.item_cache.get(&item).cloned() {
            self.record_item_profile(
                item,
                started_at.map_or(Duration::ZERO, |started| started.elapsed()),
                true,
            );
            return Ok(value);
        }
        let item_decl = self
            .program
            .items()
            .get(item)
            .cloned()
            .ok_or(EvaluationError::UnknownItem { item })?;
        let kernel = item_decl
            .body
            .ok_or(EvaluationError::MissingItemBody { item })?;
        if !item_decl.parameters.is_empty() {
            return Ok(RuntimeValue::Callable(RuntimeCallable::ItemBody {
                item,
                kernel,
                parameters: item_decl.parameters.clone(),
                bound_arguments: Vec::new(),
            }));
        }
        if matches!(item_decl.kind, ItemKind::Signal(_)) {
            let result = self.fallback.evaluate_item(item, globals);
            self.refresh_profile();
            return result;
        }
        if !self.item_stack.insert(item) {
            self.record_item_profile(
                item,
                started_at.map_or(Duration::ZERO, |started| started.elapsed()),
                false,
            );
            return Err(EvaluationError::RecursiveItemEvaluation { item });
        }
        self.eval_trace.push(EvalFrame { item, kernel });
        let result = self.evaluate_kernel(kernel, None, &[], globals);
        self.item_stack.remove(&item);
        let result = match result {
            Ok(value) => {
                self.eval_trace.pop();
                value
            }
            Err(error) => {
                self.record_item_profile(
                    item,
                    started_at.map_or(Duration::ZERO, |started| started.elapsed()),
                    false,
                );
                return Err(error);
            }
        };
        self.record_item_profile(
            item,
            started_at.map_or(Duration::ZERO, |started| started.elapsed()),
            false,
        );
        self.item_cache.insert(item, result.clone());
        Ok(result)
    }
}

impl TaskFunctionApplier for LazyJitExecutionEngine<'_> {
    fn apply_task_function(
        &mut self,
        function: RuntimeValue,
        args: Vec<RuntimeValue>,
        globals: &BTreeMap<ItemId, RuntimeValue>,
    ) -> Result<RuntimeValue, EvaluationError> {
        self.apply_runtime_callable(TASK_COMPOSITION_KERNEL_ID, function, args, globals)
    }
}

#[cfg(test)]
mod tests {
    use super::LazyJitExecutionEngine;
    use crate::{
        BackendExecutionOptions, ItemId, ItemKind, Program, compute_kernel_fingerprint,
        lower_module as lower_backend_module, validate_program,
    };
    use aivi_base::SourceDatabase;
    use aivi_core::{lower_module as lower_core_module, validate_module as validate_core_module};
    use aivi_lambda::{lower_module as lower_lambda_module, validate_module as validate_lambda_module};
    use aivi_syntax::parse_module;

    fn lower_text(path: &str, text: &str) -> Program {
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
        let lambda = lower_lambda_module(&core).expect("typed lambda lowering should succeed");
        validate_lambda_module(&lambda).expect("typed lambda should validate before backend lowering");

        let backend = lower_backend_module(&lambda).expect("backend lowering should succeed");
        validate_program(&backend).expect("backend program should validate");
        backend
    }

    fn find_item(program: &Program, name: &str) -> ItemId {
        program
            .items()
            .iter()
            .find(|(_, item)| item.name.as_ref() == name)
            .map(|(id, _)| id)
            .unwrap_or_else(|| panic!("expected backend item `{name}`"))
    }

    #[test]
    fn default_jit_engine_keeps_signal_body_kernels_lazy() {
        let backend = lower_text(
            "jit-signal-body-lazy.aivi",
            r#"
signal base = 1
signal derived = base
"#,
        );
        let derived = find_item(&backend, "derived");
        let body_kernel = match &backend.items()[derived].kind {
            ItemKind::Signal(signal) => signal
                .body_kernel
                .expect("direct signal dependency should lower a body kernel"),
            other => panic!("expected signal item, found {other:?}"),
        };
        let fingerprint = compute_kernel_fingerprint(&backend, body_kernel);

        let engine = LazyJitExecutionEngine::new(&backend, BackendExecutionOptions::default());

        assert!(!engine.kernel_plans.contains_key(&fingerprint));
    }

    #[test]
    fn eager_signal_compilation_prepares_signal_body_kernels() {
        let backend = lower_text(
            "jit-signal-body-eager.aivi",
            r#"
signal base = 1
signal derived = base
"#,
        );
        let derived = find_item(&backend, "derived");
        let body_kernel = match &backend.items()[derived].kind {
            ItemKind::Signal(signal) => signal
                .body_kernel
                .expect("direct signal dependency should lower a body kernel"),
            other => panic!("expected signal item, found {other:?}"),
        };
        let fingerprint = compute_kernel_fingerprint(&backend, body_kernel);

        let engine = LazyJitExecutionEngine::new(
            &backend,
            BackendExecutionOptions {
                eagerly_compile_signals: true,
            },
        );

        assert!(engine.kernel_plans.contains_key(&fingerprint));
    }
}

fn validate_compiled_inputs(
    kernel_id: KernelId,
    kernel: &crate::Kernel,
    plan: &CompiledKernelPlan,
    input_subject: Option<&RuntimeValue>,
    environment: &[RuntimeValue],
) -> Result<(), EvaluationError> {
    match (&plan.input_plan, input_subject) {
        (Some(expected), Some(value)) if expected.matches(value) => {}
        (Some(_), Some(value)) => {
            return Err(EvaluationError::KernelInputLayoutMismatch {
                kernel: kernel_id,
                expected: kernel
                    .input_subject
                    .expect("compiled subject plan implies subject"),
                found: value.clone(),
            });
        }
        (Some(_), None) => {
            return Err(EvaluationError::MissingInputSubject { kernel: kernel_id });
        }
        (None, Some(_)) => {
            return Err(EvaluationError::UnexpectedInputSubject { kernel: kernel_id });
        }
        (None, None) => {}
    }
    if environment.len() != plan.environment_plans.len() {
        return Err(EvaluationError::KernelEnvironmentCountMismatch {
            kernel: kernel_id,
            expected: plan.environment_plans.len(),
            found: environment.len(),
        });
    }
    for (index, (expected, value)) in plan
        .environment_plans
        .iter()
        .zip(environment.iter())
        .enumerate()
    {
        if !expected.matches(value) {
            return Err(EvaluationError::KernelEnvironmentLayoutMismatch {
                kernel: kernel_id,
                slot: crate::EnvSlotId::from_raw(index as u32),
                expected: kernel.environment[index],
                found: value.clone(),
            });
        }
    }
    Ok(())
}

fn execute_compiled_kernel(
    kernel_id: KernelId,
    plan: &mut CompiledKernelPlan,
    input_subject: Option<&RuntimeValue>,
    environment: &[RuntimeValue],
    globals: &BTreeMap<ItemId, RuntimeValue>,
) -> Result<RuntimeValue, CompiledKernelFailure> {
    let arena = Rc::new(RefCell::new(AllocationArena::new()));
    {
        let mut arena_mut = arena.borrow_mut();
        for (slot, slot_plan) in plan
            .artifact
            .signal_slots
            .iter_mut()
            .zip(plan.signal_slot_plans.iter())
        {
            let value = globals
                .get(&slot.item)
                .ok_or(CompiledKernelFailure::Fallback)?;
            if !slot_plan.write_slot(strip_signal_wrappers(value), &mut slot.cell, &mut arena_mut) {
                return Err(CompiledKernelFailure::Fallback);
            }
        }
        for (slot, slot_plan) in plan
            .artifact
            .imported_item_slots
            .iter_mut()
            .zip(plan.imported_item_slot_plans.iter())
        {
            let value = globals
                .get(&slot.item)
                .ok_or(CompiledKernelFailure::Fallback)?;
            if !slot_plan.write_slot(strip_signal_wrappers(value), &mut slot.cell, &mut arena_mut) {
                return Err(CompiledKernelFailure::Fallback);
            }
        }
    }

    let mut args =
        Vec::with_capacity(plan.environment_plans.len() + usize::from(plan.input_plan.is_some()));
    {
        let mut arena_mut = arena.borrow_mut();
        if let Some(input_plan) = &plan.input_plan {
            let value = input_subject.ok_or(CompiledKernelFailure::Evaluation(
                EvaluationError::MissingInputSubject { kernel: kernel_id },
            ))?;
            let Some(arg) = input_plan.pack_argument(value, &mut arena_mut) else {
                return Err(CompiledKernelFailure::Fallback);
            };
            args.push(arg);
        }
        for (slot_plan, value) in plan.environment_plans.iter().zip(environment.iter()) {
            let Some(arg) = slot_plan.pack_argument(value, &mut arena_mut) else {
                return Err(CompiledKernelFailure::Fallback);
            };
            args.push(arg);
        }
    }

    let call_result = with_active_arena(Rc::clone(&arena), || {
        plan.artifact.caller.call(plan.artifact.function, &args)
    })
    .map_err(|_| CompiledKernelFailure::Fallback)?;
    plan.result_plan
        .unpack_result(call_result)
        .ok_or(CompiledKernelFailure::Fallback)
}

enum CachedKernelPlan {
    Compiled(CompiledKernelPlan),
    Fallback,
}

impl CachedKernelPlan {
    fn build(program: &Program, kernel_id: KernelId) -> Self {
        let Some(compiled) = CompiledKernelPlan::compile(program, kernel_id) else {
            return Self::Fallback;
        };
        Self::Compiled(compiled)
    }
}

struct CompiledKernelPlan {
    artifact: CompiledJitKernel,
    input_plan: Option<MarshalPlan>,
    environment_plans: Vec<MarshalPlan>,
    result_plan: MarshalPlan,
    signal_slot_plans: Vec<MarshalPlan>,
    imported_item_slot_plans: Vec<MarshalPlan>,
}

impl CompiledKernelPlan {
    fn compile(program: &Program, kernel_id: KernelId) -> Option<Self> {
        let kernel = &program.kernels()[kernel_id];
        let input_plan = match kernel.input_subject {
            Some(layout) => Some(MarshalPlan::for_layout(program, layout)?),
            None => None,
        };
        let environment_plans = kernel
            .environment
            .iter()
            .map(|layout| MarshalPlan::for_layout(program, *layout))
            .collect::<Option<Vec<_>>>()?;
        let result_plan = MarshalPlan::for_layout(program, kernel.result_layout)?;
        let artifact = compile_kernel_jit_cached(program, kernel_id).ok()?;
        let signal_slot_plans = artifact
            .signal_slots
            .iter()
            .map(|slot| MarshalPlan::for_layout(program, slot.layout))
            .collect::<Option<Vec<_>>>()?;
        let imported_item_slot_plans = artifact
            .imported_item_slots
            .iter()
            .map(|slot| MarshalPlan::for_layout(program, slot.layout))
            .collect::<Option<Vec<_>>>()?;
        Some(Self {
            artifact,
            input_plan,
            environment_plans,
            result_plan,
            signal_slot_plans,
            imported_item_slot_plans,
        })
    }
}

#[derive(Clone, Copy)]
enum MarshalPlan {
    Int,
    Float,
    Bool,
    Text,
    Bytes,
    OptionInt,
    OptionFloat,
    OptionBool,
    OptionText,
    OptionBytes,
}

impl MarshalPlan {
    fn for_layout(program: &Program, layout: LayoutId) -> Option<Self> {
        match &program.layouts().get(layout)?.kind {
            LayoutKind::Primitive(PrimitiveType::Int) => Some(Self::Int),
            LayoutKind::Primitive(PrimitiveType::Float) => Some(Self::Float),
            LayoutKind::Primitive(PrimitiveType::Bool) => Some(Self::Bool),
            LayoutKind::Primitive(PrimitiveType::Text) => Some(Self::Text),
            LayoutKind::Primitive(PrimitiveType::Bytes) => Some(Self::Bytes),
            LayoutKind::Option { element } => match &program.layouts().get(*element)?.kind {
                LayoutKind::Primitive(PrimitiveType::Int) => Some(Self::OptionInt),
                LayoutKind::Primitive(PrimitiveType::Float) => Some(Self::OptionFloat),
                LayoutKind::Primitive(PrimitiveType::Bool) => Some(Self::OptionBool),
                LayoutKind::Primitive(PrimitiveType::Text) => Some(Self::OptionText),
                LayoutKind::Primitive(PrimitiveType::Bytes) => Some(Self::OptionBytes),
                _ => None,
            },
            _ => None,
        }
    }

    fn matches(self, value: &RuntimeValue) -> bool {
        match (self, value) {
            (Self::Int, RuntimeValue::Int(_))
            | (Self::Float, RuntimeValue::Float(_))
            | (Self::Bool, RuntimeValue::Bool(_))
            | (Self::Text, RuntimeValue::Text(_))
            | (Self::Bytes, RuntimeValue::Bytes(_))
            | (Self::OptionInt, RuntimeValue::OptionNone)
            | (Self::OptionFloat, RuntimeValue::OptionNone)
            | (Self::OptionBool, RuntimeValue::OptionNone)
            | (Self::OptionText, RuntimeValue::OptionNone)
            | (Self::OptionBytes, RuntimeValue::OptionNone) => true,
            (Self::OptionInt, RuntimeValue::OptionSome(value)) => {
                matches!(value.as_ref(), RuntimeValue::Int(_))
            }
            (Self::OptionFloat, RuntimeValue::OptionSome(value)) => {
                matches!(value.as_ref(), RuntimeValue::Float(_))
            }
            (Self::OptionBool, RuntimeValue::OptionSome(value)) => {
                matches!(value.as_ref(), RuntimeValue::Bool(_))
            }
            (Self::OptionText, RuntimeValue::OptionSome(value)) => {
                matches!(value.as_ref(), RuntimeValue::Text(_))
            }
            (Self::OptionBytes, RuntimeValue::OptionSome(value)) => {
                matches!(value.as_ref(), RuntimeValue::Bytes(_))
            }
            _ => false,
        }
    }

    fn pack_argument(self, value: &RuntimeValue, arena: &mut AllocationArena) -> Option<AbiValue> {
        match (self, value) {
            (Self::Int, RuntimeValue::Int(value)) => Some(AbiValue::I64(*value)),
            (Self::Float, RuntimeValue::Float(value)) => Some(AbiValue::F64(value.to_f64())),
            (Self::Bool, RuntimeValue::Bool(value)) => Some(AbiValue::I8(i8::from(*value))),
            (Self::Text, RuntimeValue::Text(value)) => Some(AbiValue::Pointer(
                encode_len_prefixed_bytes(value.as_bytes(), arena),
            )),
            (Self::Bytes, RuntimeValue::Bytes(value)) => Some(AbiValue::Pointer(
                encode_len_prefixed_bytes(value.as_ref(), arena),
            )),
            (Self::OptionInt, RuntimeValue::OptionNone)
            | (Self::OptionFloat, RuntimeValue::OptionNone)
            | (Self::OptionBool, RuntimeValue::OptionNone) => Some(AbiValue::I128(0)),
            (Self::OptionText, RuntimeValue::OptionNone)
            | (Self::OptionBytes, RuntimeValue::OptionNone) => Some(AbiValue::Pointer(ptr::null())),
            (Self::OptionInt, RuntimeValue::OptionSome(value)) => match value.as_ref() {
                RuntimeValue::Int(value) => {
                    Some(AbiValue::I128(encode_inline_option_bits(*value as u64)))
                }
                _ => None,
            },
            (Self::OptionFloat, RuntimeValue::OptionSome(value)) => match value.as_ref() {
                RuntimeValue::Float(value) => Some(AbiValue::I128(encode_inline_option_bits(
                    value.to_f64().to_bits(),
                ))),
                _ => None,
            },
            (Self::OptionBool, RuntimeValue::OptionSome(value)) => match value.as_ref() {
                RuntimeValue::Bool(value) => {
                    Some(AbiValue::I128(encode_inline_option_bits(u64::from(*value))))
                }
                _ => None,
            },
            (Self::OptionText, RuntimeValue::OptionSome(value)) => match value.as_ref() {
                RuntimeValue::Text(value) => Some(AbiValue::Pointer(encode_len_prefixed_bytes(
                    value.as_bytes(),
                    arena,
                ))),
                _ => None,
            },
            (Self::OptionBytes, RuntimeValue::OptionSome(value)) => match value.as_ref() {
                RuntimeValue::Bytes(value) => Some(AbiValue::Pointer(encode_len_prefixed_bytes(
                    value.as_ref(),
                    arena,
                ))),
                _ => None,
            },
            _ => None,
        }
    }

    fn write_slot(
        self,
        value: &RuntimeValue,
        cell: &mut [u8],
        arena: &mut AllocationArena,
    ) -> bool {
        cell.fill(0);
        match (self, value) {
            (Self::Int, RuntimeValue::Int(value)) => {
                cell[..8].copy_from_slice(&value.to_ne_bytes());
                true
            }
            (Self::Float, RuntimeValue::Float(value)) => {
                cell[..8].copy_from_slice(&value.to_f64().to_bits().to_ne_bytes());
                true
            }
            (Self::Bool, RuntimeValue::Bool(value)) => {
                cell[0] = u8::from(*value);
                true
            }
            (Self::Text, RuntimeValue::Text(value)) => {
                write_pointer_cell(cell, encode_len_prefixed_bytes(value.as_bytes(), arena))
            }
            (Self::Bytes, RuntimeValue::Bytes(value)) => {
                write_pointer_cell(cell, encode_len_prefixed_bytes(value.as_ref(), arena))
            }
            (Self::OptionInt, RuntimeValue::OptionNone)
            | (Self::OptionFloat, RuntimeValue::OptionNone)
            | (Self::OptionBool, RuntimeValue::OptionNone) => write_i128_cell(cell, 0),
            (Self::OptionText, RuntimeValue::OptionNone)
            | (Self::OptionBytes, RuntimeValue::OptionNone) => {
                write_pointer_cell(cell, ptr::null())
            }
            (Self::OptionInt, RuntimeValue::OptionSome(value)) => match value.as_ref() {
                RuntimeValue::Int(value) => {
                    write_i128_cell(cell, encode_inline_option_bits(*value as u64))
                }
                _ => false,
            },
            (Self::OptionFloat, RuntimeValue::OptionSome(value)) => match value.as_ref() {
                RuntimeValue::Float(value) => {
                    write_i128_cell(cell, encode_inline_option_bits(value.to_f64().to_bits()))
                }
                _ => false,
            },
            (Self::OptionBool, RuntimeValue::OptionSome(value)) => match value.as_ref() {
                RuntimeValue::Bool(value) => {
                    write_i128_cell(cell, encode_inline_option_bits(u64::from(*value)))
                }
                _ => false,
            },
            (Self::OptionText, RuntimeValue::OptionSome(value)) => match value.as_ref() {
                RuntimeValue::Text(value) => {
                    write_pointer_cell(cell, encode_len_prefixed_bytes(value.as_bytes(), arena))
                }
                _ => false,
            },
            (Self::OptionBytes, RuntimeValue::OptionSome(value)) => match value.as_ref() {
                RuntimeValue::Bytes(value) => {
                    write_pointer_cell(cell, encode_len_prefixed_bytes(value.as_ref(), arena))
                }
                _ => false,
            },
            _ => false,
        }
    }

    fn unpack_result(self, value: AbiValue) -> Option<RuntimeValue> {
        match (self, value) {
            (Self::Int, AbiValue::I64(value)) => Some(RuntimeValue::Int(value)),
            (Self::Float, AbiValue::F64(value)) => {
                Some(RuntimeValue::Float(RuntimeFloat::new(value)?))
            }
            (Self::Bool, AbiValue::I8(value)) => Some(RuntimeValue::Bool(value != 0)),
            (Self::OptionInt, AbiValue::I128(bits)) => decode_inline_int_option(bits),
            (Self::OptionFloat, AbiValue::I128(bits)) => decode_inline_float_option(bits),
            (Self::OptionBool, AbiValue::I128(bits)) => decode_inline_bool_option(bits),
            (Self::Text, AbiValue::Pointer(value)) => decode_text(value),
            (Self::Bytes, AbiValue::Pointer(value)) => {
                Some(RuntimeValue::Bytes(decode_len_prefixed_bytes(value)?))
            }
            (Self::OptionText, AbiValue::Pointer(value)) => {
                if value.is_null() {
                    Some(RuntimeValue::OptionNone)
                } else {
                    Some(RuntimeValue::OptionSome(Box::new(decode_text(value)?)))
                }
            }
            (Self::OptionBytes, AbiValue::Pointer(value)) => {
                if value.is_null() {
                    Some(RuntimeValue::OptionNone)
                } else {
                    Some(RuntimeValue::OptionSome(Box::new(RuntimeValue::Bytes(
                        decode_len_prefixed_bytes(value)?,
                    ))))
                }
            }
            _ => None,
        }
    }
}

#[derive(Clone)]
struct LastKernelCall {
    kernel_id: KernelId,
    input_subject: Option<RuntimeValue>,
    environment: Box<[RuntimeValue]>,
    result: RuntimeValue,
    result_layout: LayoutId,
}

enum CompiledKernelFailure {
    Evaluation(EvaluationError),
    Fallback,
}

fn decode_text(pointer: *const c_void) -> Option<RuntimeValue> {
    let bytes = decode_len_prefixed_bytes(pointer)?;
    let text = String::from_utf8(bytes.into_vec()).ok()?;
    Some(RuntimeValue::Text(text.into_boxed_str()))
}

fn strip_signal_wrappers(mut value: &RuntimeValue) -> &RuntimeValue {
    while let RuntimeValue::Signal(inner) = value {
        value = inner.as_ref();
    }
    value
}

fn write_pointer_cell(cell: &mut [u8], pointer: *const c_void) -> bool {
    let bytes = (pointer as usize).to_ne_bytes();
    if cell.len() < bytes.len() {
        return false;
    }
    cell[..bytes.len()].copy_from_slice(&bytes);
    true
}

fn write_i128_cell(cell: &mut [u8], bits: u128) -> bool {
    let bytes = bits.to_ne_bytes();
    if cell.len() < bytes.len() {
        return false;
    }
    cell[..bytes.len()].copy_from_slice(&bytes);
    true
}

const fn encode_inline_option_bits(payload: u64) -> u128 {
    ((payload as u128) << 64) | 1
}

fn decode_inline_int_option(bits: u128) -> Option<RuntimeValue> {
    if (bits as u64) == 0 {
        return Some(RuntimeValue::OptionNone);
    }
    Some(RuntimeValue::OptionSome(Box::new(RuntimeValue::Int(
        (bits >> 64) as u64 as i64,
    ))))
}

fn decode_inline_float_option(bits: u128) -> Option<RuntimeValue> {
    if (bits as u64) == 0 {
        return Some(RuntimeValue::OptionNone);
    }
    Some(RuntimeValue::OptionSome(Box::new(RuntimeValue::Float(
        RuntimeFloat::new(f64::from_bits((bits >> 64) as u64))?,
    ))))
}

fn decode_inline_bool_option(bits: u128) -> Option<RuntimeValue> {
    if (bits as u64) == 0 {
        return Some(RuntimeValue::OptionNone);
    }
    Some(RuntimeValue::OptionSome(Box::new(RuntimeValue::Bool(
        ((bits >> 64) as u64) != 0,
    ))))
}

fn record_call(profile: &mut EvaluationCallProfile, elapsed: Duration, cache_hit: bool) {
    profile.calls += 1;
    if cache_hit {
        profile.cache_hits += 1;
    }
    profile.total_time += elapsed;
    profile.max_time = profile.max_time.max(elapsed);
}
