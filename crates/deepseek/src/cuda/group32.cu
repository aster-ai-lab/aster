// Original Aster implementation, MIT AND Apache-2.0, ABI v1.
// Fragment coordinates follow NVIDIA CUDA 12.6 PTX ISA 8.5, §9.7.15.4.10:
// https://docs.nvidia.com/cuda/archive/12.6.0/parallel-thread-execution/index.html#warp-level-matrix-fragment-mma-16832
// One warp computes 16x8 outputs. Each K=32 group resets FP32 MMA
// accumulation before applying its exact UE8M0 exponent sum in FP32.
// E2M1 -> E4M3 conversion is lossless and local to registers. No BF16 weight
// expansion, shared memory, dynamic workspace, atomics or tensor host copies.

__device__ __forceinline__ unsigned fp4_e4m3(unsigned x) {
    const unsigned long long lut = 0x4c4844403c383000ULL;
    return unsigned((lut >> ((x & 7) * 8)) & 255) | ((x & 8) << 4);
}

__device__ __forceinline__ unsigned load_a(
    const unsigned char* a, unsigned row, unsigned k,
    unsigned m, unsigned width) {
    unsigned v = 0;
    if (row < m) {
        #pragma unroll
        for (unsigned j = 0; j < 4; ++j)
            v |= unsigned(a[(unsigned long long)row * width + k + j]) << (j * 8);
    }
    return v;
}

__device__ __forceinline__ unsigned load_b(
    const unsigned char* w, unsigned row, unsigned k,
    unsigned n, unsigned width, unsigned fp4) {
    unsigned v = 0;
    if (row < n) {
        #pragma unroll
        for (unsigned j = 0; j < 4; ++j) {
            unsigned long long pos = (unsigned long long)row * width + k + j;
            unsigned code = fp4 ? fp4_e4m3((w[pos / 2] >> ((pos & 1) * 4)) & 15) : w[pos];
            v |= code << (j * 8);
        }
    }
    return v;
}

extern "C" __global__ void aster_group32_mma_v1(
    const unsigned char* a, const unsigned char* asf,
    const unsigned char* w, const unsigned char* wsf,
    float* out, unsigned m, unsigned n, unsigned k, unsigned fp4) {
    unsigned lane = threadIdx.x, group = lane >> 2, thread = lane & 3;
    unsigned ar = blockIdx.y * 16 + group;
    unsigned br = blockIdx.x * 8 + group;
    unsigned ng = k / 32;
    float result[4] = {0.f, 0.f, 0.f, 0.f};
    for (unsigned kg = 0; kg < ng; ++kg) {
        unsigned kc = kg * 32 + thread * 4;
        unsigned a0 = load_a(a, ar, kc, m, k);
        unsigned a1 = load_a(a, ar + 8, kc, m, k);
        unsigned a2 = load_a(a, ar, kc + 16, m, k);
        unsigned a3 = load_a(a, ar + 8, kc + 16, m, k);
        unsigned b0 = load_b(w, br, kc, n, k, fp4);
        unsigned b1 = load_b(w, br, kc + 16, n, k, fp4);
        float d0, d1, d2, d3;
        asm volatile(
            "mma.sync.aligned.m16n8k32.row.col.f32.e4m3.e4m3.f32 "
            "{%0,%1,%2,%3}, {%4,%5,%6,%7}, {%8,%9}, {0.0,0.0,0.0,0.0};"
            : "=f"(d0), "=f"(d1), "=f"(d2), "=f"(d3)
            : "r"(a0), "r"(a1), "r"(a2), "r"(a3), "r"(b0), "r"(b1));
        float part[4] = {d0, d1, d2, d3};
        #pragma unroll
        for (unsigned i = 0; i < 4; ++i) {
            unsigned r = ar + (i >= 2 ? 8 : 0);
            unsigned c = blockIdx.x * 8 + thread * 2 + (i & 1);
            if (r < m && c < n) {
                unsigned wr = fp4 ? c : c / 32;
                int exponent = int(asf[(unsigned long long)r * ng + kg])
                    + int(wsf[(unsigned long long)wr * ng + kg]) - 254;
                result[i] += scalbnf(part[i], exponent);
            }
        }
    }
    #pragma unroll
    for (unsigned i = 0; i < 4; ++i) {
        unsigned r = ar + (i >= 2 ? 8 : 0);
        unsigned c = blockIdx.x * 8 + thread * 2 + (i & 1);
        if (r < m && c < n) out[(unsigned long long)r * n + c] = result[i];
    }
}

// Explicit slow control for exponent ranges outside MMA v1 qualification.
// Decode each operand to F32 before multiplying. In particular, preserve
// subnormal Engram rows whose small scale is cancelled by a large projection
// scale. FTZ and FMA contraction are disabled in the module compile options.
__device__ __forceinline__ float decode_scaled(unsigned code, unsigned sf) {
    unsigned e = (code >> 3) & 15, f = code & 7;
    float raw = e ? scalbnf(1.f + float(f) * .125f, int(e) - 7)
                  : scalbnf(float(f), -9);
    float v = scalbnf(raw, int(sf) - 127);
    return (code & 128) ? -v : v;
}

extern "C" __global__ void aster_raw_f32_v1(
    const unsigned char* a, const unsigned char* asf,
    const unsigned char* w, const unsigned char* wsf,
    float* out, unsigned m, unsigned n, unsigned k, unsigned fp4) {
    unsigned group = threadIdx.x >> 2, thread = threadIdx.x & 3;
    unsigned ng = k / 32;
    for (unsigned i = 0; i < 4; ++i) {
        unsigned r = blockIdx.y * 16 + group + (i >= 2 ? 8 : 0);
        unsigned col = blockIdx.x * 8 + thread * 2 + (i & 1);
        if (r >= m || col >= n) continue;
        float sum = 0.f;
        for (unsigned q = 0; q < k; ++q) {
            unsigned long long ai = (unsigned long long)r * k + q;
            unsigned long long wi = (unsigned long long)col * k + q;
            unsigned wc = fp4 ? fp4_e4m3((w[wi / 2] >> ((wi & 1) * 4)) & 15) : w[wi];
            float av = decode_scaled(a[ai], asf[(unsigned long long)r * ng + q / 32]);
            float wv = decode_scaled(wc, wsf[(unsigned long long)(fp4 ? col : col / 32) * ng + q / 32]);
            sum = __fadd_rn(sum, __fmul_rn(av, wv));
        }
        out[(unsigned long long)r * n + col] = sum;
    }
}
