# Zinc C++ adapter

A C++20 header wraps the C ABI with move semantics and std::span. Linux and macOS are supported.

Registry publishing is pending. From the repository root:

```bash
cargo build --release -p zinc-core --lib
cmake -S adapters/cpp -B target/cpp-tests
cmake --build target/cpp-tests
ctest --test-dir target/cpp-tests --output-on-failure
```

Import from your source checkout:

```cpp
#include "zinc.hpp"
```

Vendor both `adapters/cpp/include/zinc.hpp` and `include/zinc.h`, and link libzinc_core. `bytes()` borrows the mapping and must not outlive its region. Handles close on destruction. `wait()` returns false on timeout and throws on other errors; `try_wait()` checks without blocking. Names passed as string_view need not be NUL-terminated.

Names contain ASCII letters, digits, underscores, and hyphens; do not add a leading slash. Capacity must be positive and a multiple of the system page size. Keep the creator alive until other processes open the region. Closing the creator unlinks the name while existing mappings remain valid.

See the [API reference](../../docs/adapters/cpp.mdx), [notification rules](../../docs/guides/notify-wait.mdx), and [source installation guide](../../docs/getting-started/installation.mdx).
