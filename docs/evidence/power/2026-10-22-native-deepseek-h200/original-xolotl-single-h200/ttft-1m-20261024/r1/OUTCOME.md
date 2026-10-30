# Single-H2001M prefill scheduling optimization, R1

The same public prompt mix and cluster-side client were run on the isolated
optimization GPU117a, context1048576. Each round includes4 warmups and16 measured
short requests, cold/restored35700-token arithmetic, and an actual simultaneous
long/short request pair. Formal VIP serving on GPU0349 remained unchanged.
Measurements are API first-content latency and API wall throughput, not isolated
GPU decode. One mixed pair per round is not a mixed-load latency distribution.

V9 baseline: short P50.1267s/P95.1601s; cold35700 48.302s; restored.052s;
mixed-short47.183s. V10 upstream two-slot mode: mixed-short24.067s. V11 complete
DSpark seed-window capture, graph-aware cache reserve and static-cache/batch4096
profile: mixed-short12.976s, but short P50.1770s and wall throughput68.45tok/s.
That short-request regression was retained and not promoted.

V13 retained-LRU request was refused by the existing mixed-size cache guard;
the guard was not removed. QoS initial1024/replay2560 scheduling alone reduced
mixed-short to9.628s, short P50.1279s. V15 computes every layer of the first1024
positions and can hand off immediately after that complete checkpoint. It keeps
the cache guard, with unsupported keep-LRU mode disabled. Mixed-short5.383s is
88.6% below the V9 baseline; a second public retrieval/isolation pair gave5.377s.
Short P50.1291s/P95.1629s; wall throughput84.13tok/s; stream-interval estimate
93.71tok/s. Cold35700 49.219s is1.9% slower than V9; mixed-long53.044s is slower
than the47.542s baseline. These tradeoffs are not hidden by the short-request gain.

Fresh and yielded35700-token begin/middle/end record retrieval both returned the
exact expected JSON values; interleaved request53 remained separate. V15 has28
successful public requests including warmups. The original131073-token failure
edge passed with V11 complete seed capture. The QoS profile moves that exact
one-token remainder to132097 (=1024+16*8192+1), now being qualified separately.

GPU-free controlled builds reproduce the exact qualified e9433050 executable
from its original object/archive set, then replace only the generate host object.
Host policy tests pass72/76/80 cases for V11/V13/V15, plus strict flag and replay
floor assertions. GPU kernels, model weights and expert routing are unchanged.
Original Xolotl/ggml provenance and MIT notices remain in the native source
manifest and crates/deepseek/licenses/Xolotl-MIT.txt.

Parent source was merged/pushed at b1773a1416d38f00c05519750ccd77f2e2452af1;
Power remains the published c891f930 dependency. Native post-merge all-target
server check passes. Exact WSL full formatting still fails in the inherited
private Studio pin a7856a9c because app_updater/os_feed_tests.rs is absent.

Explicitly not claimed: production promotion of these candidates; actual full1M
length/quality acceptance; repaired native tool arguments; WorkBuddy GUI/client
latency acceptance on the candidate;300/500tok/s; eight-GPU admission; full CI;
a pure GPU decode rate or a sustained mixed-load P95. Runtime keys, model binaries,
private WorkBuddy state and private production prompt/output logs are excluded.
