// Public synthetic gate for sized-slot retention, using the original Xolotl
// ExpertCache and the candidate ownership policy. No model or user data.
#include "aster_cache_restore_policy.hpp"
#include "strata/core/expert_cache.hpp"
#include <cuda_runtime.h>

#include <algorithm>
#include <cstdio>
#include <cstdlib>
#include <string>
#include <vector>

namespace {
using aster_cache_restore::Record;
constexpr int64_t MiB = 1ll << 20;
constexpr int64_t layers = 4, experts = 64, count = 96, segment = 4 * MiB;

[[noreturn]] void fail(const std::string& message) {
    std::fprintf(stderr, "FAIL %s\n", message.c_str());
    std::exit(1);
}

uint8_t byte_of(const Record& row) {
    return static_cast<uint8_t>(row.layer * 53 + row.expert * 7 + 1);
}

void fill(strata::core::ExpertCache& cache, const Record& row) {
    std::vector<uint8_t> data(static_cast<size_t>(row.bytes), byte_of(row));
    std::string error;
    if (!cache.fill_slot_blocking(static_cast<int32_t>(row.slot), data.data(), error, row.bytes)) fail(error);
}

void check(strata::core::ExpertCache& cache, const std::vector<Record>& records, int64_t live) {
    std::string error;
    for (const Record& row : records) {
        if (row.slot >= live) continue;
        if (cache.slot_of(row.layer, row.expert) != row.slot) fail("expert ownership changed");
        std::vector<uint8_t> data(static_cast<size_t>(row.bytes), byte_of(row));
        if (!cache.verify_slot(static_cast<int32_t>(row.slot), data.data(), error, row.bytes)) fail(error);
    }
}
} // namespace

int main() {
    int devices = 0;
    if (cudaGetDeviceCount(&devices) != cudaSuccess || devices != 1) fail("one isolated CUDA device required");
    strata::core::ExpertCache cache;
    cache.set_segment_bytes(segment);
    std::vector<int64_t> sizes;
    std::vector<Record> records;
    for (int64_t slot = 0; slot < count; ++slot) {
        const int64_t bytes = (slot % 3 + 1) * MiB;
        sizes.push_back(bytes);
        records.push_back({slot, slot / 24, slot % 24, bytes});
    }
    std::string error;
    if (!cache.open_sized(sizes, layers, experts, error) || !cache.segmented()) fail("sized cache open: " + error);
    std::vector<uint64_t> offsets;
    for (int64_t slot = 0; slot <= count; ++slot) offsets.push_back(cache.slot_offset(slot));
    if (!aster_cache_restore::valid(records, offsets, count, layers, experts, error)) fail(error);
    for (const Record& row : records) {
        if (cache.admit(row.layer, row.expert) != row.slot) fail("initial exact-slot admission");
        fill(cache, row);
    }
    // Model decode LRU can change the expert in a slot within a layer/class.
    // Preserve that live ownership rather than restoring the initial ranking.
    for (int64_t layer = 0; layer < layers; ++layer) {
        Record& row = records[static_cast<size_t>(layer * 24)];
        row.expert = 32;
        fill(cache, row);
        cache.replace(layer, 0, 32);
    }
    check(cache, records, count);
    int64_t checked = count, restored = 0;
    for (int64_t wanted : {64 * MiB, 31 * MiB, 97 * MiB}) {
        const int64_t boundary = wanted / segment * segment;
        const int64_t keep = cache.slots_within(boundary);
        uint8_t* const first_address = cache.device_slot(0);
        std::vector<Record> prefix, tail;
        for (const Record& row : records) (row.slot < keep ? prefix : tail).push_back(row);
        if (cache.evict_from(keep) != static_cast<int64_t>(tail.size())) fail("tail eviction count");
        if (!cache.shrink(boundary, error) || cache.slots() != keep) fail("shrink: " + error);
        if (cache.device_slot(0) != first_address || cache.device_slot(static_cast<int32_t>(keep)) != nullptr)
            fail("prefix address or unmapped tail pointer");
        if (!aster_cache_restore::valid(prefix, offsets, keep, layers, experts, error)) fail(error);
        check(cache, prefix, keep);
        checked += static_cast<int64_t>(prefix.size());
        if (!cache.grow(cache.full_bytes(), error) || cache.slots() != count) fail("grow: " + error);
        std::vector<Record> proposed = prefix;
        proposed.insert(proposed.end(), tail.begin(), tail.end());
        if (!aster_cache_restore::valid(proposed, offsets, count, layers, experts, error)) fail(error);
        for (const Record& row : tail) {
            if (cache.admit_at(row.layer, row.expert, static_cast<int32_t>(row.slot)) != row.slot)
                fail("exact current-owner restoration");
            fill(cache, row);
            ++restored;
        }
        check(cache, records, count);
        checked += count;
    }
    std::printf("PASS mixed cache: 3 shrink/grow cycles, %lld restored slots, %lld slot byte comparisons, exact live ownership\n",
                static_cast<long long>(restored), static_cast<long long>(checked));
}
