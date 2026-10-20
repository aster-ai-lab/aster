# Frontend hardening independent review

The frontend A changes were reviewed independently after implementation. That scoped review found no additional defect in the IME event guards it examined. Subsequent full WebKit testing exposed the separate R11 rich-composer registration loop, which this narrower review did not cover. See [COMPOSER-LIFECYCLE.md](COMPOSER-LIFECYCLE.md) and [GUI-REVIEW.md](GUI-REVIEW.md). No product source was edited during the original independent review.

The review traced PermissionRequestComposer, AskUserQuestionComposer, PromptVariableToken, QuickPanelView and its adapter types, createComposerInputAdapter, the actual React node-view event boundary, and Tiptap editor destruction. It checked native composition flags, legacy key code 229, window capture ordering, composition-end/blur cleanup, ordinary shortcut recovery and actual API callers. The root agent separately recorded the complete eight-file renderer regression run and baseline type-check comparison; this review does not replace those receipts.

## Two suspected risks disproved

1. **An IME key from the prompt-variable textarea might reach the parent composer shortcut when its native isComposing flag is false.**

   The actual ComposerTokenNode uses ReactNodeViewRenderer's default NodeView.stopEvent. Tiptap 3.31.3 rejects input events from child TEXTAREA controls before ProseMirror's root keyboard handlers run. A real node-view DOM probe started composition in the textarea, sent Enter with keyCode 13 and isComposing false, ended composition, then sent Enter with keyCode 229. Both retained browser defaults and neither invoked the parent ProseMirror keyboard handler. No extra propagation block was needed for that hypothetical failure.

2. **A retained composition reader might throw when its editor is destroyed.**

   Tiptap 3.31.3 Editor.view exposes a stub with composing false when its real view is absent; this property read does not use the proxy's throwing path. A real Editor was created, an adapter retained, then the editor destroyed. adapter.isComposing() returned false without throwing. In the production tree QuickPanelView also shares the runtime lifecycle and removes its capture listener during cleanup. No speculative try/catch was needed.

## Focused verification

The temporary review probes reused the repository's real ComposerEditorHarness and actual Editor. They live under target/evaluations/kylin-composer-review and do not modify maintained tests or source.

From apps/work:

```bash
./node_modules/.bin/vitest run \
  --config /Users/roylin/code/aster/target/evaluations/kylin-composer-review/vitest.audit.config.ts \
  --project renderer frontend-a-final-review -t 'final-review:'
```

Result: **2 passed**, 59 unrelated cases skipped. The bounded runner output is retained in frontend-independent-review.log. Scoped git diff --check also passed. The log includes existing Vite/TanStack route-discovery warnings; the focused tests passed.

## Regression receipts and count boundaries

| Scope | Receipt | Result and limit |
| --- | --- | --- |
| R1/R4 entry guards | [frontend-a-validation.json](frontend-a-validation.json), [before](frontend-a-real-primitives-before.log), [after](frontend-a-final.log) | 15 added regressions failed against original source; 168 cases in five files passed after the fix. This is renderer coverage, not native WebKit acceptance. |
| R2/R3 real editor lifecycle | [composer-ime-final.log](composer-ime-final.log) | Five cases cover commit, explicit empty cancellation, cancellation after an empty update, missing end data, and length refusal. They use the real editor and do not manually clear `view.composing`. |
| Combined R1–R4 renderer run | [frontend-regression-final.log](frontend-regression-final.log) | Eight related files, 384 passed, before R11 was discovered. The later five-case IME rerun overlaps this total. |
| R11 rich composer lifecycle | [before](composer-lifecycle-before.log), [after](composer-lifecycle-after.log), [focused regression](composer-lifecycle-regression.log), [analysis](COMPOSER-LIFECYCLE.md) | The final regression fails with the original source's same maximum-depth stack and passes with the static-capabilities fix. Five focused files pass 33 cases; this includes the existing five IME cases and must not be added to 384 as a count of distinct tests. |
| R11 rebuilt candidate source freeze | [source-freeze-r2.json](source-freeze-r2.json), [independent proof](source-freeze-r2-independent-proof.json) | 31 source/test files match in the original and isolated trees. The previous 29 hashes are unchanged; only the R11 capabilities module and maintained lifecycle test are added. This does not prove the currently building binary or GUI. |

R11's focused harness uses the real QuickPanel and tool providers, attachment runtime, launcher-version subscription, no-model capability branch and Tiptap editor. Its successful edit and Enter send prove recovery of that source-level loop. A rebuilt application's real WebKit run remains the acceptance gate; the pre-R11 frozen package did not pass that gate.

## Explicitly not claimed

- Physical Kylin desktop, full WebKit application, candidate-window positioning, keyboard hardware or cross-window focus acceptance.
- Browser-specific lifecycle behavior not represented by these two focused DOM probes.
- Full repository CI or a type-check result from this independent review.
- A source change, commit, publication or installer change by the reviewer.
