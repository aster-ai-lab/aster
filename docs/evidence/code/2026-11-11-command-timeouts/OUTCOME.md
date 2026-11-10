# Command timeout budgets and ownership

Native TUI tests found two separate defects. A request for 900000 ms hit the
native policy's 120000 ms ceiling but reported the requested value. A shorter
1500 ms request raced the executor's own deadline: the caller dropped the
cleanup future and returned no original exit or captured output. The short
deadline race also reproduced through YOLO's host adapter.

The shared Bash path now resolves the backend budget for diagnostics and lets
qualified built-in executors own their deadline and cleanup. Native and
process-host executors opt in; the host adapter forwards its runner's choice
through the existing workspace boundary. Legacy/custom executors retain the
caller watchdog. If that watchdog fires without an observed terminal, the
result explicitly marks incomplete evidence and says termination was not
observed. No fixed grace period, model branch, new dependency, or changed
requested execution timeout was added.

Windows local-host execution retains the legacy watchdog because its existing
direct HTTP compatibility path has no internal request deadline. This is an
explicit qualification limit, not a Windows runtime pass.

## Verification

Commands ran at the Aster root with Rust 1.97.1,
`CARGO_TARGET_DIR=/Users/roylin/code/aster/target`, and `CARGO_INCREMENTAL=0`.
[checks.json](checks.json) retains exact commands, counts, and log digests.

- Policy diagnostics: both original failures reproduced individually, then
  **3 tests passed**, including a non-default native policy budget
- Bash: **73 passed, 0 failed, 1 existing ignored**
- Sandbox adapters: **32 passed, 0 failed, 0 ignored**, including actual host
  timeout capture and process-host/native contract checks
- Cancellation receipts: **7 passed, 0 failed, 0 ignored**, including incomplete
  evidence and preservation of the original settled results
- Core all-target Clippy passed with existing warnings
- Touched-file formatting and `git diff --check` passed. Workspace formatting
  still reports 18 unrelated pre-existing paths
- The final paired build passed. Both binaries independently match current
  source fingerprint `4edcab0260aa11cbaaf41373819aab27723b8325`; see
  [source-reconciliation.json](source-reconciliation.json)

The new ownership regression first failed with only caller timeout text. After
the fix it retains partial output returned after the executor's deadline.
The legacy regression also first failed and now verifies its incomplete
evidence marker. Two initial fixture compile errors were corrected before
these compiled checks; those attempts are not counted as executed test cases.

## Native observations and replay status

The earlier diagnostic pair `2a0d00d9ae724ea12f83b7dc1fe523aff1796342`
ran in native macOS Terminal with `deepseek/deepseek-flash` and the supplied ACL.
The original ACL was unchanged. Its effective-budget case passed: one original
command returned a complete timeout receipt at 120000 ms, retained requested
900000 ms, and its observed process exited. Help and context views opened and
closed while it ran, without another model call. The exact frozen scenarios
remain in [timeout-plan.json](timeout-plan.json) and
[timeout-host-plan.json](timeout-host-plan.json).

That same pair exposed the short-deadline race in Normal and YOLO. The original
observations and failures remain in [assessment.json](assessment.json) and the
adjacent artifacts. The final ownership fix has **not yet passed native
replay**: macOS was locked before its launch. The replay keeps the original
prompts, model, commands, deadlines, and output checks. Its private operator
scopes identical prompts to the new launch, preserves baseline artifacts, and
also requires original exit/capture metadata and no surviving fixture process.

agent-desktop reference typing again failed on Terminal's dynamic window
identity. The native baseline used recorded System Events text input;
guarded foreground agent-desktop physical keys successfully delivered its
approvals and local-view gestures. No unknown-delivery action was blindly
replayed.

## Explicitly not claimed

This is logic and adapter validation plus retained native reproductions, not
final native acceptance of the ownership change, a clean release candidate,
Windows/Linux runtime acceptance, or completion of the whole TUI campaign.
The existing parallel ACP patch is still uncommitted and is not included in
this commit. No product release was published. Context reporting was also
observed to conflate cumulative usage with occupancy; that separate defect
remains to be fixed and verified.
