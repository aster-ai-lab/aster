# Explicit desktop file-content attestation

Status: implementation and focused verification pass; real ACP-to-native-to-renderer acceptance passed.

Reused DiskContentAttestor and the existing descriptor-anchored file observer.
The observer hashes with a 64 KiB buffer and rejects links, special files and
file identity changes during a read. Every recorded path needs a matching
digest. One missing digest or mismatch leaves the completion gate incomplete.
The gate itself is unchanged.

ACP accepts only the explicit `a3sHost.completionAttestation: disk_content`
mode. Absence keeps the previous behavior; unknown modes are rejected. Work
declares the mode at session creation. Source-review sessions do not install
the attestor and retain their deny-all tool boundary.

Verification: 8 core tests, 1 host-metadata test and 7 session-settings tests
pass. Cases include a 64 MiB file, partial multi-file evidence, changed bytes,
absolute/parent paths, symlink escape, unknown opt-in and unchanged old hosts.
Logs: `target/bugfix-attestation-{core,acp,acp-options}.log`.

This checks byte identity at verification time; it does not freeze files
against a later external edit. Rechecking edited bytes refuses the old digest.
Platforms without the shared safe observer fail closed.

The user selected automatic attestation. Manual waivers add a user step;
an OCI guest backend is a larger independent change. Neither replaces this
bounded implementation. No waiver or verification-policy relaxation was added.

Real browser acceptance used the production renderer, native host and actual ACP/core write tool with a loopback scripted model. After user approval, the file bytes were written and the retained `turn.complete` fact has `terminal: verified` and a nonempty effect digest. The composer returned to idle. The first attempt was narrative-only because the parent repository ignored the fixture directory; an owned nested Git fixture corrected that test precondition. Both attempts remain retained. Proof: `../2026-10-20-native-bugfix-roundtrip/bugfix-native-roundtrip-proof.json`.

## Explicitly not claimed

Installation, commits and root/submodule landing remain unperformed.
