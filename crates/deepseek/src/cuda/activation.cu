// Original Aster CUDA, MIT AND Apache-2.0. No fast math or precision change.
// Every work item writes fresh output. No payload or status is read by the host.
// NVRTC has no host system-header search path. Use only builtin CUDA math
// and explicit ABI-sized scalar aliases, as the existing native kernels do.
typedef unsigned char uint8_t;
typedef unsigned long long uint64_t;

__device__ __forceinline__ float e4_positive(unsigned code) {
    unsigned exponent = code >> 3;
    unsigned mantissa = code & 7;
    if (exponent == 0) return float(mantissa) * 0.001953125f;
    return (1.0f + float(mantissa) * 0.125f) *
           __uint_as_float((exponent + 120u) << 23);
}

__device__ __forceinline__ unsigned encode_e4(float x) {
    const float absolute = fabsf(x);
    unsigned low = 0, high = 126;
    // Exactly the tensor control's seven bisection steps and even-code ties.
    #pragma unroll
    for (unsigned step = 0; step < 7; ++step) {
        unsigned middle = (low + high) / 2;
        float threshold = middle == 126 ? __uint_as_float(0x7f7fffffu) :
            (e4_positive(middle) + e4_positive(middle + 1)) * 0.5f;
        bool higher = absolute > threshold ||
            (absolute == threshold && (middle & 1));
        if (higher) low = middle + 1; else high = middle;
    }
    // Preserve the original reciprocal comparison, including negative zero.
    return low + (__fdiv_rn(1.0f, x) < 0.0f ? 128u : 0u);
}

__device__ __forceinline__ float decode_e4(unsigned code) {
    if ((code & 127u) == 127u) return __uint_as_float(0x7fc00000u);
    unsigned bits = __float_as_uint(e4_positive(code & 127u));
    return __uint_as_float(bits | ((code & 128u) << 24));
}

__device__ __forceinline__ unsigned encode_e2(float x) {
    const float absolute = fabsf(x);
    const float thresholds[7] = {0.25f,0.75f,1.25f,1.75f,2.5f,3.5f,5.0f};
    unsigned code = 0;
    #pragma unroll
    for (unsigned lower = 0; lower < 7; ++lower) {
        bool higher = (lower & 1) ? absolute >= thresholds[lower] :
                                   absolute > thresholds[lower];
        if (higher) code = lower + 1;
    }
    return code + (__fdiv_rn(1.0f, x) < 0.0f ? 8u : 0u);
}

__device__ __forceinline__ float decode_e2(unsigned code) {
    const float palette[8] = {0.0f,0.5f,1.0f,1.5f,2.0f,3.0f,4.0f,6.0f};
    unsigned bits = __float_as_uint(palette[code & 7u]);
    return __uint_as_float(bits | ((code & 8u) << 28));
}

extern "C" __global__ void aster_encode_e4m3_f32_v1(
    const float* input, uint8_t* output, unsigned count) {
    uint64_t i = uint64_t(blockIdx.x) * blockDim.x + threadIdx.x;
    if (i < count) output[i] = uint8_t(encode_e4(input[i]));
}
extern "C" __global__ void aster_decode_e4m3_f32_v1(
    const uint8_t* input, float* output, unsigned count) {
    uint64_t i = uint64_t(blockIdx.x) * blockDim.x + threadIdx.x;
    if (i < count) output[i] = decode_e4(input[i]);
}
extern "C" __global__ void aster_encode_e2m1_f32_v1(
    const float* input, uint8_t* output, unsigned count) {
    uint64_t i = uint64_t(blockIdx.x) * blockDim.x + threadIdx.x;
    if (i < count) output[i] = uint8_t(encode_e2(input[2*i]) |
                                            (encode_e2(input[2*i+1]) << 4));
}
extern "C" __global__ void aster_decode_e2m1_f32_v1(
    const uint8_t* input, float* output, unsigned count) {
    uint64_t i = uint64_t(blockIdx.x) * blockDim.x + threadIdx.x;
    if (i < count) {
        unsigned byte = input[i];
        output[2*i] = decode_e2(byte & 15u);
        output[2*i+1] = decode_e2(byte >> 4);
    }
}
