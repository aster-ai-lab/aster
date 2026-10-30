// Standalone CPU contracts for optional seed metadata and the actual upstream bounded conversation pool.
#include "aster_dspark_seed.hpp"
#include "ds41_conv_pool.hpp"
#include <cstdio>
#include <cstdlib>
#include <limits>
#include <string>

using namespace aster_dspark_seed;
using namespace strata::v41;
static int checks = 0;
static void require(bool value, const char * label) {
    ++checks;
    if (!value) { std::fprintf(stderr, "failed: %s\n", label); std::exit(1); }
}
static void rejects(const Blobs & blobs, int width, int64_t pos) {
    std::vector<float> stale{42.0f}; int rows = 99;
    require(!decode(blobs, width, pos, stale, rows) && stale.empty() && rows == 0,
            "invalid metadata must discard stale seed without partial output");
}
int main() {
    for (int width : {1, 17, 7168}) for (int count : {1, 7, 128}) {
        std::vector<float> seed((size_t) count * 3 * width);
        for (size_t i=0; i<seed.size(); ++i) seed[i] = ((int)(i%251)-125) / 128.0f;
        seed[0] = -0.0f;
        Blobs blobs; std::vector<float> out; int rows=0;
        require(encode(seed,count,width,1047544,blobs), "finite tail encodes");
        require(decode(blobs,width,1047544,out,rows) && rows==count && out.size()==seed.size()
                && std::memcmp(out.data(),seed.data(),out.size()*sizeof(float))==0, "every seed byte roundtrips");
        rejects(blobs,width,1047543); rejects(blobs,width+1,1047544);
        auto bad=blobs; bad[0][0]^=1; rejects(bad,width,1047544);
        bad=blobs; bad[0].pop_back(); rejects(bad,width,1047544);
        bad=blobs; bad[0].push_back(0); rejects(bad,width,1047544);
    }
    Blobs valid; std::vector<float> s(18,0.25f), out; int rows=0;
    require(encode(s,2,3,20,valid), "small fixture encodes");
    for (size_t n=0;n<header_bytes;++n) { auto bad=valid; bad[0].resize(n); rejects(bad,3,20); }
    auto bad=valid; put(bad[0],12,0,4); rejects(bad,3,20);
    bad=valid; put(bad[0],12,129,4); rejects(bad,3,20);
    bad=valid; put(bad[0],8,0xffffffff,4); rejects(bad,3,20);
    bad=valid; bad.push_back(bad[0]); rejects(bad,3,20);
    float nan=std::numeric_limits<float>::quiet_NaN(); bad=valid;
    std::memcpy(bad[0].data()+header_bytes,&nan,sizeof(nan)); rejects(bad,3,20);
    s[0]=nan; Blobs empty=valid;
    require(!encode(s,2,3,20,empty) && empty.empty(), "nonfinite encode is refused");
    require(!encode({},129,3,200,empty), "oversized row count is refused");
    require(!encode({},1,std::numeric_limits<int>::max(),20,empty), "oversized shape is refused before allocation");
    require(decode({},3,20,out,rows) && out.empty() && rows==0, "old records without metadata remain valid");

    Ds41PoolLayout layout;
    layout.streams.push_back({"position-stream",4,1,512});
    layout.regions.push_back({"snapshot-ring",16}); layout.max_positions=512;
    Ds41PoolConfig cfg; cfg.cuda=false; cfg.verify=true; cfg.slab_bytes=256; cfg.extent_bytes=1024;
    cfg.budget_bytes=65536;
    std::string err; Ds41ConvPool pool; require(pool.init(layout,cfg,err),"actual CPU pool initializes");
    std::vector<uint8_t> stream(2048,0x31), region(16,0x41);
    Ds41PoolTargets targets; targets.stream={stream.data()}; targets.region={region.data()}; targets.layout_fp=layout.fingerprint();
    std::vector<int32_t> tokens(20,71); std::vector<float> logits{1,2,3};
    s.assign(18,0.25f); require(encode(s,2,3,20,valid),"pool seed A encodes");
    Ds41PoolSaveOpts opts;
    opts.blobs.push_back({valid[0].data(),valid[0].size()});
    const auto a=pool.save(targets,tokens.data(),nullptr,20,logits.data(),logits.size(),err,opts);
    require(a!=0 && pool.fence(&err),"actual pool saves A and metadata");
    const auto used_with_seed=pool.stats().host_bytes;
    Ds41ConvPool bare;
    require(bare.init(layout,cfg,err),"baseline metadata-free pool initializes");
    require(bare.save(targets,tokens.data(),nullptr,20,logits.data(),logits.size(),err)!=0 && bare.fence(&err),"baseline pool saves the same boundary without seed");
    require(used_with_seed==bare.stats().host_bytes+valid[0].size(),"seed bytes are charged exactly to the existing pool host budget");
    valid[0].assign(valid[0].size(),0); // save owns the bytes: the caller may immediately reuse its buffer.
    std::fill(stream.begin(),stream.end(),0x99); std::fill(region.begin(),region.end(),0x98);
    tokens.assign(20,72); s.assign(18,-0.5f); require(encode(s,2,3,20,valid),"pool seed B encodes");
    opts.blobs={{valid[0].data(),valid[0].size()}};
    const auto b=pool.save(targets,tokens.data(),nullptr,20,logits.data(),logits.size(),err,opts);
    require(b && b!=a && pool.fence(&err),"actual pool saves unrelated B");
    tokens.assign(20,71); const auto match=pool.best_prefix(tokens.data(),nullptr,20);
    require(match.rec==a && match.pos==20,"strict token identity selects A");
    Ds41PoolRestore restored;
    require(pool.restore(a,match.boundary,0,targets,restored,err) && pool.wait(restored,&err),"actual pool restores A");
    require(decode(restored.blobs,3,20,out,rows) && rows==2 && out==std::vector<float>(18,0.25f),"A restores its own seed rather than B or reused caller bytes");
    require(stream[0]==0x31 && region[0]==0x41,"A device data agrees with A seed");
    rejects(restored.blobs,3,18); // a partial rewind cannot use seed rows from a later boundary.
    require(pool.evict(a),"eviction releases the entire A record");
    require(pool.best_prefix(tokens.data(),nullptr,20).pos==0,"evicted A cannot restore stale seed");
    require(pool.clear(false)==1 && pool.stats().used_bytes==0,"clearing releases B and all metadata budget");
    std::printf("pool_seed: %d contracts passed\n",checks);
}
