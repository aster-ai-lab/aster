#pragma once
#include <cuda_runtime_api.h>
#include <cstddef>
#include <cstdint>
extern "C" cudaError_t aster_topk35_workspace(int cols,int k,std::size_t* bytes);
extern "C" cudaError_t aster_topk35_run(const float* input,int* output,int cols,int rows,int k,
    std::uint64_t* packed,std::uint64_t* selected,std::uint64_t* sorted,void* scratch,std::size_t bytes,cudaStream_t stream);
