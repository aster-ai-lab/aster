# Feature Specification: Power serving optimization

**Feature Branch**: Existing branch; no branch or commit requested.

**Created**: 2026-10-21

**Status**: Ready for implementation after read-only analysis; no implementation claim.

**Input**: Implement the complete P0–P5 serving roadmap, run end-to-end and pressure tests on the existing eight-GPU service, and retain measured evidence.

**Gate context**: Aster Gateway integration and qualification tooling in this repository; the user explicitly directs changes to the existing `/code/Power` execution component on the GPU server and qualification of the existing `/code/aster` Gateway deployment. Neither location is a new repository. This epic does not create an in-tree Power SDK or modify Cloud schema/API contracts.

## User Scenarios & Testing

### User Story 1 - Trust the serving measurements (Priority: P1; P0 roadmap)

An operator can distinguish queue delay, first output latency, generation time, successful throughput, cached input, and cancelled work for the actual external llama-server runtime.

**Why this priority**: Configuration decisions based on fabricated token counts or completion latency presented as TTFT cannot establish an optimization.

**Independent Test**: Power provider/API sibling tests use a loopback scripted SSE upstream with separate token, usage, finish and terminal events; assertions compare reported counts and timestamps to the scripted events and retain unknown values when upstream does not supply them.

**Acceptance Scenarios**:

1. Given a role-only delta followed by text/tool output and trailing usage, when a request streams, then TTFT starts at the first meaningful output, usage retains upstream counts, and no SSE frame/byte count is labelled tokens.
2. Given zero/one output token, missing timings or a cancelled response, when metrics are emitted, then undefined rates are absent/unknown and the result is not counted as a completed successful generation.

### User Story 2 - Stream tools and reclaim cancelled capacity (Priority: P1; P1 roadmap)

A caller receives actual incremental text and tool calls through Power and Gateway and can cancel queued, prefilling or decoding requests without leaving work or permits behind.

**Why this priority**: Real streaming and cancellation determine interactive latency and whether finite capacity stays available.

**Independent Test**: Scripted upstream tests fragment UTF-8/SSE/JSON boundaries, interleave tool indices and split arguments, omit the terminal event, and observe upstream disconnect plus permit release when the downstream receiver closes.

**Acceptance Scenarios**:

1. Given split tool name/argument deltas, when forwarded, then the caller receives deltas before completion, reconstructs the exact call IDs/names/arguments, and receives the correct finish reason once.
2. Given cancellation before first token or after output, when the client disconnects, then the backend-owned lazy SSE stream synchronously drops its HTTP response without a detached producer, and capacity returns within the tested cleanup deadline; a truncated upstream EOF is an error, not a successful finish. Actual GPU work termination is measured end to end and is not inferred from HTTP drop alone.

### User Story 3 - Keep overload finite and route on truthful signals (Priority: P1; P2 roadmap)

The operator can run bounded active and queued requests and use existing Gateway worker observations without advertising unsupported cache or state transfer.

**Independent Test**: Power admission sibling tests use a paused clock and held permits to prove the active and waiting bounds, timeout, queue cancellation and permit recovery. Existing Gateway observation/scheduling tests prove stale/unsupported/full workers are treated according to current contracts.

**Acceptance Scenarios**:

1. Given all active and waiting slots occupied, when another request arrives, then it is promptly refused with the existing overload error contract; waiting never grows without bound.
2. Given an expired observation or unsupported prompt cache, when routing, then no stale readiness or invented cache affinity is used. Gateway continues to consume the existing closed observation schema.

### User Story 4 - Choose GPU slots and cache behavior from real experiments (Priority: P2; P3 roadmap)

The operator receives matched baseline/candidate measurements for actual llama-server slots 1, 2 and 4, including cold/warm prompts, cache correctness and saturation.

**Independent Test**: Aster qualification-runner tests use finite loopback fixtures to prove deadlines, concurrency accounting, percentile aggregation and result provenance. GPU experiments independently prove runtime behavior.

**Acceptance Scenarios**:

1. Given the same model hash, prompt corpus, sampling parameters and effective per-request context, when slots 1/2/4 are compared, then the record contains both latency and total throughput with memory/capacity costs and correctness results.
2. Given identical prefixes, divergent suffixes, changed model/template/tokenizer and isolated callers, when cache reuse is tested, then no incorrect answer or cross-caller reuse is accepted; an unsupported cache capability remains unsupported.

### User Story 5 - Evaluate compatible speculation and executor work (Priority: P2; P4–P5 roadmap)

The operator gets an evidence-backed decision for each speculative candidate and executor bottleneck, with implemented improvements when a capable runtime and measurements justify them.

**Independent Test**: Qualification-runner tests reject mismatched model/tokenizer/cache/slot provenance and incomplete measurements. Any adaptive policy or executor change includes owning-package hermetic refusal and correctness tests.

**Acceptance Scenarios**:

1. Given compatible draft weights or runtime-supported ngram speculation, when tested against speculation off, then the record includes accepted/proposed tokens, all draft/verification overhead, output correctness and matched concurrency. Adaptive draft depth is compared to the best fixed depth where telemetry/control exists.
2. Given no compatible weights or no runtime control/telemetry, when the candidate is evaluated, then the exact probe and reason are retained, with a concrete follow-up; no unsupported mode is silently enabled.
3. Given a measured executor bottleneck, when a capable runtime supports a scoped correction, then implement and remeasure it. Otherwise retain the profile and decision; a generic statement that Power is a sidecar does not complete this investigation.

### Edge Cases

- Tool-only, empty, malformed, multi-choice, fragmented, missing usage, finish-before-usage, terminal-without-finish and truncated streams.
- Downstream close before headers/first output, queue timeout racing admission, active cancellation, upstream stall/failure, expired worker observations and full workers.
- Slot increases that reduce per-slot context; VRAM exhaustion; warmed cache changing the workload; divergent tokenization/templates; insufficient speculative telemetry; profiler overhead.
- Dirty deployed files must be preserved. No production credential, session state or private prompt is copied into evidence.

## Requirements

### Functional Requirements

- **FR-001 (P0)**: Preserve upstream token usage/timings and label queue, TTFT, total latency and generation rates with explicit definitions. Refuse fabricated token/rate values; preserve `ChatResponseChunk`'s public fields and use existing metadata JSON for additive internal usage details.
- **FR-002 (P1)**: Forward incremental text/tool SSE, preserve trailing usage, detect malformed/truncated termination and propagate cancellation through provider, API and Gateway. Refuse a false successful end and release resources on every exit.
- **FR-003 (P2)**: Enforce finite active/waiting limits and queue deadlines using ACL configuration and cancellation-safe ownership: HTTP defaults are active 1, queued 8, queue timeout 1000 ms; refuse HTTP active 0. All four existing admission endpoints return the same 429 refusal contract for overload/expired admission. Expose truthful existing worker admission/cache/transfer observations without new Cloud contracts.
- **FR-004 (P3)**: Execute GPU slot 1/2/4 and cold/warm/divergent-cache matrices at concurrency 1/8/16/32 with finite run durations/deadlines. Refuse comparisons with changed model, prompt, effective context, sampling or unrecorded warmup.
- **FR-005 (P4)**: Probe compatible speculative candidates and fixed/adaptive draft lengths; implement adaptive control if supported and beneficial. Refuse incompatible weights/tokenizers, unsupported cache/slot combinations and claims based solely on acceptance rate.
- **FR-006 (P5)**: Profile executor CPU/GPU gaps, GGML CUDA operations, transfers, NUMA and saturation; implement any justified scoped optimization and repeat matched measurements. Refuse direct import of unsupported DeepSeek/SM100/YoCo kernels or suffix truncation into Bonsai.
- **FR-007**: Build preserved baseline and changed candidates on distinct nonproduction ports, pin source/diff/binary/model/runtime/config/corpus provenance and retain raw bounded records. Refuse a result missing either baseline or candidate; previous 103–104 token/s, 111-token samples are observations, not the formal baseline.
- **FR-008**: Exercise full client → Gateway → Power → llama-server text/tool/error/cancellation paths and pressure concurrency 1/8/16/32, including overload and recovery. Refuse completion based only on green hermetic fixtures.
- **FR-009**: Use a single-replica canary, then guarded eight-card rollout with health, correctness, latency, memory, errors and cancellation checks; restore the preserved release/config on a failed gate. Refuse rollout of unmeasured configurations or a failed canary.
- **FR-010**: Preserve existing dirty changes (Power snapshot `20ed6b9`, Gateway snapshot `02f45d`), credentials and published interfaces. Refuse destructive reset, a new repository/workspace, unrelated edits and unrequested commits.

### Key Entities

- **Serving measurement**: Aster qualification artifact with monotonic event timestamps, upstream usage, outcome and pinned provenance; not product configuration.
- **Admission lease**: Power-owned active/waiting capacity whose lifetime includes streaming and cancellation cleanup.
- **Worker observation**: Existing Power/Gateway schema with epoch, generation, expiry, capabilities and admission/cache pressure; no new schema.
- **Experiment decision**: Baseline/candidate matrix, uncertainty, correctness gate, chosen configuration and rejected alternatives for P3–P5.

## House Checklist

- [ ] New behavior has a small hermetic test in its owner (T002–T005, T009–T010).
- [ ] Closed sets retain `parse()` / `as_str()` and reject unknown values (T004, T009–T010).
- [ ] No production `unwrap()` / `expect()` without an infallibility comment (T011).
- [ ] Internal `aster-*` dependencies stay local; no new SDK/workspace/repository (T011).
- [ ] Product configuration remains ACL parsed by `aster-acl` (T004, T009–T010).
- [ ] Evidence target: `docs/evidence/power/2026-10-21-serving-optimization/OUTCOME.md` (T001, T006–T013).

These boxes remain unchecked until implementation verification; none is waived.

## Success Criteria

- **SC-001**: Every P0–P5 task has an executed test/experiment and recorded result or a measured, specific conditional decision; no task is closed by a blanket deferral.
- **SC-002**: Focused Power and Gateway hermetic suites and runner tests pass with exact counts recorded; formats and applicable workspace guards pass.
- **SC-003**: Both preserved baseline and candidate complete the same finite concurrency 1/8/16/32 campaign; raw observations report p50/p95/p99 TTFT/total latency, output token throughput, queue/refusal/cancel/error counts and VRAM. No throughput claim is made without actual token counts.
- **SC-004**: Text/tool stream integrity, cancellation cleanup, finite overload and recovery pass end to end; no cross-request cache corruption or permit leak is tolerated.
- **SC-005**: Performance promotion uses repeated matched measurements with predeclared workload/SLO budgets and uncertainty; earlier tentative 20%/25% goals are not guarantees. If a candidate fails, the decision and retained safe configuration are recorded honestly.
- **SC-006**: Canary and guarded rollout/rollback rehearsal are recorded; `OUTCOME.md` names what landed and Explicitly not claimed.

## Assumptions

- Current alias `prism-27b` selects `Ternary-Bonsai-2-27B-PTQ1_0.gguf`; llama-server build `b10787-73bfe6cc4` currently uses `-ngl 99 -c 32768 -fa on -t 8 --parallel 1 --cache-prompt`. Hashes and effective flags must be rechecked.
- Eight RTX 4090 D cards have 24 GiB each; use actual topology/memory/NUMA probes rather than treating 192 GiB as one pool. Power native model runtime/speculation is disabled; actual execution is external llama-server.
- Existing `/code/Power` and `/code/aster` are explicit user-directed deployment targets. This permission does not authorize creating or cloning a repository.
- Candidate loopback ports are Power `19200+`, Gateway `19880+`, and llama-server `19100+` after collision probes. Production `8080`, `18100–18107` and `18200–18207` remain untouched until canary gates pass.
- Cloud schema/API/auth/store behavior is unchanged; touching it would require a revised plan and the owning path's control-plane proofs.

## Explicitly Not Claimed

- DeepInfer compatibility, its benchmark speedups, SM100 kernel support, Bonsai YoCo/MTP heads, distributed KV transfer, or an unmeasured GPU speedup.
- New Cloud schema/API, a replacement public streaming envelope, an in-tree Power SDK, credential/session migration, or a new repository/workspace.
- Planning artifacts alone do not complete implementation, benchmarking, P4/P5 investigation or rollout.
