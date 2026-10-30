// Conservative MXFP4 scale-header proof for the existing |K| <= 1024 guard.
#pragma once
#include <cstdint>
namespace aster_indexer_scale {
#ifdef __CUDACC__
__host__ __device__
#endif
constexpr bool bounded_mxfp4_header(std::uint8_t exponent) noexcept {
    // Existing CUDA dequantization: E8M0(exponent) * 0.5f * nibble,
    // where the largest absolute nibble value is 12.
    // exponent134 gives max768; exponent135 can give1536.
    return exponent <= 134;
}
}
