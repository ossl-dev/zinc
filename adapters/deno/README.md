# Zinc Deno adapter

Deno.dlopen loads the core from target/release relative to this checkout. Use Deno 2. Linux and macOS are supported.

Registry publishing is pending. From the repository root:

```bash
cargo build --release -p zinc-core --lib
deno test --unstable-ffi --allow-ffi --allow-read --config adapters/deno/deno.json adapters/deno/tests/
```

Import from your source checkout:

```typescript
import { SharedRegion } from "./adapters/deno/src/mod.ts";
```

`buffer()` returns a zero-copy Uint8Array. Keep the region open while using it. `close()` is idempotent and `Symbol.dispose` is supported. `wait()` blocks the calling thread, returns false on timeout, and throws on other errors. Use a worker for an asynchronous JavaScript producer. `tryWait()` checks without blocking.

Names contain ASCII letters, digits, underscores, and hyphens; do not add a leading slash. Capacity must be positive and a multiple of the system page size. Keep the creator alive until other processes open the region. Closing the creator unlinks the name while existing mappings remain valid.

See the [API reference](../../docs/adapters/deno.mdx), [notification rules](../../docs/guides/notify-wait.mdx), and [source installation guide](../../docs/getting-started/installation.mdx).
