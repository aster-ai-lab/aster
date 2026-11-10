# Restore a session's chosen effort

The real fullscreen TUI reopened a DeepSeek session last switched to `high`
with the supplied ACL still set to `max`. ACP returned `max`, and the actual
terminal footer displayed `max`. The load acknowledged the original session
and admitted no new prompt. See [before-plan.json](before-plan.json) and
[before-result.json](before-result.json).

The TUI previously saved the successful selection as a global preference but
did not record it in the session summary. A cold ACP load could only use the
current launch preference. The TUI now records an acknowledged selection in its
existing atomic summary update, using the target session's workspace. The
existing summary carries an explicit reset separately from a missing record
and the `none` level. Copied summaries retain this distinction.

Cold load supplies the saved choice with its model identity. ACP only applies
it to the matching model, using the refreshed supported levels. A live
session, an explicit launch effort and unattended launch rules retain their
precedence. Old summaries without a choice still use launch defaults. The
change uses the existing model assembly path and the provider's current
capabilities.

## Verification

The focused tests ran from the Aster root on macOS arm64 with Rust 1.97.1,
the shared root target directory and incremental compilation disabled.

- ACP resume: **10 passed**, including six saved-effort, support and precedence
  combinations. ACP model discovery and native switch fields: **2 passed**
- Shell summary storage, round trip and copy: **3 passed**
- Pager ACL model preferences: **2 passed**
- All-target Clippy passed for ACP, Shell and Pager with existing warnings
- The paired binaries matched current source fingerprint
  `87f313b79042fb5ee64806794fc7b2300fbdd298` on macOS arm64
- Workspace formatting reports the same 18 unrelated files as the preserved
  baseline. The owned diff passed `git diff --check`

The initial compile attempts missed one function qualification and three
summary initializers. These attempts executed no tests in the failing targets.
An initial parallel ACP resume run passed nine tests and failed one existing
test while restoring a temporary directory (`DirectoryNotEmpty`). The same
test passed alone and the full resume group passed on rerun. The failing
assertion remains in place. One exact-name list probe selected zero tests.
The later correctly named invocation executed and passed. Commands, counts and
log digests are in [checks.json](checks.json) and
[artifact-digests.json](artifact-digests.json).

## Real DeepSeek restart

The source-matched pair ran the frozen [restart plan](restart-plan.json) on a
fullscreen PTY using private copies of the supplied ACL and session state.
The ACL digest matched before and after the run.

- The legacy summary loaded with the configured `max` baseline.
- Three acknowledged selections, `high`, `default` and `none`, were recorded
  in the same session summary. After restart, ACP restored each selection and
  reported the 1048576-token context window. The footer displayed `high` for
  the provider default and `none` for explicit off.
- The explicit launch argument `--reasoning-effort low` took precedence over
  the saved `none` in the load response and footer.
- The final launch used saved `none` and retained all five historical Run
  identities. A single new prompt completed with one original successful read
  receipt and the exact fresh canary in the final answer and terminal viewport.

The [restart result](restart-result.json),
[model and prompt trace](selection-trace.json) and
[source reconciliation](source-reconciliation.json) retain the scope of the run.

## Explicitly not claimed

The saved-effort evidence covers the same model and selections recorded by the
fixed build. Cross-model recovery, hidden provider compute, the full catalogue,
and Windows/Linux runtime remain outside this evidence. The native GUI context
and timeout replays still require an available desktop. Agent-desktop observed
wallpaper and no Terminal window while macOS reported a locked console.
See [native availability](native-availability.json).

The diagnostic candidate contains the separate uncommitted ACP cancellation
work. That patch is outside this commit. Release awaits the clean candidate.
