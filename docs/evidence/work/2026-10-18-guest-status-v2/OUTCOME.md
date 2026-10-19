# Guest command report compatibility

Aster now requires the corrected v2 guest command report. Legacy v1 worker
exit reports cannot prove payload completion, even when they claim VM entry
and exit code zero. The source and regression test match original Core
byte-for-byte.

## Validation

The original Core regression failed before the change. The Guest family then
passed in both checkouts: 9 passed, 1 explicitly ignored real-HVF test each.
The duplicate Aster red run was cancelled while queued after the identical
original-source regression had reproduced the bug; it is not counted as a
completed test.

The producer change is signed commit `df73f4914`. Its full authenticated OCI
Guest path has passed a real exit-7 payload with output and cleanup checks.
The public Host Service qualification also completed with exit zero: lifecycle,
owner-death recovery, and 25 soak iterations (25 VM generations) passed.
Descriptor counts stayed at 13 and every VM/endpoint/transient cleanup check
passed. `public-host-service.json` binds the report to the exact executable,
shim, system image, and producer revision. These are SDK/runtime checks, not
a proof that Aster has integrated that backend for Bash.

The original Core compatibility commit is `181fb24a`. Aster Core strict
Clippy passed; its retained log is `core-clippy.log.gz`.

## Explicitly not claimed

No complete macOS Bash integration, Core hardware Guest E2E, released runtime
artifact, or automatic desktop process replacement. Aster's other uncommitted
changes are not part of the original Core compatibility commit.
