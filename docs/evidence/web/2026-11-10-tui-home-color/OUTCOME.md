# TUI startup homepage and website color accents

Later website polish is recorded in [the Hallmark outcome](../2026-11-11-hallmark-polish/OUTCOME.md).

## Result

Follow-up to `../2026-11-10-studio-tui-overview/OUTCOME.md`.

- Added the real TUI startup homepage as the first and default TUI tab. The
  existing execution, model, context, and resume panels remain available.
  Studio has four tabs; TUI now has five. Each product still shows one main
  image at a time.
- The CLI download card now opens with that same homepage capture. Existing
  execution and control captures remain inside its disclosure and lightbox.
- Added violet and blue accents to the TUI website badge, selected tab,
  background, links, and check icons. The deployed homepage CSS is still an
  unchanged prefix; accents are scoped to the appended TUI section.
- Did not recolor or reconstruct any product screenshot. The original captures
  are monochrome. The source Aster Night theme intentionally uses graphite
  and silver chrome; other themes exist, and NO_COLOR can also remove colors.
  Historical NO_COLOR inheritance is not claimed as a confirmed cause.
- Updated the existing image allowlist, build copier, checker, documentation,
  screenshot identity tests, lightbox tests, default selection, and five-tab
  keyboard navigation. No dependencies or product Rust sources were changed.

## Homepage image fidelity

Source:
`docs/evidence/code/2026-11-10-deepseek-visual-e2e/screens/21-fixed-welcome-native.png`.
The source OUTCOME records a real native capture of the rebuilt welcome screen
with `Describe your task`, Aster 0.3.4, worktree/resume/changelog controls, and
an isolated DeepSeek test workspace.

Published image: `apps/web/public/download/screenshots/tui-home.png`.

- Dimensions: **1323 × 882**.
- Size: **255683 bytes**.
- SHA-256: `2d85ebfa398c0219aa36c328ac8726896cffa60bff7ec2bdd2e35f36a9e2e703`.
- Source, public image, Astro output, and staged output are byte-identical.
- All nine product image sources are checked by the existing homepage test.

## Verification

Host: macOS arm64. Website validation uses Node.js 24.12.0 and npm 11.6.2.

```sh
npx --yes --package=node@24.12.0 --package=npm@11.6.2 -- npm --prefix apps/web test
python3 apps/web/scripts/stage_site.py target/web-checks/2026-11-10-tui-home-site
git diff --check
```

All commands exited **0**.

- **22 Python tests passed**.
- **135 JavaScript tests passed**, 0 failed, 0 skipped.
- **223 Astro pages built**.
- Public checker: **276 files**, **9 homepage tabs**, **219 documentation
  pages**, **2 legal pages**, **5 distinct release URLs**.
- The default TUI selection is `tui-home`. Arrow keys wrap through five tabs;
  Home returns to the homepage, End reaches resume, and Studio state remains
  independent. No-JS content and original-image links remain available.
- Existing preview at `http://127.0.0.1:7100/` returns HTTP 200, serves the new
  homepage tab and the 255683-byte image, and remains running for the user's
  requested preview. No new server was needed.

Logs and receipts:
`target/web-checks/2026-11-10-tui-home-web-test.log` and
`target/web-checks/2026-11-10-tui-home-stage-receipt.json`.

Native Safari accessibility state confirmed the new default homepage and the
execution tab switch. Full visual confirmation was incomplete: the screenshot
was taken before the main panel painted, then the window changed/closed during
inspection. That screenshot is not counted as successful visual evidence; no
further actions were sent to the changed window.

## Native capture attempt and limits

Before attempting a fresh native TUI capture, rebuilt the current checkout:

```sh
CARGO_INCREMENTAL=0 cargo build --locked --offline -p aster -p aster-acp --bins
```

The build exited **0**, finishing in 2m 47s, with existing warnings. Log:
`target/web-checks/2026-11-10-tui-color-build.log`.
An earlier non-offline locked attempt refused a required lock update. This
website task did not edit Cargo manifests or Cargo.lock. The later locked,
offline build passed with the then-current checkout.

CUA refused to access `com.apple.Terminal` for safety reasons before any TUI
launch or new capture. No alternate route was used to bypass that refusal.
Therefore no fresh color-theme capture or new native TUI e2e result is claimed.
The existing authentic homepage image was reused instead. The agent-desktop
session `run-1794325825417-59812-0` was ended.

## Explicitly not claimed

- A new native color-theme screenshot, fresh model call, or Terminal e2e pass.
- Completed browser visual QA, phone testing, or cross-browser verification.
- Changing TUI themes or screenshot colors, committing, pushing, or deploying.
- Rust test/Clippy compliance: this task only rebuilt the unchanged product
  sources for the planned capture; existing product working-tree changes were
  preserved.
