# Actual unique-code1M, SIMD and literal preprocessing (R8)

The unique public-code request uses1047544 input tokens,1024 output and8
safety. Its first cold request completes in2050.044s with2012.482s TTFT;
native prefill is2004.21s (522.7 input tokens/s). Cold decode is27.24token/s.
The restored response matches the complete cold output hash. Eight serial
cached requests emit8192 tokens in351.105s:23.3321 API-wall token/s,28.4002
stream-interval token/s and7.8553s TTFT P50. All eight concurrent2 calls
finish1024 tokens and retrieve all three codes, but their full output hashes
differ. The original C2 identity gate fails and remains unchanged. This is
not a qualified concurrent-throughput result or a controlled comparison to
the earlier repeated-filler prompt.

Native numeric phases locate1208.740s in dense GPU work and399.158s in
Engram waits across41080 attention spans; summed nested counters may overlap.
Cached native prefill is0.02–0.03s while API TTFT is about7.9s. Repeated
bounded process samples show actual GPFS row reads blocked in pread64. These
observations guide both GPU work and frontend/I/O optimizations.

The AVX2 Engram decoder preserves all786432 FP8/scale/rounding/alignment
cases in each of control, enabled and ASAN/UBSAN modes. The default-off
object and full parent native binary reproduce byte-for-byte. Only the
Engram member changes for the candidate; dense CUDA code remains identical.
Its37.57x CPU conversion microbenchmark is not a model speed result.

The same GPU099e runs trace-control / SIMD / trace-control, each with28
basic and12 native-tool/matched1024 calls. All120 pass unchanged gates.
API-wall long-output rates are75.742894 /76.311310 /75.503822 token/s:
only0.75–1.07% observed improvement. The35700 cold TTFTs are37.944368 /
37.938605 /38.581855s. Real1M SIMD input has not been tested and this
candidate has not replaced the formal service. The observer's100-second
timeout is retained; exact remote client completion was observed and the
same matrix resumed without replaying completed requests.

CPU replay finds two expensive literal passes: marking3.19s and unmarking
4.48s. The bounded default-off scanner matches declared tokenizer literals
exactly; unsafe priority overlaps use the original code. Contiguous marker
code points are compressed into exact regex ranges with no gaps included.
The real Service.encode_prompt A/B/A times are9.153778 /1.647161 /9.126680s,
an approximately82% CPU encoding reduction with identical complete ID arrays.
Fifteen hermetic helper methods and25 unchanged generic frontend methods pass.
The first generic-test attempt after DS41 globally patched Service is retained
as a harness failure; the successful revision isolates generic tests in a
fresh child before importing DS41. The actual overlay then passes14 CPU
suites across both startup flags, including the complete real1M encoder.
This is CPU evidence; GPU/API TTFT and decode gains remain pending.

The earlier public fixture metadata used direct template tokenization.
Actual API literal protection changes six ID values, preserving1047544 count,
body bytes, and record positions36/523698/1047424. The canonical ID digest is
b442a593a4066e3e99b663b670db4d7d624c471f1c5f6ee906e315c99d1969c4;
the old direct-template digest ebe492 is retained as history. GPU V119
verifies the submitted body, actual count and retrieval, but did not capture
the native runtime input-ID digest. The corrected canonical metadata is
append-only; no old evidence or input body is rewritten. A same-native,
same-GPU real1M baseline/literal API campaign is running separately.
The C11 formal-service recovery overlaps its control cold-prefill run;
any cold-prefill difference needs a quiet repeat before attribution.

A read-only H200 snapshot during actual1M prefill records98% busy,
1980MHz SM/3201MHz memory matching reported maximum clocks,700W power limit,
48C GPU temperature and no active clock-throttle flags. These readings are
not Tensor Core efficiency, achieved bandwidth or a hardware speed ceiling.
The installed Nsight Compute2024.3 CLI is inventoried and the driver reports
RmProfilingAdminOnly0. No hardware counter collection has been executed yet.

C9 is unexpectedly Stopped before its recorded lease; cause and actor are
unproven. Exact terminal job/replica evidence and failed Windows health are
retained. C11 replaces only that terminal capacity while C10 continues,
preserving maximum2 H200. C11 keeps the qualified nativefa/S128/API f062
profile and same VIP/key, passes fresh28+12+strict132097 and Windows unique
backend-nonce cache reuse. No experimental candidate is promoted. Two
prelaunch operator-reference errors are retained; neither created an extra
allocation nor signalled a model process.

## Commands and scope

- manage_code_fullwindow_suite_v119.frozen.py: strict132097 plus20
  fullwindow-suite calls, including18 actual1M calls; cold/restore/c1 pass,
  complete-output C2 identity fails.
- manage_engram_simd_cpu_v137.py / manage_native_phase_simd_v138.py:
 786432 cases per mode, sanitizer proof and exact parent reproduction.
- run_phase_simd_matrix_v140r1.frozen.py:120 completed model calls across
  three same-GPU profiles, no formal service change.
- manage_literal_scan_cpu_v145r1.py:15 helper +25 isolated upstream methods
  and real protected API ID-exact A/B/A;14 API-overlay CPU suites via V148.
- run_literal_api_comparison_v151.frozen.py: pending real1M GPU/API comparison,
  unchanged20-call/cold/restore/c1/c2 gates and a finite7200s campaign budget.
- run_xolotl_stable_recovery_v157r1.frozen.py and promotion/finalization:
  exact C11 resource, fresh-instance gates and same-VIP Windows verification.
- collect_actual1m_simd_public_v146.py:55 closed bounded public diagnostics,
  actual private-key negative controls. Active control_b engine log excluded.

No300/1000 single-request achievement, goal completion, full DFlash/DeepInfer
integration, fresh WorkBuddy GUI acceptance, actual1M formal C11 execution,
new Rust workspace CI/fmt result, raw profiler report, model weight, binary,
credential or private client state is claimed or published here.
