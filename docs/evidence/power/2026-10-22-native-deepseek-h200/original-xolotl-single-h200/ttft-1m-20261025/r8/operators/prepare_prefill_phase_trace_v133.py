"""Prepare bounded opt-in phase/progress logging for the exact 9a684 generator.

This is observability, not a speed optimization or a changed model operation.
"""
import difflib
import hashlib
import json
from xolotl_trial_common import BASE

ROOT = BASE / 'prefill-phase-trace-v133r1'
parent = BASE / 'pool-seed-v104r2'
original = (parent / 'ds41_generate.cpp').read_bytes()
source = original.decode()
newline = '\r\n' if b'\r\n' in original else '\n'

def replace_once(before, after):
    global source
    before = before.replace('\n', newline)
    after = after.replace('\n', newline)
    if source.count(before) != 1:
        raise RuntimeError('exact prefill observation anchor differs: ' + before[:100])
    source = source.replace(before, after)

replace_once('    bool prefill_qos = false;', '''    bool prefill_phase_trace = false;
    if (!aster_prefill::qos_value(std::getenv("ASTER_XQ_PREFILL_PHASE_TRACE"), prefill_phase_trace)) {
        std::fprintf(stderr, "ASTER_XQ_PREFILL_PHASE_TRACE must be 0 or 1\\n"); return 2;
    }
    unsigned prefill_trace_lines = 0;
    auto trace_prefill = [&](const char * phase, int total, int offset, int width) {
        if (!prefill_phase_trace || total < 131072 || prefill_trace_lines >= 4096) return;
        ++prefill_trace_lines;
        std::fprintf(stderr, "aster prefill phase: phase=%s total=%d offset=%d width=%d monotonic_ms=%.3f\\n",
                     phase, total, offset, width, now_ms());
        std::fflush(stderr);
    };
    bool prefill_qos = false;''')

replace_once('''                    const double need = aster_prefill::reservation_bytes(std::min(dh, PB),
                                        (double) dense.span_graph_bytes(total, SB), a.vram_margin_gib);''', '''                    trace_prefill("keep_graph_begin", total, reused, SB);
                    const double need = aster_prefill::reservation_bytes(std::min(dh, PB),
                                        (double) dense.span_graph_bytes(total, SB), a.vram_margin_gib);
                    trace_prefill("keep_graph_end", total, reused, SB);''')
replace_once('''            tier.rebalance_begin_prompt();
            tier.reset_stats();''', '''            trace_prefill("rebalance_begin", total, reused, dh);
            tier.rebalance_begin_prompt();
            trace_prefill("rebalance_end", total, reused, dh);
            tier.reset_stats();''')
replace_once('''                std::printf("PP %d %d %.0f %.1f\\n", reused + done, total, ms, 1000.0 * done / std::max(ms, 1e-9));''', '''                trace_prefill("progress", total, reused + done, dh);
                std::printf("PP %d %d %.0f %.1f\\n", reused + done, total, ms, 1000.0 * done / std::max(ms, 1e-9));''')
replace_once('''                size_t frb = std::numeric_limits<size_t>::max(), totb = 0;''', '''                trace_prefill("memory_query_begin", total, reused, dh);
                size_t frb = std::numeric_limits<size_t>::max(), totb = 0;''')
replace_once('''                if (frb == std::numeric_limits<size_t>::max()) frb = 0;''', '''                if (frb == std::numeric_limits<size_t>::max()) frb = 0;
                trace_prefill("memory_query_end", total, reused, dh);''')
replace_once('''                const double g_need = pb_graph ? (double) dense.span_graph_bytes(total, SB) : 0.0;   // memoized per row count''', '''                trace_prefill("cap_graph_begin", total, reused, SB);
                const double g_need = pb_graph ? (double) dense.span_graph_bytes(total, SB) : 0.0;   // memoized per row count
                trace_prefill("cap_graph_end", total, reused, SB);''')
replace_once('''        feed_stopped = -1;
        for (int c0 = 0; c0 < n; ) {''', '''        feed_stopped = -1;
        trace_prefill("feed_begin", pos0 + n, pos0, n);
        for (int c0 = 0; c0 < n; ) {''')
replace_once('''                Ds41Dense & batch_dense = pipe.enabled() ? pipe.dense(0) : dense;
                for (;;) {''', '''                Ds41Dense & batch_dense = pipe.enabled() ? pipe.dense(0) : dense;
                const bool trace_batch = c0 < 2 * pb_cap || c0 + B >= n;
                for (;;) {''')
replace_once('''                    if (!set_cols(B)) {''', '''                    if (trace_batch) trace_prefill("columns_begin", pos0 + n, pos0 + c0, B);
                    const bool columns_ok = set_cols(B);
                    if (trace_batch) trace_prefill("columns_end", pos0 + n, pos0 + c0, B);
                    if (!columns_ok) {''')
replace_once('''                    if (batch_dense.begin_batch(ids + c0, B, pos0 + c0)) break;''', '''                    if (trace_batch) trace_prefill("inputs_begin", pos0 + n, pos0 + c0, B);
                    const bool inputs_ok = batch_dense.begin_batch(ids + c0, B, pos0 + c0);
                    if (trace_batch) trace_prefill("inputs_end", pos0 + n, pos0 + c0, B);
                    if (inputs_ok) break;''')
replace_once('''                double t0 = now_ms();
                Ds41Dense * cur_dense = &pipe.dense(0);
                Ds41MoeTier * cur_tier = &pipe.tier(0);
                int cur_st = 0;
                for (int l = 0; l < n_layer; ++l) {
                    if (stop_prefill()) return false;''', '''                double t0 = now_ms();
                Ds41Dense * cur_dense = &pipe.dense(0);
                Ds41MoeTier * cur_tier = &pipe.tier(0);
                int cur_st = 0;
                for (int l = 0; l < n_layer; ++l) {
                    if (trace_batch && (l == 0 || l == 1 || l == 14 || l == n_layer - 1))
                        trace_prefill("layer_begin", pos0 + n, pos0 + c0, l);
                    if (stop_prefill()) return false;''')

ROOT.mkdir(exist_ok=False)
files = {'ds41_generate.cpp': source.encode(), 'ds41_generate.original.cpp': original,
         'prefill-phase-trace-v133.patch': ''.join(difflib.unified_diff(
             original.decode().splitlines(True), source.splitlines(True),
             fromfile='a/tools/v41/engine/ds41_generate.cpp',
             tofile='b/tools/v41/engine/ds41_generate.cpp')).encode()}
for path in parent.iterdir():
    if path.suffix == '.hpp':
        files[path.name] = path.read_bytes()
for name, raw in files.items():
    (ROOT / name).write_bytes(raw)
manifest = dict(schema='aster-xolotl-prefill-phase-trace-v133',
    parent_source_sha256=hashlib.sha256(original).hexdigest(), parent_native_sha256='9a684b2442d4b648fff6a73024de5b7bab096657fe45897bf036c7e216895bbf',
    change='Default-off bounded numeric phase/progress trace for >=131072-token requests; same model calls and order.',
    maximum_trace_lines_per_process=4096, minimum_trace_input_tokens=131072,
    env_flag='ASTER_XQ_PREFILL_PHASE_TRACE', closed_values=['0','1'], default_enabled=False,
    model_operation_order_changed=False, gpu_qualified=False, performance_claimed=False,
    files=[dict(file=name, bytes=len(raw), sha256=hashlib.sha256(raw).hexdigest()) for name, raw in files.items()])
(ROOT / 'source-manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
print(json.dumps(dict(phase='prepared', files=len(files), parent_source_sha256=manifest['parent_source_sha256'], gpu_executed=False)))
