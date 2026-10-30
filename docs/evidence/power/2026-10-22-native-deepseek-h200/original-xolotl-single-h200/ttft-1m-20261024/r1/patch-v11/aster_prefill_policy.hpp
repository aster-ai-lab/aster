// Exact prompt execution policy; no changes to expert selection or token sampling.
#pragma once
#include <algorithm>
#include <cmath>
#include <limits>

namespace aster_prefill {
struct TailSlice { int column, count, destination; };
inline TailSlice tail_slice(int total, int keep, int begin, int width) {
    if (total < 1 || keep < 1 || keep > total || begin < 0 || begin >= total || width < 1 || width > total - begin)
        return {0, 0, 0};
    const int start = total - keep;
    const int lo = std::max(begin, start), hi = begin + width;
    return lo < hi ? TailSlice{lo - begin, hi - lo, lo - start} : TailSlice{0, 0, 0};
}
inline double reservation_bytes(int columns, double graph_bytes, double margin_gib) {
    if (columns < 1 || !std::isfinite(graph_bytes) || graph_bytes < 0 || !std::isfinite(margin_gib) || margin_gib < 0)
        return std::numeric_limits<double>::infinity();
    constexpr double GiB = 1073741824.0, MiB = 1048576.0;
    return std::max(4.3 * GiB + .95 * MiB * columns,
                    2.5 * GiB + .78 * MiB * columns + graph_bytes) + margin_gib * GiB;
}
}
