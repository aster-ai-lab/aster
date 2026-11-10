# DeepSeek visual TUI verification and cancellation receipts

Date: 2026-11-10, macOS arm64, Terminal, Rust 1.97.1.

Later YOLO host cancellation recovery is recorded in the
[follow-up verification](../2026-11-10-cancel-recovery/OUTCOME.md). The failures
below remain the results of the earlier candidates.

## Scope and current status

agent-desktop operated a real native Terminal window running the source-built
`aster` / `aster-acp` pair. The real model was `deepseek/deepseek-flash`, explicitly
pinned from the operator's `../../config.acl`, resolved against the supplied
ToWork directory as `/Users/roylin/code/config.acl`. The source file was not
edited. A private, byte-identical copy supplied the isolated test process.

This is a scoped execution record, not completion of the 508-case TUI catalogue.
The coding workflow and initial process-control observations ran on the initial
pair. After an earlier loginwindow interruption, the operator unlocked the
desktop. Headed follow-up exercised resume, explicit effort selection, forms,
Chinese IME, settings, themes, mode changes and both screen modes. The latest
rebuilt pair also verified the minimal footer correction and resumed effective
context. No login credentials were used or requested.

**Native command recovery remains failed.** A repeated send-now test on the
receipt-fix pair stopped the original process pair and admitted the replacement
once, but its final run status was `failed` with `tool_effect_unresolved`.
A successful file write is not a recovery pass. This candidate is not a clean
release build or a completed release qualification.

## Candidate identity

- Repository baseline: `e553cf89c608`, with the pre-existing five-file ACP
  cancellation patch in the shared working tree
- Initial pair: `aster-cli-pair-v1:b61531cd75633b0262d192e06a0a9feef2666413:aarch64-apple-darwin`
- Receipt-fix pair: `aster-cli-pair-v1:d0d68f2e29d3e85c41e0befd0acdfb18f61bc43d:aarch64-apple-darwin`
- Latest minimal-fix pair: `aster-cli-pair-v1:14200199ef0e08e1b6b4f5d38c0e96918dd4b815:aarch64-apple-darwin`
- Versions: CLI 0.3.4; Core/ACP 9.2.2
- Configuration and binary hashes: [candidate.json](candidate.json)
- Native grids: 174 columns by 51 rows and 110 columns by 35 rows

Both builds used:

```sh
CARGO_TARGET_DIR=/Users/roylin/code/aster/target CARGO_INCREMENTAL=0 \
  cargo build --locked -p aster -p aster-acp --bins
```

The source tree was deliberately not treated as a clean release candidate. The
pre-existing ACP patch was preserved and is not adopted as this task's code.
Profiles, raw native automation logs and complete session stores stay outside the
repository under `/tmp/aster-tui-deepseek-e2e`. UI changes affected the private
configuration copy; the original ACL hash stayed unchanged. The private
credential copy was removed after the owned pair exited.

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

## Headed follow-up

Session `c477133c-35f0-4610-a3ea-cee4d93be7d5` completed its first four model
turns. The subsequent foreground command was cancelled and its queued
replacement failed. See [followup-session.json](followup-session.json).

| Operation | Actual result | Evidence |
| --- | --- | --- |
| `/resume` | Opened, searched and restored the exact existing session without replaying tools | [Picker](screens/22-resume-popup.png), [restored ID](screens/23-resumed-session.png) |
| Effort selection | `none` and `low` each produced a real DeepSeek response; `default` restored the reported high default | [None](screens/24-effort-none.png), [context after reset](screens/26-context-followup.png) |
| Form validation | Empty required fields and amount 6 were rejected; the maximum remained 5 | [Required](screens/29-form-required-state.png), [range](screens/30-form-range.png) |
| Form submit | Complete tool receipt and written JSON both contain exactly `Lin-42`, 4 and true; the frozen form stayed unchanged | [Submitted](screens/31-form-submitted.png), [forms.json](forms.json) |
| Form cancel | OS Ctrl+C returned `form was cancelled by the user`; the receipt is complete and the model turn finished | [Cancelled](screens/33-form-cancelled.png), [forms.json](forms.json) |
| Chinese IME | Pinyin Enter committed Latin `nihao`; Space selected `你好`; neither submitted a message or changed canonical facts | [Candidate](screens/36-ime-chinese-candidate.png), [ime.json](ime.json) |
| Theme and settings | Search reached the theme picker; Aster Day was saved in isolated TOML and Aster Night was restored | [Choices](screens/41-theme-setting-picker.png), [saved day](screens/42-theme-day-saved.png), [themes.json](themes.json) |
| Permission labels | Normal, Auto and YOLO cycled; the YOLO notice and fullscreen footer agreed | [YOLO](screens/44-mode-yolo-aligned.png), [modes.json](modes.json) |
| Narrow layout | Resume search and the command palette were usable at 110 by 35 | [Search](screens/45-narrow-resume-search.png), [help](screens/46-narrow-help.png) |
| Screen modes | Minimal hosted `/help` inline and returned to fullscreen; its lowercase permission and missing default effort exposed a footer defect | [Before fix](screens/47-minimal-screen.png), [inline help](screens/48-minimal-help.png) |
| Rebuilt minimal footer | Latest pair displays `DeepSeek Flash (high) · YOLO` through the common formatter | [After fix](screens/54-minimal-labels-fixed.png), [minimal-labels.json](minimal-labels.json) |
| Rebuilt resumed context | Same session ID, DeepSeek route, YOLO and 1048576-token context | [Context](screens/55-resumed-effective-context.png) |
| Repeated send-now | Original process pair exited and replacement wrote exact bytes once, but its run failed with five incomplete receipt obligations | [Failure](screens/52-postfix-recovery-terminal.png), [postfix-queue.json](postfix-queue.json) |

Failed and cancelled runs were checked against authoritative persisted run
snapshots. They do not emit a successful `turn.complete` fact. Form cancellation
uses the existing plain error receipt contract. These distinctions prevent a
harness predicate from manufacturing a product failure or treating a partial
artifact as successful completion.

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
5. Minimal mode hardcoded lowercase permission flags and read only an explicit
   reasoning selection. It now reuses fullscreen `mode_flags` and
   `effective_reasoning_effort`, including independent plan/permission flags.

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
| Minimal prompt-info tests | 4 passed |
| Core all-target Clippy | Completed; existing warnings remain |
| Minimal all-target Clippy | Completed; existing warnings remain |
| Latest paired build | Passed; build IDs match |
| Scoped Core/ACP/pager/minimal formatting | Passed |
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
submitted task. No clipboard write was used. The unlocked desktop also allowed
native focus confirmation after an agent-desktop focus error. No fallback
attempted to operate a locked desktop. A completed Terminal process was reopened
in a new owned window before testing the latest pair. Chinese IME observations
use the actual OS commit behavior.

The owned CLI/ACP processes and Terminal window exited. The original input
source was restored and the private credential copy was removed. The original
ACL and failing sessions were preserved. See [cleanup.json](cleanup.json).

## Explicitly not claimed

- Completion of all 508 cases, all commands/settings/forms, or another platform
- A full 508-case headed campaign or universal model capability discovery
- Automatic reconciliation of genuinely unknown native command effects, or
  successful completion of the queued turn that hit that guard
- Native wire-parameter capture, maximum-context stress, universal discovery,
  a performance benchmark, warning-free Clippy or workspace-wide green checks
- A clean release build, publication, or adoption of the parallel ACP patch
- Changes to the original ACL, real user projects, account state or clipboard
