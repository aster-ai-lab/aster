# Spotlight source walks and unified architecture figures

## Result

Implemented and started the requested local preview from the isolated worktree
`D:/code/aster/.tmp/code-docs-depth-20261028`, based on `aeeb81533`.

- Source walk: http://127.0.0.1:7101/docs/code/9.1.6/internals/system-prompts/
- Architecture: http://127.0.0.1:7101/docs/code/9.1.6/internals/

The preview ran as Python PID 53768, bound to loopback port 7101. After user
review, it was stopped at the user's request at 2026-10-28T22:51:01Z. The process
identity was checked before stopping, and port 7101 has no remaining listener;
see `preview-stopped.json`. Historical HTTP read-back of three pages and all
three reading assets matches the source bytes; see `preview-verification.json`.
The user subsequently authorized committing and pushing this version to main.
The source commit `3a8b8235c59e554dab3a9d98906e35ad872c7697` was pushed to
`origin/main` as a fast-forward from `aeeb815338fd602b6b0cb06dd455a7d209f0a9a7`.
The isolated checkout needed no merge conflict resolution; the root checkout's
unrelated working changes were not staged or modified.

## Publication read-back

The [Aster website workflow](https://github.com/aster-ai-lab/aster/actions/runs/40783193490)
completed both validation and Pages deployment successfully. Its Linux run
passed all 95 Node tests and 20 Python tests and checked 89 documents / 150
routes. Actual job results and logs are retained in `pages-run.json` and
`pages-ci.log`.

The [published source walk](https://aster-ai-lab.github.io/aster/docs/code/9.1.6/internals/system-prompts/)
and all other public deployment files were read back over HTTPS. All 198 files
returned 200 and exactly matched the SHA-256 hashes of the checked local
deployment package. All six README diagram exports match the committed Git
blobs. See `stage-published.json` and `live-site-verification.json`.

The homepage and download page both link to the six latest published stable
installation assets selected by the actual release builder. All six assets
returned 200: macOS desktop 0.1.4, Windows desktop 0.1.1, Kylin desktop 0.1.2,
and all three CLI packages 0.1.4. Draft releases remain excluded. This verifies
the links and availability, not installation or application behavior.

## Design and behavior

The original browser screenshot shows long empty narrative gaps and a code
window that can remain on an earlier file while later explanations are visible.
The redesign follows the official [Code Hike Spotlight layout](https://codehike.org/docs/layouts/spotlight):
explicit step selection beside a stable code canvas.

- Compact step cards show the active explanation, takeaway and pinned source
  link. Document scrolling does not change the selected step.
- Actual Code Hike 1.1.0 `Pre` and annotation handlers render the source.
  Actual `utils/token-transitions` runs for unwrapped same-file changes;
  cross-file or wrapped changes use a short fade. Reduced motion skips these.
- The code canvas keeps its height when switching files. Focus, wrapping,
  copy, original line ranges and expanded reading are available in the frame.
  Prompt Markdown wraps by default. Normal context stays fully readable;
  readers may explicitly enable the focus mask.
- Source line numbers use CSS-generated content, so clipboard text retains the
  original code. No excerpt character or source revision was changed.
- Mobile uses an accordion with sticky navigation and a position indicator.
  Continuous reading and disabled-JavaScript views retain every explanation
  and source block.
- All Pele figures and six current README exports share a light Aster palette,
  typography and surface treatment. Flowcharts read horizontally; conceptual
  blocks use a compact grid without adding execution edges.
- Expanded diagrams retain node explanations and source links. Native Escape
  closes expanded reading, restores the original element, position and focus.
- The page-source disclosure retains all references in first-use order; larger
  lists start collapsed to keep the reading area quiet.

## Implementation

`scripts/docs_navigation.mjs` is now the authored browser entry. The existing
build/check commands use esbuild 0.25.12, already used by
`packages/intelligent-ui`, to produce one local `public/docs/navigation.js`.
The bundle includes the Code Hike MIT and diff BSD license texts. React remains
build-time rendering; no React runtime or new public runtime asset is added.

Generated Pre/SVG styles use the shared style extractor and existing generated
stylesheet. The bans on inline HTML styles/scripts, remote fonts, iframe and
native choice controls remain in force. Source references remain pinned to
`8729d7c84a2d887c61dc206ba47bf8a63425e768`.

## Verification

| Check | Result |
| --- | --- |
| Full web Node suites | 95 passed; `node-tests.log` |
| Full Python site suite | 20 passed; `python-tests.log` |
| Deterministic build/check | 89 documents / 150 routes passed; `check.log` |
| Strict unchanged STC checker | 1054 prose units; 0 mandatory and 0 advisory findings |
| Real browser interaction | Chromium 156.0.8078.73; six groups passed; zero pageerror events |
| Source fidelity | All seven walks and 38 browser-rendered excerpts exactly match their Git ranges |
| Actual token animation | 63 token animations observed in the same-file transition test |
| Responsive layout | 390, 768, 1024, 1280, 1440 and 1920 pixels; no page horizontal overflow |
| Local delivery | Three pages and three reading assets return 200 and match source bytes |

Browser checks cover stable canvas height, explicit selection, rapid switching,
copy payload, wrapping, code expansion, interactive expanded diagrams,
reduced motion and JavaScript disabled. Results are in
`browser-verification.json` and `responsive-verification.json`.

The browser executable was installed through the repository-documented
agent-browser tool. Its automatic CDP startup failed on this host; real browser
checks then used the existing Studio Playwright dependency with the same
Chromium executable. All QA browser contexts and temporary servers were closed.
The separate preview server was subsequently stopped at the user's request.

## Visual read-back

- Baseline: `before-walk-desktop.png`
- Reference: `reference-spotlight-desktop.png`
- Desktop: `final-walk-desktop.png`, `final-prompt-desktop.png`
- Mobile: `final-walk-mobile.png`, `final-prompt-mobile.png`
- Diagrams: `final-diagram-desktop.png`, `final-diagram-mobile.png`
- Expanded views: `final-code-expanded.png`, `final-diagram-expanded.png`

Screenshots were opened and inspected. The second pass tightened the step list,
removed compounded anchor spacing, fixed the code canvas height, wrapped prompt
text and kept the default context fully readable. The full expanded-diagram
capture waits for the opacity transition to finish and verifies selected-node
opacity is 1.

## Findings and test changes

- The previous scroll-driven test was replaced with a test that scrolling
  preserves the explicit Spotlight selection and its matching file.
- Token contrast checks now inspect Code Hike's extracted style classes. The
  minimum contrast requirement remains 4.5:1 on both code backgrounds.
- Static source tests now compare actual Pre text directly to Git and inspect
  original line-number attributes. The old extra line-number DOM element is
  intentionally absent.
- A browser test initially inspected restoration before the native dialog close
  event had run. It now waits for dialog detachment and still requires the
  original source element to be restored. The first failure log is retained.
- Clipboard behavior is checked at the OS boundary using a clipboard adapter;
  production selection, rendering and copy logic run unchanged.
- Existing A3S Test declarations were updated for the actual Pele role/type and
  the new Spotlight scenario. That CLI suite was not executed on this host;
  the recorded browser checks are the actual executed evidence.

## Explicitly not claimed

No Rust runtime, model prompt, evaluation input, grader, SDK binding or application
account behavior was changed or qualified by this website publication. No release
binary was rebuilt or installed. No physical mobile-device testing or native clipboard permission
qualification is claimed. The mobile evidence uses Chromium viewports, and the
clipboard evidence validates the payload at the browser API boundary.
