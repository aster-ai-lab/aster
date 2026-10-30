"""Add a bounded default-off profiler window to the fourth GEN, after its prefill."""
import difflib
import hashlib
import json
from xolotl_trial_common import BASE

root=BASE/'serve-decode-profile-v166'
parent=BASE/'prefill-phase-trace-v133r1'
original=(parent/'ds41_generate.cpp').read_bytes()
manifest=json.loads((parent/'source-manifest.json').read_text())
expected=next(row for row in manifest['files'] if row['file']=='ds41_generate.cpp')
if len(original)!=expected['bytes'] or hashlib.sha256(original).hexdigest()!=expected['sha256']:
    raise RuntimeError('exact native99f3 traced parent source differs')
text=original.decode();newline='\r\n' if b'\r\n' in original else '\n'
def change(before,after):
    global text
    before=before.replace('\n',newline);after=after.replace('\n',newline)
    if text.count(before)!=1:raise RuntimeError('unique native GEN profiler anchor differs: '+before[:90])
    text=text.replace(before,after)
change('#include "aster_dspark_seed.hpp"','#include "aster_dspark_seed.hpp"\n#include "aster_serve_profile.hpp"')
change('    bool prefill_phase_trace = false;', '''    bool serve_profile = false;
    if (!aster_profile::flag(std::getenv("ASTER_XQ_SERVE_PROFILE"), serve_profile)) {
        std::fprintf(stderr, "ASTER_XQ_SERVE_PROFILE must be 0 or 1\\n"); return 2;
    }
    aster_profile::FourthGen profile_counter(serve_profile);
    bool prefill_phase_trace = false;''')
change('''            dense.route_meter(true);
            const double td0 = now_ms();''','''            dense.route_meter(true);
            const bool profiling_gen = profile_counter.next();
            aster_profile::DecodeWindow profiling(profiling_gen, &Ds41MoeTier::profiler);
            if (profiling_gen) std::fprintf(stderr, "aster serve profile: fourth_GEN_decode_begin\\n");
            const double td0 = now_ms();''')
change('''            const double dec_ms = now_ms() - td0;
            const Ds41MoeStats st = tier.stats();''','''            const double dec_ms = now_ms() - td0;
            profiling.finish();
            if (profiling_gen) std::fprintf(stderr, "aster serve profile: fourth_GEN_decode_end\\n");
            const Ds41MoeStats st = tier.stats();''')
header='''#pragma once
#include <cstring>

namespace aster_profile {
inline bool flag(const char * value, bool & enabled) {
    if (value == nullptr || std::strcmp(value, "0") == 0) { enabled = false; return true; }
    if (std::strcmp(value, "1") == 0) { enabled = true; return true; }
    return false;
}
class FourthGen {
    bool enabled_;
    unsigned seen_ = 0;
public:
    explicit FourthGen(bool enabled) : enabled_(enabled) {}
    bool next() {
        if (!enabled_ || seen_ >= 4) return false;
        return ++seen_ == 4;
    }
};
class DecodeWindow {
    void (*callback_)(bool);
public:
    DecodeWindow(bool enabled, void (*callback)(bool)) : callback_(enabled ? callback : nullptr) {
        if (callback_) callback_(true);
    }
    DecodeWindow(const DecodeWindow &) = delete;
    DecodeWindow & operator=(const DecodeWindow &) = delete;
    DecodeWindow(DecodeWindow &&) = delete;
    DecodeWindow & operator=(DecodeWindow &&) = delete;
    void finish() {
        const auto callback = callback_;
        callback_ = nullptr;
        if (callback) callback(false);
    }
    ~DecodeWindow() { finish(); }
};
}
'''
test='''#include "aster_serve_profile.hpp"
#include <cstdio>
#include <stdexcept>
#include <type_traits>
#include <vector>
static std::vector<bool> calls;
static void callback(bool on) { calls.push_back(on); }
static unsigned cases = 0;
static void check(bool value) { ++cases; if (!value) throw std::runtime_error("profile lifecycle contract"); }
static void early_return(bool enabled) { aster_profile::DecodeWindow window(enabled, callback); }
int main() {
    bool enabled = true;
    check(aster_profile::flag(nullptr, enabled) && !enabled);
    check(aster_profile::flag("0", enabled) && !enabled);
    check(aster_profile::flag("1", enabled) && enabled);
    for (const char * value : {"", "2", "true", " 1", "1 ", "01", "-1", "false"})
        check(!aster_profile::flag(value, enabled));
    aster_profile::FourthGen off(false), on(true);
    for (unsigned index = 1; index <= 10000; ++index) {
        check(!off.next());
        check(on.next() == (index == 4));
    }
    early_return(false);check(calls.empty());
    early_return(true);check(calls == std::vector<bool>({true,false}));
    calls.clear();
    { aster_profile::DecodeWindow window(true,callback);window.finish();window.finish(); }
    check(calls == std::vector<bool>({true,false}));
    calls.clear();
    try { aster_profile::DecodeWindow window(true,callback);throw std::runtime_error("early failure"); }
    catch (const std::runtime_error &) {}
    check(calls == std::vector<bool>({true,false}));
    static_assert(!std::is_copy_constructible_v<aster_profile::DecodeWindow>);
    static_assert(!std::is_move_constructible_v<aster_profile::DecodeWindow>);
    std::printf("{\\"phase\\":\\"passed\\",\\"contracts\\":%u,\\"gpu_executed\\":false}\\n", cases);
}
'''
root.mkdir(exist_ok=False)
files={'ds41_generate.cpp':text.encode(),'ds41_generate.original.cpp':original,'aster_serve_profile.hpp':header.encode(),
       'test_serve_profile.cpp':test.encode(),'serve-decode-profile-v166.patch':''.join(difflib.unified_diff(original.decode().splitlines(True),text.splitlines(True),fromfile='a/tools/v41/engine/ds41_generate.cpp',tofile='b/tools/v41/engine/ds41_generate.cpp')).encode()}
for path in parent.glob('*.hpp'):files[path.name]=path.read_bytes()
for name,raw in files.items():(root/name).write_bytes(raw)
record=dict(schema='aster-serve-decode-profile-v166',parent_source_sha256=expected['sha256'],parent_generator_sha256='8ab829c4157968935bb4f95b31b73ebd1b60c6fb3fa6e6db144c30fc496c3715',
    parent_native_sha256='99f3eee8c7eeee54c304120db50e8e632a7c001f9ac90fadf0983040fef92c25',
    env='ASTER_XQ_SERVE_PROFILE',closed_values=['0','1'],default_enabled=False,profiled_GEN_number=4,
    prefill_profiled=False,routing_changed=False,model_math_changed=False,gpu_executed=False,
    files=[dict(file=name,bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest()) for name,raw in files.items()])
(root/'source-manifest.json').write_text(json.dumps(record,indent=2)+'\n')
print(json.dumps(dict(phase='prepared',files=len(files),gpu_executed=False,model_math_changed=False)))
