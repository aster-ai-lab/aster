// All threads take the same returns. No extra allocation or approximation.
#pragma once
#include "aster_indexer_mask_policy.hpp"

template<bool WMMA, int KV, int EMB, int HEAD, ggml_type TYPE_K>
static __device__ __forceinline__ bool aster_indexer_masked_block(
        const float* Q, const char* K, const float* W, const half* M, float* dst,
        int start, int n_kv, int i_batch, int i_stream, int nem3,
        size_t nb1, size_t nb3, size_t nbq1, size_t nbq2, size_t nbq3,
        size_t nbk2, size_t nbk3, size_t nbw1, size_t nbw3, size_t nbm1, size_t nbm3) {
    const int tid = threadIdx.y * WARP_SIZE + threadIdx.x;
    const int nt = blockDim.x * blockDim.y;
    __shared__ int eligible;
    if (tid == 0) eligible = 1;
    __syncthreads();
    const half* mask = reinterpret_cast<const half*>(reinterpret_cast<const char*>(M) + i_batch*nbm1 + (i_stream%nem3)*nbm3);
    bool ok = true;
    for (int k = tid; k < KV; k += nt)
        if (start+k < n_kv && __half_as_ushort(mask[start+k]) != 0xfc00u) ok = false;
    if (!ok) atomicExch(&eligible, 0);
    __syncthreads();
    if (!eligible) return false;

    // Masked output is exactly -inf only if the original dot/weighted sum
    // stays finite. Unsafe/NaN/Inf/out-of-range inputs run the original code.
    const char* q = reinterpret_cast<const char*>(Q) + i_batch*nbq2 + i_stream*nbq3;
    const float* w = reinterpret_cast<const float*>(reinterpret_cast<const char*>(W) + i_batch*nbw1 + i_stream*nbw3);
    for (int h = tid; h < HEAD; h += nt) ok &= aster_indexer::bounded(w[h]);
    for (int at = tid; at < HEAD*(EMB/4); at += nt) {
        const int h = at/(EMB/4), d = at%(EMB/4);
        const float4 v = reinterpret_cast<const float4*>(q + h*nbq1)[d];
        ok &= aster_indexer::bounded(v.x) && aster_indexer::bounded(v.y) &&
              aster_indexer::bounded(v.z) && aster_indexer::bounded(v.w);
    }
    for (int at = tid; at < KV*(EMB/4); at += nt) {
        const int k = at/(EMB/4), d = at%(EMB/4);
        if (start+k >= n_kv) continue;
        const void* key = K + (start+k)*nbk2 + i_stream*nbk3;
        if constexpr (WMMA) {
            __align__(16) half v[4];
            constexpr dequantize_V_t read = get_dequantize_V<TYPE_K, half, 4>();
            read(key, v, d*4);
            for (int i=0; i<4; ++i) ok &= aster_indexer::bounded(__half2float(v[i]));
        } else if constexpr (TYPE_K == GGML_TYPE_F32) {
            const float4 v = reinterpret_cast<const float4*>(key)[d];
            ok &= aster_indexer::bounded(v.x) && aster_indexer::bounded(v.y) &&
                  aster_indexer::bounded(v.z) && aster_indexer::bounded(v.w);
        } else {
            __align__(16) float v[4];
            constexpr dequantize_V_t read = get_dequantize_V<TYPE_K, float, 4>();
            read(key, v, d*4);
            for (int i=0; i<4; ++i) ok &= aster_indexer::bounded(v[i]);
        }
    }
    if (!ok) atomicExch(&eligible, 0);
    __syncthreads();
    if (!eligible) return false;
    float* output = reinterpret_cast<float*>(reinterpret_cast<char*>(dst) + i_batch*nb1 + i_stream*nb3);
    for (int k=tid; k<KV; k+=nt)
        if (start+k < n_kv) output[start+k] = __int_as_float(static_cast<int>(0xff800000u));
    return true;
}
