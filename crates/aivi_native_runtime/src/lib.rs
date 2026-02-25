mod abi;
mod builtins;
mod values;

pub use abi::{
    check_value_abi_compat, runtime_value_abi_handshake, runtime_value_abi_version,
    value_handle_clone, value_handle_new, value_handle_read_clone, value_handle_release,
    value_handle_retain, AbiCompatibilityError, AiviAbiVersion, AiviEffectHandle, AiviErrorHandle,
    AiviRefHandle, AiviValueHandle, AIVI_VALUE_ABI_MAJOR, AIVI_VALUE_ABI_MINOR,
    AIVI_VALUE_ABI_PATCH, AIVI_VALUE_ABI_VERSION,
};
pub use builtins::get_builtin;
pub use im::HashMap as ImHashMap;
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
