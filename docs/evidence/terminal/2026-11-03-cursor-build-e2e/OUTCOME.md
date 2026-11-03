# Cursor repair: release build and end-to-end delivery

Status: release build and real installed-artifact end-to-end acceptance passed: Auto and Composer each completed a two-turn interactive TUI session and a separate file-copy task. Both command-execution smoke modes passed. Rust test-driver compilation and ACP lint remain incomplete, as recorded below.

## Delivered artifacts

- Archive: `target/cursor-build-e2e-20261103/aster-cli-v0.1.8-macos.tar.gz`.
- Isolated installation: `target/cursor-build-e2e-20261103/installed/`.
- CLI 0.1.8; ACP 9.1.8. No manifest version bump, commit, tag, upload, or global installation replacement.
- Matching pair identity: `aster-cli-pair-v1:f45e03b0eb32abec36e5f5bbcfb3f2a148ae93c7:aarch64-apple-darwin`.
- Archive SHA-256: `362726bafac6c4c44b75e835d482e5580dbd863c86a5dfb2c379d6bdecd8f5a8`.

Built from the current checkout on macOS ARM64 using pinned Rust 1.97.1 and default package features. Studio was initialized; exact host and submodule identities are in `host.json`. Existing unrelated checkout changes were preserved. The release pair was copied to a dedicated artifact directory before subsequent Cargo commands, protecting the tested binaries from concurrent builds. `binaries.json` records versions, identities, sizes, and hashes. Cursor implementation files and the native task fixture match all 12 corresponding hashes from the earlier repair acceptance (`prior-core-source-comparison.json`); its 38 Core regressions and Core lint were not rerun in this delivery step.

## Commands and actual results

Commands run from the repository root. Build, test-build and lint commands used `CARGO_BUILD_JOBS=8`.

| Command or retained operation | Actual result |
| --- | --- |
| `env -u GROK_VERSION cargo build --locked --release -p aster -p aster-acp` | Passed in 14m 43s; `release-build.log` |
| `python3 scripts/aster_sandbox.py stage --profile release` | Passed; rebuilt host companions and staged the ad hoc signed HVF shim, guest/system image, libraries and rootfs; `sandbox-stage.log` |
| Existing `aster_sandbox.copy_verified` copied release sandbox into the dedicated build directory | Passed source and destination verification |
| `bash scripts/package-aster-cli.sh macos target/cursor-build-e2e-20261103/build target/cursor-build-e2e-20261103/aster-cli-v0.1.8-macos.tar.gz` | Passed, including matching executable build identities |
| Source-contract archive audit and `scripts/install.sh --prefix target/cursor-build-e2e-20261103/installed` with local `ASTER_RELEASE_BASE` | Passed: exactly 13 source-contract members, 11 payload checksums; all installed payload hashes match the archive. See `archive.json` and `install.log` |
| `python3 tests/cli_sandbox_smoke.py target/cursor-build-e2e-20261103/aster-cli-v0.1.8-macos.tar.gz --output-dir docs/evidence/terminal/2026-11-03-cursor-build-e2e/sandbox-smoke` | Passed: final archive installed into a temporary prefix, actual command exited 0 and returned Linux plus an exact random workspace nonce |
| Same smoke command with `--mode yolo --output-dir docs/evidence/terminal/2026-11-03-cursor-build-e2e/yolo-smoke` | Passed: actual command exited 0 and returned Darwin plus an exact random workspace nonce |
| `python3.12` here-document retained verbatim in `run-installed-cursor.py` | 2 passed, 0 failed: installed CLI/ACP, real local Cursor login, Auto and Composer 2.5. Each uses a separate temporary workspace with spaces/non-ASCII and isolated Aster state. Frozen prompt and 240-second timeout are in `cursor-cli-results.json`. Input bytes stay unchanged; output bytes equal the random input; final assistant text contains the unseen random marker plus `verified`; both CLI processes exit 0 with no `Turn failed`. The prompt additionally requests one `test -f output.txt && cmp input.txt output.txt` command; the retained CLI output is assistant text, not a structured per-tool receipt trace |
| `python3.12 docs/evidence/terminal/2026-11-03-cursor-build-e2e/run-installed-tui.py` | Final run: 2 model cases passed, 0 failed; 4 actual interactive turns total. Each model read unseen random workspace content and displayed its exact line followed by `completed`, then answered the unchanged `p` + `ong` prompt in a second turn. Input bytes stayed unchanged; no output file was created; no token-shaped text was observed; teardown completed without errors. See `installed-tui-results.json` and `installed-tui.log` |
| `cargo test --locked -p aster-acp --test cursor_account_tui_e2e --test cursor_native_tools_live --no-run` | Incomplete. Two attempts interrupted with exit 130: first log `test-build-stalled.log`, identical retry `test-build-retry-stalled.log`. The retry compiled ACP library and binary, then the TUI test compiler remained asleep for over 8 minutes with a cached `schemars_derive` library open |
| `cargo clippy --locked -p aster-acp --all-targets --no-deps` | Interrupted with exit 130 while waiting for the above build lock; not a lint pass. See `acp-clippy-wait.log` |
| `cargo clean --profile dev -p schemars_derive` | Cache-rebuild attempt interrupted with exit 130 while scanning the existing development dependency directory; no successful cache rebuild claimed. See `macro-cache-rebuild.log` |
| `cargo fmt --all -- --check` | Passed initially and after final source review; `fmt.log`, `fmt-final.log` |
| `python3.12 tests/workspace_dependencies.py` | 7 passed |
| `python3 tests/cli_pair_delivery.py` | 21 passed, including missing companions, mismatched pair identity, installation failures and rollback |
| `python3 tests/cli_sandbox.py` | 7 passed |
| `python3 tests/cli_sandbox_smoke_test.py` | 3 passed; success requires actual command receipts rather than assistant acknowledgement |
| Final artifact/source audit and scoped `git diff --check` | Passed: archive unchanged, all 11 installed payload hashes unchanged, both smoke results passed, both CLI cases passed, both final TUI cases passed without cleanup errors; `final-audit.json`, `source-final.json`, `diff-check.log` |

## Review and limits

The directly executed PTY script uses the installed release pair, not a debug companion or a model stub. It preserves the existing read-only Cursor scenario prompt, the original TUI reply prompt, Auto/Composer identities and the 45/240/90-second readiness/read/reply budgets. The random marker never appears in either prompt. Its only dependencies are Python's standard library and the actual installed CLI/ACP. A temporary equivalent Rust test addition was removed after this direct acceptance, leaving the existing Rust prompts and assertions unchanged. Concurrent pre-existing edits to binary selection and workspace guards were preserved.

The initial direct PTY runner reached teardown but raised an error when killing the process group (`installed-tui-cleanup-failed.log`). A subsequent run established both functional results, but waiting for process termination before closing the terminal descriptor timed out (`installed-tui-cleanup-timeouts.json`, with the exact prior runner retained). The final runner closes the PTY descriptor before waiting and treats any remaining teardown error as a failure. It passed both models with the same prompts, models, task assertions and timeouts; no matching test processes remained afterward. This is a test-runner lifecycle correction, not a product shutdown qualification.

Diagnostic reads showed cached procedural-macro libraries open in the sleeping compiler. Byte reads and signature verification succeeded; no cause is assigned to the compilation stalls. No signature or security setting was altered. After the bounded package-cache cleanup attempt also stalled in the existing dependency directory, it was stopped; direct installed-artifact PTY acceptance completed independently. Existing compiler warnings are retained in the logs.

## Explicitly not claimed

- Successful rebuilding/running of the current Rust integration-test driver or successful ACP all-target lint. The successful current TUI acceptance used the retained directly executed Python PTY driver.
- Every model in the Cursor catalog, quota availability, or bypassing account limits. Only Auto and Composer 2.5 are covered by the successful live delivery tasks.
- Windows/Linux native qualification or a repository-wide test/lint pass. No workspace-wide, ORM, inference-engine, or optional advanced-harness check was started for this delivery.
- Updating the user's global `aster`, publishing a release, or creating a clean source revision.
- Per-token streaming; the existing buffered completion contract remains.
