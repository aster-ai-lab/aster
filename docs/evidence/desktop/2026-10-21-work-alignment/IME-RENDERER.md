# Desktop input and fast-completion alignment

2026-10-21. Implements T002 / FR002–003 and preserves FR010. The renderer changes are ready for integration; native GUI and packaging acceptance remain separate gates.

## Changes and owning regressions

| Regression | Repair and maintained test |
| --- | --- |
| Candidate Enter/history navigation reached fallback shortcuts after native flags cleared | `ComposerSurface.tsx` checks the existing composition session, native flag and keyCode 229 before shortcuts. Lazy-surface tests preserve browser default and prohibit sending/history navigation. |
| Suggestion plugins consumed IME keys before the regular key handler | Runtime `handleDOMEvents.keydown` stops the ProseMirror handler chain without preventing browser default. The maintained hybrid test uses the actual editor preset and ProseMirror plugin chain. |
| Selected variable composition swallowed ProseMirror cleanup or interpreted empty cancellation as old pinyin | Runtime preserves empty versus missing data and returns false from all owned compositionend branches. Five real Tiptap cases cover final text, both cancellation orders, missing data and a rejected input-limit edit, including a later deliberate send. |
| Approval, custom-answer, variable and QuickPanel shortcuts fired during composition | Approval identity/session guards, custom-answer lifecycle guards, legacy 229 handling and QuickPanel capture-phase tracking cover Enter/Escape/Tab/arrows. Tests also use ordinary keys after the session ends. |
| Variable editing retained a stale composition session after close, token switch or blur | Reset the ref at those ownership transitions. Three public-prop/DOM cases fail before the extra repair and pass after it; no new global tracker is introduced. |
| SWR discarded a terminal history fetch overlapped by optimistic or duplicate reservation writes | Keep the optimistic mutation, then start a nonblocking revalidation with an observed error path. Guard before mutation and after its await with the committed session owner, preventing stale callbacks from writing/revalidating another key. Eight real SWR/controller/handoff cases cover both orderings, duplicate acknowledgement, session changes, suspended selection and failed background reads. |
| A newly allocated static extension list needlessly changed downstream identity | Return the existing complete static list. Actual callers only read it. The real Desktop no-model provider/runtime test stays usable and sends with both existing topContent and dockContent. |

Desktop's no-model home currently does **not** mount Work's AttachmentToolRuntime chain. Its lifecycle test was green before this change. Work's previously observed React 185 loop is not claimed as reproduced in Desktop, and no attachment runtime was added to manufacture that failure.

`topContent` was already supported inside the frame before the editor. Its public API/layout did not need a new patch. Desktop's `composerQuickPanelReady` timeout, deferred focus intent, dockContent and send-repeat protection remain in place.

## Evidence and counts

- [Initial before](ime-before.log): eight suites, 29 failures / four passes / 188 skipped. Some initial history failures reached Desktop's separate live-to-terminal observation policy first.
- [Isolated handoff/lifecycle before](ime-handoff-lifecycle-before.log): seed actual streaming status to isolate the reservation race; four handoff cases fail on missing final body, duplicate overlap, stale-session mutation and absent background read. The actual no-model lifecycle remains green.
- [Plugin-chain before](ime-plugin-before.log): all four IME/native-229 cases fail through the actual ProseMirror chain.
- [Additional variable lifecycle before](ime-variable-lifecycle-before.log): all three ownership transitions fail before ref cleanup.
- [First after](ime-after.log): ten suites / 395 passes, before adding the three ownership cases and neighboring suites.
- **[Final owning and neighboring run](ime-final-regression.log): 13 suites / 429 tests passed.** This supersedes earlier overlapping counts; they are not added together.

Final command: from `apps/desktop`, `bun x --no-install vitest run --project renderer --maxWorkers 1` with the ten changed test paths in [source receipt](ime-source-receipt.json), plus `useAgentSessionParts.test.ts`, `useConversationTurnController.test.tsx` and `useComposerFileCapabilities.test.ts`.

The 20 source/test paths and SHA-256 identities are in [ime-source-receipt.json](ime-source-receipt.json). No build, app launch, commit, staging or user-state operation was performed by this renderer work.

## Static checks and honest limits

- [Final scoped Oxfmt](ime-format-final.log): 20 owned files pass. Only owned files were formatted. AskUserQuestionComposer's prior import/class ordering difference is retained in the [baseline comparison](ime-format-baseline.log).
- [Scoped ESLint](ime-eslint.log): exit 0; two unchanged `ComposerSurface` props-dependency warnings remain.
- [Scoped Oxlint](ime-oxlint.log): `--deny-warnings` exits 1 for four existing React default-import warnings, with no production error. [HEAD source with the same tsconfig](ime-oxlint-baseline-tsconfig.log) reproduces those four warnings; the mirror also reports an unrelated old assertion diagnostic because its other imports are not a full copied application. That extra mirror diagnostic is not reported as a product result. The [first incomplete mirror attempt](ime-oxlint-baseline.log) is retained and superseded by the tsconfig-qualified check. No lint rule or allowlist was relaxed.
- Root's [full web TypeScript log](typecheck-web.log) is nonzero. The QuickPanel `anchor.width` diagnostic also exists in unchanged HEAD logic; [exact old/current source comparison](ime-typecheck-baseline.json) records the baseline and line shift. The three new test suites have no diagnostic in that run. This is not a clean-TSC claim; final builds/interactive acceptance remain root-owned.

## Review boundaries

New tests observe edits, approvals, browser default, sends, final message parts and session isolation. Real PM/Tiptap, SWR and composer providers remain active; only translation or existing data/IPC boundaries are mocked. The lazy-runtime test isolates lazy handoff, and the existing mock-heavy composer suite adds a real editor/preset only for the plugin-chain boundary.

Intentionally omitted: mirror tests for internal refs/registration counts, duplicate coverage for every ordinary shortcut permutation, and a fabricated no-model attachment mount. The new standalone IME, lifecycle and SWR suites stay below the project's size review thresholds; additions to large existing suites protect their owning entry points rather than adding new shared mocks.

Two suspected risks were checked against actual callers and rejected: internal variable TEXTAREA events are ignored by Tiptap NodeView.stopEvent, and the live QuickPanel view/adapter is mounted inside the owning Runtime rather than retained independently after it is removed. These do not justify propagation changes or an extra global lifecycle abstraction.

The renderer cases cannot certify a physical Kylin desktop, OS candidate-window behavior or Linux module/security properties. Those require the separately owned packaging/protocol/GUI evidence.


## Native approval capability follow-up

The UX owner added `allowRejectionReason` with a true legacy default and false for the native boolean-only receiver. Unsupported note controls are hidden and no note payload is emitted. Independent source review confirms the existing approval-ID composition guard, native `isComposing`, keyCode229 check and compositionend/blur cleanup are preserved. The native path never offers the unsupported note editor; legacy note editing keeps the IME guards.

Exactly two files in the original 20-file IME scope changed: PermissionRequestComposer and its sibling test. The [original receipt](ime-source-receipt-before-native-capability.json) is preserved; [the current receipt](ime-source-receipt.json) records the approved final source bytes and links the UX receipt identity. The original 429-test run is historical evidence for its original bytes. The later owning [UX final suite](ux-final-suite.log), reported by the owner as 749 passes / one skip across 27 files, includes the preserved neighboring approval IME cases and the new capability cases. This reviewer did not rerun those tests, and these overlapping counts must not be added. No broader renderer change or new platform claim is inferred from this capability review.
