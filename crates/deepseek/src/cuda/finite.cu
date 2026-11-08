// Original Aster implementation, MIT AND Apache-2.0.
// Exact finite/nonnegative predicate. No activation values leave the device.
extern "C" __global__ void aster_finite_f32_flag_v1(
    const float* input, unsigned* invalid, unsigned elements, unsigned nonnegative) {
    unsigned long long index = (unsigned long long)blockIdx.x * blockDim.x + threadIdx.x;
    bool bad = false;
    if (index < elements) {
        float value = input[index];
        float largest = __uint_as_float(0x7f7fffff);
        bad = !(value <= largest && value >= -largest) || (nonnegative && value < 0.f);
    }
    unsigned bad_lanes = __ballot_sync(0xffffffff, bad);
    if ((threadIdx.x & 31) == 0 && bad_lanes != 0) atomicOr(invalid, 1u);
}
