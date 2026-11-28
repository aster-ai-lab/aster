# Extreme Size Optimization: `aster-core`, `aster-acp`, and `aster` CLI

- **Date**: 2026-11-28
- **Base commit**: `1be48a0e2115e8809f7526b56303afd2a9c2e482` (`HEAD` at measurement; pre-existing dirty files preserved untouched)
- **Host / Toolchain**: `Darwin arm64`, `rustc 1.97.1 (8bab26f4f 2026-07-14)`, `cargo 1.97.1 (c980f4866 2026-06-30)`
- **Target features built for distribution**:
  - `aster`: default features (`apps/aster/Cargo.toml`)
  - `aster-acp`: `--no-default-features --features svg-png,account-import` (matches `.github/workflows/release-aster-cli.yml`)
  - `aster-core`: `--no-default-features --features local-code` (embedded by `aster-acp`)

## 1. Before vs. After Size & Dependency Metrics

| Artifact / Metric | Baseline (`release-dist`) | Optimized (`release-dist`) | Delta | Reduction |
| --- | ---: | ---: | ---: | ---: |
| `aster` binary (`stat -f %z`) | 148,298,304 B (141.43 MiB) | 56,323,984 B (53.71 MiB) | -91,974,320 B | **-62.02%** |
| `aster` (`gzip -c \| wc -c`) | 62,828,249 B (59.92 MiB) | 30,982,660 B (29.55 MiB) | -31,845,589 B | **-50.69%** |
| `aster-acp` binary (`stat -f %z`) | 48,560,976 B (46.31 MiB) | 21,265,632 B (20.28 MiB) | -27,295,344 B | **-56.21%** |
| `aster-acp` (`gzip -c \| wc -c`) | 21,707,785 B (20.70 MiB) | 12,202,234 B (11.64 MiB) | -9,505,551 B | **-43.79%** |
| Combined pair (`aster + aster-acp`) | 196,859,280 B (187.74 MiB) | 77,589,616 B (73.99 MiB) | -119,269,664 B | **-60.59%** |
| Packaged macOS `.tar.gz` (`scripts/package-aster-cli.sh`) | 115,127,361 B (109.79 MiB) | 73,772,156 B (70.35 MiB) | -41,355,205 B | **-35.92%** |
| `aster` unique crates (`cargo tree --edges normal --prefix none \| sort -u`) | 1,581 | 1,315 | -266 | -16.82% |
| `aster-acp` unique crates (`--no-default-features --features svg-png,account-import`) | 832 | 764 | -68 | -8.17% |
| `aster-core` unique crates (`--no-default-features --features local-code`) | 556 | 488 | -68 | -12.23% |

Notes:
- The packaged macOS `.tar.gz` includes the three unchanged Model2Vec weight/tokenizer assets (`embedding/potion-code-16M-v2.safetensors`, `embedding/potion-code-16M-v2.tokenizer.json`, `embedding/potion-code-16M-v2.config.json`) alongside `aster`, `aster-acp`, `BUILD_ID`, and `SHA256SUMS`.
- `target/release-dist/deps/libaster_core-d2b221233a28e493.rlib` measured `200,902,000 B` (vs baseline range `181,251,128`–`190,005,424 B`) because `lto = "fat"` stores full LLVM bitcode inside intermediate `.rlib` archives to enable whole-program dead-code elimination across linked executables.

### Mach-O Segment Breakdown (`size -m`)

| Binary & Segment/Section | Baseline (B) | After (B) | Delta |
| --- | ---: | ---: | ---: |
| `aster` `__TEXT` | 139,722,752 | 51,625,984 | -63.05% |
| `aster` `__TEXT.__text` | 112,858,792 | 36,830,956 | -67.37% |
| `aster` `__TEXT.__const` | 16,588,048 | 13,409,872 | -19.16% |
| `aster` `__TEXT.__eh_frame` | 8,331,096 | 143,104 | -98.28% |
| `aster` `__DATA_CONST` | 4,800,512 | 3,489,792 | -27.30% |
| `aster` `__DATA` | 1,376,256 | 1,359,872 | -1.19% |
| `aster` `__LINKEDIT` | 3,538,944 | 966,656 | -72.69% |
| `aster-acp` `__TEXT` | 45,088,768 | 18,989,056 | -57.88% |
| `aster-acp` `__TEXT.__text` | 37,910,400 | 15,621,928 | -58.79% |
| `aster-acp` `__TEXT.__const` | 3,516,744 | 2,660,680 | -24.34% |
| `aster-acp` `__TEXT.__eh_frame` | 2,839,140 | 72,392 | -97.45% |
| `aster-acp` `__DATA_CONST` | 2,080,768 | 1,867,776 | -10.24% |
| `aster-acp` `__DATA` | 81,920 | 65,536 | -20.00% |
| `aster-acp` `__LINKEDIT` | 1,310,720 | 360,448 | -72.50% |

### `[profile.release-dist]` Benchmark (`opt-level = "z"` vs `opt-level = "s"` with `lto = "fat"`)

| Profile Setting (`codegen-units = 1`, `strip = true`, `panic = "abort"`) | `aster` (B / gzip B / build time) | `aster-acp` (B / gzip B / build time) |
| --- | --- | --- |
| `opt-level = "z"`, `lto = "fat"` (**selected**) | 56,290,960 / 30,972,757 / 8m 29s | 21,265,632 / 12,202,232 / 3m 55s |
| `opt-level = "s"`, `lto = "fat"` | 86,054,704 / 38,358,215 / 9m 49s | 31,941,728 / 15,090,495 / 5m 08s |

(`opt-level = "z"` with `lto = "fat"` was chosen; final `--locked` rebuild after formatting/telemetry cfg cleanups produced `aster = 56,323,984 B` and `aster-acp = 21,265,632 B` in 7m 12s for `aster`.)

## 2. Structural Optimizations Applied

1. **`aster-core` & `aster-use-browser`**:
   - Added `crates/browser/src/cdp.rs` (lightweight `tokio-tungstenite` JSON-RPC `CdpConnection` and `LocalCdpEndpoint`) behind `quark-cdp` so `aster-core` no longer pulls `chromiumoxide` into `aster-acp` or `aster`.
   - Replaced `html2text` (`html5ever`/`markup5ever`) in `crates/code/core/src/tools/builtin/web_fetch.rs` with existing `htmd` + `pulldown-cmark`.
   - Replaced `sha256` crate in `crates/code/core/src/content_digest.rs` and callers with `sha2` (`content_digest::sha256_hex`).
   - Disabled `aster-lane` `telemetry` (`opentelemetry 0.21`) while keeping `metrics`, `monitoring`, and `distributed`.
   - Unified duplicated `base64`, `toml`, `thiserror`, and `zip` versions across `aster-core` and `aster-okf`.
2. **`aster-acp` (`aster-embedding`, `aster-office`)**:
   - Switched `crates/embedding/Cargo.toml` to `aster-core` (`default-features = false`) and `model2vec-rs` (`default-features = false, features = ["fancy-regex"]`), eliminating `onig_sys`, `hf-hub`, and `ureq`.
   - Gated `crates/office/crates/core` `collaboration` (`yrs`) and `install` (`reqwest`) behind `default = ["collaboration", "install"]` so `aster-acp` (`default-features = false`) omits both without affecting `aster-office` defaults, `aster-studio`, or `aster-office-cli`.
3. **`aster` CLI (`aster-pager`, `aster-file-utils`, `aster-tools`, `aster-otel`, `aster-telemetry`, `aster-markdown`, `aster-shell`, `apps/aster`)**:
   - Decoupled `aster-pager` runtime dependencies from `aster-core` (`crates/terminal/crates/codegen/aster-pager/Cargo.toml` moved `aster-core` to `[dev-dependencies]`): introduced local `AccountImport` mirror (`account_import.rs`), `rewrite_top_level_scalars` + `validate_acl_document` (`document.rs`), and direct `aster-acl` rewrites in `model_preferences.rs`, dropping `aster-core` and `chromiumoxide` completely from the `aster` binary graph.
   - Removed unused `gcloud-storage` from `aster-shell`, unused `tonic` from `aster-tools`, and gated `pdf_oxide` behind `pdf-read` in `aster-tools` (keeping an upfront `MAX_PDF_BYTES` size check and `serde_json/preserve_order` for standalone tests).
   - Gated `aws-sdk-s3`, `aws-config`, `aws-smithy-http-client`, and `gcloud-storage` behind optional `cloud-storage` in `aster-file-utils` while keeping `UploadMethod::Proxy` enabled.
   - Switched `opentelemetry-otlp` in `aster-otel`/`aster-telemetry` to `http-proto` + `reqwest-blocking-client` + `trace` + `metrics` + `logs` + `tls-roots` and gated `grpc-tonic`/`tls-aws-lc`/`webpki-roots` behind optional `otlp-grpc`.
   - Narrowed `apps/aster/Cargo.toml` dependency on `aster-use` to `default-features = false, features = ["extensions"]`.
   - Switched `aster-markdown/build.rs` & `src/syntax.rs` from `dump_to_uncompressed_file` to `dump_to_file`/`from_binary` (`syntaxes.bin` shrank from `912K` to `787K`), while keeping `syntect`/`two-face` on Oniguruma after testing showed `fancy-regex` broke `syntax_backend_tests`.
4. **Release Distribution Profile (`Cargo.toml`)**:
   - Updated `[profile.release-dist]` only (leaving `[profile.release]` untouched) to `opt-level = "z"`, `lto = "fat"`, `codegen-units = 1`, `strip = true`, `panic = "abort"`.

## 3. Verification Commands & Observed Results

- **Runtime pair contract & archive**:
  - `target/release-dist/aster --version` -> `aster 0.4.2 (1be48a0e2115)`
  - `target/release-dist/aster-acp --version` -> `9.3.2`
  - Both `--build-id` -> `aster-cli-pair-v1:f46210917773a535ff3102843880b5fd0e610a64:aarch64-apple-darwin`
  - `bash scripts/package-aster-cli.sh macos target/release-dist /tmp/aster-cli-after-macos.tar.gz` -> exit 0, `73,772,156 B`, verified members (`aster`, `aster-acp`, `BUILD_ID`, `SHA256SUMS`, `embedding/potion-code-16M-v2.{safetensors,tokenizer.json,config.json}`), then removed temp tarball.
- **Python guards & metadata/formatting**:
  - `cargo metadata --locked --format-version=1 >/dev/null` -> pass
  - `rustfmt --edition 2024 --check` on touched Rust files -> pass (repo-wide `cargo fmt --all -- --check` also hits pre-existing formatting differences in untouched workspace files)
  - `/opt/homebrew/opt/python@3.12/bin/python3.12 tests/workspace_dependencies.py` -> 7 tests OK
  - `/opt/homebrew/opt/python@3.12/bin/python3.12 tests/cli_release_workflow.py` -> 23 tests OK
  - `/opt/homebrew/opt/python@3.12/bin/python3.12 tests/cli_pair_delivery.py` -> 23 tests OK
- **`--locked` test & check suite**:
  - `cargo test --locked -p aster-use-browser --no-default-features --features quark-cdp` -> 4 passed, 0 failed, 2 ignored (`live_*`)
  - `cargo test --locked -p aster-use-browser` -> 23 passed, 0 failed, 2 ignored (`live_*`)
  - `cargo test --locked -p aster-embedding` -> 22 passed, 0 failed
  - `cargo check --locked -p aster-office --no-default-features` -> pass
  - `cargo test --locked -p aster-office --lib collaboration::tests::collaboration_store_is_send_and_sync` -> 1 passed, 0 failed
  - `cargo test --locked -p aster-okf` -> 94 passed, 0 failed
  - `cargo test --locked -p aster-markdown` -> 498 passed, 0 failed
  - `cargo test --locked -p aster-file-utils --lib -- --test-threads=4` -> 185 passed, 0 failed (note: 3 `queue::tests::drain_*` tests flaked under default unconstrained test concurrency before passing at `--test-threads=1` 5/5 and `--test-threads=4` 185/185)
  - `cargo test --locked -p aster-tools --lib -- pdf mcp_elicitation` -> 67 passed, 0 failed
  - `cargo test --locked -p aster-telemetry --lib` -> 281 passed, 0 failed
  - `cargo test --locked -p aster-core --no-default-features --features local-code --lib -- web_fetch quark_account content_digest session_lane_queue` -> pass
  - `cargo test --locked -p aster-pager --lib -- model_preferences account_import` -> 8 passed, 0 failed
  - `cargo test --locked -p aster --bin aster -- --test-threads=4` -> 57 passed, 0 failed
  - `cargo test --locked -p aster-acp --lib -- local_embedding::byo_factory_tests retrieval_seam_tests` (default features) -> 6 passed, 0 failed
  - `cargo check --locked -p aster-studio -p aster-office-cli` -> pass
  - `cargo check --locked -p aster-core --no-default-features --features minimal` -> pass
  - `cargo check --locked -p aster-core --features headless-search` -> pass
- **Clippy (`--locked --no-deps`)**:
  - `cargo clippy --locked -p aster-use-browser --no-default-features --features quark-cdp --all-targets --no-deps` -> pass (0 errors)
  - `cargo clippy --locked -p aster-use-browser --all-targets --no-deps` -> pass (0 errors)
  - `cargo clippy --locked -p aster-core --no-default-features --features local-code --lib --no-deps` -> pass (0 errors, 10 pre-existing warnings)
  - `cargo clippy --locked -p aster-pager --lib --no-deps` -> pass (0 errors, pre-existing warnings only)
- **Observed pre-existing / feature-expected failures kept separate**:
  - `cargo test --locked -p aster-office`: 369 passed, 99 failed (`collaboration::tests::*` fixtures encode `a3s.office.metadata` while `NATIVE_OFFICE_COLLABORATION_NAMESPACE` is `aster.office`, plus unrelated OOXML/comment/annotated failures; reproduced identically without `--no-default-features`).
  - `cargo test --offline -p aster-tools --lib` (earlier full run): 3,308 passed, 4 failed (`tier_restricted_short_circuits_with_upsell` asserts `"supergrok?referrer=aster-build"`; `pdf_size_gate_rejects_oversized` and `preserves_schema_property_order` were subsequently fixed and passed in the 67/67 run; `draining_foreground_command_survives_a_kill` passed when re-run individually).
  - `cargo test --locked -p aster-acp --no-default-features --features svg-png,account-import --lib`: 558 passed, 4 failed, 10 ignored (3 failures require bundled embedding weights intentionally excluded by `--no-default-features`, verified 6/6 under default features; 1 pre-existing `run_observation_agent_tests::actual_acp_failure_and_cancel_close_only_with_the_core_terminal_outcome` which also fails under default features).
  - `cargo test --locked -p aster-acp --test shipped_tui_e2e -- --test-threads=2` (with matching debug pair): 45 passed, 3 failed, 2 ignored (`shipped_agent_session_wire`, `recovery::shipped_tui_questions_before_approving_a_batched_write`, and `shipped_tui_launch_resume_and_visible_commands` line 3719 `"Use zh-CN only"` / completion-gate assertions in the pre-existing dirty `shipped_tui_e2e.rs` / `fact_control.rs` / `session_open.rs` worktree changes).

## Explicitly Not Claimed

- Cross-platform native `release-dist` builds and runtime execution on Linux (`x86_64`/`aarch64` GNU/musl) and Windows (`x86_64`) were not run on this `Darwin arm64` host.
- Live browser CDP execution against a logged-in Quark Chrome instance remains `#[ignore]`d (`live_login_probe_reports_availability`, `live_background_search_neither_creates_nor_closes_targets`).
- Non-default runtime upload/telemetry/PDF execution paths behind disabled optional features (`aster-file-utils/cloud-storage`, `aster-tools/pdf-read`, `aster-telemetry/otlp-grpc`) were compile/unit-tested in their default disabled configuration; enabling those features at runtime is untested end-to-end against external cloud/OTLP gRPC services.
- Full `aster-office` (99 pre-existing failures) and full `aster-tools` (`tier_restricted_short_circuits_with_upsell` pre-existing rebrand assertion) did not pass 100%, and pre-existing dirty files (`fact_control.rs`, `session_open.rs`, `shipped_tui_e2e.rs`, formatting diffs in `crates/office`) were left untouched.
