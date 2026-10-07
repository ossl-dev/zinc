# Zinc — Bun Adapter

Zero-copy shared memory for Bun via `bun:ffi`.

## Install

```bash
npm install @ossl/zinc-bun
# or
bun add @ossl/zinc-bun
```

Requires `libzinc_core.dylib` (macOS) or `libzinc_core.so` (Linux) built and accessible.

### Building from source

```bash
cargo build --release --manifest-path core/Cargo.toml
# Library at: target/release/libzinc_core.{dylib,so}
```

## Usage

```typescript
import { SharedRegion } from "@ossl/zinc-bun";

// Process A — create
const region = SharedRegion.create("/my-data", 4096);
const buf = region.buffer();
const view = new Float32Array(buf.buffer, buf.byteOffset, buf.byteLength / 4);
view[0] = 42.0;
region.notify();

// Process B — open
const region2 = SharedRegion.open("/my-data");
const buf2 = region2.buffer();
const view2 = new Float32Array(buf2.buffer, buf2.byteOffset, buf2.byteLength / 4);
console.log(view2[0]); // 42.0
region2.wait(5000);
```

## API

### `SharedRegion.create(name, capacity)`
Create a new shared region.

### `SharedRegion.open(name)`
Open an existing shared region.

### `region.buffer() → Buffer`
Zero-copy `Buffer` backed by the mmap'd region.

### `region.notify()`
Signal all waiters.

### `region.wait(timeoutMs?) → boolean`
Block until notified (default 1000ms).

### `region.close()`
Release the handle. Also available via `Symbol.dispose`.

## Publish

```bash
cd adapters/bun
bun publish
```

## Platform support

| OS | Status |
|---|---|
| Linux | ✅ |
| macOS | ✅ |

> Windows is not supported. Zinc requires POSIX `shm_open` + `mmap`.

`tryWait()` consumes a pending notification without blocking. Closing is idempotent; other operations on a closed handle throw. Borrowed buffers must be released before closing the region, and waits block the calling thread. Use a worker when the writer runs JavaScript asynchronously.
