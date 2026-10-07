#pragma once
#include "zinc.h"
#include <span>
#include <string>
#include <string_view>
#include <system_error>
#include <cstdint>
#include <stdexcept>

namespace zinc {

class SharedRegion {
public:
    static SharedRegion create(std::string_view name, std::size_t capacity) {
        if (name.find('\0') != std::string_view::npos)
            throw std::invalid_argument("name contains NUL");
        ZincHandle h{};
        if (int e = zinc_create(std::string(name).c_str(), capacity, &h); e != 0)
            throw std::system_error(-e, std::generic_category(), "zinc_create");
        return SharedRegion{h};
    }

    static SharedRegion open(std::string_view name) {
        if (name.find('\0') != std::string_view::npos)
            throw std::invalid_argument("name contains NUL");
        ZincHandle h{};
        if (int e = zinc_open(std::string(name).c_str(), &h); e != 0)
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
        return {reinterpret_cast<std::byte*>(zinc_ptr(h_)), zinc_capacity(h_)};
    }

    [[nodiscard]] std::span<const std::byte> bytes() const {
        return {reinterpret_cast<const std::byte*>(zinc_ptr(h_)), zinc_capacity(h_)};
    }

    [[nodiscard]] std::size_t capacity() const { return zinc_capacity(h_); }

    void notify() { zinc_notify(h_); }

    bool wait(uint32_t timeout_ms = 1000) {
        int code = zinc_wait(h_, timeout_ms);
        if (code == -110) return false;
        if (code != 0) throw std::system_error(-code, std::generic_category(), "zinc_wait");
        return true;
    }

    bool try_wait() {
        int code = zinc_try_wait(h_);
        if (code == -11) return false;
        if (code != 0) throw std::system_error(-code, std::generic_category(), "zinc_try_wait");
        return true;
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
