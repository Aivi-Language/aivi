mod builtins;
mod values;

pub use builtins::get_builtin;
pub use values::ClosureValue;
pub use values::KeyValue;
pub use values::{
    format_value, values_equal, Builtin, BuiltinImpl, BuiltinValue, EffectValue, ResourceValue,
    Runtime, RuntimeContext, RuntimeError, SourceValue, Value,
};

pub type R = Result<Value, RuntimeError>;

pub fn ok(value: Value) -> R {
    Ok(value)
}
