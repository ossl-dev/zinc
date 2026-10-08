import asyncio
import gc
import os
import threading
import uuid

import pytest

import zinc
from zinc import SharedRegion


def test_async_wait_keeps_event_loop_running():
    async def run():
        name = f"py_async_{uuid.uuid4().hex[:8]}"
        with SharedRegion.create(name, os.sysconf("SC_PAGESIZE")) as region:
            async def publish():
                await asyncio.sleep(0.01)
                region.as_buffer()[0] = 42
                region.notify()

            producer = asyncio.create_task(publish())
            assert await region.wait_async(1000)
            await producer
            assert region.as_buffer()[0] == 42
            assert not await region.wait_async(0)
            assert not await region.wait_async(5)

    asyncio.run(run())


def test_async_wait_errors():
    async def run():
        with SharedRegion.create(f"py_async_{uuid.uuid4().hex[:8]}", os.sysconf("SC_PAGESIZE")) as region:
            for timeout in (-1, 2**32):
                with pytest.raises(OverflowError):
                    await region.wait_async(timeout)
        with pytest.raises(ValueError, match="closed"):
            await region.wait_async()

    asyncio.run(run())


def test_async_wait_retains_handle_after_close_and_cancellation(monkeypatch):
    started = threading.Event()
    finished = threading.Event()
    native_wait = zinc._wait

    def tracked_wait(handle, timeout):
        started.set()
        try:
            return native_wait(handle, timeout)
        finally:
            finished.set()

    monkeypatch.setattr(zinc, "_wait", tracked_wait)
    name = f"py_async_{uuid.uuid4().hex[:8]}"

    async def run():
        region = SharedRegion.create(name, os.sysconf("SC_PAGESIZE"))
        pending = asyncio.create_task(region.wait_async(1000))
        assert await asyncio.to_thread(started.wait, 1)
        region.close()
        gc.collect()
        with SharedRegion.open(name) as reader:
            pending.cancel()
            with pytest.raises(asyncio.CancelledError):
                await pending
            reader.notify()
            assert await asyncio.to_thread(finished.wait, 1)

    asyncio.run(run())
    gc.collect()
    with pytest.raises(OSError):
        SharedRegion.open(name)
