# T027: independent development-app DeepSeek Flash assessment

**Result: failed host approval recovery; no normal model completion.** The
saved interrupted artifacts independently pass **8/9**, failing negative zero.
This is a completed capability assessment with a failed outcome, not an
optimized configuration qualification or a completed model submission.

## Actual run and identity

- Ninth total T01 attempt: eight terminal Core attempts plus this one CUA
  assessment. The original prompt was sent once; no replacement prompt,
  manual answer repair or model retry was used.
- Fresh workspace: `/Users/roylin/code/tasks/T01-t027-cua-01`.
  Saved files remain `pipeline.py` and `output/results.json`, together with
  the model's own temporary verification scripts.
- Native app session `01a155cf-02f1-7478-8105-070a97a1507c`; Core session
  `591d55d3-ea0e-42c5-8036-ae475c7881fc`.
- UI selected DeepSeek Flash, planning off, per-operation confirmation.
  The actual generated config bound `deepseek/deepseek-flash` and
  `https://api.deepseek.com`, adaptive presentation and retrieval disabled.
  No credentials or complete generated provider config were copied.
- Independent signed app: `target/development-stage/task-orchestration-20261020/`
  `AsterDream-TaskOrchestration-Dev.app`. Actual app PID 3167 and ACP PID 4033
  mapped to the exact staged executables. Hashes and configuration are in
  `actual-acp*.json`, `effective-config.json` and `../t027-stage-launch.json`.
- A bounded read-only observer during the existing approved Bash captured
  hardware-asset OCI Host PID 7097 and shim PID 7116, with the frozen hashes.
  See `runtime-observation.json`; earlier missed short-lived processes are
  not reclassified as failures or used to fabricate a VM count.
- `identity-after.json` verifies the same signed app/ACP/Office and hardware
  Host/shim bytes after the assessment. The original app was separately
  reopened before staging as the application-level rollback check.

## Failure and settlement

The host persisted `awaiting-approval` at 20:36:58.652 UTC. The last native fact
is a Bash decision requiring confirmation, with no answer or preparation.
The saved tool part was `approval-requested`, but the native UI showed an idle
composer, no approval interface and no Pause control. Home → reopening the
exact same conversation did not recover them. The source audit confirms that
sidebar “End” only organizes the list and does not abort a turn.

Root normally quit only this independent stage app. The original app, ACP and
the two observed OCI PIDs were then absent. Reopening the same unchanged stage
without sending a prompt reconciled the orphan to host/assistant status `error`
and visibly displayed:

> 执行已中断，运行进程已不存在。请重新运行任务。

`terminal.json` records the reopened app PID 13047 and this observed error.
The app is left open on the saved assessment. Persistence alone is not proof
that the original approval receiver remained live throughout the wait.
`approval-recovery-checkpoint.json` preserves the pre-exit metadata; the
independent source audit in `../../approval-recovery/OUTCOME.md` distinguishes
confirmed cancellation reachability from the unobserved stale-overlay race.

The original facts contain 30 model turns, 33 paired prepared/results and one
user message. Of those receipts, 29 succeeded, two failed and two were denied.
All 24 actual confirmation answers were approvals. The denied outcomes were
effective arguments not approved and the repeated-arguments guard; they were
not user refusals. The pending final decision never started a tool. Safe
structural totals and their limits are recorded in `fact-summary.json`.

## Frozen interrupted artifacts and independent evaluation

`saved-artifacts.json` binds the original saved bytes; `saved-artifacts/` holds
read-only copies made after the original processes exited, before evaluation.
No model code was repaired by the evaluator. All 12 instruction/input hashes
in the fresh workspace and original T01 match their preparation manifest.

- Pipeline: 23,208 bytes,
  SHA-256 `ad1be2d94cc2b9987e0c1ac4ff2c0483fe56765d7dc1493c1b5e241b800a9581`.
- Results: 1,439,900 bytes,
  SHA-256 `1410d96bf12818691780e94e03e6c58001060c9af862d092736f255b58c097e8`.
- The unchanged external evaluator ran all nine scenarios, including its
  270-case DP/brute-force self-check: eight passed, negative zero failed.
  The saved results exactly match the frozen pipeline's public regeneration.
  Static imports are standard library only. See `oracle.json`.
- Oracle process wall: **0.960 s**, process reaped, no outer timeout, frozen
  pipeline unchanged. Its exit 1 correctly represents failed quality.
  The one-shot supervisor's final assertion incorrectly required exit 0 after
  writing the complete report; this diagnostic is retained separately in
  `oracle-interpretation.json`. Neither task nor oracle was rerun or rescored.

## Timing boundary and limits

Submission action requested at 20:16:29.690597 UTC; the host accepted the user
message at 20:16:32.953. The last persisted approval request was 20:36:58.652;
original process absence was observed at 20:43:46. Artifacts were frozen at
20:44:08.860283. **1659.335 s** from submission request to freeze is an observed
upper bound including model, tools, approval actions, UI recovery and operator
diagnosis. It is not model inference, ordinary completed-task time or a speed
comparison against the fixed-permission Core matrix. Restart/error recovery
and the 0.960 s independent oracle are separate activities.

Native facts do not provide complete absolute model/tool intervals. The UI
action requests in `ui-action-times.json` lack request-appearance timestamps,
so pure approval wait and inference cannot be reconstructed or subtracted.
Actual GUI execution-budget scalars are unknown; the Core matrix's fixed 2M
tokens/15 minutes/128 rounds must not be assigned to this GUI run.

## Next executable work

The updated `specs/001-task-execution/plan.md` and `tasks.md` prioritize T029
approval recovery and reachable cancellation, then T028 effective host/Bash
execution context. Both use controlled fixtures before any performance claim.
Existing PTC rejection/adoption analysis and actual grep/LSP/configuration
entry verification follow. Preserve valid native authority, failure receipts,
bounded budgets and resource settlement. Do not simply raise request limits,
switch off confirmations, modify the saved answer or append paid retries.

No model-completion, T01 9/9, stable PTC speed benefit, runtime promotion,
Windows/Linux or actual local ONNX qualification is claimed. The independent
review is `independent-audit.json`; broader epic requirements remain open.
