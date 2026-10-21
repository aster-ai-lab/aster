# Shared request lifecycle and CI delivery follow-up

The follow-up is in draft PR [#9](https://github.com/aster-ai-lab/aster/pull/9). The seven original issue fixes remain on main through PR #8. Local validation below is complete; remote CI and formal release delivery are still blocked by private-source authorization.

## Confirmed mechanisms and changes

- Core cancellation now covers complete non-streaming response reads and streaming error bodies, including raw/duplex transport. Other error bodies use a bounded read.
- A known 401/403 returns immediately with any already available diagnostic. An unfinished authorization body cannot consume the outer model-call deadline and turn into a retryable timeout. Complete authorization diagnostics remain covered by a positive regression. Successful non-streaming generations retain their existing default lifetime; trusted ACL can set llm_api_timeout_ms. Productive streams retain header/idle bounds rather than a total generation deadline.
- TUI initialize and headless initialize/authenticate/new/load/fork/model setup reuse the existing bounded session RPC helper. A load timeout preserves its reason. Failed/dropped TUI connection setup cancels the owned worker; interactive eager auth retains its shorter existing first-draw bound.
- --rules appends instructions. Full prompt replacement is explicitly refused before provider calls; help now states the supported contract.
- CI resolves exact Work/API/Cloud gitlinks, checks out with per-repository Contents:read App tokens, and persists no checkout credentials. Backend packages stay in their owning workspaces. Missing toolchain/front-end prerequisites and the previously empty real ACP artifact step are repaired. The real CLI/ACP loopback suite is now maintained at tests/cli_acp_lifecycle.py and run in CI.
- The S3 campaign fixture uses verified immutable image digests because the old latest images disappeared and MinIO binary archives returned HTTP 410. A disposable real server passed create/write/read and was removed. This changes no production storage.
- CLI v0.1.2 delivery builds all three platform pairs in one run, creates a new tag atomically at the build SHA, verifies that tag, and creates a release without clobbering assets or changing the desktop latest-release channel. The updater and both installers use v0.1.2 and require matching build identities/checksums.

## Reproductions retained

Four response-body tests failed on the old transport and each failed independently. Their original logs are retained. The first parallel Linux HTTP run additionally exposed a test-environment race: one existing test shortened the process-global header deadline while the duplex test did not take its lock. That duplex test passed alone; reusing the existing lock fixed the complete group without relaxing assertions/timeouts.

A real CLI/ACP/loopback test exposed the cross-layer deadline race after the initial transport repair: with a 300ms model bound, a partial 401 body caused two main streaming requests and ended as a generic model timeout. After returning known authorization failures promptly, the same test makes one main streaming request and returns a credential error in 0.628s on macOS and 0.289s on Linux ARM64. Before/after JSON and logs are retained.

## Validation commands and counts

All Rust commands use the root workspace and pinned Rust 1.97.1, with private target directories. Work was materialized from tracked source at d02674726be5267e38d579d775444cab21483078; no credential or user session state was exported.

| Command / check | Result | Evidence |
| --- | --- | --- |
| cargo test --locked --offline -p a3s-code-core --lib llm::http:: | 31/31 on macOS ARM64 and Linux ARM64 | logs/http-final-macos.log; logs/http-and-real-cli-final-linux-arm64.log |
| cargo test --locked --offline -p a3s-code-pager --lib headless::tests:: | 36/36 on both hosts | logs/headless-macos.log; logs/startup-updater-pair-linux-arm64.log |
| cargo test --locked --offline -p a3s-code-pager --lib acp::startup_deadline_tests:: | 2/2 on both hosts | logs/startup-macos.log; logs/startup-updater-pair-linux-arm64.log |
| cargo test --locked --offline -p aster --test cli_update | 4/4 on Linux ARM64 | logs/startup-updater-pair-linux-arm64.log |
| cargo build --locked --offline -p aster -p a3s-code-acp | both succeed, same pair identity per platform | logs/pair-build-macos.log; logs/startup-updater-pair-linux-arm64.log; pair identity and E2E JSON |
| python3 tests/cli_acp_lifecycle.py --phase after --binaries <debug-bin-dir> --evidence <temp-dir> | 6/6 on both hosts | logs/real-cli-final-macos.json; logs/real-cli-final-linux-arm64.json |
| python3 tests/ci_source_checkout.py -v | 2/2 | logs/source-pin.log |
| python3 tests/workspace_dependencies.py -v | 6/6 on integrated Linux source | logs/workspace-linux.log |
| python3 tests/cli_pair_delivery.py -v | 9/9, including rollback, checksum/identity refusal and displayed version | logs/installer-faults-linux.log |
| python3 tests/desktop_update.py -v | 6/6 | logs/desktop-update-linux.log |
| cargo fmt --all -- --check; official actionlint 1.7.12 on the three edited workflows | exit 0 | logs/format.log; logs/actionlint.log |

The six real-process cases cover ACL credentials; ACL plus the legacy API-key environment variable; transmitted rules; explicit unsupported prompt replacement; a bare legacy key without provider ACL; and a stalled 401 response body. Homes, configuration, workspaces, credentials and provider are isolated/synthetic; no real account or provider was used.

A real Linux ARM64 debug pair was packaged and installed into an isolated prefix. Both installed files compare byte-for-byte with the built files, report the same pair identity, and report Aster 0.1.2. Its 289MiB debug archive is local only and is not a release asset. The check also caught and corrected the old version text in both installer completion messages.

The concurrent main brand repair superseded the initial historical tenant restoration: 36 files were identical, the newer toyou BRAND.md was retained, and unused JSON model-market configuration was excluded. Historical receipts remain. The main tenant.rs format correction has no behavior change.

Native macOS processes/scripts intermittently stalled before main at dyld entry. Failed launch samples/runs were retained; SIGCONT attempts did not resolve the delay. The final native Rust and real CLI checks completed, and independent Linux runs preserve the same assertions/timeouts. Other sessions' processes and working files were not modified.

## Remote authorization boundary

Actual Rust and backend runs at 73624d14b3b6183fecad4067cf0a436afbdebd43 stopped at `CI source GitHub App credentials are missing for apps/work` (SOURCE_KEY_SET=false), before compilation. See [Rust run](https://github.com/aster-ai-lab/aster/actions/runs/39518970410) and [backend run](https://github.com/aster-ai-lab/aster/actions/runs/39518970477), plus retained logs. This confirms the configured credential names are unavailable to those jobs; it does not claim that no other organization secrets exist.

Work explicitly prohibits deployment keys (HTTP 422), and the current connection cannot inventory organization secrets (HTTP 403). No key or secret was created/copied. The owner must configure or identify a dedicated App using CI_SOURCE_APP_ID / CI_SOURCE_APP_PRIVATE_KEY, installed for aster-ai-lab/work and kense-labs/api, kense-labs/cloud with Contents:read. Setup and the verified legacy A3S comparison are in CI_ACCESS.md.

## Explicitly not claimed

- Green remote CI, full workspace/live PostgreSQL/S3 campaigns, Windows native installation or a formal v0.1.2 release. These await configured private-source access and actual remote validation. New installer URLs stay in the draft branch.
- A production release-profile build from the local debug archives, signing/notarization, or interactive desktop/TUI and native sandbox/tool execution. The loopback cases exercise real CLI/ACP model transport without native tools.
- Exact reconstruction of the original real-provider 180-second incident: its configuration/log is unavailable. The controlled failures establish only the mechanisms recorded here. A bare legacy key is not enough to select a provider, and ACL plus that environment key was verified healthy.
