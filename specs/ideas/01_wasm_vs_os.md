# Evaluation: Compilation Targets (WASM vs OS)

## Current Landscape

AIVI's purely functional nature and explicit effect tracking make it suitable for multiple backends.

## 1. WebAssembly (WASM)

**Pros:**
* **Sandboxing**: Perfect for "high-integrity data pipelines" where untrusted code might need to run safely.
* **Portability**: Runs in browsers, edge workers (Cloudflare, Fastly), and servers (Wasmtime).
* **GC Integration**: The new WasmGC proposal suits functional languages well (avoids shipping a massive GC).

**Cons:**
* **DOM Interop**: Still has overhead, though `externref` helps.
* **System Access**: WASI is evolving but file I/O and networking are stricter/different than POSIX.

**Use Case Fit:**
AIVI as a **logic engine** embedded in other apps, or as a **backend-for-frontend** service layer.

## 2. Native OS (via LLVM or C transpilation)

**Pros:**
* **Performance**: Direct hardware access, SIMD, unboxed representations.
* **Ecosystem**: Easy C FFI.

**Cons:**
* **Build Complexity**: Cross-compilation headaches.
* **Safety**: Explicit memory management or a sophisticated GC needed.

## Recommendation

**Focus on WASM first.**

AIVI's features (generators, immutable data) map well to a high-level runtime. Targeting WASM + Garbage Collection proposal allows AIVI to be a "universal logic language" that runs everywhere safe and fast.

Native compilation can be achieved later via a WASM-to-Native compiler (like Wasmtime's JIT/AOT) or a dedicated backend if raw throughput becomes a bottleneck.
