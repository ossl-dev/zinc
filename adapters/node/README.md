# Zinc — Node.js Adapter

Zero-copy shared memory for Node.js via [napi-rs](https://napi.rs).

## Install

```bash
npm install @aspect-build/zinc
```

Requires the Zinc core library (`libzinc_core.dylib` / `libzinc_core.so`) on your `LD_LIBRARY_PATH` or adjacent to the native addon.

### Building from source

```bash
cargo build --release --manifest-path core/Cargo.toml
cd adapters/node
npm install
npx napi build --release
```

## Usage

```typescript
import { ZincRegion } from "@aspect-build/zinc";

// Process A — create
const region = ZincRegion.create("/my-data", 4096);
const buf = region.asBuffer();
const view = new Float32Array(buf.buffer, buf.byteOffset, buf.byteLength / 4);
view[0] = 42.0;
view[1] = 3.14;
region.notify();

// Process B — open
const region2 = ZincRegion.open("/my-data");
const buf2 = region2.asBuffer();
const view2 = new Float32Array(buf2.buffer, buf2.byteOffset, buf2.byteLength / 4);
console.log(view2[0]); // 42.0
region2.wait(5000);
```

## API

### `ZincRegion.create(name, capacity)`
Create a new shared region. Fails if one already exists with this name.
- `name: string` — shm name (alphanumeric, `_`, `-` only)
- `capacity: number` — size in bytes (must be page-aligned)

### `ZincRegion.open(name)`
Open an existing shared region.

### `region.asBuffer(env) → JsBuffer`
Zero-copy `Buffer` backed by the mmap'd region. No data is copied.

### `region.notify()`
Signal all waiters that data has been written.

### `region.wait(timeoutMs) → boolean`
Block until notified or timeout elapses.

## Publish

```bash
cd adapters/node
npx napi artifacts
npm publish
```

## Platform support

| OS | Status |
|---|---|
| Linux | ✅ |
| macOS | ✅ |

> Windows is not supported. Zinc requires POSIX `shm_open` + `mmap`.
