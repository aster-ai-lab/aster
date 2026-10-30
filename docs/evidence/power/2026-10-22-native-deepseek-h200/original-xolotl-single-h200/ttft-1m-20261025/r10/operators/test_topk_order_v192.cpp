#include "aster_topk_order_v192.hpp"
#include <algorithm>
#include <array>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <vector>
static void check(bool ok){if(!ok){std::fprintf(stderr,"exact packed float order failed\n");std::exit(1);}}
int main(){
    std::uint64_t checks=0;
    // Expected descending radix order includes NaNs, infinities and subnormals.
    constexpr std::array<std::uint32_t,12> values={0x7fffffffu,0x7fc00000u,0x7f800000u,0x3f800000u,0x00000001u,0x00000000u,0x80000000u,0x80000001u,0xbf800000u,0xff800000u,0xffc00000u,0xffffffffu};
    for(std::size_t i=1;i<values.size();++i){check(aster_topk::key(values[i-1],0)>aster_topk::key(values[i],1));++checks;}
    for(std::uint32_t column=0;column<1048576;++column){
        for(std::uint32_t bits:values){check(aster_topk::column(aster_topk::key(bits,column))==column);++checks;}
        check(aster_topk::key(0,column)==aster_topk::key(0x80000000u,column));++checks;
        if(column){check(aster_topk::key(0x7fc00000u,column-1)>aster_topk::key(0x7fc00000u,column));++checks;}
    }
    std::printf("{\"phase\":\"passed\",\"contracts\":%llu,\"GPU_executed\":false,\"speed_claimed\":false}\n",static_cast<unsigned long long>(checks));
}
