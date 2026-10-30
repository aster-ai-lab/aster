# Cross-platform text and checkout policy

Date: 2026-10-28.

## Result

Repository attributes now define LF for ordinary text regardless of a host's
global `core.autocrlf`, and CRLF checkouts for Windows `.bat` / `.cmd` files.
Evidence and the existing byte-identified fixture paths retain exact bytes.
No repository-wide renormalization was performed. The already clean CLI
installer and package scripts were individually materialized as LF; their
canonical Git content did not change. A separate temporary fixture exercises
reviewed renormalization of a legacy CRLF index entry.

The behavior lands with a real Git regression suite and a Windows/Linux/macOS
CI matrix. The development standards §10, constitution constraints and AGENTS
quick reference carry the same rule. The quick-reference link also uses the
actual case-sensitive filename `AGENTS.md`.

## Validation

- Linux Git, Python in WSL: **6 tests passed**, retained in
  `line-endings-linux-git.log`.
- Windows Git invoked from Python in WSL: **6 tests passed**, retained in
  `line-endings-windows-git.log`.
- Actual CLI pair delivery from the repository's LF scripts: **10 passed**,
  with no temporary newline conversion around the test.

The six tests exercise both `core.autocrlf=true` and `false`: UTF-8 source
staging, LF shell checkout and execution, CRLF Windows command-file checkout,
immutable text with CRLF/padding/blank final lines, binary bytes, and a scoped
legacy renormalization which leaves another path's raw index/worktree bytes
untouched. Shell
executable mode is checked in the index. Temporary Git fixtures stay under
this workspace's ignored `target/tmp` and are removed by the tests.

```sh
python3 tests/line_endings.py
ASTER_TEST_GIT='/mnt/c/Program Files/Git/cmd/git.exe' python3 tests/line_endings.py
python3 tests/cli_pair_delivery.py
```

The first checkout fixtures reused a worktree file whose normalized content
Git already considered clean. Their six failing subcases are retained in the
`first-checkout-fixture-*.log` records. The corrected fixture tracks its
attributes and performs a fresh checkout of its own temporary files. Strict
LF/CRLF and exact-byte assertions were retained. Both distinct failing methods
were rerun alone on each Git implementation before both full suites passed.

## Explicitly not claimed

- Native macOS or native Windows Python execution here. The Windows Git
  implementation was exercised through WSL interop; the three hosted OS
  jobs are configured for subsequent CI execution.
- Automatic correction of historical blobs, all existing worktree files,
  another session's edits, or every legacy encoding. Byte-identified data
  must not be renormalized.
- A complete workspace/CI pass. Existing unrelated formatting differences
  remain; the prior Windows/Linux TUI acceptance is recorded separately.
