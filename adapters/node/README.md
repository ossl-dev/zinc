# Zinc Node.js adapter

napi-rs links the Rust core into a native addon. Linux and macOS are supported.

Registry publishing is pending. From the repository root:

```bash
cd adapters/node
npm ci
npm run build
npm test
```

Import from your source checkout:

```javascript
const { ZincRegion } = require("./adapters/node/index.js");
```

`asBuffer()` returns a zero-copy Buffer and retains the mapping until collection. There is no explicit close method. Use `notify()`, `wait(timeoutMs)`, and `tryWait()` for notifications. Invalid unsigned 32-bit capacities and timeouts are rejected.

Names contain ASCII letters, digits, underscores, and hyphens; do not add a leading slash. Capacity must be positive and a multiple of the system page size. Keep the creator alive until other processes open the region. Closing the creator unlinks the name while existing mappings remain valid.

See the [API reference](../../docs/adapters/node.mdx), [notification rules](../../docs/guides/notify-wait.mdx), and [source installation guide](../../docs/getting-started/installation.mdx).
