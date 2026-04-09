use std::{cell::RefCell, ffi::c_void, ptr, rc::Rc, slice};

use libffi::middle::{Arg, Cif, CodePtr, Type};

thread_local! {
    static ACTIVE_ARENA: RefCell<Option<Rc<RefCell<AllocationArena>>>> = const { RefCell::new(None) };
}

#[derive(Debug, Default)]
pub struct AllocationArena {
    allocations: Vec<Box<[u8]>>,
}

impl AllocationArena {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn store_len_prefixed_bytes(&mut self, bytes: &[u8]) -> *const c_void {
        let mut encoded = Vec::with_capacity(8 + bytes.len());
        encoded.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
        encoded.extend_from_slice(bytes);
        let cell = encoded.into_boxed_slice();
        let pointer = cell.as_ptr();
        self.allocations.push(cell);
        pointer.cast()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AbiValueKind {
    I8,
    I64,
    I128,
    F64,
    Pointer,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AbiValue {
    I8(i8),
    I64(i64),
    I128(u128),
    F64(f64),
    Pointer(*const c_void),
}

impl AbiValue {
    pub const fn kind(self) -> AbiValueKind {
        match self {
            Self::I8(_) => AbiValueKind::I8,
            Self::I64(_) => AbiValueKind::I64,
            Self::I128(_) => AbiValueKind::I128,
            Self::F64(_) => AbiValueKind::F64,
            Self::Pointer(_) => AbiValueKind::Pointer,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallSignature {
    args: Box<[AbiValueKind]>,
    result: AbiValueKind,
}

impl CallSignature {
    pub fn new(args: impl Into<Box<[AbiValueKind]>>, result: AbiValueKind) -> Self {
        Self {
            args: args.into(),
            result,
        }
    }

    pub fn args(&self) -> &[AbiValueKind] {
        &self.args
    }

    pub const fn result(&self) -> AbiValueKind {
        self.result
    }
}

#[derive(Debug)]
pub struct FunctionCaller {
    signature: CallSignature,
    cif: Cif,
}

impl FunctionCaller {
    pub fn new(signature: CallSignature) -> Self {
        let arg_types = signature.args.iter().copied().map(type_for_abi_kind);
        let cif = Cif::new(arg_types, type_for_abi_kind(signature.result));
        Self { signature, cif }
    }

    pub fn signature(&self) -> &CallSignature {
        &self.signature
    }

    pub fn call(&self, function: *const u8, args: &[AbiValue]) -> Result<AbiValue, CallError> {
        if args.len() != self.signature.args.len() {
            return Err(CallError::ArityMismatch {
                expected: self.signature.args.len(),
                found: args.len(),
            });
        }

        let mut owned_args = Vec::with_capacity(args.len());
        for (index, (value, kind)) in args
            .iter()
            .copied()
            .zip(self.signature.args.iter().copied())
            .enumerate()
        {
            owned_args.push(OwnedArg::new(value, kind).map_err(|found| {
                CallError::ArgumentTypeMismatch {
                    index,
                    expected: kind,
                    found,
                }
            })?);
        }

        let ffi_args: Vec<_> = owned_args.iter().map(OwnedArg::as_arg).collect();
        let code_ptr = CodePtr(function as *mut c_void);
        let result = match self.signature.result {
            AbiValueKind::I8 => {
                // SAFETY: `FunctionCaller` only constructs a CIF from the stored signature,
                // and `OwnedArg::new` ensures the runtime arguments match that signature.
                AbiValue::I8(unsafe { self.cif.call::<i8>(code_ptr, &ffi_args) })
            }
            AbiValueKind::I64 => {
                // SAFETY: same reasoning as above.
                AbiValue::I64(unsafe { self.cif.call::<i64>(code_ptr, &ffi_args) })
            }
            AbiValueKind::I128 => {
                // SAFETY: same reasoning as above.
                AbiValue::I128(unsafe { self.cif.call::<AbiI128Repr>(code_ptr, &ffi_args) }.bits())
            }
            AbiValueKind::F64 => {
                // SAFETY: same reasoning as above.
                AbiValue::F64(unsafe { self.cif.call::<f64>(code_ptr, &ffi_args) })
            }
            AbiValueKind::Pointer => {
                // SAFETY: same reasoning as above.
                AbiValue::Pointer(unsafe { self.cif.call::<*const c_void>(code_ptr, &ffi_args) })
            }
        };
        Ok(result)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CallError {
    ArityMismatch {
        expected: usize,
        found: usize,
    },
    ArgumentTypeMismatch {
        index: usize,
        expected: AbiValueKind,
        found: AbiValueKind,
    },
}

impl std::fmt::Display for CallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ArityMismatch { expected, found } => write!(
                f,
                "foreign call received {found} argument(s), expected {expected}"
            ),
            Self::ArgumentTypeMismatch {
                index,
                expected,
                found,
            } => write!(
                f,
                "foreign call argument {} had ABI kind {found:?}, expected {expected:?}",
                index + 1
            ),
        }
    }
}

impl std::error::Error for CallError {}

pub fn with_active_arena<R>(arena: Rc<RefCell<AllocationArena>>, f: impl FnOnce() -> R) -> R {
    ACTIVE_ARENA.with(|slot| {
        let previous = slot.replace(Some(arena));
        let result = f();
        slot.replace(previous);
        result
    })
}

pub fn encode_len_prefixed_bytes(bytes: &[u8], arena: &mut AllocationArena) -> *const c_void {
    arena.store_len_prefixed_bytes(bytes)
}

pub fn decode_len_prefixed_bytes(pointer: *const c_void) -> Option<Box<[u8]>> {
    // SAFETY: callers only hand us pointers produced by the AIVI backend's byte-sequence
    // contract (u64 little-endian length prefix followed by that many bytes) or null.
    unsafe {
        let bytes = read_len_prefixed_bytes(pointer.cast())?;
        Some(bytes.into())
    }
}

pub fn lookup_runtime_symbol(symbol: &str) -> Option<*const u8> {
    match symbol {
        "aivi_text_concat" => Some(aivi_text_concat as *const () as *const u8),
        "aivi_bytes_append" => Some(aivi_bytes_append as *const () as *const u8),
        "aivi_bytes_repeat" => Some(aivi_bytes_repeat as *const () as *const u8),
        "aivi_bytes_slice" => Some(aivi_bytes_slice as *const () as *const u8),
        _ => None,
    }
}

fn type_for_abi_kind(kind: AbiValueKind) -> Type {
    match kind {
        AbiValueKind::I8 => Type::i8(),
        AbiValueKind::I64 => Type::i64(),
        AbiValueKind::I128 => Type::structure([Type::u64(), Type::u64()]),
        AbiValueKind::F64 => Type::f64(),
        AbiValueKind::Pointer => Type::pointer(),
    }
}

#[derive(Clone, Copy, Debug)]
enum OwnedArg {
    I8(i8),
    I64(i64),
    I128(AbiI128Repr),
    F64(f64),
    Pointer(*const c_void),
}

impl OwnedArg {
    fn new(value: AbiValue, expected: AbiValueKind) -> Result<Self, AbiValueKind> {
        match (value, expected) {
            (AbiValue::I8(value), AbiValueKind::I8) => Ok(Self::I8(value)),
            (AbiValue::I64(value), AbiValueKind::I64) => Ok(Self::I64(value)),
            (AbiValue::I128(value), AbiValueKind::I128) => {
                Ok(Self::I128(AbiI128Repr::from_bits(value)))
            }
            (AbiValue::F64(value), AbiValueKind::F64) => Ok(Self::F64(value)),
            (AbiValue::Pointer(value), AbiValueKind::Pointer) => Ok(Self::Pointer(value)),
            (value, _) => Err(value.kind()),
        }
    }

    fn as_arg(&self) -> Arg<'_> {
        match self {
            Self::I8(value) => Arg::new(value),
            Self::I64(value) => Arg::new(value),
            Self::I128(value) => Arg::new(value),
            Self::F64(value) => Arg::new(value),
            Self::Pointer(value) => Arg::new(value),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C, align(16))]
struct AbiI128Repr {
    low: u64,
    high: u64,
}

impl AbiI128Repr {
    const fn from_bits(bits: u128) -> Self {
        Self {
            low: bits as u64,
            high: (bits >> 64) as u64,
        }
    }

    const fn bits(self) -> u128 {
        (self.low as u128) | ((self.high as u128) << 64)
    }
}

extern "C" fn aivi_text_concat(count: i64, segments: *const *const u8) -> *const u8 {
    with_current_arena(|arena| {
        if count < 0 || segments.is_null() {
            return ptr::null();
        }
        // SAFETY: the JIT helper ABI passes `count` contiguous segment pointers.
        let segment_ptrs = unsafe { slice::from_raw_parts(segments, count as usize) };
        let mut joined = Vec::new();
        for &segment in segment_ptrs {
            // SAFETY: each segment pointer follows the same len-prefixed byte contract.
            let Some(bytes) = (unsafe { read_len_prefixed_bytes(segment) }) else {
                return ptr::null();
            };
            joined.extend_from_slice(bytes);
        }
        arena.store_len_prefixed_bytes(&joined).cast()
    })
    .unwrap_or(ptr::null())
}

extern "C" fn aivi_bytes_append(left: *const u8, right: *const u8) -> *const u8 {
    with_current_arena(|arena| {
        // SAFETY: JIT helpers only hand us pointers produced by the len-prefixed byte contract.
        let Some(left_bytes) = (unsafe { read_len_prefixed_bytes(left) }) else {
            return ptr::null();
        };
        // SAFETY: same contract as `left_bytes`.
        let Some(right_bytes) = (unsafe { read_len_prefixed_bytes(right) }) else {
            return ptr::null();
        };
        let mut joined = Vec::with_capacity(left_bytes.len() + right_bytes.len());
        joined.extend_from_slice(left_bytes);
        joined.extend_from_slice(right_bytes);
        arena.store_len_prefixed_bytes(&joined).cast()
    })
    .unwrap_or(ptr::null())
}

extern "C" fn aivi_bytes_repeat(byte: i64, count: i64) -> *const u8 {
    with_current_arena(|arena| {
        let byte = byte.clamp(0, 255) as u8;
        let count = count.max(0) as usize;
        arena.store_len_prefixed_bytes(&vec![byte; count]).cast()
    })
    .unwrap_or(ptr::null())
}

extern "C" fn aivi_bytes_slice(from: i64, to: i64, bytes: *const u8) -> *const u8 {
    with_current_arena(|arena| {
        // SAFETY: JIT helpers only hand us pointers produced by the len-prefixed byte contract.
        let Some(bytes) = (unsafe { read_len_prefixed_bytes(bytes) }) else {
            return ptr::null();
        };
        let start = if from < 0 {
            bytes.len()
        } else {
            (from as usize).min(bytes.len())
        };
        let end = if to < 0 {
            bytes.len()
        } else {
            (to as usize).min(bytes.len())
        };
        let end = end.max(start);
        arena.store_len_prefixed_bytes(&bytes[start..end]).cast()
    })
    .unwrap_or(ptr::null())
}

fn with_current_arena<R>(f: impl FnOnce(&mut AllocationArena) -> R) -> Option<R> {
    ACTIVE_ARENA.with(|slot| {
        let arena = slot.borrow().as_ref()?.clone();
        let mut arena = arena.borrow_mut();
        Some(f(&mut arena))
    })
}

unsafe fn read_len_prefixed_bytes<'a>(pointer: *const u8) -> Option<&'a [u8]> {
    if pointer.is_null() {
        return None;
    }
    let mut prefix = [0u8; 8];
    // SAFETY: caller guarantees `pointer` addresses at least the 8-byte length prefix.
    unsafe { ptr::copy_nonoverlapping(pointer, prefix.as_mut_ptr(), prefix.len()) };
    let len = u64::from_le_bytes(prefix) as usize;
    // SAFETY: caller guarantees the contract stores exactly `len` bytes immediately after prefix.
    Some(unsafe { slice::from_raw_parts(pointer.add(prefix.len()), len) })
}

#[cfg(test)]
mod tests {
    use super::*;

    extern "C" fn rotate_i128_words(value: u128) -> u128 {
        value.rotate_left(17)
    }

    #[test]
    fn function_caller_round_trips_i128_values() {
        let caller =
            FunctionCaller::new(CallSignature::new([AbiValueKind::I128], AbiValueKind::I128));
        let value = 0x0123_4567_89ab_cdef_fedc_ba98_7654_3210u128;

        assert_eq!(
            caller
                .call(
                    rotate_i128_words as *const () as *const u8,
                    &[AbiValue::I128(value)]
                )
                .expect("I128 function call should succeed"),
            AbiValue::I128(value.rotate_left(17))
        );
    }

    #[test]
    fn function_caller_reports_i128_argument_mismatches() {
        let caller =
            FunctionCaller::new(CallSignature::new([AbiValueKind::I128], AbiValueKind::I128));

        assert_eq!(
            caller
                .call(
                    rotate_i128_words as *const () as *const u8,
                    &[AbiValue::I64(1)]
                )
                .expect_err("mismatched ABI arguments should fail"),
            CallError::ArgumentTypeMismatch {
                index: 0,
                expected: AbiValueKind::I128,
                found: AbiValueKind::I64,
            }
        );
    }
}
