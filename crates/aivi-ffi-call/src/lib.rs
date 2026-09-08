#![deny(unsafe_op_in_unsafe_fn)]

use std::{cell::RefCell, ffi::c_void, ptr, rc::Rc, slice};

use cranelift_codegen::ir::{AbiParam, Signature, types};
use cranelift_jit::JITModule;
use cranelift_module::{DataId, FuncId, Module};
use libffi::middle::{Arg, Cif, CodePtr, Type};
use num_bigint::{BigInt, Sign};
use rust_decimal::Decimal;

const LEN_PREFIX_BYTES: usize = 8;
const SEQUENCE_HEADER_BYTES: usize = 16;
const MAP_HEADER_BYTES: usize = 24;
const DECIMAL_ENCODED_BYTES: usize = 20;
const BIGINT_HEADER_BYTES: usize = 16;

thread_local! {
    static ACTIVE_ARENA: RefCell<Option<Rc<RefCell<AllocationArena>>>> = const { RefCell::new(None) };
}

#[derive(Debug, Default)]
pub struct AllocationArena {
    allocations: Vec<ArenaAllocation>,
}

#[derive(Debug)]
struct ArenaAllocation {
    storage: Box<[u8]>,
    offset: usize,
    len: usize,
}

impl ArenaAllocation {
    fn readable(&self) -> &[u8] {
        &self.storage[self.offset..self.offset + self.len]
    }

    fn contains(&self, address: usize) -> bool {
        let start = self.storage.as_ptr() as usize + self.offset;
        if self.len == 0 {
            return address == start;
        }
        start
            .checked_add(self.len)
            .is_some_and(|end| address >= start && address < end)
    }
}

impl AllocationArena {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn alloc_raw_bytes_aligned(&mut self, len: usize, align: usize) -> *mut c_void {
        let Some((mut encoded, offset)) = aligned_zeroed_storage(len, align) else {
            return ptr::null_mut();
        };
        let pointer = encoded.as_mut_ptr().wrapping_add(offset);
        self.allocations.push(ArenaAllocation {
            storage: encoded.into_boxed_slice(),
            offset,
            len,
        });
        pointer.cast()
    }

    pub fn store_raw_bytes_aligned(&mut self, bytes: &[u8], align: usize) -> *const c_void {
        let Some((mut encoded, offset)) = aligned_zeroed_storage(bytes.len(), align) else {
            return ptr::null();
        };
        encoded[offset..offset + bytes.len()].copy_from_slice(bytes);
        let storage = encoded.into_boxed_slice();
        let pointer = storage.as_ptr().wrapping_add(offset);
        self.allocations.push(ArenaAllocation {
            storage,
            offset,
            len: bytes.len(),
        });
        pointer.cast()
    }

    pub fn store_len_prefixed_bytes(&mut self, bytes: &[u8]) -> *const c_void {
        let mut encoded = Vec::with_capacity(LEN_PREFIX_BYTES + bytes.len());
        encoded.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
        encoded.extend_from_slice(bytes);
        self.store_raw_bytes_aligned(&encoded, LEN_PREFIX_BYTES)
    }

    fn contains(&self, pointer: *const c_void) -> bool {
        let address = pointer as usize;
        self.allocations
            .iter()
            .any(|allocation| allocation.contains(address))
    }
}

fn aligned_zeroed_storage(len: usize, align: usize) -> Option<(Vec<u8>, usize)> {
    let align = align.max(1).checked_next_power_of_two()?;
    let storage_len = len.max(1).checked_add(align - 1)?;
    let mut storage = Vec::new();
    storage.try_reserve_exact(storage_len).ok()?;
    storage.resize(storage_len, 0);
    let base = storage.as_ptr() as usize;
    let aligned = base.checked_add(align - 1)? & !(align - 1);
    Some((storage, aligned - base))
}

/// A bounded view of memory that may be read while unmarshalling a native JIT result.
///
/// Raw addresses are never dereferenced directly. Every read must fit wholly inside one live
/// arena allocation, finalized Cranelift data object, or explicitly borrowed backend-owned slot.
pub struct ReadableMemory<'a> {
    regions: Vec<&'a [u8]>,
}

impl<'a> ReadableMemory<'a> {
    pub fn from_arena(arena: &'a AllocationArena) -> Self {
        Self {
            regions: arena
                .allocations
                .iter()
                .map(ArenaAllocation::readable)
                .collect(),
        }
    }

    pub fn with_jit_data(
        module: &'a JITModule,
        data_ids: impl IntoIterator<Item = DataId>,
        arena: &'a AllocationArena,
        borrowed_regions: impl IntoIterator<Item = &'a [u8]>,
    ) -> Option<Self> {
        let mut regions: Vec<&'a [u8]> = arena
            .allocations
            .iter()
            .map(ArenaAllocation::readable)
            .collect();
        regions.extend(borrowed_regions);
        for data_id in data_ids {
            let (pointer, len) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                module.get_finalized_data(data_id)
            }))
            .ok()?;
            if pointer.is_null() || len > isize::MAX as usize {
                return None;
            }
            // SAFETY: Cranelift guarantees that finalized data addresses `len` bytes and remains
            // valid until `module` is freed. `ReadableMemory` cannot outlive the borrowed module.
            regions.push(unsafe { slice::from_raw_parts(pointer, len) });
        }
        Some(Self { regions })
    }

    fn read(&self, pointer: *const u8, len: usize) -> Option<&'a [u8]> {
        if pointer.is_null() {
            return None;
        }
        let start = pointer as usize;
        let end = start.checked_add(len)?;
        self.regions.iter().find_map(|region| {
            let region_start = region.as_ptr() as usize;
            let region_end = region_start.checked_add(region.len())?;
            if start < region_start || end > region_end {
                return None;
            }
            let offset = start - region_start;
            region.get(offset..offset.checked_add(len)?)
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarshaledSequence {
    pub count: usize,
    pub element_size: usize,
    pub bytes: Box<[u8]>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarshaledMap {
    pub count: usize,
    pub key_size: usize,
    pub value_size: usize,
    pub bytes: Box<[u8]>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AbiValueKind {
    I8,
    I64,
    I128,
    F64,
    Pointer,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AbiValue {
    I8(i8),
    I64(i64),
    I128(u128),
    F64(f64),
    Pointer(AbiPointer),
}

impl AbiValue {
    pub const fn kind(&self) -> AbiValueKind {
        match self {
            Self::I8(_) => AbiValueKind::I8,
            Self::I64(_) => AbiValueKind::I64,
            Self::I128(_) => AbiValueKind::I128,
            Self::F64(_) => AbiValueKind::F64,
            Self::Pointer(_) => AbiValueKind::Pointer,
        }
    }

    pub fn pointer_from_arena(
        pointer: *const c_void,
        arena: Rc<RefCell<AllocationArena>>,
    ) -> Option<Self> {
        if pointer.is_null() {
            return Some(Self::Pointer(AbiPointer {
                pointer,
                arena: Some(arena),
            }));
        }
        let is_owned = arena.try_borrow().ok()?.contains(pointer);
        is_owned.then_some(Self::Pointer(AbiPointer {
            pointer,
            arena: Some(arena),
        }))
    }
}

#[derive(Clone, Debug)]
pub struct AbiPointer {
    pointer: *const c_void,
    arena: Option<Rc<RefCell<AllocationArena>>>,
}

impl AbiPointer {
    pub const fn as_ptr(&self) -> *const c_void {
        self.pointer
    }

    pub const fn is_null(&self) -> bool {
        self.pointer.is_null()
    }

    const fn is_trusted_argument(&self) -> bool {
        self.pointer.is_null() || self.arena.is_some()
    }

    const fn jit_result(pointer: *const c_void) -> Self {
        Self {
            pointer,
            arena: None,
        }
    }
}

impl PartialEq for AbiPointer {
    fn eq(&self, other: &Self) -> bool {
        self.pointer == other.pointer
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

    /// Invoke one finalized JIT function through this libffi signature.
    ///
    /// # Safety
    ///
    /// The caller must guarantee that the finalized machine code is memory-safe for the supplied
    /// arguments and every transitive pointer reachable from them. Signature equality and pointer
    /// ownership are checked here, but neither can prove the behavior of arbitrary machine code.
    pub unsafe fn call(
        &self,
        module: &JITModule,
        function: FuncId,
        args: &[AbiValue],
    ) -> Result<AbiValue, CallError> {
        if args.len() != self.signature.args.len() {
            return Err(CallError::ArityMismatch {
                expected: self.signature.args.len(),
                found: args.len(),
            });
        }

        let mut owned_args = Vec::with_capacity(args.len());
        for (index, (value, kind)) in args
            .iter()
            .zip(self.signature.args.iter().copied())
            .enumerate()
        {
            if matches!(value, AbiValue::Pointer(pointer) if !pointer.is_trusted_argument()) {
                return Err(CallError::UntrustedPointerArgument { index });
            }
            owned_args.push(OwnedArg::new(value, kind).map_err(|found| {
                CallError::ArgumentTypeMismatch {
                    index,
                    expected: kind,
                    found,
                }
            })?);
        }

        let declaration = module
            .declarations()
            .get_functions()
            .find_map(|(id, declaration)| (id == function).then_some(declaration))
            .ok_or(CallError::UnknownFunction)?;
        if declaration.signature != self.cranelift_signature(module) {
            return Err(CallError::FunctionSignatureMismatch);
        }
        let function = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            module.get_finalized_function(function)
        }))
        .map_err(|_| CallError::FunctionNotCallable)?;
        if function.is_null() {
            return Err(CallError::FunctionNotCallable);
        }

        // SAFETY: the code pointer was resolved from `module` while it is borrowed for this call,
        // and the module declaration was checked against the exact libffi signature above.
        unsafe { self.call_code_ptr(function, &owned_args) }
    }

    fn cranelift_signature(&self, module: &JITModule) -> Signature {
        let mut signature = module.make_signature();
        signature.params.extend(
            self.signature
                .args
                .iter()
                .copied()
                .map(|kind| AbiParam::new(cranelift_type_for_abi_kind(kind, module))),
        );
        signature
            .returns
            .push(AbiParam::new(cranelift_type_for_abi_kind(
                self.signature.result,
                module,
            )));
        signature
    }

    unsafe fn call_code_ptr(
        &self,
        function: *const u8,
        owned_args: &[OwnedArg],
    ) -> Result<AbiValue, CallError> {
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
                AbiValue::Pointer(AbiPointer::jit_result(unsafe {
                    self.cif.call::<*const c_void>(code_ptr, &ffi_args)
                }))
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
    UnknownFunction,
    FunctionNotCallable,
    FunctionSignatureMismatch,
    UntrustedPointerArgument {
        index: usize,
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
            Self::UnknownFunction => write!(f, "foreign call references an unknown JIT function"),
            Self::FunctionNotCallable => {
                write!(
                    f,
                    "foreign call references a JIT function that is not finalized"
                )
            }
            Self::FunctionSignatureMismatch => write!(
                f,
                "foreign call signature does not match the JIT function declaration"
            ),
            Self::UntrustedPointerArgument { index } => write!(
                f,
                "foreign call argument {} contains a pointer without a live arena owner",
                index + 1
            ),
        }
    }
}

impl std::error::Error for CallError {}

pub fn with_active_arena<R>(arena: Rc<RefCell<AllocationArena>>, f: impl FnOnce() -> R) -> R {
    let previous = ACTIVE_ARENA.with(|slot| slot.replace(Some(arena)));
    let scope = ActiveArenaScope { previous };
    let result = f();
    drop(scope);
    result
}

struct ActiveArenaScope {
    previous: Option<Rc<RefCell<AllocationArena>>>,
}

impl Drop for ActiveArenaScope {
    fn drop(&mut self) {
        ACTIVE_ARENA.with(|slot| {
            slot.replace(self.previous.take());
        });
    }
}

pub fn encode_len_prefixed_bytes(bytes: &[u8], arena: &mut AllocationArena) -> *const c_void {
    arena.store_len_prefixed_bytes(bytes)
}

pub fn decode_len_prefixed_bytes(
    memory: &ReadableMemory<'_>,
    pointer: *const c_void,
) -> Option<Box<[u8]>> {
    let prefix = memory.read(pointer.cast(), LEN_PREFIX_BYTES)?;
    let len = usize::try_from(u64::from_le_bytes(prefix.try_into().ok()?)).ok()?;
    let data_pointer = pointer_at(pointer.cast(), LEN_PREFIX_BYTES)?;
    Some(memory.read(data_pointer, len)?.into())
}

pub fn encode_marshaled_sequence(
    count: usize,
    element_size: usize,
    bytes: &[u8],
    arena: &mut AllocationArena,
) -> Option<*const c_void> {
    encode_marshaled_sequence_with_align(
        count,
        element_size,
        bytes,
        sequence_alignment_for_element_size(element_size),
        arena,
    )
}

pub fn decode_marshaled_sequence(
    memory: &ReadableMemory<'_>,
    pointer: *const c_void,
) -> Option<MarshaledSequence> {
    let view = read_marshaled_sequence_view_from(memory, pointer.cast())?;
    Some(MarshaledSequence {
        count: view.count,
        element_size: view.element_size,
        bytes: view.data.into(),
    })
}

pub fn encode_marshaled_map(
    count: usize,
    key_size: usize,
    value_size: usize,
    bytes: &[u8],
    arena: &mut AllocationArena,
) -> Option<*const c_void> {
    let entry_size = key_size.checked_add(value_size)?;
    let expected = count.checked_mul(entry_size)?;
    if expected != bytes.len() {
        return None;
    }
    let mut encoded = Vec::with_capacity(MAP_HEADER_BYTES + bytes.len());
    encoded.extend_from_slice(&(count as u64).to_le_bytes());
    encoded.extend_from_slice(&(key_size as u64).to_le_bytes());
    encoded.extend_from_slice(&(value_size as u64).to_le_bytes());
    encoded.extend_from_slice(bytes);
    let pointer = arena.store_raw_bytes_aligned(&encoded, 8);
    (!pointer.is_null()).then_some(pointer)
}

pub fn decode_marshaled_map(
    memory: &ReadableMemory<'_>,
    pointer: *const c_void,
) -> Option<MarshaledMap> {
    let view = read_marshaled_map_view_from(memory, pointer.cast())?;
    Some(MarshaledMap {
        count: view.count,
        key_size: view.key_size,
        value_size: view.value_size,
        bytes: view.data.into(),
    })
}

pub fn read_marshaled_field(
    memory: &ReadableMemory<'_>,
    pointer: *const c_void,
    offset: usize,
    size: usize,
) -> Option<Box<[u8]>> {
    let field_pointer = pointer_at(pointer.cast(), offset)?;
    Some(memory.read(field_pointer, size)?.into())
}

pub fn read_decimal_constant_bytes(
    memory: &ReadableMemory<'_>,
    pointer: *const c_void,
) -> Option<Box<[u8]>> {
    read_marshaled_field(memory, pointer, 0, DECIMAL_ENCODED_BYTES)
}

pub fn read_bigint_constant_bytes(
    memory: &ReadableMemory<'_>,
    pointer: *const c_void,
) -> Option<Box<[u8]>> {
    let header = read_marshaled_field(memory, pointer, 0, BIGINT_HEADER_BYTES)?;
    let magnitude_len = usize::try_from(u64::from_le_bytes(header[8..16].try_into().ok()?)).ok()?;
    read_marshaled_field(
        memory,
        pointer,
        0,
        BIGINT_HEADER_BYTES.checked_add(magnitude_len)?,
    )
}

pub fn lookup_runtime_symbol(symbol: &str) -> Option<*const u8> {
    match symbol {
        "aivi_arena_alloc" => Some(aivi_arena_alloc as *const () as *const u8),
        "aivi_text_concat" => Some(aivi_text_concat as *const () as *const u8),
        "aivi_int_to_text" => Some(aivi_int_to_text as *const () as *const u8),
        "aivi_float_to_text" => Some(aivi_float_to_text as *const () as *const u8),
        "aivi_bool_to_text" => Some(aivi_bool_to_text as *const () as *const u8),
        "aivi_unit_to_text" => Some(aivi_unit_to_text as *const () as *const u8),
        "aivi_bytes_append" => Some(aivi_bytes_append as *const () as *const u8),
        "aivi_path_join" => Some(aivi_path_join as *const () as *const u8),
        "aivi_bytes_repeat" => Some(aivi_bytes_repeat as *const () as *const u8),
        "aivi_bytes_slice" => Some(aivi_bytes_slice as *const () as *const u8),
        "aivi_list_new" => Some(aivi_list_new as *const () as *const u8),
        "aivi_set_new" => Some(aivi_set_new as *const () as *const u8),
        "aivi_map_new" => Some(aivi_map_new as *const () as *const u8),
        "aivi_list_len" => Some(aivi_list_len as *const () as *const u8),
        "aivi_list_get" => Some(aivi_list_get as *const () as *const u8),
        "aivi_list_slice" => Some(aivi_list_slice as *const () as *const u8),
        "aivi_list_append" => Some(aivi_list_append as *const () as *const u8),
        "aivi_decimal_add" => Some(aivi_decimal_add as *const () as *const u8),
        "aivi_decimal_sub" => Some(aivi_decimal_sub as *const () as *const u8),
        "aivi_decimal_mul" => Some(aivi_decimal_mul as *const () as *const u8),
        "aivi_decimal_div" => Some(aivi_decimal_div as *const () as *const u8),
        "aivi_decimal_mod" => Some(aivi_decimal_mod as *const () as *const u8),
        "aivi_decimal_eq" => Some(aivi_decimal_eq as *const () as *const u8),
        "aivi_decimal_gt" => Some(aivi_decimal_gt as *const () as *const u8),
        "aivi_decimal_lt" => Some(aivi_decimal_lt as *const () as *const u8),
        "aivi_decimal_gte" => Some(aivi_decimal_gte as *const () as *const u8),
        "aivi_decimal_lte" => Some(aivi_decimal_lte as *const () as *const u8),
        "aivi_bigint_add" => Some(aivi_bigint_add as *const () as *const u8),
        "aivi_bigint_sub" => Some(aivi_bigint_sub as *const () as *const u8),
        "aivi_bigint_mul" => Some(aivi_bigint_mul as *const () as *const u8),
        "aivi_bigint_div" => Some(aivi_bigint_div as *const () as *const u8),
        "aivi_bigint_mod" => Some(aivi_bigint_mod as *const () as *const u8),
        "aivi_bigint_eq" => Some(aivi_bigint_eq as *const () as *const u8),
        "aivi_bigint_gt" => Some(aivi_bigint_gt as *const () as *const u8),
        "aivi_bigint_lt" => Some(aivi_bigint_lt as *const () as *const u8),
        "aivi_bigint_gte" => Some(aivi_bigint_gte as *const () as *const u8),
        "aivi_bigint_lte" => Some(aivi_bigint_lte as *const () as *const u8),
        _ => None,
    }
}

extern "C" fn aivi_arena_alloc(size: i64, align: i64) -> *mut u8 {
    with_current_arena(|arena| {
        let Some(size) = non_negative_usize(size) else {
            return ptr::null_mut();
        };
        let Some(align) = non_negative_usize(align) else {
            return ptr::null_mut();
        };
        arena.alloc_raw_bytes_aligned(size, align).cast()
    })
    .unwrap_or(ptr::null_mut())
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

fn cranelift_type_for_abi_kind(
    kind: AbiValueKind,
    module: &JITModule,
) -> cranelift_codegen::ir::Type {
    match kind {
        AbiValueKind::I8 => types::I8,
        AbiValueKind::I64 => types::I64,
        AbiValueKind::I128 => types::I128,
        AbiValueKind::F64 => types::F64,
        AbiValueKind::Pointer => module.target_config().pointer_type(),
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
    fn new(value: &AbiValue, expected: AbiValueKind) -> Result<Self, AbiValueKind> {
        match (value, expected) {
            (AbiValue::I8(value), AbiValueKind::I8) => Ok(Self::I8(*value)),
            (AbiValue::I64(value), AbiValueKind::I64) => Ok(Self::I64(*value)),
            (AbiValue::I128(value), AbiValueKind::I128) => {
                Ok(Self::I128(AbiI128Repr::from_bits(*value)))
            }
            (AbiValue::F64(value), AbiValueKind::F64) => Ok(Self::F64(*value)),
            (AbiValue::Pointer(value), AbiValueKind::Pointer) => Ok(Self::Pointer(value.as_ptr())),
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
        let Some(count) = non_negative_usize(count) else {
            return ptr::null();
        };
        if segments.is_null()
            || count
                .checked_mul(std::mem::size_of::<*const u8>())
                .is_none_or(|bytes| bytes > isize::MAX as usize)
        {
            return ptr::null();
        }
        // SAFETY: the JIT helper ABI passes `count` contiguous segment pointers.
        let segment_ptrs = unsafe { slice::from_raw_parts(segments, count) };
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

extern "C" fn aivi_int_to_text(value: i64) -> *const u8 {
    with_current_arena(|arena| {
        arena
            .store_len_prefixed_bytes(value.to_string().as_bytes())
            .cast()
    })
    .unwrap_or(ptr::null())
}

extern "C" fn aivi_float_to_text(value: f64) -> *const u8 {
    with_current_arena(|arena| {
        arena
            .store_len_prefixed_bytes(value.to_string().as_bytes())
            .cast()
    })
    .unwrap_or(ptr::null())
}

extern "C" fn aivi_bool_to_text(value: i8) -> *const u8 {
    with_current_arena(|arena| {
        let rendered = if value == 0 { "False" } else { "True" };
        arena.store_len_prefixed_bytes(rendered.as_bytes()).cast()
    })
    .unwrap_or(ptr::null())
}

extern "C" fn aivi_unit_to_text(_: i8) -> *const u8 {
    with_current_arena(|arena| arena.store_len_prefixed_bytes(b"()").cast()).unwrap_or(ptr::null())
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

extern "C" fn aivi_path_join(base: *const u8, segment: *const u8) -> *const u8 {
    with_current_arena(|arena| {
        // SAFETY: JIT helpers only hand us pointers produced by the len-prefixed byte contract.
        let Some(base_bytes) = (unsafe { read_len_prefixed_bytes(base) }) else {
            return ptr::null();
        };
        // SAFETY: same contract as `base_bytes`.
        let Some(segment_bytes) = (unsafe { read_len_prefixed_bytes(segment) }) else {
            return ptr::null();
        };
        let Ok(base_text) = std::str::from_utf8(base_bytes) else {
            return ptr::null();
        };
        let Ok(segment_text) = std::str::from_utf8(segment_bytes) else {
            return ptr::null();
        };
        let joined = std::path::Path::new(base_text).join(segment_text);
        arena
            .store_len_prefixed_bytes(joined.to_string_lossy().as_bytes())
            .cast()
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

extern "C" fn aivi_list_new(count: i64, elements_ptr: *const u8, element_size: i64) -> *const u8 {
    with_current_arena(|arena| {
        let Some(count) = non_negative_usize(count) else {
            return ptr::null();
        };
        let Some(element_size) = non_negative_usize(element_size) else {
            return ptr::null();
        };
        let Some(byte_len) = count.checked_mul(element_size) else {
            return ptr::null();
        };
        // SAFETY: the JIT constructor ABI passes `count * element_size` contiguous bytes.
        let Some(bytes) = (unsafe { read_exact_bytes(elements_ptr, byte_len) }) else {
            return ptr::null();
        };
        encode_marshaled_sequence(count, element_size, bytes.as_ref(), arena)
            .map(|pointer| pointer.cast())
            .unwrap_or(ptr::null())
    })
    .unwrap_or(ptr::null())
}

extern "C" fn aivi_set_new(count: i64, elements_ptr: *const u8, element_size: i64) -> *const u8 {
    aivi_list_new(count, elements_ptr, element_size)
}

extern "C" fn aivi_map_new(
    count: i64,
    entries_ptr: *const u8,
    key_size: i64,
    value_size: i64,
) -> *const u8 {
    with_current_arena(|arena| {
        let Some(count) = non_negative_usize(count) else {
            return ptr::null();
        };
        let Some(key_size) = non_negative_usize(key_size) else {
            return ptr::null();
        };
        let Some(value_size) = non_negative_usize(value_size) else {
            return ptr::null();
        };
        let Some(entry_size) = key_size.checked_add(value_size) else {
            return ptr::null();
        };
        let Some(byte_len) = count.checked_mul(entry_size) else {
            return ptr::null();
        };
        // SAFETY: the JIT constructor ABI passes `count * (key_size + value_size)` bytes.
        let Some(bytes) = (unsafe { read_exact_bytes(entries_ptr, byte_len) }) else {
            return ptr::null();
        };
        encode_marshaled_map(count, key_size, value_size, bytes.as_ref(), arena)
            .map(|pointer| pointer.cast())
            .unwrap_or(ptr::null())
    })
    .unwrap_or(ptr::null())
}

extern "C" fn aivi_list_len(list_ptr: *const u8) -> i64 {
    // SAFETY: the lazy-JIT collection helpers only hand us pointers produced by
    // `encode_marshaled_sequence`/`aivi_list_new`/`aivi_list_slice`.
    unsafe { read_marshaled_sequence_view(list_ptr) }
        .and_then(|view| i64::try_from(view.count).ok())
        .unwrap_or(0)
}

extern "C" fn aivi_list_get(list_ptr: *const u8, index: i64) -> *const u8 {
    let Some(index) = non_negative_usize(index) else {
        return ptr::null();
    };
    // SAFETY: the lazy-JIT collection helpers only hand us pointers produced by
    // `encode_marshaled_sequence`/`aivi_list_new`/`aivi_list_slice`.
    let Some(view) = (unsafe { read_marshaled_sequence_view(list_ptr) }) else {
        return ptr::null();
    };
    if index >= view.count {
        return ptr::null();
    }
    view.data
        .as_ptr()
        .wrapping_add(index.saturating_mul(view.element_size))
}

extern "C" fn aivi_list_slice(list_ptr: *const u8, start: i64, element_size: i64) -> *const u8 {
    with_current_arena(|arena| {
        let Some(start) = non_negative_usize(start) else {
            return ptr::null();
        };
        let Some(element_size) = non_negative_usize(element_size) else {
            return ptr::null();
        };
        // SAFETY: the lazy-JIT collection helpers only hand us pointers produced by
        // `encode_marshaled_sequence`/`aivi_list_new`/`aivi_list_slice`.
        let Some(view) = (unsafe { read_marshaled_sequence_view(list_ptr) }) else {
            return ptr::null();
        };
        if view.element_size != element_size {
            return ptr::null();
        }
        let start = start.min(view.count);
        let remaining = view.count - start;
        let byte_start = start.saturating_mul(element_size);
        encode_marshaled_sequence(remaining, element_size, &view.data[byte_start..], arena)
            .map(|pointer| pointer.cast())
            .unwrap_or(ptr::null())
    })
    .unwrap_or(ptr::null())
}

extern "C" fn aivi_list_append(
    left_ptr: *const u8,
    right_ptr: *const u8,
    element_size: i64,
) -> *const u8 {
    with_current_arena(|arena| {
        let Some(element_size) = non_negative_usize(element_size) else {
            return ptr::null();
        };
        // SAFETY: the lazy-JIT collection helpers only hand us pointers produced by
        // `encode_marshaled_sequence`/`aivi_list_new`/`aivi_list_slice`/`aivi_list_append`.
        let Some(left) = (unsafe { read_marshaled_sequence_view(left_ptr) }) else {
            return ptr::null();
        };
        // SAFETY: same contract as `left`.
        let Some(right) = (unsafe { read_marshaled_sequence_view(right_ptr) }) else {
            return ptr::null();
        };
        if left.element_size != element_size || right.element_size != element_size {
            return ptr::null();
        }
        let Some(total_count) = left.count.checked_add(right.count) else {
            return ptr::null();
        };
        let Some(total_bytes) = total_count.checked_mul(element_size) else {
            return ptr::null();
        };
        let mut bytes = Vec::with_capacity(total_bytes);
        bytes.extend_from_slice(left.data.as_ref());
        bytes.extend_from_slice(right.data.as_ref());
        encode_marshaled_sequence(total_count, element_size, &bytes, arena)
            .map(|pointer| pointer.cast())
            .unwrap_or(ptr::null())
    })
    .unwrap_or(ptr::null())
}

extern "C" fn aivi_decimal_add(left: *const u8, right: *const u8) -> *const u8 {
    decimal_binop(left, right, |left, right| left.checked_add(right))
}

extern "C" fn aivi_decimal_sub(left: *const u8, right: *const u8) -> *const u8 {
    decimal_binop(left, right, |left, right| left.checked_sub(right))
}

extern "C" fn aivi_decimal_mul(left: *const u8, right: *const u8) -> *const u8 {
    decimal_binop(left, right, |left, right| left.checked_mul(right))
}

extern "C" fn aivi_decimal_div(left: *const u8, right: *const u8) -> *const u8 {
    decimal_binop(left, right, |left, right| left.checked_div(right))
}

extern "C" fn aivi_decimal_mod(left: *const u8, right: *const u8) -> *const u8 {
    decimal_binop(left, right, |left, right| left.checked_rem(right))
}

extern "C" fn aivi_decimal_eq(left: *const u8, right: *const u8) -> i8 {
    decimal_cmp(left, right, |cmp| cmp == std::cmp::Ordering::Equal)
}

extern "C" fn aivi_decimal_gt(left: *const u8, right: *const u8) -> i8 {
    decimal_cmp(left, right, |cmp| cmp == std::cmp::Ordering::Greater)
}

extern "C" fn aivi_decimal_lt(left: *const u8, right: *const u8) -> i8 {
    decimal_cmp(left, right, |cmp| cmp == std::cmp::Ordering::Less)
}

extern "C" fn aivi_decimal_gte(left: *const u8, right: *const u8) -> i8 {
    decimal_cmp(left, right, |cmp| cmp != std::cmp::Ordering::Less)
}

extern "C" fn aivi_decimal_lte(left: *const u8, right: *const u8) -> i8 {
    decimal_cmp(left, right, |cmp| cmp != std::cmp::Ordering::Greater)
}

extern "C" fn aivi_bigint_add(left: *const u8, right: *const u8) -> *const u8 {
    bigint_binop(left, right, |left, right| Some(left + right))
}

extern "C" fn aivi_bigint_sub(left: *const u8, right: *const u8) -> *const u8 {
    bigint_binop(left, right, |left, right| Some(left - right))
}

extern "C" fn aivi_bigint_mul(left: *const u8, right: *const u8) -> *const u8 {
    bigint_binop(left, right, |left, right| Some(left * right))
}

extern "C" fn aivi_bigint_div(left: *const u8, right: *const u8) -> *const u8 {
    bigint_binop(left, right, |left, right| {
        (!right.eq(&BigInt::from(0))).then_some(left / right)
    })
}

extern "C" fn aivi_bigint_mod(left: *const u8, right: *const u8) -> *const u8 {
    bigint_binop(left, right, |left, right| {
        (!right.eq(&BigInt::from(0))).then_some(left % right)
    })
}

extern "C" fn aivi_bigint_eq(left: *const u8, right: *const u8) -> i8 {
    bigint_cmp(left, right, |cmp| cmp == std::cmp::Ordering::Equal)
}

extern "C" fn aivi_bigint_gt(left: *const u8, right: *const u8) -> i8 {
    bigint_cmp(left, right, |cmp| cmp == std::cmp::Ordering::Greater)
}

extern "C" fn aivi_bigint_lt(left: *const u8, right: *const u8) -> i8 {
    bigint_cmp(left, right, |cmp| cmp == std::cmp::Ordering::Less)
}

extern "C" fn aivi_bigint_gte(left: *const u8, right: *const u8) -> i8 {
    bigint_cmp(left, right, |cmp| cmp != std::cmp::Ordering::Less)
}

extern "C" fn aivi_bigint_lte(left: *const u8, right: *const u8) -> i8 {
    bigint_cmp(left, right, |cmp| cmp != std::cmp::Ordering::Greater)
}

fn with_current_arena<R>(f: impl FnOnce(&mut AllocationArena) -> R) -> Option<R> {
    ACTIVE_ARENA.with(|slot| {
        let arena = {
            let active = slot.try_borrow().ok()?;
            active.as_ref()?.clone()
        };
        let mut arena = arena.try_borrow_mut().ok()?;
        Some(f(&mut arena))
    })
}

fn non_negative_usize(value: i64) -> Option<usize> {
    usize::try_from(value).ok()
}

fn sequence_alignment_for_element_size(element_size: usize) -> usize {
    match element_size {
        0 | 1 => 1,
        16.. => 16,
        _ => 8,
    }
}

fn encode_marshaled_sequence_with_align(
    count: usize,
    element_size: usize,
    bytes: &[u8],
    align: usize,
    arena: &mut AllocationArena,
) -> Option<*const c_void> {
    let expected = count.checked_mul(element_size)?;
    if expected != bytes.len() {
        return None;
    }
    let mut encoded = Vec::with_capacity(SEQUENCE_HEADER_BYTES + bytes.len());
    encoded.extend_from_slice(&(count as u64).to_le_bytes());
    encoded.extend_from_slice(&(element_size as u64).to_le_bytes());
    encoded.extend_from_slice(bytes);
    let pointer = arena.store_raw_bytes_aligned(&encoded, align.max(LEN_PREFIX_BYTES));
    (!pointer.is_null()).then_some(pointer)
}

fn decimal_binop(
    left: *const u8,
    right: *const u8,
    op: impl FnOnce(Decimal, Decimal) -> Option<Decimal>,
) -> *const u8 {
    with_current_arena(|arena| {
        let Some(left) = parse_decimal(left) else {
            return ptr::null();
        };
        let Some(right) = parse_decimal(right) else {
            return ptr::null();
        };
        let Some(result) = op(left, right) else {
            return ptr::null();
        };
        encode_decimal(result, arena)
    })
    .unwrap_or(ptr::null())
}

fn decimal_cmp(
    left: *const u8,
    right: *const u8,
    predicate: impl FnOnce(std::cmp::Ordering) -> bool,
) -> i8 {
    let Some(left) = parse_decimal(left) else {
        return 0;
    };
    let Some(right) = parse_decimal(right) else {
        return 0;
    };
    i8::from(predicate(left.cmp(&right)))
}

fn bigint_binop(
    left: *const u8,
    right: *const u8,
    op: impl FnOnce(BigInt, BigInt) -> Option<BigInt>,
) -> *const u8 {
    with_current_arena(|arena| {
        let Some(left) = parse_bigint(left) else {
            return ptr::null();
        };
        let Some(right) = parse_bigint(right) else {
            return ptr::null();
        };
        let Some(result) = op(left, right) else {
            return ptr::null();
        };
        encode_bigint(result, arena)
    })
    .unwrap_or(ptr::null())
}

fn bigint_cmp(
    left: *const u8,
    right: *const u8,
    predicate: impl FnOnce(std::cmp::Ordering) -> bool,
) -> i8 {
    let Some(left) = parse_bigint(left) else {
        return 0;
    };
    let Some(right) = parse_bigint(right) else {
        return 0;
    };
    i8::from(predicate(left.cmp(&right)))
}

fn parse_decimal(pointer: *const u8) -> Option<Decimal> {
    // SAFETY: numeric JIT helpers receive pointers using the fixed 20-byte decimal ABI contract.
    let bytes = unsafe { read_exact_bytes(pointer, DECIMAL_ENCODED_BYTES) }?;
    let mantissa = i128::from_le_bytes(bytes[..16].try_into().ok()?);
    let scale = u32::from_le_bytes(bytes[16..20].try_into().ok()?);
    Some(Decimal::from_i128_with_scale(mantissa, scale))
}

fn encode_decimal(value: Decimal, arena: &mut AllocationArena) -> *const u8 {
    let mut encoded = Vec::with_capacity(DECIMAL_ENCODED_BYTES);
    encoded.extend_from_slice(&value.mantissa().to_le_bytes());
    encoded.extend_from_slice(&value.scale().to_le_bytes());
    arena.store_raw_bytes_aligned(&encoded, 16).cast()
}

fn parse_bigint(pointer: *const u8) -> Option<BigInt> {
    // SAFETY: numeric JIT helpers receive pointers using the bigint header contract.
    let header = unsafe { read_exact_bytes(pointer, BIGINT_HEADER_BYTES) }?;
    let magnitude_len = usize::try_from(u64::from_le_bytes(header[8..16].try_into().ok()?)).ok()?;
    // SAFETY: the bigint ABI stores the declared magnitude immediately after the header.
    let bytes =
        unsafe { read_exact_bytes(pointer, BIGINT_HEADER_BYTES.checked_add(magnitude_len)?) }?;
    let sign = match bytes[0] {
        0 => Sign::NoSign,
        1 => Sign::Plus,
        2 => Sign::Minus,
        _ => return None,
    };
    let magnitude_len = u64::from_le_bytes(bytes[8..16].try_into().ok()?) as usize;
    let magnitude = bytes.get(16..16 + magnitude_len)?;
    Some(BigInt::from_bytes_le(sign, magnitude))
}

fn encode_bigint(value: BigInt, arena: &mut AllocationArena) -> *const u8 {
    let (sign, magnitude) = value.to_bytes_le();
    let mut encoded = Vec::with_capacity(BIGINT_HEADER_BYTES + magnitude.len());
    encoded.push(match sign {
        Sign::NoSign => 0,
        Sign::Plus => 1,
        Sign::Minus => 2,
    });
    encoded.extend_from_slice(&[0; 7]);
    encoded.extend_from_slice(&(magnitude.len() as u64).to_le_bytes());
    encoded.extend_from_slice(&magnitude);
    arena.store_raw_bytes_aligned(&encoded, 8).cast()
}

fn pointer_at(pointer: *const u8, offset: usize) -> Option<*const u8> {
    if pointer.is_null() {
        return None;
    }
    let base = pointer as usize;
    let address = base.checked_add(offset)?;
    Some(address as *const u8)
}

struct MarshaledSequenceView<'a> {
    count: usize,
    element_size: usize,
    data: &'a [u8],
}

struct MarshaledMapView<'a> {
    count: usize,
    key_size: usize,
    value_size: usize,
    data: &'a [u8],
}

fn read_marshaled_sequence_view_from<'a>(
    memory: &ReadableMemory<'a>,
    pointer: *const u8,
) -> Option<MarshaledSequenceView<'a>> {
    let header = memory.read(pointer, SEQUENCE_HEADER_BYTES)?;
    let count = usize::try_from(u64::from_le_bytes(header[..8].try_into().ok()?)).ok()?;
    let element_size = usize::try_from(u64::from_le_bytes(header[8..16].try_into().ok()?)).ok()?;
    let data_len = count.checked_mul(element_size)?;
    let data_pointer = pointer_at(pointer, SEQUENCE_HEADER_BYTES)?;
    Some(MarshaledSequenceView {
        count,
        element_size,
        data: memory.read(data_pointer, data_len)?,
    })
}

fn read_marshaled_map_view_from<'a>(
    memory: &ReadableMemory<'a>,
    pointer: *const u8,
) -> Option<MarshaledMapView<'a>> {
    let header = memory.read(pointer, MAP_HEADER_BYTES)?;
    let count = usize::try_from(u64::from_le_bytes(header[..8].try_into().ok()?)).ok()?;
    let key_size = usize::try_from(u64::from_le_bytes(header[8..16].try_into().ok()?)).ok()?;
    let value_size = usize::try_from(u64::from_le_bytes(header[16..24].try_into().ok()?)).ok()?;
    let entry_size = key_size.checked_add(value_size)?;
    let data_len = count.checked_mul(entry_size)?;
    let data_pointer = pointer_at(pointer, MAP_HEADER_BYTES)?;
    Some(MarshaledMapView {
        count,
        key_size,
        value_size,
        data: memory.read(data_pointer, data_len)?,
    })
}

unsafe fn read_len_prefixed_bytes<'a>(pointer: *const u8) -> Option<&'a [u8]> {
    if pointer.is_null() {
        return None;
    }
    let mut prefix = [0u8; LEN_PREFIX_BYTES];
    // SAFETY: caller guarantees `pointer` addresses at least the 8-byte length prefix.
    unsafe { ptr::copy_nonoverlapping(pointer, prefix.as_mut_ptr(), prefix.len()) };
    let len = usize::try_from(u64::from_le_bytes(prefix)).ok()?;
    if len > isize::MAX as usize {
        return None;
    }
    let data_pointer = pointer_at(pointer, prefix.len())?;
    // SAFETY: caller guarantees the contract stores exactly `len` bytes immediately after prefix.
    Some(unsafe { slice::from_raw_parts(data_pointer, len) })
}

unsafe fn read_exact_bytes(pointer: *const u8, len: usize) -> Option<Box<[u8]>> {
    if pointer.is_null() || len > isize::MAX as usize {
        return None;
    }
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(len).ok()?;
    bytes.resize(len, 0);
    let mut bytes = bytes.into_boxed_slice();
    // SAFETY: caller guarantees `pointer` addresses at least `len` readable bytes.
    unsafe { ptr::copy_nonoverlapping(pointer, bytes.as_mut_ptr(), len) };
    Some(bytes)
}

unsafe fn read_marshaled_sequence_view<'a>(
    pointer: *const u8,
) -> Option<MarshaledSequenceView<'a>> {
    let header = unsafe { read_exact_bytes(pointer, SEQUENCE_HEADER_BYTES) }?;
    let count = usize::try_from(u64::from_le_bytes(header[..8].try_into().ok()?)).ok()?;
    let element_size = usize::try_from(u64::from_le_bytes(header[8..16].try_into().ok()?)).ok()?;
    let data_len = count.checked_mul(element_size)?;
    if data_len > isize::MAX as usize {
        return None;
    }
    let data_pointer = pointer_at(pointer, SEQUENCE_HEADER_BYTES)?;
    // SAFETY: the sequence contract stores exactly `count * element_size` bytes after the header.
    let data = unsafe { slice::from_raw_parts(data_pointer, data_len) };
    Some(MarshaledSequenceView {
        count,
        element_size,
        data,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use cranelift_jit::JITBuilder;
    use cranelift_module::{DataDescription, Linkage, default_libcall_names};

    extern "C" fn rotate_i128_words(value: u128) -> u128 {
        value.rotate_left(17)
    }

    #[test]
    fn function_caller_round_trips_i128_values() {
        let caller =
            FunctionCaller::new(CallSignature::new([AbiValueKind::I128], AbiValueKind::I128));
        let value = 0x0123_4567_89ab_cdef_fedc_ba98_7654_3210u128;

        assert_eq!(
            // SAFETY: the test function has the exact ABI represented by `caller` and remains
            // live for the duration of the call.
            unsafe {
                caller.call_code_ptr(
                    rotate_i128_words as *const () as *const u8,
                    &[OwnedArg::I128(AbiI128Repr::from_bits(value))],
                )
            }
            .expect("I128 function call should succeed"),
            AbiValue::I128(value.rotate_left(17))
        );
    }

    #[test]
    fn owned_arguments_report_i128_kind_mismatches() {
        assert!(matches!(
            OwnedArg::new(&AbiValue::I64(1), AbiValueKind::I128),
            Err(AbiValueKind::I64)
        ));
    }

    #[test]
    fn function_caller_rejects_arguments_before_resolving_jit_code() {
        let mut module = JITModule::new(
            JITBuilder::new(default_libcall_names()).expect("host JIT should initialize"),
        );
        let mut signature = module.make_signature();
        signature.params.push(AbiParam::new(types::I128));
        signature.returns.push(AbiParam::new(types::I128));
        let function = module
            .declare_function("argument_mismatch", Linkage::Local, &signature)
            .expect("test function should declare");
        let caller =
            FunctionCaller::new(CallSignature::new([AbiValueKind::I128], AbiValueKind::I128));

        assert_eq!(
            // SAFETY: argument validation rejects the call before resolving machine code.
            unsafe { caller.call(&module, function, &[AbiValue::I64(1)]) }
                .expect_err("mismatched ABI arguments should fail"),
            CallError::ArgumentTypeMismatch {
                index: 0,
                expected: AbiValueKind::I128,
                found: AbiValueKind::I64,
            }
        );
    }

    #[test]
    fn function_caller_rejects_mismatched_and_unfinalized_jit_functions() {
        let mut module = JITModule::new(
            JITBuilder::new(default_libcall_names()).expect("host JIT should initialize"),
        );
        let mut signature = module.make_signature();
        signature.params.push(AbiParam::new(types::I64));
        signature.returns.push(AbiParam::new(types::I64));
        let function = module
            .declare_function("not_compiled", Linkage::Local, &signature)
            .expect("test function should declare");

        let mismatch =
            FunctionCaller::new(CallSignature::new([AbiValueKind::I8], AbiValueKind::I64));
        assert_eq!(
            // SAFETY: signature validation rejects the call before resolving machine code.
            unsafe { mismatch.call(&module, function, &[AbiValue::I8(1)]) }
                .expect_err("a mismatched declaration must not be called"),
            CallError::FunctionSignatureMismatch
        );

        let matching =
            FunctionCaller::new(CallSignature::new([AbiValueKind::I64], AbiValueKind::I64));
        assert_eq!(
            // SAFETY: finalization validation rejects the call before executing machine code.
            unsafe { matching.call(&module, function, &[AbiValue::I64(1)]) }
                .expect_err("an unfinalized function must not be called"),
            CallError::FunctionNotCallable
        );
    }

    #[test]
    fn pointer_arguments_require_a_live_arena_allocation() {
        let arena = Rc::new(RefCell::new(AllocationArena::new()));
        let outside = 42u64;
        assert!(
            AbiValue::pointer_from_arena((&outside as *const u64).cast(), Rc::clone(&arena),)
                .is_none()
        );

        let pointer = arena.borrow_mut().store_raw_bytes_aligned(&[1; 8], 8);
        assert!(AbiValue::pointer_from_arena(pointer, Rc::clone(&arena)).is_some());
    }

    #[test]
    fn function_caller_rejects_recycled_jit_result_pointers() {
        let mut module = JITModule::new(
            JITBuilder::new(default_libcall_names()).expect("host JIT should initialize"),
        );
        let mut signature = module.make_signature();
        signature
            .params
            .push(AbiParam::new(module.target_config().pointer_type()));
        signature.returns.push(AbiParam::new(types::I64));
        let function = module
            .declare_function("untrusted_pointer", Linkage::Local, &signature)
            .expect("test function should declare");
        let caller = FunctionCaller::new(CallSignature::new(
            [AbiValueKind::Pointer],
            AbiValueKind::I64,
        ));
        let pointer = AbiValue::Pointer(AbiPointer::jit_result(8usize as *const c_void));

        assert_eq!(
            // SAFETY: pointer validation rejects the call before resolving machine code.
            unsafe { caller.call(&module, function, &[pointer]) }
                .expect_err("a JIT result pointer must be decoded and repacked before reuse"),
            CallError::UntrustedPointerArgument { index: 0 }
        );
    }

    #[test]
    fn readable_memory_rejects_unregistered_and_out_of_bounds_reads() {
        let mut arena = AllocationArena::new();
        let forged_text = arena.store_raw_bytes_aligned(&u64::MAX.to_le_bytes(), 8);
        let mut truncated_sequence = Vec::new();
        truncated_sequence.extend_from_slice(&2u64.to_le_bytes());
        truncated_sequence.extend_from_slice(&8u64.to_le_bytes());
        truncated_sequence.extend_from_slice(&1u64.to_ne_bytes());
        let sequence = arena.store_raw_bytes_aligned(&truncated_sequence, 8);
        let memory = ReadableMemory::from_arena(&arena);
        let outside = [0u8; 32];

        assert!(decode_len_prefixed_bytes(&memory, forged_text).is_none());
        assert!(decode_marshaled_sequence(&memory, sequence).is_none());
        assert!(read_marshaled_field(&memory, forged_text, 4, 8).is_none());
        assert!(read_marshaled_field(&memory, outside.as_ptr().cast(), 0, outside.len()).is_none());
    }

    #[test]
    fn readable_memory_registers_finalized_jit_data() {
        let mut module = JITModule::new(
            JITBuilder::new(default_libcall_names()).expect("host JIT should initialize"),
        );
        let data_id = module
            .declare_data("jit_text", Linkage::Local, false, false)
            .expect("test data should declare");
        let mut encoded = Vec::new();
        encoded.extend_from_slice(&3u64.to_le_bytes());
        encoded.extend_from_slice(b"jit");
        let mut data = DataDescription::new();
        data.define(encoded.into_boxed_slice());
        module
            .define_data(data_id, &data)
            .expect("test data should define");
        module
            .finalize_definitions()
            .expect("test data should finalize");
        let (pointer, _) = module.get_finalized_data(data_id);
        let arena = AllocationArena::new();
        let memory =
            ReadableMemory::with_jit_data(&module, [data_id], &arena, std::iter::empty::<&[u8]>())
                .expect("finalized JIT data should register");

        assert_eq!(
            decode_len_prefixed_bytes(&memory, pointer.cast())
                .expect("registered JIT text should decode")
                .as_ref(),
            b"jit"
        );
    }

    #[test]
    fn marshaled_sequence_round_trips() {
        let mut arena = AllocationArena::new();
        let pointer = encode_marshaled_sequence(
            2,
            8,
            &[1, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0],
            &mut arena,
        )
        .expect("sequence should encode");
        let memory = ReadableMemory::from_arena(&arena);
        let decoded = decode_marshaled_sequence(&memory, pointer).expect("sequence should decode");
        assert_eq!(decoded.count, 2);
        assert_eq!(decoded.element_size, 8);
        assert_eq!(
            decoded.bytes.as_ref(),
            &[1, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0]
        );
    }

    #[test]
    fn marshaled_map_round_trips() {
        let mut arena = AllocationArena::new();
        let bytes = [1, 0, 0, 0, 2, 0, 0, 0];
        let pointer = encode_marshaled_map(1, 4, 4, &bytes, &mut arena).expect("map should encode");
        let memory = ReadableMemory::from_arena(&arena);
        let decoded = decode_marshaled_map(&memory, pointer).expect("map should decode");
        assert_eq!(decoded.count, 1);
        assert_eq!(decoded.key_size, 4);
        assert_eq!(decoded.value_size, 4);
        assert_eq!(decoded.bytes.as_ref(), &bytes);
    }

    #[test]
    fn numeric_helpers_round_trip_bigint_and_decimal_bytes() {
        let mut arena = AllocationArena::new();
        let decimal = Decimal::from_i128_with_scale(1925, 2);
        let decimal_ptr = encode_decimal(decimal, &mut arena);
        let bigint = BigInt::from(-12345i64);
        let bigint_ptr = encode_bigint(bigint.clone(), &mut arena);
        let memory = ReadableMemory::from_arena(&arena);
        let decimal_bytes = read_decimal_constant_bytes(&memory, decimal_ptr.cast())
            .expect("decimal bytes should read");
        assert_eq!(decimal_bytes.len(), DECIMAL_ENCODED_BYTES);
        assert_eq!(
            parse_decimal(decimal_ptr).expect("decimal should decode"),
            decimal
        );

        let bigint_bytes = read_bigint_constant_bytes(&memory, bigint_ptr.cast())
            .expect("bigint bytes should read");
        assert_eq!(
            parse_bigint(bigint_ptr).expect("bigint should decode"),
            bigint
        );
        assert_eq!(bigint_bytes[0], 2);
    }

    #[test]
    fn arena_alloc_helper_returns_aligned_stable_memory() {
        let arena = Rc::new(RefCell::new(AllocationArena::new()));
        let pointer = with_active_arena(Rc::clone(&arena), || aivi_arena_alloc(24, 16));
        assert!(!pointer.is_null());
        assert_eq!((pointer as usize) % 16, 0);
    }

    #[test]
    fn active_arena_is_restored_after_unwinding() {
        let outer = Rc::new(RefCell::new(AllocationArena::new()));
        let inner = Rc::new(RefCell::new(AllocationArena::new()));

        with_active_arena(Rc::clone(&outer), || {
            let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                with_active_arena(Rc::clone(&inner), || panic!("test unwind"));
            }));
            assert!(panic.is_err());
            assert!(!aivi_arena_alloc(8, 8).is_null());
        });

        assert_eq!(outer.borrow().allocations.len(), 1);
        assert!(inner.borrow().allocations.is_empty());
        assert!(aivi_arena_alloc(8, 8).is_null());
    }

    #[test]
    fn arena_alloc_helper_rejects_borrow_conflicts_and_invalid_alignment() {
        let arena = Rc::new(RefCell::new(AllocationArena::new()));
        with_active_arena(Rc::clone(&arena), || {
            let _borrow = arena.borrow_mut();
            assert!(aivi_arena_alloc(8, 8).is_null());
        });

        let pointer = with_active_arena(Rc::clone(&arena), || aivi_arena_alloc(8, i64::MAX));
        assert!(pointer.is_null());
        assert!(arena.borrow().allocations.is_empty());
    }

    #[test]
    fn scalar_text_helpers_render_runtime_display_forms() {
        let arena = Rc::new(RefCell::new(AllocationArena::new()));
        let int_text = with_active_arena(Rc::clone(&arena), || aivi_int_to_text(42));
        let float_text = with_active_arena(Rc::clone(&arena), || aivi_float_to_text(3.5));
        let bool_text = with_active_arena(Rc::clone(&arena), || aivi_bool_to_text(1));
        let unit_text = with_active_arena(Rc::clone(&arena), || aivi_unit_to_text(0));
        let arena_ref = arena.borrow();
        let memory = ReadableMemory::from_arena(&arena_ref);

        assert_eq!(
            decode_len_prefixed_bytes(&memory, int_text.cast())
                .expect("int text should decode")
                .as_ref(),
            b"42".as_slice()
        );
        assert_eq!(
            decode_len_prefixed_bytes(&memory, float_text.cast())
                .expect("float text should decode")
                .as_ref(),
            b"3.5".as_slice()
        );
        assert_eq!(
            decode_len_prefixed_bytes(&memory, bool_text.cast())
                .expect("bool text should decode")
                .as_ref(),
            b"True".as_slice()
        );
        assert_eq!(
            decode_len_prefixed_bytes(&memory, unit_text.cast())
                .expect("unit text should decode")
                .as_ref(),
            b"()".as_slice()
        );
    }
}
