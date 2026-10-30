# Documentation diagrams and source navigation

Date: 2026-10-28. Scope: the public documentation site. The release candidate
integrates this slice with upstream `1be3493a334a1feeb9e10a22ac1da3e5842985b7`
in the repository-local `.tmp/docs-source-navigation-release-20261028` worktree.
Other sessions' product changes are excluded.

## Delivered

- Eight authored Pele diagrams cover context assembly, Run control, completion
  evidence, durable memory, ACP process boundaries, protocol lifecycle,
  permission round trips and event routing. They use the existing static SVG
  renderer, accessible descriptions and horizontal scrolling.
- Nine current Core/ACP reading paths contain 48 inline implementation
  citations. Each citation links to a GitHub source line and contributes to an
  ordered, deduplicated page source list. The list appears in the right column
  on desktop and after the article on narrow screens.
- `SourceRef` reads the version's Git object, finds the author's exact source
  fragment and computes its line range. Missing files, missing fragments and
  ambiguous fragments refuse the build. No symbol-name guessing, new runtime
  dependency or browser JavaScript was added.
- Core/ACP 9.1.6 source references are pinned to public commit
  `8729d7c84a2d887c61dc206ba47bf8a63425e768`. The Pages workflow fetches Git
  history so the build can resolve that snapshot. Archived 9.1.5 MDX remains
  unchanged.
- The existing generator rebuilt 79 documents at 130 routes. Generated files
  were not hand-edited. Both product overviews now map reading questions to
  implementation entry points.

## Validation

| Check | Result |
| --- | --- |
| `npm --prefix apps/web run build` and `run check` | 79 documents, 130 routes, deterministic generated CSS; [build](build.log), [check](check.log) |
| Full website Node suite before upstream integration | 61 passed; [log](node-tests.log) |
| Full website Node suite on the merged candidate | 65 passed, including upstream's new homepage cases; [log](merged-node-tests.log) |
| `python -X utf8 -m unittest discover -s apps/web/tests -p 'test_*.py' -v` | 19 passed; [log](python-tests.log) |
| `python -X utf8 apps/web/scripts/check_site.py` on the merged candidate | 152 allowed public files, 130 documentation routes; [log](merged-site-check.log) |
| Generated-document comparison after upstream integration | All 130 HTML files and generated CSS match the previously validated bytes |
| Source-target audit | 150 distinct fixed-revision source targets exist; line ranges resolve in 15 cited source files across 51 current documents; [receipt](source-audit.json) |
| Existing static staging workflow before upstream integration | Exactly 152 files; all staged hashes equal their source and receipt hashes; [receipt](stage.log), [reopened-file verification](stage-verification.json) |
| `git diff --cached --check` in the isolated worktree | Passed; individually named staging excludes unrelated product files |

The Python suite ran in the original worktree. Its test file, checker and
staging script are identical in the merged candidate. The candidate's website
Node suite and static checker were rerun after integrating upstream.

The first Windows Node run exposed CRLF normalization in HTML code text;
restoring the MDX working copies to repository LF preserved all content and
the original assertions passed. The first Python run used the system GBK
default and failed three UTF-8 reads; rerunning with `-X utf8` passed all 19.
No tests or expected results were relaxed for these environment differences.

Adversarial checks exercise missing/ambiguous source fragments, invalid source
paths, multiline anchors, source-list deduplication, per-page isolation,
canonical/alias equality and static SVG accessibility. The full website suite
also rechecks navigation, no-JavaScript reading, code preservation and tables.

## Explicitly not claimed

- Full DeepWiki feature parity or inline citations on every documentation page.
- Real-browser pixel, viewport or interaction acceptance. Computer-use inventory
  returned no browsers and the in-app browser was unavailable; no screenshot or
  browser-suite result is represented as passing.
- Rust runtime changes, Rust test coverage, external-provider validation or
  deployment completion at the time this evidence was committed. Deployment
  is tracked by the subsequent GitHub Pages Actions run.
