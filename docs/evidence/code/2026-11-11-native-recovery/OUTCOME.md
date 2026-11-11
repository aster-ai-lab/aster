# Native visual acceptance for unsent-input recovery

Date: 2026-11-11 (Asia/Shanghai). Repository: `/Users/roylin/code/aster`.

This record completes the **native desktop acceptance** that the handoff left
open for `7e137d39f` (recover unsent queue, draft and image without automatic
replay). It uses the real `deepseek/deepseek-flash` from
`/Users/roylin/code/config.acl`, a fresh paired CLI/ACP build from this
checkout, and `agent-desktop` accessibility observation of a real Terminal
window. The earlier loopback CLI+ACP checks in
`docs/evidence/code/2026-11-11-input-recovery/OUTCOME.md` are not re-claimed
as native desktop acceptance.

## Environment, identity and ownership

- macOS aarch64, pinned Rust 1.97.1, root workspace, shared `target`,
  `CARGO_INCREMENTAL=0`, default package features.
- Fresh paired build: `cargo build --locked -p aster -p aster-acp --bin aster
  --bin aster-acp` (exit 0).
  Both binaries report
  `aster-cli-pair-v1:ad5c65763c18686d10b397aaa033c969c45a1cb4:aarch64-apple-darwin`.
  `source_pair.py` recomputed source hash
  `ad5c65763c18686d10b397aaa033c969c45a1cb4` over 13,465 files and confirmed
  `source_matches_pair: true` at HEAD `70e5f305c` (the UI/CI commit that
  landed in parallel does not touch the hashed Rust paths).
- Verification HEAD `70e5f305cd078d89f422288daa418e3997125bef`. The recovery
  commit `7e137d39f` is now on `origin/main`; no push or tag was made here.
- Native identity: launch window `w-59725` (CLI 73557, ACP 73600), resume
  window `w-59796` (CLI 84319, ACP 84378), Terminal pid 19277, tty
  `/dev/ttys004`, cwd the private workspace, lineage
  Terminal → login shell → `aster` → ACP test relay → `aster-acp`.
  `lsof` and `pgrep` verified cwd and parent/child lineage; the relay trace
  supplied the real ACP child pid.
- Isolation: ACL profile derived only from the `deepseek` provider block,
  `import_accounts = []` plus `ASTER_DISABLE_CURSOR_ACCOUNT=1`,
  `ASTER_DISABLE_CODEX_ACCOUNT=1`, `ASTER_DISABLE_GROK_ACCOUNT=1`. Private
  `state/`, `workspace/`, `logs/` under
  `.runtime/tui-full-e2e/native-recovery-20261111/`. No credential value is
  present in this evidence. The user's Simplified Pinyin input source was
  restored after both cases.
- Parallel work preserved: at verification time the tree still had the
  uncommitted ACP cancellation patch (five files), an untracked
  `docs/evidence/code/2026-11-10-acp-cancel-terminal/` directory, and a dirty
  `apps/studio` submodule. None were staged, cleaned, overwritten or included
  in this delivery.

### Session-level environment limits (test tool, not product)

This agent shell runs in a sandbox that denies `/bin/ps` and AppleEvents to
Terminal. Consequently `agent-desktop`'s commands that need its macOS app
inventory (`press --app`, `screenshot`, `list-apps`) time out. The private
harness uses `agent-desktop snapshot` for all TUI observation, a compiled
CGEvent helper for keys and clipboard, `screencapture -l <window id>` for
pixel captures, and `pgrep`/`lsof` for process identity. `frozen-inputs.json`
records the hashes of the frozen plan, relay, fixture and image. Harness
scripts remain private under `.runtime/` and are not committed.

## Native recovery case — PASS

Frozen inputs (unchanged from the prior native case): one original prompt
that runs a foreground Bash tool, two exact queued replies, one exact unsent
draft. A small deterministic 8×8 PNG was attached to the composer before the
fault. The fault closes only the real ACP child's stdin while the Bash tool
owns a live process.

| Check | Actual result |
| --- | --- |
| Paired BUILD_ID and source hash | identical; source matched at build and re-checked after the parallel UI commit |
| Admitted prompts before fault | exactly 1, digest of the original prompt |
| Foreground process, tool prepared/result receipts | 1 `tool.prepared` + 1 `tool.result`, identities equal, `tool_name: bash`, `outcome: cancelled` |
| Original Run record | 1, `status: cancelled`, prompt text exact |
| Original owners after EOF | CLI, ACP relay, ACP and fixture all retired; no `late` write from this fixture |
| Visible exit UI | `Error: Agent connection closed` and `Resume this session with: aster --resume <session>` |
| Exit snapshot `unsent-input.json` | 2 queue rows with exact texts, draft `NATIVE-EOF-UNSENT-DRAFT[Image #1]`, 1 image whose persisted bytes match the source PNG SHA-256 |
| Explicit `--resume` | restored both held rows, original cancelled history, draft and visible image chip; **0 prompt requests**; no automatic replay |
| First visible Send now | exactly 1 new prompt request, exact `NATIVE-EOF-FIRST-DONE`, second row still held, composer untouched |
| Second visible Send now | exactly 1 more request, exact `NATIVE-EOF-SECOND-DONE`; 3 runs total, unique prompt/rpc ids, no duplicate submission |
| Unsent composer | never admitted; image-bearing draft retained through all three runs |
| Input source | restored to `com.apple.inputmethod.SCIM.ITABC` |

Key captures: [before](screens/native-recovery-before.png),
[disconnected](screens/native-recovery-disconnected.png),
[resumed](screens/native-recovery-resumed.png),
[all complete](screens/native-recovery-all-complete.png). The byte-exact AX
text frames (terminal padding included) are retained privately under
`.runtime/tui-full-e2e/native-recovery-20261111/views/` and are not committed
because `docs/evidence/**` enables whitespace checks.

## Bounded matrix on the same pair

| Case | Result |
| --- | --- |
| `/resume` command | PASS — opened the real `Resume session` picker listing the session and its original prompt, with 0 new prompt requests ([capture](screens/matrix-resume-picker.png)) |
| Local queue rows with exact frozen texts | PASS — two rows with `[Send now][edit][cancel]` hover actions while a real foreground tool ran |
| Queue edit/save, row delete, forced insertion, mouse Stop, Ctrl+C, input during cancellation, orderly `/quit` recovery | **NOT VERIFIED on this pair.** These are test-tool failures: dynamic Terminal titles invalidated `agent-desktop` window identity, CGEvent typing dropped characters while the TUI repainted, and the first 300s foreground fixture expired during retries. A 900s fixture with a matching 900000ms timeout was introduced for the retry (model, prompts and oracles unchanged) but the queue-pane focus path still could not be completed. No product failure was observed; the failures are not claimed as passes. |

## Committed implementation cross-check (no rewrites)

- Authorization labels use `YOLO` in `app/app_view.rs`, `views/modal.rs`,
  `views/prompt_widget/mod.rs` and `views/dashboard/render.rs`.
- Theme canonical/display names are `aster-night`/`aster-day` and
  `Aster Night`/`Aster Day` (`settings/defs.rs`, `settings/registry.rs`);
  remaining `grok` occurrences in TUI paths are internal module names, env
  vars and unit-test fixtures, not visible labels or theme names.
- `/resume` opens the recovery picker — verified natively above (and in the
  earlier `docs/evidence/code/2026-11-10-tui-visual-controls/OUTCOME.md`).
- Model switch capability state: `DetectionStatus` (`loading`, `ready`,
  `unreported`, `unavailable`) in `crates/code/acp/src/agent/model_discovery.rs`
  feeds `capabilitiesStatus` plus `reasoningEfforts`, `totalContextTokens` and
  `contextWindowKnown` in `agent/models.rs`; the TUI shows a `detecting` flag
  while `capabilities_pending()` and applies the reported effort/context
  when ready. Earlier native captures recorded both pending and resolved
  states. No source change was needed.

## Re-verified known failures (not weakened, not hidden)

| Command | Actual result |
| --- | --- |
| `cargo test --locked -p aster-pager --lib held_queue_count_matches_pane_with_send_now_echo -- --nocapture --test-threads=1` | FAILED (exit 101): `assert_eq left: 0 right: 1` at `app/dispatch/queue.rs:3864`, 1 test run, 10,375 filtered. Same pre-existing failure as the handoff. The fixture supplies a `TaskOutput { waits: true }` wait, so the exact follow-up is whether `is_parked_on_sendable_wait()` loses the tracker state under this manually arranged `shared_queue`; it was not changed here. |
| `cargo fmt --all -- --check` | FAILED (exit 1): 33 diff hunks in 18 distinct pre-existing files, none in this delivery's paths. |
| `cargo test --locked -p aster-pager --lib input_recovery -- --nocapture --test-threads=1` | 16 passed, 0 failed, 10,360 filtered. |
| `python3 tests/release_governance.py` | 10 tests OK. |

## Release gate — BLOCKED, nothing published

`docs/release-governance.md` requires a clean tree, a reviewed commit and the
specified platform verification before publishing. At verification time the
tree still contains the uncommitted ACP cancellation patch, an untracked
evidence directory and a dirty `apps/studio` submodule. Core/ACP are at 9.2.2
with `RELEASE_9.2.2.md`, CLI at 0.3.4, and tags `aster-acp-v9.2.2` /
`aster-cli-v0.3.4` do not exist. The parallel UI/CI work is committed and
pushed; the parallel ACP patch is not. No tag, workflow dispatch, channel
manifest or `cargo publish` was run. In particular, no assumption was made
that `aster-core` is publishable via `cargo publish`; the repository contract
requires `release-code.yml` on an `aster-acp-vX.Y.Z` tag.

## Explicitly not claimed

- Full 508-case TUI campaign, a fully green package/workspace suite, or
  Windows/Linux runtime parity.
- Native re-verification on this pair of queue edit/save, row delete, forced
  insertion, mouse Stop, Ctrl+C, input during cancellation, or orderly-exit
  recovery. Previous results on older builds are not re-claimed.
- A fix for the pre-existing `held_queue_count_matches_pane_with_send_now_echo`
  failure or the 18 pre-existing `cargo fmt` files.
- Recovery after abrupt process death, power loss, corrupt/unwritable/oversized
  snapshots, concurrent controllers, or uncertain admission before
  acknowledgement.
- Reconstruction of admitted in-flight prompts, editor undo history, or
  replacement of authoritative session history.
- Any release, push, tag, channel update, or acceptance of the parallel ACP
  cancellation patch.
