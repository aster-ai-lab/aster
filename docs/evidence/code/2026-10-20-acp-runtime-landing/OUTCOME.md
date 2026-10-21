# Land the ACP long-task and tool-effect budget repairs

Date: 2026-10-20. User authorized commit, merge and push. The implementation
was frozen in an in-repository worktree at base `f35ab79dd`; the shared working
tree contains other sessions' active edits.

## Result and scope

The staged implementation removes implicit interactive ACP tool/transition
count ceilings while preserving explicit ACL, SDK, parent and role limits,
unattended limits, cancellation, timeouts and duplicate-call guards. The Core
SDK default remains 50; ACP explicitly selects no count ceiling.

Typed path/byte budget observations no longer become uncertain invocation debt
solely because their path list will not fit. Failed reads and unknown execution
outcomes still fail closed. Run-level status and non-git walk caps still close
the completion gate. Existing receipt data without the new flag defaults false.

The two Work fixture files are committed and pushed as
`33edc9a74f4042efca73157d815dfca139a53140`; the parent gitlink includes that
commit. Their bytes match the earlier frozen R5 qualification. The three
production ACP files also match that source freeze. This landing adds test-only
`cfg(test)` headers and formats the touched ACP test module.

## Fresh checks

Exact commands, exit codes and logs are in [checks-isolated.json](checks-isolated.json).
These checks passed: 686 tests, zero failures, two existing ignores. All 58
parent Rust files and the two Work Rust files passed their owned format checks.
The [source comparison](isolated-source-after.json) confirms no source changed
during qualification.

| Check | Result |
| --- | --- |
| `effect` | test result: ok. 76 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 1.49s / test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s / test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s / test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s / test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s / test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s / test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.51s / test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s / test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.58s / test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s |
| `core-connected` | test result: ok. 526 passed; 0 failed; 1 ignored; 0 measured; 4708 filtered out; finished in 66.84s |
| `capability` | test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s |

The initial shared-tree `--locked` attempts stopped before tests because an
independent terminal startup edit had removed dependencies while the lockfile
was stale. Those records are retained under `shared-tree-initial/`. A trial
whole-lock regeneration was discarded. No Cargo.lock change is staged; the
isolated checks use the base lockfile unchanged.

After the fresh checks above, an extra ACP run remained queued on the shared
build lock. It was interrupted before test assertions. The later seam, paired
binary build and Work fixture were not repeated. No cancelled or unstarted
check is counted as passing. The retained [R5 qualification](../2026-10-20-task-orchestration/no-count-limit/OUTCOME.md)
records the earlier ACP, paired build and real 64-read Work checks. The separate
[CUA assessment](../../desktop/2026-10-20-t01-deepseek-flash-cua/OUTCOME.md)
records the actual R5 task and its 8/9 independent score.

The workspace-wide checks remain red: formatting in unchanged files, and two
of six dependency checks (`apps/work/.git` exists; `a3s-gateway` has publish
metadata outside the expected contract). `pnpm lint` could not start because
pnpm is unavailable. No frontend code is part of this landing. See
[workspace results](workspace-checks-isolated.json).

## Preservation and signatures

Files were staged individually. Desktop delivery alignment, ACP package identity
and runtime-boundary work, terminal startup/refusal changes and later Core
receipt edits are excluded. Their shared-tree working contents are preserved.
The exact scope and the five overlapping Core paths are recorded in
[landing-context.json](landing-context.json).

The Work commit has a locally valid GPG signature and DCO signoff. GitHub
returned `verified: false`, `reason: unknown_key`; this is recorded separately
from local signature validity in [work-push-verification.json](work-push-verification.json).
No GitHub Verified claim is made.

## Explicitly not claimed

- A green whole-workspace CI result, a fresh latest paired binary build, or a
  replacement/launch of an installed desktop application.
- A new paid model run, VM run, task speed improvement, or repair of T01's
  negative-zero formatting failure.
- That every gate refusal is fixed, that a previously frozen ledger unfreezes,
  or that the later in-flight Core receipt edits have been included.
- GitHub Verified status, account signing changes, or validation of the skipped
  frontend lint.
