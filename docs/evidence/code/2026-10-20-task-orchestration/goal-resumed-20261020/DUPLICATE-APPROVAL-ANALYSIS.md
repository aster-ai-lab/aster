# Sequential duplicate approval: read-only analysis

The root observed successful escalated Bash calls requiring two successive
approvals during the current R2 assessment. This investigation read source
only. No approval policy, source, running app, task or model submission was
changed; no new reproduction or optimization was executed. The actual
assessment retains its original approvals and timing.

## Source explanation

| Stage | Existing behavior |
| --- | --- |
| Outer fact approval | `fact_control/tool_results.rs::confirmation_parked` sets `needs_confirmation` when the frozen permission policy asks. `conversation_runtime/recovery_support.rs::await_host_confirmation` requests the answer, and `run.confirm` appends its fact before execution resumes. |
| Prepared invocation | `fact_control/receipts.rs::prepare_tools` resolves hooks once, checks original/effective argument digests, binds Run/thread/capability admission and call identity, and creates a continuation guarded against a second execution. |
| Inner tool-owned approval | `fact_control/executor_tools.rs::run` independently checks `registry.requires_confirmation` and calls `ToolConfirmationRuntime::resolve_with_cancellation`. It does not consume an outer approval proof. |

`BashTool::requires_confirmation` is true for both `require_escalated` and
`request_network_grant`. Therefore policy Ask plus tool-owned escalation can
produce two actual, sequential requests for the same call and arguments.
This source path explains the observed pair; hiding a repeated UI row would
not settle either real receiver. Policy Allow plus escalation currently has
only the inner request, whose mandatory gate must remain.

The current `needs_confirmation` field denotes a request requirement, not a
standalone consent credential. `pre_tool_hook_resolved` denotes hook state.
The persisted confirmation fact carries a call id and boolean, not a reusable
live Run grant. No existing independent, exact-invocation approval credential
is currently passed to the inner escalation gate. The idempotency identity
also intentionally omits the delivery id and must not be repurposed as consent.

## Smallest safe follow-up, not implemented

Reuse the existing settled-confirmation and prepared-invocation evidence chain
to carry one private, single-use approval proof into that invocation's own
escalation gate. Mint it only on the current owned Run's validated settlement
path; historical facts or pending rows alone must not recreate it. Bind and
verify the live Run instance, capability binding, turn/cycle/call identity,
tool name, canonical effective argument digest, actual cwd and execution
boundary. The prepared continuation's existing one-use guard remains in force.

Only a matching proof may suppress the second request. Absence or mismatch
keeps the existing gate/refusal. In particular, Allow plus escalation still
requires an explicit tool-owned approval. The first prompt must clearly cover
the requested host execution or exact network origin before its approval can
serve both requirements. Command text resemblance, a success flag, read-only
classification, command caching, allowlists or blanket run approval cannot
replace that scope check.

Keep the proof local to the prepared call rather than a shared `ToolContext`
that `program`, `batch` or delegated children could inherit. Preserve the
current hook-change refusal: approval of original arguments never authorizes
different resolved arguments. Cwd/runner/boundary drift also invalidates reuse.
Keep budgets, cancellation, execution availability, receipts, resource
settlement and host receiver revocation unchanged.

## Required controlled regressions

- First reproduce two provider requests with one controlled tool declaring
  `requires_confirmation=true` under Ask; then require one approval and one
  execution. Count actual requests and executions, not only rendered events.
- Verify Ask without escalation still asks once; Allow with escalation asks
  once; Allow without escalation does not gain a new prompt. Deny and missing
  confirmation authority still refuse execution.
- Change command, escalation mode, network origin, cwd or target through hooks
  or context drift; original consent must not authorize the changed operation.
- Reject proof reuse across Run instances, capability generations, calls,
  retries, sibling calls and `program`/`batch`/delegated children, even when
  command text or a message anchor repeats. Replay cannot execute twice.
- Preserve denial, abort, closed-channel and timeout settlement. A late answer
  to the old approval must not settle the next receiver sharing a tool-call id.
  Retain the T029 complete binding and synchronous revocation regressions.
- Finally exercise the actual Work → ACP → Core loopback path with the frozen
  candidate and a bounded controlled command runner. This is a future gate,
  not a result of this read-only investigation.

No duplicate-approval reduction or resulting T01 speed improvement is claimed.
