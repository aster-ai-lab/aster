<a id="implementation-plan-studio--work-alignment"></a>

# Implementation Plan: AsterDream / Work alignment

**Branch**: no branch requested | **Spec**: [spec.md](spec.md)
**Gate context**: AsterDream product regime; root workspace and existing shared UI.

## Summary

Apply the proven Work fixes at AsterDream's actual callers, preserving AsterDream-specific behavior. Rebuild the shared ACP rather than duplicating Core. Native and renderer approval contracts are coordinated; update delivery remains AsterDream's own server protocol. Evidence lives in `docs/evidence/studio/2026-10-21-work-alignment`.

## Constitution Check

| Principle | Status | Evidence in plan |
| --- | --- | --- |
| I Fail closed | PASS | Exact binding, unknown state refusal, atomic update state and incomplete package refusal |
| II Evidence | PASS | Before/after receipts, scoped logs and final OUTCOME/audit |
| III Hermetic tests | PASS | Each behavior task includes owning regression; CUA adds integration evidence |
| IV Closed sets | PASS | Reuse typed state contracts; new enum parse/as_str reviewed |
| V Workspace/APIs | PASS | `just build-studio`; no Core duplication or public rename |
| VI Schema | PASS | Store changes remain AsterDream-owned; no new database migration planned |
| VII Ratchets | PASS | No allowlist widening; new UI follows existing design system |
| VIII Transports | PASS | Local ACP/cloud/other lanes remain explicit; no Work feed migration |
| IX ACL | PASS | Existing ACL parser and trust boundaries retained |
| XIII Repository | PASS | All source/artifacts in this repo, no new checkout outside it |
| XIV Names | PASS | Explicit product, runtime, execution and task identities only |
| XV Meaning | PASS | Typed task/goal states, no text-based completion/intent inference |
| XVI Pickers | PASS | Existing drawn controls; no native select |
| XVII Dialogs | PASS | Existing anchored/in-place disclosure primitives |

## Source and Ownership

| Area | AsterDream targets | Owner |
| --- | --- | --- |
| Input/rich/reply | `components/composer`, `QuickPanel`, `hooks/useAgentSessionParts.ts` and tests | composer agent |
| Plan/goal/approval UI | `pages/agents`, `data/studioAgentPlane.ts`, approval/status hooks, shared types, i18n and tests | delivery agent |
| Native approvals/models | `src-tauri/src/agent_plane`, `providers.rs`, `preferences.rs` and sibling tests | proof agent |
| Build/update/Linux delivery | `src-tauri/src/app_updater.rs`, package/build scripts, applicable native/contract tests | root |
| Shared engine | existing `crates/code` changes rebuilt; additive read-only tick on existing ACP goal/state response, with no fact append or turn queue ownership | root/native owner |

Only UI owner writes locale files; other owners send exact key/text requests. ComposerSurface remains input owner's file; UI owner uses existing topContent/dockContent callers and coordinates any prop extension. ComposerContext/Core stop interfaces belong to UI owner. Native and UI owners agree the typed approval and plan/terminal event contract before crossing files. The goal chip uses a current session/revision-bound native snapshot; Work's historical-command pause inference is not accepted as authoritative state.

## Research and Existing Patterns

- Work `ComposerSurfaceRuntime`, `composerInputAdapter`, PromptVariableToken and actual IME/lifecycle tests supply the repair, while AsterDream's timer-ready/focus/dock remain authoritative.
- Work `TaskPlanReview`, `taskPlanMode`, TaskRunSurfaces, AgentTaskProgressBar and WorkbenchProjectTray supply compact plan/goal behavior. AsterDream already has progress projections and goal draft parsing.
- Work agent_plane approval recovery/events/controllers and model default preference writer supply exact binding and rollback; AsterDream organization catalog constraints remain.
- Work updater's ready-before-event, atomic Busy and retry/error staging supply the state pattern; AsterDream `app_updater.rs` owns its distinct admin feed/archive protocol.
- Work Kylin scripts and retained native protocol/GUI probes supply qualified input-module packaging behavior; adapt product resources without copying a Work installer.

## Verification Plan

Capture clean AsterDream baseline and all existing dirty paths. First compile/check the existing host and renderer to identify prerequisites. Run small red/green cases, focused module regressions and independent reviews. Use the current A3S Test capabilities/schema; if that adapter is unavailable, record the infrastructure refusal and use supported direct CUA to obtain native evidence without pretending ACL certification. Test only isolated fixture state and owned processes. Final gates include root fmt/workspace checks, renderer build, `just build-studio`, native AsterDream flows, local package and isolated upgrade, plus Linux IME packaging/protocol boundaries.

Public release/feed changes and commits are outside this authorization. AsterDream required tenant brand assets and the pure launch parser are restored from their recorded historical source; the deleted web application and JSON model-market configuration remain removed. Packaged resources carry the selected tenant.

Existing unrelated failures are preserved, individually identified and recorded; they do not exempt any new behavior from acceptance.
