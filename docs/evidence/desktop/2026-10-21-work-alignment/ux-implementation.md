# Desktop planning, goals and approval renderer alignment

Status: renderer source ready. Identity is pinned by [ux-source-ready.json](ux-source-ready.json), 61 scoped source/test/locale files at root baseline `eb17c6ce5aa4c9ef785c12d56515e146866b1506`. This report does not claim the native bridge or a packaged GUI has passed integration; those gates belong to the separate native and final desktop runs.

## Result and trust boundaries

- Planning is an independent per-submit boolean. Permission posture remains its existing wire value; the old `plan` permission is labelled “只读权限” / “Read-only permissions” so it cannot be confused with planning. A compact switch has no extra descriptive row. Explicit non-local runtime, Paperloom and custom home mode actions cannot promise local ACP enforcement: the switch explains unavailability and sending refuses without clearing the draft. False planning keeps the existing actions.
- The submitted planning choice is captured before any asynchronous agent update/home mode preflight, remains frozen in queued payloads, and is transferred to a newly created task before adoption/autosend. Four actual AgentComposer cases reproduced the prior true/false preflight race. Model names and natural-language contents do not decide capability.
- A planning response has a compact review surface in existing `topContent`; revision or execution never overlaps the editor and never elevates permission posture. Its authority binds member, session, exact message content revision, current draft/files and asynchronous preflight ownership. Automatic draining is suspended during review. The project retains existing `dockContent`, background WebView, timer/focus paths and Paperloom/home mode actions.
- Canonical structured task plans, including valid successful tool-only plans without terminal prose, can be reviewed. Empty canonical plans remain ineligible even beside narrative text. They have a compact expandable progress control while active and a retained historical checklist. Completed controls clear without deleting history. Unknown plan states fail closed. Failed, cancelled and skipped remain distinct; a skipped step is not labelled cancelled. `data-task-plan` is hidden both in the part renderer and layout grouping, avoiding a blank “Processed” row.
- Native approval affordances depend on the live `pendingApprovals` receiver snapshot, not saved chat rows. Answers carry session, execution, anchor message, message, approval and tool identity. A known empty snapshot withdraws controls; loss of connection withdraws authority. Stop remains reachable with replacement composer overlays, but the renderer does not invent settlement on click. Reconnect and same-status updates can recover current receivers.
- Native ordinary permission answers support boolean approval with exact binding. Edited input is rejected explicitly rather than silently stripped. Native boolean-only receivers do not expose a rejection-note affordance that the bridge cannot transmit; `allowRejectionReason` defaults to true for legacy transports, and a capability change immediately hides and excludes any old typed note. The existing IME/229/composition guard is preserved. Native user questions use the separate owned form path; legacy non-native question behavior is retained when native authority is absent. No native permission UI claims that edited answers were accepted.
- Goals render from the strict typed native envelope, with safe nonzero revisions, exact session/runtime/execution binding, closed state/tick sets and explicit unavailable semantics. Cold reconnect can load through the read-only GET; late HTTP/ABA responses cannot replace newer SSE or another task. Completed/cancelled/unavailable controls hide while history remains. Selecting an explicit non-local draft hides the previous local objective. The current chip offers read-only details; this change adds no pause/resume/cancel buttons or write RPC.

## Executable evidence

| Check | Before | Final / evidence |
| --- | --- | --- |
| Native rejection-note capability | 2 failures, 1 pass | 46 owning/adjacent pass; [before](ux-permission-capability-before.log), [after](ux-permission-capability-after.log) |
| Live approval authority/Stop | 8 failures, 27 pass | Included in 756-pass final suite; [before](ux-approval-before.log) |
| Native receiver reconnect | 1 failure, 20 pass, 1 skip | Included final; [before](ux-plane-before.log) |
| Independent plan flag / member scope | 2 failures, 1 pass | Included final; [before](ux-planning-before.log) |
| Canonical task-plan projection | 1 failure | Included final; [before](ux-plan-projection-before.log) |
| Cross-window permission snapshot | 1 failure, 3 pass | Included final; [before](ux-plan-storage-before.log) |
| Submit-choice preflight race | 4 actual caller failures | All four pass final; [before](ux-plan-submit-before.log) |
| Explicit home-mode action guard | 2 semantic failures against saved original module | Included final; [before](ux-mode-guard-before.log) |
| Tool-only and empty canonical review | 2 failures, 1 pass | Three pass final; [before](ux-plan-only-before.log), [after](ux-plan-only-after.log) |
| Non-local draft goal display | 1 failure: prior local objective stayed visible | Included final; [before](ux-goal-lane-before.log) |
| Focused and adjacent preservation suites | — | **29 files, 756 pass, 1 existing skip, 0 fail**; [command](ux-final-suite-command.json), [log](ux-final-suite.log) |
| Scoped format / ESLint | — | Exit 0 / 0 errors, 10 existing warnings; [format](ux-format-final.log), [ESLint](ux-eslint-final.log) |
| Locale contract | — | 21,932 translations pass; [log](ux-i18n-final.log) |
| Scoped real compiler comparison | 20 original diagnostics | 18 current, **0 added** with the same tsconfig and original tracked source overlays; [comparison](ux-typecheck-comparison.json) |

The suite uses the real AgentComposer caller (including its preflight and queue boundaries), real home mode module and plan/review components, shared schema parsers, and mocked transport endpoints for deterministic ownership races. It does not substitute for actual ACP/native or GUI evidence.

## Existing failures and bounded scope

The whole web TypeScript check remains nonzero ([log](ux-typecheck-web-final.log)); no allowlist was changed. The scoped comparison loads the actual original tracked versions of all owned files, omits the new files from the original root list, and compares filename/code/message/target under the same compiler configuration. It proves no newly added diagnostic in this scope, not a green whole-tree build.

The older AgentPage suite has six sidebar expectation failures. The same six fail, with 72 pass, when the original production modules are restored through a read-only source loader ([baseline](ux-agent-page-baseline.log)); these failures are neither removed nor counted as passing. A test mock was completed with the existing `useUpdateAgent` hook so it can reach those unchanged assertions. No production AgentPage code was edited.

## Naming interpretation

The root [development standards §5](../../../development-standards.md) says “Files are snake_case.” The more specific existing [Desktop naming rules §3](../../../../apps/desktop/docs/references/architecture/naming-conventions.md) require PascalCase for React component files, camelCase for TypeScript sources, and matching test filenames; Desktop `naming/path-case` enforces those zones at error level. Parent explicitly directed this scope to follow the more specific active frontend convention. The nine new TS/TSX files use those names; no naming rule, constitution text, exemption or allowlist was changed. Rust files remain snake_case.

The final capability closure added `PermissionRequestComposer.tsx` and its sibling test to this scope with parent/peer coordination. Their existing IME changes from the composer owner were retained; this change adds no ACP rejection-note field or native answer DTO change.

## Actual desktop-send blocker closure

The parent’s actual desktop run found the first submitted Chinese user bubble crashed the chat boundary. `UserBubbleMessage` referenced `isLatestAssistantMessage`, which existed only in another component’s scope. The actual unmocked MessageFrame → MessageContent render reproduced that ReferenceError (one failed, two assistant controls passed). The fix removes the meaningless assistant-only prop from the user bubble and uses its existing false default; the assistant caller is unchanged. The same canonical composer context is imported directly, preserving context identity and the renderer layer boundary.

The new real-component regression includes Chinese body/timestamp and latest/older assistant state. Four owning/adjacent suites pass all 104 checks; the expanded final suite passes 756 checks in 29 files, with one existing skip. [Before](ux-user-bubble-before.log), [after](ux-user-bubble-after.log), and [source identity](ux-source-ready.json) record this closure. The old 59-file receipt is retained as [ux-source-ready-before-user-bubble.json](ux-source-ready-before-user-bubble.json). This runtime error is fixed, not dismissed as a baseline diagnostic. Its scoped TS2304 disappears: 20 original diagnostics, 18 current, zero added.

## Remaining integration gates

The renderer wire contract requires the native implementation of full approval bindings, receiver withdrawal on Stop/disconnect, per-prompt `planFirst`, and the typed goal GET/status/SSE envelope. The native final contract/real companion results are recorded in [ux-native-independent-review.md](ux-native-independent-review.md). A fresh final host/renderer build and actual Desktop GUI tests must verify these endpoints together before the product is described as delivered. This report does not claim a packaged release, a real user update, or final desktop acceptance.
