// A bounded CUDA-counter prerequisite. No model, weights, context or API server.
#include <cuda_runtime.h>
#include <cstdint>
#include <cstdio>
#include <vector>

__global__ void aster_h200_counter_smoke(uint32_t * data, int count) {
    const int i = blockIdx.x * blockDim.x + threadIdx.x;
    if (i < count) {
        uint32_t value = static_cast<uint32_t>(i);
        for (int round = 0; round < 64; ++round) value = value * 1664525u + 1013904223u;
        data[i] = value;
    }
}
int main() {
    constexpr int count = 65536;
    int devices = 0;
    if (cudaGetDeviceCount(&devices) != cudaSuccess || devices != 1) return 2;
    cudaDeviceProp properties{};
    if (cudaGetDeviceProperties(&properties, 0) != cudaSuccess || properties.major != 9 || properties.minor != 0) return 2;
    uint32_t * data = nullptr;
    if (cudaMalloc(&data, count * sizeof(uint32_t)) != cudaSuccess) return 2;
    aster_h200_counter_smoke<<<count / 256, 256>>>(data, count);
    if (cudaDeviceSynchronize() != cudaSuccess) return 3;
    std::vector<uint32_t> actual(count);
    if (cudaMemcpy(actual.data(), data, count * sizeof(uint32_t), cudaMemcpyDeviceToHost) != cudaSuccess) return 3;
    for (int i = 0; i < count; ++i) {
        uint32_t expected = static_cast<uint32_t>(i);
        for (int round = 0; round < 64; ++round) expected = expected * 1664525u + 1013904223u;
        if (actual[i] != expected) return 4;
    }
    if (cudaFree(data) != cudaSuccess) return 3;
    std::puts("{\"phase\":\"passed\",\"GPU_executed\":true,\"checked_output_words\":65536,\"model_forward_executed\":false}");
}
