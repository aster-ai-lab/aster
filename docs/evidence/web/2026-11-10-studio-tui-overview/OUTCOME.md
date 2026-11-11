# Aster website: Studio and TUI presentation

Follow-up: the TUI startup homepage and scoped color accents are recorded in
[the later outcome](../2026-11-10-tui-home-color/OUTCOME.md), which updates the
TUI tab/image counts below.

## Result and scope

Updated the existing `apps/web` homepage and download page. The reference was
<https://octop.cloud/#overview>; the visual baseline remains the deployed Aster
homepage at <https://aster-ai-lab.github.io/aster/>.

- Preserved the deployed hero, typography, navigation, scenario styling,
  security cards, and violet galaxy download section. The original 52,338-byte
  `home.css` is an unchanged prefix of the final file; 704 bytes of scoped TUI
  rules are appended. Baseline SHA-256:
  `5498388514038f5be4a713f960c2d76c65afb642b557961718625cbde3025de3`.
  `cosmic.js` is unchanged.
- Studio and TUI each have four independently controlled tabs and one main
  image at a time. Studio covers research, documents, data, and development;
  TUI covers execution, model reasoning options, context usage, and resuming
  sessions. Desktop image/text placement alternates between products; the
  existing small-screen single-column behavior is reused. There is no extra
  screenshot grid or homepage FAQ section.
- All eight product images are original application captures. The TUI images
  use `object-fit: contain` and link to their original files. The download
  page's fabricated terminal transcript, animation, and obsolete styles were
  removed; additional real TUI captures are inside a native disclosure and use
  the existing image lightbox. No application UI was recreated in HTML.
- Rewrote homepage and download copy around concrete actions and configuration
  requirements. Retained the existing brand headline. Removed vague promises,
  engineering slogans, and the download table's generic comparison with
  traditional work. MCP and Skills cards link to existing versioned guides.
- Reused the current tab controller for both products, including arrow keys,
  Home, End, focus, and reduced motion. Fixed a visually reproduced initial
  indicator sizing problem by resynchronizing after page resources load.
  Existing Studio demonstration steps remain; TUI has no invented replay.
- Registered the four new PNGs in the existing build/staging contract. Deep
  documentation hyperlinks are checked against `DOCS_PAGES`; documentation
  assets, unknown routes, queries, and missing fragments remain refused.

No dependencies, release data, Rust sources, or Studio submodule sources were
changed by this task. No commit, push, or deployment was performed. Unrelated
working-tree changes were preserved.

## Capability and image sources

Capabilities were checked against `README.md`, `apps/studio/README.md`, the
existing Office/use documentation, terminal command implementations, and the
recorded TUI control acceptance evidence. Model reasoning options depend on
the selected model; review and trace capabilities are not described as
mandatory steps or a native TUI Plan mode.

The four existing Studio images are `workspace.png`, `office.png`,
`lineage.png`, and `review.png` under `apps/web/public/download/screenshots/`.
They show real Studio UI with isolated demonstration data.

The new public TUI images are byte-identical copies of these existing native
captures. Their originating OUTCOMEs record source builds and real DeepSeek
sessions; pixels were not edited.

| Public image | Original evidence capture | SHA-256 |
| --- | --- | --- |
| `tui.png` | `docs/evidence/code/2026-11-10-tui-visual-controls/screens/16-real-model-completed.png` | `802af9377ff5eaeb6842cb53bcd239386d0b229c2c4cdd729de2c9bb56393bc0` |
| `tui-model.png` | `docs/evidence/code/2026-11-10-deepseek-visual-e2e/screens/02-effort-menu.png` | `fdef87f9efbf38bad26220aa033f60e6881dae8f50dbb62498001d19b9666f18` |
| `tui-context.png` | `docs/evidence/code/2026-11-10-deepseek-visual-e2e/screens/26-context-followup.png` | `37d8a5caa161a03d0261957c01f9d8cd29858e91feb9f1ef8b03c0dfefea3d6d` |
| `tui-resume.png` | `docs/evidence/code/2026-11-10-deepseek-visual-e2e/screens/22-resume-popup.png` | `6cd222b355ef95595cf31773744d110a5c66526be57d85f0daa92141fdcf9a4b` |

The execution image records actual file creation, reading, and an independent
20-byte file check. It is 1425 × 717 pixels, 151,559 bytes. The model image is
1350 × 926; context and resume images are each 1367 × 926. Homepage tests
compare all eight built images with their original captures. Staging records
hashes for all public assets.

## Commands and counts

Host: macOS, arm64. Node.js 24.12.0 and npm 11.6.2 match the website CI workflow.

```sh
npx --yes --package=node@24.12.0 --package=npm@11.6.2 -- npm --prefix apps/web test
```

Final exit status: **0**. Log:
`target/web-checks/2026-11-10-website-final-test.log`.

- Astro: **223 pages built**.
- Public check: **275 files**, **219 documentation pages**, **2 legal pages**,
  **5 distinct verified download URLs**, and **8 homepage tabs**.
- Python: **22 tests passed**.
- JavaScript: **135 tests passed**, **0 failed**, **0 skipped**.
- Checks include syntax, real capture identity, project-relative links under
  `/aster/`, independent product tabs, keyboard navigation, two visible product
  panels, download disclosures and lightboxes, clipboard, Canvas/WebGL
  fallbacks, reduced motion, release links, and documentation contracts.
- A focused test first reproduced the initial indicator error (`36px` instead
  of `102px` after resources loaded). It passes in the final full run.

```sh
npx --yes --package=node@24.12.0 -- node --test --test-name-pattern='tab indicators use the final layout' apps/web/tests/home.test.cjs
npx --yes --package=node@24.12.0 --package=npm@11.6.2 -- npm --prefix apps/web run preview -- --host 127.0.0.1 --port 7100
python3 apps/web/scripts/stage_site.py target/web-checks/2026-11-10-website-reviewed
git diff --check
```

The focused pre-fix command exited 1 as expected. The final full test command,
public staging, and final diff whitespace check exited 0. Stage receipt:
`target/web-checks/2026-11-10-website-reviewed-receipt.json`.

## Native browser verification

The user requested `agent-desktop`. Its bundled desktop and platform guides
were read; accessibility, screen-recording, and automation permissions reported
granted. Final QA used a separate Safari window showing the current checkout's
fresh Astro build at `http://127.0.0.1:7100/`.

```sh
agent-desktop --session run-1794318758945-7936-0 launch Safari --activate --timeout 10000
agent-desktop --session run-1794318758945-7936-0 snapshot --app Safari浏览器 --window-id w-53620 -i --compact --max-depth 16
agent-desktop --session run-1794318758945-7936-0 click @su3tko00xw1rj:e26 --timeout-ms 5000
```

The click used a fresh session-qualified snapshot ref for the TUI context tab.
It returned `delivered_verified` through semantic AXPress. A subsequent
snapshot was complete with **64 refs** and the context tab selected. Records:
`target/web-checks/final-safari-context-action.json` and
`target/web-checks/final-safari-context-snapshot.json`.

Native CUA states and screenshots verified the retained hero, Studio document
presentation, all four TUI tab selections, real model/context imagery, and
independent Studio/TUI state. Reloading the final build visibly confirmed that
the first selected tab's indicator now spans the correct button. Earlier native
inspection also opened the download page's real TUI lightbox and its original
image link. The earlier successful native screenshot is
`target/web-checks/real-tui-lightbox.png`.

A final `agent-desktop screenshot` attempt failed because CoreGraphics returned
no current display mode. It is not counted as successful screenshot evidence;
CUA window screenshots were available and visually inspected instead. The
in-app browser host was unavailable. No Studio/desktop application e2e was
performed; these are website browser checks against the fresh web build.

Owned preview teardown uses the existing command:

```sh
npx --yes --package=node@24.12.0 --package=npm@11.6.2 -- npm --prefix apps/web run preview -- stop
```

Teardown exited 0 and stopped owned preview pid 97757. The owned Safari preview
window was closed and the agent-desktop session was ended successfully.

## Explicitly not claimed

- Production deployment, commits, pushes, release refresh, or installer runs.
- Studio/TUI application changes, Rust checks, or a new live model run; existing
  native capture evidence was reused without editing pixels.
- A complete mobile interaction pass, native phone testing, or cross-browser
  coverage. The current small-screen layout rules and keyboard behavior are
  covered by local source/DOM checks, not a native phone run.
- Download network success. Earlier native browser inspection encountered CORS
  errors fetching GitHub archives; release URLs and static link contracts were
  checked, and those archive requests were not changed by this task.
- Full development-standards compliance: `docs/development-standards.md` is
  absent in this checkout. Root `AGENTS.md` and the constitution were read.
