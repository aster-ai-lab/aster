# Outcome: Core/ACP 9.3.2 & Aster CLI 0.4.2 Release

## Release Scope & Anchors

- **Release commit (`RELEASE_SHA`)**: `61b8abbb845ebe259a022058e142cb38676702de` (`main`), layered immediately atop prep commit `59bcb6afdc1aaf3c2400dfba364867db1289b69d`
- **Core / ACP**: `9.3.2` (`aster-core`, `aster-acp`)
  - Tag: `aster-acp-v9.3.2` -> `https://github.com/aster-ai-lab/aster/releases/tag/aster-acp-v9.3.2` (`isDraft: false`, `isPrerelease: false`, `--latest=false`)
- **Aster CLI**: `0.4.2` (`aster` + `aster-acp` pair + `embedding/`)
  - Tag: `aster-cli-v0.4.2` -> `https://github.com/aster-ai-lab/aster/releases/tag/aster-cli-v0.4.2` (`isDraft: false`, `isPrerelease: false`, `--latest=false`)
- **Channels & Repository Latest**:
  - `aster-cli-stable` `latest.json` -> `version: "0.4.2"`, 7 assets (`macos`, `linux`, `linux-arm64`, `windows`, `linux-musl`, `linux-musl-arm64`, `kylin-x86_64`)
  - Desktop compat manifest (`v0.2.4/latest.json`) -> `version: "0.4.2"`, 6 assets (`linux-arm64` omitted per contract)
  - Repository `/releases/latest` -> `v0.2.4` (preserved)
  - Historical releases and tags (`aster-acp-v9.2.*`, `aster-cli-v0.3.*`, etc.) preserved untouched
- **Manual**: `crates/code/manual/RELEASE_9.3.2.md` (indexed in `crates/code/manual/README.md`)
- **Submodule isolation**: `apps/studio` untouched and excluded from parent commits and release staging
- **Included changes**:
  - Version bump (`crates/code/core/Cargo.toml`, `crates/code/acp/Cargo.toml`, `apps/aster/Cargo.toml`, `Cargo.lock`)
  - E07 / E46 / E54 crash-recovery & continuation reconciliation fixes
  - First-principles token & KV-cache optimization Rounds 1–7 (`docs/evidence/code/2026-11-26-first-principles-token-optimization/OUTCOME.md`)
  - Pre-release surgical fixes: `crates/code/core/src/prompts.rs` (`turn_output_language_override` keeping `"Use {language} only as fallback."`), `crates/code/core/src/agent/llm_invoker/fact_governance.rs` (`stream_idle_timeout()` semantic-progress watchdog), and commit `61b8abbb8` (`crates/code/acp/src/agent.rs`, `session_open.rs`, `retrieval_seam_tests.rs`: `HOST_SEMANTIC_READINESS_TIMEOUT = 10s` so fast first turns wait boundedly for cold CPU local indexing)

## Local Verification (Darwin aarch64)

| Check / Suite | Command | Result |
| --- | --- | --- |
| Workspace formatting | `cargo fmt --all -- --check` | Passed (exit 0) |
| Git whitespace / conflict check | `git diff --check` | Passed (exit 0) |
| Core & ACP clippy | `cargo clippy --locked -p aster-core --all-targets --no-deps` / `-p aster-acp` | Passed (exit 0) |
| Prompts unit slice | `cargo test --locked -p aster-core --lib -- prompts::` | 48 passed; 0 failed |
| Token optimization measurements | `cargo test --locked -p aster-core --lib token_optimization_measurements_report -- --nocapture` | 1 passed; 0 failed (`standing_bytes=5247 lang_override_bytes=384 pre_analysis_sys_bytes=480 pre_analysis_full_bytes=1579 tool_count=14 raw_defs_bytes=14081 raw_schema_bytes=11047 wire_schema_bytes=8111 schema_saved_bytes=2936 openai_wire_tools_bytes=11594 full_session_raw_bytes=19621 full_session_wire_bytes=16458`) |
| Fact governance unit slice | `cargo test --locked -p aster-core --lib fact_governance` | 10 passed; 0 failed |
| Manual index contract | `cargo test --locked -p aster-core --test manual_index` | 3 passed; 0 failed |
| Core targeted qualification slice (`59bcb6afd`) | `RUST_MIN_STACK=8388608 cargo test --locked -p aster-core --lib -- agent_api::goal_facade::tests::execution::feedback convergence prompts::tests final_delivery confirmation_snapshot completion_attestor --test-threads=1` | 86 passed; 0 failed (5966 filtered out) |
| ACP stdio conformance (`59bcb6afd`) | `cargo test --locked -p aster-acp --test acp_conformance_stdio -- --test-threads=1` | 15 passed; 0 failed |
| ACP shipped TUI E2E (`59bcb6afd`) | `cargo test --locked -p aster-acp --test shipped_tui_e2e -- --test-threads=1` | 41 passed; 0 failed; 2 ignored |
| ACP retrieval seam & PTY retrieval (`61b8abbb8`) | `cargo test --locked -p aster-acp --lib retrieval_seam_tests` && `cargo test --locked -p aster-acp --test shipped_tui_e2e -- retrieval:: --test-threads=1` | 5 passed; 0 failed (seam) / 3 passed; 0 failed (retrieval PTY) |
| Aster CLI binary unit suite | `cargo test --locked -p aster --bin aster -- --test-threads=4` | 57 passed; 0 failed |
| Release & workspace governance scripts | `python3.12` on `tests/{release_governance,code_release_workflow,cli_release_workflow,workspace_dependencies,cli_pair_delivery,cli_pair_identity,aster_cli_linux,cli_musl,studio_checkout}.py` + `bash -n scripts/{install,package-aster-cli,publish-aster-cli}.sh` | Passed (18 + 7 + 23 + 7 + 23 + 3 + 28 + 2 + 1) |

## Local Pre-Tag Live & Candidate Smoke (Darwin aarch64)

- **Local candidate CLI pair** (`d75f49664` + prep tree): `aster-cli-v0.4.2-macos.tar.gz` (SHA-256 `feeb0bedae5c38cefff3345aad1df5557d2f3329925dc34dc4bbd92781d6455f`), `shipped_tui_launch_resume_and_visible_commands` against unpacked `aster` + `aster-acp`: 1 passed; 0 failed (68.13s).
- **Local candidate ACP** live smoke against `kimi-01/k3.1-pro` (`release_smoke -- one_real_acp_turn_ends_with_end_turn`): 1 passed; 0 failed (`stop="end_turn" reply="READY" seconds=4.5`, 4.55s).

## CI Qualification & Release Workflows (`61b8abbb845ebe259a022058e142cb38676702de`)

| Workflow | Run ID | Outcome | Notes |
| --- | --- | --- | --- |
| `Cross-platform text` | `45851037525` | `success` | `ubuntu-latest`, `macos-latest`, `windows-latest` line endings |
| `Qualify aster` (`qualify-code.yml`) | `45851037296` | `success` | `Core/ACP tests (ubuntu-latest)` (19m21s), `Core/ACP portable tests (windows-latest)` (22m44s), `Core/ACP tests (macos-latest)` incl. Deterministic PTY E2E (52m45s) |
| `Qualify aster-cli` (`qualify-aster-cli.yml`) | `45851037297` | `success` | `Aster CLI tests (ubuntu-latest)` |
| `Release aster-acp` (`release-code.yml`) | `45859559824` | `success` | Published `aster-acp-v9.3.2` (`linux`, `linux-musl`, `linux-musl-arm64`, `macos`, `windows`, `SHA256SUMS`) |
| `Release aster-cli` (`release-aster-cli.yml`) | `45859560196` | `success` | Built all 7 CLI pair archives across `windows`, `linux` (+ `kylin-x86_64`), `linux-arm64`, `linux-musl`, `linux-musl-arm64`, `macos-bins` + `macos` |
| `Publish aster-cli channel` (`release-aster-cli-channel.yml`) | `45888111440` | `success` | Updated `aster-cli-stable` and `v0.2.4` compat `latest.json` |

(Historical pre-fix `qualify-code.yml` run `45841579192` on `59bcb6afd` failed 1/41 on `macos-latest` in `retrieval::shipped_retrieval_stdio_executes_embedded_semantic_search` due to `WorkspaceSemanticFallbackReason::Building` race, resolved by `61b8abbb8` and green on `45851037296`.)

## Published GitHub Release Assets & Checksums

### `aster-acp-v9.3.2` (`--latest=false`)

- Release URL: `https://github.com/aster-ai-lab/aster/releases/tag/aster-acp-v9.3.2`
- `SHA256SUMS` (505 B):
  - `9c5ec310870e94973c77403ff353f2c79a3fdab822beae04424d066ed45d12d0  ./aster-acp-v9.3.2-linux-musl-arm64.tar.gz` (54,086,818 B)
  - `19e3ba396463f9aa3b6636e1fde74e15d553b9b934e7420937c9cfcdd21420e6  ./aster-acp-v9.3.2-linux-musl.tar.gz` (54,699,840 B)
  - `99476d4c8ada64ffa47d2e048ee0dbc797314fa551855d3082d22c7ffd57772c  ./aster-acp-v9.3.2-linux.tar.gz` (52,041,144 B)
  - `80758bc6d271a4beeae32adf4833a031bdedfec7a6cf41381c9b9f2d6ef8c29e  ./aster-acp-v9.3.2-macos.tar.gz` (49,917,960 B)
  - `9bafa59ec0080c1411259e71deb4631ecc379740a54a450f556ec91626d2b7ce  ./aster-acp-v9.3.2-windows.zip` (51,372,574 B)

### `aster-cli-v0.4.2` (`--latest=false`)

- Release URL: `https://github.com/aster-ai-lab/aster/releases/tag/aster-cli-v0.4.2`
- Assets (`latest.json`: 990 B):
  - `de8a271fe85f3a1ff1801a465ad3fd07b690fa94ff93e275c6f791af113ca1f5  aster-cli-v0.4.2-macos.tar.gz` (112,441,526 B)
  - `ef66c0bf20b1bbbd60b221c5aa833e9d8764ddc3be817dafe8240e21db0a97c6  aster-cli-v0.4.2-linux.tar.gz` (123,274,129 B)
  - `78061ec96eb61a3d7ae176f0a14ff3b1eac72a6aa9e0ee064f95e1eef75a3367  aster-cli-v0.4.2-linux-arm64.tar.gz` (116,997,795 B)
  - `e896399c4aa1e5e8a2ed763ca53208087f26b6b2bf5e5ebc33cf8ff1ef7f46c8  aster-cli-v0.4.2-linux-musl.tar.gz` (131,954,991 B)
  - `69dad5300c1d364ac94d054efbc1fc466a2a77c205d5587da6682df7dafc3f1d  aster-cli-v0.4.2-linux-musl-arm64.tar.gz` (128,101,053 B)
  - `ef66c0bf20b1bbbd60b221c5aa833e9d8764ddc3be817dafe8240e21db0a97c6  aster-cli-v0.4.2-kylin-x86_64.tar.gz` (123,274,129 B, byte-identical to `linux`)
  - `2cb6d925f54fca9227d9ef8a614c2ba2477da96e2730b5334f934530a68c3cbb  aster-cli-v0.4.2-windows.zip` (115,008,551 B)
- **CI `aster-cli-v0.4.2-macos.tar.gz` unpack smoke**:
  - `aster --version` -> `aster 0.4.2 (61b8abbb845e)`
  - `aster-acp --version` -> `9.3.2`
  - Matching `--build-id` -> `aster-cli-pair-v1:a66ae4d44a4d7e3de7c76c9d4e9391fa7e96ab8a:aarch64-apple-darwin`
  - `shasum -a 256 -c SHA256SUMS` -> `aster: OK`, `aster-acp: OK`
  - `aster-acp --help` -> `Speaks Agent Client Protocol on stdio over aster-core 9.3.2.`
- **Post-publish endpoint & HTTP checks**:
  - `gh api repos/aster-ai-lab/aster/releases/latest --jq .tag_name` -> `v0.2.4`
  - All 16 public URLs (`aster-acp-v9.3.2/{SHA256SUMS + 5 archives}`, `aster-cli-v0.4.2/{latest.json + 7 archives}`, `aster-cli-stable/latest.json`, `v0.2.4/latest.json`) returned `HTTP 200` via `curl -fsSLI`.

## Explicitly Not Claimed

- Live `scripts/install.sh` or `aster update` end-to-end execution from the newly published public GitHub URLs was not executed locally after publish (HTTP 200 reachability, manifest JSON schema/assets, CI Windows install/rollback verification, CI musl static verification, and CI macOS archive unpack/checksum verification cover the contract).
- Native `apps/studio` desktop build/packaging is out of scope (`apps/studio` retains unrelated local modifications and was never committed or staged).
