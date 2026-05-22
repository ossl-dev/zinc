"""Smoke tests for the Zinc Python adapter.
Requires: libzinc_core built (cargo build --release -p zinc-core)
Run: pytest adapters/python/tests/ -v
"""
import pytest
import sys
import os

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))


@pytest.fixture(autouse=True)
def setup_path():
    """Ensure core lib is findable — adjust path for dev layout."""
    import pathlib
    root = pathlib.Path(__file__).parent.parent.parent.parent
    core_lib = root / "core" / "target" / "release"
    if core_lib.exists():
        # Prepend to let cffi find the dylib
        orig = os.environ.get("DYLD_LIBRARY_PATH", "")
        os.environ["DYLD_LIBRARY_PATH"] = f"{core_lib}:{orig}" if orig else str(core_lib)
        yield
        os.environ["DYLD_LIBRARY_PATH"] = orig
    else:
        pytest.skip("core library not built")


class TestSharedRegion:
    def test_create_and_buffer(self):
        from zinc import SharedRegion
        import uuid
        name = f"pytest_{uuid.uuid4().hex[:8]}"
        cap = 4096

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
        cap = 4096

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
        cap = 4096

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

        r = SharedRegion.create(name, 4096)
        with pytest.raises(OSError):
            SharedRegion.create(name, 4096)
        r.close()

    def test_open_nonexistent_fails(self):
        from zinc import SharedRegion
        with pytest.raises(OSError):
            SharedRegion.open("nonexistent_region_xyz")
