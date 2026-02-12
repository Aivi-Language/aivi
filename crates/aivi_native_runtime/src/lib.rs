mod builtins;
mod values;

pub use builtins::get_builtin;
pub use values::{format_value, values_equal, Builtin, BuiltinImpl, BuiltinValue, EffectValue, Runtime, RuntimeContext, RuntimeError, Value};
pub use values::ClosureValue;

pub type R = Result<Value, RuntimeError>;

pub fn ok(value: Value) -> R {
    Ok(value)
}
