# T022 real Work → ACP qualification

The final fresh-companion group passed: **5 passed, 0 failed, exit 0**. The controlled loopback provider supplied tool calls; actual Work launch/config, ACP, Core governance, Office validation, event delivery, original facts and captured artifacts executed. No real model or command VM was started.

```sh
cargo build --locked --offline -p a3s-code-acp
cargo test --locked --offline -p aster-dream --lib agent_plane::turns::local_acp_tests::delivery::program:: -- --ignored --test-threads=1 --nocapture
```

The build completed successfully in `companion-final-build.log`. The final test log is `integration-qualified.log`; machine-readable results, log digests and case observations are `qualified.json`. The actual binary was `/Users/roylin/code/aster/target/debug/a3s-code-acp`, SHA-256 `8e4d312fb2dc6b4e634e472ea87f48e8a8f749050d5a34f774ef8ee36f1ffdaf`. Every case verified its mapped macOS executable, stable pre/post binary digest and actual PID exit after owned cleanup. The same binary served all five cases. The binary freeze window has been released to the parent owner.

| Case | Observed result |
| --- | --- |
| Nested reads → structured check → native document | Canonical fresh code-profile request; 4 timed child receipts and source anchors; native save/reopen proof; one captured DOCX, 2466 bytes, equal to disk and correct content; both inputs unchanged; Work final success. |
| Unrecovered required check failure | Native file physically exists, but original parent Failed plus nested failed check remain visible. Deliberately false success text cannot certify it: Work error, producer failed, no artifact has traced status. |
| Successful recovery | Original failed parent retained; a second successful parent writes a new after identity to the same exact path, reads and checks it. Both original receipts remain. Work succeeds with one captured 16-byte artifact equal to the repaired bytes. |
| Rejected approval | Original exact confirmation.answered fact has approved=false. No execution receipt is manufactured for the refused parent; no nested write or artifact. Persisted approval is false, framework final text is denied, conversation success certifies no artifact. |
| Active nested cancellation | First read completed while program was live. A second nested Office operation was observed waiting for permission; Work.abort cancelled that same actor. Original parent Cancelled with incomplete effects; Work paused, producer cancelled, no artifact, source unchanged, child exited. |

The fresh host ACL is temporary A3S_CONFIG with code presentation and workspace retrieval disabled. An async group Mutex and --test-threads=1 serialize the RAII environment guard. Prior environment values restore on unwind; HOME and user ACL are untouched. The actual provider requests and generated launch ACL verify the trusted profile reaches the child process, beyond Core-only tests.

The required-failure RED was a product defect, not a fixture-only mismatch: `required-failure-isolated-red.log` originally demonstrated a failed parent and nested check followed by automatic byte certification. The shared minimal ledger/attestor repair is explained in `CORE-FAILURE-CONTRACT.md`. Its final focused checks are **14 attestor + 60 ledger + 11 fact-cache = 85 distinct passing tests** in `core-checks.json`. Independent cache review found a remove-negative-and-rehash coverage forgery, reproduced in `failed-effects-cache-red.log` and rejected by the final exact-prefix map comparison. The reviewer rechecked the fix.

Historical attempts also include fixture mistakes: late PID/session capture after error cleanup, expecting a model explanation after a framework short-circuit denial, expecting an execution receipt for an unadmitted refusal, and treating null cancellation exit_code as a process result. These were corrected without weakening typed failure, original denial, cancellation or artifact assertions. Two recovery attempts encountered a controlled-provider transport error; the isolated successful recovery and final complete group are retained. Historical filenames containing green/final do not imply a pass; qualified.json lists them separately.

The earlier tight busy loop never proved its read completed or cancellation was issued. It remains explicitly unqualified in `busy-loop-fixture.json` and the RED logs. The successful active cancellation uses an observable asynchronous child, not a rejected parent approval. No claim is made that the busy-loop interrupt path was qualified by this slice.

The final group wall time was 92.80 seconds, including large binary software hashes, fixture setup and cleanup. This is not a task-speed score. Per-case host_turn_ms includes launch and early identity lookup but excludes hashes, cleanup and background extraction drain. Only successful program metadata supplies per-child intervals; missing failure/cancel child intervals stay unavailable. VM count and provider-reported tokens remain null. This does not establish natural model quality, whole T01 speed, or actual Windows/Linux process behavior.

Final source byte identities are in `source-final.json`, including the separately owned shared tool-visibility fix integrated into this companion. The tree is shared and dirty; the snapshot does not claim exact whole-tree reproducibility. The root owner retains the final paired Aster/ACP/Office/Work build and subsequent frozen matrix/UI qualification.
