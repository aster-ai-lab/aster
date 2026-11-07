# OUTCOME: Aster harness without in-tree sandbox

Date: 2026-11-07

## Change

Aster root workspace no longer ships `kbox`, OCI companions, or an
enforcing sandbox runtime (API retained as the `aster-confine` stub). CLI archives are the `aster` + `aster-acp`
pair only. Command isolation is external (kbox).

## Commands run

- `cargo check -p aster-core`
- `cargo check -p aster`
- `cargo test -p aster --bin aster cli_pair_update -- --test-threads=4`
- `cargo test -p aster-core --lib sandbox:: -- --test-threads=4`
- `cargo test -p aster --bin aster cli_pair_update -- --test-threads=4`
- `python3 tests/cli_pair_delivery.py`
- `python3 tests/cli_release_workflow.py`

## Results

- `aster-core` / `aster` compile.
- `aster-core` sandbox module tests: 27 passed.
- `cli_pair_update` unit tests: 10 passed.
- `cli_pair_delivery.py`: 15 passed.
- `cli_release_workflow.py`: 15 passed.

## Explicitly not claimed

- `apps/cloud` still vendors its own `crates/box` / `crates/sandbox` copies in
  the cloud workspace; that workspace was not migrated to kbox in this change.
- Studio submodule UI copy mentioning kbox was not rewritten.
- Full workspace `cargo test --workspace` was not run.
- End-to-end kbox integration adapter inside Core was not added; default
  native BashSandbox refuses until a host injects a runner, process-host, or
  yolo path.

## Follow-up thorough scrub (same day)

- Renamed stub crate `aster-sandbox` → `aster-confine`.
- Removed cloud vendored `crates/box`, `crates/sandbox`, `crates/oci-runtime`.
- Scrubbed product docs, plugins registry catalog/locks, web sandbox docs, runner/gateway naming to kbox.
- Node-agent main now refuses with an external-kbox message; box_runtime modules removed.

### Still historical only

- Frozen `docs/evidence/**` and some `apps/cloud/docs/evidence/**` blobs may still contain the old names as recorded artifacts.

## Thorough scrub completion

### Removed / renamed
- Stub `aster-sandbox` → `aster-confine` (codegen path).
- Cloud vendored `crates/box`, `crates/sandbox`, `crates/oci-runtime` (and `oci-spec` path patch).
- Cloud tools: `build_aster_box_linux.sh`, Darwin sandbox host probe / enroll scripts.
- Web sandbox docs pages; test now asserts they are gone.

### Cloud compile posture
- `cargo check -p aster` (root): pass.
- `cargo check -p aster-cloud-control-plane` (apps/cloud): pass after local `legacy_box_plan` stand-in and compose-test rewrite.
- `cargo check -p aster-cloud-node-agent`: pass; `box_host` cfg never enabled; `box_build` / `box_port` are fail-closed stubs; binary still refuses at `main`.

### Intentional residual strings
Comments and architecture-test forbidden prefixes that document the removal (`legacy_box_plan`, `kbox_runtime::` forbid list, node-agent retirement notes). Frozen `docs/evidence/**` blobs unchanged.

### Explicitly not claimed
- No end-to-end kbox adapter for Core BashSandbox.
- Cloud BoxBuild digest/ACL canonicalization is a local stand-in, not the retired aster-box-runtime algorithm.
- Cloud node-agent does not execute box builds.
