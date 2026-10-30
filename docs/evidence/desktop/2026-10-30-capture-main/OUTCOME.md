# Capture changes submitted to main

Base: Aster `376239d65c277e61de67a716b6d1d2579cb492c4`, private Studio
`e518c91e41e220478dadfba69e10488038128f8f`. This is the bounded recorder,
composer and usage interaction change requested in the conversation. It does not
complete every row of `specs/027-scrawl-evidence`.

## Result

- Current first-party package, module, helper, skill and bundle names use Capture.
  Upstream provenance and legacy `.scrawl`/`scrawl.json` reads remain preserved.
- The shared screenshot/recording split buttons have identical 24 × 30 CSS-pixel
  targets and 20-pixel icon-center spacing. Recording menus show screen, area,
  window and hide icons. The hide checkbox stays open when toggled, keeps its
  icon in either state, and places its check on the right.
- Native recording commands, owner-bound delivery, elapsed-time/Cancel/Finish
  controls, helper packaging and the main-window capabilities are connected.
  The home draft uses its own recording owner. Explicit insertion does not send
  a message or silently upload source video.
- Sending one capture emits contact sheet then summary, followed by the next
  attachment in its original order. Replacing only the grouping helper with the
  base implementation makes the new send regression fail; restoring the candidate
  passes it. Inputs and assertions are identical between these runs.
- The daily-usage view uses the real existing usage buckets/range and the shared
  terrain component. Orbit no longer changes fit/zoom; sub-5px movement does not
  start dragging; clicks, pointer capture, touch, pinch, keyboard and reset work.
- Three editor preferences are declared in the generator source and regenerated
  into TypeScript and the native key list. Existing `/aster` backup defaults are
  preserved by correcting their source definitions before regeneration.
- The final packaging source requires `capture-helper` and the microphone usage
  description. Missing either refuses sealing. No application version was bumped.

## Validation

Product checks ran in the isolated publish worktree with the pinned Rust toolchain.
Baseline comparisons used the existing isolated baseline worktree.
Exact output is retained losslessly in `checks.tar.gz`; `manifest.json` records
entry sizes/hashes and was compared with a reopened archive.

| Check | Result |
| --- | --- |
| Capture library, no default features | 391 passed, 1 ignored |
| Studio recorder tests | 14 passed |
| Studio visual processing tests | 33 passed |
| Voice library after shared cpal update | 45 passed |
| Screenshot, recorder, terrain, attachment and capability renderer suites | 246 passed, 28 files |
| Actual AgentComposer capture/workspace send regressions | 2 passed; 155 unrelated cases filtered |
| Editor/terrain after preference regeneration | 39 passed, already covered above |
| Delivery sealing, including missing helper/microphone description | 20 passed |
| Workspace dependency guards | 6 passed |
| `cargo fmt --all -- --check` | Passed |
| Owning-package `cargo clippy --locked --offline -p capture -p a3s-studio -p a3s-code-voice --all-targets --no-deps` | Passed with existing warnings |
| Renderer production build | Passed |
| Translation check | Passed, 23,386 translations |
| ESLint on changed TypeScript sources | No errors; 15 warnings retained |
| Oxlint on 78 capture-owned files excluding two baseline-failing composer pages | Passed with denied warnings |
| Chromium/WebKit menu checks | 32 checks across 4 viewport/theme cases passed |
| Chromium terrain interactions | 10 checks passed, zero page errors |

The menu receipts record both button pairs at exactly 20px icon-center spacing.
The terrain fixture has 371 calendar cells and 365 enabled dates. The PNGs were
reopened and inspected. Browser processes and the owned Vite server are closed
at the end of verification.

Whole-renderer TypeScript is not green: 368 diagnostics, compared with the earlier
373-diagnostic base log. No additional `(file, error-code)` count was introduced.
The only diagnostic-bearing changed files are WorkbenchHome and the main entry
point, at their unchanged pre-existing CSS/notice statements. Rendered union-type
messages can differ after adding preference keys; the audit retains those details.
Full changed-file Oxlint retains the same four WorkbenchHome errors and React
import warning observed on the unchanged base pages. One existing AgentComposer
case expects Chinese output while its translation fixture uses English; it fails
on the unchanged base too and was not modified to obtain a pass.

The first typecheck without the repository's 8 GiB Node option exhausted memory;
the recorded final result uses the package's prescribed memory option. The first
menu attempt overlapped source/dependency rebuilds and lost focus during reload;
the final frozen-source run passes without changing assertions. A3S Test refused
the installed browser adapter 0.3.2 (requires >=0.4.0, <0.5.0), so the repository's
existing Playwright was used.

## Explicitly not claimed

- A newly installed/released app, a full current-commit native CUA recording,
  external-window recording, physical touch/trackpad acceptance, or new TCC grants.
  Earlier native screen/area acceptance belongs to its recorded source snapshot.
- Completion of agent recording requests, recording-frame redaction/derivatives,
  standalone CLI distribution, iPhone/CaptureKit/ingest/retention, or other-platform
  recording. The embedded recording library and host capability are macOS-only;
  the preserved standalone GPUI application is opt-in and was not built here.
- A clean repository-wide typecheck/lint or green remote CI. Known baseline
  failures are retained; no frozen grader, prompt, model or timeout was changed.
- Changes to the original dirty root/index or unrelated worktrees. Studio product
  source is committed only to the private submodule; the parent records its gitlink.
- GitHub recognition of the signing key. Commits are locally GPG-verified and
  DCO-signed; the existing account/key association can still show `unknown_key`.
