# Native approval recovery repair

## Current qualification (reopened goal)

The root's current-source owning approval group passes, and all connected
native groups pass on its frozen compiler artifact: 29 unique passed tests and
2 explicit external-evidence skips. See
[native-v2.json](../../goal-resumed-20261020/native-v2.json) and the deduplicated
[summary](../../goal-resumed-20261020/t029-summary.json). Current sources are
identified separately in `native-source-current.json`: the form fixture's
empty execution id was aligned with its actual `Execution`, and an unbounded
status receive gained a timeout. The initial hanging fixture process was
terminated and reaped; it was not counted as an assertion RED. This supersedes
the pending verification status below without deleting its original evidence.
Frontend revalidation is 182 passed with one existing conditional skip.
The controlled T029 gate is complete; desktop CUA and task speed remain separate.

## Original repair checkpoint

T029 changes the Work-owned live approval contract. This is a source repair
with two observed pre-fix regressions; the production change has **not yet
passed compilation or GREEN tests**. No real model, VM, CUA operation, user
session mutation, app replacement, or paid T01 retry was performed by this
native repair agent.

## Observed RED

Root ran this owning-package command before the production repair:

```sh
cargo test --locked --offline -p aster-dream --lib approval_recovery_tests -- --test-threads=1
```

- [RED-1.log](RED-1.log): 2 tests, 1 passed / 1 failed, exit 101;
  compilation 52.90 seconds, tests 0.66 seconds. Attach lacked a live receiver
  snapshot. The multithreaded abort case passed and is not counted as a
  reproduced abort failure.
- [RED-2.log](RED-2.log): 2 tests, 0 passed / 2 failed, exit 101;
  compilation 7.31 seconds, tests 0.60 seconds. The current-thread abort test
  performs synchronous abort then response without yielding; the existing
  implementation still accepted that answer. The second failure again showed
  the missing live receiver snapshot. The test bodies use `catch_unwind` and
  call the existing async runtime cleanup before resuming a panic.

## Source repair

- `LiveApproval` retains the original oneshot sender with its exact execution,
  native anchor, actual message, approval and tool-call identities. Attach
  returns `streamStatus`; normal `topic.status` events, including their
  existing atomic SSE replay, carry the same `pendingApprovals` identities.
  Closed receivers, aborted owners and bindings from another execution are
  excluded. Stored pending parts never create a sender.
- The existing approval route now requires an explicit session and all scoped
  identities. It rejects missing, null or unknown fields. The shared native
  responder checks every binding before removing its sender; a wrong answer
  cannot consume the valid receiver. The existing Rust responder remains
  available for internal callers and uses the same live/abort checks.
- Abort synchronously revokes approval senders. The permission wait gives
  abort priority and checks it again before using an answer. A request that
  observes cancellation after mode application refuses instead of registering
  or allowing an action. Actual terminal settlement is still published by the
  existing turn owner, rather than inferred from the abort acknowledgement.
- The status publication guard now checks both execution and anchor. A later
  retry can reuse the same message anchor, so checking the anchor alone could
  publish a retired execution over the new owner. This additional source
  finding has a regression assertion but has **not** been executed as RED or
  GREEN because package resolution became unavailable first.
- Captured repair permissions use `TurnStream::capture()`'s actual message id;
  their native execution and original anchor remain unchanged. The existing
  captured fixture now waits for authoritative attach status after checkpoint,
  checks that no approval chunk was emitted, verifies the distinct message and
  anchor identities, and responds to that same receiver. It explicitly closes
  test runtimes after settlement, including assertion failures through
  `catch_unwind` followed by cleanup before resuming the panic.

Independent frontend review found another source-level window: the live map
can revoke a receiver before the existing EventHub cache is refreshed. The
same live-lock-owned helper now refreshes that cached pending set after
response (including a closed receiver), abort, and permission-wait cleanup
(including client death). EventHub updates and publishes under its existing
observations → statuses lock order and only for the same cached execution;
the awaiting status remains unchanged. Current-thread tests capture the SSE
snapshot synchronously after response/abort and before polling turn settlement;
the closed-receiver test checks the same cache withdrawal. An EventHub test
checks that a retired execution cannot replace the newer cached owner. These
added regressions remain unexecuted under the build block.

The final native source list and SHA-256 values are in
[source-freeze.json](source-freeze.json). The six existing `LiveTurn` test
initializers were updated only to carry the real attempt field required by
the attach snapshot; no model budget, permission mode, sandbox, configuration
format or dependency was broadened.

## Current verification and commands to finish

Passed static checks on the twelve owned native source files:

Exact arguments, exit codes and empty outputs are retained in
[static-checks.json](static-checks.json) and [static-checks.log](static-checks.log).
All twelve frozen source hashes matched before and after both checks.

```sh
rustup run 1.97.1 rustfmt --edition 2024 --config skip_children=true --check <owned files>
git -C apps/work diff --check -- <owned paths relative to apps/work>
```

Root reported that the first production GREEN command could not select
`aster-dream` after an unrelated workspace migration removed Work from the
member list. Root restored only the Work member under the user's AGENTS
instructions. The subsequent build graph is blocked by a separate Flow
migration: root and Work refer to package `a3s-flow` at `crates/flow`, whose
current manifest declares `aster-flow-legacy`. This native agent did not edit
any manifest or lockfile. A package-resolution error is not a test failure or
a RED reproduction. Root's attempts are retained in
[build-blocker.json](build-blocker.json).

Once the owning build graph is restored, root must run the following filters
against current source with `-p aster-dream --lib` and `-- --test-threads=1`:

| Filter | Scope |
| --- | --- |
| `approval_recovery_tests` | 3 tests: live reconnect/SSE and single answer, wrong/closed/orphan receiver refusal, abort and actual paused settlement |
| `approval_response_tests` | 1 test: complete typed answer body and strict missing/null/unknown-field refusal |
| `workbench_repair_permission_is_visible_on_its_own_message_and_keeps_the_original_owner` | 1 existing fixture extended for captured recovery |
| `status_publication_requires_the_current_live_owner_and_withdraws_at_settlement` | 1 existing test extended for a retired execution at the same anchor |
| `synchronous_approval_withdrawal_cannot_replace_a_newer_cached_execution` | 1 new EventHub test for execution binding and nonterminal withdrawal |
| `question_recovery_tests`, `question_route_tests`, `turns_form_tests`, `execution_status_tests`, `message_delete_tests`, `scheduled_recovery_tests` | Existing affected owner and cancellation regressions |

Use the existing root lockfile. Root may need one offline resolution to restore
the existing Work member's removed lock entries; subsequent checks use
`--locked --offline`. No independent workspace or second lockfile is needed.

## Explicitly not claimed

This record does not claim production GREEN, a completed T029 gate, full
workspace CI, a repaired frozen CUA session, Windows/Linux execution, or any
task speedup. It does not establish the unique cause of the original missing
approval UI. Frontend verification is a separate owning result, and neither
frontend tests nor static source review substitutes for native compilation and
the required turn-settlement assertions.
