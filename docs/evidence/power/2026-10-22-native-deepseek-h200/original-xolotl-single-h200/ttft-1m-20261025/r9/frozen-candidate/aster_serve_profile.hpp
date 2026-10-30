#pragma once
#include <cstring>

namespace aster_profile {
inline bool flag(const char * value, bool & enabled) {
    if (value == nullptr || std::strcmp(value, "0") == 0) { enabled = false; return true; }
    if (std::strcmp(value, "1") == 0) { enabled = true; return true; }
    return false;
}
class FourthGen {
    bool enabled_;
    unsigned seen_ = 0;
public:
    explicit FourthGen(bool enabled) : enabled_(enabled) {}
    bool next() {
        if (!enabled_ || seen_ >= 4) return false;
        return ++seen_ == 4;
    }
};
class DecodeWindow {
    void (*callback_)(bool);
public:
    DecodeWindow(bool enabled, void (*callback)(bool)) : callback_(enabled ? callback : nullptr) {
        if (callback_) callback_(true);
    }
    DecodeWindow(const DecodeWindow &) = delete;
    DecodeWindow & operator=(const DecodeWindow &) = delete;
    DecodeWindow(DecodeWindow &&) = delete;
    DecodeWindow & operator=(DecodeWindow &&) = delete;
    void finish() {
        const auto callback = callback_;
        callback_ = nullptr;
        if (callback) callback(false);
    }
    ~DecodeWindow() { finish(); }
};
}
