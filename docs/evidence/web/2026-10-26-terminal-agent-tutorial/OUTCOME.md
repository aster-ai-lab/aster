# Terminal-agent tutorial series

Date: 2026-10-26. Scope: the MDX documentation website, its diagrams, section navigation, publication contract and regression coverage. This continues the code-highlighting and Pele infrastructure recorded in [the visuals phase](../2026-10-26-docs-visuals/OUTCOME.md).

## Delivered

Thirteen new MDX pages form one route and twelve chapters under `/docs/code/tutorial/`. They follow the same reader-created Rust practice package, `apps/tutorial-agent`, using the root workspace and local SDK dependencies.

| Chapter | Concrete material |
| --- | --- |
| 01: Session and workspace | Cargo member/dependencies, argument parsing, UTF-8 path validation, trusted ACL loading, read-only permission policy, session creation and close-on-error path |
| 02: Streaming | `stream_with_run`, exact Run identity, tool preparation versus actual execution, worker settlement, retry and terminal-state interpretation |
| 03: Terminal UI | Ratatui/Crossterm dependencies, App reducer, bounded UTF-8 reply buffer, keyboard/event select loop, scroll controls and terminal restoration |
| 04: Permissions | PermissionPolicy, explicit ConfirmationManager and separate broadcast, pending-call refresh, exact-ID replies, timeout/expiry handling and approval keys |
| 05: Verification | Small failing-test exercise, trusted verification contract, digest/report matching and stale/failed-command exercises |
| 06: Persistence | Stable host-selected session identity, file SessionStore, explicit save, resume, exact cancellation and bounded settlement |
| 07: Harness | Compose recipe, tool versus transition limits, model attempts, compaction, and when to add a host component or Assembler |
| 08: Context | Static/dynamic context sources, Skills, MCP installation, visibility, failure modes and connection ownership |
| 09: Tools | Complete side-effect-free Rust Tool, closed argument schema, registration plus permission, directory scripts and governed invocation |
| 10: Memory | V1 versus V2, file repository, namespace binding, bounded recall, activation/admission/use and resume isolation |
| 11: Goals | Exact workspace service identity, WorkPolicy, goal creation, plan proposal/acceptance, one-tick streaming and finite host bounds |
| 12: Delivery | Resource lifetimes, cancellation/cleanup outcomes, reducer/UTF-8 tests and failure-oriented acceptance scenarios |

Two new Pele diagrams show the practice application's layers and its permission/confirmation path. Together with the reference pages there are five themed Pele SVG diagrams. Shiki highlights six authored fence languages (Rust, ACL, JavaScript, TOML, Bash and plaintext) with the tested high-contrast palette.

The catalog now has 26 pages. Tutorials and Meta Harness references are separate sections; tutorial cards use local chapter numbers 01–12, and navigation no longer repeats the project index next to an authored chapter number. Public routes of existing reference pages did not change. Exact publication now includes 47 files.

## Review corrections

- The SDK accepts strings for `Agent::new` and `session_async`; Path arguments now explicitly validate UTF-8 before conversion.
- A retained Agent/Runtime keeps ownership clear across the application's life.
- An injected confirmation manager's private broadcast is consumed explicitly; it is not assumed to merge into the model event stream.
- Approval is bound to the displayed tool ID; stale confirmation replies refresh the UI rather than being treated as consent.
- Plan acceptance references the proposal the user saw, and tick replay displays the existing result instead of spawning new work.
- Runtime, tool and memory examples preserve the existing options chain rather than silently replacing permissions or storage with fresh defaults.

## Validation

| Command / check | Result |
| --- | --- |
| `npm --prefix apps/web ci --workspaces=false --ignore-scripts --no-audit --no-fund` | Clean isolated install; [log](install.log) |
| `npm --prefix apps/web run build` / `run check` | 26 pages and generated stylesheet synchronized |
| Full website Node suite | 52 passed; [log](node-tests.log) |
| `python3 -m unittest discover -s apps/web/tests -p 'test_*.py' -q` | 19 passed; [log](python-tests.log) |
| `python3 apps/web/scripts/check_site.py` | 47 files, 26 documentation pages, original 6 download links and 4 homepage tabs |
| `python3 apps/web/scripts/stage_site.py target/aster-site-tutorial-final-20261026` | 47-file staging, all hashes match final public files; [receipt](stage-receipt.json) |
| Parse extracted Rust fences with `rustfmt --edition 2024 --emit stdout` | 39 snippets parsed, 0 syntax failures; [scope and counts](source-checks.json) |
| Resolve authored repository `blob/main` / `tree/main` paths against checkout | 68 references exist locally; remote availability not tested |
| `a3s-test check` / `run apps/web/tests/docs.acl --browser-driver standalone --json` | 4 scenarios, 105 steps passed, no cleanup errors; [report](browser-report.json) |
| `git diff --check -- apps/web .github/workflows/aster-pages.yml` | Passed |

Browser coverage visits all twelve chapters, the local tutorial cards, TOML/Rust highlighting, both tutorial diagrams and grouped mobile navigation. Interactive review reached the setup, streaming, TUI, confirmation and delivery chapters, and [finished successfully](interactive-review.json).

Screenshots: [route](screenshots/tutorial-desktop.png), [desktop TUI code](screenshots/tutorial-tui-code-desktop.png), [mobile confirmation chapter](screenshots/tutorial-permissions-mobile.png), [mobile delivery](screenshots/tutorial-delivery-mobile.png).

## Explicitly not claimed

The practice Rust package is instructional content for readers to create; no new workspace member, Cargo lockfile or shipped binary was added. Rust snippets were checked against source signatures and parsed, but the complete practice package was not compiled or executed. The instructions include integration steps and exercises, not a qualified production TUI implementation.

No live model, MCP service, filesystem sandbox, memory-backend campaign, external resource cleanup or real terminal session was exercised by these website tests. No production deployment or remote source-link verification. Chromium desktop/mobile website review does not establish Safari, Firefox, screen-reader or native-terminal compatibility. Existing evidence directories and unrelated repository changes were preserved.
