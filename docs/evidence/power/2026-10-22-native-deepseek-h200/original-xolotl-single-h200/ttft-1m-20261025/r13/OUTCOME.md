# Bounded actual-decode tracing preparation and upstream review (R13)

The graph-type guard full-model flag0/1/0 comparison is still in progress at
this retained checkpoint. Its first profile passed28 ordinary and12 extended
requests, including tools and byte-matched1024 output, then strict132097 input.
The short-input8x1024 API wall baseline is75.2938 token/s. The active1047544-input
cold request has not completed; no new actual1M speed gain is claimed.

The next profiler is prepared to capture the complete fourth actual1M/1024
native GEN manually, using the exact observed C15 device0/UUID/PCI mapping.
After export it selects only the unique ordered successful same-thread
cudaProfilerStart_v4000 / cudaProfilerStop_v4000 interval. Straddling kernels
are refused. Three unprofiled controls and the original canonical1024 output
gate remain required. A retained debug trace may be exported before the final
canonical-output failure; it never changes that failure into acceptance.

The literal metric reader is validated on the retained actual CUDA smoke trace:
30 exact metric identities,18000 samples in a6.0032-second marker window. Its
first strict attempt rejected negative integer warp values. Its next attempt
rejected an empty typeName label, which all30 actual identities contain. Both
failures and original parsers are retained. The corrected reader keeps the
empty type label and definitive metric name/typeId/metricId exactly, preserves
all signed raw values, and qualifies no physical utilization values. It does
not clamp negative samples, infer aliases, convert clock units, or discard
uncomfortable data. Numeric identity, coverage, finite-value and cardinality
checks remain enforced. CUDA timing and model-output gates remain separate.

A finite Windows continuation waits for the existing comparison375 and every
model supervisor to drain. Only then may it run a fresh physical-C15 tiny
manual start/stop/export check and the expensive actual1M diagnostic. It
checks the exact resource role, independent formal health and remaining lease,
never retries ambiguous launches and requests no extra GPU allocation.
These stages are prepared; a new C15 GPU profiler result is not claimed.

The exact upstream Xolotl revision0b9a0fc was fetched as a source archive inside
the existing workspace, with its hash retained. It is36 commits ahead of the
active e9d0826 baseline. The active comparison and formal engine are unchanged.
The newly lowered short-turn batch threshold128->16 can be tried through an
environment variable already supported by the old native binary. It is a TTFT
candidate, not a measured decode-speed gain. The conversation-seeded VRAM
change requires mixed expert blob sizes; applicability to the currentV3 is
unproven. CPU expert and file-tier optimizations have their own preconditions.
The upstream decode document measures a5090 Windows configuration, not H200;
it motivates checking small kernels and per-layer host round trips in our own
trace, without transferring its performance claims to this server.

Validation command from helpers/: python -B -m unittest
test_nsys_decode_window_v321 test_nsys_raw_decode_metrics_v386
test_native_resume_v197 test_full_output_protocol_v247 test_nsys_lifecycle_v302
(36 methods). Both literal raw-parser unit tests and the actual retained
SQLite proof also pass on Linux. Full evidence hashes and two actual-key
negative controls pass. Operator scripts freeze private deployment paths and
are not a standalone install bundle. Rust workspace checks were not run.

## Explicitly not claimed

Completed guard full-model comparison, new actual-model decode trace, validated
physical GPU counters, fresh formal actual1M input acceptance, WorkBuddy GUI or
authenticated-search acceptance,300/1000 single-request token/s, experimental
formal promotion or goal completion. No keys, sessions, user content, model
weights, source archive binary assets or raw profiler reports are published.
The same verified C14 formal VIP/key/native remain in use at the checkpoint.
