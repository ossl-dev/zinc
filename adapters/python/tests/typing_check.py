import os
from typing import TYPE_CHECKING, Any

import numpy as np
from numpy.typing import NDArray

from zinc import SharedRegion


class Region(SharedRegion):
    pass


async def check_types() -> None:
    with Region.create("typing", os.sysconf("SC_PAGESIZE")) as region:
        child: Region = region
        buf: memoryview = child.as_buffer()
        raw: NDArray[np.uint8] = child.as_numpy()
        structured: NDArray[Any] = child.as_numpy(np.dtype([("x", "f4")]))
        ready: bool = await child.wait_async(100)
        if ready and child.try_wait():
            buf[0] = int(raw[0]) + int(structured[0]["x"])
        child.notify()
        child.wait(0)
    opened: Region = Region.open("typing")
    opened.close()


if TYPE_CHECKING:
    SharedRegion.create("bad", "size")  # type: ignore[arg-type]
    SharedRegion.open(42)  # type: ignore[arg-type]
