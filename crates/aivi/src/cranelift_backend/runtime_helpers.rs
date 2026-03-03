//! `extern "C"` runtime helpers callable from Cranelift JIT-compiled code.
//!
//! Every helper receives `*mut JitRuntimeCtx` as its first argument.
//! Non-scalar values are passed/returned as `*mut Value` (heap-boxed).

use std::collections::HashMap;
use std::sync::Arc;

use crate::runtime::values::Value;
use crate::runtime::RuntimeError;

use super::abi::{self, JitRuntimeCtx};

include!("runtime_helpers/core.rs");
include!("runtime_helpers/boxing.rs");
include!("runtime_helpers/values.rs");
include!("runtime_helpers/interaction.rs");
include!("runtime_helpers/patterns.rs");
include!("runtime_helpers/generate.rs");
include!("runtime_helpers/aot.rs");
