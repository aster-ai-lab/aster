# Implementation Plan: Office quality

**Branch**: current worktree | **Spec**: [spec.md](spec.md)
**Gate context**: Shared Office library, Rust implementation and Studio consumers.

## Summary

Inventory existing implementations and tests, fix shared controls first, then
complete editor-by-editor functionality and design work. Preserve all current
public entry points and document fidelity. Reuse the existing design system and
test tools. Acceptance lives only in `docs/evidence/office/2026-10-30-quality`.

## Constitution Check

| Principle | Status | Evidence in the plan |
| --- | --- | --- |
| I. Fail closed | PASS | Preserve boundary validation, named errors and last good content |
| II. Evidence retained | PASS | OUTCOME.md and retained command/browser evidence |
| III. Hermetic tests | PASS | Real controls/codecs/commands and owning Rust tests |
| IV. Closed sets | PASS | Keep current typed formats/commands and refuse unknown values |
| V. Workspaces/public APIs | PASS | Root workspace; source changes inside Studio submodule only |
| VI. Local schema changes | N-A | No store/schema change planned |
| VII. Shrink-only ratchets | PASS | Extend Office coverage; no exceptions added |
| VIII. Replaceable transports | PASS | Preserve existing editor/kernel/host boundaries |
| IX. One configuration language | PASS | No new product configuration |
| X. Commands/tests | PASS | Preserve host permissions; sibling Rust tests |
| XI. Independent judgment | PASS | Findings require reproduction or explicit source proof |
| XII. Adversarial review | PASS | Review focus, data loss, overlay, IME and failure cases |
| XIII. One repository | PASS | Existing tree/submodule; no new repository |
| XIV. Names not guessed | PASS | Existing identifiers and typed option values only |
| XV. Meaning not matched | PASS | Mechanical control parsing only |
| XVI. Pickers drawn | PASS | Remove remaining native controls; shared picker contracts |
| XVII. Dialogs stay in place | PASS | Shared opacity/scale recipes and reduced-motion proof |

## Source and prior art

| Area | Existing source / pattern |
| --- | --- |
| Design roles | DESIGN.md; packages/ui/src/styles/tokens/spec-roles.css |
| Controls | packages/ui SearchSelect/Combobox/dialog; Office primitives and focus scope |
| Office editors | packages/office/src/internal/features/work/editors |
| Native operations | crates/office/crates/{core,web-kernel,formula-parser,cli} |
| Host | apps/studio/src/renderer/pages/documents and knowledge/message consumers |
| Tests | Office Bun/Rstest setup, Studio Vitest/Playwright and existing Aster Test ACL |

## Verification plan

1. Record baseline package typecheck, exact-file tests and Rust owner suites.
2. Add small failing regressions before each behavior fix; run their actual
   implementation with minimal host mocks.
3. Verify standalone package build and all public exports, then real browser
   workflows/styles at desktop and narrow widths with actual editor content.
4. Verify Studio integration, root dependency contracts, applicable Cargo
   checks and `cargo fmt --all -- --check`; triage pre-existing failures.
5. Reopen outputs and audit each FR/task against direct evidence. A narrow pass
   cannot satisfy the complete goal.

No live account/model or publication is required for hermetic Office acceptance.
