// Exact radix order: float score descending, then original column ascending.
// CUB radix sorting treats signed zeros equally and otherwise orders float bits.
#pragma once
#include <cstdint>
#if defined(__CUDACC__)
#define ASTER_TOPK_HD __host__ __device__
#else
#define ASTER_TOPK_HD
#endif
namespace aster_topk {
ASTER_TOPK_HD constexpr std::uint64_t key(std::uint32_t bits,std::uint32_t column) {
    if ((bits&0x7fffffffu)==0) bits=0;
    const std::uint32_t order=(bits&0x80000000u)?~bits:(bits^0x80000000u);
    return (static_cast<std::uint64_t>(order)<<32)|static_cast<std::uint32_t>(~column);
}
ASTER_TOPK_HD constexpr std::uint32_t column(std::uint64_t packed) {
    return ~static_cast<std::uint32_t>(packed);
}
}
#undef ASTER_TOPK_HD
