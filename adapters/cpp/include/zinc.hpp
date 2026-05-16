#pragma once
#include "../../include/zinc.h"
#include <span>
#include <string>
#include <string_view>
#include <system_error>
#include <cstdint>

namespace zinc {

class SharedRegion {
public:
    static SharedRegion create(std::string_view name, std::size_t capacity) {
        ZincHandle h{};
        if (int e = zinc_create(name.data(), capacity, &h); e != 0)
            throw std::system_error(-e, std::generic_category(), "zinc_create");
        return SharedRegion{h};
    }

    static SharedRegion open(std::string_view name) {
        ZincHandle h{};
        if (int e = zinc_open(name.data(), &h); e != 0)
            throw std::system_error(-e, std::generic_category(), "zinc_open");
        return SharedRegion{h};
    }

    SharedRegion(SharedRegion&& other) noexcept : h_{other.h_} {
        other.h_ = nullptr;
    }

    SharedRegion& operator=(SharedRegion&& other) noexcept {
        if (this != &other) {
            close();
            h_ = other.h_;
            other.h_ = nullptr;
        }
        return *this;
    }

    SharedRegion(const SharedRegion&) = delete;
    SharedRegion& operator=(const SharedRegion&) = delete;

    ~SharedRegion() { close(); }

    [[nodiscard]] std::span<std::byte> bytes() {
        return {static_cast<std::byte*>(zinc_ptr(h_)), zinc_capacity(h_)};
    }

    [[nodiscard]] std::span<const std::byte> bytes() const {
        return {static_cast<const std::byte*>(zinc_ptr(h_)), zinc_capacity(h_)};
    }

    [[nodiscard]] std::size_t capacity() const { return zinc_capacity(h_); }

    void notify() { zinc_notify(h_); }

    bool wait(uint32_t timeout_ms = 1000) {
        return zinc_wait(h_, timeout_ms) == 0;
    }

    void close() {
        if (h_) {
            zinc_close(h_);
            h_ = nullptr;
        }
    }

private:
    explicit SharedRegion(ZincHandle h) : h_{h} {}
    ZincHandle h_{};
};

} // namespace zinc
