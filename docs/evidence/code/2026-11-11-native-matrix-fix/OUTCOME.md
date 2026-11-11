# Queue-wait fix and native gesture matrix

Date: 2026-11-11 (Asia/Shanghai). Repository: `/Users/roylin/code/aster`.
Continues `docs/evidence/code/2026-11-11-native-recovery/OUTCOME.md`, whose
record of the earlier recovery acceptance and its unverified matrix is kept
as-is. This record covers the follow-up fix and the matrix re-run on the
rebuilt pair.

## 1. Real defect fixed (shared root cause)

`app::dispatch::queue::tests::held_queue_count_matches_pane_with_send_now_echo`
had failed in both the old and new test programs (`left: 0 right: 1` at
`app/dispatch/queue.rs:3864`). Root cause:

- `resolve_turn_activity_unenriched()` is documented as the wait-identity
  resolver for predicates, but it injected a **display-only** synthetic
  `WaitingReason::PromptAck` whenever `send_now_awaiting_current()` was true.
- That overwrote the tracker's real `TaskOutput { waits: true }` wait.
  `is_parked_on_sendable_wait()` then returned false, so
  `held_queue_count()` returned 0 and `renders_parked()` hid the queued hint,
  while the queue pane still showed the genuine held row.
- `is_sendable_wait()` deliberately excludes `PromptAck`, so every consumer
  of the unenriched resolver lost the wait at the same time.

Fix (`3ebe8a65f`, one file, 10 insertions / 5 deletions): move the synthetic
`PromptAck` into `resolve_turn_activity()` — the display layer — and let the
unenriched resolver return the tracker's real wait. `enrich_waiting_activity`
is still applied on both branches, and `PromptAck` is an identity case there,
so the spinner is unchanged. No assertion was weakened, no test text or
answer was special-cased, and no new abstraction was added.

## 2. Verification of the fix

| Command | Actual result |
| --- | --- |
| `cargo test --locked -p aster-pager --lib held_queue_count_matches_pane_with_send_now_echo -- --nocapture --test-threads=1` | 1 passed, 0 failed (was FAILED) |
| `cargo test --locked -p aster-pager --lib app::dispatch::queue -- --test-threads=1` | 77 passed |
| `... app::agent_view::session` | 40 passed |
| `... app::agent_view::queue` | 47 passed |
| `... views::turn_status` | 45 passed |
| `cargo test --locked -p aster-pager --lib -- --test-threads=1` on HEAD (baseline) | 10346 passed; 26 failed; 4 ignored |
| same command with the fix | 10347 passed; 25 failed; 4 ignored |
| Failure-set diff | `new failures: []`; `fixed: [held_queue_count_matches_pane_with_send_now_echo]` |
| `cargo clippy --locked -p aster-pager --all-targets --no-deps` | exit 0, 51 pre-existing warnings |
| `cargo fmt --all -- --check` | exit 1, 33 hunks in the same 18 pre-existing files; changed file not in the diff |
| `cargo test --locked -p aster-pager --lib input_recovery -- --test-threads=1` | 16 passed |
| `cargo build --locked -p aster -p aster-acp --bin aster --bin aster-acp` | exit 0 |

The 25 remaining lib failures (insta snapshots, diagnostics, status-line
sanitizing, etc.) are unchanged from the baseline; none was weakened or
hidden. The `.snap.new` files those failures produced were removed so they
could not pollute the source hash.

Rebuilt pair: `aster-cli-pair-v1:61a5b499b92fc6a9c43a4c060228050401d861f6:aarch64-apple-darwin`.
`source_pair.py` recomputed `61a5b499…` over 13,465 files with
`source_matches_pair: true`. HEAD `3ebe8a65f`. Rust 1.97.1, macOS aarch64,
shared `target`, `CARGO_INCREMENTAL=0`.

## 3. Native gesture matrix on the rebuilt pair

Real `deepseek/deepseek-flash`, isolated ACL/state/workspace/logs,
`import_accounts = []` plus account-import disable flags, frozen prompts and
timeouts (`frozen-inputs.json`). Window `w-60932` (main), `w-61039` (resume);
process lineage and BUILD_ID verified at each launch; no credential value is
in this evidence. The user's Simplified Pinyin input source was restored.

| Case | Actual result |
| --- | --- |
| Queue edit | PASS — hovering a row revealed `[Send now][edit][cancel]`; clicking `[edit]` loaded the exact row text, saving produced `MATRIX-V2-EDIT-SAVED` in `#1` ([before](screens/matrix-v2-queued.png), [saved](screens/matrix-v2-edit-saved.png)) |
| Queue row delete | PASS — clicking the second row's `[cancel]` action removed only that row ([capture](screens/matrix-v2-after-delete.png)) |
| Forced insertion | PASS — `Ctrl+O` produced exactly one `send_now: true` prompt; original run `cancelled`, edited row `completed` once, original `tool.prepared`/`tool.result` identities matched ([capture](screens/matrix-v2-forced-insertion.png)) |
| Mouse Stop | PASS — clicking `[stop]` cancelled its own run and retired its foreground fixture, no extra prompt ([capture](screens/matrix-v2-mouse-stop.png)) |
| Ctrl+C + input during cancellation | PASS — `Ctrl+C` cancelled its own run and retired the fixture; `MATRIX-V2-DURING-CANCEL-LOCAL-INPUT` stayed in the composer and never became a prompt ([capture](screens/matrix-v2-ctrl-c-during-input.png)) |
| Orderly quit | PASS — graceful quit saved `MATRIX-V2-ORDERLY-EXIT-DRAFT` to `unsent-input.json`, owners retired, 0 model prompts ([before](screens/matrix-v2-before-quit.png)) |
| Explicit `--resume` | PASS — restored the draft with a visible `Draft restored` cue, 0 prompt requests, no automatic replay ([capture](screens/matrix-v2-resumed.png)) |

Raw-data audit: 4 main runs (3 cancelled with 2 identity-matched receipts
each, 1 completed pure-text reply with no tool receipts), 4 prompt requests
with 4 unique prompt ids and 4 unique rpc ids, exactly one `send_now` flag,
0 requests on the resume and quit sessions, 0 `late` markers, 0 surviving
fixtures, and the final resume CLI retired. No duplicate submission, ghost
execution or automatic replay was observed.

### Tool problems, classified (not product failures)

- `Ctrl+Q` was swallowed by the terminal layer and never reached the TUI.
  The graceful-quit funnel was verified with SIGTERM, which dispatches the
  same `Action::Quit` and `Snapshot::take`. **The UI Ctrl+Q gesture itself
  is not claimed as verified.**
- `/quit` consumes the composer as a slash command, so it cannot test draft
  persistence. The first attempt used it and produced no snapshot; that was a
  harness mistake, not a product failure.
- `run_records` land asynchronously. An early `records[-2]` index aborted
  one phase after `Ctrl+O` had already been delivered. The insertion was
  verified from the relay trace and session store without replaying the
  request.
- Compose parsing on the welcome screen had to learn that the empty composer
  is wrapped in a decorative `▀▀▀` border; this was a private-harness parser
  fix, not a product change.
- The agent shell denies `/bin/ps` and AppleEvents to Terminal, so
  `agent-desktop` commands needing its app inventory time out. Observation
  used AX snapshots, input a private CGEvent helper, pixel captures
  `screencapture -l <window id>`, and process identity `pgrep`/`lsof`.

## 4. Release gate — still BLOCKED, nothing published

The parallel ACP cancellation patch (five files), its untracked
`docs/evidence/code/2026-11-10-acp-cancel-terminal/` directory and the dirty
`apps/studio` submodule were preserved and are still uncommitted. Local
commits are not pushed. `scripts/publish-aster-cli.sh` refuses to run when
`git status --porcelain` is non-empty; `release-code.yml` requires an
`aster-acp-v9.2.2` tag with matching Core/ACP versions and a successful
`qualify-code.yml` run. No tag, workflow dispatch, `cargo publish`, channel
manifest update or push was performed. Core is not assumed publishable via
`cargo publish`.

## Explicitly not claimed

- The full 508-case TUI campaign, a fully green package/workspace suite, or
  Windows/Linux runtime parity.
- The UI `Ctrl+Q` quit gesture (SIGTERM was used instead).
- A fix for the 25 remaining pre-existing `aster-pager` lib failures or the
  18 pre-existing `cargo fmt` files.
- Queue reorder (`shift+j`/`shift+k`), dirty-edit discard, image recovery on
  this pair, repeated forced insertion, queue saturation, or every blocking
  cancellation phase.
- Recovery after abrupt process death, power loss, corrupt/unwritable/
  oversized snapshots, concurrent controllers, or uncertain admission.
- Any release, push, tag, or acceptance of the parallel ACP cancellation
  patch.
