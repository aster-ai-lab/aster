#include "aster_prefill_policy.hpp"
#include <cassert>
#include <cmath>
#include <cstdio>
#include <vector>
int main() {
    int cases = 0;
    // Every tail row must appear exactly once, including a final single-token fallback.
    for (int total : {1, 2, 3, 127, 128, 129, 8191, 8192, 8193, 131072, 131073, 1048512}) {
        for (int batch : {1, 2, 127, 128, 8192}) {
            const int keep = std::min(total, 128);
            std::vector<int> seen(keep), values(keep, -1);
            for (int begin = 0; begin < total; ) {
                const int width = std::min(batch, total - begin);
                const auto s = aster_prefill::tail_slice(total, keep, begin, width);
                for (int j = 0; j < s.count; ++j) {
                    ++seen[s.destination + j];values[s.destination + j] = begin + s.column + j;
                }
                begin += width;
            }
            for (int j = 0; j < keep; ++j) {
                assert(seen[j] == 1);assert(values[j] == total - keep + j);
            }
            ++cases;
        }
    }
    assert(aster_prefill::tail_slice(10, 11, 0, 2).count == 0);
    assert(aster_prefill::tail_slice(10, 2, 9, 2).count == 0);
    constexpr double G = 1073741824.0, M = 1048576.0;
    for (int columns : {128, 4096, 8192}) {
        for (double graph : {0.0, 1.16 * G, 4.56 * G, 9.06 * G}) {
            const double n = aster_prefill::reservation_bytes(columns, graph, .5);
            assert(n >= 4.3 * G + .95 * M * columns + .5 * G);
            assert(n >= 2.5 * G + .78 * M * columns + graph + .5 * G);
            ++cases;
        }
    }
    assert(std::isinf(aster_prefill::reservation_bytes(0, 0, .5)));
    assert(std::isinf(aster_prefill::reservation_bytes(1, -1, .5)));
    bool enabled = true;
    assert(aster_prefill::qos_value(nullptr, enabled) && !enabled);
    assert(aster_prefill::qos_value("0", enabled) && !enabled);
    assert(aster_prefill::qos_value("1", enabled) && enabled);
    for (const char * bad : {"", "2", "true", " 1", "1x"}) assert(!aster_prefill::qos_value(bad, enabled));
    assert(aster_prefill::initial_width(8192, 0, 35700) == 1024);
    assert(aster_prefill::initial_width(8192, 1024, 35700) == 8192);
    assert(aster_prefill::initial_width(128, 0, 35700) == 128);
    assert(aster_prefill::initial_width(400, 0, 400) == 400);
    assert(aster_prefill::yield_width(8192, 1024, 128, 2560) == 2560);
    assert(aster_prefill::yield_width(8192, 0, 128, 2560) == 1024);
    assert(aster_prefill::yield_width(128, 1024, 128, 2560) == 128);
    // Check exact tail coverage after a small first chunk and a replay-width handoff.
    for (int total : {8193, 35700, 131073, 1048512}) {
        std::vector<int> seen(128);
        for (int begin = 0; begin < total; ) {
            int width = std::min(8192, total - begin);
            width = aster_prefill::initial_width(width, begin, total);
            if (begin == 1024) width = aster_prefill::yield_width(width, begin, 128, 2560);
            const auto s = aster_prefill::tail_slice(total, 128, begin, width);
            for (int j = 0; j < s.count; ++j) ++seen[s.destination + j];
            begin += width;
        }
        for (int n : seen) assert(n == 1);
        ++cases;
    }
    std::printf("PASS %d tail/budget/window cases plus strict QoS flag and replay-floor checks\n", cases);
}
