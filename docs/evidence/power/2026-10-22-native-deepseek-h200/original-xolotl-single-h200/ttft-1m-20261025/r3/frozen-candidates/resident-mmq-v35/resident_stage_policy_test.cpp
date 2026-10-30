#include "aster_resident_stage_policy.hpp"
#include <cstdio>
#include <cstdlib>
#include <cstdint>
#include <initializer_list>
int main() {
    int checked = 0;
    auto require = [&](bool ok) { if (!ok) std::exit(1); ++checked; };
    for (int reserved : {-1, 0, 1}) {
        const auto p = aster_resident_stage::plan(13, 16, 47, reserved);
        require(p.valid && p.slots == 2);
        for (size_t wave = 0; wave < 7; ++wave) require(p.half(wave) >= 0 && p.half(wave) <= 1 && p.half(wave) != reserved);
    }
    const auto p = aster_resident_stage::plan(16, 16, 16, -1);
    require(p.valid && p.slots == 1 && p.half(0) == 0 && p.half(1) == 1);
    for (int64_t b : {int64_t(-1), int64_t(0)}) require(!aster_resident_stage::plan(b, 16, 32, -1).valid);
    require(!aster_resident_stage::plan(17, 16, 32, -1).valid);
    require(!aster_resident_stage::plan(16, 16, 15, -1).valid);
    require(!aster_resident_stage::plan(16, 16, 32, 2).valid);
    require(!aster_resident_stage::plan(16, 16, 32, -2).valid);
    const auto huge = aster_resident_stage::plan(INT64_MAX, INT64_MAX, INT64_MAX, 0);
    require(huge.valid && huge.slots == 1 && huge.half(3) == 1);
    std::printf("PASS resident stage policy: %d ownership and bounds checks\n", checked);
}
