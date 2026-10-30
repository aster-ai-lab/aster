# Actual full-window qualification and next single-H200 candidates, R2

V15 completed an actual 1,048,504-token public input, 64-token output budget and
the existing eight-token API safety margin in a 1,048,576-token context. The
first attempt omitted that margin and returned HTTP 400 before model execution;
both attempts are retained. The guard was preserved.

Cold first content took 2,226.692 seconds; completion took 2,227.226 seconds.
The 17-token JSON answer retrieved the exact beginning, middle and end record
values. An unrelated request returned 53; restoring the original full context
returned the same correct JSON, with 1,048,504 cached tokens, first content in
1.396 seconds and completion in 1.903 seconds. The cold engine counters report
2,222.74 seconds of batch work: dense 1,485.87, experts 701.55; cache opening
took 2.45 seconds. Expert time is not asserted to be pure PCIe transfer time.

The following cache-hit load used the identical full-window prompt, 17 actual
output tokens per request, two warmups and 16 measured requests per level:

| Concurrency | Successes | API wall output tok/s | First-content P50 / P95 | Stream-interval tok/s estimate |
|---|---:|---:|---:|---:|
| 1 | 16/16 | 9.331 | 1.374 / 1.410 s | 38.730 |
| 2 | 16/16 | 12.142 aggregate | 1.893 / 4.716 s | 33.967 per stream |

Every response passed strict three-position JSON and cache-accounting checks.
This is a fixed-input cache-hit load, not cold concurrent prefill, long-output
generation, general million-token reasoning quality, or a sustained-load P95.
The earlier 84.13 tok/s result is for short inputs with a configured 1M window.

V15 was promoted to the existing server VIP and existing authorized credential.
The exact Service UID/resourceVersion, previous selector, new allocation UID,
owner reference and bounded lease were checked. Windows directly received a
complete SSE answer of 42; that single cached 18-token input took 0.047 seconds
and is not a latency distribution. The new server-side expiry helper preserves
a Service reassigned to another allocation. The previous API was idle before
SIGTERM, stopped/reaped, and its RJob and Replica were explicitly stopped before
requesting the next single H200. Maximum simultaneous task-owned GPUs remains
two. No Windows forwarding or resident Windows helper is involved.

The successor resource passed actual H200 CUDA validation and a fresh-device
synthetic cache gate. Its six-hour frozen lease ends at the time recorded in
the resource receipt. The public V15 allocation has its separate finite lease.

V18/V19 add a separate checked mixed-slot cache-retention opt-in. Before DMA,
the complete live/restored map checks slot ownership, expert identity, sizes and
capacity. Exact-slot restore refuses collisions. V19 also suspends mixed-cache
LRU admission while saved tail slots are loaned, including the one-token prefill
remainder, and refuses CUDA-copy failures in byte verification. Both controlled
builds reproduce the exact V15 control executable before replacing only the
MoE host object; 30 host ownership/capacity cases pass. Existing debug-print
compiler warnings are retained. GPU kernels, model weights and routing remain
unchanged. The synthetic test passed on both idle optimization devices: three
shrink/grow cycles, 194 exact-slot restorations and 478 slot-byte comparisons
per device. Real-model retained-cache performance is not established by that
synthetic gate.

The tokenizer candidate memoizes only identical UTF-8 text and explicit boolean
special-token mode, per tokenizer instance. Literal-span overrides use the
original encoder; every result is a fresh list. Its retained entries have a
32 MiB accounted-byte bound and four-entry limit. It infers no prefix, performs
no normalization or truncation, and writes no prompt cache to disk. On the same
CPU process, original full-window encoding median was 1.137 seconds; first
memo fill was 1.208 seconds and five-hit median was 0.027 seconds. Exact IDs for
all 1,048,504 tokens matched SHA-256
`b9b006a173dc68f8da156b6bd01e0d90c3abdef62f24561475e8b4fa17e5374c`.
Twenty-four Unicode/mode/literal-span comparisons, ten hermetic memo tests,
23 upstream security tests and the upstream DS41 regression command passed.
These timings are CPU-only on the jump host, not H200 decode or API speedups.

Commands retained in `operators/` include the full-window V17 launcher and
worker, V20 full-window reuse load, V18/V19 controlled builds and observers,
GPU-gate compilation/runners, V21 CPU tokenizer staging, V15 promotion and
Windows finalization, guarded VIP cleanup, old-resource stop, successor
submission, and V23 candidate launch. Their receipts and source SHA manifests
are retained beside the frozen source candidates. V23 combines checked cache
retention and exact token memoization on the separate successor H200; its
real-model qualification was started while public V15 remained available.

Its first real 35,700-token cold request completed correctly in 109.915 seconds,
versus V15's 49.219 seconds. Counters recorded dense 16.04 seconds, experts
37.68 seconds and 54.901 seconds in cache regrowth with 1,151 actual expert-slot
byte checks, all correct. Verification overhead explains much of the regression;
even its recorded dense-plus-expert sum does not establish a prefill speedup.
The candidate runs on a different H200/host. Its 28-request basic qualification
subsequently passed, including fresh/yielded three-position retrieval and
independent 53. Mixed-short first content regressed to 31.115 seconds; restored
35,700-token first content was 0.040 seconds. The initial partial receipt and
separate final receipt are both retained. Full-window candidate qualification
is still pending. Cache hits alone are not accepted as a performance result.

Source provenance: the original MoE host source SHA is
`e123b8209be7cd0b9da8ceaf29c97d2e7edf2ed43850e8525ad89261aa4599e0`;
the original deployed Python adapter SHA is
`5180c184d1a421dd5b56e799360fe6306b96d41da2d443d0bb68dae6714f2e5d`.
The upstream Xolotl MIT notice remains in
`crates/deepseek/licenses/Xolotl-MIT.txt`. Runtime credentials, private WorkBuddy
state, model weights and compiled binaries are excluded from publication.

The publication audit verifies staged blob bytes against the SHA manifests and
uses both actual runtime credentials as negative controls. Unified patch files
preserve their mandatory single-space blank context lines; only those patch
data files have an end-of-line whitespace attribute exception. Complete source
snapshots continue to receive ordinary whitespace checks.

## Explicitly not claimed

- V19/V23 real-model speed improvement, completed V23 full-window qualification,
  or promotion of the tokenizer/cache candidates.
- Pure GPU decode figures, 84 tok/s for full-window input, long-output 1M load,
  high concurrency, eight-GPU admission, or indefinite service availability.
- Repaired native tool arguments or WorkBuddy GUI/tool-call acceptance. The
  original API's known empty-arguments defect remains open.
- A Rust-native serving engine: this runtime remains Xolotl C++/CUDA plus its
  Python API. Native Aster source work and its earlier tests are separate.
- Full CI or a new full-workspace formatting pass. This landing retains
  deployment source/evidence and updates source provenance; unrelated product
  files and their existing formatting limitation are unchanged.

The active user goal remains full single-H200 1M XQ inference-speed optimization.
