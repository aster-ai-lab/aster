"""Freeze a checked, opt-in D2D resident-expert MMQ candidate and its gates."""
import ast
import difflib
import hashlib
import json
from pathlib import Path

B = Path('D:/code/aster/target/h200-native')
R = B / 'resident-mmq-v35'
R.mkdir(exist_ok=False)
original = (B / 'ttft-v19/ds41_moe.cpp').read_text()
text = original

def replace_once(old, new):
    global text
    if text.count(old) != 1:
        raise RuntimeError('exact V19 source anchor differs: ' + old[:80])
    text = text.replace(old, new)

header = r'''// Pure bounds and half-ownership policy; no CUDA and no inferred resource sizes.
#pragma once
#include <cstddef>
#include <cstdint>
namespace aster_resident_stage {
struct Plan {
    bool valid = false;
    size_t slots = 0;
    int first_half = 0;
    bool alternate = false;
    int half(size_t wave) const { return alternate ? static_cast<int>((wave + first_half) & 1) : first_half; }
};
inline Plan plan(int64_t blob, int64_t stride, int64_t half_bytes, int reserved_half) {
    if (blob <= 0 || stride < blob || half_bytes < stride || reserved_half < -1 || reserved_half > 1) return {};
    return {true, static_cast<size_t>(half_bytes / stride), reserved_half < 0 ? 0 : 1 - reserved_half, reserved_half < 0};
}
} // namespace aster_resident_stage
'''
test = r'''#include "aster_resident_stage_policy.hpp"
#include <cstdio>
#include <cstdlib>
#include <cstdint>
int main() {
    int checked = 0;
    auto require = [&](bool ok) { if (!ok) std::exit(1); ++checked; };
    for (int reserved : {-1, 0, 1}) {
        const auto p = aster_resident_stage::plan(13, 16, 47, reserved);
        require(p.valid && p.slots == 2);
        for (size_t wave = 0; wave < 7; ++wave) require(p.half(wave) >= 0 && p.half(wave) <= 1 && p.half(wave) != reserved);
    }
    const auto p = aster_resident_stage::plan(16, 16, 16, -1);
    require(p.valid && p.slots == 1 && p.half(0) == 0 && p.half(1) == 1);
    for (int64_t b : {int64_t(-1), int64_t(0)}) require(!aster_resident_stage::plan(b, 16, 32, -1).valid);
    require(!aster_resident_stage::plan(17, 16, 32, -1).valid);
    require(!aster_resident_stage::plan(16, 16, 15, -1).valid);
    require(!aster_resident_stage::plan(16, 16, 32, 2).valid);
    require(!aster_resident_stage::plan(16, 16, 32, -2).valid);
    const auto huge = aster_resident_stage::plan(INT64_MAX, INT64_MAX, INT64_MAX, 0);
    require(huge.valid && huge.slots == 1 && huge.half(3) == 1);
    std::printf("PASS resident stage policy: %d ownership and bounds checks\n", checked);
}
'''.replace('#include <cstdint>', '#include <cstdint>\n#include <initializer_list>')
replace_once('#include "aster_cache_restore_policy.hpp"', '#include "aster_cache_restore_policy.hpp"\n#include "aster_resident_stage_policy.hpp"')
replace_once('mmq_subruns = 0; };', 'mmq_subruns = 0; int64_t resident_pack_waves = 0, resident_pack_bytes = 0; };')
anchor = '''    launch(run_e, run_p);
    in_resident = false;'''
replacement = r'''    int resident_last_half = -1;
#if defined(DS4_MOE_MMQ)
    // Explicit opt-in. Resident blobs are scattered ranked slots; MMQ needs one
    // uniform stride. Repack only their exact bytes into the existing DMA halves.
    // A half containing this layer's already-prestaged arena experts is reserved.
    static const int resident_mmq = [] {
        const char* value = std::getenv("ASTER_XQ_RESIDENT_MMQ");
        if (!value || std::strcmp(value, "0") == 0) return 0;
        if (std::strcmp(value, "1") == 0) return 1;
        return -1;
    }();
    if (resident_mmq < 0) {
        std::fprintf(stderr, "ds4_moe: ASTER_XQ_RESIDENT_MMQ must be 0 or 1\n");
        return false;
    }
    bool resident_strided = true;
    for (size_t q = 1; q < run_p.size(); ++q)
        resident_strided &= run_p[q] == run_p[0] + q * static_cast<unsigned long long>(SL);
    if (resident_mmq && use_mmq && !resident_strided) {
        const int reserved = gp.c_pre_layer == static_cast<int>(layer) && gp.c_pre_sl == SL ? gp.c_pre_half : -1;
        const auto packing = aster_resident_stage::plan(BL, SL, gp.c_half, reserved);
        if (!packing.valid) {
            std::fprintf(stderr, "ds4_moe: resident MMQ stage bounds/ownership refused\n");
            return false;
        }
        for (size_t begin = 0, wave = 0; begin < run_e.size(); ++wave) {
            const size_t end = begin + std::min(packing.slots, run_e.size() - begin);
            const int half = packing.half(wave);
            ck(cudaStreamWaitEvent(gp.s_cp, gp.c_used[half], 0), "resident stage wait used");
            std::vector<int32_t> experts;
            std::vector<unsigned long long> pointers;
            for (size_t q = begin; q < end; ++q) {
                uint8_t* destination = static_cast<uint8_t*>(gp.c_stage[half]) + (q - begin) * static_cast<size_t>(SL);
                ck(cudaMemcpyAsync(destination, reinterpret_cast<const void*>(run_p[q]), static_cast<size_t>(BL),
                                   cudaMemcpyDeviceToDevice, gp.s_cp), "resident stage d2d");
                experts.push_back(run_e[q]);
                pointers.push_back(reinterpret_cast<unsigned long long>(destination));
            }
            ck(cudaEventRecord(gp.c_copied[half], gp.s_cp), "resident stage copied");
            ck(cudaStreamWaitEvent(gp.s, gp.c_copied[half], 0), "resident stage wait copied");
            launch(experts, pointers);
            ck(cudaEventRecord(gp.c_used[half], gp.s), "resident stage used");
            resident_last_half = half;
            if (prof) {
                ++g_cprof.resident_pack_waves;
                g_cprof.resident_pack_bytes += static_cast<int64_t>(end - begin) * BL;
            }
            begin = end;
        }
    } else
#endif
    {
        launch(run_e, run_p);
    }
    in_resident = false;'''
replace_once(anchor, replacement)
replace_once('int h_last = used_pre ? 1 - h0 : -1;', 'int h_last = used_pre ? 1 - h0 : resident_last_half;')
replace_once('        g_cprof = ChunkProf{};', '''        std::fprintf(stderr, "resident MMQ stage: %lld waves, %lld exact D2D bytes\\n",
                     (long long) c.resident_pack_waves, (long long) c.resident_pack_bytes);
        g_cprof = ChunkProf{};''')
(R/'ds41_moe.cpp').write_text(text, newline='\n')
(R/'aster_resident_stage_policy.hpp').write_text(header, newline='\n')
(R/'resident_stage_policy_test.cpp').write_text(test, newline='\n')
for name in ['aster_cache_restore_policy.hpp', 'cache_policy_test.cpp']:
    (R/name).write_bytes((B/'ttft-v19'/name).read_bytes())
(R/'resident-mmq-v35.patch').write_text(''.join(difflib.unified_diff(original.splitlines(True), text.splitlines(True), fromfile='v19/ds41_moe.cpp', tofile='v35/ds41_moe.cpp')), newline='\n')
# Preserve the upstream F32 dequant reference and its 3e-2 activation-rounding
# gate. Only force scattered resident ownership for both tested layers.
upstream = B/'xolotl-trial-source-e9d0826d71c5/source/tools/v41/engine/test_ds41_moe.cpp'
raw = upstream.read_text()
if raw.count('cfg.slots = 64;') != 1 or raw.count('for (int l : layers) for (int e = 0; e < g.n_experts; ++e) ranked.emplace_back(l, e);') != 1:
    raise RuntimeError('exact upstream real-weight reference gate differs')
gate = raw.replace('cfg.slots = 64;', 'cfg.slots = 768;')
gate = gate.replace('for (int l : layers) for (int e = 0; e < g.n_experts; ++e) ranked.emplace_back(l, e);', 'for (int l : layers) for (int parity : {1, 0}) for (int e = parity; e < g.n_experts; e += 2) ranked.emplace_back(l, e);')
(R/'test_resident_mmq.cpp').write_text(gate, newline='\n')
(R/'upstream-test_ds41_moe.cpp').write_bytes(upstream.read_bytes())
worker = (B/'ttft-v19/build_worker.py').read_text()
anchor = "    candidate = list(link)"
worker = worker.replace(anchor, '''    run(['/usr/bin/c++', '-O2', '-std=c++20', '-Wall', '-Wextra', '-Werror', str(ROOT / 'resident_stage_policy_test.cpp'), '-o', str(ROOT / 'resident_stage_policy_test')], 'resident-stage-policy-compile', 15, ROOT)
    run([str(ROOT / 'resident_stage_policy_test')], 'resident-stage-policy-test', 10, ROOT)
'''+anchor)
anchor = "    STATE.update(phase='built', cache_policy_tests_passed=True,"
worker = worker.replace(anchor, '''    gate = list(candidate)
    gate[gate.index(str(baseline / 'ds41_generate.cpp.o'))] = str(ROOT / 'test_resident_mmq.cpp')
    gate[gate.index('-o') + 1] = str(ROOT / 'test_resident_mmq')
    gate[1:1] = ['-std=c++20', '-I'+str(parent/'source/include'), '-I'+str(parent/'source/tools/v41/engine'), '-I'+str(parent/'source/third_party/llama.cpp/ggml/include')]
    run(gate, 'real-weight-gate-compile', 45, parent / 'build')
    STATE['real_weight_gate_binary'] = dict(path=str(ROOT/'test_resident_mmq'), sha256=sha(ROOT/'test_resident_mmq'))
    STATE['resident_stage_policy_tests_passed'] = True
'''+anchor)
ast.parse(worker)
(R/'build_worker.py').write_text(worker, newline='\n')
manifest = {'schema':'aster-xolotl-resident-mmq-v35-source', 'parent_candidate':'checked-cache-v19', 'upstream_reference_test_sha256':hashlib.sha256(upstream.read_bytes()).hexdigest(), 'v19_source_sha256':hashlib.sha256((B/'ttft-v19/ds41_moe.cpp').read_bytes()).hexdigest(), 'new_gpu_kernels':False, 'additional_vram_buffers':False, 'resident_flag':'ASTER_XQ_RESIDENT_MMQ=0|1', 'files':[]}
for path in sorted(R.iterdir()):
    manifest['files'].append(dict(file=path.name,bytes=path.stat().st_size,sha256=hashlib.sha256(path.read_bytes()).hexdigest()))
(R/'source-manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
print(json.dumps({'prepared':True, 'root':str(R), 'source_files':len(manifest['files'])}))
