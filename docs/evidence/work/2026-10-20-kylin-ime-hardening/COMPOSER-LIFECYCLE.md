# Rich composer registration loop

The final pre-fix Linux GUI did not remain in the rich composer after focus. The host log records React error 185 and its error boundary removes the rich composer; WorkbenchHome then shows its separate fallback textarea. That textarea has no send keyboard handler. Consequently, the earlier screenshots of ordinary Return inserting a newline cannot prove a rich-editor IME guard failure or success.

The passive WebKit observer confirms an initial DIV with `view.composing=false`, then the fallback TEXTAREA. Its later ordinary Return is `key=Enter`, `code=Enter`, `keyCode=13`, `isComposing=false`, with `defaultPrevented=false` both before and after bubbling. The observer only adds passive event listeners and a bounded logging channel. The same visible newline behavior was seen without instrumentation, with no IM daemon, and with the Fcitx synchronous-mode experiment. Instrumentation can perturb timing and is not shipped.

## Confirmed source cause

MissingAgentHomeComposerInner subscribes to the launcher version. Before a model is selected, it calls `agentFileExtensions()` for its attachment capabilities. That function previously allocated a new array on every render. AttachmentToolRuntime's callback depends on that array; its effect therefore disposes and re-registers the attachment launcher after each version-triggered render. Each registration changes the launcher version again.

ComposerToolFooterActionsSync's layout effect also republishes footer actions when that version changes. In the actual GUI and hermetic reproduction, React reports the nested update at QuickPanelProvider's footer state setter, called by that layout effect. The source cause is the unstable static capability array, rather than a footer comparison, missing key handler on the fallback, or an IME composition flag.

The fix holds the complete extension list once in the existing capabilities module. `agentFileExtensions()` keeps its `string[]` API and returns that stable list. Both production callers only pass the list as capabilities; the attachment controller maps, filters and reads its length, and no production caller mutates the list. The model-aware hook continues to copy the same source when it derives an active model's capabilities.

## Verification

The maintained regression uses real QuickPanelProvider, ComposerToolProvider, the real attachment runtime, a launcher-version subscriber, the no-model capabilities branch and the real Tiptap editor. It asserts that the editor remains usable for an edit and an ordinary Enter send. No provider, editor lifecycle or launcher registration is mocked.

- Original HEAD capabilities source with the final regression: **1 failed**, `Maximum update depth exceeded`, at QuickPanelProvider:107 then ComposerToolRuntime:218. See `composer-lifecycle-before.log`.
- Fixed source, focused regression: **1 passed**. See `composer-lifecycle-after.log`.
- Final five-file regression: **33 passed**. See `composer-lifecycle-regression.log`.
- Scoped Oxlint with denied warnings, ESLint, Oxfmt check and `git diff --check`: passed.
- Two changed source/test files copied to the isolated build worktree with matching SHA256. See `composer-lifecycle-isolated-sync.json`.

The parent's later complete TypeScript run found two new diagnostics in this test: its generic mock callback type and the possibly absent DOM editor. The test now uses the actual composer callback prop type and returns the narrowed editor from `waitFor`, matching the maintained IME harness. No production behavior or assertion changed. Its focused rerun passed 1/1; scoped lint/format checks passed and the test-only copy has matching SHA256 in `composer-lifecycle-typefix-sync.json`. See `composer-lifecycle-typefix-test.log`. The original r2 freeze proof predates this type-only correction; a later source receipt must include its updated test hash. The complete type-check result remains a separate parent-owned gate.

This frontend subtask did not rebuild or package a binary; the parent owns those steps. The pre-R11 frozen installer remains statically intact but did not pass rich-composer GUI acceptance. A rebuilt application needs the real WebKit IME tests, including ordinary Return and candidate Return controls, before any full GUI success claim.
