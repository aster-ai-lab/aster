# CLI help and diagnostic branding

The shipped binary exposed legacy labels in workspace help, the disabled
Dashboard error, the invalid worker-count notice and setup guidance without a
principal. The old build failed all four checks in the frozen
[plan](plan.json). The raw output is retained losslessly in
[before-output.json](before-output.json).

Workspace help and the unknown-gate error now name `ASTER_WORKSPACE_COMMAND`.
The existing environment alias remains readable, with the Aster name taking
precedence when both are set. The Dashboard error reports the disabled state
directly. Worker notices describe the worker count and its adjustment. Setup
keeps the current deployment-key instruction.

The same four real CLI commands passed with their original exit statuses.
Dashboard used a PTY to reach its startup gate. The disk-usage command still
returned JSON while reporting the invalid worker count. The isolated setup
case had no deployment principal. See [after.json](after.json) and the
lossless [after output](after-output.json).

The 11 focused CLI tests and 32 Pager CLI tests passed. All-target Clippy for
both packages passed after restoration of the existing
[environment test fixtures](../2026-11-11-cli-env-fixtures/OUTCOME.md). The
first Clippy failure remains in [checks.json](checks.json) with the exact
commands, counts and digests. Workspace formatting reports the same 18
unrelated baseline paths. The owned diff passes `git diff --check`.

The CLI and ACP built together on macOS arm64 with Rust 1.97.1 and the shared
root target directory. The independent
[source reconciliation](source-reconciliation.json) matches fingerprint
`333b719ba6663e760471acdde304e7e4aef4cb54`.

## Explicitly not claimed

The evidence covers these four user-visible paths and the tested workspace
setting precedence. Other visible surfaces, native window interactions and
other platforms remain outside this run. The diagnostic pair contains the
separate uncommitted ACP work. Release awaits the clean candidate.
