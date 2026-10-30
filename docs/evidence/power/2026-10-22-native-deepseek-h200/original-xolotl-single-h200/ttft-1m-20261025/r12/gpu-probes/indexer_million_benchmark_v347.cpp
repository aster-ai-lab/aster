// Same-binary million-row operator A/B/A. This measures no model tokens.
#include "ggml.h"
#include "ggml-backend.h"
#include "ggml-cuda.h"
#include <algorithm>
#include <chrono>
#include <cstdio>
#include <cstdlib>
#include <fstream>
#include <vector>
static void fail(const char* s){std::fprintf(stderr,"FAIL %s\n",s);std::exit(1);}
static uint32_t next(uint32_t& x){x^=x<<13;x^=x>>17;x^=x<<5;return x;}
int main(int argc,char** argv){
 if(argc!=2)return 2;
 auto backend=ggml_backend_cuda_init(0);if(!backend)fail("CUDA backend");
 std::ofstream output(argv[1],std::ios::binary);if(!output)fail("score output");
 int cases=0;
 for(int heads:{32,64})for(int nq:{1,6})for(int masked:{0,50,90}){
  constexpr int dim=128,nk=1048576;
  auto ctx=ggml_init({2*1024*1024,nullptr,true});if(!ctx)fail("graph metadata");
  auto Q=ggml_new_tensor_4d(ctx,GGML_TYPE_F32,dim,heads,nq,1);
  auto K=ggml_new_tensor_4d(ctx,GGML_TYPE_MXFP4,dim,1,nk,1);
  auto W=ggml_new_tensor_4d(ctx,GGML_TYPE_F32,heads,nq,1,1);
  auto M=ggml_new_tensor_4d(ctx,GGML_TYPE_F16,nk,nq,1,1);
  auto scores=ggml_lightning_indexer(ctx,Q,K,W,M);
  auto graph=ggml_new_graph_custom(ctx,128,false);ggml_build_forward_expand(graph,scores);
  auto buffer=ggml_backend_alloc_ctx_tensors(ctx,backend);if(!buffer)fail("GPU tensors");
  uint32_t rng=0x8d713e29;
  std::vector<float> q(dim*heads*nq),w(heads*nq),got(size_t(nk)*nq);
  for(auto& f:q)f=(int(next(rng)%2049)-1024)/2048.f;
  for(auto& f:w)f=(next(rng)%257)/2048.f;
  std::vector<unsigned char> key(ggml_nbytes(K));
  if(key.size()!=size_t(nk)*4*17)fail("actual MXFP4 key layout");
  for(size_t at=0;at<key.size();at+=17){key[at]=122+next(rng)%6;for(int i=1;i<17;++i)key[at+i]=next(rng)&255;}
  std::vector<uint16_t> mask(size_t(nk)*nq);
  const int boundary=int(int64_t(nk)*(100-masked)/100);
  for(int t=0;t<nq;++t)for(int r=0;r<nk;++r)mask[size_t(t)*nk+r]=r<boundary?0:0xfc00;
  ggml_backend_tensor_set(Q,q.data(),0,q.size()*4);ggml_backend_tensor_set(K,key.data(),0,key.size());
  ggml_backend_tensor_set(W,w.data(),0,w.size()*4);ggml_backend_tensor_set(M,mask.data(),0,mask.size()*2);
  for(int i=0;i<6;++i)if(ggml_backend_graph_compute(backend,graph)!=GGML_STATUS_SUCCESS)fail("warmup compute");
  ggml_backend_synchronize(backend);
  std::vector<double> times;
  for(int round=0;round<5;++round){
   auto start=std::chrono::steady_clock::now();
   for(int repeat=0;repeat<64;++repeat)if(ggml_backend_graph_compute(backend,graph)!=GGML_STATUS_SUCCESS)fail("timed compute");
   ggml_backend_synchronize(backend);
   double ms=std::chrono::duration<double,std::milli>(std::chrono::steady_clock::now()-start).count()/64;
   times.push_back(ms);
  }
  ggml_backend_tensor_get(scores,got.data(),0,got.size()*4);
  uint64_t count=got.size();output.write(reinterpret_cast<const char*>(&count),sizeof(count));output.write(reinterpret_cast<const char*>(got.data()),got.size()*4);if(!output)fail("complete score write");
  std::printf("{\"case\":%d,\"rows\":1048576,\"heads\":%d,\"queries\":%d,\"masked_percent\":%d,\"wall_ms_per_graph\":[",cases,heads,nq,masked);
  for(size_t i=0;i<times.size();++i)std::printf("%s%.9f",i?",":"",times[i]);std::puts("]}");std::fflush(stdout);
  ggml_backend_buffer_free(buffer);ggml_free(ctx);++cases;
 }
 ggml_backend_free(backend);std::printf("PASS million-row operator benchmark cases: %d\n",cases);
}
