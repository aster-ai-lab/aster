// Synthetic CUDA gate for the exact helper used by the production candidate.
#include "common.cuh"
#include "fattn-common.cuh"
#include "aster_indexer_mask_skip.cuh"
#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <vector>
#include <limits>

static void ck(cudaError_t e) { if(e!=cudaSuccess) { std::fprintf(stderr,"CUDA: %s\n",cudaGetErrorString(e)); std::exit(1); } }
template<class T> static T* alloc(size_t n) { T* p=nullptr; ck(cudaMalloc(reinterpret_cast<void**>(&p),n*sizeof(T)));return p; }

template<int HEAD,bool WMMA,ggml_type TK>
__global__ void probe(const float* q,const char* k,const float* w,const half* m,float* dst,int* flags,int nk,size_t key_stride) {
    constexpr int KV=WMMA?32:64;
    const bool skipped=aster_indexer_masked_block<WMMA,KV,128,HEAD,TK>(
        q,k,w,m,dst,blockIdx.x*KV,nk,0,0,1,nk*4,nk*4,128*4,HEAD*128*4,HEAD*128*4,
        key_stride,nk*key_stride,HEAD*4,HEAD*4,nk*2,nk*2);
    if(threadIdx.x==0&&threadIdx.y==0) flags[blockIdx.x]=skipped;
}

template<int HEAD,bool WMMA> static int suite() {
    constexpr int KV=WMMA?32:64;
    constexpr ggml_type TK=WMMA?GGML_TYPE_F16:GGML_TYPE_F32;
    int count=0;
    for(int nk : {33,64,65}) for(int scenario=0;scenario<12;++scenario) {
        std::vector<float> q(HEAD*128,.25f),k(nk*128,.5f),w(HEAD,.125f),out(nk,7.f);
        std::vector<half> masks(nk,__float2half(-std::numeric_limits<float>::infinity()));
        switch(scenario) {
            case 1:masks.back()=__float2half(0.f);break;
            case 2:masks.back()=__float2half(-.5f);break;
            case 3:masks.back()=__float2half(std::numeric_limits<float>::infinity());break;
            case 4:masks.back()=__float2half(std::numeric_limits<float>::quiet_NaN());break;
            case 5:q[0]=std::numeric_limits<float>::quiet_NaN();break;
            case 6:w.back()=std::numeric_limits<float>::infinity();break;
            case 7:k.back()=std::numeric_limits<float>::quiet_NaN();break;
            case 8:q.back()=1025.f;break;
            case 9:k.back()=1025.f;break;
            case 10:w[0]=-1025.f;break;
            case 11:std::fill(q.begin(),q.end(),1024.f);std::fill(k.begin(),k.end(),1024.f);std::fill(w.begin(),w.end(),1024.f);break;
        }
        const int blocks=(nk+KV-1)/KV;
        std::vector<int> got(blocks,-1),want(blocks,1);
        std::vector<half> kh(k.size());
        if(WMMA) for(size_t i=0;i<k.size();++i) kh[i]=__float2half(k[i]);
        bool qw=true;
        for(float v:q) qw&=aster_indexer::bounded(v);
        for(float v:w) qw&=aster_indexer::bounded(v);
        for(int b=0;b<blocks;++b) {
            bool ok=qw;
            for(int r=b*KV;r<std::min(nk,(b+1)*KV);++r) {
                uint16_t bits=0;std::memcpy(&bits,&masks[r],2);ok&=bits==0xfc00u;
                for(int d=0;d<128;++d) ok&=aster_indexer::bounded(WMMA?__half2float(kh[r*128+d]):k[r*128+d]);
            }
            want[b]=ok;
        }
        float* qd=alloc<float>(q.size());float* wd=alloc<float>(w.size());float* od=alloc<float>(out.size());
        half* md=alloc<half>(masks.size());int* fd=alloc<int>(got.size());
        const size_t key_bytes=k.size()*(WMMA?sizeof(half):sizeof(float));char* kd=alloc<char>(key_bytes);
        ck(cudaMemcpy(qd,q.data(),q.size()*4,cudaMemcpyHostToDevice));ck(cudaMemcpy(wd,w.data(),w.size()*4,cudaMemcpyHostToDevice));
        ck(cudaMemcpy(md,masks.data(),masks.size()*2,cudaMemcpyHostToDevice));ck(cudaMemcpy(od,out.data(),out.size()*4,cudaMemcpyHostToDevice));
        ck(cudaMemcpy(kd,WMMA?static_cast<const void*>(kh.data()):static_cast<const void*>(k.data()),key_bytes,cudaMemcpyHostToDevice));
        probe<HEAD,WMMA,TK><<<blocks,dim3(32,8)>>>(qd,kd,wd,md,od,fd,nk,128*(WMMA?2:4));
        ck(cudaGetLastError());ck(cudaDeviceSynchronize());
        ck(cudaMemcpy(got.data(),fd,got.size()*sizeof(int),cudaMemcpyDeviceToHost));ck(cudaMemcpy(out.data(),od,out.size()*4,cudaMemcpyDeviceToHost));
        if(got!=want) {std::fprintf(stderr,"FAIL eligibility head%d wmma%d nk%d scenario%d\n",HEAD,WMMA,nk,scenario);std::exit(1);}
        for(int r=0;r<nk;++r) {
            const bool ok=want[r/KV]?(std::isinf(out[r])&&std::signbit(out[r])):out[r]==7.f;
            if(!ok) {std::fprintf(stderr,"FAIL masked write/fallback head%d scenario%d row%d\n",HEAD,scenario,r);std::exit(1);}
        }
        ck(cudaFree(qd));ck(cudaFree(wd));ck(cudaFree(od));ck(cudaFree(md));ck(cudaFree(fd));ck(cudaFree(kd));++count;
    }
    return count;
}
int main() {
    int n=0;ck(cudaGetDeviceCount(&n));if(n!=1)return 2;
    const int cases=suite<32,false>()+suite<64,false>()+suite<32,true>()+suite<64,true>();
    std::printf("PASS exact masked-block CUDA eligibility/write/fallback: %d cases\n",cases);
}
