# T022 controlled Work → real ACP fixture

Status: final fresh-companion group passed all five cases (5 passed, 0 failed). See qualified.json and integration-qualified.log. All initial RED logs are retained. The parent OUTCOME.md is maintained by the root owner.

Test files are `apps/work/src-tauri/src/agent_plane/local_acp_delivery_tests.rs` and its new sibling `local_acp_program_tests.rs`. Only the loopback provider is scripted. Work launches this workspace's real ACP/Core/Office. No real model service or command VM is required.

Each fresh Work session selects a temporary trusted host ACL through A3S_CONFIG (code presentation, workspace retrieval disabled). A group Mutex plus --test-threads=1 serializes the environment guard. The previous variable is restored on unwind. HOME and user ACL are untouched. This is not proof of concurrent process environment mutation safety.

Five cases:

1. Two bounded parallel reads → structured percentage check → native DOCX save/reopen proof; source files unchanged; captured document bytes equal saved bytes and final reply.
2. Native write → unrecovered required check failure → deliberately false success final; original parent Failed and nested failure stay visible; no traced artifact/success certification.
3. Failed parent write/check → explicit second parent repair of the same path with a new after identity → read-back and successful structured check; both original receipts stay retained while repaired bytes can be certified.
4. Actual rejected approval → exact original confirmation.answered approved=false → no execution receipt, write or artifact. The real framework denial text is denied and the Work conversation may end success. This is not a successful artifact certification.
5. Active program executes its first read; a second nested office_file operation is observed waiting for permission; Work.abort cancels the same actor. Original parent outcome is Cancelled with incomplete effects, Work is paused and producer cancelled, no artifact is published, actual child exits. This is distinct from rejecting the parent before it runs.

Each case inspects the actual fresh ACP provider request for only canonical program schema and checks the generated trusted launch ACL. Actual child PID/session are captured while the turn is live because error settlement can remove its runtime. On macOS the mapped executable path must match; the binary digest before/after must be stable; the PID must be gone after owned cleanup. Linux/Windows actual-process qualification remains separate.

Original receipts and denial facts come from the exact ACP session's schema-bound marker and bounded effect log. Native success retains actual child output byte counts, monotonic offsets, durations and source anchors. Per-program offsets are not a global child clock. VM and provider-reported token counts remain null/unavailable; they are not inferred from tool names or estimated bytes.

Host time is Work turn open through terminal, including launch and early process identity lookup. It excludes binary hashes, cleanup and background extraction drain. These are forced mechanism cases, not natural-language performance experiments. Builds and Cargo lock wait are not task latency.

The earlier busy-loop fixture timed out before it proved a completed read or issued cancellation. Its source, limits and unresolved boundary are retained in busy-loop-fixture.json and the corresponding RED logs. ACP runs a multithread runtime plus LocalSet; QuickJS uses spawn_blocking with a private current-thread runtime. The old timeout is not declared a passed interrupt or settlement proof.

Final command (serial, stable fresh companion required):

```sh
cargo test --locked --offline -p aster-dream --lib agent_plane::turns::local_acp_tests::delivery::program:: -- --ignored --test-threads=1 --nocapture
```
