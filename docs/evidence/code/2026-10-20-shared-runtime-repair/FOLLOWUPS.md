# Broader validation follow-ups

These failures were found by expanding beyond the original 12 findings. They
are not claimed fixed. Their exact failing tests were run alone before being
classified. No production guard, test, or allowlist was weakened to hide them.

## Confirmed at the unchanged starting commit

A detached worktree at `d4a8393f12a6d526a4d08693ac76e73e87414b14`
reproduced all five selected library failures and the Git seam failure.
`git diff --exit-code -- crates/code/core crates/code/acp` was empty.
Work's tracked manifest, entry points, and delivery contract were restored
from its recorded submodule commit solely for workspace member discovery and
the existing include_str fixture. Work was not compiled by this comparison.
The worktree was then removed; logs are retained here.

| Follow-up | Exact contract that still fails | Next investigation |
| --- | --- | --- |
| F1 | document_read::run_tests::fresh_actual_runs_do_not_reuse_a_previous_pin_even_when_scope_addresses_repeat: three roots recorded where four are required. | DocumentBridge pin / Run instance ownership; determine whether an old pin is reused or the test's event accounting is stale. |
| F2 | plugin_mcp::tests::timed_out_native_mcp_call_revokes_readiness_and_reaps_before_retirement: zero model calls where two are required. | Native MCP readiness and the projected model capability catalog before timeout / retirement. |
| F3 | plugin_mcp::tests::native_skill_and_real_stdio_mcp_stay_frozen_and_ready_processes_retain_their_package: missing process PID log. | Native process activation contract at PluginSelectionHost::apply and the first Run. |
| F4 | plugin_mcp::tests::failed_later_mcp_preparation_keeps_earlier_process_bytes_until_observed_rollback: missing process PID log. | MCP preparation / rollback lease ownership and when startup is observable. |
| F5 | plugin_selection::tests::cancelled_actual_task_native_tool_settles_descendants_before_package_retirement: child never reaches the start fixture before its deadline. | Delegated native tool admission and cancellation ownership. |
| F6 | v912_readonly_git_parallel_commit_stays_ok: a foreign commit's guest.txt is charged to the read-only turn's completion gate. | Typed Git effect provenance and command / Run observation attribution. Do not waive the path or infer ownership from command text. |

Evidence: [baseline library run](logs/clean-head-acp-baseline.log),
[baseline Git seam](logs/clean-head-seam-baseline.log),
[current full library run](logs/repair-acp-full.log), and
[isolated current runs](logs/repair-acp-alone-results.json).
The older 07:15 binary comparison is only supporting history; the unchanged
starting-commit worktree is the baseline authority.

Four other failures from the parallel full-library campaign passed alone:
workspace replacement during MCP preparation, native harness cancellation,
tool-context descendant cancellation, and native MCP-call cancellation.
Their parallel instability remains unclaimed, with exact outcomes in the
isolated-run JSON. A passing isolated retry is not a claim that the full
suite is green.

## Repository baseline outside the product repair

- tests.workspace_dependencies.WorkspaceDependenciesTest.test_desktop_sources_and_executable_fixtures_are_present: apps/work/.git exists.
- tests.workspace_dependencies.WorkspaceDependenciesTest.test_internal_dependencies_are_local_and_unpublishable: a3s-gateway publish is None.

Both were reproduced alone. Their source owners and existing guards remain
unchanged.
