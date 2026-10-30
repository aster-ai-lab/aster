// Exhaustive actual MXFP4 decode bounds and the exact candidate helper, including CUDA graph replay.
#include "common.cuh"
#include "fattn-common.cuh"
#include "aster_indexer_mask_skip.cuh"
#include <algorithm>
#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <limits>
#include <vector>

static void ck(cudaError_t e) { if(e!=cudaSuccess) {std::fprintf(stderr,"CUDA %s\n",cudaGetErrorString(e));std::exit(1);} }
static void fail(const char* s) {std::fprintf(stderr,"FAIL %s\n",s);std::exit(1);}
template<class T> static T* alloc(size_t n) {T* p=nullptr;ck(cudaMalloc(reinterpret_cast<void**>(&p),n*sizeof(T)));return p;}
__global__ void all_values(float* f,float* h,int* header) {
    int at=blockIdx.x*blockDim.x+threadIdx.x;if(at>=4096)return;
    block_mxfp4 b;b.e=at/16;const unsigned char nib=at%16;
    for(int i=0;i<16;++i)b.qs[i]=nib|(nib<<4);
    __align__(16) float v[4];__align__(16) half w[4];
    get_dequantize_V<GGML_TYPE_MXFP4,float,4>()(&b,v,0);
    get_dequantize_V<GGML_TYPE_MXFP4,half,4>()(&b,w,0);
    f[at]=v[0];h[at]=__half2float(w[0]);header[at]=aster_indexer_scale::bounded_mxfp4_header(b.e);
}
template<int HEAD,bool WMMA,bool HEADER>
__global__ void helper(const float* q,const char* k,const float* w,const half* m,float* dst,int* flags,int nk) {
    constexpr int KV=WMMA?32:64;constexpr size_t stride=4*sizeof(block_mxfp4);
    bool skipped=aster_indexer_masked_block<WMMA,KV,128,HEAD,GGML_TYPE_MXFP4,HEADER>(
        q,k,w,m,dst,blockIdx.x*KV,nk,1,1,2,nk*4,2*nk*4,128*4,HEAD*128*4,2*HEAD*128*4,
        stride,nk*stride,HEAD*4,2*HEAD*4,nk*2,2*nk*2);
    if(threadIdx.x==0&&threadIdx.y==0)flags[blockIdx.x]=skipped;
}
static bool bounded(float f) {return std::isfinite(f)&&std::abs(f)<=1024.f;}
static bool old_key_ok(const block_mxfp4& b) {
    static const int values[16]={0,1,2,3,4,6,8,12,0,-1,-2,-3,-4,-6,-8,-12};
    const double d=b.e==255?std::numeric_limits<double>::infinity():std::ldexp(.5,b.e==0?-128:int(b.e)-127);
    for(int i=0;i<16;++i)for(int n:{int(b.qs[i]&15),int(b.qs[i]>>4)})
        if(!bounded(static_cast<float>(d*values[n])))return false;
    return true;
}
template<int HEAD,bool WMMA,bool HEADER> static int suite() {
    constexpr int KV=WMMA?32:64;int cases=0;
    for(int nk:{33,64,65})for(int scenario=0;scenario<16;++scenario) {
        std::vector<float> q(4*HEAD*128,std::numeric_limits<float>::quiet_NaN()),w(4*HEAD,std::numeric_limits<float>::quiet_NaN()),out(4*nk,7.f);
        std::fill(q.begin()+3*HEAD*128,q.end(),.25f);std::fill(w.begin()+3*HEAD,w.end(),.125f);
        std::vector<half> masks(4*nk,__float2half(0.f));std::fill(masks.begin()+3*nk,masks.end(),__float2half(-INFINITY));
        std::vector<block_mxfp4> key(2*nk*4);
        for(auto& b:key){b.e=255;std::memset(b.qs,0x77,16);}
        for(size_t i=nk*4;i<key.size();++i){key[i].e=127;std::memset(key[i].qs,0x22,16);}
        auto& tail=key.back();
        switch(scenario) {
            case 1:masks.back()=__float2half(0.f);break;
            case 2:masks.back()=__float2half(-.5f);break;
            case 3:masks.back()=__float2half(INFINITY);break;
            case 4:masks.back()=__float2half(std::numeric_limits<float>::quiet_NaN());break;
            case 5:q[3*HEAD*128]=std::numeric_limits<float>::quiet_NaN();break;
            case 6:w.back()=INFINITY;break;
            case 7:tail.e=255;std::memset(tail.qs,0,16);break;
            case 8:q.back()=1025.f;break;
            case 9:tail.e=135;std::memset(tail.qs,0x77,16);break;
            case 10:w.back()=-1025.f;break;
            case 11:std::fill(q.begin()+3*HEAD*128,q.end(),1024.f);std::fill(w.begin()+3*HEAD,w.end(),1024.f);for(size_t i=nk*4;i<key.size();++i){key[i].e=134;std::memset(key[i].qs,0x77,16);}break;
            case 12:tail.e=134;std::memset(tail.qs,0xff,16);break;
            case 13:tail.e=135;std::memset(tail.qs,0,16);break;
            case 14:tail.e=0;std::memset(tail.qs,0xff,16);break;
            case 15:tail.e=254;std::memset(tail.qs,0x88,16);break;
        }
        const auto initial_masks=masks;const auto initial_key=key;
        int blocks=(nk+KV-1)/KV;std::vector<int> got(blocks,-1),want(blocks);
        float* qd=alloc<float>(q.size());float* wd=alloc<float>(w.size());float* od=alloc<float>(out.size());
        half* md=alloc<half>(masks.size());int* fd=alloc<int>(blocks);char* kd=alloc<char>(key.size()*sizeof(block_mxfp4));
        ck(cudaMemcpy(qd,q.data(),q.size()*4,cudaMemcpyHostToDevice));ck(cudaMemcpy(wd,w.data(),w.size()*4,cudaMemcpyHostToDevice));
        cudaStream_t stream;ck(cudaStreamCreate(&stream));cudaGraph_t graph;cudaGraphExec_t exec;
        ck(cudaStreamBeginCapture(stream,cudaStreamCaptureModeThreadLocal));
        helper<HEAD,WMMA,HEADER><<<blocks,dim3(32,8),0,stream>>>(qd,kd,wd,md,od,fd,nk);
        ck(cudaStreamEndCapture(stream,&graph));ck(cudaGraphInstantiate(&exec,graph,nullptr,nullptr,0));
        for(int pass=0;pass<3;++pass) {
            masks=initial_masks;key=initial_key;
            if(pass==1)masks.back()=__float2half(0.f);
            if(pass==2){masks.back()=__float2half(-INFINITY);key.back().e=135;std::memset(key.back().qs,0x77,16);}
            bool qw=true;for(size_t i=3*HEAD*128;i<q.size();++i)qw&=bounded(q[i]);for(size_t i=3*HEAD;i<w.size();++i)qw&=bounded(w[i]);
            for(int b=0;b<blocks;++b) {
                bool ok=qw;
                for(int r=b*KV;r<std::min(nk,(b+1)*KV);++r) {
                    uint16_t bits;std::memcpy(&bits,&masks[3*nk+r],2);ok&=bits==0xfc00u;
                    for(int block=0;block<4;++block) {const auto& v=key[nk*4+r*4+block];ok&=HEADER?(v.e<=134):old_key_ok(v);}
                }
                want[b]=ok;
            }
            std::fill(out.begin(),out.end(),7.f);
            ck(cudaMemcpy(kd,key.data(),key.size()*sizeof(block_mxfp4),cudaMemcpyHostToDevice));ck(cudaMemcpy(md,masks.data(),masks.size()*2,cudaMemcpyHostToDevice));ck(cudaMemcpy(od,out.data(),out.size()*4,cudaMemcpyHostToDevice));
            if(pass==0)helper<HEAD,WMMA,HEADER><<<blocks,dim3(32,8),0,stream>>>(qd,kd,wd,md,od,fd,nk);
            else ck(cudaGraphLaunch(exec,stream));
            ck(cudaGetLastError());ck(cudaStreamSynchronize(stream));
            ck(cudaMemcpy(got.data(),fd,blocks*sizeof(int),cudaMemcpyDeviceToHost));ck(cudaMemcpy(out.data(),od,out.size()*4,cudaMemcpyDeviceToHost));
            if(got!=want){std::fprintf(stderr,"FAIL eligibility heads%d wmma%d header%d nk%d scenario%d pass%d\n",HEAD,WMMA,HEADER,nk,scenario,pass);std::exit(1);}
            for(int r=0;r<4*nk;++r){bool selected=r>=3*nk;bool skip=selected&&want[(r-3*nk)/KV];if(skip?!(std::isinf(out[r])&&std::signbit(out[r])):out[r]!=7.f)fail("masked writes or stream/batch isolation");}
            ++cases;
        }
        ck(cudaGraphExecDestroy(exec));ck(cudaGraphDestroy(graph));ck(cudaStreamDestroy(stream));
        ck(cudaFree(qd));ck(cudaFree(wd));ck(cudaFree(od));ck(cudaFree(md));ck(cudaFree(fd));ck(cudaFree(kd));
    }
    return cases;
}
int main() {
    int devices=0;ck(cudaGetDeviceCount(&devices));if(devices!=1)fail("exact single visible CUDA device required");
    float *fd=alloc<float>(4096),*hd=alloc<float>(4096);int* ad=alloc<int>(4096);
    all_values<<<16,256>>>(fd,hd,ad);ck(cudaGetLastError());ck(cudaDeviceSynchronize());
    std::vector<float> f(4096),h(4096);std::vector<int>a(4096);
    ck(cudaMemcpy(f.data(),fd,4096*4,cudaMemcpyDeviceToHost));ck(cudaMemcpy(h.data(),hd,4096*4,cudaMemcpyDeviceToHost));ck(cudaMemcpy(a.data(),ad,4096*4,cudaMemcpyDeviceToHost));
    int admitted=0;float maximum=0;
    for(int i=0;i<4096;++i){if(a[i]!=(i/16<=134))fail("exhaustive GPU header predicate");if(a[i]){if(!bounded(f[i])||!bounded(h[i])||std::abs(f[i])>768||std::abs(h[i])>768)fail("actual float/half MXFP4 bound");maximum=std::max(maximum,std::abs(f[i]));++admitted;}}
    if(admitted!=2160||maximum!=768.f||f[135*16+7]!=1536.f||h[135*16+7]!=1536.f)fail("safe maximum or unsafe135 negative control");
    ck(cudaFree(fd));ck(cudaFree(hd));ck(cudaFree(ad));
    int cases=suite<32,false,false>()+suite<64,false,false>()+suite<32,true,false>()+suite<64,true,false>()+
              suite<32,false,true>()+suite<64,false,true>()+suite<32,true,true>()+suite<64,true,true>();
    std::printf("PASS MXFP4 actual CUDA bounds: 4096 float + 4096 half; admitted2160; max768; unsafe135=1536\n");
    std::printf("PASS MXFP4 eligibility/write/strides/changed-input graph cases: %d\n",cases);
}
