# Core / ACP handbook, version architecture and Markdown rendering

Date: 2026-10-26. Scope: `apps/web`, its static documentation output, and the website workflow's documentation inputs. Runtime API references correspond to the local Core and ACP 9.1.6 source files whose hashes are retained in [source-hashes.json](source-hashes.json).

## Result

- Product → version → chapter is now the catalog hierarchy. Code has the preserved 9.1.5 articles and current 9.1.6 handbook; ACP has an independent 9.1.6 handbook. Defaults are explicit. Version content, helpers and Pele sources live in separate directories. Tests also build independent two-product/two-version fixtures, including missing-page fallback and deletion of obsolete generated pages.
- Fixed-version canonical addresses, source references, breadcrumbs, adjacent pages and product/version switchers derive from the catalog. Legacy addresses render the explicit default version. Relative article links resolve from their pinned source location, including cross-product links when viewed through a legacy address.
- Source links and version status moved from the left navigation to the bottom of the desktop right rail. At narrow widths they follow the article in normal document flow. See [desktop](screenshots/interactive-code-version-desktop.png) and [mobile](screenshots/interactive-resources-mobile.png).
- Design philosophy and Recursive Self-Improvement chapters describe mutable execution policy, fixed authorization, evidence, completion and resource ownership. The RSI chapter separates supplied Core contracts from host-owned candidate evaluation and promotion. A scoped Pele cycle and primary research references accompany the explanation.
- The current Code handbook has 37 pages, including the twelve-chapter tutorial, Meta Harness guides and ten functional-reference pages. Its coverage index matches all 30 SDK capability IDs, canonical operations, tiers and host ownership flags in `sdk_capabilities.rs`.
- The independent ACP handbook has 13 pages. Its extension index matches all 23 accepted method names/prefix families across 17 HostMethod branches. It documents standard lifecycle, startup/configuration, models, permissions/questions/forms, replay and pagination, native plugins/MCP, Office/document bridging, memory/knowledge, goals/review and unattended execution. It explicitly distinguishes text-only ACP prompts from Core attachments, ACP memory CRUD from Core candidate activation, and the hybrid requirement of ACP workspace search.
- The final catalog contains 78 logical MDX documents, 128 public documentation routes, and 149 total staged public files. No client React runtime or documentation JavaScript was introduced.

## Markdown rendering correction

The screenshot reported by the user exposed a real gap: MDX's CommonMark parser did not parse GFM pipe tables. They were emitted as paragraph text. The existing custom React tables did not exercise this path.

Added pinned `remark-gfm` 4.0.1 following the [official MDX GFM guide](https://mdxjs.com/guides/gfm/). All 14 ordinary Markdown tables now render through the same accessible, focusable overflow container used by the site. Header scopes, alignment through CSS, table typography, blockquotes, task lists, long links and footnotes are handled without inline styles or browser scripts. Footnote labels are localized.

Regression tests compare each real document's parsed table count with its emitted semantic tables. A fixture additionally checks alignment, escaped pipes, inline code, emphasis/deletion, disabled task checkboxes, autolinks and footnote round trips. The exact reported table was reviewed at 1440×1000 and 390×844: [desktop](screenshots/interactive-markdown-table-desktop.png), [mobile](screenshots/interactive-markdown-table-mobile.png).

## Validation

| Check | Result |
| --- | --- |
| `npm --prefix apps/web ci --workspaces=false --ignore-scripts --no-audit --no-fund` | Clean isolated install passed; [log](install.log) |
| `npm --prefix apps/web run check` | 78 documents / 128 routes and generated CSS match; [log](generated-check.log) |
| `node --test apps/web/tests/*.test.cjs apps/web/tests/*.test.mjs` | 57 passed, 0 failed; [log](node-tests.log) |
| `python3 -m unittest discover -s apps/web/tests -p 'test_*.py'` | 19 passed, 0 failed; [log](python-tests.log) |
| `python3 apps/web/scripts/check_site.py` | 149 files, 128 documentation routes; [result](site-check.json) |
| `a3s-test check apps/web/tests/docs.acl --json` | Suite accepted |
| `a3s-test run apps/web/tests/docs.acl --browser-driver standalone --json` | 5 scenarios / 142 steps passed, no cleanup errors; [report](browser-report.json) |
| A3S Test interactive review | Product/version switching, fallback, footer placement, ACP, design/RSI and Markdown table checked; [report](interactive-review.json) |
| Extracted code and source references | 41 Rust snippets parsed with rustfmt; 8 statement fragments required a surrounding async function; 14 JSON blocks parsed; 106 repository references exist; [scope](source-checks.json) |
| `python3 apps/web/scripts/stage_site.py target/aster-site-handbook-20261026` | 149 files, all receipt hashes match final public sources; [receipt](stage-receipt.json) |
| `git diff --check -- apps/web .github/workflows/aster-pages.yml docs/evidence/web/2026-10-26-capability-handbook` | Passed |

The default installed A3S browser adapter is unsupported, so browser checks use the compatible standalone driver. An earlier interactive browser surface was lost; its unfinished review was closed and a new session performed the final review. Intermediate ACL failures were stale selectors assuming a final navigation group or an unpinned relative URL; selectors now match the versioned structure and the complete suite was rerun successfully.

## Explicitly not claimed

- Rust snippet parsing is not Rust type checking or execution. The complete tutorial-agent package was not compiled or run end to end.
- Coverage of the public contract inventory is not qualification of every provider, Cargo feature, external service or platform. No new live model, MCP, browser-search, S3, telemetry or performance campaign is claimed.
- The 9.1.5 text is preserved, but its runtime source links still use main; it is not a release-tag-pinned source archive. Current 9.1.6 references also describe a local working tree, and uncommitted runtime changes may not yet be on the remote main branch.
- The site was staged locally, not published. No commit, deployment, Studio source migration or unrelated worktree repair was performed.
