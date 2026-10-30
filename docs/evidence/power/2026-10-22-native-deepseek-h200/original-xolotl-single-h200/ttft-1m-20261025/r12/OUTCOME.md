# Actual H200 GPU findings and service recovery (R12)

All numbers below have distinct acceptance scopes. No new experimental native
binary was promoted to the formal service.

The completed TopK actual-1M flag0/1/0 comparison uses 1047544 input and 1024
output tokens per call. C1 complete outputs match across all three profiles.
API wall throughput is 23.3733 / 35.5234 / 23.4142 token/s: candidate improves
51.98% and 51.72% against the two controls. C2 full-output identity fails in
all profiles. This is a qualified C1 result, not complete model acceptance.
The cause of the concurrent-output divergence remains unproven.

An MXFP4 header bound avoids dequantizing a key row just to decide whether a
masked row may be skipped. Actual CUDA float/half boundary tests, 1152 helper
cases, and the original 450 full-score gates pass. A subsequent actual replay
test fails six BF16 cases; that retained failure exposes a separate existing
CUDA Graph cache defect. Switching F16 to BF16 with identical pointers, shapes
and strides reuses a stale F16 kernel: 390 of 780 scores are wrong. Disabling
graphs yields zero wrong scores. Both UID0 and nonzero UID paths are tested.

The default-off source-type guard checks exact input types before the UID
fastpath. Its separate bounded host cache preserves the upstream graph ABI.
Release, ASAN/UBSAN, concurrent cache and closed-mode host checks pass. Actual
GPU qualification passes 1350 graph replay cases against 270 original direct
oracle cases, including changed inputs and same executable replay. Each profile
has 416340 byte-identical score values; compute-sanitizer reports zero errors.
The original raw CUDA control object differs only in one typed STT_FILE NVCC
temporary filename. A separately retained normalization proof rejects code or
symbol mutations and establishes exact equality of all other object bytes.

With that correctness guard fixed, a separate same-binary million-row operator
flag0/1/0 benchmark preserves every full-score array. The MXFP4 header candidate
regresses unmasked cases by about 2-11.4%, while 90%-masked synthetic cases gain
about 53.6-72.1%. Global enablement is rejected and remains off. These are
operator timings, not model token/s.

The guarded CUDA core is now linked into a complete native model binary. The
unchanged 2bc83 control relinks byte-for-byte; eight invalid guard modes refuse
before model load. A new C15 same-binary guard0/1/0 full-model comparison has
been prepared and started, preserving the original 40 ordinary requests and
20 full-window calls per profile. No result from that running comparison is
claimed here. Original indexer kernels remain unchanged; the rejected header
candidate is excluded.

The old C13 formal allocation was officially Stopped; its actor/cause is not
proved. C14 replaces it with the unchanged fa019 native binary and the repaired
API5834/frontend11215/toolhelperfda429. All 40 ordinary requests and strict
132097-input checks pass. Short-input 8x1024 API wall throughput is 74.9084
token/s. Windows reaches the same VIP directly, with the same key and no
Windows forwarding. Unique-prompt cache priming on C14 identifies the serving
backend. Installed WorkBuddy CLI Read returns 442; repeated searches terminate
after three calls and report COMPLETED=3. GUI and authenticated WebSearch
success remain unverified. The installed CLI first-output time is not the
model's isolated TTFT.

Recovery preserves the failed stale-manifest and obsolete API-hash attempts.
The corrected current service manifest is checked against actual C14 files,
API/native PID identities and the live plan. C12 is officially Succeeded before
one C15 optimization H200 is requested; maximum active task roles remain two.

Hardware collection remains incomplete for actual model 1M decode. Three
completed native 1M/1024 controls agree with each other but differ from the
API canonical output. API rendering and effective-argument checks do not
explain it. A tiny interactive NSYS workload verifies manual capture/export;
the retained decode-window parser requires exact successful start/stop APIs,
same thread, an ordered unique pair, and no straddling kernels. This is not an
actual-model hardware trace.

Validation: retained immutable operator workers/probes and receipts; published
directory Python decode-window tests (8); full append-only evidence hashes;
two actual-key negative controls; scoped git whitespace checks. Rust workspace
checks were not run because this landing changes deployment evidence and
standalone CUDA/C++ experiment sources only.

## Explicitly not claimed

300 or 1000 single-request token/s, goal completion, complete C2 acceptance,
fresh formal actual-1M execution, WorkBuddy GUI/authenticated-search acceptance,
an actual-model decode hardware trace, or a causal link from the graph-cache
defect to the prior native/API or concurrent-output mismatches. No weights,
binaries, credentials, raw profiler reports, user queries or session state are
published. Resource leases and service availability are point-in-time proofs.
