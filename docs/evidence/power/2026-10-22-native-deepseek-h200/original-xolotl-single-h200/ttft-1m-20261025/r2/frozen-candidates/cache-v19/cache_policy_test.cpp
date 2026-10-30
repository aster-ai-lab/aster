#include "aster_cache_restore_policy.hpp"

#include <cstdio>
#include <cstdlib>
#include <vector>

using aster_cache_restore::Record;

int main() {
    const std::vector<uint64_t> offsets{0, 256, 768, 1024, 1792};
    const std::vector<Record> mapping{{3, 0, 0, 600}, {0, 0, 1, 128}, {1, 1, 0, 512}};
    int checked = 0;
    auto expect = [&](const std::vector<Record>& records, const std::vector<uint64_t>& sizes,
                      int64_t live, int64_t layers, int64_t experts, bool wanted) {
        std::string error = "old error";
        const bool got = aster_cache_restore::valid(records, sizes, live, layers, experts, error);
        ++checked;
        if (got != wanted || (got && !error.empty()) || (!got && error.empty())) {
            std::fprintf(stderr, "case %d failed: got %d wanted %d, %s\n", checked, got, wanted, error.c_str());
            std::exit(1);
        }
    };
    expect(mapping, offsets, 4, 2, 4, true);
    expect({}, {0}, 0, 2, 4, true);
    expect({{1, 1, 0, 512}}, offsets, 2, 2, 4, true);
    for (int64_t field = 0; field < 4; ++field) {
        for (int64_t bad : {-1ll, 0ll, 100ll}) {
            auto changed = mapping;
            if (field == 0) changed[0].slot = bad;
            if (field == 1) changed[0].layer = bad;
            if (field == 2) changed[0].expert = bad;
            if (field == 3) changed[0].bytes = bad;
            const bool wanted = ((field == 1 || field == 2) && bad == 0) || (field == 3 && bad == 100);
            expect(changed, offsets, 4, 2, 4, wanted);
        }
    }
    auto duplicate_slot = mapping; duplicate_slot.push_back({1, 1, 1, 128});
    auto duplicate_expert = mapping; duplicate_expert.push_back({2, 1, 0, 128});
    expect(duplicate_slot, offsets, 4, 2, 4, false);
    expect(duplicate_expert, offsets, 4, 2, 4, false);
    expect(mapping, offsets, 3, 2, 4, false);
    expect(mapping, offsets, 5, 2, 4, false);
    expect(mapping, {}, 4, 2, 4, false);
    expect(mapping, {1, 256, 768, 1024, 1792}, 4, 2, 4, false);
    expect(mapping, {0, 256, 256, 1024, 1792}, 4, 2, 4, false);
    expect(mapping, {0, 256, 255, 1024, 1792}, 4, 2, 4, false);
    expect(mapping, offsets, -1, 2, 4, false);
    expect(mapping, offsets, 4, 0, 4, false);
    expect(mapping, offsets, 4, 2, 0, false);
    expect(mapping, offsets, 4, INT64_MAX, INT64_MAX, false);
    for (size_t i = 0; i < mapping.size(); ++i) {
        auto too_large = mapping;
        too_large[i].bytes = static_cast<int64_t>(offsets[mapping[i].slot + 1] - offsets[mapping[i].slot]) + 1;
        expect(too_large, offsets, 4, 2, 4, false);
    }
    std::printf("PASS %d sized cache ownership/capacity cases\n", checked);
}
