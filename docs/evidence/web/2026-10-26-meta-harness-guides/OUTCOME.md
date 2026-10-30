# Meta Harness capability guides

Date: 2026-10-26. Scope: documentation and the public website's section navigation. No Rust runtime implementation, user configuration, commit or deployment was changed.

## Coverage added

Eight MDX pages under `/docs/code/meta-harness/` document the following verified source surfaces:

| Guide | Implementation read |
| --- | --- |
| Capability index | `meta_harness.rs`, `agent_api/session_options.rs`, `manual/META_HARNESS.md` |
| Components, budgets and compaction | `meta_harness.rs`, `a3s-effect` `coding.rs` / `compose.rs`, `fact_control.rs`, ACP `harness_assemble.rs` |
| Host components and full trees | `HostHarnessRegistry`, `HostHarnessAssembler`, `admit_component_tree`, component fold and transition-key selection in `a3s-effect` |
| Context, tools and Hooks | Prompt slots, `ContextProvider`, `StaticContextProvider`, skills/MCP/dynamic tools, `ProgramExecutor`, Hook contract and events |
| Agent directories | `DirectoryProjection`, ACL script/schedule declarations, program arguments, UTC due query and host deduplication |
| Persistence and recovery | Session store, async session resume, run checkpoint resume, exact run IDs, checkpoint export and unresolved tool effects |
| Verification and resources | `CompletionAttestor`, module-level `DiskContentAttestor`, Turn/Run cleanup ownership and completion settlement |
| Work policy and durable goals | `WorkPolicyV1`, typed goal facade, same-log execution and current `HOST_GOAL_CONTROL.md` continuation bounds |

The four introductory articles remain separate. The project catalog now accepts an optional nonempty `section` field, and grouped navigation and section chapter cards use it. Source paths and public nested URLs remain explicit. Introductory cards do not expand into the whole reference manual.

## Important distinctions retained

- Component order is assembly order; runnable transitions are chosen using log state and sorted keys.
- Removing the budget or compaction view slot does not itself remove scheduler limits.
- The current `compact_after_chars` accounting uses UTF-8 byte lengths. Token compaction requires reported prompt-token data.
- Host registries and assemblers are Rust interfaces; ACP accepts only its three exact current recipes.
- Directory schedules do not start a daemon or mark a UTC slot consumed. The host must deduplicate and submit prompts.
- Attestations and content checks do not prove business behavior. Unknown external effects still block successful completion.
- The current host continuation defaults were taken from `HOST_GOAL_CONTROL.md`, including actual-mutation progress and unattended bounds, rather than the older progress wording in the engineering overview.

## Validation

| Command / check | Result |
| --- | --- |
| `npm --prefix apps/web run build` | 13 MDX pages generated |
| `npm --prefix apps/web run check` | All generated HTML matches source |
| Website Node tests, including section/schema and nested-link tests | 48 passed, 0 failed, 0 skipped; [log](node-tests.log) |
| `python3 -m unittest discover -s apps/web/tests -p 'test_*.py' -q` | 19 passed; [log](python-tests.log) |
| `python3 apps/web/scripts/check_site.py` | 33 public files, 13 docs pages, original 6 download links and 4 homepage tabs |
| `python3 apps/web/scripts/stage_site.py target/aster-site-meta-harness-final-20261026` | 33 checked public files; [receipt](stage-receipt.json) |
| `rustfmt --edition 2021 --emit stdout` on extracted Rust fences, wrapped as snippet bodies / a trait signature | 11 snippets parsed successfully; syntax only |
| `a3s-test check apps/web/tests/docs.acl --json` | Three scenarios admitted |
| `a3s-test run apps/web/tests/docs.acl --browser-driver standalone --json` | 3 scenarios, 62 steps passed; [report](browser-report.json) |
| `git diff --check -- apps/web` | Passed |

Browser review used standalone Chromium through A3S Test at 1440 × 1000 and 390 × 844. It exercised the capability-index link, all seven topic headings, active section navigation, mobile directory expansion and code-example visibility. The Meta Harness scenario retained [no page errors](evidence/meta-errors.json). The owned interactive session also finished [without cleanup error](interactive-review.json).

Screenshots: [desktop capability index](screenshots/meta-harness-desktop.png), [mobile extension guide](screenshots/meta-harness-mobile.png).

## Explicitly not claimed

Rust examples were checked against the source interfaces and parsed, but were not type-checked or executed against a live model, MCP service or sandbox. No Rust runtime qualification, performance result, cross-framework comparison result, remote source-link availability or live deployment is claimed. Memory and workspace-retrieval algorithms still link to their dedicated repository manuals; these new pages document their integration points.
