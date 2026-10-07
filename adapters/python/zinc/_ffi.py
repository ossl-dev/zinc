from cffi import FFI
import pathlib
import platform

ffi = FFI()

ffi.cdef("""
    typedef void* ZincHandle;
    int32_t zinc_create(const char *name, uintptr_t capacity, ZincHandle *out);
    int32_t zinc_open(const char *name, ZincHandle *out);
    uint8_t *zinc_ptr(ZincHandle h);
    uintptr_t zinc_capacity(ZincHandle h);
    void zinc_close(ZincHandle h);
    void zinc_notify(ZincHandle h);
    int32_t zinc_wait(ZincHandle h, uint32_t timeout_ms);
    int32_t zinc_try_wait(ZincHandle h);
    uint32_t zinc_version(void);
""")

_suffix = {"Linux": "so", "Darwin": "dylib"}[platform.system()]
_lib_path = pathlib.Path(__file__).parent.parent.parent.parent / "target" / "release" / f"libzinc_core.{_suffix}"
lib = ffi.dlopen(str(_lib_path))
