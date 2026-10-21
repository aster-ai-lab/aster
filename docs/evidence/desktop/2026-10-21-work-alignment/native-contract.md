# Native alignment contract

Scope: Desktop's existing local ACP lane, organization model defaults, and typed read-only goal state. Desktop remains an independent application. No Work development seed, gateway credential bypass, or replacement launcher is introduced.

Approval responses require the Work wire identity: URL approvalId plus body approved, sessionId, executionId, anchorMessageId, messageId and toolCallId. Receiver ownership and cancellation are checked under the live-turn lock. Reconnect carries an authoritative pendingApprovals array; an empty array revokes controls, while persisted chat rows grant no authority. Cancellation withdraws receivers before acknowledging the request; acknowledgement does not mean the turn has settled.

Planning is per prompt: turnContext.planFirst is a boolean, carried as _meta.a3sTurn.planFirst. _meta.permissionPosture carries the independent approval posture. A malformed planning value is refused before stored rows or live ownership. The shared ACP's existing planning policy enforces write restrictions; native prose alone does not grant planning authority.

GET /api/agent-sessions/{id}/goal, streamStatus.goalSnapshot and topic.status.goalSnapshot share the following envelope:

- Known: status=known, sessionId, acpSessionId, executionId (nullable), goal (nullable).
- A goal contains goalId, goalRevision, planRevision (nullable), goalSequence, state, objective and tick. Revision/sequence are positive safe JS integers. States: active, paused, waiting, completed, cancelled. Tick: advance, wait, needs_input, paused, budget_exhausted, reconcile, complete; null only for cancelled.
- Unavailable: status=unavailable, the same nullable identity fields, goal=null and a required closed reason. No inferred historical goal is returned.
- A known null goal requires the ACP's explicit definition:null and tick:null. Missing tick from an older agent is incomplete, not proof that a goal is absent.
- Goal authority is the actual runtime's ACP binding. The Desktop session ID is an outer host scope, not the Core session ID. Responses from a replaced runtime or turn cannot update a newer owner's snapshot.
- Completed/cancelled hide composer controls; history remains available.

The existing a3s/goal/state RPC gains only additive tick, computed from the captured native definition by Core's read-only preview. It does not acquire a turn, tick an execution, recover a Run or append facts. Cancelled/no goal return tick:null.

Model defaults update all three device values (chat.default_model_id, feature.quick_assistant.model_id, feature.translate.model_id). Unrelated preferences remain member-scoped. Existing provider/model eligibility and Desktop's organization gateway credential admission remain unchanged. Synchronous persistence failures restore the previous defaults; this does not claim power-loss atomicity across the existing independent provider and ACL stores.

Hermetic native turn tests inject an explicit synthetic gateway tuple through an internal test-only spawn entry. The product entry still calls the existing gateway_decision_env and organization credential path. Tests do not open or copy the developer's signed-in session, and no product switch is exposed.

Final source and validation: see [native-RCA.md](native-RCA.md), [native-source-final.json](native-source-final.json) and [native-final-validation.json](native-final-validation.json). The actual stdio request wire name is `_a3s/goal/state`; the public/internal extension method remains `a3s/goal/state`. Incoming extension notifications decode exactly one leading underscore before internal dispatch. All initial failures and infrastructure attempts remain in their original logs.
