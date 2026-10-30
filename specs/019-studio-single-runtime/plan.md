# Implementation Plan: Studio single runtime and authority convergence

**Branch**: current branch | **Spec**: [`specs/019-studio-single-runtime/spec.md`] | **Input**: [`specs/019-studio-single-runtime/spec.md`]

**Gate context**: Product regime across `crates/code` and the private `apps/studio` submodule.

## Summary

Converge existing Core/ACP/Studio behavior instead of adding another abstraction stack. First remove small duplicate mechanisms through complete vertical slices, then migrate session/execution state and finally Goal/Schedule/Review. Each cutover is single-writer and deletes the retired authority path in the same structural change.

## Constitution Check

*GATE: Re-read `.specify/memory/constitution.md`. A NON-NEGOTIABLE violation stops the plan.*

| Principle | Status | Evidence in the plan |
| --- | --- | --- |
| I. Fail closed | PASS | No load/store fallback; unknown content/event/catalog values refuse |
| II. Evidence retained | PASS | `docs/evidence/code/2026-10-25-studio-single-runtime/OUTCOME.md` |
| III. Hermetic tests | PASS | Owning-package ACP/Core/Studio tests plus renderer tests |
| IV. Closed sets | PASS | Execution source/attendance/event/status use typed parse/as_str |
| V. One workspace / published APIs | PASS | Root Cargo workspace and existing public Rust APIs only |
| VI. Local schema changes | PASS | Core owns execution schema; Studio owns presentation-only schema |
| VII. Shrink-only ratchets | PASS | Raw request, duplicate driver and parallel-authority ceilings only shrink |
| VIII. Replaceable transports | PASS | Transport remains in ACP/RuntimeClient; no transport fields in shared UI facts |
| IX. One configuration language | PASS | ACL only; JSON remains wire/test evidence |
| XIII. One repository | PASS | No new repo; Studio source only in its existing private submodule |
| XIV. Names are not guessed | PASS | IDs come from stored/protocol identities, never inferred from text |
| XV. Meaning is not matched | PASS | Execution source and schedules are typed, not prompt/tool-name inference |
| XVI. Pickers are drawn | PASS | Existing design-system pickers only |
| XVII. Dialogs stay in place | PASS | No new dialog motion |

## Project Structure

### Documentation (this feature)

```text
specs/019-studio-single-runtime/
├── spec.md
├── plan.md
├── research.md
├── data-model.md
├── tasks.md
└── contracts/runtime-authority.md
```

### Source Code

| Layer | Path | New files |
| --- | --- | --- |
| ACP driver | `crates/code/acp/src/agent/` | Shared admitted-Run attempt driver and sibling tests |
| Core read models | `crates/code/core/src/agent_api/` | Only missing public snapshot/boundary APIs |
| Studio runtime | `apps/studio/src-tauri/src/agent_plane/` | `runtime_client.rs` and sibling tests |
| Studio presentation | `apps/studio/src/renderer/` | Typed Plan/event consumers; no second execution store |
| Architecture checks | `tests/` | Shrink-only runtime-authority boundary checks |

## Research

See `research.md`. Existing Core Session/Run/Goal/Plan/Review APIs are reused. A new pattern is introduced only where current APIs lack a required boundary, such as a typed Plan snapshot or a single Studio transport facade.

## Vertical Cutovers

1. **Authority ratchet and Plan**: publish the same Core Plan in live/load and remove new TodoWrite projection dependency.
2. **Admitted-Run driver**: share event drain, cancellation and worker settlement between interactive and background entry points.
3. **RuntimeClient**: migrate all existing Studio ACP commands without changing wire semantics; forbid new raw callers.
4. **Permission**: remove Studio pre-decision after Core owns the complete explanation and policy view.
5. **Session/transcript/fork**: add missing Core boundaries, migrate offline with one writer, then remove Studio's execution ledger.
6. **Execution profiles and Goal/Schedule**: parameterize attendance on the same driver and feed calendar/Goal wake into one scheduler.
7. **Review/document/catalog**: migrate each complete vertical path and delete the parallel implementation in the same change.
8. **Retirement**: remove migration adapters and old schemas, then run full behavior and architecture gates.

Preparatory APIs may land additively, but no production dual-write is permitted. No permanent `legacy` or `substituted` runtime profile is introduced.

## Verification Plan

- Hermetic tests: `cargo test -p a3s-code-acp` focused filters, focused `a3s-code-core` tests, `cargo test -p a3s-studio` focused native tests and existing Studio Vitest projects.
- Ratchet: raw Runtime request call sites, independent attempt loops, Plan rawInput consumers, permission pre-decisions and alternate execution ledgers only shrink.
- Contract: actual ACP stdio for Plan, prompt/cancel, load and permission conformance.
- Migration: one writer, no fallback, stable identities and explicit unmapped/conflicting history.
- Explicitly separate: renderer preview is not native GUI; scripted models are not real-model qualification; short tests are not soak.
