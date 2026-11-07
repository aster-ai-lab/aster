# Outcome: crash-hardening follow-up (wire / dual-write / schema / seed)

Date: 2026-11-07  
Host: macOS (darwin), workspace `/Users/roylin/code/aster`

## Goal

Continue the ranked crash audit: remove remaining peak-memory and stack
amplifiers after image-fact by-ref and task stack isolation, and keep focused
plus ACP seam e2e green.

## Changes

1. **Wire path** — `Attachment` caches shared base64 (`OnceLock` + `Arc<str>`);
   `ImageSource.data` is `Arc<str>` so `Message` clones do not reallocate.
   `ToolCommand` stashes `Attachment` arcs across the queue `Value` boundary
   (`image_stash_id`) instead of a second base64 encode/decode.
2. **`tool.result` dual-write** — `receipt_fact` leaves top-level `output` empty;
   fold/validate/read resolve text from `receipt.output`. Legacy facts that
   still carry both copies remain accepted when they match.
3. **Legacy inline load** — decoding inline `data` optionally persists under
   `{effect_dir}/images/` for later path-based reads.
4. **`generate_object` schema refuse** — same 256 KiB / depth-32 ceilings as
   tool parameter schemas, applied before `jsonschema::build` and at
   `generate_blocking` / `generate_streaming` entry.
5. **Seed `effect_dir`** — `FactRun::seed_history` and session fork pass the
   session effect directory into `append_seeded_tool` so seeded tool images
   stay by-ref.

## Verification

| Check | Result |
| --- | --- |
| `cargo test --locked -p aster-core --lib user_message_fact_stores_image_by_ref` | ok |
| `cargo test --locked -p aster-core --lib tool_result_images_are_stored_by_ref` | ok |
| `cargo test --locked -p aster-core --lib test_attachment_base64_data` | ok (shared Arc) |
| `cargo test --locked -p aster-core --lib test_attachment_to_content_block` | ok |
| `cargo test --locked -p aster-core --lib test_message_tool_result_with_images` | ok |
| `cargo test --locked -p aster-core --lib test_queued_tool_result_preserves_metadata` | ok |
| `cargo test --locked -p aster-core --lib both_adapters_send_attachments` | ok (4 matrix cells) |
| `cargo test --locked -p aster-core --lib checkpoint_preserves_user_and_tool_image` | ok |
| `cargo test --locked -p aster-core --lib generate_blocking_refuses_deep_schemas` | ok |
| `cargo test --locked -p aster-effect --test tool_effects` | 14 passed |
| `cargo test --locked -p aster-effect --test tool_effects committed_terminal_prefix` | ok (empty top-level `output`) |
| `cargo test --locked -p aster-acp --test v912_seam v912_pasted_image_is_fact_by_ref` | ok — `ORACLE pasted-image by-ref stub-after-tools ok` |
| `cargo fmt -p aster-core -p aster-effect -p aster-acp` | applied |

## Explicitly not claimed

- Reproduced OOM/jetsam or stack_overflow on the original kstack paste session
  with this binary.
- ACP exit-signal observability (pager still surfaces “transport closed” only).
- Migration rewrite of already-written legacy `user.message` facts (inline
  `data` remains readable; load may persist a disk copy).
- Workspace index WARN storms or Studio tao launch panics.
- `cargo clippy -D warnings` workspace-green (pre-existing lints remain).
