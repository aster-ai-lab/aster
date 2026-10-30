"""Build a sized-slot checked candidate; never edit the qualified upstream tree."""
import difflib, hashlib, json
from pathlib import Path

BASE = Path(__file__).resolve().parent
root = BASE / 'ttft-v18'
original = (BASE / 'xolotl-mixed-keep-v18-ds41_moe.original.cpp').read_bytes()
recipe = json.loads((BASE / 'xolotl-mixed-keep-v18-build-recipe.json').read_text())
if hashlib.sha256(original).hexdigest() != recipe['source_sha256']:
    raise RuntimeError('frozen qualified MoE source differs')
source = original.decode()

def replace_once(before, after):
    global source
    if source.count(before) != 1:
        raise RuntimeError('source anchor differs: ' + before[:100])
    source = source.replace(before, after, 1)

replace_once('#include "ds41_moe.hpp"', '#include "ds41_moe.hpp"\n#include "aster_cache_restore_policy.hpp"')
replace_once('int64_t Ds41MoeTier::shrink_cache_for(int64_t need_bytes, std::string& err) {', '''#if defined(DS4_MOE_CUDA)
namespace {
// A separate, closed opt-in from uniform KEEP_LRU. Mixed slots require both
// the complete ownership/size gate and exact slot restoration below.
int aster_keep_mixed_mode() {
    static const int mode = [] {
        const char* value = std::getenv("ASTER_XQ_KEEP_MIXED");
        if (!value || std::strcmp(value, "0") == 0) return 0;
        if (std::strcmp(value, "1") == 0) return 1;
        return -1;
    }();
    return mode;
}

bool aster_cache_map_valid(const Ds41MoeImpl& im, const strata::core::ExpertCache& cache,
                           bool restoring, std::string& err) {
    if (cache.per_layer_admission()) {
        err = "mixed cache restore does not support per-layer admission";
        return false;
    }
    std::vector<uint64_t> offsets;
    offsets.reserve(static_cast<size_t>(cache.full_slots()) + 1);
    for (int64_t slot = 0; slot <= cache.full_slots(); ++slot)
        offsets.push_back(cache.slot_offset(slot));
    std::vector<aster_cache_restore::Record> records;
    for (int64_t layer = 0; layer < im.g.n_layers; ++layer)
        for (int64_t expert = 0; expert < im.g.n_experts; ++expert) {
            const int32_t slot = cache.slot_of(layer, expert);
            if (slot >= 0) records.push_back({slot, layer, expert, im.bl[static_cast<size_t>(im.g.cls(layer, expert))]});
        }
    if (restoring)
        for (const auto& entry : im.shrunk)
            if (entry[0] < cache.slots())
                records.push_back({entry[0], entry[1], entry[2],
                                  im.bl[static_cast<size_t>(im.g.cls(entry[1], entry[2]))]});
    return aster_cache_restore::valid(records, offsets, cache.slots(), im.g.n_layers, im.g.n_experts, err);
}
} // namespace
#endif

int64_t Ds41MoeTier::shrink_cache_for(int64_t need_bytes, std::string& err) {''')
replace_once('''    if (im_->cfg.vram_lru && (!keep_lru || im_->mixed_sizes)) {
        err = "--vram-lru (uniform cache needs STRATA_SERVE_KEEP_LRU=1)";
        return -1;
    }''', '''    const int mixed_mode = aster_keep_mixed_mode();
    if (mixed_mode < 0) { err = "ASTER_XQ_KEEP_MIXED must be exactly 0 or 1"; return -1; }
    if (im_->cfg.vram_lru && (!keep_lru || (im_->mixed_sizes && mixed_mode != 1))) {
        err = "--vram-lru requires KEEP_LRU=1; sized slots also require checked ASTER_XQ_KEEP_MIXED=1";
        return -1;
    }
    if (im_->mixed_sizes && mixed_mode == 1 && !aster_cache_map_valid(*im_, *im_->gpu->cache, false, err))
        return -1;''')
replace_once('''    std::vector<std::array<int64_t, 3>> fjobs;   // slot, layer, expert (not in the arena: read from the file)
    int64_t added = 0;''', '''    const bool checked_mixed = im_->mixed_sizes && aster_keep_mixed_mode() == 1;
    if (checked_mixed && !aster_cache_map_valid(*im_, c, true, err)) {
        im_->shrunk.clear();
        return -1;
    }
    std::vector<std::array<int64_t, 3>> fjobs;   // slot, layer, expert (not in the arena: read from the file)
    int64_t added = 0;''')
replace_once('''        if (c.admit_at(j[1], j[2], j[0]) < 0) continue;   // re-admitted elsewhere meanwhile - leave the slot empty''', '''        if (c.admit_at(j[1], j[2], j[0]) < 0) {
            if (checked_mixed) {
                err = "mixed cache exact-slot restoration refused";
                im_->shrunk.clear();
                return -1;
            }
            continue;   // uniform legacy path: leave a re-admitted expert's old slot empty
        }''')
replace_once('''        std::fprintf(stderr, "[keep-chk] %lld refilled slots checked, %lld bad\\n", (long long) n, (long long) bad);
    }''', '''        std::fprintf(stderr, "[keep-chk] %lld refilled slots checked, %lld bad\\n", (long long) n, (long long) bad);
        if (checked_mixed && bad != 0) {
            err = "mixed cache refilled expert bytes differ from source";
            im_->shrunk.clear();
            return -1;
        }
    }''')

out = root / 'ds41_moe.cpp'
with out.open('x', encoding='utf-8', newline='\n') as stream:
    stream.write(source)
patch = ''.join(difflib.unified_diff(original.decode().splitlines(True), source.splitlines(True),
                                     fromfile='a/tools/v41/engine/ds41_moe.cpp',
                                     tofile='b/tools/v41/engine/ds41_moe.cpp'))
(root / 'mixed-cache-checked-v18.patch').write_text(patch, encoding='utf-8', newline='\n')
print(json.dumps(dict(candidate=str(out), sha256=hashlib.sha256(out.read_bytes()).hexdigest(),
                     original_unchanged=True, gpu_qualification_passed=False)))
