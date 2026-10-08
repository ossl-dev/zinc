import asyncio

from ._ffi import ffi, lib


class SharedRegion:
    """A shared mapping retained by each exported buffer."""

    def __init__(self, handle):
        self._h = ffi.gc(handle, lib.zinc_close)

    @classmethod
    def create(cls, name: str, capacity: int) -> "SharedRegion":
        out = ffi.new("ZincHandle *")
        _check(lib.zinc_create(_name(name), capacity, out))
        return cls(out[0])

    @classmethod
    def open(cls, name: str) -> "SharedRegion":
        out = ffi.new("ZincHandle *")
        _check(lib.zinc_open(_name(name), out))
        return cls(out[0])

    def _handle(self):
        if self._h is None:
            raise ValueError("region is closed")
        return self._h

    def as_buffer(self) -> memoryview:
        handle = self._handle()
        ptr = ffi.gc(lib.zinc_ptr(handle), lambda _, owner=handle: None)
        return memoryview(ffi.buffer(ptr, lib.zinc_capacity(handle)))

    def as_numpy(self, dtype=None):
        """A NumPy view sharing the mapping's bytes and lifetime."""
        import numpy as np
        return np.frombuffer(self.as_buffer(), dtype=np.uint8 if dtype is None else dtype)

    def notify(self) -> None:
        lib.zinc_notify(self._handle())

    def wait(self, timeout_ms: int = 1000) -> bool:
        return _wait(self._handle(), timeout_ms)

    async def wait_async(self, timeout_ms: int = 1000) -> bool:
        """Wait in a worker thread. Cancellation does not stop the native wait."""
        handle = self._handle()
        return await asyncio.to_thread(_wait, handle, timeout_ms)

    def try_wait(self) -> bool:
        code = lib.zinc_try_wait(self._handle())
        if code == -11:
            return False
        _check(code)
        return True

    def close(self) -> None:
        self._h = None

    def __enter__(self):
        self._handle()
        return self

    def __exit__(self, *args):
        self.close()


def _name(name: str) -> bytes:
    if "\0" in name:
        raise ValueError("name contains NUL")
    return name.encode()


def _check(code: int) -> None:
    if code != 0:
        raise OSError(-code, f"zinc error {code}")


def _wait(handle, timeout_ms: int) -> bool:
    code = lib.zinc_wait(handle, timeout_ms)
    if code == -110:
        return False
    _check(code)
    return True
