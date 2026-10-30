// Isolate graph identity: the same graph, addresses, dimensions and byte strides, but F16/BF16 keys.
#include "ggml.h"
#include "ggml-backend.h"
#include "ggml-cuda.h"
#include <algorithm>
#include <cmath>
#include <cstdio>
#include <fstream>
#include <vector>
int main(int argc,char** argv) {
    if(argc!=2)return 2;
    constexpr int dim=128,heads=32,nk=65;
    auto backend=ggml_backend_cuda_init(0);if(!backend)return 2;
    auto ctx=ggml_init({2*1024*1024,nullptr,true});if(!ctx)return 2;
    auto Q=ggml_new_tensor_4d(ctx,GGML_TYPE_F32,dim,heads,1,1);
    auto K=ggml_new_tensor_4d(ctx,GGML_TYPE_F16,dim,1,nk,1);
    auto W=ggml_new_tensor_4d(ctx,GGML_TYPE_F32,heads,1,1,1);
    auto M=ggml_new_tensor_4d(ctx,GGML_TYPE_F16,nk,1,1,1);
    auto S=ggml_lightning_indexer(ctx,Q,K,W,M);
    auto graph=ggml_new_graph_custom(ctx,128,false);ggml_build_forward_expand(graph,S);
    auto buffer=ggml_backend_alloc_ctx_tensors(ctx,backend);if(!buffer)return 2;
    std::vector<float> q(dim*heads,.125f),k(dim*nk,1.f),w(heads,1.f/heads),got(nk);
    std::vector<unsigned char> packed(ggml_nbytes(K)),mask(ggml_nbytes(M),0);
    ggml_backend_tensor_set(Q,q.data(),0,q.size()*4);ggml_backend_tensor_set(W,w.data(),0,w.size()*4);ggml_backend_tensor_set(M,mask.data(),0,mask.size());
    std::ofstream out(argv[1],std::ios::binary);if(!out)return 2;
    int mismatches=0,runs=0;
    for(auto type:{GGML_TYPE_F16,GGML_TYPE_BF16,GGML_TYPE_F16,GGML_TYPE_BF16}) {
        K->type=type;
        if(ggml_quantize_chunk(type,k.data(),packed.data(),0,nk,dim,nullptr)!=packed.size())return 2;
        ggml_backend_tensor_set(K,packed.data(),0,packed.size());
        for(int repeat=0;repeat<3;++repeat) {
            if(ggml_backend_graph_compute(backend,graph)!=GGML_STATUS_SUCCESS)return 2;
            ggml_backend_tensor_get(S,got.data(),0,got.size()*4);
            for(float f:got)if(f!=16.f)++mismatches;
            out.write(reinterpret_cast<const char*>(got.data()),got.size()*4);
            std::printf("TYPE %d REPEAT %d FIRST %.9g EXPECT16\n",int(type),repeat,got[0]);++runs;
        }
    }
    ggml_backend_buffer_free(buffer);ggml_free(ctx);ggml_backend_free(backend);
    std::printf("RESULT runs%d values%d mismatches%d\n",runs,runs*nk,mismatches);
    return mismatches?3:0;
}
