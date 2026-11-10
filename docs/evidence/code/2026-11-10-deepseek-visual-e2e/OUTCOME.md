# DeepSeek visual TUI verification and cancellation receipts

Date: 2026-11-10, macOS arm64, Terminal, Rust 1.97.1.

## Scope and current status

agent-desktop operated a real native Terminal window running the source-built
`aster` / `aster-acp` pair. The real model was `deepseek/deepseek-flash`, explicitly
pinned from the operator's `../../config.acl`, resolved against the supplied
ToWork directory as `/Users/roylin/code/config.acl`. The source file was not
edited. A private, byte-identical copy supplied the isolated test process.

This is a scoped execution record, not completion of the 508-case TUI catalogue.
The coding workflow and process-control observations below ran on the initial
pair. The final rebuilt pair has passing focused checks and an observed welcome
copy correction. Further headed interaction is **blocked by macOS loginwindow**:
agent-desktop cannot focus the app and CoreGraphics reports no current display
mode. Resume, post-fix forms, explicit effort selection, and Chinese IME
acceptance remain unexecuted in this campaign. No login credentials were used
or requested.

## Candidate identity

- Repository baseline: `e553cf89c608`, with the pre-existing five-file ACP
  cancellation patch in the shared working tree
- Initial pair: `aster-cli-pair-v1:b61531cd75633b0262d192e06a0a9feef2666413:aarch64-apple-darwin`
- Final pair: `aster-cli-pair-v1:d0d68f2e29d3e85c41e0befd0acdfb18f61bc43d:aarch64-apple-darwin`
- Versions: CLI 0.3.4; Core/ACP 9.2.2
- Configuration and binary hashes: [candidate.json](candidate.json)
- Native grid: 174 columns by 51 rows

Both builds used:

```sh
CARGO_TARGET_DIR=/Users/roylin/code/aster/target CARGO_INCREMENTAL=0 \
  cargo build --locked -p aster -p aster-acp --bins
```

The source tree was deliberately not treated as a clean release candidate. The
pre-existing ACP patch was preserved and is not adopted as this task's code.
The copied credentials, profiles, raw native automation logs and complete session
stores stay outside the repository under `/tmp/aster-tui-deepseek-e2e`.

## Actual native observations

| Operation | Observed result | Evidence |
| --- | --- | --- |
| Source-built startup and model pin | The paired source binaries launched with DeepSeek Flash; the menu resolved none/low/high/max and a reported high default | [Initial welcome](screens/01-launch.png), [effort menu](screens/02-effort-menu.png) |
| Read-only diagnosis | Real tool reads identified both defects; source and checker hashes stayed unchanged | [Diagnosis](screens/03-real-diagnosis.png) |
| Approval | An actual edit required permission; approval allowed that invocation; execution asked separately | [Edit approval](screens/04-edit-approval.png) |
| Partial repair | Only `rate.py` changed; the actual checker exited 1 and printed `SHOP-HOLD`; `fee.py` and the checker remained unchanged | [Partial task](screens/09-partial-hold.png) |
| Full repair | The second repair produced `SHOP-PASS`; an independent checker tested seven additional counts per function, all 14 passed | [Completion](screens/10-real-task-complete.png), [holdout.json](holdout.json) |
| Mode and context display | Session info showed the actual session ID, DeepSeek model, YOLO and 1048576-token effective context | [Session info](screens/11-session-info.png) |
| Mouse Stop | Both exact parent and child processes were alive before the click and absent after the TUI settled; no late-write marker | [Before](screens/15-mouse-descendants-alive.png), [after](screens/16-mouse-descendants-settled.png), [oracle](mouse-descendants.json) |
| Ctrl+C | agent-desktop refused injection before delivery; the existing macOS System Events route delivered Ctrl+C; both original processes exited and no late marker appeared | [Cancelled](screens/17-keyboard-cancelled.png), [oracle](keyboard-live.json) |
| Send-now | The foreground process pair exited; the queued replacement was admitted once and created the exact requested bytes | [Queued](screens/18-send-now-queued.png), [settled](screens/19-send-now-settled.png), [oracle](insert-live.json) |
| Uncertain effects | The replacement's terminal result failed with `tool_effect_unresolved`; it is not a full recovery pass despite the file being created | [Retained failure](screens/20-unconfirmed-effects-preserved.png), [session summary](session-summary.json) |
| Welcome advertisement | The welcome screen advertised `/feedback`, but typing it produced an unavailable-command error | [Refusal](screens/14-advertised-feedback-unavailable.png) |
| Rebuilt welcome | The new source-built pair displays `Describe your task` | [Native window capture](screens/21-fixed-welcome-native.png) |

The partial repair was an explicitly scoped task to repair only one function.
Its per-file `verified` annotation is not evidence that the complete shop checker
passed. The actual checker failure and subsequent pass are retained separately.

The first mouse attempt is excluded from descendant-cancellation acceptance:
the child marker's timestamp precedes the tool terminal by about seven seconds.
The repeated observation required both exact processes to be alive immediately
before cancellation. The original observation and its late marker were retained;
they were not erased to obtain a passing result.

## Defects corrected

1. `execute_tool_with_deadline` awaited the original invocation during cancellation
   cleanup, then discarded its returned result. The fact adapter independently
   replaced a returned result whenever cancellation had been observed. Both now
   preserve the original terminal result. They do not mark a non-returning or
   incompletely observed operation as settled.
2. The welcome subtitle pointed at a command that Aster disables. It now uses an
   open-ended task invitation, without enabling the unavailable command.
3. An imported legacy account description retained old product copy. The common
   description now uses `Imported Legacy CLI account`; account/provider identity
   and user-authored model names are unchanged.
4. Required all-target Clippy exposed a pre-existing schema-test generator passing
   a `String` to the current `Arc<str>` image field. Its construction now uses
   `.into()`; serialization assertions and generated values were not weakened.

## Regression and non-overfitting checks

The new regression failed before the runtime fix: the tool returned its original
result, but the recorded outcome was `Cancelled` instead of `Succeeded`.

The same test runs successful, failed and incomplete-evidence variants through
the real preparation/execution/receipt path. It asserts one original invocation,
the exact original identity, independently read written bytes, the recorded file
change, and retained uncertainty only for incomplete evidence. Production code
has no fixture names, model-name checks, expected-output matching or test-only
branch. The independent coding checker and 14 held-out input checks were frozen
before product changes. No model prompt, grader, approval policy, sandbox gate
or reconciliation guard was relaxed to manufacture success.

The native cancellation receipts have actual incomplete-effect obligations.
Process exit alone does not close them, and this change does not rewrite those
old receipts. The model's own interpretation of a late marker is not an oracle.
Original statuses and receipt identities are retained in the summaries.

## Verification

All Cargo checks used the shared root target and `CARGO_INCREMENTAL=0`, with one
Cargo owner. Exact source command summaries are in [results.json](results.json).

| Check | Actual result |
| --- | --- |
| New receipt regression before fix | 1 failed, as expected |
| Core receipt, cancellation and timeout tests | 114 passed |
| Persisted schema roundtrip | 11 passed |
| ACP terminal and imported-account tests | 17 passed |
| Core all-target Clippy | Completed; existing warnings remain |
| Final paired build | Passed; build IDs match |
| Scoped Core/ACP/pager formatting | Passed |
| Workspace formatting | Failed on 18 pre-existing files; unrelated code was not bulk formatted |
| `git diff --check` | Passed |

## Desktop automation qualifications

agent-desktop handled native window discovery, resizing, text input, pointer
input, snapshots and screenshots. It also reported failures that were retained
as infrastructure observations: animated-title identity mismatches, invalid
macOS menu-modifier metadata for Ctrl+C, and later unavailable display-mode
information at loginwindow. Actions with unknown delivery were inspected before
another action was attempted.

The repository's existing macOS approach supplied bounded fallbacks for affected
focus/ASCII-key operations and native window capture. These are not claimed as
successful agent-desktop deliveries. Ordinary automation selected ABC and waited
for the actual input-source change. A preedit commit was not treated as a
submitted task. No clipboard write was used. Chinese IME acceptance still needs
its own qualified post-build interaction run.

## Explicitly not claimed

- Completion of all 508 cases, all commands/settings/forms, or another platform
- A post-fix full headed campaign while the desktop is at loginwindow
- Automatic reconciliation of genuinely unknown native command effects, or
  successful completion of the queued turn that hit that guard
- Native wire-parameter capture, maximum-context stress, universal discovery,
  a performance benchmark, warning-free Clippy or workspace-wide green checks
- A clean release build, publication, or adoption of the parallel ACP patch
- Changes to the original ACL, real user projects, account state or clipboard
