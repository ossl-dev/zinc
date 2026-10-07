# Zinc Python adapter

CFFI loads the core from the checkout; NumPy integration is optional. Linux and macOS are supported.

Registry publishing is pending. From the repository root:

```bash
cargo build --release -p zinc-core --lib --example interop
python3 -m pip install -e 'adapters/python[test]'
python3 -m pytest adapters/python/tests -q
```

Import from your source checkout:

```python
from zinc import SharedRegion
```

`as_buffer()` and `as_numpy(dtype=...)` retain the mapping. `close()` prevents new operations while existing views remain valid. `wait()` returns false on timeout and raises on other errors. `try_wait()` consumes a pending notification. NumPy defaults to uint8 and also accepts structured dtypes.

Names contain ASCII letters, digits, underscores, and hyphens; do not add a leading slash. Capacity must be positive and a multiple of the system page size. Keep the creator alive until other processes open the region. Closing the creator unlinks the name while existing mappings remain valid.

See the [API reference](../../docs/adapters/python.mdx), [notification rules](../../docs/guides/notify-wait.mdx), and [source installation guide](../../docs/getting-started/installation.mdx).
