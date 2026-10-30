"""Append immutable full1M, masked-indexer, and CPU API evidence; retain historical bytes."""
import hashlib
import json
import time
from pathlib import Path

B = Path('D:/code/aster/target/h200-native')
W = Path('D:/code/aster/.wt-h200-native-integration')
E = W / 'docs/evidence/power/2026-10-22-native-deepseek-h200/original-xolotl-single-h200'
D = E / 'ttft-1m-20261025/r4'
if D.exists():
    raise RuntimeError('R4 exists; append another immutable revision')


def read(name):
    return json.loads((B / name).read_text(encoding='utf-8-sig'))


full = read('xolotl-resident-v38-fullwindow-latest.json')
load = read('xolotl-resident-fullwindow-load-v41-latest.json')
basic = read('xolotl-indexer-basic-v44-latest.json')
numeric = read('xolotl-indexer-gpu-numerical-v40r2-latest.json')
memory = read('xolotl-indexer-memcheck-v43-latest.json')
cpu = read('xolotl-api-adapter-v46-latest.json')
suite = read('xolotl-indexer-fullwindow-suite-v45-latest.json')
for receipt in [full, load, basic]:
    if receipt['result']['phase'] != 'passed' or receipt['exact_client_alive']:
        raise RuntimeError('completed full model and load receipts required')
if (full['result']['actual_prompt_tokens'] != 1048504 or not full['result']['full1m_cache_restore_passed']
        or load['result']['measured_requests'] != 32 or len(basic['result']['tests']) != 28):
    raise RuntimeError('actual full-window and finite request counts differ')
if (numeric['result']['phase'] != 'passed' or numeric['exact_numerical_alive']
        or numeric['result']['actual_graph_cases'] != 360 or numeric['result']['helper_cases'] != 144
        or not numeric['result']['full_score_byte_identity_passed']
        or memory['result']['phase'] != 'passed' or not memory['result']['zero_errors']
        or memory['exact_memcheck_alive']):
    raise RuntimeError('revised CUDA mathematical/memory gates incomplete')
if (cpu['cpu-preflight-result.json']['phase'] != 'passed' or cpu['exact_cpu_client_alive']
        or cpu['real-tokenizer-benchmark.json']['phase'] != 'passed'):
    raise RuntimeError('combined adapter CPU gates incomplete')
keys = []
for path in B.glob('access-private-*/api-key.txt'):
    raw = path.read_bytes().strip()
    if len(raw) >= 24 and raw not in keys:
        keys.append(raw)
if not keys:
    raise RuntimeError('actual credential negative controls missing')

manifest_path = E / 'manifest.json'
original_manifest = manifest_path.read_bytes()
manifest = json.loads(original_manifest)
old_rows = list(manifest['files'])
for row in old_rows:
    raw = (E / row['file']).read_bytes()
    if len(raw) != row['bytes'] or hashlib.sha256(raw).hexdigest() != row['sha256']:
        raise RuntimeError('historical published evidence changed: ' + row['file'])
D.mkdir(parents=True)
files = []


def put(raw, name, source=None):
    if len(raw) > 2 * 1024**2 or any(key in raw for key in keys):
        raise RuntimeError('public size/credential gate refused: ' + name)
    path = D / name
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open('xb') as output:
        output.write(raw)
    files.append(dict(file=name, bytes=len(raw), sha256=hashlib.sha256(raw).hexdigest(), source=source))


def copy(name, target):
    put((B / name).read_bytes(), target, name)


def data(value, name):
    put((json.dumps(value, indent=2, ensure_ascii=False) + '\n').encode(), name)


for folder in ['indexer-v40', 'indexer-v40r2', 'api-adapter-v46']:
    for path in sorted((B / folder).iterdir()):
        if not path.is_file() or path.suffix not in ['.cpp', '.cu', '.cuh', '.hpp', '.py', '.json', '.patch']:
            raise RuntimeError('unreviewed candidate artifact: ' + str(path))
        copy(folder + '/' + path.name, 'frozen-candidates/' + folder + '/' + path.name)

receipts = [
    'xolotl-resident-v38-remainder-intent.json', 'xolotl-resident-v38-remainder-latest.json',
    'xolotl-resident-v38-fullwindow-intent.json', 'xolotl-resident-v38-fullwindow-latest.json',
    'xolotl-resident-full1m-v38-numeric-phases.json',
    'xolotl-resident-fullwindow-load-v41-intent.json', 'xolotl-resident-fullwindow-load-v41-latest.json',
    'xolotl-resident-v41-drain.json',
    'xolotl-indexer-v40-parent-provenance.json', 'xolotl-indexer-v40-original-build-recipes.json',
    'xolotl-indexer-v40-dense-build-recipes.json',
    'xolotl-indexer-cpu-preflight-v40-intent.json', 'xolotl-indexer-cpu-preflight-v40-latest.json',
    'xolotl-indexer-gpu-build-v40-intent.json', 'xolotl-indexer-gpu-build-v40-latest.json',
    'xolotl-indexer-gpu-numerical-v40-intent.json', 'xolotl-indexer-gpu-numerical-v40-latest.json',
    'xolotl-indexer-v40-alignment-diagnostic-intent.json', 'xolotl-indexer-v40-alignment-diagnostic-latest.json',
    'xolotl-indexer-gpu-build-v40r2-intent.json', 'xolotl-indexer-gpu-build-v40r2-latest.json',
    'xolotl-indexer-gpu-numerical-v40r2-intent.json', 'xolotl-indexer-gpu-numerical-v40r2-latest.json',
    'xolotl-indexer-memcheck-v43-intent.json', 'xolotl-indexer-memcheck-v43-latest.json',
    'xolotl-indexer-basic-v44-intent.json', 'xolotl-indexer-basic-v44-latest.json',
    'xolotl-indexer-fullwindow-suite-v45-intent.json', 'xolotl-indexer-fullwindow-suite-v45-latest.json',
    'xolotl-indexer-v45-live-numeric.json',
    'xolotl-api-adapter-v46-intent.json', 'xolotl-api-adapter-v46-latest.json',
    'xolotl-windows-readonly-availability-v46.json',
]
for name in receipts:
    copy(name, 'receipts/' + name)
operators = [
    'prepare_xolotl_indexer_mask_v40.py',
    'launch_xolotl_indexer_cpu_preflight_v40.frozen.py',
    'launch_xolotl_indexer_gpu_build_v40.frozen.py', 'observe_xolotl_indexer_gpu_build_v40.frozen.py',
    'launch_xolotl_indexer_gpu_build_v40r2.frozen.py', 'observe_xolotl_indexer_gpu_build_v40r2.frozen.py',
    'launch_xolotl_indexer_service_v44.frozen.py', 'manage_xolotl_indexer_service_v44.frozen.py',
    'manage_xolotl_indexer_fullwindow_suite_v45.frozen.py', 'xolotl_indexer_fullwindow_suite_worker_v45.frozen.py',
    'launch_xolotl_resident_fullwindow_load_v41.frozen.py', 'observe_xolotl_resident_fullwindow_load_v41.frozen.py',
    'xolotl_fullwindow_reuse_load_worker_v20.frozen.py', 'drain_xolotl_resident_after_fullwindow_v41.frozen.py',
    'prepare_xolotl_api_adapter_v46.py', 'stage_xolotl_api_adapter_v46.frozen.py',
    'retain_xolotl_fullwindow_indexer_r4.py', 'audit_h200_staged_publication_v1.py',
]
for name in operators:
    if not (B / name).exists():
        raise RuntimeError('exact operator missing before retention: ' + name)
    copy(name, 'operators/' + name)
service = read('xolotl-indexer-service-v44-intent.json')
public_service = {k: service[k] for k in ['root', 'allocation_job', 'allocation_uid', 'replica', 'replica_uid', 'gpu_uuid', 'pod_ip', 'host_ip', 'process']}
public_service.update(binary_sha256=service['plan']['binary_sha256'], context_tokens=1048576,
                      prefill_batch_tokens=8192, attention_sub_batch_tokens=128, parallel=2,
                      exact_mask_skip=True, cpu_token_memo_enabled=False, strict_json_overlay_enabled=False,
                      formal_promoted=False, basic28_passed=True, full1m_qualified=False,
                      full1m_current_stage=suite['suite-result.json']['current_stage'],
                      lease_ends_unix=service['plan']['lease_ends_unix'])
data(public_service, 'private-optimization-v44-public-metadata.json')
summary = dict(schema='aster-xolotl-fullwindow-indexer-r4', unix=time.time(),
               v35_full1m_tests=full['result']['tests'], v35_fullwindow_load=load['result']['levels'],
               v35_full1m_qualified=True, same_gpu_355=True,
               v44_basic_short=basic['result']['short_summary'],
               v44_basic_long=[row for row in basic['result']['tests'][20:]],
               v44_full1m_qualified=False, v44_actual_full1m_status=suite['suite-result.json'],
               first_v40_gpu_failure_retained=True, revised_cuda_actual_graphs=360,
               revised_cuda_helper_cases=144, revised_cuda_full_scores_byte_identical=True,
               revised_cuda_memcheck_errors=0, api_adapter_cpu_only=cpu['real-tokenizer-benchmark.json'],
               formal_promoted=False, speed_300_qualified=False, workbuddy_gui_e2e_qualified=False,
               credentials_retained=False)
data(summary, 'round-summary.json')
outcome = f'''# Full-window baseline and exact masked-indexer qualification (R4)

The V35 resident-MMQ profile completed an actual 1,048,504-token input on one
H200 (GPU355). Cold first text took 2078.504349 seconds and the request ended
in 2079.097857 seconds. Exact begin/middle/end retrieval, an interleaved
53 control, and restoration of the full KV prefix passed. The same-card
fixed cache-hit load passed 32 measured requests plus two warmups. At
concurrency one/two its API-wall output throughput was 8.580983/10.491099
tokens/s, and first-text P95 was 1.573262/5.158947 seconds. Each completion
contained 17 tokens. This is a finite fixed-prompt cache-hit test.

V40 moves the existing candidate mask before the indexer and skips only fully
masked blocks whose bounded inputs guarantee a finite original sum. It keeps
unmasked arithmetic, weight bytes and routing unchanged. The first revision
failed on F16 WMMA with misaligned shared-memory reads; the failure and its
11,266-error diagnostic remain retained. V40r2 explicitly aligns the three
WMMA shared matrices to 32 bytes. Its 144 helper cases and 360 actual CUDA
graphs pass; all 138,780 scores per profile are byte-identical. Compute
Sanitizer passes 90 enabled graphs with zero errors.

V44 loads this binary on the independent optimization H200. All 28 basic
model requests passed, including retrieval and isolation. Its 35.7K cold
first text was 39.632166 seconds versus the earlier same-card V35 40.030457;
short API-wall throughput was 83.542783 versus 83.264951 tokens/s. These small
differences do not establish a clear speed gain. V45 passed strict132097
and is at `{suite['suite-result.json']['current_stage']}` as of this snapshot;
its new actual1M qualification is incomplete. Historical evidence is unchanged.

The V46 API candidate combines strict complete JSON-object/native DSML
argument parsing with the existing bounded exact-text token cache. Cache mode
is a closed 0/1 flag, default off, with at most 32 MiB/four entries. Six CPU
suites pass: upstream security, upstream DS41, ten memo unit tests, six strict
grammar unit tests, two combined tests containing 64 complete stream cases,
and the actual1M tokenizer check. The latter preserves all IDs and measures
CPU encode medians of 1.179206 seconds uncached and 0.026364 on an exact hit;
24 Unicode/special/literal-span comparisons pass. GPU/API benefits are pending.

## Commands and counts

The frozen operators and their receipts retain the exact inputs/commands:

- `launch_xolotl_resident_fullwindow_v38.frozen.py fullwindow` (R3 operator):
  three completed API requests, actual1M retrieval and full KV restore passed.
- `launch_xolotl_resident_fullwindow_load_v41.frozen.py`: 2 warmups and
  16 measured requests at each of concurrency1/2, all 34 passed.
- V40/r2 build workers reproduce the V35 control binary byte-for-byte,
  then compile the exact dense host object and the CUDA indexer replacement.
  The original dense host object is also reproduced byte-for-byte.
- `indexer_mask_policy_test.cpp`: 66 CPU checks. GPU numeric worker:
  144 helper cases, 360 graphs and full-score byte identity, all passed.
- `compute-sanitizer --tool memcheck`: revised enabled graph driver,
  90 graphs, exit0, zero errors. The first failed revision is retained.
- `manage_xolotl_indexer_service_v44.frozen.py observe_client`: phase passed,
  28 requests, exact client gone. V45 is observed by its exact live process;
  a timeout is never treated as permission to resubmit.
- `stage_xolotl_api_adapter_v46.frozen.py`: CPU-only preflight, six suites,
  no credentials copied and no GPU code executed. Source worker retains
  each concrete Python command, exit status and tokenizer ID digest.

The formal service remains on V31 (GPUb8), same VIP/key, context1M; this
instance's actual132097 gate and earlier same-profile actual1M proof remain
distinct. Windows direct authenticated health/models are HTTP200 at this
snapshot; this check sends no inference request. Maximum task-owned GPUs:2.

## Explicitly not claimed

- No isolated V35-vs-V15 full1M gain: the older V15 full1M run used GPU117a.
- No V44 full1M completion, sustained gain, promotion or new kernel speedup.
- No cold1M concurrency, long-output1M benchmark, soak test or reliable P95
  estimate outside the finite blocks described above.
- No300/500/1000 tokens/s single-request generation qualification.
- No end-to-end V46 cache/tool acceptance or current WorkBuddy GUI acceptance.
- No Rust-native inference claim: this runtime uses Xolotl C++/CUDA and Python API.
- No new full Rust workspace CI/fmt claim; this revision retains external
  candidate source, finite tests, and evidence only. Existing upstream warning
  output is retained. No weights, compiled objects/binaries, credentials or
  private client state are included.
'''
put(outcome.encode(), 'OUTCOME.md')
data(dict(schema='aster-xolotl-fullwindow-indexer-r4-manifest', unix=time.time(), files=files), 'manifest.json')
existing = {row['file'] for row in old_rows}
for path in sorted(D.rglob('*')):
    if path.is_file():
        name = path.relative_to(E).as_posix()
        if name in existing:
            raise RuntimeError('immutable evidence collision')
        raw = path.read_bytes()
        manifest['files'].append(dict(file=name, bytes=len(raw), sha256=hashlib.sha256(raw).hexdigest()))
if manifest['files'][:len(old_rows)] != old_rows:
    raise RuntimeError('historical manifest row order changed')
manifest['latest_fullwindow_indexer_revision'] = 'ttft-1m-20261025/r4'
newline = '\r\n' if b'\r\n' in original_manifest else '\n'
if newline == '\r\n' and b'\n' in original_manifest.replace(b'\r\n', b''):
    raise RuntimeError('historical root manifest has mixed newline style')
manifest_path.write_bytes((json.dumps(manifest, indent=2) + '\n').replace('\n', newline).encode())
path = W / 'docs/source-manifest.json'
source_manifest = json.loads(path.read_text(encoding='utf-8'))
entry = source_manifest['original_xolotl_h200_20261024']
relative = D.relative_to(W).as_posix()
entry['latest_resident_mmq_candidate'].update(full1m_candidate_qualified=True,
    actual_full1m_prompt_tokens=1048504, full1m_cache_restore_passed=True,
    full1m_evidence=relative, same_gpu_full1m_cold_ttft_seconds=2078.5043493108824,
    fullwindow_fixed_cache_load=load['result']['levels'], production_promoted=False)
entry['latest_masked_indexer_candidate'] = dict(evidence=relative,
    binary_sha256=service['plan']['binary_sha256'], basic28_passed=True,
    full1m_qualified=False, numerical_cases=144, actual_graphs=360,
    full_scores_byte_identical=True, memcheck_errors=0, speed_gain_proven=False,
    production_promoted=False, current_suite_phase=suite['suite-result.json']['phase'])
entry['latest_combined_api_candidate'] = dict(evidence=relative, cpu_preflight_passed=True,
    exact_memo_mode_default='0', complete_stream_cases=64, actual_full1m_token_ids_identical=True,
    cpu_only_encode_baseline_median_seconds=1.1792060369625688,
    cpu_only_encode_memo_hit_median_seconds=0.026364211924374104,
    gpu_qualified=False, production_promoted=False)
roles = entry['current_serving_optimization_roles']
roles['unix'] = time.time()
roles['goal_met'] = False
roles['optimization'].update(current_profile='indexer_mask_v44', binary_sha256=service['plan']['binary_sha256'],
    basic28_passed=True, full1m_candidate_qualified=False, qualified_v35_full1m_baseline=True,
    exact_live_fullwindow_suite='qualification-suite-v45', production_promoted=False)
roles['publication']['evidence_revision'] = 'ttft-1m-20261025/r4'
roles['next_action'] = 'Observe exact live V45 client; compare completed actual1M against same-GPU V35. CPU-only V46 adapter is prepared, not GPU-qualified. Profile dense GPU work and compare attention sub-batches after the current suite finishes; keep formal V31 unchanged until qualification.'
path.write_text(json.dumps(source_manifest, indent=2, ensure_ascii=False) + '\n', encoding='utf-8', newline='\n')
attributes = W / '.gitattributes'
rule = '/docs/evidence/power/2026-10-22-native-deepseek-h200/original-xolotl-single-h200/ttft-1m-20261025/r4/frozen-candidates/**/*.patch -text whitespace=-blank-at-eol,blank-at-eof,space-before-tab,cr-at-eol\n'
text = attributes.read_text(encoding='utf-8')
if rule.strip() in text:
    raise RuntimeError('R4 patch rule unexpectedly already present')
attributes.write_text(text.rstrip() + '\n' + rule, encoding='utf-8', newline='\n')
print(json.dumps(dict(phase='retained', revision='r4', public_files=sum(p.is_file() for p in D.rglob('*')),
    historical_entries_preserved=len(old_rows), credential_occurrences=0, formal_promoted=False)))
