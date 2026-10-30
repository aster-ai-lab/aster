# Meta Harness documentation review

Date: 2026-10-26. Scope: the existing MDX documentation, catalog validation and documentation regression suite in `apps/web`. No Rust runtime implementation, commit or deployment was changed.

## Findings addressed

1. **Full-tree precedence was too broad.** The Assembler guide now distinguishes replacement of the component tree from numeric configuration already applied to `HarnessConfig`. `FactRun::admit_graph` chooses the Assembler, but the session path sets recipe `step_limit` and `model_attempts` before calling it.
2. **Directory scripts had no complete invocation example.** The guide now includes a JavaScript `run(ctx)` and a Rust caller using `governed_tool`. It checks nested JavaScript `exitCode` and outer Rust `exit_code`, distinguishes host-approved `tool()` from permission-governed calls, and explains that the directory is also the example's workspace.
3. **Directory arguments do not yet match the complete program schema.** `DirectoryScript::program_arguments()` supplies language, path and allowed tools. `ProgramTool::execute` also requires `type`. The caller now explicitly adds `type: "script"`. This is a documented compatibility step; the runtime helper was not changed.
4. **Cancellation guidance named no usable APIs.** The recovery guide now distinguishes cancellation requests, exact-ID cancellation, waiting for admission release, and retained terminal-state queries. It explains both settlement deadlines and why stream EOF is insufficient.
5. **The catalog accepted the same MDX file at multiple routes in one project.** Layout lookup uses that file to determine chapter position, so such entries could produce incorrect numbers and neighbors. Both JavaScript build validation and the Python publication contract now reject them, with regression assertions. Separate projects may still each have their own `index.mdx`.

The capability index links to the existing Agent setup, the context guide links directly to the script example, and the attestor guide clarifies which workspace to inspect when using an isolated worktree. All 51 authored references to this repository resolve to existing local files or directories; no broken-reference repair was needed.

## Validation

| Command / check | Result |
| --- | --- |
| `npm --prefix apps/web run build` | 13 MDX pages generated |
| `npm --prefix apps/web run check` | Generated HTML matches source |
| `node --test apps/web/tests/download.test.cjs apps/web/tests/canvas.test.cjs apps/web/tests/cosmic.test.cjs apps/web/tests/home.test.cjs apps/web/tests/docs.test.mjs` | 48 passed; [log](node-tests.log) |
| `python3 -m unittest discover -s apps/web/tests -p 'test_*.py' -q` | 19 passed; [log](python-tests.log) |
| `python3 apps/web/scripts/check_site.py` | 33 public files, 13 docs pages, 6 download links and 4 homepage tabs |
| `python3 apps/web/scripts/stage_site.py target/aster-site-docs-review-20261026` | Exact 33-file staging; [receipt](stage-receipt.json) |
| Extract all Rust fences and parse snippet wrappers with `rustfmt --edition 2021 --emit stdout` | 13 snippets parsed; syntax only |
| Execute the authored JavaScript example using Node `vm` with a mocked `ctx.tool` | Successful read returned; tool failure rejected |
| Resolve authored GitHub `blob/main` and `tree/main` paths against this checkout | 51 references exist locally |
| `a3s-test check apps/web/tests/docs.acl --json` | Three scenarios admitted |
| `a3s-test run apps/web/tests/docs.acl --browser-driver standalone --json` | 3 scenarios, 64 steps passed, no cleanup errors; [report](browser-report.json) |
| `git diff --check -- apps/web` | Passed |

Browser regression includes the new script section's table-of-contents jump. The interactive review inspected the updated script example at 390 × 844 and finished successfully: [session result](interactive-review.json), [mobile script screenshot](screenshots/directory-script-mobile.png). The regression retained the [desktop capability index](screenshots/meta-harness-desktop.png) and [mobile component guide](screenshots/meta-harness-mobile.png).

## Explicitly not claimed

Rust type-checking did not complete. Initializing `apps/studio` at the parent-pinned revision was refused because the existing checkout has local changes; those changes were preserved. The populated checkout was left on its existing revision. The root `cargo check --locked -p a3s-code-core --lib --message-format=json` then waited for another desktop build's shared target lock without producing any compiler artifacts; only this review's waiting Cargo process was stopped. [Captured diagnostic](cargo-check.stderr).

No current-checkout Rust compilation, live model or MCP execution, sandbox qualification, remote GitHub-link availability, production deployment, performance result or external-framework re-verification is claimed. The JavaScript check used mocked tools, not the QuickJS program runtime. Existing evidence directories were preserved.
