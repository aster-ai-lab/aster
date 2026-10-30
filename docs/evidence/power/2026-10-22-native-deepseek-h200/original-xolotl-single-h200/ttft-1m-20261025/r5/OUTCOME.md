# Actual1M long output, strict tool repair and service recovery (R5)

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
