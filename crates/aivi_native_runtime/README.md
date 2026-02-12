# aivi_native_runtime

Runtime support crate for AIVI's experimental "native" Rust backend.

The native backend lowers AIVI programs to standalone Rust that depends on this crate for:
- the `Value` model and application semantics
- effects/resources execution (`EffectValue`, `ResourceValue`, cancellation)
- builtin implementations (`get_builtin`)

This crate is not intended to be used directly by end users yet; its API is driven by the generated
Rust output.

