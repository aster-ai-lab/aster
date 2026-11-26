# Outcome: Core/ACP 9.3.2 & Aster CLI 0.4.2 Release

## Release Scope

- **Core / ACP**: `9.3.2` (`aster-core`, `aster-acp`, tag `aster-acp-v9.3.2`, `--latest=false`)
- **Aster CLI**: `0.4.2` (`aster`, tag `aster-cli-v0.4.2`, `--latest=false`)
- **Channel manifests**: `aster-cli-stable` and desktop Latest compat manifest (`v0.2.4` `latest.json` without `linux-arm64`)
- **Manual**: `crates/code/manual/RELEASE_9.3.2.md` (indexed in `crates/code/manual/README.md`)
- **Submodule isolation**: `apps/studio` untouched and excluded from parent commits and release staging
- **Included changes**:
  - Version bump (`crates/code/core/Cargo.toml`, `crates/code/acp/Cargo.toml`, `apps/aster/Cargo.toml`, `Cargo.lock`)
  - E07 / E46 / E54 crash-recovery & continuation reconciliation fixes
  - First-principles token & KV-cache optimization Rounds 1–7 (`docs/evidence/code/2026-11-26-first-principles-token-optimization/OUTCOME.md`)
  - Pre-release surgical fixes: `crates/code/core/src/prompts.rs` (`turn_output_language_override` keeping `"Use {language} only as fallback."`) and `crates/code/core/src/agent/llm_invoker/fact_governance.rs` (`stream_idle_timeout()` semantic-progress watchdog)

## Pre-Commit Verification (Darwin aarch64, after final Rust edits)

| Check / Suite | Command | Result |
| --- | --- | --- |
| Workspace formatting | `cargo fmt --all -- --check` | Passed (exit 0) |
| Git whitespace / conflict check | `git diff --check` | Passed (exit 0) |
| Core clippy | `cargo clippy --locked -p aster-core --all-targets --no-deps` | Passed (exit 0) |
| ACP clippy | `cargo clippy --locked -p aster-acp --all-targets --no-deps` | Passed (exit 0) |
| Prompts unit slice | `cargo test --locked -p aster-core --lib -- prompts::` | 48 passed; 0 failed |
| Token optimization measurements | `cargo test --locked -p aster-core --lib token_optimization_measurements_report -- --nocapture` | 1 passed; 0 failed (`standing_bytes=5247 lang_override_bytes=384 pre_analysis_sys_bytes=480 pre_analysis_full_bytes=1579 tool_count=14 raw_defs_bytes=14081 raw_schema_bytes=11047 wire_schema_bytes=8111 schema_saved_bytes=2936 openai_wire_tools_bytes=11594 full_session_raw_bytes=19621 full_session_wire_bytes=16458`) |
| Fact governance unit slice | `cargo test --locked -p aster-core --lib fact_governance` | 10 passed; 0 failed |
| Manual index contract | `cargo test --locked -p aster-core --test manual_index` | 3 passed; 0 failed |
| Core targeted qualification slice | `RUST_MIN_STACK=8388608 cargo test --locked -p aster-core --lib -- agent_api::goal_facade::tests::execution::feedback convergence prompts::tests final_delivery confirmation_snapshot completion_attestor --test-threads=1` | 86 passed; 0 failed (5966 filtered out) |
| ACP stdio conformance | `cargo test --locked -p aster-acp --test acp_conformance_stdio -- --test-threads=1` | 15 passed; 0 failed |
| ACP shipped TUI E2E | `cargo test --locked -p aster-acp --test shipped_tui_e2e -- --test-threads=1` | 41 passed; 0 failed; 2 ignored |
| Aster CLI binary unit suite | `cargo test --locked -p aster --bin aster -- --test-threads=4` | 57 passed; 0 failed |
| Release & workspace governance scripts | `python3.12` on `tests/{release_governance,code_release_workflow,cli_release_workflow,workspace_dependencies,cli_pair_delivery,cli_pair_identity,aster_cli_linux,cli_musl,studio_checkout}.py` + `bash -n scripts/{install,package-aster-cli,publish-aster-cli}.sh` | Passed (18 + 7 + 23 + 7 + 23 + 3 + 28 + 2 + 1) |

## Local Candidate Packaging & Smoke (Darwin aarch64, pre-tag)

- **CLI pair archive**: `target/release-prep/cli-0.4.2-acp-9.3.2/macos-arm64-dev/aster-cli-v0.4.2-macos.tar.gz`
  - SHA-256: `feeb0bedae5c38cefff3345aad1df5557d2f3329925dc34dc4bbd92781d6455f`
  - Members: 7 regular members (`aster`, `aster-acp`, `BUILD_ID`, `SHA256SUMS`, 3 `embedding/*` files)
  - Unpacked `--version`: `aster 0.4.2 (d75f49664724)` / `9.3.2`
  - Matching `--build-id`: `aster-cli-pair-v1:a8cd2da1f5702aa4b30a8854e9b009716b06f82b:aarch64-apple-darwin`
  - Checksum verification (`shasum -a 256 -c SHA256SUMS`): `aster: OK`, `aster-acp: OK`
  - Unpacked TUI/ACP PTY E2E smoke (`ASTER_TUI_BIN=.../aster ASTER_ACP_BIN=.../aster-acp cargo test --locked -p aster-acp --test shipped_tui_e2e -- shipped_tui_launch_resume_and_visible_commands --exact --nocapture`): 1 passed; 0 failed (68.13s)
- **Standalone ACP archive**: `target/release-prep/cli-0.4.2-acp-9.3.2/macos-arm64-dev/aster-acp-v9.3.2-macos.tar.gz`
  - SHA-256: `1abb8777b9bfbb73b3fe22dd18c6b6882ea7ee3fe3c91f006e758453b405b8c2`
  - Live `kimi-01/k3.1-pro` smoke (`release_smoke -- one_real_acp_turn_ends_with_end_turn`): 1 passed; 0 failed (`stop="end_turn" reply="READY" seconds=4.5`, 4.55s)

## Public GitHub Release & CI Execution

*Pending post-commit push, CI qualification, tagging, and publishing.*

## Explicitly Not Claimed (Pre-Commit Checkpoint)

- Cross-platform Windows, Linux x86-64/arm64/musl builds and GitHub Actions workflow runs (`qualify-code.yml`, `qualify-aster-cli.yml`, `release-code.yml`, `release-aster-cli.yml`, `release-aster-cli-channel.yml`) have not executed yet and will be recorded once completed.
- `README.md` current-release banner update is intentionally deferred until after `aster-cli-v0.4.2` and `aster-cli-stable` are live so `scripts/publish-aster-cli.sh` runs cleanly against the tagged `main` commit.
