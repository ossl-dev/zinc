from ._ffi import ffi, lib
from typing import Optional

class SharedRegion:
    """Zero-copy shared memory region accessible from any language."""

    def __init__(self, _handle):
        self._h = _handle

    @classmethod
    def create(cls, name: str, capacity: int) -> "SharedRegion":
        out = ffi.new("ZincHandle *")
        _check(lib.zinc_create(name.encode(), capacity, out))
        return cls(out[0])

    @classmethod
    def open(cls, name: str) -> "SharedRegion":
        out = ffi.new("ZincHandle *")
        _check(lib.zinc_open(name.encode(), out))
        return cls(out[0])

    def as_buffer(self) -> memoryview:
        ptr = lib.zinc_ptr(self._h)
        size = lib.zinc_capacity(self._h)
        return memoryview(ffi.buffer(ptr, size))

    def as_numpy(self, dtype=None):
        """Zero-copy numpy view of the shared region."""
        import numpy as np
        buf = self.as_buffer()
        if dtype is not None:
            return np.frombuffer(buf, dtype=dtype)
        return np.frombuffer(buf, dtype=np.uint8)

    def notify(self) -> None:
        lib.zinc_notify(self._h)

    def wait(self, timeout_ms: int = 1000) -> bool:
        return lib.zinc_wait(self._h, timeout_ms) == 0

    def close(self) -> None:
        if self._h:
            lib.zinc_close(self._h)
            self._h = None

    def __del__(self):
        self.close()

    def __enter__(self):
        return self

    def __exit__(self, *args):
        self.close()


def _check(code: int) -> None:
    if code != 0:
        raise OSError(-code, f"zinc error {code}")
