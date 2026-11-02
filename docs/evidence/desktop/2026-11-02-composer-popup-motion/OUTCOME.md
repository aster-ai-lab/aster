# Composer popup motion

The reference's #1 shows the menu opening from its trigger; #3–#4 show one gray
hover plate moving between stationary labels. The change follows those details.

- Anchored composer menus fade in and scale from 97% over 150 ms. Popup
  positioning transforms and item hit targets stay fixed.
- One rounded gray plate follows the pointer with a 420 ms spring and no
  overshoot. Hover does not change selection. Existing checks and switches
  still show the chosen values. Keyboard focus and selection update immediately.
- Reduced motion disables these transitions. Existing modal dialogs keep the
  shared dialog component's fade-and-scale behavior.

The shared behavior reaches permission, model and nested model menus, scope,
project, workspace files, goal details, composer options, prompt optimization,
slash/@ candidates, token previews, recording and screenshot menus, toolbar
shortcuts, and the branch/worktree and parameter selectors. SearchSelect opts
in at composer call sites, so its other consumers keep their existing behavior.
No dependency was added. The old permission row entrance rules were removed.

[Actual renderer recording](popup-demo.mp4), [permission](permission.png),
[options](options.png), [model](model.png), [scope](scope.png),
[optimization](optimize.png), [slash candidates](slash.png).

## Checks

Host: macOS arm64; isolated production renderer on port 4537, using the existing
repository fixtures and Playwright workflow. Commands are in [checks.json](checks.json).

- [Final browser campaign](browser-qualified.json): 20 passed, zero failed,
  skipped or flaky. Chromium and WebKit each run seven new popup checks and
  three existing permission interaction checks. They cover actual DOM geometry,
  hover without selection, draft preservation, Escape, keyboard focus,
  consecutive choices and reduced motion.
- [Renderer checks](renderer-final.log): 92 passed across five suites.
  [Goal/worktree checks](goal-worktree-tests.log): six passed across two suites.
- [SearchSelect checks](ui-recheck.log): 23 passed, including a new test that
  distinguishes hover, disabled choices and actual selection.
- [Actual animation samples](motion-samples.json): entrance opacity and scale
  transitions were observed; 49 animation frames include intermediate hover
  positions and the final target. The recording is synthetic fixture UI.
- Scoped formatting, Oxlint and ESLint pass with existing warnings. The
  [translation check](i18n-recheck.log) validates 25,598 entries.
- [Web typecheck](typecheck-web-recheck.log): 231 existing errors in this shared
  checkout; no diagnostics name this task's changed production files.
  The package-manager wrapper also hit an unrelated unused-patch error; direct
  installed tools were used. No dependency or lockfile was changed for this task.

## Earlier failures and corrections

The first browser campaign failed because the shared fixture's broad agent
route returned a paginated object for `/api/agent-workspaces`, whose real
contract is an array. That route now falls through to its existing UI handler.
Production workspace parsing was not relaxed. The second campaign passed five
cases and found two issues: its model fixture lacked the existing provider
capability metadata, and a more specific item transition overrode reduced
motion. The fixture now uses the existing configured-provider contract, and the
reduced-motion rule takes precedence.

An interrupted campaign encountered conflict markers and overwritten markup
from a concurrent edit. Those results are retained in `browser-final.json`;
`browser-recheck.json` and `browser-qualified.json` each pass all 20 cases.
The first renderer rerun caught a missing DOM containing block in jsdom.
Geometry measurement now returns when no containing block exists; real browser
checks still exercise the measured layout. No fake dimensions were added.
The Studio UI-package test wrapper could not load the external test file;
the package's own installed runner passes all 23 cases.

The optional recording script initially failed because tsx inserted a helper
into a browser-evaluated nested function. The corrected instrumentation samples
actual frames without changing product code, test assertions or timeouts.

## Scope and limits

The shared checkout contains substantial concurrent work. Only this task's
source changes were selected for the commits; tests ran on the shared working
copy. [source.json](source.json) records both tested and committed source hashes.
Screenshots were reopened and checked. The source capture, audio, and native
user window screenshots are not included.

No packaged application was rebuilt, installed or restarted. The current
native fork1 window and Windows/Linux native runtimes were not exercised.
Full workspace lint/typecheck is not claimed green. The installed A3S browser
adapter remains incompatible with its engine; it was not modified.

Studio commit `ec5128e56` is pushed to remote main with a valid local GPG
signature and DCO sign-off. See [publication.json](publication.json) for GitHub's
signature status.
