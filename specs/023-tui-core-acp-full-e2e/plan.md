# Implementation Plan: Current Core and ACP in Aster TUI

**Branch**: Current shared worktree | **Spec**: [spec.md](spec.md)
**Gate context**: Product regime; terminal composition, pager client, ACP adapter and Core. The credential regression exposed shared ACL codec data loss, so the scope also includes `crates/acl` and the matching API/Cloud copies. The Cloud instructions were read before that edit. Their owning ACL tests and the unchanged shared-crate drift ratchet apply; no control-plane application or migration is changed.

## Summary

Keep the existing paired-binary and ACP subprocess architecture. Inspect current source contracts, map their entire surface, reproduce integration gaps, and change the owning shared path with its tests. Validate actual PTY and ACP workflows, then a separate live-model campaign. Review for overfitting before the final full-scope audit.

On 2026-10-28 the user confirmed default and already configured capabilities must execute; disabled capabilities must explicitly refuse. The 18 baseline and 12 advanced groups remain in the inventory. No new S3, OTLP, evaluation or state-graph host is installed solely for this campaign.

## Constitution Check

| Principle | Status | Evidence in the plan |
| --- | --- | --- |
| I. Fail closed | PASS | Missing/incompatible pair, protocol, authority and resource failures receive refusal cases. |
| II. Evidence retained | PASS | `docs/evidence/code/2026-10-27-tui-core-acp/OUTCOME.md` and per-case evidence. |
| III. Hermetic tests | PASS | Owning Rust tests plus actual-process scripted-model PTY/stdio cases. |
| IV. Closed sets | PASS | Explicit Core inventory, ACP methods and command metadata; unknown values refuse. |
| V. Workspace / APIs | PASS | Existing root workspace and public Core interfaces; no SDK binding or package renames. |
| VI. Local schema ownership | PASS | Any persistence change stays in its owning package and has restart tests. |
| VII. Ratchets | PASS | Existing runtime-boundary and dependency ceilings are unchanged. |
| VIII. Transports | PASS | Existing ACP channels and typed host contracts; no provider-specific TUI transport. |
| IX. Configuration | PASS | ACL model and host/workspace configuration; UI preferences do not acquire agent authority. |
| XIII. Repository | PASS | Work stays in this repository; private Studio implementation is not copied into the parent. The final workspace format check required formatting 11 existing Studio Rust files inside that submodule. |
| XIV–XV. Names and meaning | PASS | Capability identifiers come from source; models retain decisions; no oracle or intent matching. |
| XVI–XVII. Product UI | PASS | Existing terminal widgets; no native selects or new desktop animation. |

## Project Structure

| Layer | Existing source / pattern | Planned work |
| --- | --- | --- |
| Composition and delivery | `apps/aster/src/main.rs`, `scripts/cli_pair_build.rs`, `tests/cli_pair_delivery.py` | Current source pair and identity checks; fix only proven defects. |
| Pager ACP bridge | `crates/terminal/crates/codegen/a3s-code-pager/src/acp/a3s_spawn.rs` | Current metadata, methods, lifecycle and state projections. |
| Terminal features | Pager `src/slash`, `src/app/effects`, `src/app/dispatch`, `src/views` | Connect declared features to current contracts with reducer and PTY cases. |
| ACP | `crates/code/acp/src/agent/{commands,host_methods,session_open,turn}.rs` | Fix protocol gaps once in the adapter; actual stdio regression. |
| Core | `crates/code/core/src/sdk_capabilities.rs` and public facades | Inventory and upstream fixes only where evidence proves the defect belongs here. |
| Shared ACL codec | `crates/acl` and identical copies in the API/Cloud submodules | Preserve quoted keys through parse/generate; same regression and owning tests in each copy, no increased drift baseline. |
| E2E | `apps/aster/tests/tui_one_agent.rs`, `tests/e2e/*.acl`, ACP stdio fixtures | Extend real workflow coverage, bounded evidence and process cleanup. |

## Research

- Existing one-agent PTY test proves startup, ACL model selection and no hidden in-process fallback; it does not submit full coding workflows.
- Core declares 30 capability groups, including advanced/host-owned surfaces. All must be accounted for in the matrix.
- Pager memory operations use legacy `a3s/memory/*` DTOs; current ACP exposes `a3s/memory/manage` and `a3s/memory/snapshot`. Investigate reachability and semantics before adapting; renaming methods alone is not a fix.
- Reuse A3S Test TUI ACL lifecycle and existing Rust PTY helpers; browser-adapter capability failure is not assumed to block TUI execution without testing it.
- Preserve the extensive staged and unstaged source work. Build input snapshots, not a clean-HEAD assumption, identify results.

## Verification Plan

1. Build source pair, run boundary/dependency/delivery checks and existing paired PTY/ACP campaigns to establish a baseline.
2. Derive the matrix from current Core, ACP and pager source, marking every missing activation, incompatible schema and uncovered behavior.
3. For each gap: reproduce → owning hermetic refusal/success cases → smallest shared fix → actual terminal/ACP workflow.
4. Cover both render modes, streaming, tools and approvals, context, models, sessions/recovery, goals/tasks, memory and extensions. Exercise configured optional activations and the explicit refusal of disabled ones.
5. Run authorized live-model coding, cancellation and restart cases without persisting credentials in test artifacts.
6. Recheck source/binary identities, format and workspace guards; independently review negative cases, test oracles, cleanup and all matrix requirements.

No delegated agents, new dependency or new transport framework is planned. Any additional host/runtime requirement discovered by the matrix stays open until satisfied; this plan does not redefine a refusal as feature completion.
