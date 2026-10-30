// Reject invalid experimental modes before main or any model load.
#include "aster_indexer_mask_policy.hpp"
#include <cstdio>
__attribute__((constructor)) static void aster_mxfp4_header_mode_guard() {
    const auto header = aster_indexer::parse(std::getenv("ASTER_XQ_INDEXER_MXFP4_HEADER"));
    if (header == aster_indexer::Mode::invalid) {
        std::fputs("ASTER_XQ_INDEXER_MXFP4_HEADER must be 0 or 1\n", stderr);
        std::_Exit(2);
    }
    if (header == aster_indexer::Mode::on &&
        aster_indexer::parse(std::getenv("ASTER_XQ_INDEXER_MASK_SKIP")) != aster_indexer::Mode::on) {
        std::fputs("ASTER_XQ_INDEXER_MXFP4_HEADER requires ASTER_XQ_INDEXER_MASK_SKIP=1\n", stderr);
        std::_Exit(2);
    }
}
