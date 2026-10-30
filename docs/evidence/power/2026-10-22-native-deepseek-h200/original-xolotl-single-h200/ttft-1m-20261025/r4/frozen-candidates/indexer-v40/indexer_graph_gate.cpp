// Full-score identity gate: original post-mask graph versus pre-mask graph,
// with the real ggml CUDA indexer, every supported key type and odd row counts.
#include "ggml.h"
#include "ggml-backend.h"
#include "ggml-cuda.h"
#include <algorithm>
#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <fstream>
#include <random>
#include <string>
#include <vector>

static void fail(const char* text) {std::fprintf(stderr,"FAIL %s\n",text);std::exit(1);}
int main(int argc,char** argv) {
    if(argc!=3||(std::string(argv[2])!="0"&&std::string(argv[2])!="1")) return 2;
    const bool mask_first=std::string(argv[2])=="1";
    std::ofstream output(argv[1],std::ios::binary|std::ios::out|std::ios::trunc);
    if(!output)fail("bounded score output could not open");
    ggml_backend_t backend=ggml_backend_cuda_init(0);
    if(!backend)fail("one CUDA backend required");
    std::mt19937 rng(7729);
    std::uniform_real_distribution<float> random(-1.f,1.f);
    int cases=0;
    for(int heads : {32,64}) for(ggml_type type : {GGML_TYPE_F32,GGML_TYPE_F16,GGML_TYPE_BF16,GGML_TYPE_Q4_0,GGML_TYPE_Q4_1,GGML_TYPE_Q5_0,GGML_TYPE_Q5_1,GGML_TYPE_Q8_0,GGML_TYPE_MXFP4})
        for(int pattern=0;pattern<5;++pattern) {
            constexpr int dim=128,nk=257,nq=3,streams=2;
            const int mask_streams=pattern%2?1:streams;
            ggml_context* ctx=ggml_init({2*1024*1024,nullptr,true});
            if(!ctx)fail("graph metadata allocation");
            auto* Q=ggml_new_tensor_4d(ctx,GGML_TYPE_F32,dim,heads,nq,streams);
            auto* K=ggml_new_tensor_4d(ctx,type,dim,1,nk,streams);
            auto* W=ggml_new_tensor_4d(ctx,GGML_TYPE_F32,heads,nq,1,streams);
            auto* V=ggml_new_tensor_4d(ctx,GGML_TYPE_F32,nk,nq,1,mask_streams);
            auto* C=ggml_new_tensor_4d(ctx,GGML_TYPE_F32,nk,nq,1,mask_streams);
            ggml_tensor* mask=mask_first?ggml_add(ctx,V,C):V;
            auto* scores=ggml_lightning_indexer(ctx,Q,K,W,ggml_cast(ctx,mask,GGML_TYPE_F16));
            if(!mask_first)scores=ggml_add(ctx,scores,C);
            ggml_cgraph* graph=ggml_new_graph_custom(ctx,128,false);ggml_build_forward_expand(graph,scores);
            auto buffer=ggml_backend_alloc_ctx_tensors(ctx,backend);
            if(!buffer)fail("graph tensor allocation");
            std::vector<float> q(ggml_nelements(Q)),k(ggml_nelements(K)),w(ggml_nelements(W)),v(ggml_nelements(V)),c(v.size()),got(ggml_nelements(scores));
            for(float& x:q)x=random(rng);
            for(float& x:k)x=random(rng);
            for(float& x:w)x=std::abs(random(rng))*.125f;
            const float negative=-INFINITY;
            for(int s=0;s<mask_streams;++s) for(int t=0;t<nq;++t) for(int r=0;r<nk;++r) {
                const int at=(s*nq+t)*nk+r;
                v[at]=r<201+t*17?0.f:negative;
                c[at]=0.f;
                if(pattern==1)c[at]=negative;
                else if(pattern==2)c[at]=((r/64+t+s)%4==0)?0.f:negative;
                else if(pattern==3)c[at]=(r%2)?0.f:negative;
                else if(pattern==4)c[at]=(r%3)?-.5f:0.f; // finite bias must use original work, never skip
            }
            std::vector<unsigned char> packed(ggml_nbytes(K));
            const size_t bytes=ggml_quantize_chunk(type,k.data(),packed.data(),0,nk*streams,dim,nullptr);
            if(bytes!=packed.size())fail("exact key quantization size");
            ggml_backend_tensor_set(Q,q.data(),0,q.size()*4);ggml_backend_tensor_set(K,packed.data(),0,packed.size());
            ggml_backend_tensor_set(W,w.data(),0,w.size()*4);ggml_backend_tensor_set(V,v.data(),0,v.size()*4);ggml_backend_tensor_set(C,c.data(),0,c.size()*4);
            if(ggml_backend_graph_compute(backend,graph)!=GGML_STATUS_SUCCESS)fail("indexer graph compute");
            ggml_backend_tensor_get(scores,got.data(),0,got.size()*4);
            const uint32_t count=static_cast<uint32_t>(got.size());
            output.write(reinterpret_cast<const char*>(&count),sizeof(count));output.write(reinterpret_cast<const char*>(got.data()),got.size()*4);
            if(!output)fail("finite score output write");
            std::printf("CASE %d heads%d type%d pattern%d rows%u\n",cases,heads,static_cast<int>(type),pattern,count);
            ggml_backend_buffer_free(buffer);ggml_free(ctx);++cases;
        }
    ggml_backend_free(backend);
    std::printf("PASS actual CUDA indexer score cases: %d\n",cases);
}
