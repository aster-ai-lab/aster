#pragma once
#include <cmath>
#include <cstring>
namespace aster_draft {
enum class Mode { off, c010, c025, c050, invalid };
inline Mode parse(const char * raw) {
    if (!raw || std::strcmp(raw, "0") == 0) return Mode::off;
    if (std::strcmp(raw, "0.1") == 0) return Mode::c010;
    if (std::strcmp(raw, "0.25") == 0) return Mode::c025;
    if (std::strcmp(raw, "0.5") == 0) return Mode::c050;
    return Mode::invalid;
}
inline float threshold(Mode mode) {
    switch (mode) {
        case Mode::c010: return 0.1f;
        case Mode::c025: return 0.25f;
        case Mode::c050: return 0.5f;
        default: return 0.0f;
    }
}
// Only shorten an already-selected draft prefix. Target verification and
// acceptance, logits, weights and routing stay in the existing engine path.
inline int truncate(Mode mode, const float * logits, int count) {
    if (count < 0 || count > 5 || mode == Mode::invalid) return 0;
    if (mode == Mode::off || count == 0) return count;
    if (!logits) return 0;
    float product = 1.0f;
    int selected = 0;
    while (selected < count &&
           (product *= 1.0f / (1.0f + std::exp(-logits[selected]))) >= threshold(mode)) ++selected;
    return selected;
}
}
