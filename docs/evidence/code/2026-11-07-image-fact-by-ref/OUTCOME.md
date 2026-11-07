# Outcome: image fact by-reference + post-tool pixel stub

Date: 2026-11-07

## Goal

Stop multimodal turns from re-inlining and re-decoding pasted image
bytes on every fact rebuild / model complete (correlated with ACP
`transport closed` on session `865619d1` after an image paste).

## Changes

1. `Attachment.data` is `Arc<[u8]>` so clones share pixels.
2. `user.message` facts persist bytes under `{effect_dir}/images/<sha256>.ext`
   and store only `{media_type,path,sha256,byte_len}` — never base64 `data`.
3. Legacy facts with inline `data` still load.
4. After tool/model activity on a user image turn, later `complete()` calls
   stub pixels with a digest note instead of re-sending the multimodal payload.
5. `SessionSurface.effect_dir` threads the durable image root into fold/checkpoint.

## Verification

```text
cargo test -p aster-core --lib user_message_fact_stores_image_by_ref -- --nocapture
  → ok

cargo test -p aster-core --lib user_attachment_plan -- --nocapture
  → 2 passed (restore after compaction; stub after tools)

cargo test -p aster-core --lib checkpoint_preserves_user -- --nocapture
  → ok

cargo test -p aster-core --lib persist_session_image_is_content -- --nocapture
  → ok

cargo test -p aster-core --lib both_adapters_send_attachments -- --nocapture
  → ok (4 matrix cells)

cargo test -p aster-core --lib both_adapters_stream_attachments -- --nocapture
  → ok (4 matrix cells)

cargo test -p aster-acp --test v912_seam v912_pasted_image_is_fact_by_ref -- --nocapture
  → ok
  ORACLE pasted-image by-ref stub-after-tools ok

cargo fmt -p aster-core -p aster-acp
  → applied

cargo clippy --locked -p aster-core --all-targets --no-deps -- -D warnings
  → fails on pre-existing lints outside this change (sandbox timing probe,
    windows_shell lifetimes, etc.). No new diagnostics on history_seed /
    session_folder image helpers / llm Attachment Arc change.
```

## Explicitly not claimed

- Did not reproduce or prove OOM/jetsam on the original kstack session with
  this binary; transport-closed root cause remains correlational.
- Tool-result envelopes may still inline base64 for tool-emitted images
  (`FactToolImage.data`); only `user.message` paste path is by-ref.
- Outbound per-request multimodal byte budget (L4) and ACP exit-signal
  observability (L5) are not in this land.
- Installed `aster` on PATH is not updated by this worktree change.

## Follow-up land: tool.result by-ref + stub

Also in this evidence directory:

1. `FactToolImage` / `runner_result_value` / `fact_tool_runner_value` persist
   under `{effect_dir}/images/` and store path/sha256/byte_len (legacy
   `data` still readable).
2. After a later `model.turn`, tool-result pixels are stubbed on subsequent
   `complete()` via `tool_image_ids_to_stub`.
3. `ExecutorTools.effect_dir` threads the durable root into production writes.

### Additional verification

```text
cargo test -p aster-core --lib tool_result_images_are_stored_by_ref -- --nocapture
  → ok

cargo test -p aster-acp --test v912_seam v912_tool_read_image -- --nocapture
  → ok
  ORACLE tool-image by-ref stub-after-model ok

cargo test -p aster-acp --test v912_seam v912_pasted_image -- --nocapture
  → ok (regression)

cargo test -p aster-core --lib both_adapters_stream_attachments -- --nocapture
  → ok (4 matrix cells)
```

## Remaining same-class risks

1. **Compaction `kept` / `message_from_fact`** — folded tool lines can still
   re-store large envelopes as text when kept; by-ref shrinks them but
   prose paste of raw output remains a risk for oversized text tools.
2. **In-memory `ImageSource.data: String`** — defer encode to the LLM
   adapter boundary.
3. **ACP admit peak** — pager still base64-wires paste even when
   `file://` exists.
4. **Secondary** — `ToolCommand` JSON images; `seed_history` without
   `effect_dir` may still legacy-inline seeded tool images;
   `load_session_image` returns `Vec` not `Arc`.
