"""Patch the exact stable generate source, preserving its model kernels and link inputs."""
import difflib,hashlib,json
from pathlib import Path
BASE=Path('D:/code/aster/target/h200-native');root=BASE/'ttft-v11'
recipe=json.loads((BASE/'xolotl-ttft-v11-build-recipe.json').read_text());original=(BASE/'xolotl-ttft-v11-ds41_generate.original.cpp').read_text()
if hashlib.sha256(original.encode()).hexdigest()!=recipe['source_sha256']:raise RuntimeError('stable source byte identity differs')
s=original
def replace(old,new):
    global s
    if s.count(old)!=1:raise RuntimeError('source anchor is not unique: '+old[:100])
    s=s.replace(old,new)
replace('#include "ds41_conversation.hpp"','#include "ds41_conversation.hpp"\n#include "aster_prefill_policy.hpp"')
replace('int bcap_n = 0, bcap_got = 0;', 'int bcap_have[3] = {0, 0, 0};\n    int bcap_n = 0, bcap_got = 0;')
anchor='    // the layer-major batch width for THIS turn:'
helper='''    // Capture the complete last seed window across batch boundaries, including the
    // sequential one-token remainder. All three hidden taps must cover every row.
    auto capture_seed = [&](Ds41Dense & current, int layer, int total, int begin, int width) -> bool {
        if (bcap_n <= 0) return true;
        int j = 0; while (j < 3 && layer != dm_layers[j]) ++j;
        if (j == 3) return true;
        const auto slice = aster_prefill::tail_slice(total, bcap_n, begin, width);
        if (slice.count == 0) return true;
        if (!current.read_state_cols(slice.column, slice.count, vx)) { ferr = current.last_error(); return false; }
        const size_t HC = vx.size() / ((size_t) slice.count * n_embd);
        for (int t = 0; t < slice.count; ++t) {
            float * dst = bcap_vh.data() + (size_t) (slice.destination + t) * 3 * n_embd + (size_t) j * n_embd;
            for (size_t h = 0; h < HC; ++h) {
                const float * xs = vx.data() + ((size_t) t * HC + h) * n_embd;
                for (int64_t d = 0; d < n_embd; ++d) dst[d] += xs[d] / (float) HC;
            }
        }
        bcap_have[j] += slice.count;
        bcap_got = std::min({bcap_have[0], bcap_have[1], bcap_have[2]});
        return true;
    };
'''
replace(anchor,helper+anchor)
start=s.index('                    // serve DSpark seed (bcap_n > 0):')
end=s.index('\n                }\n                fd_dense += now_ms() - t0;',start)
s=s[:start]+'''                    if (!capture_seed(*cur_dense, l, n, c0, B)) return false;'''+s[end:]
replace('                    if (!cur_dense->finish(l, crouted.data())) { ferr = cur_dense->last_error(); return false; }','                    if (!cur_dense->finish(l, crouted.data())) { ferr = cur_dense->last_error(); return false; }\n                    if (!capture_seed(*cur_dense, l, n, c0, m)) return false;')
replace('            bcap_n = bcap; bcap_got = 0;', '            bcap_n = bcap; bcap_got = 0;\n            std::fill_n(bcap_have, 3, 0);')
replace('''                    const double need = (4.3 * 1073741824.0 + 0.95 * 1048576.0 * (double) std::min(dh, PB)) +
                                        a.vram_margin_gib * 1073741824.0;''','''                    const double need = aster_prefill::reservation_bytes(std::min(dh, PB),
                                        (double) dense.span_graph_bytes(total, SB), a.vram_margin_gib);
                    if (!std::isfinite(need)) {
                        std::printf("ERR prefill graph reservation is invalid\\n"); std::fflush(stdout); continue;
                    }''')
replace('if (bcap_got < 1) { ferr = "DSpark batch seed captured nothing"; ok = false; }','if (bcap_got != bcap) { ferr = "DSpark batch seed window is incomplete"; ok = false; }')
(root/'ds41_generate.cpp').write_text(s,newline='\n')
(root/'prefill-v11.patch').write_text(''.join(difflib.unified_diff(original.splitlines(keepends=True),s.splitlines(keepends=True),fromfile='a/tools/v41/engine/ds41_generate.cpp',tofile='b/tools/v41/engine/ds41_generate.cpp')),newline='\n')
print(json.dumps(dict(phase='frozen',original_sha256=recipe['source_sha256'],candidate_sha256=hashlib.sha256(s.encode()).hexdigest(),patch_lines=len((root/'prefill-v11.patch').read_text().splitlines()))))
