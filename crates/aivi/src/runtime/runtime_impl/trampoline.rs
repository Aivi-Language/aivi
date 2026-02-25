// Trampoline removed: Cranelift JIT handles all execution.
// The trampoline was the interpreter's stack-safe evaluation loop for HIR expressions.
// With Cranelift, apply() and run_effect_value() work directly without a trampoline.
