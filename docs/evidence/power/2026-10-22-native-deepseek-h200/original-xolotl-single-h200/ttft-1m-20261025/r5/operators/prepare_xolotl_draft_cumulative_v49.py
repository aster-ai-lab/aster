"""Port only upstream cumulative DSpark truncation with a strict opt-in mode."""
import difflib
import hashlib
import json
from pathlib import Path

B = Path(__file__).resolve().parent
D = B / 'draft-cumulative-v49'
if D.exists():
    raise RuntimeError('cumulative draft candidate is already frozen')
D.mkdir()
original = (B / 'ttft-v15/ds41_generate.cpp').read_bytes()
if hashlib.sha256(original).hexdigest() != '196c58faf7c1674869fd49757b3f5775ab672ec37fc121ebeb32e511cdaf8578':
    raise RuntimeError('exact deployed V15 generator source differs')
header = '''#pragma once
#include <cmath>
#include <cstring>
namespace aster_draft {
enum class Mode { off, c010, c025, c050, invalid };
inline Mode parse(const char * raw) {
    if (!raw || std::strcmp(raw, "0") == 0) return Mode::off;
    if (std::strcmp(raw, "0.1") == 0) return Mode::c010;
    if (std::strcmp(raw, "0.25") == 0) return Mode::c025;
    if (std::strcmp(raw, "0.5") == 0) return Mode::c050;
    return Mode::invalid;
}
inline float threshold(Mode mode) {
    switch (mode) {
        case Mode::c010: return 0.1f;
        case Mode::c025: return 0.25f;
        case Mode::c050: return 0.5f;
        default: return 0.0f;
    }
}
// Only shorten an already-selected draft prefix. Target verification and
// acceptance, logits, weights and routing stay in the existing engine path.
inline int truncate(Mode mode, const float * logits, int count) {
    if (count < 0 || count > 5 || mode == Mode::invalid) return 0;
    if (mode == Mode::off || count == 0) return count;
    if (!logits) return 0;
    float product = 1.0f;
    int selected = 0;
    while (selected < count &&
           (product *= 1.0f / (1.0f + std::exp(-logits[selected]))) >= threshold(mode)) ++selected;
    return selected;
}
}
'''
s = original.decode()
anchor = '#include "aster_prefill_policy.hpp"'
if s.count(anchor) != 1:
    raise RuntimeError('exact generator include anchor differs')
s = s.replace(anchor, anchor + '\n#include "aster_draft_cumulative.hpp"')
anchor = 'int main(int argc, char ** argv) {'
if s.count(anchor) != 1:
    raise RuntimeError('exact main entry differs')
s = s.replace(anchor, anchor + '''
    const auto draft_cumulative = aster_draft::parse(std::getenv("ASTER_XQ_DSPARK_CUM"));
    if (draft_cumulative == aster_draft::Mode::invalid) {
        std::fprintf(stderr, "ASTER_XQ_DSPARK_CUM must be 0, 0.1, 0.25, or 0.5\\n"); return 2;
    }''')
anchor = 'const bool refeed = dirty && ((P + 1) & 1);'
if s.count(anchor) != 2:
    raise RuntimeError('exact serve/one-shot draft truncation sites differ')
s = s.replace(anchor, 'k = aster_draft::truncate(draft_cumulative, sp_conf, k);\n' + ' ' * 20 + anchor, 1)
position = s.rfind(anchor)
s = s[:position] + 'k = aster_draft::truncate(draft_cumulative, sp_conf, k);\n' + ' ' * 12 + s[position:]
(D / 'aster_draft_cumulative.hpp').write_text(header, encoding='utf-8', newline='\n')
(D / 'ds41_generate.cpp').write_text(s, encoding='utf-8', newline='\n')
(D / 'ds41_generate.original.cpp').write_bytes(original)
test = r'''#include "aster_draft_cumulative.hpp"
#include <cassert>
#include <cstdio>
#include <limits>
#include <algorithm>
#include <array>
#include <utility>
#include <vector>
int main() {
    using aster_draft::Mode;
    int checks = 0;
    assert(aster_draft::parse(nullptr) == Mode::off); ++checks;
    for (const auto & row : std::vector<std::pair<const char *,Mode>>{
            {"0",Mode::off},{"0.1",Mode::c010},{"0.25",Mode::c025},{"0.5",Mode::c050}}) {
        assert(aster_draft::parse(row.first) == row.second); ++checks;
    }
    for (const char * raw : {"", "1", "0.10", "0.50", "true", " 0.1", "0.1 ", "NaN", "inf", "-0.1", "0.1junk"}) {
        assert(aster_draft::parse(raw) == Mode::invalid); ++checks;
    }
    const float nan = std::numeric_limits<float>::quiet_NaN();
    const float inf = std::numeric_limits<float>::infinity();
    struct Case { std::array<float,5> logits; std::array<int,3> selected; };
    const std::vector<Case> inputs{
        {{0,0,0,0,0},{3,2,1}}, {{10,10,10,10,10},{5,5,5}},
        {{-10,-10,-10,-10,-10},{0,0,0}}, {{10,-10,10,10,10},{1,1,1}},
        {{inf,inf,inf,inf,inf},{5,5,5}}, {{-inf,0,0,0,0},{0,0,0}},
        {{nan,0,0,0,0},{0,0,0}}, {{10,nan,0,0,0},{1,1,1}},
        {{1,2,3,4,5},{5,5,5}}};
    const Mode modes[] = {Mode::off,Mode::c010,Mode::c025,Mode::c050};
    for (int index=0; index<4; ++index) {
        const Mode mode=modes[index];
        for (const auto & row : inputs) for (int count=0; count<=5; ++count) {
            const int expected = mode==Mode::off ? count : std::min(count,row.selected[index-1]);
            const int actual=aster_draft::truncate(mode,row.logits.data(),count);
            assert(actual==expected && actual>=0 && actual<=count); ++checks;
        }
    }
    float half[5] = {0,0,0,0,0};
    assert(aster_draft::truncate(Mode::c050,half,5)==1); ++checks;
    assert(aster_draft::truncate(Mode::c025,half,5)==2); ++checks;
    assert(aster_draft::truncate(Mode::c010,half,5)==3); ++checks;
    assert(aster_draft::truncate(Mode::off,nullptr,5)==5); ++checks;
    assert(aster_draft::truncate(Mode::c010,nullptr,5)==0); ++checks;
    assert(aster_draft::truncate(Mode::c010,half,6)==0); ++checks;
    assert(aster_draft::truncate(Mode::c010,half,-1)==0); ++checks;
    assert(aster_draft::truncate(Mode::invalid,half,5)==0); ++checks;
    std::printf("PASS %d strict-mode, draft-prefix, nonfinite and boundary checks\n",checks);
}
'''
(D / 'policy_test.cpp').write_text(test, encoding='utf-8', newline='\n')
(D / 'cumulative-draft-v49.patch').write_text(''.join(difflib.unified_diff(original.decode().splitlines(keepends=True), s.splitlines(keepends=True),
    fromfile='v15/ds41_generate.cpp', tofile='v49/ds41_generate.cpp')), encoding='utf-8', newline='\n')
metadata = dict(upstream_commit='3fb0375b227119289acbef1190a92fc1f0a3c5be',
    upstream_diff='xolotl-upstream-dspark-cumulative-v48-diff.json',
    original_generator_sha256=hashlib.sha256(original).hexdigest(),
    patched_generator_sha256=hashlib.sha256(s.encode()).hexdigest(), mode_env='ASTER_XQ_DSPARK_CUM',
    modes=['0','0.1','0.25','0.5'], default='0', insertion_sites=2,
    target_acceptance_changed=False, routing_changed=False, gpu_qualified=False,
    generator_compiled=False, production_promoted=False)
(D / 'source-manifest.json').write_text(json.dumps(metadata, indent=2) + '\n')
print(json.dumps(dict(phase='prepared', **metadata)))
