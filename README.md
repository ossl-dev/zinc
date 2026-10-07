<img src="ZINC.png" alt="Zinc logo" width="100%" height="auto"/>

# Zinc - Universal Shared Memory Library

[![CI](https://github.com/ossl-dev/zinc/actions/workflows/ci.yml/badge.svg)](https://github.com/ossl-dev/zinc/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](core/Cargo.toml)
[![Rust](https://img.shields.io/badge/core-Rust-orange)](core/)
[![Python](https://img.shields.io/badge/adapter-Python-3776AB)](adapters/python/)
[![Go](https://img.shields.io/badge/adapter-Go-00ADD8)](adapters/go/)
[![Node.js](https://img.shields.io/badge/adapter-Node.js-339933)](adapters/node/)
[![Bun](https://img.shields.io/badge/adapter-Bun-FBF0DF)](adapters/bun/)
[![Deno](https://img.shields.io/badge/adapter-Deno-000000)](adapters/deno/)
[![C++](https://img.shields.io/badge/adapter-C++-00599C)](adapters/cpp/)
[![Java](https://img.shields.io/badge/adapter-Java-ED8B00)](adapters/java/)
[![C#](https://img.shields.io/badge/adapter-C%23-512BD4)](adapters/csharp/)

Zinc gives processes in **any language** access to the same memory pages, across Rust, Python, Go, Node.js, Bun, Deno, C++, Java, C#, and more. The data path uses `mmap` with zero-copy views. Notifications use Linux futex calls or a macOS polling fallback.

**Windows is not supported.** Zinc is a Linux/macOS library built on POSIX shared memory (`shm_open` + `mmap`).

Think `SharedArrayBuffer`, but cross-language and cross-process.

---

## Why

`SharedArrayBuffer` lets worker threads share memory within a process. Sharing video frames, model outputs, or game state across processes often means serializing data through a socket or writing platform-specific shared-memory code. Zinc provides shared mappings through one core and a set of language adapters.

Zinc maps the same physical RAM pages into both processes via POSIX shared memory. Write a float in Python, read it in Go without serializing it or copying it through a socket. Mapping shared pages avoids an extra transfer step. Reads, writes, and synchronization still have a cost.

---

## Quick start

Use the adapters from a source checkout; registry publishing is still on the roadmap. Keep the creator running while readers open the region. These snippets use 16384 bytes, valid for common Linux and macOS page sizes; query the system page size for other targets.

```bash
git clone https://github.com/ossl-dev/zinc
cd zinc
cargo build --release --manifest-path core/Cargo.toml
```

### Rust
```rust
use zinc_core::SharedRegion;

let region = SharedRegion::create("my-data", 16384)?;
unsafe { std::ptr::write(region.as_ptr() as *mut f32, 42.0) };
region.notify();
std::io::stdin().read_line(&mut String::new())?;
```

### Python
```python
from zinc import SharedRegion
import numpy as np

r = SharedRegion.open("my-data")
if not r.wait(1000):
    raise TimeoutError("writer did not publish data")
arr = r.as_numpy(dtype=np.float32)
print(arr[0])  # 42.0, same physical memory
```

### Go
```go
import (
    "zinc"
    "unsafe"
)

r, err := zinc.Open("my-data")
if err != nil { panic(err) }
defer r.Close()
if !r.Wait(1000) { panic("writer did not publish data") }
data := r.Bytes()
val := *(*float32)(unsafe.Pointer(&data[0]))
```

### TypeScript (Bun)
```ts
import { SharedRegion } from "./adapters/bun/src/index.ts";

const r = SharedRegion.open("my-data");
if (!r.wait(1000)) throw new Error("writer did not publish data");
const buffer = r.buffer();
const view = new Float32Array(buffer.buffer, buffer.byteOffset, buffer.byteLength / 4);
console.log(view[0]); // 42.0
```

That's it. Every language sees the same bytes. Use atomic data or an application protocol to coordinate concurrent access. Notify/wait publishes updates but does not provide mutual exclusion.

---

## How it works

```
┌──────────────────────────────────────┐
│  Python  │  Go  │  C++  │ Java │ C#  │  ← Adapters (thin FFI wrappers)
│  (cffi)  │(cgo) │(hpp)  │(JNA) │(P/Invoke)
├──────────────────────────────────────┤
│        include/zinc.h                │  ← C ABI (opaque void* handles)
│   zinc_create / zinc_open / ...      │
├──────────────────────────────────────┤
│        Rust core (cdylib)            │  ← Single source of truth
│   SharedRegion / Ring / Sync         │
├──────────────────────────────────────┤
│   POSIX shm_open + mmap + futex      │  ← Platform layer
│   (Linux, macOS)                     │
└──────────────────────────────────────┘
```

The C ABI exposes nine functions. FFI adapters call the core through that ABI; Rust and Node use the crate directly. The Rust core compiles to `libzinc_core.{so,dylib}`. Adapters handle native types, error translation, and view lifetimes.

### The 9 C functions

| Function | Purpose |
|---|---|
| `zinc_create` | Create owned region, returns opaque handle |
| `zinc_open` | Open existing region |
| `zinc_ptr` | Raw pointer to data area (after the first header page) |
| `zinc_capacity` | Usable bytes |
| `zinc_close` | Drop handle, unmap, maybe unlink |
| `zinc_notify` | Signal waiters (futex on Linux, polling on macOS) |
| `zinc_wait` | Block until notified or timeout |
| `zinc_try_wait` | Consume a pending notification without blocking |
| `zinc_version` | Major/minor version for compatibility checks |

### Ownership model

- Creator owns the segment, others open it
- Ref-counted via atomic in the 64-byte header
- Closing the creator unlinks the name
- Existing mappings remain valid until their handles close
- Crash recovery remains on the roadmap

---

## Performance

Shared mappings avoid a separate data transfer. Reads and writes still use memory bandwidth and cache coherence, and notifications add synchronization costs. See the [benchmark documentation](docs/performance/benchmarks.mdx) for measured workloads and their limits.

Target: notify/wait roundtrip < 5µs on Linux.

---

## Language adapters

| Language | Mechanism | Status | Path | README |
|---|---|---|---|---|
| Rust | Direct crate | Core ready | `core/` | [API](docs/adapters/rust.mdx) |
| Python | cffi + numpy | Tests passing | `adapters/python/` | [README](adapters/python/README.md) |
| Go | cgo | Tests passing | `adapters/go/` | [README](adapters/go/README.md) |
| Node.js | napi-rs | Tests passing | `adapters/node/` | [README](adapters/node/README.md) |
| Bun | bun:ffi | Tests passing | `adapters/bun/` | [README](adapters/bun/README.md) |
| Deno | Deno.dlopen | Tests passing | `adapters/deno/` | [README](adapters/deno/README.md) |
| C++ | Header-only RAII | Tests passing | `adapters/cpp/` | [README](adapters/cpp/README.md) |
| Java | JNA | Tests passing | `adapters/java/` | [README](adapters/java/README.md) |
| C# | P/Invoke | Tests passing | `adapters/csharp/` | [README](adapters/csharp/README.md) |


---

## Building

```bash
# Build the Rust core (generates libzinc_core + include/zinc.h)
cargo build --release --manifest-path core/Cargo.toml
```

Output: `target/release/libzinc_core.{so,dylib}`

---

## Prerequisites

- **Rust 1.99.0** for development (pinned via rustup); the core library supports Rust 1.85 or later, [rustup.rs](https://rustup.rs/)
- Optional: **Moon ≥ 2.0**, [moonrepo.dev](https://moonrepo.dev/) (monorepo tool)
- Language runtimes as needed

---

## Development

See [`DEVELOPMENT_GUIDE.md`](./DEVELOPMENT_GUIDE.md) for full build instructions, test suite, and architecture notes.

---

## License

MIT (declared in the Cargo package metadata).
