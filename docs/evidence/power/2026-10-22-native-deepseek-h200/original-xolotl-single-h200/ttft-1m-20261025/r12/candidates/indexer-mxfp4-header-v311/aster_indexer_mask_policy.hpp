// Closed experimental switch and a conservative no-overflow contract.
#pragma once
#include <cstring>
#include <cstdlib>
namespace aster_indexer {
enum class Mode { off, on, invalid };
inline Mode parse(const char* text) {
    if (!text || std::strcmp(text, "0") == 0) return Mode::off;
    if (std::strcmp(text, "1") == 0) return Mode::on;
    return Mode::invalid;
}
constexpr float max_abs = 1024.0f;
// 128 products, at most64 heads and weights bounded by1024 cannot overflow
// an F32 accumulator. WMMA checks its actual half-dequantized K values too.
#if defined(__CUDACC__)
__host__ __device__
#endif
inline bool bounded(float value) { return value >= -max_abs && value <= max_abs; }
} // namespace aster_indexer
