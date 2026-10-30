# Codex account discovery on Windows

Follow-up verification and repairs to the existing check blockers are recorded
in [Windows check blockers](../2026-10-30-remaining-checks/OUTCOME.md).

## Change and scope

The installed `aster-cli-v0.1.4` pair could not discover the local Codex
catalog on a Windows host without `HOME`, despite a valid login under
`USERPROFILE/.codex`. Explicit account import alone did not fix discovery;
explicit import plus `A3S_CODEX_HOME` did.

The importer now reuses the ACP configuration loader's existing host-home
resolver: non-empty `HOME`, then non-empty `USERPROFILE`. Non-empty
`A3S_CODEX_HOME` and `CODEX_HOME` retain their existing higher priority.
Account import still requires the host's opt-in. This change adds no
dependency and changes no published Rust API.

Only the Codex fallback and the internal visibility/documentation of that
existing helper changed in production. Unrelated working-tree edits were
left in place. No binaries were installed or published during this change.

## Environment

- Native Windows x64, Windows build 22631.
- Pinned Rust `1.97.1-x86_64-pc-windows-msvc`.
- Commands ran at the root workspace with Studio already initialized at
  `5aeb4400ce56ccdac74185e459488c6a6120c370`.
- ACP default features: `account-import` and `svg-png`.
- The locally installed protoc was supplied through `PROTOC` where needed.

## Validation

| Command | Result |
| --- | --- |
| `cargo test --locked -p a3s-code-acp --test codex_account_stdio` | 2 passed, 0 failed; 8 actual-ACP child-process discovery cases |
| `cargo test --locked -p a3s-code-acp --lib codex_account::tests -- --test-threads=1` | 6 passed, 0 failed, 352 filtered out |
| `cargo clippy --locked -p a3s-code-acp --all-targets --no-deps` | Exit 0; existing warnings remain |
| `rustfmt --edition 2021 --check crates/code/acp/src/codex_account.rs crates/code/acp/src/config.rs crates/code/acp/tests/codex_account_stdio.rs` | Passed |
| `git diff --check -- crates/code/acp/src/codex_account.rs crates/code/acp/src/config.rs` | Passed |
| `cargo fmt --all -- --check` | Could not launch rustfmt: Windows OS error 206, command/filename too long |
| `cargo fmt -p a3s-code-acp -- --check` | Existing import-order differences in `src/agent.rs` at lines 9 and 41; that pre-existing modified file was not reformatted |
| `py -3 tests/workspace_dependencies.py` | Host Python failed before executing the check: `ModuleNotFoundError: No module named 'encodings'` |

The two new integration tests initialize the real ACP executable with
synthetic JWTs and catalogs. They cover absent and empty `HOME`, `HOME`
precedence over `USERPROFILE`, both explicit directory overrides, and empty
override fallback. All fixture directories include spaces and non-ASCII
characters. Overrides are set only on child commands, so parallel cases
do not mutate the test runner's environment. No model request is made.

Confirmed existing lint findings include unused code/imports in Core and
ACP, ACP argument-count/boolean-style warnings, and an unchanged
`field_reassign_with_default` warning in the old Codex unit tests. New
production changes and the new integration test introduced no reported
lint findings. These checks do not establish whole-workspace compliance.

## Concurrent Git timeout diagnosis

The user's installed TUI recorded `git` tool `status` failing with
`Failed to get status: Git command timed out after 30000ms`. The same batch's
`branch` and `remote` calls succeeded. The 30-second deadline is defined by
`GIT_COMMAND_TIMEOUT` in `crates/code/core/src/git.rs`.

Read-only follow-ups against the same workspace completed successfully:

- `git status --porcelain`: 1.714 seconds, exit 0.
- `status --porcelain --short` with Aster's Git arguments and environment:
  2.056 seconds, exit 0.
- Verbatim Windows executable/workspace path combinations: 2.177-2.229
  seconds, exit 0.
- The real Rust `WorkspaceServices::local(workspace).git().status().await`
  backend: 4.692 seconds, success. It was invoked through a temporary Cargo
  example removed after diagnosis.

This establishes a recorded status timeout and functioning Git discovery,
not its original trigger. No Git timeout, capture, or execution policy was
changed. The precise follow-up is to capture which internal Git invocation
times out, its elapsed phase, and competing subprocess activity if it recurs;
the current error does not identify the timed-out subcommand. User session
logs and credentials were not copied into this evidence directory.

## Explicitly not claimed

- Native Linux or macOS runtime verification; the portable regression tests
  are available for their existing CI jobs.
- A clean whole-workspace format, test, or dependency-guard result.
- Live Codex inference, token renewal, or account entitlement verification.
- A fix or reproducible root cause for the reported Git timeout.
- A new stable release or replacement of the installed CLI/ACP pair.

Other account importers and the developer-evaluation-only
`CodexLoginClient::from_local_login` still contain their own `HOME`-based
resolution. They were not changed. Their follow-up is a separate host-home
compatibility review with hermetic tests; the TUI's imported Codex client
uses `from_parts` and does not take that developer-evaluation path.
