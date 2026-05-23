# Zinc — Development Guide

Zinc is a cross-process shared memory library with a Rust core and C ABI surface, enabling zero-copy data sharing across **any** language — Python, Node.js, Bun, Deno, Go, C++, Java, C#, and more. 

One Rust crate compiles to `libzinc_core.{so,dylib}`. Every language adapter calls the same C ABI via its native FFI mechanism. No reimplementation of logic in adapters.

---

## Code Structure

```
zinc/
├── .moon/                     # Moon monorepo config
│   ├── workspace.yml
│   └── toolchain.yml
├── .github/workflows/ci.yml   # 2-platform CI (Linux, macOS)
│
├── core/                      # Rust — the heart of everything
│   ├── Cargo.toml
│   ├── build.rs               # cbindgen → ../include/zinc.h
│   ├── cbindgen.toml
│   ├── moon.yml               # Moon task config
│   └── src/
│       ├── lib.rs             # pub(crate) re-exports + extern "C" surface
│       ├── error.rs           # ZincError (thiserror)
│       ├── header.rs          # RegionHeader — #[repr(C, align(64))]
│       ├── region.rs          # SharedRegion — create/open/close/unlink
│       ├── ring.rs            # Lock-free MPSC notification ring
│       ├── sync.rs            # Cross-process notify/wait (futex + spin fallback)
│       └── platform/
│           ├── mod.rs         # cfg-gated dispatch
│           ├── unix.rs        # Shared POSIX backend (shm_open + mmap)
│           ├── linux.rs       # Re-exports unix
│           ├── macos.rs       # Re-exports unix
│
├── include/                   # cbindgen output (committed)
│   └── zinc.h
│
├── adapters/
│   ├── node/                  # napi-rs → .node addon
│   ├── bun/                   # bun:ffi
│   ├── deno/                  # Deno.dlopen
│   ├── python/                # cffi + numpy zero-copy
│   ├── go/                    # cgo
│   ├── cpp/                   # Header-only RAII wrapper
│   ├── java/                  # JNA
│   └── csharp/                # P/Invoke
│
├── benches/throughput.rs      # Throughput + latency benchmark
├── tests/runner.sh            # Cross-language integration tests
├── RFC-001.md                 # Architecture rationale
└── NEW_ARCHITECTURE.md        # Full implementation plan
```

Start reading in `core/src/region.rs` — that's where `SharedRegion::create()` and `SharedRegion::open()` live.

---

## Prerequisites

- **Rust ≥ 1.82** — [rustup.rs](https://rustup.rs/)
- **Moon ≥ 2.0** — `curl -fsSL https://moonrepo.dev/install/moon.sh | bash`
- Language runtimes as needed (Python, Node, Go, etc.)

```bash
rustc --version && cargo --version && moon --version
```

---

## Building

```bash
# Build the Rust core (generates libzinc_core.dylib + include/zinc.h)
cargo build --release --manifest-path core/Cargo.toml

# Or via Moon
moon run core:build
```

Outputs:
- `core/target/release/libzinc_core.{dylib,so,dll}` — loaded by all adapters via FFI
- `include/zinc.h` — auto-generated C header (opaque `void*` handles)

---

## Tests

```bash
# Rust core unit tests (8 tests: create/open/read/write/refcount/notify/wait)
cargo test --manifest-path core/Cargo.toml

# With linting
cargo clippy --manifest-path core/Cargo.toml -- -D warnings

# Cross-language integration tests
bash tests/runner.sh
```

---

## Benchmarks

```bash
cargo run --release --manifest-path core/Cargo.toml --example throughput
# Or run directly:
cargo run --release --bin zinc_bench
```

---

## Architecture Notes

### The C ABI is the contract

Every adapter calls the same 8 C functions in `include/zinc.h`:

| Function | Purpose |
|---|---|
| `zinc_create` | Create owned region, returns opaque handle |
| `zinc_open` | Open existing region |
| `zinc_ptr` | Raw pointer to data area (after 64-byte header) |
| `zinc_capacity` | Usable bytes |
| `zinc_close` | Drop handle, unmap, maybe unlink |
| `zinc_notify` | Signal waiters (futex on Linux, atomic+spin elsewhere) |
| `zinc_wait` | Block until notified or timeout |
| `zinc_version` | Major/minor version for compatibility checks |

### Ownership model

`SharedRegion` creator owns the segment. Others open it. Ref-counted via atomic in the 64-byte header. Unlink only by the owner. Enforced at the type level.

### Performance

- `RegionHeader` is exactly 64 bytes (one cache line) — no false sharing
- `parking_lot` mutexes (not std), `CachePadded` atomics
- Lock-free MPSC ring (256 slots, cache-aligned) for notification tokens
- Zero heap allocations in hot path (`zinc_ptr`, `zinc_notify`)
- Linux: futex for kernel-assisted wait. macOS: adaptive spin with yield

### Platform Support

- **Linux**: `shm_open` + `mmap` + futex — full support
- **macOS**: `shm_open` + `mmap` + spin-wait — full support

**Windows is not supported.** Zinc is a POSIX-only library. `shm_open` and `mmap` do not exist on Windows, and there are no plans to port them.

---

## Making Changes

**Rust core (`core/`):**

```bash
cargo test --manifest-path core/Cargo.toml
cargo clippy --manifest-path core/Cargo.toml -- -D warnings
```

The header regenerates automatically on build via `cbindgen`. Never edit `include/zinc.h` by hand.

**Language adapter:**

Rebuild the core first (`cargo build --release`), then test the adapter against the fresh library.

> **Windows is not supported.** Zinc requires `shm_open` and `mmap`, which are POSIX APIs not available on Windows.

---

## Commit Convention

```
[component] short description
```

Components: `core`, `adapter/node`, `adapter/python`, `adapter/go`, etc.

Examples: `[core] add notify_seq field for proper futex sync`, `[adapter/python] add as_numpy zero-copy view`
