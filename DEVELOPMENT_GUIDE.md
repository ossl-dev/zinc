# Zinc development guide

Zinc maps shared memory between processes on Linux and macOS. The Rust core owns mapping, lifecycle, and notification behavior. Language adapters expose that core through C FFI or napi-rs.

## Repository layout

| Path | Purpose |
|------|---------|
| `core/src/region.rs` | Region creation, validation, notification cursors, cleanup |
| `core/src/platform/unix.rs` | POSIX shared memory and mapping calls |
| `core/src/header.rs` | The 64-byte, versioned header |
| `core/src/sync.rs` | Linux futex wait and macOS polling fallback |
| `core/src/ring.rs` | Configurable queue with multiple producers and consumers |
| `core/src/lib.rs` | C ABI and Rust exports |
| `core/benches/` | Criterion latency, write throughput, and ring benchmarks |
| `core/examples/` | Comparison programs and the Rust/Python interop fixture |
| `include/zinc.h` | Generated C header |
| `adapters/` | Language bindings and integration tests |
| `tests/runner.sh` | Local integration runner |
| `RFC-001.md` | Rationale for the shared-memory API |
| `ROADMAP.md` | Completed work and planned features |

## Build

Development uses Rust 1.99.0, pinned in `rust-toolchain.toml`, Moon, and CI. Rustup installs the pinned compiler, rustfmt, and Clippy when you run Cargo in this checkout. The core library still supports Rust 1.85; CI checks that minimum separately. Moon is optional.

If Homebrew's Cargo takes precedence over rustup, put `~/.cargo/bin` first in your shell's `PATH` to use the repository pin.

```bash
cargo build --release -p zinc-core
```

The Cargo workspace writes libraries to `target/release/`, not `core/target/`. The build generates `include/zinc.h`; change Rust declarations or `core/cbindgen.toml` rather than editing the header.

The Node addon is a separate Cargo package because it has a different release and runtime integration:

```bash
cd adapters/node
npm ci
npm run build
npm test
```

## Verify

```bash
cargo fmt --all -- --check
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

For adapters, build the core first. Python tests also use the interop example:

```bash
cargo build --release -p zinc-core --lib --example interop --locked
python3 -m pip install -e 'adapters/python[test]'
bash tests/runner.sh
```

The runner tests installed language runtimes and prints explicit skips for missing tools. Set `ZINC_PYTHON` to use a virtual environment. Java requires Maven; C# requires the .NET 8 SDK. CI runs all adapters on Linux and macOS, and the core also has an aarch64 Linux job.

The cross-process test starts a Rust creator, reads its bytes and notification from Python, and then shuts down the creator. It fails if creation, sharing, or cleanup fails.

## Benchmarks

```bash
cargo bench -p zinc-core --bench latency
cargo bench -p zinc-core --bench throughput
cargo bench -p zinc-core --bench ring
```

Latency uses two regions for a thread ping-pong. Throughput measures writes and the same-thread notification fast path. Ring benchmarks measure push/pop and batches with contending producers, including thread startup. These are different workloads; do not present their timings as interchangeable IPC latency.

The comparison examples are exploratory programs. `grpc` uses protobuf messages over length-prefixed TCP, not HTTP/2 gRPC. `redis` needs a local Redis server. See the [benchmark documentation](docs/performance/benchmarks.mdx) for limits.

## Memory and lifecycle

The first page contains the header and padding. Data begins at the next page boundary. Capacity must be positive and a multiple of the system page size. Names contain ASCII letters, digits, underscores, and hyphens, with a maximum of 25 bytes on macOS and 250 on Linux.

Closing the creator removes the name. Existing openers keep their mappings until they close. The header count tracks handles but does not govern unlinking or repair crashes. Node buffers and Python views retain mappings while they are alive; other borrowed views require the caller to keep the region open.

Notifications publish writes with release/acquire ordering. They coalesce and do not protect data against later concurrent writes. Use atomic data or a protocol that prevents simultaneous reads and writes. Raw C handles must be closed exactly once, after every operation using them has finished.

Zinc memory is volatile. It provides no disk durability or automatic crash recovery.

## Changes and commits

Keep fixes scoped, add regression tests for behavioral changes, and update the affected API docs and roadmap. Existing commit messages use short descriptions such as `Fix shared region validation and mapping cleanup`.
