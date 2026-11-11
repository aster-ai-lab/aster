# Unsent input recovery

The CLI now saves session-owned local queues and composer contexts on orderly
exit, including an observed ACP connection closure. Resume loads them
asynchronously. Recovered queue entries remain held until an explicit send;
recovery does not rewrite admitted Core history or replay the interrupted turn.
The saved contexts retain queued wire payloads, images, styled chips, edit
ownership, and stashed drafts. Loading does not overwrite fresh composer input.
Private atomic writes reuse the existing session directory and filesystem
helpers. Invalid snapshots fail visibly without replacing the previous file.

## Environment and ownership

- macOS, aarch64, pinned Rust 1.97.1, root workspace, shared `target`,
  `CARGO_INCREMENTAL=0`, default package features.
- Base commit: `81f657b7bd1425a21b0d9a4e7d447348b2658655`.
- The tested CLI and ACP reported the same paired BUILD_ID:
  `aster-cli-pair-v1:ad5c65763c18686d10b397aaa033c969c45a1cb4:aarch64-apple-darwin`.
  Source matching was recorded at build time. Later unrelated UI changes are
  excluded from this delivery.
- The diagnostic build also contained five pre-existing ACP source changes
  concerning foreground cancellation. They remain outside this commit; their
  working-tree diff was preserved. This evidence does not independently accept
  or deliver those changes.
- Real native model attempts used only `deepseek/deepseek-flash`, an isolated
  ACL profile and state, with account imports disabled. Successful deterministic
  EOF/resume checks used the actual CLI and ACP with a loopback model fixture.

## Verification

Commands ran from the repository root. Test filters overlap; counts are not
additive. Private logs and fixture artifacts remain under
`.runtime/tui-full-e2e/input-recovery` and
`.runtime/tui-full-e2e/queue-input-recovery`.

| Command | Actual result |
| --- | --- |
| `cargo test --locked -p aster-pager --lib input_recovery -- --nocapture --test-threads=1` | 16 passed |
| `cargo test --locked -p aster-pager --lib app::tests -- --nocapture --test-threads=1` | 71 passed |
| `cargo test --locked -p aster-pager --lib finish_run -- --nocapture --test-threads=1` | 6 passed |
| `cargo test --locked -p aster-pager --lib app::dispatch -- --nocapture --test-threads=1` | 1,871 passed, 1 pre-existing failure |
| `cargo test --locked -p aster-pager --lib queue -- --nocapture --test-threads=1` | 448 passed, the same pre-existing failure |
| `cargo clippy --locked -p aster-pager --all-targets --no-deps` | Passed; existing warnings remain |
| `cargo build --locked -p aster -p aster-acp --bin aster --bin aster-acp` | Passed for the final source snapshot |
| `cargo fmt --all -- --check` | Failed on the same 18 pre-existing files as the retained baseline; no new affected paths |

The application and exit tests preceded an equivalent event-loop change from a
fold to a loop. The final recovery tests, Clippy and paired build include that
change. No additional recovery implementation edits followed those checks.

The unchanged failure is
`app::dispatch::queue::tests::held_queue_count_matches_pane_with_send_now_echo`.
The retained pre-change executable and the new executable both fail with the
same count mismatch (actual 0, expected 1). It was not weakened or repaired as
part of recovery. The session-load test was updated to supply the new typed
recovery completion before checking its original queue assertions.

## Actual CLI interaction

The EOF producer retained the original prompts and timeouts. Closing only the
real ACP child's stdin produced one cancelled original Run, one bound cancelled
tool receipt, no surviving foreground process, and CLI exit code 1. The original
grader passed: one prompt request and one model generation.

The recovery observer fails against the retained old result: both waiting rows
and the composer draft are missing. Against the new binaries, explicit resume
restores both exact waiting rows and the exact draft, shows the original
history, and makes zero model POST requests.

Actual terminal mouse clicks on each visible `Send now` action then produce
exactly two prompt requests and two completed model generations. Releasing the
first leaves the second held. Both expected replies match, the original Run
remains cancelled, and the unsent composer remains intact. There are no
unrequested model POSTs. The fixture producer, observer, and controller hashes
are retained privately with the results.

An earlier manual actuator used an unbound shortcut and sent no request. Its
artifacts were retained separately before switching to the visible button;
model inputs, deadlines and assertions were unchanged.

## Native desktop attempt and stop

Native acceptance for this patch is incomplete. The first attempt was invalid:
the operator delayed past the foreground fixture lifetime, so the original and
queued tasks completed before fault injection. Its controller refused to inject
the late fault and its artifacts were preserved.

A second fresh source-qualified native window encountered an observer race and
undelivered typing. Later `agent-desktop` typing filled the composer but did not
submit it. At the user's explicit stop request, this attempt had zero prompt
requests and 166 composer characters. The qualified CLI was terminated
gracefully; its CLI and ACP owners retired, and the original input source was
restored. No native EOF/resume success is claimed.

## Explicitly not claimed

- Completion of the full TUI matrix, a fully green package/workspace suite,
  current native desktop recovery acceptance, or Windows/Linux runtime parity.
- Recovery after abrupt process death, power loss, an uncertain admission
  before acknowledgement, or concurrent controllers editing the same snapshot.
- Guaranteed persistence of new edits when storage is corrupt, unwritable,
  oversized, or stalled. Save errors are reported; preserving the old file
  cannot guarantee that new input was written.
- Reconstructing admitted in-flight prompts, editor undo history, or replacing
  the authoritative session history.
- A release, push, acceptance of the unrelated ACP patch, or completion of the
  remaining campaign. Work stops after this local delivery at the user's request.
