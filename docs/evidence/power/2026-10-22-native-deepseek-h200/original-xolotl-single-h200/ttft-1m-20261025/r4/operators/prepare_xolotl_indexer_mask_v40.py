"""Freeze an exact-mask candidate from the deployed sources, with bounded math eligibility."""
import difflib
import hashlib
import json
from pathlib import Path

B=Path('D:/code/aster/target/h200-native')
R=B/'indexer-v40'
provenance=json.loads((B/'xolotl-indexer-v40-parent-provenance.json').read_text())
for row in provenance['files']:
    raw=(R/row['name']).read_bytes()
    if len(raw)!=row['bytes'] or hashlib.sha256(raw).hexdigest()!=row['sha256']:raise RuntimeError('exact deployed source snapshot differs')
if (R/'source-manifest.json').exists():raise RuntimeError('V40 candidate sources already frozen')

policy=r'''// Closed experimental switch and a conservative no-overflow contract.
#pragma once
#include <cstring>
#include <cstdlib>
namespace aster_indexer {
enum class Mode { off, on, invalid };
inline Mode parse(const char* text) {
    if (!text || std::strcmp(text, "0") == 0) return Mode::off;
    if (std::strcmp(text, "1") == 0) return Mode::on;
    return Mode::invalid;
}
constexpr float max_abs = 1024.0f;
// 128 products, at most64 heads and weights bounded by1024 cannot overflow
// an F32 accumulator. WMMA checks its actual half-dequantized K values too.
#if defined(__CUDACC__)
__host__ __device__
#endif
inline bool bounded(float value) { return value >= -max_abs && value <= max_abs; }
} // namespace aster_indexer
'''
(R/'aster_indexer_mask_policy.hpp').write_text(policy,newline='\n')
helper=r'''// All threads take the same returns. No extra allocation or approximation.
#pragma once
#include "aster_indexer_mask_policy.hpp"

template<bool WMMA, int KV, int EMB, int HEAD, ggml_type TYPE_K>
static __device__ __forceinline__ bool aster_indexer_masked_block(
        const float* Q, const char* K, const float* W, const half* M, float* dst,
        int start, int n_kv, int i_batch, int i_stream, int nem3,
        size_t nb1, size_t nb3, size_t nbq1, size_t nbq2, size_t nbq3,
        size_t nbk2, size_t nbk3, size_t nbw1, size_t nbw3, size_t nbm1, size_t nbm3) {
    const int tid = threadIdx.y * WARP_SIZE + threadIdx.x;
    const int nt = blockDim.x * blockDim.y;
    __shared__ int eligible;
    if (tid == 0) eligible = 1;
    __syncthreads();
    const half* mask = reinterpret_cast<const half*>(reinterpret_cast<const char*>(M) + i_batch*nbm1 + (i_stream%nem3)*nbm3);
    bool ok = true;
    for (int k = tid; k < KV; k += nt)
        if (start+k < n_kv && __half_as_ushort(mask[start+k]) != 0xfc00u) ok = false;
    if (!ok) atomicExch(&eligible, 0);
    __syncthreads();
    if (!eligible) return false;

    // Masked output is exactly -inf only if the original dot/weighted sum
    // stays finite. Unsafe/NaN/Inf/out-of-range inputs run the original code.
    const char* q = reinterpret_cast<const char*>(Q) + i_batch*nbq2 + i_stream*nbq3;
    const float* w = reinterpret_cast<const float*>(reinterpret_cast<const char*>(W) + i_batch*nbw1 + i_stream*nbw3);
    for (int h = tid; h < HEAD; h += nt) ok &= aster_indexer::bounded(w[h]);
    for (int at = tid; at < HEAD*(EMB/4); at += nt) {
        const int h = at/(EMB/4), d = at%(EMB/4);
        const float4 v = reinterpret_cast<const float4*>(q + h*nbq1)[d];
        ok &= aster_indexer::bounded(v.x) && aster_indexer::bounded(v.y) &&
              aster_indexer::bounded(v.z) && aster_indexer::bounded(v.w);
    }
    for (int at = tid; at < KV*(EMB/4); at += nt) {
        const int k = at/(EMB/4), d = at%(EMB/4);
        if (start+k >= n_kv) continue;
        const void* key = K + (start+k)*nbk2 + i_stream*nbk3;
        if constexpr (WMMA) {
            __align__(16) half v[4];
            constexpr dequantize_V_t read = get_dequantize_V<TYPE_K, half, 4>();
            read(key, v, d*4);
            for (int i=0; i<4; ++i) ok &= aster_indexer::bounded(__half2float(v[i]));
        } else if constexpr (TYPE_K == GGML_TYPE_F32) {
            const float4 v = reinterpret_cast<const float4*>(key)[d];
            ok &= aster_indexer::bounded(v.x) && aster_indexer::bounded(v.y) &&
                  aster_indexer::bounded(v.z) && aster_indexer::bounded(v.w);
        } else {
            __align__(16) float v[4];
            constexpr dequantize_V_t read = get_dequantize_V<TYPE_K, float, 4>();
            read(key, v, d*4);
            for (int i=0; i<4; ++i) ok &= aster_indexer::bounded(v[i]);
        }
    }
    if (!ok) atomicExch(&eligible, 0);
    __syncthreads();
    if (!eligible) return false;
    float* output = reinterpret_cast<float*>(reinterpret_cast<char*>(dst) + i_batch*nb1 + i_stream*nb3);
    for (int k=tid; k<KV; k+=nt)
        if (start+k < n_kv) output[start+k] = __int_as_float(static_cast<int>(0xff800000u));
    return true;
}
'''
(R/'aster_indexer_mask_skip.cuh').write_text(helper,newline='\n')
dense=(R/'ds41_dense.original.cpp').read_text()
original_dense=dense
if dense.count('#include "ds41_dense.hpp"')!=1:raise RuntimeError('exact dense include differs')
dense=dense.replace('#include "ds41_dense.hpp"','#include "ds41_dense.hpp"\n#include "aster_indexer_mask_policy.hpp"')
anchor='    int64_t idx_tile = 0;'
if dense.count(anchor)!=1:raise RuntimeError('exact indexer state differs')
dense=dense.replace(anchor,anchor+'\n    bool indexer_mask_first = false;')
anchor='    Impl & im = *p_;\n    im.cfg = cfg;'
if dense.count(anchor)!=1:raise RuntimeError('exact dense init differs')
dense=dense.replace(anchor,anchor+'''
    const auto mask_mode = aster_indexer::parse(std::getenv("ASTER_XQ_INDEXER_MASK_SKIP"));
    if (mask_mode == aster_indexer::Mode::invalid) {
        err = "ASTER_XQ_INDEXER_MASK_SKIP must be 0 or 1";
        return false;
    }
    im.indexer_mask_first = mask_mode == aster_indexer::Mode::on;
''')
score='                        ggml_tensor * sc = ggml_lightning_indexer(gc, iq_c, ksrc, iw_c, ggml_cast(gc, vis, GGML_TYPE_F16));'
if dense.count(score)!=2:raise RuntimeError('two exact row/single-sequence indexer sites required')
offset=0
for _ in range(2):
    start=dense.index(score,offset)
    end=dense.index('                        if (L.cand_src)',start)
    old=dense[start:end]
    lines=old.splitlines(True)
    if len(lines)<3 or 'sc = ggml_reshape_2d(gc, sc, gcap, nq);' not in lines[1]:raise RuntimeError('exact score shape differs')
    candidate=''.join(lines[2:])
    if candidate.count('sc = ggml_add(gc, sc, m);')!=1:raise RuntimeError('exact candidate block mask differs')
    candidate=candidate.replace('sc = ggml_add(gc, sc, m);','candidate_mask = m;')
    replacement='                        ggml_tensor * candidate_mask = nullptr;\n'+candidate+'''                        ggml_tensor * index_mask = im.indexer_mask_first && candidate_mask ? ggml_add(gc, vis, candidate_mask) : vis;
                        ggml_tensor * sc = ggml_lightning_indexer(gc, iq_c, ksrc, iw_c, ggml_cast(gc, index_mask, GGML_TYPE_F16));
                        sc = ggml_reshape_2d(gc, sc, gcap, nq);
                        if (candidate_mask && !im.indexer_mask_first) sc = ggml_add(gc, sc, candidate_mask);
'''
    dense=dense[:start]+replacement+dense[end:]
    offset=start+len(replacement)
(R/'ds41_dense.cpp').write_text(dense,newline='\n')
(R/'dense-mask-first-v40.patch').write_text(''.join(difflib.unified_diff(original_dense.splitlines(True),dense.splitlines(True),fromfile='qualified/ds41_dense.cpp',tofile='v40/ds41_dense.cpp')),newline='\n')

cu=(R/'lightning-indexer.original.cu').read_text()
original_cu=cu
anchor='#include "convert.cuh"'
if cu.count(anchor)!=1:raise RuntimeError('exact CUDA helper includes differ')
cu=cu.replace(anchor,anchor+'\n#include "aster_indexer_mask_skip.cuh"')
decl='template <int WARPS_PER_BLOCK, int K_VECS_PER_BLOCK, int64_t N_EMBD, int64_t N_HEAD, ggml_type TYPE_K>'
if cu.count(decl)!=3:raise RuntimeError('exact WMMA/fallback/vector template count differs')
cu=cu.replace(decl,decl[:-1]+', bool MASK_SKIP>')
args='''            Q, K, W, M, dst, start_kv, n_kv, i_batch, i_stream, static_cast<int>(nem3),
            nb1, nb3, nbq1, nbq2, nbq3, nbk2, nbk3, nbw1, nbw3, nbm1, nbm3'''
anchor='    const int start_kv = blockIdx.x * K_VECS_PER_BLOCK;'
if cu.count(anchor)!=1:raise RuntimeError('exact WMMA block geometry differs')
cu=cu.replace(anchor,anchor+'''
    if constexpr (MASK_SKIP) {
        if (aster_indexer_masked_block<true, K_VECS_PER_BLOCK, N_EMBD, N_HEAD, TYPE_K>(
'''+args+''')) return;
    }
''')
anchor='    const int start_kv = start_kv_block + i_warp * K_VECS_PER_WARP;'
if cu.count(anchor)!=1:raise RuntimeError('exact vector block geometry differs')
cu=cu.replace(anchor,anchor+'''
    if constexpr (MASK_SKIP) {
        // Eligibility is block-wide even though the unchanged vector work is per warp.
        if (aster_indexer_masked_block<false, K_VECS_PER_BLOCK, N_EMBD, N_HEAD, TYPE_K>(
'''+args.replace('dst, start_kv,','dst, start_kv_block,')+''')) return;
    }
''')
start=cu.index('#define LIGHTNING_INDEXER_CASE(')
end=cu.index('\nvoid ggml_cuda_lightning_indexer(',start)
old_macro=cu[start:end]
# Select at the host, so MASK_SKIP=false contains the exact original arithmetic
# and does not reserve the eligibility helper's shared memory or registers.
macro=r'''#define ASTER_INDEXER_LAUNCH(kernel, n_embd, n_head, type_K, mask_skip)                 \
    kernel<WARPS_PER_BLOCK, K_VECS_PER_BLOCK, n_embd, n_head, type_K, mask_skip>            \
        <<<grid, block, 0, ctx.stream()>>>(                                              \
        q_d, k_d, w_d, m_d, dst_d, n_stream, n_batch, n_kv,                               \
        nb1, nb2, nb3, nbq1, nbq2, nbq3, nbk1, nbk2, nbk3,                               \
        nbw1, nbw2, nbw3, nbm1, nbm2, nbm3, nem3)

#define LIGHTNING_INDEXER_CASE(kernel, n_embd, n_head, K, type_K)                         \
    if (K->type == (type_K)) {                                                           \
        if (mask_skip) { ASTER_INDEXER_LAUNCH(kernel,n_embd,n_head,type_K,true); }          \
        else { ASTER_INDEXER_LAUNCH(kernel,n_embd,n_head,type_K,false); }                  \
    } else
'''
cu=cu[:start]+macro+cu[end:]
anchor='void ggml_cuda_lightning_indexer(ggml_backend_cuda_context & ctx, ggml_tensor * dst) {'
if cu.count(anchor)!=1:raise RuntimeError('exact CUDA host entry differs')
cu=cu.replace(anchor,anchor+'''
    static const auto mode = aster_indexer::parse(std::getenv("ASTER_XQ_INDEXER_MASK_SKIP"));
    if (mode == aster_indexer::Mode::invalid) GGML_ABORT("ASTER_XQ_INDEXER_MASK_SKIP must be 0 or 1");
    const bool mask_skip = mode == aster_indexer::Mode::on;
''')
(R/'lightning-indexer.cu').write_text(cu,newline='\n')
(R/'indexer-mask-skip-v40.patch').write_text(''.join(difflib.unified_diff(original_cu.splitlines(True),cu.splitlines(True),fromfile='qualified/lightning-indexer.cu',tofile='v40/lightning-indexer.cu')),newline='\n')

test=r'''#include "aster_indexer_mask_policy.hpp"
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <limits>
int main() {
    int checks=0;
    auto require=[&](bool yes) { if (!yes) std::exit(1); ++checks; };
    using aster_indexer::Mode;
    require(aster_indexer::parse(nullptr)==Mode::off);
    require(aster_indexer::parse("0")==Mode::off);
    require(aster_indexer::parse("1")==Mode::on);
    for (const char* text : {"", "2", "true", "false", "10", "01", " 1", "1 "}) require(aster_indexer::parse(text)==Mode::invalid);
    const float inf=std::numeric_limits<float>::infinity(), nan=std::numeric_limits<float>::quiet_NaN();
    for (float x : {-1024.f,-1.f,-0.f,0.f,1.f,1024.f}) require(aster_indexer::bounded(x));
    for (float x : {-1025.f,1025.f,inf,-inf,nan}) require(!aster_indexer::bounded(x));
    // The graph rewrite only reassociates exact {+0,-inf} masks. Include edge
    // values and non-finite scores; NaN payloads are not claimed bitwise equal.
    for (float raw : {0.f,-0.f,1.f,-1.f,3.125f,-1024.f,std::numeric_limits<float>::max(),-std::numeric_limits<float>::max(),inf,-inf,nan})
        for (float causal : {0.f,-inf}) for (float candidate : {0.f,-inf}) {
            const float old=(raw+causal)+candidate, next=raw+(causal+candidate);
            require((std::isnan(old)&&std::isnan(next)) || std::memcmp(&old,&next,sizeof(float))==0);
        }
    std::printf("PASS indexer mask policy and reassociation: %d checks\n",checks);
}
'''.replace('#include <limits>','#include <limits>\n#include <initializer_list>')
(R/'indexer_mask_policy_test.cpp').write_text(test,newline='\n')
out=dict(schema='aster-indexer-exact-mask-v40-sources',flag='ASTER_XQ_INDEXER_MASK_SKIP=0|1',new_weight_precision=False,approximate_topk=False,block_skip_only_for_exact_negative_infinity=True,bounded_qkw_max_abs=1024,compiled=False,gpu_qualified=False,source_files=[])
for p in sorted(R.iterdir()):
    if p.is_file():out['source_files'].append(dict(file=p.name,bytes=p.stat().st_size,sha256=hashlib.sha256(p.read_bytes()).hexdigest()))
(R/'source-manifest.json').write_text(json.dumps(out,indent=2)+'\n')
print(json.dumps({'source_frozen':True,'files':len(out['source_files']),'compiled':False,'gpu_qualified':False}))
