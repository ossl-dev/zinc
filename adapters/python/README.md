# Zinc Python adapter

CFFI loads the core from the checkout; NumPy integration is optional. Linux and macOS are supported.

Registry publishing is pending. From the repository root:

```bash
cargo build --release -p zinc-core --lib --example interop
python3 -m pip install -e 'adapters/python[test]'
python3 -m pytest adapters/python/tests -q
python3 -m mypy --strict adapters/python/tests/typing_check.py
```

Import from your source checkout:

```python
from zinc import SharedRegion
```

`as_buffer()` and `as_numpy(dtype=...)` retain the mapping. `close()` prevents new operations while existing views remain valid. `wait()` returns false on timeout and raises on other errors. `try_wait()` consumes a pending notification. NumPy defaults to uint8 and also accepts structured dtypes.

`await region.wait_async(timeout_ms=1000)` runs the native wait in a worker thread and keeps the event loop free. It returns false on timeout and retains the mapping until the worker finishes, even after `close()` or cancellation. Cancelling the coroutine does not stop the native wait; it can still consume a notification before its timeout. Avoid concurrent waits on the same handle.

The package includes type stubs and a `py.typed` marker. NumPy annotations are available with the `numpy` extra.

Names contain ASCII letters, digits, underscores, and hyphens; do not add a leading slash. Capacity must be positive and a multiple of the system page size. Keep the creator alive until other processes open the region. Closing the creator unlinks the name while existing mappings remain valid.

See the [API reference](../../docs/adapters/python.mdx), [notification rules](../../docs/guides/notify-wait.mdx), and [source installation guide](../../docs/getting-started/installation.mdx).
