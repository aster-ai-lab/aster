# Pele diagram migration

## Result

Replaced all 23 current SeeCode figures and six README exports with Pele 1.10.0.
Published to https://aster-ai-lab.github.io/aster/ from commit
`494ae4944374a908b2013dd041f9e70d3984ee01`. Pages run
[40770177426](https://github.com/aster-ai-lab/aster/actions/runs/40770177426)
completed successfully. Live read-back matched all 198 staged files, and all six
current README export blobs matched GitHub.

## Changes

- Authored Mermaid flowcharts, sequences and blocks under each product/version's
  `_diagrams` directory. Catalog entries and public SVG URLs remain stable.
- Retained every original node identity, label, relationship label, explanatory
  step and source selector. Ordered step/source data equals the previous
  committed definitions; see `migration-inventory.json`.
- Converted conceptual layers and independent Goal decisions to arrow-free
  blocks. Their arrangement does not assert execution order.
- Shared one Pele renderer for existing figures, guided figures and exports.
  Guides use explicit rendered node IDs for click and keyboard focus;
  references to absent nodes fail the build.
- Kept previous/next, complete view, finite narration, reduced-motion manual
  controls, source citations and seven Code Hike source walks with 38 stops.
- Removed the retired vendor tree, project settings, watermark assets and
  SeeCode animation/assembly controls. Historical provenance remains.
- Kept generated 9.1.5 documentation byte-identical to its previous state.
  The dirty root checkout was preserved. Latest upstream legal pages and
  footer links were retained in the isolated worktree.

## Verification

| Check | Result |
| --- | --- |
| Complete web Node suites | 92 passed in the final Linux CI run |
| Final documentation and diagram integration suites | 30 passed |
| Final diagram/prose regression suite | 14 passed |
| Python site suite | 20 passed in final CI; all four affected local cases passed in stable-checkout reruns |
| Deterministic generation | 89 documents / 150 routes passed |
| Unchanged STC checker, strict mode | 1054 extracted prose units; 0 mandatory and 0 advisory findings |
| Independent diagram audit | 23 SVGs parse; original node identities and ordered step/source data retained |
| Public staging | 198 files; independent source/receipt byte comparison passed |
| Release refresh | Six defaults still match the latest published stable installation assets |

Commands and exact outputs are retained in adjacent logs. Final staging is
`target/aster-pele-current-releases-20261029`; see
`stage-current-releases.json` and `stage-current-verification.json`. Earlier
failed checks remain available. The final full CI log is `ci-run-final.log`.

## Publication and live verification

- Final CI passed all 92 Node and 20 Python cases, then deployed Pages. The
  deployment identity and steps are retained in `pages-run-final.json`.
- Requested all 198 public files, using canonical directory URLs for HTML.
  Every response returned HTTP 200 and its SHA-256 matched the reviewed staging
  receipt, including all document routes, SVGs, scripts and styles.
- Every published HTML/CSS/JS/SVG file was checked for retired SeeCode markup,
  resource references and assembly selectors; none remained.
- Verified the six current README exports through GitHub's contents API and
  matched their Git blob identities to the local committed files.
- Parsed both live download surfaces, compared their six direct asset URLs to
  fresh stable Release metadata, and received HTTP 200 for every selected asset.
- macOS desktop v0.1.4 became published during this task. CI automatically
  selected it. The source snapshot was refreshed and staged again before the
  final run. Windows desktop remains 0.1.1, Kylin desktop 0.1.2 and CLI 0.1.4.
- The local preview service remains stopped; port 7101 had no listener.

See `live-site-verification.json` for timestamps, URLs, hashes, Release records,
README blob identities and installation-asset response statuses.

## Findings resolved

- Pele 1.10.0 block syntax does not accept `accTitle` and `accDescr`. Block files
  use supported syntax. The shared renderer supplies SVG titles, descriptions
  and stable accessible IDs from authored guide metadata.
- Prose extraction still referenced the retired renderer's `spec` object.
  Updated it to the guide schema and added a real CLI regression case checking
  all 1054 prose units. The STC checker and its rules were not modified.
- The existing browser scenario still selected a SeeCode sequence attribute.
  Updated it to the actual interactive Pele sequence and group role.
- Archived Pele figures retain their original source-authored accessible
  descriptions. Current exports include a native SVG background for README
  readability.
- An upstream footer fast-forward occurred while the initial Python process
  still held the previous homepage checker in memory. Four tests subsequently
  saw the new `terms/` link with that old checker. The failed log is retained;
  reruns use one stable checkout. No checker was relaxed for this migration.

- The first staging command included already-staged deletions, so Git refused
  that path list. A shell error-propagation mistake then committed the prior
  vendor removals alone. The remaining migration is committed immediately
  afterward; acceptance uses the complete migration and its successful Pages
  run, never that intermediate commit.

- The remote README directory contains seven SVGs, while the current catalog
  owns six. The additional `meta-harness.svg` is a preserved Hairline historical
  illustration, unreferenced by the current root README. The read-back verifier
  now follows the authoritative catalog mapping. The asset authoring guide was
  updated to the current six-image Pele build and passed strict STC checking.
- The first live byte comparison used the pre-publication macOS download
  snapshot. Only the homepage and download page differed, because CI had
  correctly adopted newly published macOS v0.1.4. That receipt is retained as
  `live-site-before-release-sync.json`; final source, staging and live bytes all
  agree after the refresh.

## Explicitly not claimed

No Rust runtime, model prompt, evaluation, grader, dependency version or homepage
design was changed by this migration. No measured task-quality benefit is
claimed. No native browser screenshot or pixel qualification is claimed:
browser inventory had no enabled surfaces. DOM, SVG, source, static-site and
deployment checks provide the recorded evidence. The local preview service was
not started.
