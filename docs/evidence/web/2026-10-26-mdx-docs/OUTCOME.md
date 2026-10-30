# Aster MDX documentation site

Date: 2026-10-26. Scope: the current working tree's public website and its local documentation build. No deployment or commit was performed.

## Delivered

- `/docs/` is a documentation portal. `/docs/code/` contains four MDX chapters: architecture, execution, framework comparison and host integration.
- `apps/web/content/docs/catalog.json` owns project identities, chapter ordering and stable routes. It drives project cards, grouped desktop/mobile navigation, breadcrumbs, project-local previous/next links and the Python publishing set. New projects use independent directories, versions and routes.
- Repository-authored MDX renders to checked-in HTML at build time. Public documentation requires no JavaScript or React runtime. Diagrams, comparison tables, code blocks, mobile disclosures, page contents and source references remain usable without scripts.
- Homepage documentation links, Pages workflow triggers, isolated dependency installation, stale-output detection, static checks and artifact staging include the documentation.
- The original download asset boundary remains unchanged. Documentation has a separate exact publishing set and approved-reference set. Sources, dependencies, unknown assets and other tenant data are excluded.

## Verification

| Command / proof | Result |
| --- | --- |
| `npm --prefix apps/web ci --workspaces=false --ignore-scripts --no-audit --no-fund` | 116 build packages installed from the local site lockfile |
| `npm --prefix apps/web run build` | 5 MDX pages generated |
| `npm --prefix apps/web run check` | 5 generated pages match their sources |
| `node --test apps/web/tests/download.test.cjs apps/web/tests/canvas.test.cjs apps/web/tests/cosmic.test.cjs apps/web/tests/home.test.cjs apps/web/tests/docs.test.mjs` | 47 passed, 0 failed, 0 skipped; [log](node-tests.log) |
| `python3 -m unittest discover -s apps/web/tests -p 'test_*.py' -v` | 19 passed; [log](python-tests.log) |
| `python3 apps/web/scripts/check_site.py` | 25 public files; 5 documentation pages; 6 original download links; 4 homepage tabs |
| `python3 apps/web/scripts/stage_site.py target/aster-site-mdx-20261026` | 25 checked public files; no private sources required |
| `a3s-test check apps/web/tests/docs.acl --json` | Two scenarios admitted |
| `a3s-test run apps/web/tests/docs.acl --browser-driver standalone --json` | 2 scenarios, 33 steps passed; [report](browser-report.json) |
| `git diff --check` scoped to changed existing website files and workflow | Passed |

The Node proofs cover catalog traversal/duplicate/unknown-field refusal, adding another project identity, deterministic generation, stale HTML detection, active navigation, headings, accessible SVG labels, no runtime scripts and local links under the `/aster/` project prefix. Python also serves all 25 staged files under `/aster/`, refuses MDX/catalog source requests with 404, and checks injected behavior, remote assets and missing fragments.

Browser evidence uses standalone `agent-browser 0.26.0`, protocol 15. Desktop is 1440 × 1000; mobile is 390 × 844. Both scenarios retained empty page-error arrays: [desktop](desktop-errors.json), [mobile](mobile-errors.json). Interactive inspection additionally verified keyboard panning in an overflowing diagram and finished its owned session with no cleanup error.

- [Documentation portal](screenshots/portal-desktop.png)
- [Execution diagram, desktop](screenshots/workflow-desktop.png)
- [Execution diagram, mobile](screenshots/workflow-mobile.png)
- [Framework comparison, mobile](screenshots/comparison-mobile.png)

## Findings resolved

The initial mobile regression could scroll toward a distant next-page link without activating it. It reproduced in the isolated mobile scenario. Removing optional global smooth scrolling made the unchanged navigation assertions pass; no arbitrary sleep or retry was added. The initial Python refusal fixture also expected an older relative homepage URL; correcting the fixture proved the same path-escape refusal without changing that boundary.

## Explicitly not claimed

- No live deployment, GitHub Actions run, external download availability or remote source-link availability was verified.
- No cross-framework performance benchmark or new claim about A3S Code runtime qualification is made.
- No Rust, desktop, API or database implementation changed or was tested. Existing unrelated working-tree edits and merge conflicts were preserved.
- No full-text search, translated documentation, historical-version switcher or fabricated pages for future projects were added.
