//! Persistent cache surfaces for compiled backend artifacts.
//!
//! The backend keeps two related identities:
//! - a stable content fingerprint for a backend program or kernel, used by query/runtime layers to
//!   decide whether a compilation unit is semantically unchanged, and
//! - a disk-cache key that layers compiler-version and codegen-target namespace data on top of that
//!   stable fingerprint before reading or writing machine-code artifacts under XDG cache.
//!
//! Cache misses and corrupt entries are treated as non-fatal misses; the backend simply recompiles
//! and rewrites a fresh artifact.

use std::{
    collections::{BTreeMap, btree_map::Entry},
    env, fs,
    hash::{Hash, Hasher},
    io::{Cursor, Read},
    path::{Path, PathBuf},
};

use cranelift_codegen::binemit::Reloc;
use rustc_hash::FxHasher;

use crate::{
    CompiledKernel, CompiledKernelArtifact, CompiledProgram, KernelFingerprint, KernelId,
    codegen::{
        CachedJitCallableDescriptor, CachedJitCompiledKernel, CachedJitDataSlot,
        CachedJitFunctionTarget, CachedJitKernelArtifact, CachedJitLiteralData, CachedJitReloc,
        CachedJitRelocTarget, compile_kernel, compile_kernel_jit_with_cache_artifact,
        compile_program, compute_kernel_fingerprint, instantiate_cached_jit_kernel,
    },
    program::Program,
};

/// Magic bytes: ASCII "AIVI" + format version byte.
const PROGRAM_CACHE_MAGIC: &[u8; 5] = b"AIVI\x02";
/// Magic bytes: ASCII "AIVK" + format version byte.
const KERNEL_CACHE_MAGIC: &[u8; 5] = b"AIVK\x01";
/// Magic bytes: ASCII "AIVJ" + format version byte.
const JIT_KERNEL_CACHE_MAGIC: &[u8; 5] = b"AIVJ\x01";

const COMPILER_VERSION: &str = env!("CARGO_PKG_VERSION");
const SHARED_CODEGEN_SETTINGS: &[(&str, &str)] =
    &[("enable_llvm_abi_extensions", "1"), ("opt_level", "speed")];

/// In-memory cache for per-kernel object artifacts owned by the backend layer.
#[derive(Clone, Debug, Default)]
pub struct BackendKernelArtifactCache {
    artifacts: BTreeMap<KernelFingerprint, CompiledKernelArtifact>,
}

impl BackendKernelArtifactCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.artifacts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.artifacts.is_empty()
    }

    pub fn get(&self, fingerprint: KernelFingerprint) -> Option<&CompiledKernelArtifact> {
        self.artifacts.get(&fingerprint)
    }

    pub fn get_by_kernel(
        &self,
        program: &Program,
        kernel_id: KernelId,
    ) -> Option<&CompiledKernelArtifact> {
        if !program.kernels().contains(kernel_id) {
            return None;
        }
        self.get(compute_kernel_fingerprint(program, kernel_id))
    }

    pub fn insert(&mut self, artifact: CompiledKernelArtifact) -> Option<CompiledKernelArtifact> {
        self.artifacts.insert(artifact.fingerprint(), artifact)
    }

    pub fn get_or_compile(
        &mut self,
        program: &Program,
        kernel_id: KernelId,
    ) -> Result<&CompiledKernelArtifact, CodegenErrors> {
        if !program.kernels().contains(kernel_id) {
            let error = compile_kernel(program, kernel_id)
                .err()
                .expect("compiling a missing kernel should produce a backend codegen error");
            return Err(error);
        }
        let fingerprint = compute_kernel_fingerprint(program, kernel_id);
        match self.artifacts.entry(fingerprint) {
            Entry::Occupied(entry) => Ok(entry.into_mut()),
            Entry::Vacant(entry) => {
                let artifact = compile_kernel(program, kernel_id)?;
                Ok(entry.insert(artifact))
            }
        }
    }
}

/// Compute a stable content fingerprint for one backend program.
pub fn compute_program_fingerprint(program: &Program) -> u64 {
    let mut hasher = FxHasher::default();
    format!("{program:?}").hash(&mut hasher);
    hasher.finish()
}

/// Compute a stable 64-bit disk-cache key by layering compiler/codegen namespace
/// identity over a stable backend-program fingerprint.
pub fn compute_program_cache_key_from_fingerprint(fingerprint: u64) -> u64 {
    fingerprint ^ cache_namespace_hash().rotate_left(32)
}

/// Compute a stable 64-bit cache key for a backend program.
pub fn compute_program_cache_key(program: &Program) -> u64 {
    compute_program_cache_key_from_fingerprint(compute_program_fingerprint(program))
}

/// Compute a disk-cache key for one kernel artifact from its stable content fingerprint.
pub fn compute_kernel_cache_key(fingerprint: KernelFingerprint) -> u64 {
    compute_program_cache_key_from_fingerprint(fingerprint.as_raw())
}

fn compiler_version_hash() -> u64 {
    let mut version_hasher = FxHasher::default();
    COMPILER_VERSION.hash(&mut version_hasher);
    version_hasher.finish()
}

fn cache_namespace_hash() -> u64 {
    let mut hasher = FxHasher::default();
    compiler_version_hash().hash(&mut hasher);
    native_codegen_target_identity().hash(&mut hasher);
    for (name, value) in SHARED_CODEGEN_SETTINGS {
        name.hash(&mut hasher);
        value.hash(&mut hasher);
    }
    hasher.finish()
}

fn native_codegen_target_identity() -> String {
    cranelift_native::builder()
        .map(|builder| builder.triple().to_string())
        .unwrap_or_else(|_| {
            format!(
                "{}-{}-{}",
                std::env::consts::ARCH,
                std::env::consts::OS,
                std::env::consts::FAMILY
            )
        })
}

fn cache_dir() -> Option<PathBuf> {
    let base = env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))?;
    Some(base.join("aivi").join("compiled"))
}

fn program_cache_path_in(cache_root: &Path, key: u64) -> PathBuf {
    cache_root.join(format!("program-{key:016x}.bin"))
}

fn kernel_cache_path_in(cache_root: &Path, key: u64) -> PathBuf {
    cache_root.join("kernels").join(format!("{key:016x}.bin"))
}

fn jit_kernel_cache_path_in(cache_root: &Path, key: u64) -> PathBuf {
    cache_root.join("jit-kernels").join(format!("{key:016x}.bin"))
}

fn read_u32(cursor: &mut Cursor<&[u8]>) -> Option<u32> {
    let mut buf = [0u8; 4];
    cursor.read_exact(&mut buf).ok()?;
    Some(u32::from_le_bytes(buf))
}

fn read_u64(cursor: &mut Cursor<&[u8]>) -> Option<u64> {
    let mut buf = [0u8; 8];
    cursor.read_exact(&mut buf).ok()?;
    Some(u64::from_le_bytes(buf))
}

fn read_u8(cursor: &mut Cursor<&[u8]>) -> Option<u8> {
    let mut buf = [0u8; 1];
    cursor.read_exact(&mut buf).ok()?;
    Some(buf[0])
}

fn read_boxed_str(cursor: &mut Cursor<&[u8]>) -> Option<Box<str>> {
    let len = read_u32(cursor)? as usize;
    let mut buf = vec![0u8; len];
    cursor.read_exact(&mut buf).ok()?;
    String::from_utf8(buf).ok().map(String::into_boxed_str)
}

fn read_boxed_bytes(cursor: &mut Cursor<&[u8]>) -> Option<Box<[u8]>> {
    let len = read_u64(cursor)? as usize;
    let mut buf = vec![0u8; len];
    cursor.read_exact(&mut buf).ok()?;
    Some(buf.into_boxed_slice())
}

fn write_boxed_str(buf: &mut Vec<u8>, value: &str) {
    let bytes = value.as_bytes();
    buf.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    buf.extend_from_slice(bytes);
}

fn write_boxed_bytes(buf: &mut Vec<u8>, value: &[u8]) {
    buf.extend_from_slice(&(value.len() as u64).to_le_bytes());
    buf.extend_from_slice(value);
}

fn serialize_compiled_kernel(buf: &mut Vec<u8>, kernel: &CompiledKernel) {
    buf.extend_from_slice(&kernel.kernel.as_raw().to_le_bytes());
    buf.extend_from_slice(&kernel.fingerprint.as_raw().to_le_bytes());

    let symbol = kernel.symbol.as_bytes();
    buf.extend_from_slice(&(symbol.len() as u32).to_le_bytes());
    buf.extend_from_slice(symbol);

    let clif = kernel.clif.as_bytes();
    buf.extend_from_slice(&(clif.len() as u32).to_le_bytes());
    buf.extend_from_slice(clif);

    buf.extend_from_slice(&(kernel.code_size as u64).to_le_bytes());
}

fn deserialize_compiled_kernel(cursor: &mut Cursor<&[u8]>) -> Option<CompiledKernel> {
    let kernel_raw = read_u32(cursor)?;
    let fingerprint = KernelFingerprint::new(read_u64(cursor)?);
    let symbol = read_boxed_str(cursor)?;
    let clif = read_boxed_str(cursor)?;
    let code_size = read_u64(cursor)? as usize;
    Some(CompiledKernel {
        kernel: KernelId::from_raw(kernel_raw),
        fingerprint,
        symbol,
        clif,
        code_size,
    })
}

fn serialize_program(compiled: &CompiledProgram) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(PROGRAM_CACHE_MAGIC);

    let object = compiled.object();
    buf.extend_from_slice(&(object.len() as u64).to_le_bytes());
    buf.extend_from_slice(object);

    let kernels = compiled.kernels();
    buf.extend_from_slice(&(kernels.len() as u32).to_le_bytes());
    for kernel in kernels {
        serialize_compiled_kernel(&mut buf, kernel);
    }
    buf
}

fn deserialize_program(bytes: &[u8]) -> Option<CompiledProgram> {
    let mut cursor = Cursor::new(bytes);

    let mut magic = [0u8; 5];
    cursor.read_exact(&mut magic).ok()?;
    if &magic != PROGRAM_CACHE_MAGIC {
        return None;
    }

    let object_len = read_u64(&mut cursor)? as usize;
    let mut object = vec![0u8; object_len];
    cursor.read_exact(&mut object).ok()?;

    let kernel_count = read_u32(&mut cursor)? as usize;
    let mut kernels = Vec::with_capacity(kernel_count);
    for _ in 0..kernel_count {
        kernels.push(deserialize_compiled_kernel(&mut cursor)?);
    }

    Some(CompiledProgram::new(object, kernels))
}

fn serialize_kernel_artifact(artifact: &CompiledKernelArtifact) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(KERNEL_CACHE_MAGIC);

    let object = artifact.object();
    buf.extend_from_slice(&(object.len() as u64).to_le_bytes());
    buf.extend_from_slice(object);
    serialize_compiled_kernel(&mut buf, artifact.metadata());
    buf
}

fn deserialize_kernel_artifact(bytes: &[u8]) -> Option<CompiledKernelArtifact> {
    let mut cursor = Cursor::new(bytes);

    let mut magic = [0u8; 5];
    cursor.read_exact(&mut magic).ok()?;
    if &magic != KERNEL_CACHE_MAGIC {
        return None;
    }

    let object_len = read_u64(&mut cursor)? as usize;
    let mut object = vec![0u8; object_len];
    cursor.read_exact(&mut object).ok()?;
    let metadata = deserialize_compiled_kernel(&mut cursor)?;
    Some(CompiledKernelArtifact::new(object, metadata))
}

/// Load a cached `CompiledProgram` for the given key, if a valid entry exists.
pub fn load_cached_program(key: u64) -> Option<CompiledProgram> {
    let cache_root = cache_dir()?;
    load_cached_program_from(&cache_root, key)
}

fn load_cached_program_from(cache_root: &Path, key: u64) -> Option<CompiledProgram> {
    let path = program_cache_path_in(cache_root, key);
    let bytes = fs::read(&path).ok()?;
    deserialize_program(&bytes)
}

/// Persist a `CompiledProgram` to the disk cache under the given key.
/// Silently ignores I/O failures so a missing or read-only cache never breaks compilation.
pub fn store_cached_program(key: u64, compiled: &CompiledProgram) {
    let Some(cache_root) = cache_dir() else {
        return;
    };
    store_cached_program_in(&cache_root, key, compiled);
}

fn store_cached_program_in(cache_root: &Path, key: u64, compiled: &CompiledProgram) {
    let path = program_cache_path_in(cache_root, key);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(&path, serialize_program(compiled));
}

/// Load a cached per-kernel object artifact, if a valid entry exists.
pub fn load_cached_kernel_artifact(
    fingerprint: KernelFingerprint,
) -> Option<CompiledKernelArtifact> {
    let cache_root = cache_dir()?;
    load_cached_kernel_artifact_from(&cache_root, fingerprint)
}

fn load_cached_kernel_artifact_from(
    cache_root: &Path,
    fingerprint: KernelFingerprint,
) -> Option<CompiledKernelArtifact> {
    let path = kernel_cache_path_in(cache_root, compute_kernel_cache_key(fingerprint));
    let bytes = fs::read(&path).ok()?;
    let artifact = deserialize_kernel_artifact(&bytes)?;
    (artifact.fingerprint() == fingerprint).then_some(artifact)
}

/// Persist a per-kernel object artifact to the disk cache.
/// Silently ignores I/O failures so a missing or read-only cache never breaks compilation.
pub fn store_cached_kernel_artifact(
    fingerprint: KernelFingerprint,
    artifact: &CompiledKernelArtifact,
) {
    if artifact.fingerprint() != fingerprint {
        return;
    }
    let Some(cache_root) = cache_dir() else {
        return;
    };
    store_cached_kernel_artifact_in(&cache_root, fingerprint, artifact);
}

fn store_cached_kernel_artifact_in(
    cache_root: &Path,
    fingerprint: KernelFingerprint,
    artifact: &CompiledKernelArtifact,
) {
    let path = kernel_cache_path_in(cache_root, compute_kernel_cache_key(fingerprint));
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(&path, serialize_kernel_artifact(artifact));
}

/// Compile a backend program, consulting the disk cache first to skip Cranelift
/// codegen for unchanged programs. Falls back to full compilation on cache miss
/// or any deserialization error.
pub fn compile_program_cached(program: &Program) -> Result<CompiledProgram, CodegenErrors> {
    let Some(cache_root) = cache_dir() else {
        return compile_program(program);
    };
    compile_program_cached_in_dir(&cache_root, program)
}

fn compile_program_cached_in_dir(
    cache_root: &Path,
    program: &Program,
) -> Result<CompiledProgram, CodegenErrors> {
    let key = compute_program_cache_key(program);
    if let Some(cached) = load_cached_program_from(cache_root, key) {
        return Ok(cached);
    }
    let compiled = compile_program(program)?;
    store_cached_program_in(cache_root, key, &compiled);
    Ok(compiled)
}

/// Compile one backend kernel, consulting the disk cache first to skip Cranelift codegen for
/// unchanged per-kernel artifacts.
pub fn compile_kernel_cached(
    program: &Program,
    kernel_id: KernelId,
) -> Result<CompiledKernelArtifact, CodegenErrors> {
    if !program.kernels().contains(kernel_id) {
        return compile_kernel(program, kernel_id);
    }
    let Some(cache_root) = cache_dir() else {
        return compile_kernel(program, kernel_id);
    };
    compile_kernel_cached_in_dir(&cache_root, program, kernel_id)
}

fn compile_kernel_cached_in_dir(
    cache_root: &Path,
    program: &Program,
    kernel_id: KernelId,
) -> Result<CompiledKernelArtifact, CodegenErrors> {
    let fingerprint = compute_kernel_fingerprint(program, kernel_id);
    if let Some(cached) = load_cached_kernel_artifact_from(cache_root, fingerprint) {
        return Ok(cached);
    }
    let compiled = compile_kernel(program, kernel_id)?;
    store_cached_kernel_artifact_in(cache_root, fingerprint, &compiled);
    Ok(compiled)
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use aivi_base::SourceDatabase;
    use aivi_core::{lower_module as lower_core_module, validate_module as validate_core_module};
    use aivi_lambda::{
        lower_module as lower_lambda_module, validate_module as validate_lambda_module,
    };
    use aivi_syntax::parse_module;

    use super::*;
    use crate::{lower_module as lower_backend_module, validate_program};

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
        validate_lambda_module(&lambda)
            .expect("typed lambda should validate before backend lowering");

        let backend = lower_backend_module(&lambda).expect("backend lowering should succeed");
        validate_program(&backend).expect("backend program should validate");
        backend
    }

    fn find_item(program: &Program, name: &str) -> crate::ItemId {
        program
            .items()
            .iter()
            .find(|(_, item)| item.name.as_ref() == name)
            .map(|(id, _)| id)
            .unwrap_or_else(|| panic!("expected backend item `{name}`"))
    }

    fn with_temp_cache_dir<R>(f: impl FnOnce(&Path) -> R) -> R {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "aivi-backend-cache-test-{}-{unique}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("temp cache root should be created");
        let result = f(&dir);
        let _ = fs::remove_dir_all(&dir);
        result
    }

    #[test]
    fn program_cache_key_reuses_stable_fingerprint_with_namespace_layering() {
        let backend = lower_text(
            "cache-program-fingerprint.aivi",
            "value total:Int = 21 + 21\nvalue other:Int = 1 + 1\n",
        );
        let changed = lower_text(
            "cache-program-fingerprint.aivi",
            "value total:Int = 21 + 21\nvalue other:Int = 2 + 2\n",
        );

        let fingerprint = compute_program_fingerprint(&backend);

        assert_eq!(
            compute_program_cache_key(&backend),
            compute_program_cache_key_from_fingerprint(fingerprint)
        );
        assert_ne!(fingerprint, compute_program_fingerprint(&changed));
        assert_ne!(
            compute_program_cache_key(&backend),
            compute_program_cache_key(&changed)
        );
    }

    #[test]
    fn compile_program_cached_recovers_from_corrupt_disk_entry() {
        let backend = lower_text("cache-program-corrupt.aivi", "value total:Int = 21 + 21\n");

        with_temp_cache_dir(|cache_root| {
            let key = compute_program_cache_key(&backend);
            let path = program_cache_path_in(cache_root, key);
            fs::create_dir_all(
                path.parent()
                    .expect("cache file should have a parent directory"),
            )
            .expect("program cache parent should be created");
            fs::write(&path, b"corrupt-program-cache")
                .expect("corrupt program cache entry should be written");

            let compiled = compile_program_cached_in_dir(cache_root, &backend)
                .expect("corrupt program cache should recompile");
            let loaded = load_cached_program_from(cache_root, key)
                .expect("recompiled program cache entry should deserialize cleanly");

            assert_eq!(compiled, loaded);
            assert_ne!(
                fs::read(&path).expect("recompiled cache file should be readable"),
                b"corrupt-program-cache"
            );
        });
    }

    #[test]
    fn compile_kernel_cached_recovers_from_corrupt_disk_entry() {
        let backend = lower_text("cache-kernel-corrupt.aivi", "value total:Int = 21 + 21\n");
        let total = backend.items()[find_item(&backend, "total")]
            .body
            .expect("total should lower into a body kernel");

        with_temp_cache_dir(|cache_root| {
            let fingerprint = compute_kernel_fingerprint(&backend, total);
            let path = kernel_cache_path_in(cache_root, compute_kernel_cache_key(fingerprint));
            fs::create_dir_all(
                path.parent()
                    .expect("cache file should have a parent directory"),
            )
            .expect("kernel cache parent should be created");
            fs::write(&path, b"corrupt-kernel-cache")
                .expect("corrupt kernel cache entry should be written");

            let compiled = compile_kernel_cached_in_dir(cache_root, &backend, total)
                .expect("corrupt kernel cache should recompile");
            let loaded = load_cached_kernel_artifact_from(cache_root, fingerprint)
                .expect("recompiled kernel cache entry should deserialize cleanly");

            assert_eq!(compiled, loaded);
            assert_ne!(
                fs::read(&path).expect("recompiled cache file should be readable"),
                b"corrupt-kernel-cache"
            );
        });
    }
}
