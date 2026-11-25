# Local preparation for Core / ACP 9.3.1 and CLI 0.4.1

The package manifests and workspace lockfile agree on Core / ACP `9.3.1`
and CLI `0.4.1`. `cargo update --workspace --offline` changed only those
three versions in the lockfile. `cargo metadata --locked --offline --no-deps`
confirmed that all three packages retain `publish = false`. The coordinated
source release notes are in [RELEASE_9.3.1.md](../../../../crates/code/manual/RELEASE_9.3.1.md).

GitHub and the remote tag refs confirmed that `aster-acp-v9.3.1` and
`aster-cli-v0.4.1` were available. The published references were
`aster-acp-v9.3.0` and `aster-cli-v0.4.0`. The worktree was based on
`1e5aaae5561a42785a77a17d5c748aa82ba368db` and contained the uncommitted
Core, ACP and CLI changes. The Studio submodule pointer stayed at its
existing commit.

## Release gate

`python3 tests/cli_release_workflow.py -v ReleaseAssembly.test_current_release_requires_both_musl_archives`
reproduced a new-version draft being accepted with four archives. The
publisher now requires both musl architectures starting with CLI `0.4.0`.
The existing older-run fixture still accepts its historical four-archive
shape. A missing archive fails before the script writes a manifest or
creates a draft.

## Native local candidates

Built from the repository root on macOS 27.2.1 arm64 with pinned Rust 1.97.1
and the development profile. The CLI pair used the distribution feature
selection and the standalone ACP used default features.

```sh
env -u GROK_VERSION cargo build --locked -p aster --bin aster
env -u GROK_VERSION cargo build --locked -p aster-acp --bin aster-acp --no-default-features --features svg-png,account-import
env -u GROK_VERSION cargo build --locked -p aster-acp --bin aster-acp
bash scripts/package-aster-cli.sh macos "$PAIR_DIR" "$CLI_ARCHIVE"
```

Both CLI binaries reported the same identity
`aster-cli-pair-v1:9c94c3037a653f50cd66f1cdee233c76bb43745d:aarch64-apple-darwin`.
The archive contained seven regular files. The stock installer checked its
digest and pair manifests and installed it into an isolated home. A fresh
zsh found both commands through the generated startup configuration.
`--version` reported CLI `0.4.1` and ACP `9.3.1`. The pair passed help
checks and the three offline embedding files were installed. The standalone
ACP archive contained one executable, and its help reported Core `9.3.1`.

| Local archive in `target/release-prep/cli-0.4.1-acp-9.3.1/macos-arm64-dev/` | SHA-256 |
| --- | --- |
| `aster-cli-v0.4.1-macos.tar.gz` | `8c84ce242f7dbef368200c2e1f33afe33847d8736d862126473d20a8a86627e9` |
| `aster-acp-v9.3.1-macos.tar.gz` | `e68274ba0890c358ce2dec1401e2517539085deb82bd31bb0d2664bab16a9829` |

## Verification

- Python 3.12 passed `tests/release_governance.py` (18),
  `tests/cli_release_workflow.py` (22), `tests/code_release_workflow.py`
  (7), `tests/cli_pair_delivery.py` (23), `tests/cli_pair_identity.py` (3),
  `tests/aster_cli_linux.py` (28), and `tests/workspace_dependencies.py` (7).
  The system Python 3.9 could not import `tomllib` for the Core workflow test.
- `cargo fmt --all -- --check`, `git diff --check`, and Bash syntax checks
  for the installer, packager and publisher passed.
- `RUST_MIN_STACK=8388608 cargo test --locked -p aster-core --lib -- goal_facade::tests compaction::tests compaction_budget_tests compaction_carry agent_api::tests::compact async_mcp mcp::manager::tests llm_invoker --test-threads=1`
  passed 290 focused Core tests.
- `RUST_MIN_STACK=8388608 cargo test --locked -p aster-acp --lib --test acp_conformance_stdio -- --test-threads=1`
  passed 555 ACP unit tests with 10 ignored and all 15 stdio conformance tests.
- The compiled `shipped_tui_e2e` harness ran
  `shipped_tui_launch_resume_and_visible_commands --exact --nocapture`
  with `ASTER_TUI_BIN` and `ASTER_ACP_BIN` pointing to the pair installed
  from the candidate archive. It passed in 86.53 seconds.
- `cargo test --locked -p aster-acp --test release_smoke --no-run` built
  the real-provider smoke harness. Its
  `one_real_acp_turn_ends_with_end_turn --ignored --exact --nocapture`
  case passed in 14.14 seconds with the standalone candidate selected by
  `ASTER_RELEASE_SMOKE_BIN` and model `kimi-01/k3.1-pro`. ACP returned
  `READY` with `end_turn`. Its state was isolated and the operator ACL
  checksum matched before and after the run.
- `cargo check --locked --workspace --all-targets` stopped because the
  local Swift toolchain could not load `SwiftUIMacros.StateMacro` for the
  existing capture helper.
- `SDKROOT="$(xcrun --sdk macosx26.5 --show-sdk-path)" cargo check --locked --workspace --all-targets`
  passed in 2 minutes 6 seconds with the installed Command Line Tools.
  The resulting `capture-helper` targets macOS 15 and passed a QR smoke test.

## Explicitly not claimed

- Clean-source release builds, GitHub branch qualification, formal platform
  archives, or native Linux and Windows execution for these candidates.
- Publication, tags, stable-channel updates, or changes to the desktop Latest.

## macOS release cache recovery

The first [CLI tag build](https://github.com/aster-ai-lab/aster/actions/runs/45587301980)
passed five platform lanes. Its macOS lane restored no cache and reached
the 180-minute job deadline while building `aster`. The same release
workflow can prewarm the macOS cache on `main` for the original tag
commit. A subsequent rerun of the original macOS job retains the source
tag and the other platform archives. The warm run stores build cache only.

With Python 3.12, `tests/cli_release_workflow.py` passed 23 tests and
`tests/release_governance.py` passed 18. The recovery test covers a valid
tag run and refuses a non-tag run, a different workflow, a changed tag,
and a source outside main.
