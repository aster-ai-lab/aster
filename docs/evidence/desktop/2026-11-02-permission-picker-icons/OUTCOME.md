# Permission picker icons and colors

The user requested icons and colors that match each mode's meaning.

| Mode | Lucide icon | Color | Reason |
| --- | --- | --- | --- |
| Ask before changes | ShieldQuestionMark | Blue `#1a5fd6` | A permission decision is needed |
| Read-only | Eye | Graphite `#3f3f3f` | Inspection, distinct from the separate planning control |
| Smart approval | ShieldCheck | Teal `#0f766e` | Automatic authorization within its existing limits |
| Full access | ShieldAlert | Deep red `#b42318` | Broad authority that needs a visible warning |

The separate Plan mode option retains Route. An unavailable catalog uses
CircleHelp rather than the normal approval icon. All glyphs retain the existing
16 px size and 1.7 stroke. A single color map now drives trigger text and
icons, replacing two identical maps. The neutral backgrounds, names, checked
indicator and High risk text stay readable without relying on color alone.

[Preview](preview.png), [Chromium](chromium.png), [WebKit](webkit.png).

## Verification

Host: macOS aarch64. Exact commands and exits: [checks.json](checks.json).

- [Existing renderer suites](renderer.log): 29 passed.
- [Production-renderer browser campaign](browser.json): 6 passed, 0 failed,
  0 skipped, 0 flaky. Chromium and WebKit each cover consecutive choices,
  keyboard focus/selection, quick toggles, and reduced motion at 960×640.
- [Authored color contrast](contrast.json): 17 foreground/background pairs
  pass 4.5:1; the minimum is 4.93:1. This calculation covers the posture colors,
  including the risky trigger's hover background. It is not a claim about
  every caption or all product accessibility requirements.
- [Oxlint](oxlint.log), [formatting](format.log), and [ESLint](eslint.log):
  no errors; ESLint retains two existing dynamic-translation warnings.
- [Web typecheck](typecheck-web.log): exit 2, 235 errors in the shared checkout,
  with no diagnostics naming the changed picker or its tests. Full workspace
  checks were not rerun and are not claimed green.

The [first browser campaign](browser-first.json) passed five checks and failed
Chromium's initial Enter-to-open assertion. The test now sends its first Enter
to the trigger locator, rather than focusing it and sending a separate global
keyboard event during startup. Later Escape focus return and arrow navigation
still use and assert actual keyboard focus. No assertion, timeout or product
keyboard handler was weakened; the earlier failure remains retained.

The preview runs an isolated renderer with the repository fixture on port 4537,
not the user's native application. Existing Playwright tooling is used because
the previous task established that the installed A3S browser adapter was
incompatible; that adapter was not changed or retested here.

Only this task's three source deltas are selected. Other work in the shared
checkout is excluded from the commit; the test results still reflect that
shared checkout. Source and image digests are in [source.json](source.json).
Studio commit `986c9c4edd12275371e050aafd9aa4cc490a43e4` is published on
remote `main`, with a valid local signature and DCO sign-off. GitHub reports
`verified: false`, `unknown_key`; account settings are not changed here.
[Publication details](publication.json) record the result.

## Explicitly not claimed

- Installation or interaction with the user's current Tauri window.
- Windows/Linux native testing or Safari application qualification.
- A clean full TypeScript/lint gate, English visual acceptance, or a full
  accessibility audit. The browser fixture uses Chinese.
- Changes to authorization policy, model prompts, IPC or native execution.
