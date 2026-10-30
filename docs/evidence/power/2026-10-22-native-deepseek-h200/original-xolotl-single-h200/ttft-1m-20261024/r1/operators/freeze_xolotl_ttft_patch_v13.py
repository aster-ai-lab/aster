"""Retain large steady prefill batches with an exact, bounded initial scheduling window."""
import difflib,hashlib,json
from pathlib import Path
BASE=Path('D:/code/aster/target/h200-native');previous=BASE/'ttft-v11';root=BASE/'ttft-v13';root.mkdir(exist_ok=True)
original=(previous/'ds41_generate.cpp').read_text();s=original
def replace(old,new):
    global s
    if s.count(old)!=1:raise RuntimeError('unique QoS source anchor differs: '+old[:80])
    s=s.replace(old,new)
replace('int main(int argc, char ** argv) {','''int main(int argc, char ** argv) {
    bool prefill_qos = false;
    if (!aster_prefill::qos_value(std::getenv("ASTER_XQ_PREFILL_QOS"), prefill_qos)) {
        std::fprintf(stderr, "ASTER_XQ_PREFILL_QOS must be 0 or 1\\n"); return 2;
    }''')
replace('''                int B = std::min(pb_cap, n - c0);''','''                int B = std::min(pb_cap, n - c0);
                if (prefill_qos) B = aster_prefill::initial_width(B, c0, n);
                // Poll control before allocating columns. A waiting short request
                // needs only the exact replay floor, rather than another full batch.
                if (feed_yield && c0 + B < n) {
                    const int cap = prefill_qos ? aster_prefill::yield_width(B, c0, SB, enc_skip ? ER : 0) : B;
                    if (feed_yield(c0, cap)) { B = cap; yield_here = true; }
                }''')
replace('''                if (feed_yield && c0 + B < n) yield_here = feed_yield(c0, B);''','''                // An allocation fallback below the replay floor cannot publish a
                // partial prefix; finish the read and release the reservation normally.
                if (yield_here && enc_skip && c0 > 0 && B < ER) yield_here = false;''')
(root/'ds41_generate.cpp').write_text(s,newline='\n')
header=(previous/'aster_prefill_policy.hpp').read_text();pos=header.rfind('\n}')
header=header[:pos]+'''
inline bool qos_value(const char * raw, bool & enabled) {
    enabled = false;
    if (!raw) return true;
    if (raw[0] == '0' && raw[1] == 0) return true;
    if (raw[0] == '1' && raw[1] == 0) { enabled = true; return true; }
    return false;
}
inline int initial_width(int normal, int begin, int total) {
    return begin == 0 && total > 8192 ? std::min(normal, 1024) : normal;
}
inline int yield_width(int normal, int begin, int sub_batch, long long replay) {
    const long long floor = begin == 0 ? 1024 : std::max<long long>(sub_batch, replay);
    return (int) std::min<long long>(normal, floor);
}
'''+header[pos:]
(root/'aster_prefill_policy.hpp').write_text(header,newline='\n')
test=(previous/'policy_test.cpp').read_text();anchor='    std::printf("PASS %d tail coverage and context graph budget cases\\n", cases);'
if test.count(anchor)!=1:raise RuntimeError('policy test result anchor differs')
extra='''    bool enabled = true;
    assert(aster_prefill::qos_value(nullptr, enabled) && !enabled);
    assert(aster_prefill::qos_value("0", enabled) && !enabled);
    assert(aster_prefill::qos_value("1", enabled) && enabled);
    for (const char * bad : {"", "2", "true", " 1", "1x"}) assert(!aster_prefill::qos_value(bad, enabled));
    assert(aster_prefill::initial_width(8192, 0, 35700) == 1024);
    assert(aster_prefill::initial_width(8192, 1024, 35700) == 8192);
    assert(aster_prefill::initial_width(128, 0, 35700) == 128);
    assert(aster_prefill::initial_width(400, 0, 400) == 400);
    assert(aster_prefill::yield_width(8192, 1024, 128, 2560) == 2560);
    assert(aster_prefill::yield_width(8192, 0, 128, 2560) == 1024);
    assert(aster_prefill::yield_width(128, 1024, 128, 2560) == 128);
    // Check exact tail coverage after a small first chunk and a replay-width handoff.
    for (int total : {8193, 35700, 131073, 1048512}) {
        std::vector<int> seen(128);
        for (int begin = 0; begin < total; ) {
            int width = std::min(8192, total - begin);
            width = aster_prefill::initial_width(width, begin, total);
            if (begin == 1024) width = aster_prefill::yield_width(width, begin, 128, 2560);
            const auto s = aster_prefill::tail_slice(total, 128, begin, width);
            for (int j = 0; j < s.count; ++j) ++seen[s.destination + j];
            begin += width;
        }
        for (int n : seen) assert(n == 1);
        ++cases;
    }
    std::printf("PASS %d tail/budget/window cases plus strict QoS flag and replay-floor checks\\n", cases);'''
test=test.replace(anchor,extra);(root/'policy_test.cpp').write_text(test,newline='\n')
(root/'build_worker.py').write_bytes((previous/'build_worker.py').read_bytes())
(root/'prefill-v11.patch').write_text(''.join(difflib.unified_diff((BASE/'xolotl-ttft-v11-ds41_generate.original.cpp').read_text().splitlines(keepends=True),s.splitlines(keepends=True),fromfile='a/tools/v41/engine/ds41_generate.cpp',tofile='b/tools/v41/engine/ds41_generate.cpp')),newline='\n')
(root/'prefill-qos-v13.patch').write_text(''.join(difflib.unified_diff(original.splitlines(keepends=True),s.splitlines(keepends=True),fromfile='v11/ds41_generate.cpp',tofile='v13/ds41_generate.cpp')),newline='\n')
print(json.dumps(dict(phase='frozen',source_sha256=hashlib.sha256(s.encode()).hexdigest(),qos_default=False,enabled_profile_first_batch=1024,yield_min_replay=2560)))
