# Zinc — Deno Adapter

Zero-copy shared memory for Deno via `Deno.dlopen`.

## Install

### JSR

```bash
deno add @aspect-build/zinc
```

### Direct import

```typescript
import { SharedRegion } from "./mod.ts";
```

Requires `libzinc_core.dylib` (macOS) / `libzinc_core.so` (Linux) at the expected path, or set `DENO_LIB_ZINC` env var.

### Building from source

```bash
cargo build --release --manifest-path core/Cargo.toml
```

## Usage

```typescript
import { SharedRegion } from "@aspect-build/zinc";

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

### `region.buffer() → Uint8Array`
Zero-copy view of the shared memory.

### `region.notify()`
Signal all waiters.

### `region.wait(timeoutMs?) → boolean`
Block until notified.

### `region.close()`
Release the handle.

## Publish

```bash
cd adapters/deno
deno publish
```

## Platform support

| OS | Status |
|---|---|
| Linux | ✅ |
| macOS | ✅ |
| Windows | ⏳ |
