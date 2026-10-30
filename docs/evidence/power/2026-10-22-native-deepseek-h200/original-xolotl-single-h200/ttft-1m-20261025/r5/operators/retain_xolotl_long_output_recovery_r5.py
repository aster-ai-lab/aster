"""Append immutable completed1M-long-output, rejected draft, API recovery and storage evidence."""
import hashlib
import json
import subprocess
import time
from pathlib import Path

B = Path('D:/code/aster/target/h200-native')
W = Path('D:/code/aster/.wt-h200-native-integration')
E = W / 'docs/evidence/power/2026-10-22-native-deepseek-h200/original-xolotl-single-h200'
D = E / 'ttft-1m-20261025/r5'
if D.exists() or subprocess.check_output(['git', 'status', '--porcelain'], cwd=W):
    raise RuntimeError('immutable R5 requires an exclusive clean integration worktree')


def read(name):
    return json.loads((B / name).read_text(encoding='utf-8-sig'))


full = read('xolotl-indexer-fullwindow-suite-v45-latest.json')
width = read('xolotl-dense-subbatch-v47r1-controller.json')
long = read('xolotl-fullwindow-long-suite-v56-latest.json')
post = read('xolotl-post-concurrency-v66-latest.json')
promotion = read('xolotl-stable-recovery-v58-promotion.json')
stable = read('xolotl-stable-service-v58-intent.json')
local = read('xolotl-engram-local-copy-v63-latest.json')
storage = read('xolotl-engram-storage-probe-v64-latest.json')
cli = read('workbuddy-stream-e2e-v61-result.json')
tool = read('workbuddy-native-read-e2e-v60-result.json')
if (full['suite-result.json']['phase'] != 'passed' or full['exact_client_alive']
        or width['phase'] != 'passed' or sum(row['tests'] for row in width['profiles']) != 84
        or long['suite-result.json']['phase'] != 'failed' or long['exact_client_alive']
        or len(long['fullwindow/request-result.json']['tests']) != 20
        or long['fullwindow/request-result.json']['error'] != 'ValueError: full-window long-output identity differs'
        or post['result']['phase'] != 'passed' or post['exact_client_alive']
        or not post['result']['post_concurrent_single_matches_cold']
        or promotion['phase'] != 'promoted' or cli['phase'] != 'passed' or tool['phase'] != 'passed'
        or local['result']['phase'] != 'passed' or local['exact_copy_process_alive']
        or storage['result']['phase'] != 'passed' or storage['exact_probe_process_alive']):
    raise RuntimeError('exact completed gates and retained concurrent failure required')
q = long['fullwindow/request-result.json']
if (not q['full1m_cache_restore_passed'] or not q['exact_restored_long_output_matched']
        or q['fixture']['actual_prompt_tokens'] != 1047544
        or q['levels'][0]['requests'] != 8 or len(q['levels']) != 1):
    raise RuntimeError('completed actual1M-plus1024 scope differs')
keys = []
for path in B.glob('access-private-*/api-key.txt'):
    raw = path.read_bytes().strip()
    if len(raw) >= 24 and raw not in keys:
        keys.append(raw)
if not keys:
    raise RuntimeError('actual credential negative controls required')
manifest_path = E / 'manifest.json'
original = manifest_path.read_bytes()
manifest = json.loads(original)
old_rows = list(manifest['files'])
for row in old_rows:
    raw = (E / row['file']).read_bytes()
    if len(raw) != row['bytes'] or hashlib.sha256(raw).hexdigest() != row['sha256']:
        raise RuntimeError('historical raw evidence changed: ' + row['file'])
files = []
D.mkdir(parents=True)


def put(raw, name, source=None):
    if len(raw) > 2 * 1024**2 or any(key in raw for key in keys):
        raise RuntimeError('bounded public credential gate refused: ' + name)
    path = D / name
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open('xb') as output:
        output.write(raw)
    files.append(dict(file=name, bytes=len(raw), sha256=hashlib.sha256(raw).hexdigest(), source=source))


def copy(name, target):
    put((B / name).read_bytes(), target, name)


def data(value, name):
    put((json.dumps(value, indent=2, ensure_ascii=False) + '\n').encode(), name)


for folder in ['draft-cumulative-v49', 'api-adapter-v52']:
    for path in sorted((B / folder).iterdir()):
        if not path.is_file():
            continue
        if path.suffix not in ['.cpp', '.cu', '.cuh', '.hpp', '.py', '.json', '.patch']:
            raise RuntimeError('unreviewed candidate artifact: ' + str(path))
        copy(folder + '/' + path.name, 'frozen-candidates/' + folder + '/' + path.name)
patterns = [
    'xolotl-indexer-fullwindow-suite-v45-*.json', 'xolotl-indexer-full1m-v45-*.json',
    'xolotl-indexer-v45-drain.json', 'xolotl-dense-subbatch-v47*.json',
    'xolotl-upstream-*-v48-*.json', 'xolotl-draft-*-v49-*.json',
    'xolotl-draft-*-v50-*.json', 'xolotl-*-v51-*.json', 'xolotl-api-hybrid-v52-*.json',
    'xolotl-draft-*-v53-*.json', 'xolotl-draft-recovery-v55-*.json',
    'xolotl-fullwindow-long-fixture-v54*.json', 'xolotl-fullwindow-long-suite-v56-*.json',
    'xolotl-fullwindow-long-v56-postmortem.json', 'xolotl-stable-service-v57-network-diagnostic.json',
    'xolotl-stable-v31-early-stop-v57-reason.json', 'xolotl-single1m-cont8-lease-pending.json',
    'xolotl-cont8-resource-v58-latest.json', 'xolotl-stable-extended-v58-*.json',
    'xolotl-stable-recovery-v58-*.json', 'xolotl-stable-service-v58-intent.json',
    'xolotl-stable-v58-public-parent.json', 'xolotl-windows-models-v58.json',
    'xolotl-stable-edge-suite-v59-*.json', 'xolotl-workbuddy-*-v62-*.json',
    'xolotl-engram-local-*-v63-*.json', 'xolotl-engram-storage-probe-v64-*.json',
    'xolotl-readiness-watch-v65-*.json', 'xolotl-post-concurrency-v66-*.json',
    'xolotl-old-cleanup-v31-after-recovery-v65.json',
    'workbuddy-native-read-e2e-v60-intent.json', 'workbuddy-native-read-e2e-v60-result.json',
    'workbuddy-stream-e2e-v61-intent.json', 'workbuddy-stream-e2e-v61-result.json',
]
receipt_names = sorted({p.name for pattern in patterns for p in B.glob(pattern)})
for name in receipt_names:
    copy(name, 'receipts/' + name)
operators = [
    'prepare_xolotl_dense_subbatch_v47.py', 'launch_xolotl_dense_subbatch_v47.frozen.py',
    'manage_xolotl_dense_subbatch_v47.frozen.py', 'run_xolotl_dense_subbatch_v47r1.frozen.py',
    'prepare_xolotl_draft_cumulative_v49.py', 'launch_xolotl_draft_cpu_build_v49.frozen.py',
    'stage_xolotl_draft_cuda_headers_v49.frozen.py', 'launch_xolotl_draft_native_link_v49.frozen.py',
    'launch_xolotl_draft_service_v50.frozen.py', 'manage_xolotl_draft_service_v50.frozen.py',
    'manage_xolotl_draft_extended_v50.frozen.py', 'run_xolotl_draft_comparison_v50.frozen.py',
    'xolotl_draft_long_output_worker_v50.frozen.py', 'launch_xolotl_tool_diagnostic_v51.frozen.py',
    'prepare_xolotl_api_hybrid_v52.py', 'stage_xolotl_api_hybrid_v52.frozen.py',
    'test_xolotl_native_http_v52.frozen.py', 'launch_xolotl_draft_service_v53.frozen.py',
    'manage_xolotl_draft_service_v53.frozen.py', 'manage_xolotl_draft_extended_v53.frozen.py',
    'run_xolotl_draft_comparison_v53.frozen.py', 'run_xolotl_draft_recovery_v55.frozen.py',
    'xolotl_fullwindow_long_fixture_v54.py', 'test_xolotl_fullwindow_long_fixture_v54.py',
    'xolotl_fullwindow_long_prepare_v54.frozen.py', 'xolotl_fullwindow_long_prepare_v54r1.frozen.py',
    'stage_xolotl_fullwindow_long_fixture_v54.frozen.py', 'stage_xolotl_fullwindow_long_fixture_v54r1.frozen.py',
    'observe_xolotl_fullwindow_long_fixture_v54.frozen.py', 'observe_xolotl_fullwindow_long_fixture_v54r1.frozen.py',
    'xolotl_fullwindow_long_worker_v54.frozen.py', 'xolotl_fullwindow_long_suite_worker_v56.frozen.py',
    'manage_xolotl_fullwindow_long_suite_v56.frozen.py',
    'submit_xolotl_single1m_cont8_lease.frozen.py', 'xolotl_cont8_common_v58.py',
    'xolotl_qualified_api_worker_v53.frozen.py', 'launch_xolotl_stable_recovery_v58.frozen.py',
    'manage_xolotl_stable_recovery_v58.frozen.py', 'manage_xolotl_stable_extended_v58.frozen.py',
    'run_xolotl_stable_recovery_v58.frozen.py', 'promote_xolotl_stable_recovery_v58.frozen.py',
    'finalize_xolotl_stable_recovery_v58.frozen.py', 'xolotl_stable_edge_suite_worker_v59.frozen.py',
    'manage_xolotl_stable_edge_suite_v59.frozen.py', 'workbuddy_native_read_e2e_worker_v60.frozen.py',
    'launch_workbuddy_native_read_e2e_v60.frozen.py', 'workbuddy_stream_e2e_worker_v61.frozen.py',
    'launch_workbuddy_stream_e2e_v61.frozen.py', 'xolotl_engram_local_copy_worker_v63.frozen.py',
    'stage_xolotl_engram_local_copy_v63.frozen.py', 'observe_xolotl_engram_local_copy_v63.frozen.py',
    'xolotl_engram_storage_probe_worker_v64.frozen.py', 'stage_xolotl_engram_storage_probe_v64.frozen.py',
    'xolotl_readiness_watch_worker_v65.frozen.py', 'launch_xolotl_readiness_watch_v65.frozen.py',
    'xolotl_post_concurrency_worker_v66.frozen.py', 'manage_xolotl_post_concurrency_v66.frozen.py',
    'retain_xolotl_long_output_recovery_r5.py', 'audit_h200_staged_publication_v1.py',
]
for name in operators:
    copy(name, 'operators/' + name)
public_service = {key: stable[key] for key in ['allocation_job', 'allocation_uid', 'replica', 'replica_uid', 'gpu_uuid', 'pod_ip']}
public_service.update(base_url=promotion['base_url'], model=stable['plan']['model_name'],
    context_tokens=1048576, binary_sha256=stable['plan']['binary_sha256'],
    api_sha256='f062d9d39131959da9c981b6704e2ec27f89d8e2806730062dcabc3eb27682ba',
    prefill_batch_tokens=8192, prefill_chunk_tokens=128, parallel=2,
    conversation_cache_mib=8192, conversation_cache_slots=8,
    same_existing_key=True, credentials_retained=False, windows_forwarding=False,
    lease_ends_unix=stable['plan']['lease_ends_unix'], instance_actual_prompt_tokens_qualified=132097,
    actual_prompt_tokens_qualified=132097, full_1m_length_qualified=False,
    instance_full_1m_execution_tested=False, engine_profile_full_1m_length_qualified=True,
    native_tool_roundtrip_passed=True, workbuddy_bundled_cli_e2e_passed=True, workbuddy_gui_e2e_passed=False,
    workbuddy_first_turn_latency_accepted=False)
data(public_service, 'current-serving-public-metadata.json')
summary = dict(schema='aster-xolotl-long-output-recovery-r5', unix=time.time(),
    masked_indexer_full1m=full['fullwindow/request-result.json']['tests'],
    masked_indexer_fixed_cache_hit_load=full['cache-load/request-result.json']['levels'],
    attention_subbatch_profiles=width['profiles'],
    actual1m_long_original_suite_phase='failed', actual1m_long_cold_and_restore_passed=True,
    actual1m_long_fixture=q['fixture'], actual1m_long_single_load=q['levels'],
    actual1m_long_concurrency2_exact_identity_passed=False,
    actual1m_long_post_concurrency_single_matched=True,
    rejected_cumulative010=read('xolotl-draft-cum010-v53-output-divergence.json'),
    cumulative025_and050_gpu_tested=False,
    off_long_short_input=read('xolotl-draft-extended-v53-off-latest.json')['result']['long_summary'],
    off_repeat_long_short_input=read('xolotl-draft-extended-v53-off_repeat-latest.json')['result']['long_summary'],
    current_serving=public_service, workbuddy_default_two_turns=cli['tests'],
    installed_read_tool_passed=True, realistic_engram_wait_seconds=71.936,
    local_engram_203gb_exact_full_readbacks_passed=True,
    local_engram_storage_probe=storage['result']['summaries'], local_engram_api_speed_gain_proven=False,
    maximum_authorized_h200=2, single_request_300_met=False, goal_complete=False)
data(summary, 'round-summary.json')
outcome = '''# Actual1M long output, strict tool repair and service recovery (R5)

The exact masked-indexer profile completed actual1M on GPU355: 1,048,504
input tokens,17 output tokens, exact begin/middle/end retrieval and KV restore.
Cold first text was1924.463232s versus the earlier same-card V35 2078.504349s
(7.411152% less time, one finite cold block). Fixed1M cache-hit output
throughput was8.810567/10.309948 tokens/s at concurrency1/2,32 measured plus
two warmup requests. The second level regressed1.7267% versus V35.
The sub-batch128/256/512 comparison completed84 basic requests. S512's
35,700-token cold first text was37.751653s versus39.675669s for the repeated
S128 control; its actual1M long-output evidence is separately scoped below.

The cumulative-draft native build reproduces the parent generator object
and nativefa control executable byte-for-byte before replacing only the
generator object. Its240 policy checks pass in optimized and ASAN/UBSAN
builds; eight invalid startup modes refuse before model execution. The
0.1 mode fails its matched1024-token greedy-output gate at character821.
The failed receipt is retained and that service was drained. Modes0.25/0.5
were not GPU-tested. Off and repeated-off each pass28 basic plus12 extended
requests, including native tool/result return and eight measured1024-token
outputs:75.674695 and75.757702 API-wall tokens/s on short inputs.

V50 reproduced the real native JSON argument failure. V51 captured a complete
JSON object followed by one redundant native parameter terminator. V52
accepts only that exact outer suffix while retaining duplicate/nonfinite/
unknown-suffix refusals. Eight CPU suites pass, including actual HTTP
red/green reproduction, four forced/free streamed/nonstreamed tool cases,
64 combined stream cases and unchanged real1M tokenizer IDs. Real GPU
OpenAI tool/result returns and installed WorkBuddy Read/followup now pass.

V56 tests1,047,544 input tokens plus1024 output tokens and8 safety tokens.
Cold first text was1892.476596s;1024 tokens completed in1939.080745s.
Exact three-position retrieval, an isolated53 request, KV restoration and
the entire1024-token restored output pass. Eight single-active cache-hit
requests match the complete cold output:28.708409 API-wall tokens/s,
29.313315 stream-interval estimate and0.738381/0.783754s first-text P50/P95.
All eight concurrent requests complete1024 tokens and retrieve all three
codes, but their tutorial continuations differ from the single-request
greedy reference. The ORIGINAL SUITE REMAINS FAILED. No concurrency2 block
wall throughput is claimed because that block aborted its summary. V66's
subsequent single request again matches the complete cold1024-token output;
the observed difference is not proof of cross-conversation KV mixing.
Low-priority CPU/disk staging overlapped the tail of this cold capacity
request; its timing is not presented as an isolated A/B improvement.

The former formal job042 stopped at03:48:49 CST before its recorded06:20
lease. Kubernetes reports RJobStopped without establishing who or why.
Replacement jobc016/GPU5539 follows fresh terminal old job/replica proof,
keeping a maximum of two H200. It uses the actual1M-qualified nativefa/S128
profile plus corrected API f062 and exact bounded token memo1. Fresh
28 basic,12 extended and strict132097 gates pass. The same owned VIP
10.102.93.68 and key are retained. Direct Windows health/models and a
unique target-pod nonce followed by full-prefix KV reuse through the VIP
prove the new backend. No Windows forwarding is installed.

Installed WorkBuddy tests use fresh memory-only headless sessions, never
an existing user conversation. The Read-only profile actually reads the
public173/269 fixture with preserved file_path and answers442. The default
two-turn conversation answers hi twice: first output104.623173s, then
2.733718s. The first turn is still unacceptable; this is not a latency
success. Native logs attribute71.936s of its100.27s prefill to Engram
read/dequantization completion waiting. Exact table files totaling
203,073,077,576 bytes are staged on the optimization pod's local overlay
disk; both original-source and local full-readback SHA256 match the
published LFS hashes. Twelve CPU/disk sampled-row blocks preserve every
byte. At4096 rows/table median dispatch-plus-read time is0.288581s on
GPFS versus0.225260s locally. This is a storage component result, not
proof of model/API speedup. The live formal model still uses GPFS tables.
A finite server-side readiness observer records exact job/VIP health
without inference, restarts, resource creation or a Windows dependency.

## Commands and counts

- V45 suite observer: strict132097, three actual1M requests and34 fixed
  cache-hit requests completed, exact client gone.
- V47r1 controller: three sequential profiles,28 requests each,84 total.
- V49 generator CPU reproduction/native relink: exact original object and
  native control hashes;240 distinct policy checks in two CPU build modes;
  eight invalid-startup refusals. No positive-cutoff speed qualification.
- V52 CPU preflight: eight suites, exact original HTTP failure reproduced,
  revised four-mode HTTP green proof and unchanged actual1M token IDs.
- V53 off/cum010 and V55 off-repeat: exact native/tool/1024-output receipts;
  the cumulative identity failure is retained without changing its gate.
- V56 original suite: two remainder requests and20 full-window client
  requests completed. Single load8/8 passes; concurrent8/8 finishes/retrieves,
  but8/8 differs in continuation hash. Original suite phase failed.
- V66: one post-concurrency single1024-output request, complete hash matches.
- V58/V59 recovery:28 basic,12 native-tool/long-output and two strict132097
  requests pass, followed by actual Windows same-VIP backend verification.
- WorkBuddy V60/V61: installed Read/followup442 and same-process hi/hi pass;
  headless only, no settings mutation and no session persistence.
- V63: two203GB-total full copy/readback SHA256 checks; original sources
  untouched. V64:12 CPU/disk blocks, two sample sizes,16 threads, all bytes
  identical. No GPU kernel or model configuration changed by these tasks.

## Explicitly not claimed

- No300/500/1000 tokens/s single-request generation qualification.
- No passed full V56 concurrency/byte-identity suite or cause of its drift.
- No local-storage model/API speed gain, adoption, natural-language1M
  throughput claim, cold1M concurrency, long soak or general P95 guarantee.
- No new formal-instance actual1M execution: its fresh instance proof is
  132097; nativefa/S128 actual1M profile evidence is from GPU355/V45.
- No current WorkBuddy GUI verification or accepted cold first-turn latency.
- No Rust-native runtime claim; serving uses Xolotl C++/CUDA plus Python.
- No new full Rust workspace CI/fmt claim. This landing preserves external
  candidate source and bounded evidence. No model weights, binaries,
  compiled objects, credentials or private client session state are retained.
'''
put(outcome.encode(), 'OUTCOME.md')
data(dict(schema='aster-xolotl-long-output-recovery-r5-manifest', unix=time.time(), files=files), 'manifest.json')
existing = {row['file'] for row in old_rows}
for path in sorted(D.rglob('*')):
    if path.is_file():
        name = path.relative_to(E).as_posix()
        if name in existing:
            raise RuntimeError('immutable evidence collision')
        raw = path.read_bytes()
        manifest['files'].append(dict(file=name, bytes=len(raw), sha256=hashlib.sha256(raw).hexdigest()))
if manifest['files'][:len(old_rows)] != old_rows:
    raise RuntimeError('historical row order changed')
manifest['latest_fullwindow_indexer_revision'] = 'ttft-1m-20261025/r5'
newline = '\r\n' if b'\r\n' in original else '\n'
manifest_path.write_bytes((json.dumps(manifest, indent=2) + '\n').replace('\n', newline).encode())
source_path = W / 'docs/source-manifest.json'
source = json.loads(source_path.read_text(encoding='utf-8'))
entry = source['original_xolotl_h200_20261024']
relative = D.relative_to(W).as_posix()
access = entry['server_access_1m']
access.setdefault('historical_service_snapshots', []).append(dict(access['active_service'],
    terminal_phase='Stopped', terminal_time_cst='2026-10-25T03:48:49+08:00', evidence=relative))
access.update(active_service=public_service, evidence=relative, full_1m_length_qualified=False,
              engine_profile_full_1m_length_qualified=True, instance_full_1m_execution_tested=False)
entry['latest_masked_indexer_candidate'].update(evidence=relative, full1m_qualified=True,
    speed_gain_proven=True, speed_gain_scope='one same-GPU cold1048504 block,7.411152% less TTFT',
    fullwindow_fixed_cache_hit_load=full['cache-load/request-result.json']['levels'],
    production_promoted=True, current_suite_phase='passed')
entry['latest_combined_api_candidate'].update(evidence=relative,
    api_sha256=public_service['api_sha256'], cpu_suites=8, gpu_qualified=True, production_promoted=True,
    strict_single_native_parameter_close_suffix=True, installed_workbuddy_read_tool_passed=True)
entry['latest_cumulative_draft_candidate'] = dict(evidence=relative,
    native_sha256='34f3c963304445e50c4ae398194f4b4745a0b1868a6b4a1d6668068aebcf96e6',
    positive_cutoff_010_qualified=False, failed_greedy_identity=True, cutoff025_gpu_tested=False,
    cutoff050_gpu_tested=False, original_native_control_reproduced=True, cpu_policy_cases=240,
    production_promoted=False)
entry['latest_actual1m_long_output'] = dict(evidence=relative, original_suite_phase='failed',
    actual_input_tokens=1047544, actual_output_tokens=1024, single_active_cache_hit=q['levels'][0],
    full_restored_output_identical=True, concurrency2_exact_identity_passed=False,
    post_concurrency_single_identical=True, speed300_met=False)
entry['latest_engram_storage_candidate'] = dict(evidence=relative, full_original_and_local_sha256_passed=True,
    bytes=203073077576, sampled_blocks=12, sampled_raw_bytes_identical=True,
    storage_component=storage['result']['summaries'], model_api_gain_proven=False,
    production_promoted=False)
roles = entry['current_serving_optimization_roles']
roles.update(unix=time.time(), goal_met=False, serving=public_service,
    next_action='Diagnose concurrent-path continuation drift; test real English/WorkBuddy prefill with the exact local Engram tables; profile dense GPU/host routing for the unmet300 milestone. Preserve formal c016 and same VIP/key.')
roles['optimization'].update(current_profile='off_repeat34f/S512/f062',
    actual1m_plus1024_single_qualified=True, full_concurrency_suite_passed=False,
    current_fullwindow_client_terminal=True, production_promoted=False)
roles['publication']['evidence_revision'] = 'ttft-1m-20261025/r5'
source_path.write_text(json.dumps(source, indent=2, ensure_ascii=False) + '\n', encoding='utf-8', newline='\n')
attributes = W / '.gitattributes'
rule = '/docs/evidence/power/2026-10-22-native-deepseek-h200/original-xolotl-single-h200/ttft-1m-20261025/r5/frozen-candidates/**/*.patch -text whitespace=-blank-at-eol,blank-at-eof,space-before-tab,cr-at-eol\n'
text = attributes.read_text(encoding='utf-8')
if rule.strip() in text:
    raise RuntimeError('R5 scoped patch attribute already exists')
attributes.write_text(text.rstrip() + '\n' + rule, encoding='utf-8', newline='\n')
print(json.dumps(dict(phase='retained', revision='r5', public_files=sum(path.is_file() for path in D.rglob('*')),
    historical_entries_preserved=len(old_rows), credential_occurrences=0, formal_promoted=True,
    actual1m_long_concurrency_suite_passed=False, single_request300_met=False)))
