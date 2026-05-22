# Zinc — Universal Shared Memory Library

Zinc gives processes in **any language** direct access to the same physical memory — across Rust, Python, Go, Node.js, Bun, Deno, C++, Java, C#, and more. No serialization, no copies, no kernel round-trips. Just `mmap` under the hood and a zero-copy view of the same bytes in every language.

Think `SharedArrayBuffer`, but cross-language and cross-process.

---

## Why

`SharedArrayBuffer` lets Worker threads share memory within a single process. There's no equivalent across processes or languages. If you want two processes to share a large dataset — video frames, ML model outputs, game state, a shared cache — your options are: serialize it, copy it through a socket, deserialize it on the other side.

Zinc maps the same physical RAM pages into both processes via POSIX shared memory. Write a float in Python, read it in Go. No syscall, no copy, no serialization. The transfer time is zero because there's nothing to transfer — it's already there.

---

## Quick start

```bash
git clone https://github.com/aspect-build/zinc
cd zinc
cargo build --release --manifest-path core/Cargo.toml
```

### Rust
```rust
use zinc_core::SharedRegion;

let region = SharedRegion::create("/my-data", 4096)?;
unsafe { std::ptr::write(region.as_ptr() as *mut f32, 42.0) };
region.notify();
```

### Python
```python
from zinc import SharedRegion

r = SharedRegion.open("/my-data")
arr = r.as_numpy(dtype=np.float32)
print(arr[0])  # 42.0 — same physical memory
```

### Go
```go
import "zinc"

r, _ := zinc.Open("/my-data")
data := r.Bytes()
val := *(*float32)(unsafe.Pointer(&data[0]))
```

### TypeScript (Bun / Deno / Node)
```ts
import { SharedRegion } from "zinc-bun";

const r = SharedRegion.open("/my-data");
const view = new Float32Array(r.buffer());
console.log(view[0]); // 42.0
```

That's it. Every language sees the same bytes. Use `Atomics` on a typed array view for synchronization if you need it.

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
│   (Linux, macOS, Windows stub)       │
└──────────────────────────────────────┘
```

Every language adapter calls the same 8 C functions via its native FFI mechanism. The Rust core compiles to `libzinc_core.{so,dylib,dll}`. No logic is reimplemented in adapters.

### The 8 C functions

| Function | Purpose |
|---|---|
| `zinc_create` | Create owned region, returns opaque handle |
| `zinc_open` | Open existing region |
| `zinc_ptr` | Raw pointer to data area (after 64-byte header) |
| `zinc_capacity` | Usable bytes |
| `zinc_close` | Drop handle, unmap, maybe unlink |
| `zinc_notify` | Signal waiters (futex on Linux, spin elsewhere) |
| `zinc_wait` | Block until notified or timeout |
| `zinc_version` | Major/minor version for compatibility checks |

### Ownership model

- Creator owns the segment, others open it
- Ref-counted via atomic in the 64-byte header
- Only the creator may unlink (enforced at the type level)
- Automatic cleanup when all handles are dropped

---

## Performance

The shared buffer path has no "transfer" to benchmark — both processes access the same RAM. The cost is a single `mmap` setup call, then memory reads/writes at native speed (CPU cache → RAM bandwidth).

Target: notify/wait roundtrip < 5µs on Linux.

---

## Language adapters

| Language | Mechanism | Path |
|---|---|---|
| Rust | Direct crate | `core/` |
| Python | cffi + numpy | `adapters/python/` |
| Go | cgo | `adapters/go/` |
| Node.js | napi-rs | `adapters/node/` |
| Bun | bun:ffi | `adapters/bun/` |
| Deno | Deno.dlopen | `adapters/deno/` |
| C++ | Header-only RAII | `adapters/cpp/` |
| Java | JNA | `adapters/java/` |
| C# | P/Invoke | `adapters/csharp/` |

---

## Building

```bash
# Build the Rust core (generates libzinc_core + include/zinc.h)
cargo build --release --manifest-path core/Cargo.toml
```

Output: `core/target/release/libzinc_core.{dylib,so,dll}`

---

## Prerequisites

- **Rust ≥ 1.82** — [rustup.rs](https://rustup.rs/)
- **Moon ≥ 2.0** — [moonrepo.dev](https://moonrepo.dev/) (monorepo tool)
- Language runtimes as needed

---

## Development

See [`DEVELOPMENT_GUIDE.md`](./DEVELOPMENT_GUIDE.md) for full build instructions, test suite, and architecture notes.

---

## License

MIT — see [`LICENSE`](./LICENSE).
