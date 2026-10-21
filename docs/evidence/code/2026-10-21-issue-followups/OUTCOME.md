# Issue follow-up validation (integration in progress)

This branch continues the seven issue fixes in PR #8. The source is aligned to main 64cfd79cac690e78c288c374ddba0c7650e5bfcc; source landing, remote CI and release publication are separate claims.

## Confirmed behavior

Core now cancels non-streaming body reads and streaming error-body reads, including raw/duplex requests. Error-body reads have a deadline and retain their HTTP status. A silent 401 body cannot be reclassified into a retryable 503. Four loopback regression tests failed on the old transport, and each failed independently; the repaired full HTTP group passed 29/29 on macOS and 29/29 on Linux x86-64 after integration. Productive stream lifetimes remain covered by the existing positive regression.

TUI initialize/authenticate and headless initialize/authenticate/new/load/fork/model startup reuse the existing bounded session RPC helper. A timed-out load preserves the reason instead of reporting a missing session. Sibling virtual-clock tests cover silent live sidecars. Full prompt replacement remains explicitly unsupported; --rules appends instructions and CLI help states the boundary.

CI checks out private Work/API/Cloud at exact gitlink revisions using a repository-scoped Contents:read GitHub App token. It installs missing tools/Office target, retains the backend package matrix with the correct workspace manifests, and runs the real native ACP artifact case. Source-pin parser: 2/2. Official actionlint 1.7.12 passed the three edited workflows.

The disposable S3 fixture uses pinned Bitnami legacy image digests after the old latest images disappeared and MinIO archive URLs returned HTTP 410. Both images were pulled and a real create/write/read cycle passed; the owned test container was removed. No production storage is changed.

Python installer fault injection passed 9/9 on Linux. Workspace boundary guard passed 6/6 on both macOS and a Linux Rust 1.97.1 container. Desktop release boundary passed 6/6. Fresh baseline real CLI/ACP/loopback cases passed 4/4 before the new lifecycle guards. New source-matched binaries and final startup/update tests are still being built.

The initial historical tenant restoration was validated locally, then superseded by the concurrent main brand repair: 36 of its files are identical, the newer toyou BRAND.md is retained, and the unused JSON model-market configuration is excluded. Existing restoration receipts remain as historical evidence.

## Commands and evidence

- cargo test --locked --offline -p a3s-code-core --lib llm::http:: — 29 passed, 0 failed (macOS; process launch was delayed before main).
- The four response_lifecycle_tests ran separately against the old transport — 4 independently confirmed failures, each at the test's 1-second assertion.
- python3 tests/ci_source_checkout.py -v — 2 passed.
- python3 tests/workspace_dependencies.py -v — 6 passed on each host.
- python3 tests/cli_pair_delivery.py -v — 9 passed on Linux.
- python3 tests/desktop_update.py -v — 6 passed.
- cargo fmt --all -- --check; actionlint ci.yml backend.yml release-aster-cli.yml — passed before integration.

Native macOS spawned executables/scripts intermittently remained at dyld entry. Process samples and failed launch runs are retained; SIGCONT attempts did not resolve that stall. Independent Linux runs preserve the original assertions/timeouts. No other session's processes or files were modified.

## Explicitly not claimed

- Remote CI success: Work deploy keys are forbidden (HTTP 422); current org-secret inventory is denied (HTTP 403). No credentials were copied or created. CI_SOURCE_APP_ID / CI_SOURCE_APP_PRIVATE_KEY still require owner configuration or confirmation of an existing dedicated App.
- A published v0.1.2 CLI release, refreshed installer delivery, final integration startup/update counts, or a full workspace/live backend campaign. These are pending validation, not completed by closing issues.
- An exact reconstruction of the original real-provider 180-second incident: that configuration/log is unavailable. The healthy and controlled-fault cases establish the mechanisms stated here.

## Integration checkpoint

Rebased onto 64cfd79cac690e78c288c374ddba0c7650e5bfcc, preserving the newer CLI updater, MIT package metadata, ACP routing and brand repair. Work source was materialized from the exact new gitlink d02674726be5267e38d579d775444cab21483078 using tracked git archive only. No credential or user data was exported.

The first parallel Linux HTTP group exposed an environment-isolation race: the existing header-timeout test changes a process-global deadline while the separate duplex test was not taking its lock. The duplex test passed alone. Reusing that same test-only lock for the sibling HTTP tests made the complete final group pass 29/29; original assertions and timeouts are unchanged. Source-pin 2/2, workspace 6/6, Unix installer 9/9 and desktop updater 6/6 were rerun against the integrated tree. Official actionlint and workspace format checks passed.

The v0.1.2 updater uses its new pinned release and requires both BUILD_ID and SHA256SUMS before promotion. Its regression now requires an older two-file archive to be refused with both installed files unchanged. The concurrent main's tenant.rs formatting was corrected for the mandatory workspace format gate; its behavior is unchanged. Interactive eager auth retains its existing shorter soft bound, and a dropped/failed TUI connection setup cancels its owned worker through the platform cancellation guard.
