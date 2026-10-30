// Standalone research path; no existing CUDA archive or serving binary is changed.
#include "aster_topk_order_v192.hpp"
#include <cuda_runtime.h>
#include <cub/version.cuh>
#include <cub/device/device_topk.cuh>
#include <cub/device/device_radix_sort.cuh>
#include <cuda/execution>
#include <cuda/stream_ref>
#include <cuda/std/execution>
#include <algorithm>
#include <cstddef>
#include <cstdint>
#if CUB_VERSION != 300500
#error "exact official CCCL3.5.0 headers required"
#endif

namespace {
using Key=std::uint64_t;
auto environment(cudaStream_t stream) {
    return cuda::std::execution::env{cuda::stream_ref{stream},cuda::execution::require(
        cuda::execution::determinism::not_guaranteed,cuda::execution::tie_break::unspecified,
        cuda::execution::output_ordering::unsorted)};
}
__global__ void pack_keys(const float* input,Key* packed,int cols,std::size_t count) {
    const std::size_t i=static_cast<std::size_t>(blockIdx.x)*blockDim.x+threadIdx.x;
    if(i<count)packed[i]=aster_topk::key(__float_as_uint(input[i]),static_cast<unsigned>(i%static_cast<unsigned>(cols)));
}
__global__ void unpack_keys(const Key* packed,int* output,std::size_t count) {
    const std::size_t i=static_cast<std::size_t>(blockIdx.x)*blockDim.x+threadIdx.x;
    if(i<count)output[i]=static_cast<int>(aster_topk::column(packed[i]));
}
}

extern "C" cudaError_t aster_topk35_workspace(int cols,int k,std::size_t* bytes) {
    if(cols<1 || k<1 || k>cols || !bytes)return cudaErrorInvalidValue;
    std::size_t select=0,sort=0;
    auto status=cub::DeviceTopK::MaxKeys(nullptr,select,static_cast<const Key*>(nullptr),
        static_cast<Key*>(nullptr),cols,k,environment(nullptr));
    if(status!=cudaSuccess)return status;
    status=cub::DeviceRadixSort::SortKeysDescending(nullptr,sort,static_cast<const Key*>(nullptr),static_cast<Key*>(nullptr),k,0,64,nullptr);
    if(status==cudaSuccess)*bytes=std::max(select,sort);
    return status;
}

extern "C" cudaError_t aster_topk35_run(const float* input,int* output,int cols,int rows,int k,
    Key* packed,Key* selected,Key* sorted,void* scratch,std::size_t bytes,cudaStream_t stream) {
    if(!input || !output || !packed || !selected || !sorted || !scratch || !bytes || cols<1 || cols>1048576 || rows<1 || rows>17 || k<1 || k>cols)
        return cudaErrorInvalidValue;
    const std::size_t n=static_cast<std::size_t>(cols)*rows,nout=static_cast<std::size_t>(k)*rows;
    pack_keys<<<static_cast<unsigned>((n+255)/256),256,0,stream>>>(input,packed,cols,n);
    auto status=cudaGetLastError();if(status!=cudaSuccess)return status;
    const auto env=environment(stream);
    for(int row=0;row<rows;++row){
        std::size_t used=bytes;
        status=cub::DeviceTopK::MaxKeys(scratch,used,packed+static_cast<std::size_t>(row)*cols,
            selected+static_cast<std::size_t>(row)*k,cols,k,env);
        if(status!=cudaSuccess)return status;
        used=bytes;
        status=cub::DeviceRadixSort::SortKeysDescending(scratch,used,selected+static_cast<std::size_t>(row)*k,
            sorted+static_cast<std::size_t>(row)*k,k,0,64,stream);
        if(status!=cudaSuccess)return status;
    }
    unpack_keys<<<static_cast<unsigned>((nout+255)/256),256,0,stream>>>(sorted,output,nout);
    return cudaGetLastError();
}
