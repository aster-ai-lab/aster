# Hallmark polish of the Aster website

The [style follow-up](../2026-11-11-website-style-follow-up/OUTCOME.md)
records the stylesheet correction and final landing checks.

## Result and scope

Used the user-requested [Hallmark](https://github.com/Nutlope/hallmark), version
2.0.0 at commit `407fa689f4a276f2fd389208d271d0fef08f5058`, in **polish** mode.
This retains the existing system and layout rather than choosing another theme.

- Preserved Aster's Ubuntu typography, brand headline, deployed homepage
  composition, galaxy download section, Studio/TUI tabs, and release links.
  All nine real product captures remain unchanged; TUI starts on its homepage.
- Removed redundant heading labels, tracked capitals, arbitrary ordinals,
  decorative status labels, duplicated floating callouts, and the hand-drawn
  window bar around an already real screenshot. The download hero now uses
  the existing Aster logo asset instead of a drawn approximation.
- Simplified decorative gradient washes, retained a solid blue download hero,
  and kept the existing violet galaxy. Lifted stylesheet colors and fonts into
  the existing root token system without adding public dependencies or files.
- Improved text/focus contrast, minimum control hit areas, mobile tab labels,
  image-bearing grid bounds, and the CLI release row. Product headings still
  say what Studio and TUI do; the mobile headings avoid a one-character orphan.
- Replaced the existing caption-only autoplay with manual **步骤说明**.
  Buttons name the actual actions; 下一步 advances only on click. There is no
  playing state, timer, or suggestion that the static app image is live video.
  Updated the three affected behavior tests to verify the intentional change,
  including that flushing timers cannot advance the selected step.
- The screenshot caption on the blue download hero now has a white surface
  and dark text, including its enlargement link.

The ongoing task already owned the dirty website paths. Their before state was
snapshotted under `target/web-checks/hallmark-2026-11-11/source-before/`, with a
staged baseline and Hallmark's before inventory; no commit, stash, or worktree
was used. Unrelated working-tree work was preserved. No Rust or Studio source,
release snapshot, package manifest, or lockfile was changed by this follow-up.

## Hallmark verification

Hallmark was downloaded into ignored `target/`; its checker and gates were not
modified. Puppeteer Core was installed only under `target/hallmark-tools` for
this explicitly requested tool. Rendering used an already installed Chromium
headless shell, with no access to the user's browser sessions.

Working directory for both recorded runs:
`target/web-checks/hallmark-2026-11-11`.

```sh
HALLMARK_PUPPETEER=/Users/roylin/code/aster/target/hallmark-tools \
CHROME_PATH='/Users/roylin/Library/Caches/ms-playwright/chromium_headless_shell-1234/chrome-headless-shell-mac-arm64/chrome-headless-shell' \
node /Users/roylin/code/aster/target/hallmark-407fa689/scripts/sloplint.mjs \
  site/index.html site/home.css site/download/index.html site/download/styles.css \
  --genre modern-minimal --before --render \
  --url http://127.0.0.1:7100/ --url http://127.0.0.1:7100/download/ --json
```

The final command replaces `--before` with `--kept`. Final exit: **0**.

- **0 Floor FAIL**, **0 open REFLEX**, **0 rejected waivers**.
- **2 retained Reflex categories**: the real distribution chooser (3), and the
  established glass navigation/product controls (29). Reasons are in the CSS
  stamps. There are **11 waived findings** across repeated source/URL checks.
- **3 answered** Finish findings, **7 dismissed** warnings, **229 notes** from
  keeping the owner's layout/spacing at polish depth.
- **1 acknowledged kept WARN**: `401020304` was a generated figure key derived
  from the old numeric playback controls. The manual explanation changes that
  key. It is not a missing app image: all nine capture identities still pass.
  Zero warnings is explicitly not claimed.
- Retained **66/66 links**, **55/55 script hooks**. Two product headings were
  reworded on the record. No route, form, download target, or product capture
  was removed.
- Checked widths: **320, 375, 414, 768, 1024, 1280, 1440, 1920px**.
- **20 captures** per full sweep: two routes rendered both as checked local
  files and via the live server. Captures cover desktop folds plus full pages
  at desktop and phone widths; these are four renderings of two unique routes.

Baseline findings were 598 FAIL and 276 REFLEX across those source/runtime
contexts, including repeated color literals; these are checker findings, not
598 distinct product defects.

Retained records under `target/web-checks/hallmark-2026-11-11/`:
`baseline.json`, `final.json`, `.hallmark/before.json`, `.hallmark/preflight.json`,
`.hallmark/log.json`, and `review-score.json`. The rendered comparison was
removed during local cleanup.

The fresh-context Hallmark finish reviewer found two concrete issues: caption
contrast and the misleading autoplay. Both were fixed and scored **resolved**
against recaptured pages and the actual source. No third review was invented.
A later root visual check corrected the phone heading orphan and reran the
same eight-width sweep successfully.

## Project verification and delivery

Host: macOS arm64. Website CI's Node.js 24.12.0 and npm 11.6.2 were used.

```sh
npx --yes --package=node@24.12.0 --package=npm@11.6.2 -- npm --prefix apps/web test
npx --yes --package=node@24.12.0 --package=npm@11.6.2 -- npm --prefix apps/web run check
python3 apps/web/scripts/stage_site.py target/web-checks/2026-11-11-hallmark-site
git diff --check
```

All final commands exited **0**.

- Full tests: **22 Python**, **135 JavaScript**, **0 failed**, **0 skipped**.
  The full run includes the new manual-step behavior. Subsequent changes were
  style and review metadata; final build/public checks and browser checks ran
  against those current files.
- Astro: **223 pages**. Public check: **276 files**, **219 documentation
  pages**, **2 legal pages**, **9 homepage tabs**, **5 unique release URLs**.
- Staged HTML, CSS, and scripts are byte-identical to the final build.
  Capture byte identity is checked by the project tests.
- Logs: `project-tests.log`, `final-check.log`, and `final-stage.json` in the QA
  directory above. Early checks caught the removed logo reference required by
  the public contract; the real asset was retained in the footer to fix it.

The prior user-requested preview remains running at
`http://127.0.0.1:7100/`. No production publication was attempted.

## Explicitly not claimed

- Zero Hallmark notes/warnings, a full redesign, or replacing the user's design
  with a Hallmark catalog theme.
- Native phone testing, other browser engines, a new live TUI/model session,
  recoloring product captures, or new Studio/desktop e2e evidence.
- Installer download success, release refresh, commits, pushes, or deployment.
- Rust checks or complete development-standards compliance. The referenced
  `docs/development-standards.md` is absent in this checkout; root AGENTS and
  the constitution were applied.
