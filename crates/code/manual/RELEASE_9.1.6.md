# Core / ACP 9.1.6 release candidate

## Scope

- Release `aster-core` and `aster-acp` as version 9.1.6 from one clean, committed source revision.
- Preserve the existing Rust package names and public APIs. Both packages remain local to this workspace with `publish = false`; this candidate does not restore registry or foreign-language SDK publishing.
- Ordinary ACP model turns accept validated image attachments; explicit text-only model selections reject them before admission. Audio, embedded-resource and resource-link prompt blocks fail with `invalid_params` instead of being silently dropped. Initialization does not advertise embedded-context prompt support.
- Automatic goal continuation carries the previous judgment's complete, ordered `remaining_criteria` into the bound step's next execution input as advisory data. Empty feedback preserves the original prompt, and step acceptance still requires fresh workspace evidence and a new judgment (#20).
- Optional workflow features remain disabled in the default CLI/ACP build.

Aster CLI 0.1.4 already contains Core / ACP 9.1.6. That product release does not create or replace this repository's `aster-v9.1.6` source release, and its earlier qualification does not certify later working-tree changes.

## Required qualification

Build from the repository root with the pinned Rust toolchain. Before tagging, record the exact source and submodule revisions and complete:

1. `cargo fmt -p aster -p aster-core -p aster-acp -- --check`
2. `python3 tests/workspace_dependencies.py`
3. `python3 tests/studio_checkout.py`
4. `python3 tests/cli_pair_delivery.py`
5. `python3 tests/aster_cli_linux.py`
6. `python3 tests/cli_release_workflow.py`
7. Focused Core and ACP tests for the changes included in the frozen candidate.
8. A locked release build of `aster` and `aster-acp`, followed by the applicable hosted macOS, Windows, and Linux release lanes.

Do not tag from a dirty worktree or from a revision with unresolved conflicts. Do not treat selected local tests as whole-workspace, real-model, or cross-platform qualification.

## Local qualification on 2026-10-28

The following entries are retained historical records. Their counts and binary hashes do not qualify the current merged source.

### Earlier working-tree record

- Rust formatting and all five release preflight contract suites passed.
- Both issue #20 feedback tests passed.
- Core: 5,489 passed in the main lane and 15 passed in the serial sandbox lane; no failures. The lanes had 39 and 5 ignored tests respectively, with the sandbox module excluded from the main lane to avoid global environment and companion-resource races.
- ACP: 423 passed, 0 failed, 8 ignored.
- Locked release builds completed for `aster` and `aster-acp`; `aster-acp --version` reports 9.1.6.
- Tests requiring real Bash are explicitly skipped when no qualified process backend is installed. They must still run in a qualified hosted environment before tagging.

Logs are `target/core-full-regression.log`, `target/core-sandbox-regression.log`, and `target/acp-full-regression.log`. These results qualify the current working tree only; a clean committed candidate and hosted platform lanes remain required.

### Retained frozen-branch record

Earlier working-tree logs are `target/core-full-regression.log`, `target/core-sandbox-regression.log`, and `target/acp-full-regression.log`. They do not certify the clean committed candidate by themselves.

The frozen release branch starts from `807095e08` and adds commit `f63d9428e3aef838cda08d02eefc6e74313fa78c`, completing the issue #20 implementation and adapting stale test assertions to the current result envelopes. Its clean-candidate checks are:

- Rust formatting and all five release preflight contract suites passed.
- Both issue #20 feedback tests passed.
- ACP conformance: 10 passed. ACP unit regression: 377 passed, 0 failed, 8 ignored.
- Locked release build completed for `aster` and `aster-acp`; `aster-acp --version` reports 9.1.6 and `aster --version` reports 0.1.4 with matching source prefix `f63d9428e3ae`.
- Release binary SHA-256: ACP `a68b115a72f0e1e33e419ab35a96f44c393e8d64c608f34d19dbbeafd71feb08`; Aster `f51ce90f4cc23886b08b8e68eec4bd89f2d09ca1cf9a40e25158bb98bb9ebfcd`.
- Tests requiring real Bash remain explicitly skipped when no qualified process backend is installed. Hosted platform lanes are still required before tagging.

## Qualification limits

Scripted ACP and controlled-model checks do not establish real-model Terminal Bench latency or token improvements. A local macOS build does not certify Windows or Linux artifacts. The release evidence must state any remaining failures and untested recovery, multi-platform, and soak scenarios. The hosted Windows lane runs portable Core/ACP tests and the locked ACP release build; it does not claim the full ACP unit suite, whose Unix-specific fixtures require Unix runners.
