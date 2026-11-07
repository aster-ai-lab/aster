# Core / ACP release-convergence verification

## Scope

Only `crates/code/core` and `crates/code/acp` behavior and tests were in scope.
No `apps/cloud` source was changed and no full-repository indexing rebuild was
run in this pass. The retained changes cover bounded grep execution, sparse
aster-vec delta primary keys, and typed reconciliation failure counts.

## Tests

Environment: Darwin arm64, rustc 1.97.1, target directory
`target/grep-budget-tests`.

- Core local workspace: **43 passed, 0 failed**.
- Core retrieval: **188 passed, 0 failed**.
- ACP unattended end-to-end: **33 passed, 0 failed**.
- ACP stdio conformance: **14 passed, 0 failed**.
- Total: **278 passed, 0 failed**.
- `cargo clippy --locked -p aster-core --all-targets --no-deps`: exit 0, with
  existing warnings only.
- `cargo clippy --locked -p aster-acp --all-targets --no-deps`: exit 0, with
  existing warnings only.
- `python3 tests/workspace_dependencies.py`: **7 tests, OK**.
- Scoped rustfmt and `git diff --check`: passed.

Primary commands:

```bash
CARGO_TARGET_DIR=target/grep-budget-tests CARGO_BUILD_JOBS=8 \
  cargo test --locked -p aster-core --lib workspace::local::tests -- --test-threads=1
CARGO_TARGET_DIR=target/grep-budget-tests CARGO_BUILD_JOBS=8 \
  cargo test --locked -p aster-core --lib workspace::retrieval::
CARGO_TARGET_DIR=target/grep-budget-tests CARGO_BUILD_JOBS=8 \
  cargo test --locked -p aster-acp --test unattended_e2e -- --test-threads=1
CARGO_TARGET_DIR=target/grep-budget-tests CARGO_BUILD_JOBS=8 \
  cargo test --locked -p aster-acp --test acp_conformance_stdio -- --test-threads=1
```

## Build and real tasks

Initial default-target build attempts reached the 300-second tool limit during
compilation/linking. The paired build subsequently completed in
`target/grep-budget-tests` with `RUSTC_WRAPPER=` and reduced parallelism. CLI
and ACP both report
`aster-cli-pair-v1:d88830a94fddaa8f5c043ce525ea51a70cc659a9:aarch64-apple-darwin`.
The pinned sandbox download cache was reused after one upstream URL returned
404; all staged sandbox files then passed `aster_confine.py verify`.

- Immediate incident grep through the real CLI→ACP path: exit 0 in
  **63.973 seconds**.
- Full repository-analysis task through the real CLI→ACP path: exit 0 in
  **98.502 seconds**.
- `scripts/package-aster-cli.sh macos` produced
  `aster-cli-v0.1.8-macos.tar.gz` with **13 required members**, including the
  shared BUILD_ID, SHA256SUMS, CLI, ACP, and complete sandbox companions.
- Archive SHA-256:
  `69c5712f891f75e5a26444aceed46bd44aef533ec8a6d9fed41d7df92f9da747`.
- `tests/cli_sandbox_smoke.py` passed after isolated installation: actual tool
  exit 0 and output `ASTER_RELEASE_SANDBOX_OK`, `Linux`, and the exact random
  workspace nonce.
- The same smoke test with `--mode yolo` passed with `Darwin` and its exact
  random workspace nonce.
- No Git commit, version bump, release tag, or upload was created.

## Explicitly not claimed

- No default-target clean-build claim is made; the qualified paired build and
  archive were produced in `target/grep-budget-tests`.
- Full `cargo fmt --all -- --check` is blocked by an unrelated existing diff
  in `apps/studio/src-tauri/src/providers_local.rs`; that file was left
  untouched.
- No Windows or Linux build/runtime verification was performed. Linux here is
  only the macOS sandbox guest's reported system, not a Linux host release
  qualification.
- No default-budget full-index Ready claim or release publication claim is
  made. The archive is a local release candidate, not a published release.
