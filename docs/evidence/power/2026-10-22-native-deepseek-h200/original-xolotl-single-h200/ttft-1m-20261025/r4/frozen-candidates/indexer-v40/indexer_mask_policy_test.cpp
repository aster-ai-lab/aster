#include "aster_indexer_mask_policy.hpp"
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <limits>
#include <initializer_list>
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
