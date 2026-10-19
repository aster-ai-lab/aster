# T032 seven effective-argument refusals

The final R2 run contains seven `tool_effect_effective_arguments_unapproved`
results. They are proven preparation refusals with unchanged argument digests;
the available receipts do not identify which permission predicate refused them.
This is a read-only diagnosis, not a production fix or another evaluation.

The exact-session whitelist projection is
[t032-effective-args-diagnosis.json](t032-effective-args-diagnosis.json).
Its source is the same final 187-record snapshot used by
[t032-tool-final.json](t032-tool-final.json), SHA-256
`35dab7ca5af67ee7dacf2d4a365baf3606034f95fba677b477201b67f8fe749b`.
The native terminal state is separately frozen in
[t032-native-metadata-final.json](t032-native-metadata-final.json): error,
`Agent stopped before completion: max_turn_requests`.

## Observed facts

| Model fact | Prepared fact | Result fact | needs_confirmation | Confirmation answer | Original/effective digest |
| --- | --- | --- | --- | --- | --- |
| 140 | 141 | 142 | false | absent | equal |
| 143 | 144 | 145 | false | absent | equal |
| 150 | 151 | 152 | false | absent | equal |
| 157 | 158 | 159 | false | absent | equal |
| 164 | 165 | 166 | false | absent | equal |
| 171 | 172 | 173 | false | absent | equal |
| 182 | 183 | 184 | false | absent | equal |

All seven calls are `bash` with `sandbox_permissions=require_escalated`.
Each argument object has exactly `command`, `justification` and
`sandbox_permissions`; command and justification are strings and nonempty after
trimming. Their baseline is absent, recovery is `never_replay`, and their typed
effects are complete with zero changes. The eight neighboring successful Bash
calls in model-fact window 128–182 have valid, nonempty justification, equal
original/effective digests, a baseline, `needs_confirmation=true` and an approved
confirmation answer. Model fact 128 also had an approved answer, but subsequently
ended in the separately reported inner confirmation timeout (result 131).

Only command hashes and byte lengths were retained. None of the seven refused
commands has an identical command SHA-256 in any other requested call in this
run. The 16 commands in the comparison window are also all byte-distinct. Thus
the evidence does not show the exact same command succeeding after only its
invocation ID changed. This does not test semantic equivalence of different
shell scripts.

## What the current source establishes

In `crates/code/core/src/fact_control/receipts.rs:220–249`, the generic refusal
can be produced by either of these conditions:

1. The effective permission decision is `Deny`.
2. The effective permission decision is `Ask`, and either the effective digest
   differs from the original or the original call did not park while the
   preparation confirmation provider requires approval.

The seven equal digest pairs eliminate the changed-digest arm. They do not
separate `Deny` from `Ask + unconfirmed_ask`: the persisted facts contain neither
the permission decision nor the provider recheck result. A false
`needs_confirmation` is computed by Core, rather than supplied as authority by
the model (`fact_control/tool_results.rs:383–432`). Its producer has several
false exits, including non-Ask permission, skill refusal, execution availability
and disabled/no confirmation provider (`tool_results.rs:331–360`). Therefore
false alone is not proof of lost authorization.

The baseline is created only after denial and argument-error checks succeed
(`receipts.rs:286–309`). Its absence is consistent with refusal before tool
execution; it is not evidence that a workspace mutation caused the refusal.
The actual complete, zero-change receipt is retained without reading file
contents.

The current Bash schema permits those three string arguments and requires
`command` (`tools/builtin/bash.rs:524–573`). Its escalation justification check
is at lines 626–633; all seven pass the corresponding presence, type and
nonempty checks. The current availability preflight returns `Ok` for
`require_escalated` at lines 583–603. These facts do not support a missing or
empty justification explanation for this run.

There is a separately testable diagnostic boundary: `argument_error` can coexist
with a later generic permission denial, which wins rejection selection
(`receipts.rs:63–67`, `239–249`, `319–338`). This source observation is not proof
that any of these seven inputs failed argument validation. A fix must preserve
real policy/skill refusals rather than allow execution merely to improve an
error message.

## Source and run boundaries

The R2 pre/post paired-build manifests and T005 freeze bind `receipts.rs` to
`2e0d104e1026aba7625072c77475164f3efc96ae8d6d355f89c2f19677e6f6a6`.
The current file and R3 post-canonical manifests bind it to
`226107d143da7986451083e318aa8b087a947941e87bec1c1c07f220dfda455a`.
Current line references above describe the latter source. A hash record is not
an archived copy of the former file, so this review does not claim that every
current receipt branch was the identical R2 binary source.

The T005-owned `owned_fact.rs` and `turn_outcome.rs` hashes remain identical
between the R2 manifests and current source. Their bound-Agent confirmation
provider extraction is still present at current lines 196 and 321 respectively.
The [T005 implementation](t005/IMPLEMENTATION.md) explicitly retained the receipt
approval recheck and argument-rewrite guard. Merely observing a call to
`tools.agent.confirmation_provider()` in preparation is not proof that it is
unbound: `ExecutorTools.agent` originates from the captured session surface.
No cache reuse, task-local loss or unique provider drift is established here.

## Smallest next proof

Use the existing controlled original-Run confirmation snapshot fixture and
receipt preparation cases, with no real model, shell or VM. Record/assert the
permission and provider decisions at both the decision and preparation seams
for equal arguments. Cover an unchanged Ask requiring approval, disabled HITL,
explicit Deny, changed Ask arguments and invalid arguments. If any mismatch
reproduces, change only that authority seam and retain the Deny and changed-input
negative controls. A separate invalid-input control can verify error precedence
without treating these seven calls as proven invalid.

No source, task, policy, runtime or application was changed. No test, Cargo,
GUI, model or VM execution was performed for this diagnosis. Ordinary model
text/reasoning, command text, justification text, successful tool output,
configuration and credentials were not copied to this evidence.
