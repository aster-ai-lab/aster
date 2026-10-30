// Pure bounds and half-ownership policy; no CUDA and no inferred resource sizes.
#pragma once
#include <cstddef>
#include <cstdint>
namespace aster_resident_stage {
struct Plan {
    bool valid = false;
    size_t slots = 0;
    int first_half = 0;
    bool alternate = false;
    int half(size_t wave) const { return alternate ? static_cast<int>((wave + first_half) & 1) : first_half; }
};
inline Plan plan(int64_t blob, int64_t stride, int64_t half_bytes, int reserved_half) {
    if (blob <= 0 || stride < blob || half_bytes < stride || reserved_half < -1 || reserved_half > 1) return {};
    return {true, static_cast<size_t>(half_bytes / stride), reserved_half < 0 ? 0 : 1 - reserved_half, reserved_half < 0};
}
} // namespace aster_resident_stage
