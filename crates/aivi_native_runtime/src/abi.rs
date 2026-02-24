use std::ffi::c_void;
use std::fmt;
use std::sync::Arc;

use crate::{RuntimeError, Value};

pub const AIVI_VALUE_ABI_MAJOR: u16 = 0;
pub const AIVI_VALUE_ABI_MINOR: u16 = 1;
pub const AIVI_VALUE_ABI_PATCH: u16 = 0;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AiviAbiVersion {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

pub const AIVI_VALUE_ABI_VERSION: AiviAbiVersion = AiviAbiVersion {
    major: AIVI_VALUE_ABI_MAJOR,
    minor: AIVI_VALUE_ABI_MINOR,
    patch: AIVI_VALUE_ABI_PATCH,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AbiCompatibilityError {
    pub required: AiviAbiVersion,
    pub runtime: AiviAbiVersion,
    detail: &'static str,
}

impl fmt::Display for AbiCompatibilityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "AIVI value ABI mismatch: {detail} (required {}.{}.{}, runtime {}.{}.{})",
            self.required.major,
            self.required.minor,
            self.required.patch,
            self.runtime.major,
            self.runtime.minor,
            self.runtime.patch,
            detail = self.detail
        )
    }
}

impl std::error::Error for AbiCompatibilityError {}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AiviValueHandle {
    pub raw: *const c_void,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AiviErrorHandle {
    pub raw: *const c_void,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AiviEffectHandle {
    pub raw: *const c_void,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AiviRefHandle {
    pub raw: *const c_void,
}

impl AiviValueHandle {
    pub const NULL: Self = Self {
        raw: std::ptr::null(),
    };

    pub const fn is_null(self) -> bool {
        self.raw.is_null()
    }
}

impl AiviErrorHandle {
    pub const NULL: Self = Self {
        raw: std::ptr::null(),
    };

    pub const fn is_null(self) -> bool {
        self.raw.is_null()
    }
}

impl AiviEffectHandle {
    pub const NULL: Self = Self {
        raw: std::ptr::null(),
    };

    pub const fn is_null(self) -> bool {
        self.raw.is_null()
    }
}

impl AiviRefHandle {
    pub const NULL: Self = Self {
        raw: std::ptr::null(),
    };

    pub const fn is_null(self) -> bool {
        self.raw.is_null()
    }
}

pub fn runtime_value_abi_version() -> AiviAbiVersion {
    AIVI_VALUE_ABI_VERSION
}

pub fn check_value_abi_compat(required: AiviAbiVersion) -> Result<(), AbiCompatibilityError> {
    let runtime = runtime_value_abi_version();
    if required.major != runtime.major {
        return Err(AbiCompatibilityError {
            required,
            runtime,
            detail: "major version differs",
        });
    }
    if required.minor > runtime.minor {
        return Err(AbiCompatibilityError {
            required,
            runtime,
            detail: "required minor version is newer than runtime",
        });
    }
    Ok(())
}

pub fn runtime_value_abi_handshake(
    required_major: u16,
    required_minor: u16,
) -> Result<(), RuntimeError> {
    check_value_abi_compat(AiviAbiVersion {
        major: required_major,
        minor: required_minor,
        patch: 0,
    })
    .map_err(|err| RuntimeError::Message(err.to_string()))
}

pub fn value_handle_new(value: Value) -> AiviValueHandle {
    AiviValueHandle {
        raw: Arc::into_raw(Arc::new(value)).cast::<c_void>(),
    }
}

pub fn value_handle_retain(handle: AiviValueHandle) -> AiviValueHandle {
    if handle.is_null() {
        return AiviValueHandle::NULL;
    }
    // SAFETY: non-null handles are created from Arc::into_raw in this module.
    let arc = unsafe { Arc::from_raw(handle.raw.cast::<Value>()) };
    let cloned = Arc::clone(&arc);
    let _ = Arc::into_raw(arc);
    AiviValueHandle {
        raw: Arc::into_raw(cloned).cast::<c_void>(),
    }
}

pub fn value_handle_release(handle: AiviValueHandle) {
    if handle.is_null() {
        return;
    }
    // SAFETY: each release consumes one reference previously created via Arc::into_raw.
    unsafe {
        drop(Arc::from_raw(handle.raw.cast::<Value>()));
    }
}

pub fn value_handle_clone(handle: AiviValueHandle) -> AiviValueHandle {
    let Some(value) = value_handle_read_clone(handle) else {
        return AiviValueHandle::NULL;
    };
    value_handle_new(value)
}

pub fn value_handle_read_clone(handle: AiviValueHandle) -> Option<Value> {
    if handle.is_null() {
        return None;
    }
    // SAFETY: non-null handles are created from Arc::into_raw in this module.
    let arc = unsafe { Arc::from_raw(handle.raw.cast::<Value>()) };
    let value = arc.as_ref().clone();
    let _ = Arc::into_raw(arc);
    Some(value)
}

#[no_mangle]
pub extern "C" fn aivi_value_abi_runtime_version() -> AiviAbiVersion {
    runtime_value_abi_version()
}

#[no_mangle]
pub extern "C" fn aivi_value_abi_is_compatible(required_major: u16, required_minor: u16) -> u8 {
    check_value_abi_compat(AiviAbiVersion {
        major: required_major,
        minor: required_minor,
        patch: 0,
    })
    .is_ok() as u8
}

#[no_mangle]
pub extern "C" fn aivi_value_handle_retain(handle: AiviValueHandle) -> AiviValueHandle {
    value_handle_retain(handle)
}

#[no_mangle]
pub extern "C" fn aivi_value_handle_release(handle: AiviValueHandle) {
    value_handle_release(handle);
}

#[no_mangle]
pub extern "C" fn aivi_value_handle_clone(handle: AiviValueHandle) -> AiviValueHandle {
    value_handle_clone(handle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abi_compat_allows_same_major_and_older_minor() {
        let required = AiviAbiVersion {
            major: AIVI_VALUE_ABI_MAJOR,
            minor: AIVI_VALUE_ABI_MINOR.saturating_sub(1),
            patch: 7,
        };
        assert!(check_value_abi_compat(required).is_ok());
    }

    #[test]
    fn abi_compat_rejects_major_mismatch() {
        let required = AiviAbiVersion {
            major: AIVI_VALUE_ABI_MAJOR + 1,
            minor: 0,
            patch: 0,
        };
        let err = check_value_abi_compat(required).expect_err("major mismatch should fail");
        assert!(err.to_string().contains("major version differs"));
    }

    #[test]
    fn retain_release_keeps_value_alive() {
        let h1 = value_handle_new(Value::Int(42));
        let h2 = value_handle_retain(h1);
        value_handle_release(h1);
        assert!(matches!(value_handle_read_clone(h2), Some(Value::Int(42))));
        value_handle_release(h2);
    }

    #[test]
    fn clone_returns_independent_handle() {
        let h1 = value_handle_new(Value::Text("abc".to_string()));
        let h2 = value_handle_clone(h1);
        assert!(!h1.is_null());
        assert!(!h2.is_null());
        assert_ne!(h1.raw, h2.raw);
        value_handle_release(h1);
        assert!(matches!(
            value_handle_read_clone(h2),
            Some(Value::Text(text)) if text == "abc"
        ));
        value_handle_release(h2);
    }
}
