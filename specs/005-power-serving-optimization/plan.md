# Implementation Plan: Power serving optimization

**Branch**: Existing branch; no branch/commit requested | **Spec/Input**: [spec.md](spec.md)

**Gate context**: Complete P0–P5 implementation and qualification for the existing user-directed Power/llama-server execution service and Aster Gateway, with no Cloud contract changes.

## Summary

Extend the existing Power Prism provider, API metrics and admission owner; preserve the shared public chunk envelope and Gateway observation schema. Add reproducible qualification scripts/tests to Aster, compare preserved and changed builds on distinct ports, run the full GPU/cache/speculation/profile campaign and promote only configurations supported by correctness and measured evidence. P4/P5 are conditional engineering investigations with mandatory probes and recorded decisions, not omitted phases.

## Optimization route and release decisions

| Priority | Deliverable | Evidence required to proceed |
| --- | --- | --- |
| P0 | Truthful timing/usage and explicit HTTP errors | Compare client meaningful-output timestamps with actual native counts; never infer tokens from SSE frames. |
| P1 | Incremental text/tool streams and cancellation | Real Gateway → Power → native queued/pre-output/mid-output cancellation, idle accounting and recovery; reject truncated streams. |
| P2 | Finite admission and cheaper metadata loading | Active/waiting/deadline bounds, uniform overload refusals, privacy tests and cold/warm separation. |
| P3 | Choose one versus two versus four slots, then eight replicas | Same model and 32768 context per sequence; three matched finite repeats, correctness, latency, throughput, queue and whole-card VRAM gates. |
| P4 | Speculative decoding, conditional | Compatible runtime and actual draft/acceptance counters; target-output parity, partial rejection, cold history, isolation and concurrent-load tests before promotion. |
| P5 | Executor changes, conditional | Measured CPU/GPU gaps, overhead and a reproducible bottleneck; reject regressions and unsupported model/kernel substitutions. |

DeepInfer supplies scheduling, ownership and verification ideas. Its published
SM100/DeepSeek-V4.1 implementation is not a supported replacement for the current
SM89/PTQ1 hybrid model. The 27B model already fits one GPU; this route measures
independent replicas and per-replica slots before introducing cross-GPU execution.

Current qualification checkpoints are recorded in `OUTCOME.md`, not asserted as
release acceptance here. Four slots have triggered the predeclared 90% whole-card
guard while co-resident with the existing worker. That result rejects this
co-resident profile; it does not establish an absolute four-slot runtime limit.
Preserve 32768 per sequence rather than shrinking context to pass the guard.
Retain existing CUDA Graph/MMQ/NUMA defaults unless matched measurements justify
a change. Keep speculative decoding disabled until its broader gates pass.

## Constitution Check

| Principle | Status | Evidence in the plan |
| --- | --- | --- |
| I. Fail closed | PASS | T002–T005 reject false stream ends, overload, stale/unsupported observations and invalid measurements; T009–T010 reject unsupported optimizations. |
| II. Evidence retained | PASS | T001, T006–T013 produce `docs/evidence/power/2026-10-21-serving-optimization/OUTCOME.md` and bounded raw records. |
| III. Tests follow path | PASS | Owner-local hermetic tests accompany each behavior change; real service campaigns supplement them. No Cloud store/authority change or PostgreSQL proof is planned. |
| IV. Closed sets | PASS | Existing enum/error contracts; new policy values require parse/as_str and refusal tests. |
| V. Workspaces / APIs | PASS | Root Aster workspace and owning APIs preserved; `ChatResponseChunk` unchanged; no additional workspace/SDK. |
| VI. Local schema | N-A | No persistence/schema migration. |
| VII. Shrink-only ratchets | PASS | No allowlist expansion; applicable guards run in T011. |
| VIII. Replaceable transports | PASS | Provider data travels in existing metadata; backend-specific fields stay out of the shared envelope. |
| IX. ACL configuration | PASS | Existing Power ACL owner/parser; benchmark JSON/JSONL are data/results, not a second product configuration format. |
| X. Commands/controllers/test files | PASS | Existing HTTP surfaces/contracts only; new tests in sibling files. No new Cloud controller. |
| XI–XII. Judgment / adversarial review | PASS | Compatibility probes precede enablement; T011 challenges stream boundaries, cancellation races, cache correctness and provenance. |
| XIII. One repository | PASS | New Aster scripts/tests/docs land here; `/code/Power` is an existing explicitly requested external execution component. No new repository or clone. |
| XIV. Names | N-A | Exact stored model aliases/configuration identity only; no name extraction. |
| XV. Meaning | PASS | Workloads/policies use explicit typed choices; no regex intent decisions. |
| XVI. Pickers | N-A | No UI. |
| XVII. Dialogs | N-A | No UI. |

User direction explicitly identifies the existing external Power repository as an execution target. This is not an exception authorizing a new Aster repository. The shared plan procedure's source-table product examples do not invent an in-tree SDK or authorize a Cloud module. Read local/remote owning instructions before implementation.

## Project Structure

### Documentation

`specs/005-power-serving-optimization/{spec.md,plan.md,tasks.md,research.md}` contains planning; acceptance lives in `docs/evidence/power/2026-10-21-serving-optimization/OUTCOME.md`. Existing contracts are reused, so no duplicate data-model or contracts document is required.

### Aster source and integration table

| Layer | Path in this workspace | Planned change / existing pattern |
| --- | --- | --- |
| Qualification runner | `scripts/power_serving_qualification.py` (new) | Bounded client-only benchmark/provenance runner, following finite process/artifact patterns in `tests/e2e/startup.py` and `tests/cli_pair_delivery.py`; no Python SDK or bindings. |
| Runner proof | `tests/power_serving_qualification.py` (new) | Standard-library loopback fixtures, fake clock/result data and deadlines; reject invalid comparisons. |
| Gateway integration | `apps/gateway/src/inference/observed_tokens.rs`, `scheduling.rs` | Reuse trusted usage and scheduling; change only a proven integration defect. |
| Gateway proofs | `apps/gateway/src/inference/power_observation_contract_tests.rs`, `power_observation_projection_tests.rs`, `scheduling_tests.rs` | Existing schema/freshness/pressure and unsupported capability patterns. |
| Gateway full-path proofs | `apps/gateway/src/inference/distributed_serving/client_tests.rs`, `response_tests.rs` | Existing upstream/client response contract patterns; add narrowly scoped tests if integration changes. |
| Acceptance evidence | `docs/evidence/power/2026-10-21-serving-optimization/OUTCOME.md` | Commands, exact counts, provenance, matrices, decisions, rollout and Explicitly not claimed. |

### Existing external execution component: `/code/Power`

| Owner / exact path | Change and proof |
| --- | --- |
| `src/backend/prism/{mod.rs,stream.rs,timings.rs}` | Usage/timing provenance, incremental tool streaming, trailing usage, terminal validation and cancellation; new sibling `src/backend/prism/streaming_tests.rs`. |
| `src/backend/types.rs` | Preserve public `ChatResponseChunk`; no field addition. |
| `src/api/openai/chat.rs`, `src/server/metrics.rs` | Truthful TTFT/usage and outcomes; sibling test files adjacent to changed owner, registered through existing test modules. |
| `src/admission.rs`, `src/server/{limiter.rs,state.rs,worker_observation.rs}`, `src/config.rs`, `src/api/health.rs` | Finite admission, ACL validation and truthful existing observations; sibling admission/worker observation tests. |
| Existing llama-server binary/source and launch configuration | Slot/cache/speculation/profile experiments, pinned actual GGUF/ggml runtime. Any executor change stays in the already-present authorized source tree and requires owner instructions, focused tests and measured proof; do not create/clone a tree. |

Remote dirty work is preserved by snapshots `20ed6b9` (Power) and `02f45d` (Gateway) plus exact dirty diffs/hashes. Do not reset/stash/overwrite those changes. The runtime baseline is the preserved actually deployed artifact; its exact source/features cannot be reconstructed. The snapshots establish source preservation, not binary/source equivalence. The candidate has separate verified source/build identity.

## Research and decisions

See [research.md](research.md) for primary DeepInfer references, existing prior art, runtime compatibility and metric definitions. Directly transferable work is provider/admission/integration correctness. Token-loop overlap, DSpark confidence heads and YoCo bounded replay require executor/model capabilities; a sidecar queue cannot implement them.

## Verification Plan

1. **Hermetic before deployment**: Power focused provider/API/admission tests; root `cargo test -p aster-gateway power_observation`, `cargo test -p aster-gateway scheduling`, relevant response/client tests; `python tests/power_serving_qualification.py`. Record exact owner commands/features and counts; adapt test filters to actual new test modules.
2. **Required source checks**: `cargo fmt --all -- --check` from each existing owning workspace with its pinned toolchain; in Aster `python tests/workspace_dependencies.py`, `python tests/crate_drift_report.py` and applicable existing CI checks. Record baseline failures separately; do not fix unrelated files or weaken guards.
3. **Paired artifacts**: Build baseline and candidate before service replacement. Record git HEAD, dirty diff hash, binary SHA256, toolchain/features, GGUF SHA256, llama build, launch flags, GPU/topology/NUMA, Gateway/Power ACL hashes with secrets redacted, corpus hash and unique port mapping. Probe candidate loopback Power `19200+`, Gateway `19880+`, llama-server `19100+` for collisions. Preserve production `8080`, `18100–18107`, `18200–18207` until canary gates; do not replace production to obtain a baseline.
4. **Finite campaign**: Each baseline/candidate scenario has warmup, 60-second bounded steady phase, 120-second per-request deadline, 15-second shutdown/cleanup bound and an overall campaign deadline. Run three matched repeats at concurrency 1/8/16/32. If long-context work requires a different deadline, predeclare it for both arms before execution and record the reason. Run cancellation before first output, mid-generation and queued; overload bursts beyond active+waiting capacity; recover to a healthy idle worker after each scenario. HTTP defaults are active 1, queued 8, timeout 1000 ms, configurable in ACL; active 0 is refused and all four admission endpoints use uniform 429 refusals. Use a backend-owned lazy stream with synchronous HTTP drop, not a detached producer; measure GPU stop separately rather than assuming it follows the HTTP close.
5. **GPU matrix**: Compare actual slots 1/2/4 on one canary GPU first. Hold effective per-request context and prompt/output budgets equal; changing `-c` with slots is explicit memory/capacity cost. Separate cold, warm identical-prefix, divergent-prefix, near-limit-context and isolated-caller cases. Chosen configuration then runs on eight GPUs under the same load matrix. Retain OOM/refusal results as results.
6. **Speculation matrix**: Probe model/tokenizer compatibility, available weights, runtime options and telemetry. Compare off, supported ngram/draft candidates and fixed lengths; use adaptive policy only with real accepted/proposed telemetry and control. Include draft memory and total execution time. If incapable, retain exact command/output and a specific next dependency. No incompatible config is deployed.
7. **Executor profile**: Sample CPU scheduling, GPU kernels/gaps, transfers, NUMA and memory under measured workloads; compare instrumented/uninstrumented overhead. Implement a bounded change only for an observed bottleneck on a capable runtime and re-run correctness plus matched benchmark. Produce a concrete no-change decision if no justified change exists.
8. **Promotion and rollback**: Before comparing, record user-relevant latency/error/memory budgets from formal baseline and workload requirements; no retrospective threshold change to pass. Require correctness, stable finite capacity and measured benefit with repeat variability reported. Canary one replica, hold a full matched campaign, then roll out one GPU at a time to eight with readiness and pressure checks. Any corruption, credential/isolation defect, leak, repeated OOM, unexpected error or breached declared SLO triggers restore of prior binary/ACL/launch config and health/recovery proof. Rehearse restoration on canary.

No performance test, profile, real service test or rollout is implicitly waived. A missing prerequisite produces an explicit unresolved task and evidence, not a completion claim. This epic adds no Cloud schema/API/store change and therefore does not run a new live PostgreSQL campaign.

### Predeclared public transport condition and phase order

Before any selected-interface authenticated run, preserve the failed original
default-route public attempt. A finite no-auth experiment is pinned as
`network-per-socket-interface-ab1.json` SHA
`1bf7b6df35434c6dab397ea216526c3ff430f631ec26135ed43429b516645281`:
sixteen owned IPv4 sockets selecting the proved physical interface4 returned401,
while eight of sixteen default-route attempts timed out during connect. This
permits preparing a separate transport condition; it does not repair or relabel
the original path and does not establish absence of intermediary software.

The new `public-selected-interface.py` wrapper leaves the original public
helper, qualification runner, corpus and four frozen instruments unchanged.
Only this process's sockets to exact117.162.0.179:8080 use AF_INET with
IP_UNICAST_IF31, network-order interface input, host-order readback before/after
connect and actual source-address hash matched to the pinned physical-interface
receipt. Unsupported options, source mismatch or failure stop without fallback.
Other sockets and the SSH credential subprocess retain their original behavior;
no route, VPN, proxy, client source or system setting changes. The credential
continues through the existing protected SSH pipe into process memory only.

The shared eight-card campaign completes before this independent public run,
and execution needs a separate operator GO. The original public helper's model
discovery, four functional cases, expected outputs, usage/termination checks and
60-second pressure cells at concurrency1/8/16/32 remain unchanged, as do the
120-second request deadlines and failure/stop rules. Each cell receives a fresh
selected-interface namespace, without automatic retries or old-result overwrite.
Report the specified outgoing interface as a changed network factor; results
cannot establish a software-only causal speedup, default-route repair, general
VPN repair or Work/Darwin-client acceptance.

## Live-production measurement contract (declared before a new eight-card run)

The first production-GPU7 cancellation run was confounded by a real simultaneous
public request. Its failed result remains unchanged. Content-free Gateway
metadata proves request `0dcec368-f354-4fc7-9c9d-4b5cf9b8e603` was sent to
production Power18207 at 2026-10-21T03:28:12.547755Z. Exact native task/count
reconciliation cannot attribute that work to the benchmark. The selected worker
was actually rolled back, with its original ACL/artifact/profile restored.

Keep the original qualification runner, matrix, cancellation and eight-card
instruments frozen. Repeat the single-GPU cancellation/overload and full matrix
under a fresh isolated namespace, with production Power7 structurally stopped
and the other seven production workers serving. The original strict native
idle/exact-accounting gates continue to apply to this isolated run.

For the later eight-card run, taking all eight public workers offline throughout
a matrix would defeat the availability requirement. A separate
`eight-card-live-qualification.py` instrument instead measures verified benchmark
goodput on the actually deployed eight GPUs while production remains active:

- Preserve three repeats at concurrency 1/8/16/32 with 60-second dispatch,
  120-second absolute requests, finite cleanup, the same corpus/context/sampling,
  exact quality, bounded closed-set refusals and the 90% whole-card guard.
- Verify actual production Power/native systemd process identity, artifact and
  profile, private-wrapper provenance and the upgraded shared Gateway artifact.
  Every successful own request requires actual upstream-authoritative counts,
  exact wire reconciliation, valid finish/DONE and expected text/tool output.
- Each GPU's native counters must be complete, finite integral, monotonic and
  from the same process generation. Its native completion delta must be at least
  its own verified successful-output sum. Record any excess as
  `unattributed_native_completion_delta`; do not silently subtract it or label it
  external without separate attribution evidence. Never offset a deficit on one
  GPU using a surplus on another. Warmups receive separate identical accounting.
- Require all own HTTP requests to end and private Power active/waiting/running
  gauges to return to zero. Record shared native activity without requiring
  unrelated production requests to become idle. Isolated cancellation gates
  remain a separate requirement and cannot be replaced by these shared checks.
- Report successful own requests/wall time and upstream-authoritative own output
  tokens/wall time. Do not report native-exclusive throughput, isolated absolute
  capacity, an eight-card causal before/after ratio, or production-wide admission
  capacity: private and production Power form distinct admission pools.

The separate instrument requires meaningful refusal/accounting/identity/cleanup
fixtures and independent review before execution. Real authenticated public
functional, persistent conversation, cancellation/recovery and finite pressure
acceptance remain required after the rollout. This amendment changes attribution
under demonstrated concurrent traffic; it does not turn failed data into a pass.
