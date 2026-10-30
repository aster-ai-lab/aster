#pragma once
#include <cstddef>
#include <cstdint>
#include <cstring>
#include <mutex>
#include <unordered_map>
#include <utility>
#include <vector>

namespace aster_graph_types {
enum class Mode { off, on, invalid };
inline Mode parse(const char* raw) {
    if(raw==nullptr||std::strcmp(raw,"0")==0)return Mode::off;
    if(std::strcmp(raw,"1")==0)return Mode::on;
    return Mode::invalid;
}

// Separate host cache preserves the existing ggml_cuda_graph ABI across the
// already-qualified CUDA archive. An eviction always requests recapture.
template<std::size_t MaxEntries=1024,std::size_t MaxBytes=32*1024*1024>
class Cache {
    static_assert(MaxEntries>0&&MaxBytes>0);
    std::mutex mutex_;
    std::unordered_map<std::uintptr_t,std::vector<std::uint16_t>> entries_;
    std::size_t bytes_=0;
public:
    bool changed(std::uintptr_t key,const std::vector<std::uint16_t>& signature) {
        std::lock_guard<std::mutex> lock(mutex_);
        const std::size_t bytes=signature.size()*sizeof(std::uint16_t);
        auto old=entries_.find(key);
        if(old!=entries_.end()&&old->second==signature)return false;
        if(old!=entries_.end()){bytes_-=old->second.size()*sizeof(std::uint16_t);entries_.erase(old);}
        if(bytes>MaxBytes)return true;
        if(entries_.size()>=MaxEntries||bytes_>MaxBytes-bytes){entries_.clear();bytes_=0;}
        entries_.emplace(key,signature);bytes_+=bytes;
        return true;
    }
};
}
