# Implementation preflight

Scope: existing `specs/031-cli-mise-replacement`, current checkout, 2026-11-03.

| Finding | Severity | Location / tasks | Resolution |
| --- | --- | --- | --- |
| All FR-001–014 and 24 capability families have producing tasks and final gates | note | spec/plan/tasks, T001–T014 | Preserve frozen inputs and unchecked work |
| Required 35+14 inventory is documented but not reconciled in one machine-readable ledger | warning | M0, T008/T012/T013 | Collect exact native package identities and source hashes; retain separate implementation, verification and publication states |
| Ordinary plugin jobs survive renderer dismissal but only exist in memory | warning | M1, T008, capabilities_registry_jobs.rs | Reproduce host restart; persist tenant-owned jobs and reconcile against actual installed publications |
| Frozen prune reference is present but actual Aster rejects the command | warning | M2, T004 | Implement shared tracking/selection/removal and run unchanged reference comparisons |
| Native GUI, platform and final-archive checks remain required | warning | M1/M8, T012/T014 | Record prerequisites separately; never infer acceptance from unit tests or compilation |
| No new workspace, non-ACL configuration, native picker or ratchet expansion is planned | note | all changed paths | Reuse owning stores, atomic-file patterns and existing supported-feature checks |

Verdict: ready for implementation in bounded slices. No constitution blocker was
found in the plan. Implementation and release completion remain unclaimed.
