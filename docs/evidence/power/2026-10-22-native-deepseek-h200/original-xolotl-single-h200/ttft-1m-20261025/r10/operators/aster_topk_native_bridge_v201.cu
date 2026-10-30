// Private experimental bridge. The installed engine and its archives are unchanged.
// Compile the exact retained original source with its public operation renamed.
#define ggml_cuda_op_top_k aster_ggml_cuda_op_top_k_original_v201
#include "top-k.original.cu"
#undef ggml_cuda_op_top_k

#include "aster_topk35_api_v192.hpp"
#include <cstring>
#include <limits>

namespace {
int checked_mode() {
    const char * value = std::getenv("ASTER_XQ_TOPK35");
    if (value == nullptr || std::strcmp(value, "0") == 0) return 0;
    if (std::strcmp(value, "1") == 0) return 1;
    std::fprintf(stderr, "ASTER_XQ_TOPK35 requires exactly 0 or 1\n");
    std::exit(2);
}
// Validate before main and before model loading. No CUDA initialization here.
const int mode = checked_mode();
}

void ggml_cuda_op_top_k(ggml_backend_cuda_context & ctx, ggml_tensor * dst) {
    const ggml_tensor * src = dst->src[0];
    const int64_t cols = src->ne[0];
    const int64_t rows = ggml_nrows(src);
    const int64_t k = dst->ne[0];
    if (mode == 0 || src->type != GGML_TYPE_F32 || dst->type != GGML_TYPE_I32 ||
        !ggml_is_contiguous(src) || !ggml_is_contiguous(dst) ||
        cols < 65536 || cols > 1048576 || rows < 1 || rows > 1048576 || k != 512) {
        aster_ggml_cuda_op_top_k_original_v201(ctx, dst);
        return;
    }

    constexpr int max_chunk_rows = 17;
    constexpr size_t allocation_limit = 256ull * 1024 * 1024;
    const int chunk_rows = static_cast<int>(std::min<int64_t>(rows, max_chunk_rows));
    const size_t input_count = static_cast<size_t>(cols) * chunk_rows;
    const size_t output_count = static_cast<size_t>(k) * chunk_rows;
    size_t workspace_bytes = 0;
    CUDA_CHECK(aster_topk35_workspace(static_cast<int>(cols), static_cast<int>(k), &workspace_bytes));
    const size_t key_bytes = (input_count + 2 * output_count) * sizeof(uint64_t);
    if (workspace_bytes == 0 || workspace_bytes > allocation_limit ||
        key_bytes > allocation_limit - workspace_bytes) {
        aster_ggml_cuda_op_top_k_original_v201(ctx, dst);
        return;
    }
    ggml_cuda_pool & pool = ctx.pool();
    ggml_cuda_pool_alloc<uint64_t> packed(pool, input_count);
    ggml_cuda_pool_alloc<uint64_t> selected(pool, output_count);
    ggml_cuda_pool_alloc<uint64_t> sorted(pool, output_count);
    ggml_cuda_pool_alloc<unsigned char> workspace(pool, workspace_bytes);
    const auto * source = static_cast<const float *>(src->data);
    auto * result = static_cast<int *>(dst->data);
    for (int64_t offset = 0; offset < rows; offset += chunk_rows) {
        const int count = static_cast<int>(std::min<int64_t>(rows - offset, chunk_rows));
        CUDA_CHECK(aster_topk35_run(source + static_cast<size_t>(offset) * cols,
            result + static_cast<size_t>(offset) * k, static_cast<int>(cols), count,
            static_cast<int>(k), packed.get(), selected.get(), sorted.get(),
            workspace.get(), workspace_bytes, ctx.stream()));
    }
    CUDA_CHECK(cudaGetLastError());
}
