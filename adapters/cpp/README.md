# Zinc — C++ Adapter

Header-only RAII wrapper over the C ABI for C++20. Zero cost abstraction — the `zinc::SharedRegion` class wraps the C handle with move semantics and `std::span` access.

## Install

No build step. Copy `include/zinc.hpp` into your project, or use the included path.

Requires `libzinc_core.dylib` / `libzinc_core.so` at link time.

### CMake

```cmake
add_library(zinc INTERFACE)
target_include_directories(zinc INTERFACE ${ZINC_ROOT}/adapters/cpp/include)
target_link_libraries(your_target PRIVATE zinc)
```

### Conan / vcpkg

Pending package registration. For now, vendor the header.

## Usage

```cpp
#include "zinc.hpp"
#include <iostream>

int main() {
    // Process A — create
    auto region = zinc::SharedRegion::create("/my-data", 4096);
    auto bytes = region.bytes();
    *reinterpret_cast<float*>(bytes.data()) = 42.0f;
    region.notify();

    // Process B — open
    auto region2 = zinc::SharedRegion::open("/my-data");
    region2.wait(5000);
    auto bytes2 = region2.bytes();
    float val = *reinterpret_cast<const float*>(bytes2.data());
    std::cout << val << "\n"; // 42.0
}
```

## API

### `zinc::SharedRegion::create(name, capacity) → SharedRegion`
Create a new shared region. Throws `std::system_error` on failure.

### `zinc::SharedRegion::open(name) → SharedRegion`
Open an existing shared region.

### `region.bytes() → std::span<std::byte>`
Zero-copy span of the shared memory.

### `region.capacity() → std::size_t`
Size of the region.

### `region.notify()`
Signal all waiters.

### `region.wait(timeoutMs=1000) → bool`
Block until notified.

### `region.close()`
Release the handle (also in destructor).

## Publish

The header is published as part of the Zinc core release on GitHub.

```bash
gh release upload v0.1.0 adapters/cpp/include/zinc.hpp
```

## Platform support

| OS | Status |
|---|---|
| Linux | ✅ |
| macOS | ✅ |

> Windows is not supported. Zinc requires POSIX `shm_open` + `mmap`.

`try_wait()` consumes a pending notification without blocking. `wait()` returns false on timeout and throws on other failures. Names passed as `std::string_view` need not be NUL-terminated. Spans borrow the mapping and must not outlive it.

```bash
cmake -S adapters/cpp -B /tmp/zinc-cpp
cmake --build /tmp/zinc-cpp
ctest --test-dir /tmp/zinc-cpp --output-on-failure
```
