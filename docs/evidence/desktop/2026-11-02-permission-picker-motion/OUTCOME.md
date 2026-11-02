# Composer permission picker: motion and readability

## Reference and decision

The user's Scrawl contact sheet, all three full-size frames, and a 12-frame
zoom spanning 0:00.500–0:01.500 were reviewed before source changes.
At #1 (0:00), the center control starts the expansion. At #2 (0:02), the
options have fanned out and the hovered action has a strong local highlight.
At #3 (0:06), pressing the center closes the group. The continuity comes from
a stable origin, a short sequence, and one clear hover target.

The permission picker keeps its labeled list: the four modes carry different
authority and need readable consequences. The reference's continuity is used
without replacing the descriptions with icon-only radial targets.

## Changes

- The unchanged trigger rectangle gets an open fill and inset ring. Keyboard
  focus uses an ink outline on both the trigger and items.
- The panel fades and scales from 97% over 150 ms, using the Radix origin.
  Rows enter from that edge with a 3 px offset and 15 ms stagger, capped at
  60 ms. Transitions can reverse; no JavaScript animation clock was added.
- Closing retains the existing 200 ms Radix exit interval. The leaving panel
  cannot receive pointer events. Selection callbacks retain their existing
  immediate timing, including when a parent replaces the composer.
- Reduced motion removes transitions and the exit animation. Content, checked
  state and actions stay available immediately.
- Selected mode fill is the design's neutral selected-row color. Small icon
  tiles preserve each mode's existing semantic color; Full access retains its
  red text and High risk label.
- Row padding and the English/Chinese descriptions are shorter. The four
  modes remain fully visible at the tested 960×640 desktop size. If vertical
  space is insufficient, supporting content still scrolls; it is not truncated.
  The separate Plan mode option and next-run authority hint remain present.
- The change uses the existing shared dropdown and callbacks. It adds no
  dependency and changes no authorization policy, model prompt, IPC or native
  permission store.

[Preview at 1280×900](preview.png). The four minimum-size captures are
[Chromium](chromium.png), [WebKit](webkit.png),
[Chromium with reduced motion](chromium-reduced.png), and
[WebKit with reduced motion](webkit-reduced.png).

## Verification

Host: macOS aarch64. Browser checks run the production renderer at an owned
local preview on port 4537, with the repository's isolated Workbench fixture.
They do not attach to the user's native window. Exact commands and exits are
in [checks.json](checks.json).

- [Browser tests](browser.json): 6 passed, 0 failed, 0 skipped and 0 flaky
  results. Three scenarios each run on Chromium and WebKit: all four choices
  with consecutive opens, stable anchor and panel bounds; keyboard selection,
  Escape focus return and interrupted opening; reduced motion with no active
  menu animations at the 960×640 minimum desktop size.
- [Renderer tests](renderer.log): 29 passed across the existing picker and
  real shared-primitive dismissal suites. The older picker suite has a local
  UI stand-in; keyboard and popup acceptance are supplied by the real-primitive
  suite and browser tests, not claimed from that stand-in.
- [Oxlint](oxlint.log) passes. [ESLint](eslint.log) has no errors and two existing
  warnings for dynamic translation keys. [Formatting](format.log) passes for
  all five changed source paths.
- [Translation validation](i18n.log): 25,484 translations checked. The existing
  synchronization command was used after changing five keys in each locale.
- [Full TypeScript check](typecheck.log) fails with 257 errors in the shared
  checkout. No diagnostics name the changed picker or new browser test. This
  does not establish a clean full-repository typecheck.

The initial interactive A3S capability check reported browser adapter 0.3.2,
while the installed test engine requires >=0.4.0 and <0.5.0. No A3S session was
started. The existing Playwright workflow was used instead. Its missing WebKit
runtime was installed through the existing Playwright installer.

The initial test setup incorrectly required an optional native installer
dialog; [the stopped attempt](initial-browser-attempt.txt) is retained. The
[following failed attempts](browser-first.json) and
[second attempt](browser-second.json) also exposed test assumptions: a 390 px
viewport is below the supported desktop minimum, keyboard events must await
Radix's deferred roving focus, and the whole menu need not fit without scroll.
The final assertions retain full visibility of all four permission choices,
actual selection, bounds, focus and reduced-motion behavior. They wait for the
real Workbench home instead of interacting with a transient loading composer.
No production keyboard handler or permission check was bypassed.

## Selection and publication

The checkout has extensive concurrent work. Only this task's three tracked
source deltas and two new source files are selected for Studio publication.
The locale candidates include only the five permission-copy changes per
language; unrelated locale changes are excluded. Source and image digests are
in [source.json](source.json). Checks use the shared checkout, not a clean
checkout of the partially selected publication tree.

Commits use cryptographic signatures and DCO sign-off. GitHub did not recognize
the configured signing key (`unknown_key`, `verified: false`). Studio commit
`62653218fa1a96f7776b2e433178478d19fd9c16` is published on remote `main`; its local
signature verifies. [Publication details](publication.json) retain the result.
This task does not change account signing-key settings.

## Explicitly not claimed

- Native Tauri window interaction, a desktop rebuild/install, or replacement
  of the user's running application.
- Windows/Linux native behavior, Safari application qualification, or mobile
  layout below Aster's desktop minimum. WebKit here is Playwright's engine.
- English visual layout acceptance; both locale files pass translation checks,
  while the retained browser campaign uses the fixture's Chinese locale.
- A clean full TypeScript/lint gate. The equivalent scoped read-only lint,
  formatting and i18n checks avoid running the repository-wide mutating lint
  script over other sessions' unfinished work.
- New ACP authorization, provider-recovery, or GLM evaluations. This UI-only
  change leaves those contracts unchanged.
