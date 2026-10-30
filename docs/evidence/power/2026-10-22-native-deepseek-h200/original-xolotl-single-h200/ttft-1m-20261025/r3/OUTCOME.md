# Checked resident-expert MMQ and stable resource rollover, R3

The exact V15 b2586171 engine and original 5180c184 Python API now serve the
same server VIP/key on a fresh H200 b8 allocation04278df1. The new instance
passed 28 basic public requests and the 132097-token one-token-remainder case.
The original actual1048504-token V17/V20 evidence remains on retired GPU117a.
Same binary/profile qualification and fresh-instance execution are separate:
this refreshed public instance has not rerun the full1M test. The public
source metadata now expresses this scope explicitly. Windows confirmed a
complete answer42 and cached354 tokens for a unique nonce primed only on the
target PodIP. Endpoint-object inspection was RBAC Forbidden; that failed
attempt is retained, with no privilege escalation or endpoint-read claim.
Windows has no forwarding process. The formal resource lease ends06:20CST.

The previous allocation and its Replica both reached Succeeded at their frozen
deadline before one replacement optimization H200 was requested. The new
cont7 RJob08538623 and Replica7bbf9c96 passed actual CUDA allocation/memory
round-trip checks on GPU355373e1, with its own07:29CST lease. Maximum task-owned
H200s stays two. These are finite resources, not indefinite availability.

V35 builds on V19's checked sized-slot ownership and suspended tail-slot LRU
admission. Its separate closed-set ASTER_XQ_RESIDENT_MMQ=0|1 opt-in copies exact
resident expert blob bytes, device-to-device, into the two existing DMA halves.
Their uniform strides allow the already compiled MMQ kernels to handle a
resident run that previously fell back to MMVQ on scattered ranked pointers.
The current layer's prestaged half is reserved; used/copied events order reuse.
No allocation call, new CUDA kernel, weight change, expert dropping or routing
bias is introduced. Existing MMQ scratch can be used earlier; unchanged buffer
code is not a claim of identical peak memory in every configuration.

Controlled GPU-free relinking reproduced exact V15 b2586171 before replacing
only the MoE host object. Candidate SHA256 is
fedf3b4199ec2c34915ca86b40f53656e27282763f34fa5eb0883b42e649eedd.
Thirty sized-slot ownership/capacity cases and32 resident-stage bounds/half
ownership checks passed. The new policy compiled with Wall/Wextra/Werror.
Two inherited MoE debug-print compiler warnings remain in the receipt.

On the independent idle H200, the original synthetic gate passed three
shrink/grow cycles,194 exact-slot restorations and478 byte comparisons.
Real mixed-format weights on layers0/39 then passed the upstream F32-dequant
reference threshold3e-2 with both flag values:4 single-token rows and16 chunk
rows per flag. Worst relative error was0.01242 without packing and0.01262 with
packing; no chunk row was non-finite. This is activation rounding tolerance,
not bitwise equality or a full-model quality suite. The corrected flag1 gate
recorded31 actual packing waves and839854080 exact D2D bytes.

The first numerical gate passed its actual math checks but failed the required
branch-counter assertion because upstream close() did not flush chunk counters.
That failed receipt is retained. The corrected gate adds release_chunk() before
close(), changing only the test, and passes. It does not restart a timed-out
model or hide the first failure.

Paired512-token operator runs on the same device, two layers, three repetitions
per layer and flag, gave warm last-two means45.85->34.55ms on layer0 (-24.65%)
and32.20->29.60ms on layer39 (-8.07%). These are routed-expert operator times,
not API speedups or isolated full-model decode rates.

The following full-model API profiles use the same physical GPU355373e1, same
original tokenizer/API, ctx1048576, prompt batch8192, chunk128, parallel2,
DSpark depth5/confidence0.45, exact routing, and the same public client/fixtures.
Dense/chunk diagnostic timing counters are enabled consistently in all three;
the expensive presence-only DS41_CHECK_ARENA byte rereads and tokenizer memo
are absent. The byte/numerical gates above ran before these timings.

Each profile completed4 warmups,16 measured short requests, cold/restored35700
arithmetic, one overlapping long/short pair, fresh three-position retrieval and
one yielded retrieval/isolation pair:28 requests per profile,84 total.

| Profile | Short TTFT P50/P95 s | API wall output tok/s | Cold35700 TTFT s | Restored TTFT s | Mixed-short TTFT s | Yielded retrieval TTFT s |
|---|---:|---:|---:|---:|---:|---:|
| v15_control | 0.1285 / 0.1586 | 85.361 | 48.913 | 0.0621 | 5.395 | 54.415 |
| v35_keep | 0.1326 / 0.1770 | 83.214 | 56.089 | 0.0609 | 2.521 | 56.369 |
| v35_packed | 0.1309 / 0.1632 | 83.265 | 40.030 | 0.0607 | 1.585 | 39.523 |

Packed cold35700 first-content change versus this same-card V15 block is
-18.16%; mixed-short change is-70.61%. Short-request throughput, cold
prefill and mixed-request latency remain separate measurements. One finite
block and one overlap pair per profile do not establish a sustained P95 or
general quality. The retained-cache-only profile's regression is preserved.

The sequential controller waits for exact client completion before draining
only the isolated service. The first control drain used completed-client proof
and health; health has no slot-idleness fields. That operator version is kept.
The corrected subsequent drain checks /slots is_processing explicitly before
the exact-UID/start-tick SIGTERM; stopped/reaped state and idle GPU are required
before the next profile. The formal VIP backend was never used for these
comparison requests. Packed candidate remains isolated for the strict132097
remainder and actual1048504-token cold/retrieval/KV-restore gates.

Commands use the pinned Windows Python3.12.14 executable and -X utf8:

- prepare_xolotl_resident_mmq_v35.py; launch_xolotl_resident_mmq_build_v35.frozen.py;
  observe_xolotl_resident_mmq_build_v35.py.
- launch_xolotl_resident_mmq_gpu_gate_v36.frozen.py (retained failed assertion);
  launch_xolotl_resident_mmq_gpu_gate_v36r2.frozen.py;
  observe_xolotl_resident_mmq_gpu_gate_v36r2.py (passed).
- run_xolotl_resident_compare_v37.frozen.py, with explicit v15_control,
  v35_keep and v35_packed service/client handles;84/84 public requests passed.
- launch_xolotl_resident_fullwindow_v38.frozen.py remainder, then fullwindow;
  their launch/observation receipts show actual scope as of this revision.
- Python AST parsing for12 new operator/worker preparation files; git diff
  --cached --check; audit_h200_staged_publication_v1.py before publication.

Frozen source and exact numeric/public receipts are retained beside this file.
The upstream Xolotl MIT provenance remains in the established source manifest
and crates/deepseek/licenses/Xolotl-MIT.txt. No model binary, compiled artifact,
credential, private WorkBuddy state or production prompt/output is published.
Historical R1/R2 evidence bytes and manifest order remain unchanged.

The strict candidate132097-token remainder gate finished with exact answer42,
actual API prompt_tokens132097, first content143.701s,
and total143.761s. Its passed client is gone. The actual full-window
V38 request has started with1048504 prompt tokens plus64 output budget and the
unchanged8-token safety margin; its retained receipt is still prefilling with
the exact client alive. No completed candidate1M timing is available here.
For cold35700 the measured dense stage stays16.52->16.56s, while experts change
28.46->21.83s versus V15 (retention-only37.74s). These stage counters support an
expert-path improvement, with the longer-context dense/indexer path still open.

## Explicitly not claimed

- Completed packed-candidate full1M qualification or production promotion.
- A full1M execution timing on the refreshed formal b8 instance, cold1M
  concurrency, long-output full1M load, a soak test or pure GPU decode speed.
- Bitwise equivalence of MMVQ/MMQ activation rounding, general model quality,
  higher short-input output speed,300/500/1000tok/s or eight-GPU admission.
- Fixed native tool arguments or WorkBuddy GUI/tool-call acceptance.
- A Rust-native serving runtime: this is Xolotl C++/CUDA plus its Python API.
- New full Rust CI or a new workspace-formatting pass. This revision retains
  candidate source/validation and deployment evidence; prior full formatting
  limitations in the private Studio pin remain separately recorded.

The active user goal remains full single-H2001M XQ inference-speed optimization.
