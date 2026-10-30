// CPU-only measurement through the upstream Ds41Engram public hash API.
// No table files are opened and no GPU or model forward pass is executed.
#include "ds41_engram.hpp"
#include <algorithm>
#include <bit>
#include <chrono>
#include <cstdint>
#include <cstdio>
#include <fstream>
#include <vector>

using namespace strata;
using namespace strata::v41;

int main(int argc,char** argv) {
    static_assert(std::endian::native==std::endian::little);
    if(argc!=3){std::fprintf(stderr,"exact GGUF metadata and u32 input files required\n");return 2;}
    const auto start=std::chrono::steady_clock::now();
    GgufModel model=GgufModel::open(argv[1]);
    Ds41Geometry geometry;
    const std::string error=ds41_read_geometry(model,geometry);
    if(!error.empty()){std::fprintf(stderr,"metadata geometry: %s\n",error.c_str());return 2;}
    if(geometry.engram_layers.size()!=2 || geometry.engram_dim!=256 || geometry.engram_ngram<2 || geometry.engram_ngram>8 || geometry.engram_heads<1 || geometry.engram_heads>24){
        std::fprintf(stderr,"exact bounded Engram geometry differs\n");return 2;
    }
    Ds41Engram engram;engram.setup_hash(geometry);
    const int64_t cols=engram.n_cols();
    if(cols!=24){std::fprintf(stderr,"expected24 hash columns per layer\n");return 2;}
    std::ifstream input(argv[2],std::ios::binary);
    std::vector<uint32_t> ids(1047544);
    if(!input.read(reinterpret_cast<char*>(ids.data()),static_cast<std::streamsize>(ids.size()*sizeof(uint32_t))) || input.peek()!=std::char_traits<char>::eof()){
        std::fprintf(stderr,"exact actual1M input byte count differs\n");return 2;
    }
    for(size_t pos=0;pos<ids.size();++pos){
        if(ids[pos]>=static_cast<uint64_t>(geometry.vocab)){std::fprintf(stderr,"input ID outside exact vocabulary\n");return 2;}
        engram.set_token(static_cast<int64_t>(pos),static_cast<int>(ids[pos]));
    }
    std::vector<uint64_t> keys;keys.reserve(ids.size()*48);
    int64_t rows[24];
    for(size_t pos=0;pos<ids.size();++pos)
        for(int layer=0;layer<2;++layer){
            engram.hash(static_cast<int64_t>(pos),layer,rows);
            for(int col=0;col<24;++col){
                if(rows[col]<0 || rows[col]>=geometry.engram_rows[static_cast<size_t>(layer)] || static_cast<uint64_t>(rows[col])>UINT32_MAX){
                    std::fprintf(stderr,"row outside declared table geometry\n");return 2;
                }
                keys.push_back((static_cast<uint64_t>(layer)<<32)|static_cast<uint32_t>(rows[col]));
            }
        }
    const size_t references=keys.size();
    std::sort(keys.begin(),keys.end());keys.erase(std::unique(keys.begin(),keys.end()),keys.end());
    const size_t first=static_cast<size_t>(std::lower_bound(keys.begin(),keys.end(),1ull<<32)-keys.begin());
    const double seconds=std::chrono::duration<double>(std::chrono::steady_clock::now()-start).count();
    std::printf("{\"phase\":\"passed\",\"GPU_executed\":false,\"forward_executed\":false,\"row_tables_opened\":false,\"prompt_tokens\":%zu,\"hash_references\":%zu,\"unique_row_keys\":%zu,\"layer_unique_rows\":[%zu,%zu],\"raw_cache_row_bytes\":264,\"complete_prompt_raw_row_bytes\":%llu,\"geometry_fingerprint\":%llu,\"seconds\":%.9f,\"speed_improvement_claimed\":false}\n",
        ids.size(),references,keys.size(),first,keys.size()-first,
        static_cast<unsigned long long>(keys.size())*264ull,
        static_cast<unsigned long long>(engram.geometry_fingerprint()),seconds);
}
