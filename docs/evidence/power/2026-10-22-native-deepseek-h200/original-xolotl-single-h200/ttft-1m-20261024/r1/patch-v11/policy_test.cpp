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
    std::printf("PASS %d tail coverage and context graph budget cases\n", cases);
}
