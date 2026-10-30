// Tiny real-CUDA validation of the exact interactive start/stop path; no model.
#include <cuda_runtime.h>
#include <cuda_profiler_api.h>
#include <chrono>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <unistd.h>
#include <vector>
__global__ void aster_interactive_nsys_smoke(std::uint32_t* out,int n) {
    const int i=blockIdx.x*blockDim.x+threadIdx.x;
    if(i<n) {std::uint32_t v=i;for(int j=0;j<64;++j)v=v*1664525u+1013904223u;out[i]=v;}
}
int main(int argc,char** argv) {
    if(argc!=2)return 2;
    int count=0;cudaDeviceProp props{};
    if(cudaGetDeviceCount(&count)!=cudaSuccess || count!=1 || cudaGetDeviceProperties(&props,0)!=cudaSuccess || props.major!=9)return 2;
    char uuid[33];
    for(int i=0;i<16;++i)std::snprintf(uuid+2*i,3,"%02x",static_cast<unsigned char>(props.uuid.bytes[i]));
    if(std::strcmp(uuid,argv[1]))return 3;
    constexpr int n=65536;std::uint32_t* data=nullptr;
    if(cudaMalloc(&data,n*sizeof(*data))!=cudaSuccess)return 4;
    std::puts("READY_SMOKE");std::fflush(stdout);
    char command[32];
    if(!std::fgets(command,sizeof(command),stdin) || std::strcmp(command,"GO\n"))return 5;
    if(cudaProfilerStart()!=cudaSuccess)return 6;
    const auto begin=std::chrono::steady_clock::now();int launches=0;
    do {
        aster_interactive_nsys_smoke<<<n/256,256>>>(data,n);
        if(cudaDeviceSynchronize()!=cudaSuccess)return 7;
        ++launches;usleep(5000);
    } while(std::chrono::steady_clock::now()-begin<std::chrono::seconds(6));
    std::vector<std::uint32_t> actual(n);
    if(cudaMemcpy(actual.data(),data,n*sizeof(*data),cudaMemcpyDeviceToHost)!=cudaSuccess)return 8;
    for(int i=0;i<n;++i) {
        std::uint32_t v=i;for(int j=0;j<64;++j)v=v*1664525u+1013904223u;
        if(actual[i]!=v)return 9;
    }
    if(cudaProfilerStop()!=cudaSuccess)return 10;
    std::printf("DONE_SMOKE %d %d\n",n,launches);std::fflush(stdout);
    if(!std::fgets(command,sizeof(command),stdin) || std::strcmp(command,"QUIT\n"))return 11;
    if(cudaFree(data)!=cudaSuccess)return 12;
}
