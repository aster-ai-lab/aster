# Implementation Plan: Evaluation-driven delivery hardening

**Branch**: Existing | **Spec**: [spec.md](spec.md) | **Input**: Agreed seven-package optimization plan.
**Gate context**: Product paths; root Cargo workspace. Office is the shared library used by Studio and ACP.

## Summary

Fix shared spreadsheet display, attachment transport identity and current artifact availability first. Reuse native Office and existing ACP tools/verification metadata for bounded reads and necessary calculations. Diagnose approval refusals with a minimal governed-call fixture; retain security behavior. Build one identified candidate and validate through background CUA and real selected-model attempts, preserving the earlier baseline and unrelated dirty edits.

## Constitution Check

| Principle | Status | Evidence in the plan |
| --- | --- | --- |
| I. Fail closed | PASS | Scoped reads/publication; unknown arguments, denied access and unavailable backend remain refused. |
| II. Evidence retained | PASS | Baseline untouched; OUTCOME.md and per-command/per-attempt evidence. |
| III. Hermetic tests | PASS | Every behavior task includes its test; GUI/live checks supplement them. |
| IV. Closed sets | PASS | Typed calculation/source operations with explicit input parsing. |
| V. Workspace / APIs | PASS | Existing root workspace, NativeOfficeEditor, Tool/ToolOutput; public APIs preserved. |
| VI. Local schema | PASS | Metadata owned by existing Studio/ACP boundaries; no database migration. |
| VII. Shrink-only | PASS | No allowlist relaxation. |
| VIII. Transports | PASS | No transport-specific public envelope additions. |
| IX. Configuration | PASS | No new product configuration; existing ACL authority retained. |
| XIII. Repository | PASS | Source stays here; Studio private submodule respected; no new checkout. |
| XIV. Names | PASS | Exact supplied file metadata; no extension/name guessing. |
| XV. Meaning | PASS | Explicit operation sets; model judges meaning; no prose-to-deliverable inference. |
| XVI. Pickers | PASS | No new native select. |
| XVII. Dialogs | PASS | No dialog-motion change. |

## Project Structure

| Layer | Existing source / test target | Change |
| --- | --- | --- |
| Shared Office UI | packages/office/src/internal/features/work/work-spreadsheet-file-import.ts and owning regression test | Preserve typed values and generate valid numeric display. Trace alternate fast importer too. |
| Studio attachments | apps/studio/src/renderer/pages/agents/AgentComposer.tsx; shared file identity helper/tests; src-tauri/src/agent_plane/turns.rs | Full supplied filename throughout staging; safe long-name behavior. |
| Studio products | taskConversation/turn.ts, TaskConversationMessage.tsx, TaskProductDownload.tsx, owning tests | Explicit candidates plus capability-scoped current availability and open validation. |
| Native source adapter | crates/code/acp/src/document_read.rs, plugin_tools.rs, new source read module/test if required | Reuse aster_office NativeOfficeEditor; bounded DOCX/XLSX/PDF access. |
| Execution facts | crates/code/core/src/sandbox/oci.rs, agent/turn_context.rs, existing execution-target tests; Studio acp.rs | Actual backend/lifetime availability facts and qualified companion validation. |
| Tool contract | core tools/builtin/patch.rs, fact_control/receipts.rs and sibling tests; ACP registration | Reproduce schema/approval issues; fix only proven mismatches. |
| Output verification | crates/code/acp/src/output_check.rs, output_check_metrics.rs, tests and session policy | Date/sequence calculation and source-grounded final review guidance. |
| Acceptance | docs/evidence/code/2026-10-23-evaluation-hardening/ | Baseline hashes, checks, candidate identity, CUA artifacts, independent verdicts and OUTCOME.md. |

## Research

- Existing Office importer stores source.v but may omit display m for General numbers. Verify both SheetJS and package fast paths before choosing the shared fix; no numeric-to-text workaround.
- Existing OfficeFileTool already reopens and hashes products; use its artifact metadata, not another registry or guessed shell paths.
- Existing file.batch_get_metadata and file handles provide availability checks; preserve workspace capability boundaries and denied/missing equivalence.
- Existing NativeOfficeEditor is already used by office_file; project bounded source reads in ACP, not a Python/Node wrapper.
- Existing output_check already handles text length and percentage change; extend it narrowly.
- Existing patch schema advertises patches mode. Compare actual candidate projection/execution before changing behavior.
- Existing OCI execution-target metadata distinguishes guest from host. Add only the missing lifecycle/capability facts; reuse existing companion discovery and tests.

## Verification Plan

1. Record dirty baseline and hashes; preserve concurrent work without staging or reverting it.
2. Run each new focused hermetic test, then its connected caller tests. Regression assertions must include refusals, typed-value preservation and actual output contents.
3. Run applicable repository CI checks, cargo fmt --all -- --check, workspace dependency and native-control ratchets. Isolate distinct failures before attribution.
4. Build terminal plus ACP together and Studio plus Office from the root with the pinned toolchain; freeze source/submodule/diff, executable, renderer, sandbox and model identities.
5. Use Aster Test if its admitted driver is compatible. Otherwise retain the exact infrastructure failure and drive background native CUA with semantic AX and app-bound keys, never raising the app or taking the pointer.
6. Follow SC-002/003/004. Never feed model answers, evaluators or earlier submissions. Keep measured regressions and semantic failures visible.
7. Write OUTCOME.md with Result, What landed, commands/counts and Explicitly not claimed. No commit or push.

## Recovery note — 2026-10-23

The user authorized restoration after external root restore/clean removed implementation and evidence. This document was recovered from the original task patch. Prior baseline screenshots/logs are currently missing; historical summaries are not current proof. All implementation gates require new verification. Retained Studio changes and original evaluation inputs are preserved.
