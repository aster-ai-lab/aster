# Current workspace check gaps

Read-only audit of the current campaign's completed check logs. No source,
manifest, test or user state was changed and no Cargo command was run by this
audit. Full workspace CI remains failing; the failures below are retained
separately from the scoped optimization regressions.

## Formatting

`cargo fmt --all -- --check` finished with exit 1 in 32.392 s. The
[current log](workspace-fmt.log) contains differences in 61 distinct paths.
All 61 paths also appeared in the earlier
[workspace formatting log](../workspace-fmt.log); no new path appeared, and
14 of that earlier log's 75 paths are absent from the current differences.

The intersection with the current 43 owned source paths is empty. The
[final scoped check](scoped-format-final.json) covers exactly those 43 paths
and both edition groups returned exit 0. At this audit's pre-build checkpoint,
all hashes matched the [before-build source manifest](owned-source-before-build.json).
The later [post-build guard](owned-source-after-paired-build.json) records
semantic `porcelain.rs` drift, leaving 42/43 matching owned sources. The earlier
scoped check is not a formatting or behavior qualification of that later source
revision. Neither checkpoint establishes a passing full-workspace check.

Command metadata: [workspace-fmt.json](workspace-fmt.json).

## Workspace dependencies

The initial `python3 tests/workspace_dependencies.py` attempt could not import
`tomllib`; see [its log](workspace-dependencies.log). Root reran the check with
Python 3.13. That command finished with exit 1 in 11.975 s, with two failures:

| Failing assertion | Observed state |
| --- | --- |
| Desktop sources must share the root repository | `apps/work/.git` exists |
| Every workspace package must be unpublishable | `a3s-gateway` reports `publish=None`; `apps/gateway/Cargo.toml` does not declare `publish=false` |

Both failures are also present in the earlier
[dependency-check log](../workspace-dependencies.log), recorded before this
campaign. Neither condition is a change in the 43 owned source files. This
audit does not infer which parallel migration owns their resolution.

Current evidence: [Python 3.13 log](workspace-dependencies-py313.log) and
[command result](workspace-dependencies-py313.json). Source/manifests and the
nested repository were not changed to make this check pass.

## Remaining acceptance boundaries

The controlled T028/T029/T005 regressions and scoped formatting do not replace
the final paired product build, desktop launch, live approval controls or the
new T01 assessment. Their status and timing require their own root-owned
evidence; no task-speed improvement is asserted by this audit.
