"""Smoke tests for the Zinc Python adapter.
Requires: libzinc_core built (cargo build --release -p zinc-core)
Run: pytest adapters/python/tests/ -v
"""
import pytest
import sys
import os

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))


class TestSharedRegion:
    def test_create_and_buffer(self):
        from zinc import SharedRegion
        import uuid
        name = f"pytest_{uuid.uuid4().hex[:8]}"
        cap = os.sysconf("SC_PAGESIZE")

        r = SharedRegion.create(name, cap)
        buf = r.as_buffer()
        assert len(buf) == cap
        buf[0] = 0xAB
        assert buf[0] == 0xAB
        r.close()

    def test_open_and_read(self):
        from zinc import SharedRegion
        import uuid
        name = f"pytest_{uuid.uuid4().hex[:8]}"
        cap = os.sysconf("SC_PAGESIZE")

        owner = SharedRegion.create(name, cap)
        buf = owner.as_buffer()
        buf[0:4] = b"ZINC"

        reader = SharedRegion.open(name)
        rbuf = reader.as_buffer()
        assert bytes(rbuf[0:4]) == b"ZINC"

        reader.close()
        owner.close()

    def test_notify_wait(self):
        from zinc import SharedRegion
        import uuid, threading, time
        name = f"pytest_{uuid.uuid4().hex[:8]}"
        cap = os.sysconf("SC_PAGESIZE")

        region = SharedRegion.create(name, cap)
        result = {"signaled": False}

        def writer():
            time.sleep(0.05)
            r2 = SharedRegion.open(name)
            buf = r2.as_buffer()
            buf[0] = 42
            r2.notify()
            r2.close()

        t = threading.Thread(target=writer)
        t.start()

        assert region.wait(timeout_ms=5000)
        assert region.as_buffer()[0] == 42
        t.join()
        region.close()

    def test_create_duplicate_fails(self):
        from zinc import SharedRegion
        import uuid
        name = f"pytest_{uuid.uuid4().hex[:8]}"

        r = SharedRegion.create(name, os.sysconf("SC_PAGESIZE"))
        with pytest.raises(OSError):
            SharedRegion.create(name, os.sysconf("SC_PAGESIZE"))
        r.close()

    def test_open_nonexistent_fails(self):
        from zinc import SharedRegion
        with pytest.raises(OSError):
            SharedRegion.open("nonexistent_region_xyz")


    def test_buffer_survives_close_and_gc(self):
        import gc
        import uuid
        from zinc import SharedRegion
        name = f"py_gc_{uuid.uuid4().hex[:8]}"
        region = SharedRegion.create(name, os.sysconf("SC_PAGESIZE"))
        buf = region.as_buffer()
        region.close()
        region.close()
        del region
        gc.collect()
        buf[0] = 42
        with SharedRegion.open(name) as reader:
            assert reader.as_buffer()[0] == 42
        del buf
        gc.collect()
        with pytest.raises(OSError):
            SharedRegion.open(name)

    def test_closed_operations(self):
        import uuid
        from zinc import SharedRegion
        region = SharedRegion.create(f"py_close_{uuid.uuid4().hex[:8]}", os.sysconf("SC_PAGESIZE"))
        assert not region.try_wait()
        region.notify()
        assert region.try_wait()
        assert not region.wait(0)
        region.close()
        for operation in [region.as_buffer, region.notify, region.wait, region.try_wait]:
            with pytest.raises(ValueError, match="closed"):
                operation()

    def test_structured_numpy_view(self):
        import uuid
        import numpy as np
        from zinc import SharedRegion
        name = f"py_np_{uuid.uuid4().hex[:8]}"
        dtype = np.dtype([("x", "f4"), ("y", "f4")])
        with SharedRegion.create(name, os.sysconf("SC_PAGESIZE")) as owner:
            array = owner.as_numpy(dtype)
            array[0] = (1.5, 2.5)
            with SharedRegion.open(name) as reader:
                other = reader.as_numpy(dtype)
                assert other[0]["x"] == 1.5
                assert other[0]["y"] == 2.5
                del other
            del array

    def test_embedded_nul_rejected(self):
        from zinc import SharedRegion
        with pytest.raises(ValueError, match="NUL"):
            SharedRegion.create("name\0suffix", os.sysconf("SC_PAGESIZE"))
