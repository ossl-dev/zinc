# Zinc Roadmap

Things to build, fix, and improve. Checked boxes mean shipped.

Anyone looking to contribute: pick an unchecked box, open an issue saying you're working on it, send a PR. Keep it scoped to one item per PR.

---

## Phase 1, Ship what we have

Stuff that's built but not released or not finished.

### Registry publishing

- [ ] Publish `zinc-core` to crates.io
- [ ] Publish `zinc-shm` to PyPI
- [ ] Publish `@ossl/zinc` to npm (Node)
- [ ] Publish `@ossl/zinc-bun` to npm (Bun)
- [ ] Publish `@ossl/zinc` to JSR (Deno)
- [ ] Publish Go adapter (`go get`)
- [ ] Publish Java adapter to Maven Central
- [ ] Publish C# adapter to NuGet

### Adapters

- [x] Java adapter: finish FFI bindings, write integration tests
- [x] C# adapter: finish FFI bindings, write integration tests
- [x] C++ adapter: add CMakeLists.txt for easy build, write usage examples
- [x] Deno adapter: add `deno test` suite
- [x] Bun adapter: add `bun test` suite
- [x] Go adapter: test on Linux and macOS in CI

### CI / infra

- [x] Wire up `cargo bench` (criterion), benchmarks for throughput and latency
- [x] Add per-adapter CI jobs (Python test, Go vet + test, Node build)
- [x] Add CI badge to README
- [x] Add `valgrind` leak check to CI (Linux)
- [x] Add cargo-deny for license + advisory checks

### Docs

- [x] Add API reference page per adapter (all 9 languages have docs)
- [x] Add troubleshooting page (common errors, `/dev/shm` permissions, macOS quirks)
- [x] Add "migrating from v1" guide (if anyone was on the Zig version)

---

## Phase 2, Core improvements

Stuff that makes the existing thing better, faster, safer.

### Synchronization

- [ ] Replace macOS spin-wait with `__ulock_wait` / `__ulock_wake` (private API, needs detection + graceful fallback)
- [x] Add `try_wait()`, non-blocking check on notify_seq
- [ ] Support waiting on multiple regions (`wait_any` / `wait_all`)
- [ ] Linux: switch to `FUTEX_WAIT_BITSET` for multi-region wake targeting

### Memory model

- [ ] Add `SharedRegion::resize()`, grow/shrink a region (needs ftruncate + remap)
- [ ] Add `MAP_HUGETLB` support (huge pages for large regions, gated behind feature flag)
- [ ] Add `MAP_POPULATE` option to pre-fault pages on create (avoid page-fault latency in hot path)
- [ ] Expose `mlock` / `munlock` to prevent paging for latency-sensitive workloads
- [ ] Support custom alignment beyond page size (e.g. 2MB for GPU DMA)

### The ring buffer

- [x] Make ring capacity configurable (`RingStorage::new` and `Ring::from_raw_with_capacity`)
- [x] Support multiple producers and consumers in the ring
- [x] Add `Ring::try_push`, returning a boolean when a slot cannot be reserved
- [x] Add ring stats: `len()`, `is_empty()`, `remaining()`
- [x] Benchmarks for ring push/pop under contention

### Error handling

- [x] Distinguish `NotFound` vs `PermissionDenied` at platform level
- [x] Add `ZincError::WouldBlock` as distinct from `RingFull` (for non-blocking ops)
- [x] Platform error messages: include the syscall that failed and errno string

### Platform support

- [x] Test and document aarch64 Linux (core CI job and local container validation)
- [ ] FreeBSD support (also has `shm_open` + `mmap`, needs `libc` crate cfg)
- [ ] Illumos/SmartOS support investigation
- [x] Document `vm.max_map_count` tuning for Linux when using many regions

---

## Phase 3, New features

New capabilities that expand what Zinc can do.

### Higher-level data structures

- [ ] **SharedRing**, a proper byte-stream ring buffer on top of a SharedRegion. Multiple producers push bytes, consumers drain them. Replacement for the old v1 ring.
- [ ] **SharedQueue**, typed fixed-capacity queue. Push struct T, pop struct T. Uses the lock-free ring underneath.
- [ ] **SharedMap**, key-value store in shared memory. Think a `HashMap<K, V>` that lives in the region. Needs a concurrent hash table design.
- [ ] **SharedArena**, bump allocator in shared memory. Allocate fixed-size slots, free in bulk.

### Streaming

- [x] Add `SharedRegion::write_at(offset, data)` and `read_at(offset, buf)` with bounds checks
- [x] Add `SharedRegion::write_bytes(offset, val, count)`, like `memset` on the region
- [ ] Add `SharedRegion::compare_and_swap(offset, old, new)` for lock-free data structures in shared memory

### Security

- [ ] **PID allowlisting**, restrict which PIDs can open a region (stored in header, checked on open)
- [ ] **CRC32 integrity**, optional checksum over data area, verified on open
- [ ] **Access modes**, read-only open (MAP_PRIVATE or PROT_READ), read-write open
- [ ] **Region ownership token**, shared secret for openers to prove they're allowed

### IPC patterns

- [ ] **PubSub**, one writer, many readers. Writer notifies, all readers wake.
- [ ] **Request-Reply**, two regions paired: one for request, one for reply. Higher-level wrapper over notify/wait.
- [ ] **Broadcast**, writer writes once, notifies all readers. Compare to Unix domain socket broadcast.

### Tooling

- [ ] `zinc-cli`, a small CLI tool: `zinc list` (show active regions), `zinc inspect <name>` (dump header), `zinc rm <name>` (force-unlink stale regions)
- [ ] `zinc-top`, TUI that shows active regions, ref counts, sizes, PIDs (like `htop` for shared memory)
- [ ] `/dev/shm` monitoring hook, optional background thread that scans for stale zinc regions and cleans them up

---

## Phase 4, Language adapter improvements

Each adapter should feel native, not like an FFI wrapper.

### Python

- [ ] Async notify/wait: `await region.wait_async()` using `asyncio` + thread pool
- [x] NumPy structured dtype support: `region.as_numpy(dtype=np.dtype([('x', 'f4'), ('y', 'f4')]))`
- [ ] Type stubs (`.pyi` files)

### Node

- [ ] Worker thread sync primitives using `Atomics` on the shared buffer
- [ ] TypeScript generics: `ZincRegion<T>` with typed buffer views

### Go

- [x] `SharedRegion.Read([]byte)` and `Write([]byte)` with offset tracking, plus `ReadAt`, `WriteAt`, and `Seek`
- [ ] Channel-like API: `region.Send(val)` / `region.Recv()`

### C++

- [x] Move semantics for `SharedRegion` (move constructor, move assignment)
- [ ] `std::span<T>` and `std::mdspan<T>` zero-copy views
- [ ] Integration with `libunifex` / `std::execution` senders for async wait

### Java

- [x] `java.nio.ByteBuffer` zero-copy view via JNA
- [x] `AutoCloseable` implementation for try-with-resources

### C\#

- [ ] `Span<T>` and `Memory<T>` zero-copy views
- [x] `IDisposable` pattern for RAII cleanup

---

## Phase 5, Ecosystem & observability

### Benchmarks

- [ ] Real-world benchmarks: ML inference pipeline (Python GPU → Go server → JS frontend), game state sync (C++ physics → rendering process)
- [ ] Latency histograms with percentiles (not just p50/p99)
- [ ] Throughput under contention (N writers, M readers)
- [ ] Comparison to `memfd_create` + `pidfd_getfd` on Linux 6.x

### Observability

- [ ] OpenTelemetry tracing: span per create/open/close/notify/wait
- [ ] Metrics: active region count, total bytes mapped, notify/wait latency histogram
- [ ] Health check endpoint for long-running processes that use Zinc

### Bindings & interop

- [ ] Ruby adapter (FFI via `ruby-ffi`)
- [ ] Swift adapter (via `zinc.h` + module map)
- [ ] Zig adapter (direct `@cImport` of `zinc.h`)
- [ ] Lua adapter (LuaJIT FFI)
- [ ] PHP adapter (FFI, PHP 7.4+)

---

## Bugs & known issues

Not yet triaged into phases. Fix anytime.

- [x] macOS: reduce idle wait CPU with bounded spinning and sleep backoff; kernel wait support remains in Phase 2
- [ ] Borrowed views: enforce mapping lifetime for Bun, Deno, Go, C++, Java, and C# views (currently a caller contract)
- [ ] Crash recovery: zombie regions if creator dies without closing (need staleness detection)
- [ ] `AlreadyExists` on stale regions: creator crashed, old name still in `/dev/shm`, new create fails. Need `create_or_replace` mode.
- [x] Enforce and document platform name limits, including the `/zinc_` prefix
- [x] Verify `zinc_version()` packing: a u16 header version occupies the upper 16 bits of u32 without overflow
- [x] Node adapter: retain the region while `asBuffer` views are alive
- [x] Publish the immutable header with an atomic release/acquire magic value before reading fields
- [x] Document that `name_hash` is diagnostic; name lookup uses the full POSIX name
- [x] Document volatile shared memory and the absence of disk durability guarantees

---

## Ideas / maybe someday

Not committed. Brainstorming parking lot.

- **Zero-copy serialization**: integrate with `rkyv` or `flatbuffers` so structs can be written directly into shared memory with no encode step
- **Persistent regions**: backed by a file on disk instead of `shm_open` (use `mmap` on a real file, survives reboots)
- **Network-backed regions**: RDMA or `memcached`-like protocol to share a region across machines. Very different beast.
- **GPU shared memory**: map the same region into CUDA or Metal address space. `cudaHostRegister` on the mmap'd pages.
- **WASM support**: compile core to WASI, use `wasi:shm` proposal if/when it lands
- **SharedRegion as a file descriptor**: expose the underlying fd so tools like `sendmsg` with `SCM_RIGHTS` can pass regions to other processes
- **Compression**: transparent LZ4/zstd on write, useful for large sparse regions
- **Encryption at rest**: if persistent regions happen, encrypt pages before they hit disk
