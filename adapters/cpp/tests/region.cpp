#include "zinc.hpp"
#include <cassert>
#include <unistd.h>

int main() {
    const std::size_t capacity = static_cast<std::size_t>(sysconf(_SC_PAGESIZE));
    const std::string name = "cpp_" + std::to_string(getpid());
    const std::string extended = name + "_suffix";
    auto owner = zinc::SharedRegion::create(std::string_view(extended.data(), name.size()), capacity);
    auto reader = zinc::SharedRegion::open(name);
    owner.bytes()[0] = std::byte{42};
    assert(reader.bytes()[0] == std::byte{42});
    assert(!reader.try_wait());
    owner.notify();
    assert(reader.try_wait());
    assert(!reader.wait(0));
    auto moved = std::move(owner);
    assert(owner.capacity() == 0);
    moved.close();
    moved.close();
    assert(reader.bytes()[0] == std::byte{42});
    auto replacement = zinc::SharedRegion::create(name, capacity);
    reader = std::move(replacement);
    assert(reader.bytes()[0] == std::byte{0});
    reader.close();
    bool rejected = false;
    try {
        zinc::SharedRegion::open(std::string_view("bad\0name", 8));
    } catch (const std::invalid_argument&) {
        rejected = true;
    }
    assert(rejected);
    ZincHandle out = reinterpret_cast<ZincHandle>(1);
    assert(zinc_open(nullptr, &out) == -22);
    assert(out == nullptr);
}
