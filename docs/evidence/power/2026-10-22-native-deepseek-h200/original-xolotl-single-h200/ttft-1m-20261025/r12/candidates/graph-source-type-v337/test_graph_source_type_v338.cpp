#include "aster_graph_source_type_v338.hpp"
#include <cassert>
#include <cstdio>
#include <thread>

int main() {
    using namespace aster_graph_types;
    assert(parse(nullptr)==Mode::off&&parse("0")==Mode::off&&parse("1")==Mode::on);
    for(const char* s:{"","00","01","2","-1","true","false"," 1","1 ","1\n"})assert(parse(s)==Mode::invalid);
    Cache<> cache;
    assert(cache.changed(1,{1,30,65535}));
    assert(!cache.changed(1,{1,30,65535}));
    assert(cache.changed(1,{30,30,65535}));
    assert(!cache.changed(1,{30,30,65535}));
    assert(cache.changed(1,{1,30,65535}));
    assert(cache.changed(1,{1,30,0}));
    assert(cache.changed(2,{1,30,0}));
    assert(!cache.changed(1,{1,30,0}));
    Cache<2,100> entries;
    assert(entries.changed(1,{1})&&entries.changed(2,{1})&&entries.changed(3,{1}));
    assert(entries.changed(1,{1})); // evicted key must recapture
    assert(!entries.changed(3,{1}));
    Cache<20,8> bytes;
    assert(bytes.changed(1,{1,2})&&bytes.changed(2,{1,2})&&bytes.changed(3,{1,2}));
    assert(bytes.changed(1,{1,2}));
    assert(!bytes.changed(3,{1,2}));
    assert(bytes.changed(3,{1,2,3,4,5})); // oversized signatures are never accepted as stable
    assert(bytes.changed(3,{1,2,3,4,5}));
    assert(bytes.changed(3,{1,2}));
    assert(!bytes.changed(3,{1,2}));
    std::vector<std::thread> threads;
    for(int i=0;i<8;++i)threads.emplace_back([&,i]{std::vector<std::uint16_t> signature={static_cast<std::uint16_t>(i)};assert(cache.changed(100+i,signature));for(int j=0;j<1000;++j)assert(!cache.changed(100+i,signature));});
    for(auto& t:threads)t.join();
    std::puts("PASS graph source-type modes, changes, absences, entry/byte evictions, oversize and 8000 concurrent stable lookups");
}
