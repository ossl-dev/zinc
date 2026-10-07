# Zinc — Python Adapter

Zero-copy shared memory for Python via [cffi](https://cffi.readthedocs.io/). Includes optional [NumPy](https://numpy.org/) integration for ML/data workflows.

## Install

```bash
pip install zinc-shm
```

Requires `libzinc_core.dylib` / `libzinc_core.so` built and findable.

### Building from source

```bash
cargo build --release --manifest-path core/Cargo.toml
pip install -e adapters/python
```

## Usage

```python
from zinc import SharedRegion

# Process A — create
r = SharedRegion.create("/my-data", 4096)
buf = r.as_buffer()
buf[0:8] = (42).to_bytes(8, 'little')
r.notify()

# Process B — open
r2 = SharedRegion.open("/my-data")
buf2 = r2.as_buffer()
value = int.from_bytes(buf2[0:8], 'little')
print(value)  # 42
```

### NumPy integration

```python
import numpy as np
from zinc import SharedRegion

r = SharedRegion.create("/tensor", 4096)
arr = r.as_numpy(dtype=np.float32)  # zero-copy!
arr[0] = 3.14159
arr[1] = 2.71828
r.notify()
```

## API

### `SharedRegion.create(name, capacity)`
Create a new shared region.
- `name: str` — shm name
- `capacity: int` — size in bytes (page-aligned)

### `SharedRegion.open(name)`
Open an existing shared region.

### `region.as_buffer() → memoryview`
Zero-copy `memoryview` of the shared region.

### `region.as_numpy(dtype=np.float32) → numpy.ndarray`
Zero-copy NumPy array view.

### `region.notify()`
Signal all waiters.

### `region.wait(timeout_ms=1000) → bool`
Block until notified.

### `region.close()`
Release the handle. Use as context manager with `with`:

```python
with SharedRegion.create("/data", 4096) as r:
    arr = r.as_numpy(dtype=np.float64)
    arr[0] = 1.0
```

## Testing

```bash
pytest adapters/python/tests/ -v
```

## Publish

```bash
cd adapters/python
pip install build twine
python -m build
twine upload dist/*
```

## Platform support

| OS | Status |
|---|---|
| Linux | ✅ |
| macOS | ✅ |

> Windows is not supported. Zinc requires POSIX `shm_open` + `mmap`.

`close()` prevents new operations on the handle. Exported memoryviews and NumPy arrays retain the mapping until they are released, so they remain valid after close. If a creator still has exported views, its name also remains until those views are released.

`try_wait()` consumes a pending notification without blocking. `wait()` returns false on timeout and raises `OSError` for other failures. Structured NumPy dtypes work through `as_numpy(dtype=...)`.
