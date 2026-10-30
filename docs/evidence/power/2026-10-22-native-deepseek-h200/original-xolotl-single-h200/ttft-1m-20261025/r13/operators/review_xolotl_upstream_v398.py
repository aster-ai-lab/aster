"""Pin applicability of recent upstream changes without replacing an active experiment."""
import hashlib,json,time
from pathlib import Path
from xolotl_trial_common import BASE
fetch=json.loads((BASE/'xolotl-upstream-fetch-v397.json').read_text());root=Path(fetch['root']);comparison=json.loads((BASE/'xolotl-upstream-comparison-v396.json').read_text())
if fetch['commit']!=comparison['head'] or fetch['phase']!='fetched_exact':raise RuntimeError('exact fetched upstream comparison differs')
paths=['tools/v41/engine/ds41_generate.cpp','tools/v41/engine/ds41_moe.cpp','tools/v41/engine/ds41_dense.cpp','docs/ds4/V41_DECODE_PROFILE.md'];pins=[]
for name in paths:
 row=next(r for r in fetch['files'] if r['file']==name);raw=(root/name).read_bytes()
 if len(raw)!=row['bytes'] or hashlib.sha256(raw).hexdigest()!=row['sha256']:raise RuntimeError('exact reviewed upstream source differs')
 pins.append(row)
generate=(root/paths[0]).read_text();moe=(root/paths[1]).read_text();old=(BASE/'serve-decode-profile-v166/ds41_generate.original.cpp').read_text()
for text,anchor in [(generate,'static const int batch_min = std::getenv("STRATA_SERVE_BATCH_MIN") ? std::atoi(std::getenv("STRATA_SERVE_BATCH_MIN")) : 16;'),(old,'static const int batch_min = std::getenv("STRATA_SERVE_BATCH_MIN") ? std::atoi(std::getenv("STRATA_SERVE_BATCH_MIN")) : 128;'),(moe,'if (im_->mixed_sizes && im_->gpu && !im_->gpu->cache && want_cache) {')]:
 if anchor not in text:raise RuntimeError('reviewed exact source precondition differs')
out=dict(unix=time.time(),phase='reviewed',base=comparison['base'],head=fetch['commit'],ahead_commits=comparison['total_commits'],archive_sha256=fetch['archive_sha256'],reviewed_sources=pins,
 changes=[dict(name='Short turn layer-major batches',upstream_commit='f9155bac0d8a496e9f8cf4eb722abf53d71b6e97',old_default=128,new_default=16,flag='STRATA_SERVE_BATCH_MIN',old_native_already_supports_explicit_flag=True,H200_GPU_tested=False,
 next_test='Same native and GPU128/16/128; 32/50/85 new-token multi-turn fixtures plus native tools, exact output, cache isolation, and unchanged full acceptance. Report TTFT separately from completion throughput.'),
 dict(name='Conversation-seeded VRAM',upstream_commit='35f0f36a90d019cc6f912d5187c507c323945223',flag='STRATA_VRAM_CONV',requires_mixed_expert_blob_sizes=True,current_V3_applicability_proven=False,H200_gain_claimed=False),
 dict(name='Periodic route-bias refresh',flags=['STRATA_BIAS_REFRESH'],changes_routing_when_enabled_with_nonzero_bias=True,current_route_bias=0,current_route_bias_file=0,H200_gain_claimed=False),
 dict(name='CPU PTQ1/Q2 rows SIMD',requires_CPU_expert_work=True,current_ordinary_numeric_diagnostics_CPU_expert_calls=0,H200_decode_gain_proven=False),
 dict(name='Layer read-ahead and direct aligned reads',flags=['STRATA_LAYER_RA','STRATA_READ_BOUNCE'],requires_expert_file_tier_IO=True,Engram_IO_is_separate=True,actual1M_applicability_pending_trace=True,H200_gain_claimed=False),
 dict(name='Conditional DSpark eviction',flag='STRATA_SERVE_DSPARK_EVICT_FIT',purpose='Avoid unneeded eviction/reload on turns fitting one sweep',current_H200_applicability_pending=True)],
 source_diagnostic_scope='Upstream document measures RTX5090, Windows, V2/Q6_K, route bias0.5/0.3. It is not an H200 measurement.',
 source_diagnostic_followups=['Fuse dense small kernels','Remove per-layer host round trips'],
 active_ABA375_runtime_changed=False,formal_service_changed=False,new_H200_speed_gain_proven=False,goal_complete=False)
with (BASE/'xolotl-upstream-applicability-v398.json').open('x') as f:f.write(json.dumps(out,indent=2)+'\n')
print(json.dumps(dict(reviewed=True,upstream_commits=out['ahead_commits'],next_target='STRATA_SERVE_BATCH_MIN128/16/128 TTFT comparison',H200_gain_proven=False)))
