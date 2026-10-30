# Sidebar scroll retention and runnable tutorial examples

Date: 2026-10-26. Scope: `apps/web`, the root-workspace `apps/tutorial-agent` reference host, and the ACP interactive-client example. This record extends, and does not replace, the earlier [handbook evidence](../2026-10-26-capability-handbook/OUTCOME.md).

## Result

The documentation sidebar used ordinary full-page links. Each navigation recreated its independently scrolling chapter list at position zero. The generated pages now load one deferred, external script that saves the chapter-list offset before navigation and restores it on the destination page. State belongs to a browser tab and an explicit product/version pair. Hidden mobile navigation does not overwrite the desktop offset; unavailable storage leaves ordinary links usable. Main article scrolling remains under the browser's control. No React client runtime or intercepted navigation was added.

The fix was checked with exact offset assertions and a real browser's long navigation list. Compare the [menu before clicking](screenshots/sidebar-before-click.png) with the [destination menu](screenshots/sidebar-after-click.png). The selected chapter changes while the lower menu segment stays visible. The site validator permits only the exact local navigation-script path, with no inline script body or event attributes.

The terminal-agent tutorial now links to a complete root-workspace reference package, rather than leaving readers to assemble disconnected snippets. Its offline example uses temporary files and a fixed injected model client to exercise reading, an exact-ID permission decision, a separate business answer, and the terminal state of the original Run. Runtime tests cover selected approval/rejection, deny, invalid arguments, cancellation, workspace-bound persistence, and stale goal-plan revisions. Compiled chapter functions cover Harness composition, context, file memory and goal commands. Ordinary conversation explicitly disables automatic planning; goal options enable it deliberately.

ACP gained a client chapter and a runnable Rust example for streamed updates, permission choices, business answers, structured forms, cancellation, model switching and session loading. Two focused tests check option identity, the distinction between answers and tool approval, form shape, and pending-request cancellation. The example uses existing dependencies and leaves the public SDK unchanged.

The final site contains 79 logical MDX documents, 130 public documentation routes and 152 public files. Code 9.1.6 has 37 pages, preserved Code 9.1.5 has 27, ACP 9.1.6 has 14, and the portal has one. The package and documentation inputs are hashed in [source-hashes.json](source-hashes.json).

## Validation

| Command / check | Result |
| --- | --- |
| `npm --prefix apps/web run check` | 79 documents / 130 routes; generated output matches; [log](generated-check.log) |
| `node --test apps/web/tests/*.test.cjs apps/web/tests/*.test.mjs` | 59 passed; includes exact sidebar offsets, product/version isolation, mobile preservation, blocked/corrupt storage; [log](node-tests.log) |
| `python3 -m unittest discover -s apps/web/tests -p 'test_*.py'` | 19 passed; includes rejection of substituted/inline navigation scripts; [log](python-tests.log) |
| `node --check apps/web/public/docs/navigation.js` | Passed |
| `python3 apps/web/scripts/check_site.py` | 152 files / 130 documentation routes; [result](site-check.json) |
| `a3s-test run apps/web/tests/docs.acl --browser-driver standalone --json` | 6 scenarios / 148 steps passed; [report](browser-report.json) |
| A3S Test interactive sidebar review | Passed, cleanup succeeded; [report](sidebar-review.json) and before/after images above |
| `cargo check --locked --offline -p tutorial-agent --all-targets` | Passed during implementation; [log](tutorial-check.log); final source was subsequently compiled by the test run below |
| `CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 cargo test --locked --offline -j 4 -p tutorial-agent -- --nocapture` | 4 unit tests + 6 runtime integration tests passed; zero doctests; [log](tutorial-tests.log) |
| `cargo test --locked --offline -p a3s-code-acp --example interactive_client` | 2 passed; [log](acp-client-tests.log) |
| `a3s-test run apps/tutorial-agent/tests/tui.acl --tui-executable "$PWD/target/debug/examples/offline_tui" --tui-working-directory "$PWD" --tui-columns 120 --tui-rows 36 --json` | 1 scenario / 13 steps passed against the rebuilt offline example; [report](tui-report.json), [terminal recording](terminal/offline-flow.vt) |
| `/opt/homebrew/bin/python3.13 tests/workspace_dependencies.py` | 6 passed; [log](workspace-dependencies.log) |
| `cargo fmt -p tutorial-agent -- --check`; `rustfmt --edition 2021 --check crates/code/acp/examples/interactive_client.rs` | Both passed |
| `cargo fmt --all -- --check` | Blocked by formatting differences in 19 existing, unrelated files; [path-only summary](workspace-format-summary.json); private Studio source diffs were not copied |
| `python3 apps/web/scripts/stage_site.py target/aster-site-runnable-docs-20261026` | 152 staged files; all hashes match final public sources; [receipt](stage-receipt.json) |
| Focused `git diff --check` | Passed |

The terminal driver does not accept the named F6/F8/F9 keys. The host therefore offers documented Alt-key equivalents, which the native regression uses. Earlier tests exposed two real tutorial defects: a private import path and default automatic planning consuming a scripted conversation response. The import now uses the public reexport, and conversation/goal planning choices are explicit. Each initially failing stream/persistence case passed alone before the complete final run passed.

Several owned incremental Rust builds stopped making visible progress and were interrupted. The final tutorial run completed with incremental compilation and debug information disabled, as recorded above; this is not a claim that the default incremental-build delay was diagnosed. ACP's final default-profile test also completed. Earlier incomplete or failing runs are not counted as passes.

## Explicitly not claimed

- Offline scripted interaction proves host wiring and terminal behavior, not real-model coding quality. Live providers, external MCP/services, platform sandbox enforcement, remote memory and performance were not qualified by this change.
- The ACP example compiled and passed its reply-handling tests; a complete client-to-companion round trip against a real provider was not performed.
- The small TUI deliberately does not render structured forms; it reports that limitation and points to the ACP client. Chapter helper functions are compiled integrations, not features silently enabled in the basic host.
- With JavaScript or session storage disabled, documentation remains readable and navigable but sidebar scroll retention is unavailable.
- The site was staged locally, not deployed. No commit, private Studio source migration, or repair of unrelated worktree changes is claimed. Runtime source links still describe the local working tree and may precede remote main.
