# Natural-prefill Engram path comparison and rejected FP8 decoder (R6)

The same physical GPU355/native34f/API f062/S512/memo1 profile runs fresh
processes in GPFS/local/GPFS order. Only the Engram table directory changes;
all profiles explicitly use2M raw row-cache entries. The203GB original/local
full-SHA table proof is retained in R5. Original source page caches are not
evicted. This measures the complete storage path and its cache behavior,
not an isolated physical-disk or guaranteed cold-OS-cache effect.

| Profile | 8,244-token cold TTFT | 25,671-token cold TTFT | Short-input1024-output API tokens/s |
|---|---:|---:|---:|
| gpfs_a | 17.470967s | 29.255494s | 75.571289 |
| local | 11.101519s | 21.916497s | 74.949695 |
| gpfs_b | 15.061002s | 26.934768s | 75.474930 |
| lut_local | 10.166466s | 20.888774s | 75.752442 |
| original local repeat | 10.077658s | 20.661710s | not measured in this seven-request repeat |

Each directory profile passes seven exact natural-input/KV-restore requests
plus twelve native-tool/1024-output requests,57 total. Inputs8244/25671 are
public technical documentation built from committed sources with exact
tokenizer counts and retained bytes/IDs. All three retrieval values and
restored complete output hashes match. Both GPFS controls remain slower
than the local path; continuous short-input decode has no material gain.

The opt-in FP8 lookup candidate covers all256 encodings and all256 scale
bytes under four rounding modes:262144 scaled and1024 conversion cases
per run. Control0,enabled1 andASAN/UBSAN enabled1 pass; invalid mode2 is
refused. Exact original and default0 Engram objects reproduce byte-for-byte.
The native34f control executable is reproduced before replacing only the
Engram archive member. The candidate passes19 real model requests with
unchanged tool, retrieval and complete1024-token outputs. Its apparent
improvement versus the first local block disappears against the repeated
original local control. No model/API gain is established and it is not adopted.

Two startup-budget refusals occurred before any model child existed.
Their original receipts/controllers are retained. Exact staged inputs,
private credential hash, absent child/result/log files, idle GPU and the
unchanged resource expiry were verified before recovering the same roots.
The second finite seven-request client has420s process/480s lease bounds;
its retrieval and output-identity checks are unchanged. No GPU was added.

The bounded short-input concurrency diagnostic retains all complete rows,
same/mixed-prompt pair walls, exact reference results and post-serial checks.
Its phase is failed with 8 recorded requests.
Read its failed/passed scope in round-summary.json; it cannot qualify or
replace the still-failed V56 actual1M concurrency suite.

## Commands and counts

- run_xolotl_engram_comparison_v71.py: first two19-request profiles pass;
  final bootstrap refuses its obsolete1800s startup floor before launching.
- recover_xolotl_unstarted_gpfs_b_v71r1.py and finish_xolotl_engram_and_lut_v78.py:
  exact final19-request directory profile, controlled native link and19 decoder requests pass.
- prepare_xolotl_engram_fp8_lut_v72.py, run_xolotl_engram_fp8_cpu_v72.py and
  build_xolotl_engram_fp8_cpu_v73.py: exhaustive CPU checks and exact object reproduction.
- repeat_xolotl_local_control_v82.py: original720s startup guard refusal retained;
  recover_xolotl_unstarted_local_repeat_v83.py and finish_xolotl_local_repeat_v84.py:
  seven unchanged original local natural-input controls pass.
- manage_xolotl_short_concurrency_v85.py launch/observe: finite original-native
  diagnostic; exact terminal result retained without changing its identity gate.
- collect_xolotl_engram_numeric_v71.py: bounded numeric source-linked model
  diagnostics and read-only formal observer proof. No inference on formal GPU.

## Explicitly not claimed

- No300/500/1000 token/s single-request qualification or goal completion.
- No actual1M input in this round, natural-language1M capacity proof,
  cold1M concurrency, long soak or general P95 guarantee.
- No FP8 lookup API speed gain/adoption; no full1M concurrent-identity fix
  or established root cause from a short-input diagnostic alone.
- No promotion to the formalGPU5539 service, no fresh WorkBuddy GUI or
  cold-first-turn acceptance and no Rust-native serving-runtime claim.
- No new Rust workspace CI/fmt claim. Only external candidate sources,
  public fixtures, operators and bounded evidence are retained; no model
  weights, binaries, compiled objects, credentials or private client state.
