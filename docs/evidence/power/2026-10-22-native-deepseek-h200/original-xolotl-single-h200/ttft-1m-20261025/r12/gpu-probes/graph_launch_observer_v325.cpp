#include <cuda_runtime_api.h>
#include <atomic>
#include <cstdint>
static std::atomic<uint64_t> launches{0},last_exec{0};
extern "C" cudaError_t __real_cudaGraphLaunch(cudaGraphExec_t,cudaStream_t);
extern "C" cudaError_t __wrap_cudaGraphLaunch(cudaGraphExec_t graph,cudaStream_t stream) {
    cudaError_t e=__real_cudaGraphLaunch(graph,stream);
    if(e==cudaSuccess){last_exec.store(reinterpret_cast<uintptr_t>(graph));launches.fetch_add(1);}
    return e;
}
extern "C" uint64_t aster_graph_launch_count(){return launches.load();}
extern "C" uint64_t aster_graph_last_exec(){return last_exec.load();}
