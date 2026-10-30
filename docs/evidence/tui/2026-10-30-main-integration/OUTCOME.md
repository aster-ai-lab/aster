# Main integration of Windows Codex and Git fixes

Integration base: remote `main` commit
`3da5b37b9e0b1e642e598d062631843f67dd37ed`.

The original checkout was on a divergent local `main` with other tasks' dirty
files and a local Gateway commit. An isolated worktree inside this repository
was created from the fetched remote tip. Only the scoped fixes below were
applied; the original checkout and its index were preserved.

## Integrated scope

- Codex account discovery reuses the existing HOME/USERPROFILE resolver.
- Read-only Git tools omit mutation snapshots; Git writes retain their path
  ownership observations and existing authorization.
- The actual ACP catalog and actual Git tool have new hermetic regressions,
  including paths containing spaces and Chinese characters. Both run in CI.
- Windows soak tests use the existing platform API wrapper, redirect fixtures
  consume bounded complete headers, the benchmark retains its rerank mode
  before moving options, and repository text reads explicitly use UTF-8.
- The dirty-worktree test compares the actual provider path while preserving
  refusal, directory-survival, and unchanged-content assertions.
- The upstream shipped-TUI test file uses POSIX scripts (`/bin/sh`,
  `/bin/sleep`, `/usr/bin/python3`) and Unix executable permissions. Its missing
  Unix compilation gate caused four Windows all-target errors. The explicit
  `cfg(unix)` now states that fixture limitation; Linux/macOS cases and their
  assertions are unchanged. The separate native Windows regressions remain
  enabled. This Unix fixture suite is not counted as Windows coverage.

Remote main already contains the original form/display fixtures (Git blobs
`cca5347bcc168892fd574b098b317f601c2158a2` and
`f54fab94f6174ed81b6c94328cc793b07a53fc39`), and its UI already uses the native
JSON Render contract. Those upstream files were retained without changes;
the old checkout's OpenUI compatibility repair is superseded here.

No unrelated Gateway, deployment, permission-policy, website, OCR, generated
Office, or private-submodule work is included. No submodule gitlink changes
are included. Studio remains pinned to
`f6d648642fc5f4f3999ec2fd537e4e2e42cb45bc`, Power to
`24b364c6935c1b62a689d91839ec394bfc74c059`. Their pinned sources were checked
out from the existing local Git object caches before Cargo commands.

The preceding evidence records describe the earlier dirty checkout and its
binaries; their measurements and earlier check results are historical:
[Codex](../2026-10-30-codex-home-resolution/OUTCOME.md),
[Git](../2026-10-30-git-read-query-stall/OUTCOME.md),
[check repairs](../2026-10-30-remaining-checks/OUTCOME.md).

## Verification of the scoped source

Native Windows x64, pinned Rust 1.97.1, default package features. Python checks
use the existing working Astral CPython 3.12.14; the MCP fixture launcher sets
`PY_PYTHON3=Astral/CPython3.12.14` in the check process only. Cargo uses the
existing build cache; no source lockfile or toolchain pin was changed.

- Core/ACP package format checks: passed.
- Dependency boundary guard: 6 passed.
- `cargo clippy --locked -p a3s-code-core -p a3s-code-acp --all-targets --no-deps`:
  passed with existing warnings, after fixing the Unix fixture compilation gate.
- Core Git and HTTP unit regressions: 19 passed.
- Actual Git read batch and original form integration: 3 passed.
- Windows MCP and session lifecycle soaks: 2 passed, exercising all 30 child
  add/call/remove cycles and all 200 session open/close cycles.
- ACP Codex catalog integration: 2 passed across 8 home-discovery scenarios.
- Existing Codex account units: 6 passed.
- Upstream Form Core catalog contract: 4 passed.
- `cargo build --locked -p aster -p a3s-code-acp`: passed.
- Staged scope and whitespace checks: passed, exactly 16 files.

This is 36 passed Rust cases and 6 passed dependency guard cases on the
isolated integration base. No evaluation input, model, deadline, or acceptance
threshold was changed.

During verification remote main advanced to
`d6b8e4a884df0dc3db2036fccac75c7fa3270c67`. Its diff from the integration base
was reviewed: Gateway/Apofasi changes plus a Gateway dependency edge in the
lockfile, with no Core/ACP source, test, or submodule-pin changes. This upstream
history is retained by a normal merge before pushing; no force push is used.

After that merge, the Core/ACP format checks, the actual Git regression
(1 case), the ACP Codex catalog regressions (2 cases / 8 scenarios), and all
6 dependency guard cases passed again. The incoming merge changed no Core/ACP
source or regression files. The merge had no conflicts, and staged whitespace
checks passed.

No official binary release, installed stable replacement, native Linux/macOS
runtime verification, or full-workspace test result is claimed.
