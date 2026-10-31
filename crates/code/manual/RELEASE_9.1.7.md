# Core / ACP 9.1.7 release candidate

## Scope

- Release `aster-core` and `aster-acp` as version 9.1.7 from one clean, committed source revision.
- Preserve the existing Rust package names and public APIs. Both packages remain local to this workspace with `publish = false`; this candidate does not restore registry or foreign-language SDK publishing.
- Align ACP permission prompts with coding-agent industry practice: the first HITL ask names the execution boundary (workspace sandbox, host escalation, or a single mediated network origin), maps session posture onto the interactive approval closed set for risk copy, and refuses `allow-always` learning for host or network-grant approvals so a scoped consent cannot widen silently.
- Explicit deny rules, catastrophic bash denials, and the completion gate remain non-bypassable under every posture, including YOLO.
- Optional workflow features remain disabled in the default CLI/ACP build.

## Required qualification

Build from the repository root with the pinned Rust toolchain. Before tagging, record the exact source and submodule revisions and complete:

1. `cargo fmt -p aster -p aster-core -p aster-acp -- --check`
2. `python3 tests/workspace_dependencies.py`
3. `python3 tests/studio_checkout.py`
4. `python3 tests/cli_pair_delivery.py`
5. `python3 tests/aster_cli_linux.py`
6. `python3 tests/cli_release_workflow.py`
7. `python3 tests/release_governance.py`
8. Focused Core and ACP tests for the permission-boundary changes included in the frozen candidate, including:
   - `cargo test --locked -p aster-acp --lib execution_boundary_tests`
   - ACP unit regression for the permission ask path
9. A locked release build of `aster` and `aster-acp`, followed by the applicable hosted macOS, Windows, and Linux release lanes.

Do not tag from a dirty worktree or from a revision with unresolved conflicts. Do not treat selected local tests as whole-workspace, real-model, or cross-platform qualification.

Tag the clean candidate as `aster-acp-v9.1.7` (never a bare `v9.1.7`, and never the retired `aster-v9.1.7` / `a3s-code-v*` namespaces). The `release-code.yml` workflow must create the Core/ACP Release with `--latest=false`. Archive names share the same `aster-acp-v9.1.7` prefix.

## Qualification limits

Scripted ACP and controlled-model checks do not establish real-model Terminal Bench latency or token improvements. A local macOS build does not certify Windows or Linux artifacts. The release evidence must state any remaining failures and untested recovery, multi-platform, and soak scenarios. The hosted Windows lane runs portable Core/ACP tests and the locked ACP release build; it does not claim the full ACP unit suite, whose Unix-specific fixtures require Unix runners.

Mediated-network Gate 4 residuals, macOS descendant-process containment without companions, and the durable single-use approval-proof credential (beyond the existing fact-path coalesce) remain documented follow-ups and are out of scope for 9.1.7.

## Local qualification on 2026-10-31

These entries qualify the working-tree candidate only. A clean committed SHA and hosted platform lanes remain required before tagging `aster-acp-v9.1.7`.

- `cargo test --locked -p aster-acp --lib execution_boundary`: 4 passed.
- `cargo test --locked -p aster-acp --lib only_a_learnable_bash`: 1 passed (sandboxed learnable path still offers `allow-always`).
- `python3.12 tests/workspace_dependencies.py`: passed.
- Core and ACP Cargo versions both report `9.1.7`; `RELEASE_9.1.7.md` is present.
- Locked release build of `aster-acp` completed; `aster-acp --version` reports `9.1.7`.
- SHA-256 of the local release binary: `e4137e6e03aa0e49543847306e40e8e6f6a0e9fb93386a0e0c4d04ce1b065705` (host-local; not a release asset digest).
- `tests/cli_pair_delivery.py` / `tests/cli_release_workflow.py` / `tests/release_governance.py` hit environment timeouts under concurrent cargo load; re-run them on the clean candidate before tagging.
