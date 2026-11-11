# ACP native configuration overlays

Status: the scoped native change and the KRun Aster binding task passed.
The complete workspace and Harness acceptance gates remain open.

`aster-acp` accepts repeatable `--config-overlay FILE` arguments and appends these
explicitly authorized ACL layers through the existing configuration loader.
The original base discovery, workspace admission, model selection, native MCP
manager and permission behavior remain in their owning implementations.
The existing public loader continues to call the new loader with no overlays.

Changes cover configuration, library export, ACP argument parsing and help, and
focused configuration and parser tests. No Studio or core permission code changed.

## Checks

All native builds and execution used KBox Linux/aarch64, Rust 1.97.1, root Cargo
workspace and the existing release-dist profile:

```sh
cargo test --locked --profile release-dist -p aster-acp --no-default-features --features svg-png,account-import --lib config:: -- --nocapture
cargo test --locked --profile release-dist -p aster-acp --no-default-features --features svg-png,account-import --bin aster-acp configuration_overlays -- --nocapture
cargo clippy --locked --offline -p aster-acp --no-default-features --features svg-png,account-import --all-targets --no-deps
cargo build --locked --offline --profile release-dist -p aster
cargo build --locked --offline --profile release-dist -p aster-acp --no-default-features --features svg-png,account-import
bash scripts/package-aster-cli.sh linux-arm64 /target/release-dist /out/aster-cli-linux-arm64.tar.gz
cargo fmt -p aster-acp -- --check
```

Configuration tests: 31 passed. Full compiled ACP argument test executable:
10 passed; this includes the separately filtered test, which is not counted twice.
Clippy passed with existing warnings. Paired release build, package formatting,
ELF dependency inspection and native overlay help passed.
Both binaries require at most GLIBC 2.30; the KRun package declares 2.31.

The reusable source snapshot is based on `6a8bf40f3d10101165e18cf0f60fbd17e6213f81`
and is dirty. All five changed feature paths match this checkout; the change origin
HEAD was `78ca852039eb16e94a6080fc43dc58be4274907a`.
The 14,007-entry snapshot digest is
`sha256:2d352dbb89c79ccd85b1f51d7fd1913746b7d46bb505f80a6c40ca73181d9491`.
Two existing root test fixtures were copied verbatim to the partial snapshot so
all-target Clippy could run. The source bytes were rechecked after the build.

KRun's final `aster/acp-0.1.4.zip` used archive SHA256
`075b52a5c934e9cd23c27724cbc608eb69e15125432bf23921209fed3d4cbd96`.
The final package SHA256 is
`0b3bae59d6045a074226275d14cbb12590dd60631397f7e0a5b63e4a1ab71ad7`.
Its real external DeepSeek task recorded native Skill, PATH tool and registered
MCP calls, checked randomized results in an explicitly mounted external directory,
and ended with zero private executions and volumes.

Commands, actual logs, retained failed attempts, source patch, hashes, receipts
and credential cleanup checks live in
[KRun's evidence](../../../../../kense/krun/docs/evidence/2026-11-11-native-bindings-aster-final/OUTCOME.md).

## Existing failures and limits

Whole-workspace `cargo fmt --all -- --check` reports 18 existing paths outside
ACP. The offline all-target workspace check stopped because `ab_glyph 0.2.32`
was not cached. Neither is reported as passed.
The first build lacked a locked crate; two new tests initially asserted incorrect
native path semantics; a later build lacked existing root test fixtures.
The fixed tests preserve the native trust rules, and all failed outputs are retained.
The first KRun package had incorrect repository metadata and was retained;
the corrected version reran the real task.

## Explicitly not claimed

No whole-workspace green result, original OAuth mutation, authentication refresh,
complete long-running Harness acceptance, managed model service, local inference,
benchmark, release or push is claimed. No weights or inference backend were added.
