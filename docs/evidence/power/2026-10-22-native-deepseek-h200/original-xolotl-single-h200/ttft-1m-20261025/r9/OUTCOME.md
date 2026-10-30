# Canonical real1M literal API comparison and hardware profiler preparation (R9)

The same H200099e/native99f3/S512/1M/batch2 configuration completes the
baseline/literal API comparison. The literal overlay passes28 standard and12
native-tool/1024-output gates. Short-prompt8x1024 API-wall throughput is
75.503822 versus75.789067 token/s, an observed0.38% difference.

Each fullwindow profile completes20 calls, including18 actual1047544-token
inputs with1024 output tokens. Cold, isolated-restored and all8 sequential
full-output SHA values match. Sequential8-call API-wall throughput is
23.040565 versus23.101921 token/s; TTFT P50 is7.889305 versus7.884743 seconds.
These differences do not qualify a material end-to-end improvement. The CPU
encoder's earlier82% isolated timing reduction is not an API speed claim.
Both profiles'8 concurrency2 calls structurally finish and retrieve all3
records, but complete-output identity fails. Their failed acceptance remains
visible and no candidate is promoted. Baseline cold prefill overlapped C11
recovery; cold-time differences are not a controlled attribution.

Exact live API PID/start/UID, public startup flags, four source hashes and
closed disabled-helper warning counts were inspected without an inference
request or exposing other environment variables. Flags1/1 and pinned files
are present, with no declared disabled-helper warning. This is not an
in-process scanner-hit trace or a native runtime input-ID digest capture.

The canonical sender fixture has1047544 protected API IDs and digest b442a593.
Its private u32/CSV files are not published; exact bytes/hashes and generator
source are retained. Default-off native fourth-GEN decode hooks pass20015
lifecycle contracts in each of release and ASAN/UBSAN modes. The original
host object reproduces byte-for-byte, and all original GPU archives remain
in the subsequent controlled-link plan. The planned diagnostic keeps three
unprofiled128-ID controls before capturing the fourth decode; it does not
replace the1024-output API gates or establish actual1M API throughput.

Installed Nsight Compute2024.3 CPU stdin/stdout smoke passes. Its all-metric
query ignores the intended base filter in all mode and exceeds the declared
read bound. That refusal is retained. Only the waiting local observer was
stopped; no model or formal service was signalled. A suffix-only CSV query
then confirms all8 exact metrics on NVIDIA H200 in67053 bytes. Twelve
hermetic numeric/query parser methods pass before the revised capture launch.
Clock/cache controls remain none; raw profiler reports remain private. The
new actual-model hardware-counter run is pending this snapshot.

A CPU-only executable links the exact original Engram host object and uses
its public setup_hash/hash API on the same canonical IDs. It opens no row
table and executes no model forward or GPU work. The50,282,112 references
map to12,953,683 unique layer/row keys:6,476,814 and6,476,869 by layer.
Their264-byte raw rows total3,419,772,312 bytes, excluding map overhead.
This supports testing16M cache rows against the present2M fill-only cache;
capacity, hit ratio and actual inference speed have not been changed/tested.

## Commands and scope

- run_literal_api_comparison_v151.frozen.py: completed same-native API A/B;
  manage_literal_fullwindow_suite_v150.py observe --profile control|literal.
- manage_serve_profile_cpu_v167.py: exact host object and20015x2 lifecycle proof.
- preflight_installed_ncu_protocol_v176.py: retained overbroad metric refusal;
  inspect_ncu_metric_suffix_v177.py and12 parser methods confirm the revision.
- run_fullwindow_then_hardware_profile_v173r1.frozen.py: after completed API
  calls and exact idle drain, link168 then bounded actual1M profiler172r1.
- run_engram_hash_footprint_v181.py: one finite isolated jump-host CPU footprint.
- observe_hardware_pipeline_v175.py: exact C11/C10 resource identity and direct
  Windows formal health, without inference or clock/cache policy changes.

## Explicitly not claimed

No material API improvement, fixed C2 determinism,300/1000 single-request
achievement, goal completion, actual-model hardware counters, kernel speedup,
larger cache performance, fresh WorkBuddy GUI acceptance, fresh formal C11
actual1M execution, new Rust workspace CI/fmt result, raw profiler report,
weight, compiled binary, API credential or private client state is claimed
or published. The formal C11 VIP/key/configuration remains unchanged.
