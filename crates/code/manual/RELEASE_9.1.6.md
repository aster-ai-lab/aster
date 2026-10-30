# Core / ACP 9.1.6 release candidate

## Scope

- Release `a3s-code-core` and `a3s-code-acp` as version 9.1.6 from one clean, committed source revision.
- Preserve the existing Rust package names and public APIs. Both packages remain local to this workspace with `publish = false`; this candidate does not restore registry or foreign-language SDK publishing.
- ACP rejects image, audio, embedded-resource, and resource-link prompt blocks with `invalid_params` instead of silently dropping them. Unsupported prompts fail before a model call, and initialization does not advertise embedded-context prompt support.
- Automatic goal continuation carries the previous judgment's complete, ordered `remaining_criteria` into the bound step's next execution input as advisory data. Empty feedback preserves the original prompt, and step acceptance still requires fresh workspace evidence and a new judgment (#20).
- Optional workflow features remain disabled in the default CLI/ACP build.

Aster CLI 0.1.4 already contains Core / ACP 9.1.6. That product release does not create or replace the A3S Code `v9.1.6` source release, and its earlier qualification does not certify later working-tree changes.

## Required qualification

Build from the repository root with the pinned Rust toolchain. Before tagging, record the exact source and submodule revisions and complete:

1. `cargo fmt -p aster -p a3s-code-core -p a3s-code-acp -- --check`
2. `python3 tests/workspace_dependencies.py`
3. `python3 tests/studio_checkout.py`
4. `python3 tests/cli_pair_delivery.py`
5. `python3 tests/aster_cli_linux.py`
6. `python3 tests/cli_release_workflow.py`
7. Focused Core and ACP tests for the changes included in the frozen candidate.
8. A locked release build of `aster` and `a3s-code-acp`, followed by the applicable hosted macOS, Windows, and Linux release lanes.

Do not tag from a dirty worktree or from a revision with unresolved conflicts. Do not treat selected local tests as whole-workspace, real-model, or cross-platform qualification.

## Local qualification on 2026-10-28

- Rust formatting and all five release preflight contract suites passed.
- Both issue #20 feedback tests passed.
- Core: 5,489 passed in the main lane and 15 passed in the serial sandbox lane; no failures. The lanes had 39 and 5 ignored tests respectively, with the sandbox module excluded from the main lane to avoid global environment and companion-resource races.
- ACP: 423 passed, 0 failed, 8 ignored.
- Locked release builds completed for `aster` and `a3s-code-acp`; `a3s-code-acp --version` reports 9.1.6.
- Tests requiring real Bash are explicitly skipped when no qualified process backend is installed. They must still run in a qualified hosted environment before tagging.

Logs are `target/core-full-regression.log`, `target/core-sandbox-regression.log`, and `target/acp-full-regression.log`. These results qualify the current working tree only; a clean committed candidate and hosted platform lanes remain required.

## Qualification limits

Scripted ACP and controlled-model checks do not establish real-model Terminal Bench latency or token improvements. A local macOS build does not certify Windows or Linux artifacts. The release evidence must state any remaining failures and untested recovery, multi-platform, and soak scenarios.
