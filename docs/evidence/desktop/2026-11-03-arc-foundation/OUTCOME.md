# Arc foundation consistency — 2026-11-03

The user selected Aster's compact dimensions with Arc material and interaction. Shared controls now use 30px quiet controls, 36px form controls, 40px large actions, 13px labels, 8px control radii and 12px default floating surfaces. Caller-specified sizes remain authoritative.

## Delivered changes

- The existing Arc foundation owns semantic surfaces, text, borders, danger text, shadows and compact dimensions. No new theme engine or component dependency was added.
- Native buttons, anchors, slotted actions and custom loaders share the same variant skin. Disabled/loading links prevent activation; anchored labels preserve the caller's label/chevron distribution.
- Dropdown menus, context menus, selects, SearchSelect, command lists and popover choices share one measured hover plate. Hover does not choose an item. Keyboard feedback is immediate. Filtering clears stale geometry; scrolling retargets the plate to the visible item. Nested portals retain their existing selection and dismissal owners.
- Studio's duplicate composer hover component and stylesheet were deleted. Virtualized candidates and floating model panels use the shared owner explicitly; primitive-based surfaces inherit it automatically.
- Every Arc CSS entry declares the canonical CSS layer order. A real-browser regression reproduced an input radius being reset from 8px to 0 when Preflight loaded later; the delivered source preserves 8px.
- Native event dispatch could checkpoint a microtask before React captured a nested portal click. Deferring outside dismissal until the event has completed preserves the nested action. The browser regression now invokes the nested action once.
- Default dialog/drawer material and typography follow the same foundation. Modal movement stays in place. Reduced-motion media rules override more specific popup entrance selectors.

The [component audit](../../../../specs/040-arc-foundation-consistency/contracts/components.json) records 53 source owners, explicit exports, static imports and individual decisions. These are source relationships, not a count of accepted native widgets. The [Storybook review](../../../../packages/ui/stories/components/primitives/foundation.stories.tsx) renders the real public components.

## Qualification

Host: macOS arm64. Node 26.8.1, installed repository dependencies. No dependency install or Rust source change.

| Check | Result | Evidence |
| --- | --- | --- |
| Selected UI source, focused component tests | 31 files, 335 passed | `replay-controls-qualified.log.gz` |
| Selected UI source, package typecheck | Exit 0 | `replay-typecheck-ui.log.gz` |
| Selected renderer production build | Exit 0; existing warnings retained | `replay-renderer-final.log.gz` |
| Foundation browser suite, Chromium and WebKit | 30 passed, no skips/retries | `replay-browser-qualified.json` |
| Actual workbench suite, Chromium and WebKit | 20 passed, no skips/retries | `replay-workbench-final.json` |
| Final light/dark artifact refresh, both engines | 4 passed | `final-review.json` |
| Focused Studio behavior checks | 60 and 62 passed in overlapping focused selections | `studio-final-tests.log.gz`, `studio-picker-tests.log.gz` |
| Translation check | 25,482 entries passed | `i18n-check.log.gz` |
| Working-source UI full suite | 998 passed; 6 existing theme-contract failures | `ui-final-tests.log.gz` |
| Studio web typecheck | 384 diagnostics in the concurrent tree; none in changed production files | `typecheck-studio.log.gz` |
| Scoped ESLint | 0 errors, 15 warnings, including ignored E2E files | `eslint-studio.log.gz` |
| Scoped Oxlint with deny-warnings | Existing unsafe-finally warning in `homeComposerMode.tsx`; gate fails | `oxlint-studio.log.gz` |

The working tree contained concurrent popup-presence, dialog, toast and other changes. Qualification therefore also extracted tracked source, applied only this task's selected deltas, and built/tested that source under the ignored `work/` directory. No Git worktree or additional repository was created. The selected-source build retained the existing entrance keyframes and added the shared 97% scale recipe; it did not publish the concurrent presence/transition rewrite. The [source manifest](source-manifest.json) identifies every selected file by SHA-256 and the Studio commit.

The browser harness supplies the real semantic host canvas beneath translucent controls. Contrast measurements alpha-composite actual computed backgrounds. The enabled action/value samples range above 6.21:1 in both themes/engines; this is not an accessibility certification for placeholders, metadata, transition frames or every product page. Samples are in `control-contrast.json`.

The workbench fixture admits one gateway model. Its test asserts that count and checks that row's feedback. Multirow movement is checked through seven real shared choice surfaces in the foundation suite. Old localized close-button and model-trigger lookups were updated to the current accessible close action and stable component markers. No business selection or permission guard was bypassed.

## Retained failures and follow-ups

The six UI-wide failures are one theme-build case and five migration-contract cases. The contract rejects Tailwind adapter variables in existing renderer stylesheets (`aster-dream-theme.css` and, in an isolated attempt, `richtext.css`). Each failing test file was rerun in isolation; see `theme-build-isolated.log.gz` and `theme-migration-isolated.log.gz`. Those stylesheets, generated theme output and the checker were not changed here. Follow-up: reconcile those stylesheet variables with the existing runtime semantic contract, regenerate from the owning source, then rerun the theme gate.

The Studio-wide typecheck and existing unsafe-finally/cross-page/i18n warnings remain outside this presentation change. Follow-up: resolve the exact diagnostics in the retained logs; passing focused tests does not make those global gates pass.

Earlier failed qualification attempts are retained in `browser-family-recheck.json`, `replay-browser.json` and `replay-workbench-qualified.json`. They show the Chromium scroll-plate failure, reduced-motion/focus findings in a candidate containing an unrelated dialog edit, and stale workbench fixture selectors. Final results above use the corrected selected source and current fixture contract, without retries or relaxed geometry/activation thresholds.

## Visual review and limits

Reopened the retained light/dark review images and actual workbench permission, options and model images. They use production components and synthetic fixture data. Screenshots from the user's native window and draft were not published.

Native macOS hover acceptance was not obtained: exact-window foreground binding was refused in the available automation session. The user's running app was not restarted or replaced. Windows/Linux native runtime behavior, packaged desktop installation, every business-specific size/color override and Office/editor engines were not qualified. The 53-owner audit distinguishes these limits from the browser checks.

Studio changes were signed and DCO-signed off, then pushed to remote `main`. Local GPG verification succeeds. GitHub's verification result is recorded separately; no account keys were changed. The parent commit contains shared UI source, this evidence, the bounded specification and only the Studio gitlink.
