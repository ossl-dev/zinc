# Zinc Bun adapter

bun:ffi loads the core from target/release relative to this checkout. Linux and macOS are supported.

Registry publishing is pending. From the repository root:

```bash
cargo build --release -p zinc-core --lib
bun test adapters/bun/tests
```

Import from your source checkout:

```typescript
import { SharedRegion } from "./adapters/bun/src/index.ts";
```

`buffer()` returns a zero-copy Buffer. Keep the region open while using it. `close()` is idempotent and `Symbol.dispose` is supported. `wait()` blocks the calling thread, returns false on timeout, and throws on other errors. Use a worker for an asynchronous JavaScript producer. `tryWait()` checks without blocking.

Names contain ASCII letters, digits, underscores, and hyphens; do not add a leading slash. Capacity must be positive and a multiple of the system page size. Keep the creator alive until other processes open the region. Closing the creator unlinks the name while existing mappings remain valid.

See the [API reference](../../docs/adapters/bun.mdx), [notification rules](../../docs/guides/notify-wait.mdx), and [source installation guide](../../docs/getting-started/installation.mdx).
