# Aster documentation publication candidate

Date: 2026-10-26. Target: `aster-ai-lab/aster` main and its existing GitHub Pages workflow. Base: `f9bddc250b977a3d6eb311ffc7d2f8b36df6b95d`.

## Result

The publication was assembled in an isolated worktree based on the latest remote main. The original checkout has unrelated staged/unstaged edits and an unresolved checkout-action merge; none of that state was reset, staged or merged into this candidate. Studio and Power were checked out at their exact parent gitlinks through local detached worktrees. Their sources and gitlinks are not part of the change.

The candidate contains the product/version documentation architecture, 79 MDX documents, 130 static documentation routes, Markdown rendering and highlighting, themed Pele figures, sidebar scroll retention, the tutorial reference host and the ACP interactive client. The only Core prerequisite is the isolated-context SessionOptions flag and its context/skill/memory wiring, with a regression case. Cargo membership and the lockfile add only the tutorial package; unrelated local dependency and application-version edits were left behind.

The clean-source coverage check caught a release mismatch: five ACP methods described by the local draft were not accepted by remote main. The published ACP index now matches all 18 accepted method names/prefix families across 12 branches. Session-history/tooling/search documentation explicitly distinguishes those unavailable extensions from the existing ACP load/replay and tool events. The memory envelope is corrected to 20 KiB, and unimplemented knowledge-compilation metadata is no longer presented as an authorization contract. All 114 distinct repository source paths in MDX resolve in this candidate; see [source-links.json](source-links.json).

## Validation

Validation commands ran from the isolated candidate worktree. Website inputs and the exact deployment file set are represented by [stage-receipt.json](stage-receipt.json).

| Check | Result |
| --- | --- |
| `npm --prefix apps/web ci --workspaces=false --ignore-scripts --no-audit --no-fund` | Passed; [log](npm-install.log) |
| `npm --prefix apps/web run build`; `npm --prefix apps/web run check` | 79 documents / 130 routes; [build](build.log), [check](generated-check.log) |
| `JS_DOM_MODULE=<installed jsdom> node --test apps/web/tests/*.test.cjs apps/web/tests/*.test.mjs` | 59 passed; [log](node-tests.log) |
| `python3 -m unittest discover -s apps/web/tests -p 'test_*.py'` | 19 passed; [log](python-tests.log) |
| `python3 apps/web/scripts/check_site.py` | 152 files / 130 documentation routes; [result](site-check.json) |
| `a3s-test run <port-adjusted docs.acl> --browser-driver standalone --json` | 6 scenarios / 148 steps passed; only the local origin changed from port 4192 to 4193; [input](browser.acl), [report](browser-report.json) |
| `python3 apps/web/scripts/stage_site.py <fresh destination>` | 152 approved public files; [receipt](stage-receipt.json) |
| `/opt/homebrew/bin/python3.13 tests/workspace_dependencies.py` | 6 passed; [log](workspace-dependencies.log) |
| Changed Rust files and tutorial package formatting | Passed |
| `cargo test --locked --offline -j 4 -p tutorial-agent -- --nocapture` | Core and tutorial compiled; 4 UI/argument tests and 6 runtime tests passed; zero doctests; [log](tutorial-tests-at-submission.log) |
| `cargo fmt --all -- --check` | Three existing Gateway files differ from rustfmt; [path-only summary](workspace-format-summary.json); those files are unchanged in this candidate |

Rust runs use the pinned toolchain, offline locked resolution, four jobs, incremental compilation disabled and dev debug information disabled. On this nested macOS worktree, encoded Rust flags are the repository's single `-C force-unwind-tables=yes -C link-arg=-ObjC` sequence, avoiding repeated ancestor/worktree configuration flags. Interrupted preliminary builds are not counted as passes. The isolated-context, ACP and native TUI results are recorded separately after submission; the prior local tests are not a claim that those candidate checks already completed. Text-log trailing blank lines are normalized for the repository whitespace check.

## Publication

Content commit `e011dc3ab113663bf929eb80a5097178993cbc4d` was fast-forwarded to remote main. The existing `aster-pages.yml` workflow completed both validate and deploy successfully: [deployment run](https://github.com/aster-ai-lab/aster/actions/runs/40332112120), [structured result](pages-result.json). No force push or Pages-settings change was used.

The published [documentation site](https://aster-ai-lab.github.io/aster/docs/) was checked over HTTPS. Nine selected pages/assets, including both Code versions, ACP references/client/session pages, styles and the navigation script, match candidate file hashes exactly; see [live-check.json](live-check.json). The follow-up evidence commit changes no deployment files.

## Checks completed after submission

- Isolated-context Core regression: 1 passed, 5,455 filtered out; [log](core-isolation-tests.log).
- ACP interactive-client example: 2 passed; [log](acp-client-tests.log).
- Rebuilt offline terminal host: 1 scenario / 13 steps passed; [report](tui-report.json), [terminal recording](terminal/offline-flow.vt).
- The complete tutorial package's 4 unit and 6 runtime tests had already passed before the content commit, as recorded above.

## Explicitly not claimed

- Earlier evidence directories retain their own local working-tree scope, including the earlier 23-method ACP draft. Their results are not substituted for the clean-main checks in this record.
- Real-provider coding quality, external service/platform qualification and a complete ACP client/companion/provider round trip remain unverified.
- The pre-existing Gateway formatting differences and the original checkout's unrelated edits/merge state are not repaired by this publication.
- No private Studio sources, user state, other tenant files, MDX sources or dependencies are uploaded to GitHub Pages. A successful Pages deployment is not a claim that all other repository CI jobs passed.
