// Compare the actual linked ggml Top-K against the isolated packed-key prototype.
// Timings are synchronized operator-harness wall times, never API/model rates.
#include "aster_topk35_api_v192.hpp"
#include "ggml.h"
#include "ggml-backend.h"
#include "ggml-cuda.h"
#include <algorithm>
#include <array>
#include <bit>
#include <chrono>
#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <limits>
#include <random>
#include <vector>

static void require(bool ok,const char* message){if(!ok){std::fprintf(stderr,"FAIL %s\n",message);std::exit(1);}}
static void cu(cudaError_t status){if(status!=cudaSuccess){std::fprintf(stderr,"CUDA %s\n",cudaGetErrorString(status));std::exit(2);}}
template<class T>static T* memory(std::size_t count){T* pointer=nullptr;cu(cudaMalloc(reinterpret_cast<void**>(&pointer),count*sizeof(T)));return pointer;}
static double now(){return std::chrono::duration<double,std::micro>(std::chrono::steady_clock::now().time_since_epoch()).count();}
static bool same(const std::vector<int>& reference,const std::vector<int>& actual,int cols,int rows,int pattern,const char* stage){
    for(std::size_t i=0;i<reference.size();++i)if(reference[i]!=actual[i]){
        std::printf("{\"kind\":\"case\",\"phase\":\"failed\",\"cols\":%d,\"rows\":%d,\"pattern\":%d,\"stage\":\"%s\",\"first_mismatch\":%zu,\"reference_ID\":%d,\"candidate_ID\":%d}\n",cols,rows,pattern,stage,i,reference[i],actual[i]);std::fflush(stdout);return false;
    }
    return true;
}
int main(){
    int devices=0;cu(cudaGetDeviceCount(&devices));require(devices==1,"one visible optimization GPU required");
    cudaDeviceProp props{};cu(cudaGetDeviceProperties(&props,0));require(props.major==9&&props.minor==0,"SM90 H200 required");
    ggml_backend_t backend=ggml_backend_cuda_init(0);require(backend!=nullptr,"original CUDA backend required");
    int cases=0;
    for(int cols:{4097,65537,1048576})for(int rows:{1,6,17})for(int pattern=0;pattern<6;++pattern){
        if(rows==17&&(cols!=1048576||(pattern!=0&&pattern!=2)))continue;
        constexpr int k=512;
        auto* ctx=ggml_init({1024*1024,nullptr,true});require(ctx!=nullptr,"bounded graph metadata allocation");
        auto* input=ggml_new_tensor_2d(ctx,GGML_TYPE_F32,cols,rows);ggml_set_input(input);
        auto* output=ggml_top_k(ctx,input,k);ggml_set_output(output);
        auto* graph=ggml_new_graph_custom(ctx,32,false);ggml_build_forward_expand(graph,output);
        auto buffer=ggml_backend_alloc_ctx_tensors(ctx,backend);require(buffer!=nullptr,"original reference tensor allocation");
        const std::size_t n=static_cast<std::size_t>(cols)*rows,nout=static_cast<std::size_t>(k)*rows;
        std::vector<float> values(n);std::mt19937 random(19937+cols+rows*31+pattern);
        constexpr std::array<std::uint32_t,12> special={0x7fc00000u,0xffc00000u,0x7f800000u,0xff800000u,0u,0x80000000u,1u,0x80000001u,0x3f800000u,0xbf800000u,0x7fffffffu,0xffffffffu};
        for(std::size_t i=0;i<n;++i){
            const int column=static_cast<int>(i%cols);
            if(pattern==0)values[i]=static_cast<float>(static_cast<int>(random()%2000001)-1000000)/65536.f;
            else if(pattern==1)values[i]=static_cast<float>(static_cast<int>(random()%5)-2);
            else if(pattern==2)values[i]=(column/256)%32==0?static_cast<float>(random()%100000)/4096.f:-std::numeric_limits<float>::infinity();
            else if(pattern==3)values[i]=-std::numeric_limits<float>::infinity();
            else if(pattern==4)values[i]=std::bit_cast<float>(i%2?0u:0x80000000u);
            else values[i]=std::bit_cast<float>(special[random()%special.size()]);
        }
        std::vector<int> reference(nout),actual(nout);
        auto set=[&](const std::vector<float>& v){ggml_backend_tensor_set(input,v.data(),0,v.size()*sizeof(float));ggml_backend_synchronize(backend);};
        auto original=[&](){require(ggml_backend_graph_compute(backend,graph)==GGML_STATUS_SUCCESS,"actual linked ggml reference compute");ggml_backend_synchronize(backend);};
        auto get_reference=[&](){ggml_backend_tensor_get(output,reference.data(),0,nout*sizeof(int));};
        std::size_t bytes=0;cu(aster_topk35_workspace(cols,k,&bytes));require(bytes>0&&bytes<=256ull*1024*1024,"bounded prototype scratch required");
        auto* packed=memory<std::uint64_t>(n);auto* selected=memory<std::uint64_t>(nout);auto* sorted=memory<std::uint64_t>(nout);auto* result=memory<int>(nout);auto* scratch=memory<unsigned char>(bytes);
        cudaStream_t stream=nullptr;cu(cudaStreamCreateWithFlags(&stream,cudaStreamNonBlocking));
        auto candidate=[&](){cu(aster_topk35_run(static_cast<const float*>(input->data),result,cols,rows,k,packed,selected,sorted,scratch,bytes,stream));};
        auto get_candidate=[&](){cu(cudaStreamSynchronize(stream));cu(cudaMemcpy(actual.data(),result,nout*sizeof(int),cudaMemcpyDeviceToHost));};
        set(values);original();get_reference();candidate();get_candidate();if(!same(reference,actual,cols,rows,pattern,"uncaptured"))return 3;
        cudaGraph_t captured=nullptr;cudaGraphExec_t executable=nullptr;
        cu(cudaStreamBeginCapture(stream,cudaStreamCaptureModeThreadLocal));candidate();cu(cudaStreamEndCapture(stream,&captured));cu(cudaGraphInstantiateWithFlags(&executable,captured,0));
        for(int i=0;i<3;++i){cu(cudaGraphLaunch(executable,stream));get_candidate();if(!same(reference,actual,cols,rows,pattern,"graph_replay"))return 3;}
        std::vector<float> changed(n,std::bit_cast<float>(pattern==3?0x80000000u:0xff800000u));
        set(changed);original();get_reference();cu(cudaGraphLaunch(executable,stream));get_candidate();if(!same(reference,actual,cols,rows,pattern,"graph_input_change"))return 3;
        set(values);original();get_reference();cu(cudaGraphLaunch(executable,stream));get_candidate();if(!same(reference,actual,cols,rows,pattern,"restored_input"))return 3;
        double a[5]{},b[5]{},c[5]{};
        for(int i=0;i<5;++i){
            double start=now();original();a[i]=now()-start;
            start=now();candidate();cu(cudaStreamSynchronize(stream));b[i]=now()-start;
            start=now();cu(cudaGraphLaunch(executable,stream));cu(cudaStreamSynchronize(stream));c[i]=now()-start;
        }
        get_candidate();if(!same(reference,actual,cols,rows,pattern,"after_measured_pairs"))return 3;
        auto mean=[](const double* data){double sum=0;for(int i=0;i<5;++i)sum+=data[i];return sum/5;};
        std::printf("{\"kind\":\"case\",\"phase\":\"passed\",\"cols\":%d,\"rows\":%d,\"k\":512,\"pattern\":%d,\"all_IDs_equal\":true,\"graph_replays\":3,\"graph_changed_input_exact\":true,\"scratch_bytes\":%zu,\"reference_host_sync_us\":%.6f,\"candidate_host_sync_us\":%.6f,\"candidate_graph_host_sync_us\":%.6f,\"model_speed_claimed\":false}\n",cols,rows,pattern,bytes,mean(a),mean(b),mean(c));std::fflush(stdout);++cases;
        cu(cudaGraphExecDestroy(executable));cu(cudaGraphDestroy(captured));cu(cudaStreamDestroy(stream));
        cu(cudaFree(scratch));cu(cudaFree(result));cu(cudaFree(sorted));cu(cudaFree(selected));cu(cudaFree(packed));
        ggml_backend_buffer_free(buffer);ggml_free(ctx);
    }
    ggml_backend_free(backend);require(cases==38,"exact38 shape/pattern gates required");
    std::printf("{\"kind\":\"summary\",\"phase\":\"passed\",\"cases\":%d,\"GPU_executed\":true,\"model_forward_executed\":false,\"api_speed_claimed\":false}\n",cases);
}
