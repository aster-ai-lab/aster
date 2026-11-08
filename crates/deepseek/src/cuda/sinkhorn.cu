// Original Aster implementation, MIT AND Apache-2.0.
// Input is the pinned XQ row-softmax + epsilon, admitted in [0,2].
// Keep all 39 normalizations in one warp. No approximate math or FMA.
extern "C" __global__ void aster_xq_sinkhorn4_f32_v1(
    const float* input, float* output, unsigned tokens) {
    unsigned lane = threadIdx.x & 31;
    unsigned token = blockIdx.x * 4 + threadIdx.x / 32;
    if (token >= tokens || lane >= 16) return;
    float value = input[(unsigned long long)token * 16 + lane];
    const unsigned mask = 0x0000ffff;
    // First normalize columns (axis 1). Then row/column for iterations 1..19.
    for (unsigned iteration = 0; iteration < 20; ++iteration) {
        if (iteration != 0) {
            float sum = __fadd_rn(value, __shfl_xor_sync(mask, value, 1));
            sum = __fadd_rn(sum, __shfl_xor_sync(mask, sum, 2));
            value = __fdiv_rn(value, __fadd_rn(sum, 0.000001f));
        }
        float sum = __fadd_rn(value, __shfl_xor_sync(mask, value, 4));
        sum = __fadd_rn(sum, __shfl_xor_sync(mask, sum, 8));
        value = __fdiv_rn(value, __fadd_rn(sum, 0.000001f));
    }
    output[(unsigned long long)token * 16 + lane] = value;
}
