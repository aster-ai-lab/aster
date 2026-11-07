# Research: Power serving optimization

**Date**: 2026-10-21. Findings distinguish observed source behavior from proposed adaptations. This document contains no performance result or implementation claim.

## Existing execution boundary and preserved state

- Actual configured alias is `prism-27b`; selected file is `Ternary-Bonsai-2-27B-PTQ1_0.gguf`. Execution is external llama-server `b10787-73bfe6cc4` with `-ngl 99 -c 32768 -fa on -t 8 --parallel 1 --cache-prompt`, not the disabled Power native model/speculation runtime. Reprobe hashes/effective settings in T001.
- The server has eight 24-GiB RTX 4090 D devices and dual NUMA. Measure real topology, free VRAM and process placement. A multi-GPU aggregate memory total is not a single allocatable device.
- Existing user-directed targets are `/code/Power` and deployed Gateway `/code/aster`. Preserve Power's three dirty files at snapshot `20ed6b9` and Gateway's five at snapshot `02f45d`; retain source/diff hashes rather than resetting them. No repository is created or cloned.
- HTTP admission decision: default active 1, queued 8 and queue timeout 1000 ms in ACL; reject HTTP active 0 and use uniform 429 refusals at all four existing admission endpoints. A backend-owned lazy SSE stream must drop its HTTP response synchronously on cancellation; no detached producer. GPU work termination remains a separate real-service measurement.
- Candidate loopback port ranges are Power `19200+`, Gateway `19880+`, llama-server `19100+`, subject to collision probes. Production `8080`, `18100–18107`, `18200–18207` remain untouched until canary gates.
- Observed 103–104 decode token/s from 111-token generations is preliminary only: short runs, cache state and overhead make it unsuitable for formal throughput/SLO conclusions. Tentative 20%/25% aspirations are not acceptance guarantees.

## In-repo prior art and ownership

| Decision | Existing source / reason |
| --- | --- |
| Reuse trusted token usage | `apps/gateway/src/inference/observed_tokens.rs` and `token_reconcile.rs` own observed upstream usage; do not invent frame/byte token counts. |
| Reuse worker signals | `apps/gateway/src/inference/scheduling.rs`, `power_observation_contract_tests.rs`, `power_observation_projection_tests.rs` already model epoch/generation/expiry, admission pressure, cache capability and transfer health. Existing projection is not itself a serving-capacity proof. |
| Keep upstream/client contracts | `apps/gateway/src/inference/distributed_serving/{client.rs,response.rs}` and their sibling tests; preserve existing envelope and errors. |
| Finite local qualification tooling | `tests/cli_pair_delivery.py` provides repository-owned process/artifact qualification patterns. New Power workload/provenance runner is needed because these do not benchmark serving or token timing. |
| Preserve dependency/governance checks | `tests/workspace_dependencies.py`, `tests/crate_drift_report.py`; keep internal dependencies local and do not grow drift/allowlists. |

Power owning paths are `src/backend/prism/{mod.rs,stream.rs,timings.rs}` (upstream streaming/timings), `src/api/openai/chat.rs` and `src/server/metrics.rs` (wire/metrics), `src/admission.rs` and `src/server/limiter.rs` (capacity), `src/config.rs`/`src/server/state.rs` (configuration composition), and `src/server/worker_observation.rs`/`src/api/health.rs` (observations). The existing public `src/backend/types.rs::ChatResponseChunk` stays unchanged. Existing metadata JSON can carry additive usage without breaking struct-literal callers.

## DeepInfer facts and transfer decisions

The inspected primary tree identified commit `774ce244b87cc0eb8d4b94ab9f5de2ab22049f01`; pin any source-derived comparison to the actual inspected revision. Its [README](https://github.com/deepseek-ai/DeepInfer/blob/774ce244b87cc0eb8d4b94ab9f5de2ab22049f01/README.md) lists only DeepSeek-V4.1-Flash and NVIDIA SM100 / Ascend 950, with Linux, GCC ≥13, Python 3.12 and CUDA 13 for NVIDIA. Benchmarks use four SM100 for prefill and 64 for decode within scale-up domains. No result transfers numerically to 4090 D/Bonsai/ggml. The release omits a complete KV-cache system, disaggregation deployment, DP load balancing and online serving wrapper.

1. **Scheduling**: [worker.py](https://github.com/deepseek-ai/DeepInfer/blob/774ce244b87cc0eb8d4b94ab9f5de2ab22049f01/deepinfer/worker/worker.py) launches current forward, postprocesses the previous step, then samples; decode postprocessing can be deferred. GPU remapping reuses previous sampled/draft tokens and accepted counts. This requires executor token-loop access and event/lifetime correctness. Proposed Power changes directly address request admission/cancellation; a Rust HTTP queue is not this GPU overlap. T010 must profile the actual executor before proposing a port.
2. **Confidence verification**: [mtp_varlen.hpp](https://github.com/deepseek-ai/DeepInfer/blob/774ce244b87cc0eb8d4b94ab9f5de2ab22049f01/csrc/engine/mtp_varlen.hpp) optimizes expected accepted tokens divided by profiled forward plus sampling time, correcting estimates with runtime samples. [DFlash](https://github.com/deepseek-ai/DeepInfer/blob/774ce244b87cc0eb8d4b94ab9f5de2ab22049f01/deepinfer/model_executor/models/mtp/dflash.py) requires dedicated heads and confidence parameters. [sampler.py](https://github.com/deepseek-ai/DeepInfer/blob/774ce244b87cc0eb8d4b94ab9f5de2ab22049f01/deepinfer/model_executor/layers/sampler.py) still applies target verification/rejection and residual sampling. Adapt the cost objective only with real compatible proposals and runtime controls; confidence is not permission to skip target validation. T009 probes exact candidate capability instead of assuming Bonsai has MTP heads.
3. **KV ownership**: [block_manager.hpp](https://github.com/deepseek-ai/DeepInfer/blob/774ce244b87cc0eb8d4b94ab9f5de2ab22049f01/csrc/engine/block_manager.hpp) implements reference-counted/pinned blocks; [scheduler.hpp](https://github.com/deepseek-ai/DeepInfer/blob/774ce244b87cc0eb8d4b94ab9f5de2ab22049f01/csrc/engine/scheduler.hpp) shares forked full prefix blocks and copies partial tails/state. This is not a turnkey distributed prefix cache. Existing external llama-server prompt reuse must be measured separately from Power-native cache support; never advertise a capability simply because another layer has a cache flag.
4. **Bounded replay**: [model_metadata_utils.py](https://github.com/deepseek-ai/DeepInfer/blob/774ce244b87cc0eb8d4b94ab9f5de2ab22049f01/deepinfer/model_executor/model_metadata_utils.py) requires YoCo consumer layers and MoE V4.1; [model_runner.py](https://github.com/deepseek-ai/DeepInfer/blob/774ce244b87cc0eb8d4b94ab9f5de2ab22049f01/deepinfer/worker/model_runner.py) checks retained suffix ≥ SWA window + maximum backtracking. Do not transplant `keep_tokens=140` or drop Bonsai context without model/runtime architectural proof. T008 instead validates supported prompt reuse and full context correctness.

## Measurement definitions

- Use monotonic event times: request start, admission, upstream start, first meaningful text/tool delta, last meaningful delta, verified terminal/close. Role-only/comment/heartbeat frames are not first meaningful output. Queue delay = admission minus request start; end-to-end TTFT = first meaningful output minus request start; total latency includes queue through terminal.
- Usage tokens are upstream-reported actual prompt/completion counts. Backend timing fields remain distinguishable from client timings. Unknown stays unknown; no counting SSE frames or UTF-8 bytes as tokens. Client delta timestamps do not establish per-token timestamps: report their interval as delta latency, the engine's actual decode rate separately, and actual completed output tokens divided by complete request/campaign wall time. Do not derive a purported token decode rate from coalesced SSE deltas or buffered JSON.
- Successful aggregate throughput counts actual output tokens from completed requests over the declared measurement interval; state the window/drain accounting. Retain partial/cancelled work separately. Report first tool-output latency separately where useful, and never label tool JSON bytes as token counts.
- Raw results record success/refusal/error/timeout/cancel outcomes, counts, queue/latency distributions and VRAM. A percentile without adequate sample count is labelled descriptive; include repeat/sample counts and uncertainty, not a synthetic assurance from a small sample.

## Experiments and decisions that must be executed

| Phase | Comparison / required decision |
| --- | --- |
| P0/P1 | Old/new metrics against scripted upstream truth, then actual tool/text streaming and cancellation through both preserved and candidate end-to-end paths. |
| P2 | Bounded active/waiting admission, timeout/cancel races, overload burst and recovery; stale observation rejection and truthful unsupported cache/transfer signals. |
| P3 | Slots 1/2/4 with equal effective request context, cold/warm/divergent/isolated/cache near-limit cases, then selected config across eight cards; retain VRAM and context tradeoff. |
| P4 | Probe available compatible draft weights and runtime ngram/draft support, match tokenizer/model/cache/slots, compare off and fixed depths; compare adaptive control when actual telemetry/control exists. Record executed incompatibility/no-capability evidence and exact follow-up otherwise. |
| P5 | Bounded actual CPU/GPU/NUMA/kernel profile and profiler overhead; implement only a proven scoped bottleneck supported by the existing runtime and repeat correctness/benchmark, or record concrete profile-based no-change decision. |

All performance campaigns use concurrency 1/8/16/32, finite warmup/60-second measurement/120-second request deadline/15-second cleanup, three matched repeats and a campaign deadline. Any altered long-context deadline is declared identically for both arms before execution. Record predeclared SLO budgets, actual token counts, model/binary/source/config/corpus hashes, sampling, warmup and cache conditions. Do not trade lower per-request context for apparent slot throughput without showing that cost.

## Rejected alternatives

- Replace llama.cpp with DeepInfer: unsupported hardware/model/runtime and missing serving components make this an unjustified migration.
- Claim SSE wrapping or final buffered tool output is true streaming: it does not prove incremental delivery/cancellation.
- Add fields to public `ChatResponseChunk`: avoid breaking callers; use existing metadata.
- Report green fixtures as end-to-end/GPU evidence: fixtures prove contracts, while paired real service campaigns prove deployed behavior.
- Close P4/P5 as generally out of scope: scope includes capability probes, measurements and conditional implementation/decisions.
- Create a sibling Power/llama repository or new Aster SDK: prohibited; work only in the existing explicit targets and repository-owned integration tooling.

## Remaining uncertainty

Exact runtime speculative options, available compatible local weights, cache behavior across slots, current dirty-tree build status, profiler availability, and throughput/SLO tradeoffs require T001 and T007–T010. They do not prevent correct provider/admission implementation. They must be resolved with recorded experiments before final completion; no guessed support or performance claim is permitted.
