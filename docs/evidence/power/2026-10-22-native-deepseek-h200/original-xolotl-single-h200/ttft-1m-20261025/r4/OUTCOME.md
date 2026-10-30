# Full-window baseline and exact masked-indexer qualification (R4)

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
and is at `fullwindow` as of this snapshot;
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
