# Compilation Target: WebAssembly

AIVI compiles exclusively to **WebAssembly** (WASM).

---

## Why WASM

| Property | Benefit for AIVI |
| :--- | :--- |
| **Sandboxed** | Perfect for high-integrity data pipelines |
| **Portable** | Runs in browsers, edge workers, servers |
| **WasmGC** | Native garbage collection suits functional languages |
| **Component Model** | First-class module system matches AIVI modules |
| **WASI** | Standardized system access without POSIX baggage |

---

## Target Features

### WasmGC (Garbage Collection)

AIVI leverages the WasmGC proposal for:
- Immutable reference types
- Efficient closures
- No need to ship a custom GC

### Component Model

AIVI modules map directly to WASM components:
- `module aivi/std/calendar` → standalone `.wasm` component
- Shared types via component interfaces
- Composable without recompilation

### WASI Preview 2

System access through capability-based handles:
- Files, sockets, clocks as typed resources
- No ambient authority
- Explicit effect tracking aligns with AIVI's `Effect` system

---

## Deployment Targets

| Environment | Runtime | Use Case |
| :--- | :--- | :--- |
| Browser | V8/SpiderMonkey | Interactive frontend |
| Edge | Cloudflare Workers, Fastly | Low-latency APIs |
| Server | Wasmtime, WasmEdge | Backend services |
| Embedded | wasm3 | IoT, constrained devices |

---

## Native Performance

For performance-critical workloads:
- **Wasmtime AOT** compiles WASM to native code
- **SIMD** available via explicit intrinsics
- No separate native backend needed

---

## Non-Goals

- Direct LLVM/native compilation
- C FFI (use WASM component imports instead)
- Platform-specific syscalls
