import pathlib
import subprocess
import uuid

from zinc import SharedRegion


def test_rust_create_python_read():
    root = pathlib.Path(__file__).resolve().parents[3]
    executable = root / "target" / "release" / "examples" / "interop"
    name = f"interop_{uuid.uuid4().hex[:8]}"
    process = subprocess.Popen(
        [str(executable), name], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True
    )
    try:
        assert process.stdout.readline().strip() == "ready"
        with SharedRegion.open(name) as region:
            assert region.try_wait()
            assert bytes(region.as_buffer()[:4]) == b"ZINC"
    finally:
        process.communicate("\n", timeout=5)
    assert process.returncode == 0
