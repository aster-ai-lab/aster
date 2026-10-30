# H200 Top-K GPU comparison and stable-service recovery (R10)

The actual CUDA12.6.20/CUB2.5 engine links a full-argsort Top-K fallback.
The isolated official CCCL3.5.0 packed-key selection prototype passes all38
original-reference ordered-index cases, including ties, signed zero, NaN/Inf,
changed inputs and CUDA Graph replays. Both release and ASAN/UBSAN modes
pass14,680,074 integer-order contracts. No model forward is performed by
the operator harness.

For random finite1M-column inputs, synchronized operator host-wall times
change from100.484766us to61.133203us for one row,7255.715430us to301.214648us
for six rows, and7700.772266us to811.849219us for17 rows (candidate graph).
The six-row observation is approximately24.1x. These are operator timings,
not model/API token rates. All4097-column cases regress; the private native
bridge leaves the original path below65536 columns and for K other than512.
The bridge defaults off and refuses unknown mode values before main/model
loading. The original native99f3 control reproduces byte-for-byte; the separate
candidate2bc83 is compiled without replacing the stable native or archives.
Exact model metadata confirms Top-K512,40 layers and eight index-source layers.

The real1M profiler's first revision incorrectly required RESUME0, although
the native protocol emits the marker only for positive reuse. The cold128-ID
completion and failure are retained. Eighteen numeric/query/native-protocol
parser methods pass before R2. R2 then completes three1047544-input/128-output
controls with identical IDs. The fourth GEN reaches its decode marker, but
Nsight Compute aborts its first mul_mat_vec_q capture and the target exits9.
No actual-model hardware counters are collected. Native/worker cleanup is
verified. A later own-cgroup observation shows a220GiB peak and one OOM kill;
the exact killed process and causal allocation are not established.

A separate bounded tiny CUDA kernel correctly writes65536 words and obtains
all eight hardware metrics. The old parser assumes long-format CSV, while
installed NCU raw output is wide with a units row. Its failed receipt remains.
Seven hermetic wide-parser methods pass, and the same existing CSV is reparsed
successfully without another GPU run. Only eight whitelisted numeric metrics
are published. They characterize the tiny kernel, not model utilization.
Raw reports and CSV remain private. Nsight Systems2025.3.1 discovery supports
one visible H200 and cuda-visible metric sampling; no such sampling is started
in this revision, preserving the timed model campaign.

After all C10 workers finish and the optimization GPU is idle, C10 is stopped
and verified terminal before one replacement C12 is submitted. C12 obtains
the same physical H200099e, preserving the independent formal card and the
maximum two-GPU scope. The same native2bc83 binary is prepared for flag0/1/0
API A/B/A with the unchanged40 ordinary and20 fullwindow calls per profile.

At06:53:29UTC the independent formal C11 unexpectedly becomesStopped before
its recorded lease. Its worker health was200 approximately two seconds earlier.
The experimental controller stops subsequent work. Exact status and declared
lifecycle policy are retained: maxRunningDuration0s, autoDelete720h and
nonpreemptible. Neither actor nor root cause is established. C11 is terminal
before replacement C13 is requested; maximum allocated task GPUs remains two.

C13 recovers the same qualified nativefa019/S128/API f062 stable profile.
Fresh28 basic,12 native-tool/1024-output, and strict132097-input gates pass.
The existing VIP/key is switched and Windows nonce/cache-reuse verification
passes, with a47ms first token for the short nonce call. The stable8x1024
API-wall result is72.941584 token/s. Fresh C13 actual1M execution and WorkBuddy
GUI acceptance are not claimed. Read-only readiness observation is restored.

The prepared213r1 controller resumes the existing C12 control_a service and
completed basic-client handle after formal recovery, passes the12 extended
gates, and starts the complete actual1M suite. Its comparison remains pending.
The prior C2 full-output identity failure remains visible; no Top-K model gain,
experimental formal promotion,300/1000 achievement or goal completion is claimed.

## Scope and reproduction

- manage_topk_harness_v203.py: compiled actual original-reference operator
  harness; launch_gpu once then observe_gpu, all38 GPU gates passed.
- compile_topk_native_bridge_v202.py: separate default-off candidate, parent
  binary reproduction and seven invalid-mode-before-main refusals.
- manage_ncu_actual1m_v172r2.py observe: existing failed/reaped diagnostic.
- recover_ncu_smoke_and_memory_v207.py: seven wide-parser tests plus existing
  tiny-kernel CSV recovery; no additional GPU execution.
- run_topk_native_model_aba_v213r1.frozen.py: existing C12 full-window A/B/A;
  observe_current_h200_roles_v219.py verifies C13/C12 and Windows health.
- run_xolotl_stable_recovery_v217.frozen.py, promote/finalize217: completed
  unchanged stable40/132097 and Windows backend/cache verification.

No binary, weights, third-party header archive, raw profiler report/CSV,
credential, private client state or new Rust workspace CI/fmt result is published.
