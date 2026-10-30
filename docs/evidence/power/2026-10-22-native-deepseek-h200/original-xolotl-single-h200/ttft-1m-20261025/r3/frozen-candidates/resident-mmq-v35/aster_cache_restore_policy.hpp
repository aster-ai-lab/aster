#pragma once

#include <cstddef>
#include <cstdint>
#include <span>
#include <string>
#include <unordered_set>

namespace aster_cache_restore {

struct Record {
    int64_t slot;
    int64_t layer;
    int64_t expert;
    int64_t bytes;
};

// A sized cache can only restore an expert into a slot which fits its own
// format class. Validate the complete live ownership plus the proposed tail
// before an admission or a DMA can change either table or device memory.
inline bool valid(std::span<const Record> records, std::span<const uint64_t> offsets,
                  int64_t live_slots, int64_t layers, int64_t experts, std::string& error) {
    auto refuse = [&](const char* reason) { error = reason; return false; };
    if (layers <= 0 || experts <= 0 || live_slots < 0 || offsets.empty() || offsets[0] != 0 ||
        static_cast<uint64_t>(live_slots) >= offsets.size())
        return refuse("invalid cache restore geometry");
    for (size_t i = 1; i < offsets.size(); ++i)
        if (offsets[i] <= offsets[i - 1]) return refuse("non-increasing sized cache offsets");
    // Pair keys use the caller's actual domain, with an explicit multiplication
    // bound rather than a lossy layer/expert packing or an unbounded allocation.
    if (static_cast<uint64_t>(layers) > UINT64_MAX / static_cast<uint64_t>(experts))
        return refuse("cache expert domain overflow");
    std::unordered_set<int64_t> slots;
    std::unordered_set<uint64_t> identities;
    for (const Record& record : records) {
        if (record.slot < 0 || record.slot >= live_slots || record.layer < 0 || record.layer >= layers ||
            record.expert < 0 || record.expert >= experts || record.bytes <= 0)
            return refuse("cache restore record outside its domain");
        const auto slot = static_cast<size_t>(record.slot);
        if (static_cast<uint64_t>(record.bytes) > offsets[slot + 1] - offsets[slot])
            return refuse("expert bytes exceed sized cache slot");
        const uint64_t identity = static_cast<uint64_t>(record.layer) * static_cast<uint64_t>(experts) +
                                  static_cast<uint64_t>(record.expert);
        if (!slots.insert(record.slot).second || !identities.insert(identity).second)
            return refuse("duplicate cache slot or expert ownership");
    }
    error.clear();
    return true;
}

} // namespace aster_cache_restore
