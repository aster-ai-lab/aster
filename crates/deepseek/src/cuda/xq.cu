// Original Aster CUDA math control, MIT AND Apache-2.0.
// Packed layouts follow pinned Xolotl v41_formats.py at
// b3452666e07a6374d06bb8c2185dffa271e8f65f; MIT notice is retained in
// crates/deepseek/licenses/Xolotl-MIT.txt. No expanded resident weight copy.
// One warp per output, F32 multiplication/reduction. This is an explicit
// correctness control, not a Tensor Core or throughput qualification.

__device__ __forceinline__ unsigned xq_u16(const unsigned char* p) {
    return unsigned(p[0]) | (unsigned(p[1]) << 8);
}

__device__ __forceinline__ float xq_half(const unsigned char* p) {
    unsigned h = xq_u16(p), e = (h >> 10) & 31, f = h & 1023;
    unsigned sign = (h & 0x8000) << 16;
    if (e == 0) {
        float v = float(f) * (1.f / 16777216.f);
        return __uint_as_float(__float_as_uint(v) | sign);
    }
    return __uint_as_float(sign | ((e + 112) << 23) | (f << 13));
}

__device__ __forceinline__ float xq_value(const unsigned char* row, unsigned col, unsigned type) {
    if (type == 0) {
        const unsigned char* p = row + (unsigned long long)col * 4;
        unsigned bits = unsigned(p[0]) | (unsigned(p[1]) << 8)
            | (unsigned(p[2]) << 16) | (unsigned(p[3]) << 24);
        return __uint_as_float(bits);
    }
    if (type == 1) return xq_half(row + (unsigned long long)col * 2);
    if (type == 8) {
        const unsigned char* p = row + (unsigned long long)(col / 32) * 34;
        return float(int((signed char)p[2 + col % 32])) * xq_half(p);
    }
    if (type == 42) {
        const unsigned char* p = row + (unsigned long long)(col / 64) * 18;
        unsigned i = col % 64;
        int q = int((p[2 + i / 4] >> ((i % 4) * 2)) & 3) - 1;
        return float(q) * xq_half(p);
    }
    if (type == 143) {
        const unsigned char* p = row + (unsigned long long)(col / 128) * 28;
        unsigned i = col % 128, pos, power;
        if (i < 80) { pos = i % 16; power = i / 16; }
        else if (i < 120) { pos = 16 + (i - 80) % 8; power = (i - 80) / 8; }
        else { pos = 24 + (i - 120) % 2; power = (i - 120) / 2; }
        unsigned mul = power == 0 ? 1 : power == 1 ? 3 : power == 2 ? 9 : power == 3 ? 27 : 81;
        int q = int((((unsigned(p[pos]) * mul) & 255) * 3) >> 8) - 1;
        return float(q) * xq_half(p + 26);
    }
    // Rust admission refuses all remaining type IDs except Q3_K=11.
    const unsigned char* p = row + (unsigned long long)(col / 256) * 110;
    unsigned i = col % 256, h = i / 128, j = (i % 128) / 32, lane = i % 32;
    int low = int((p[32 + h * 32 + lane] >> (2 * j)) & 3);
    int q = low - (((p[lane] >> (4 * h + j)) & 1) ? 0 : 4);
    unsigned sub = i / 16, s = sub % 4;
    unsigned a = p[96 + s], c = p[100 + s], hi = p[104 + s], scale;
    if (sub < 4) scale = (a & 15) | ((hi & 3) << 4);
    else if (sub < 8) scale = (c & 15) | (((hi >> 2) & 3) << 4);
    else if (sub < 12) scale = (a >> 4) | (((hi >> 4) & 3) << 4);
    else scale = (c >> 4) | (((hi >> 6) & 3) << 4);
    return (float(q) * xq_half(p + 108)) * float(int(scale) - 32);
}

extern "C" __global__ void aster_xq_warp_f32_v1(
    const float* a, const unsigned char* w, float* out,
    unsigned m, unsigned n, unsigned k, unsigned type, unsigned row_bytes) {
    unsigned lane = threadIdx.x & 31;
    unsigned long long flat = (unsigned long long)blockIdx.x * 4 + threadIdx.x / 32;
    if (flat >= (unsigned long long)m * n) return;
    unsigned token = unsigned(flat / n), row_index = unsigned(flat % n);
    const unsigned char* row = w + (unsigned long long)row_index * row_bytes;
    const float* input = a + (unsigned long long)token * k;
    float sum = 0.f;
    for (unsigned col = lane; col < k; col += 32) sum += input[col] * xq_value(row, col, type);
    for (unsigned mask = 16; mask > 0; mask /= 2) sum += __shfl_down_sync(0xffffffff, sum, mask);
    if (lane == 0) out[flat] = sum;
}

extern "C" __global__ void aster_xq_gather_f32_v1(
    const unsigned* ids, const unsigned char* w, float* out,
    unsigned tokens, unsigned rows, unsigned columns, unsigned type, unsigned row_bytes) {
    unsigned long long flat = (unsigned long long)blockIdx.x * blockDim.x + threadIdx.x;
    if (flat >= (unsigned long long)tokens * columns) return;
    unsigned token = unsigned(flat / columns), col = unsigned(flat % columns), row = ids[token];
    // Admission checks ids on the same immutable, event-tracked snapshot.
    // This extra branch keeps malformed indices from reading any other row.
    if (row >= rows) { out[flat] = __uint_as_float(0x7fc00000); return; }
    out[flat] = xq_value(w + (unsigned long long)row * row_bytes, col, type);
}
