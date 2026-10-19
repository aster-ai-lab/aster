# Aster CLI P0 closeout

The shipped `aster` binary was exercised from `apps/aster/tests/cli_p0.rs`
against an isolated home and a loopback model. Source builds do not download
or replace the binary. `setup --json` reports an unconfigured team without
writing install files. `version --json` names the product Aster. On macOS the
process sets `A3S_REFUSE_UNQUALIFIED_NATIVE_SHELL=1` before the pager starts,
and an unescalated shell tool does not create its side-effect file.

## Commands and counts

Rust 1.97.1, macOS, debug test profile. Each command exited 0.

```
cargo test -p aster --test cli_p0 -- --test-threads=1 --nocapture
```

1 passed, 0 failed, 21.81s. The process printed these oracle names:
`version`, `doctor`, `inspect`, `setup`, `update-check`, `share`,
`completions`, `sessions`, `usage-missing`, `export-missing`, `memory`,
`mcp`, `plugin`, `worktree`, `du`, `leader`, `logout`, `trace`, `models`,
`headless-create`, `sessions-list`, `usage`, `trace-local`, `export`,
`continue`, `resume-id`, `resume-title`, `fork`, `delete`, `sandbox-resume`,
`shell-refusal`, `isolated-home`.

`update --check --json` reported `updateAvailable=false` and
`autoUpdate=false`. The binary length was unchanged. `setup --json` reported
`configured=false` and left the managed-config files absent. Session share
exited 1 with `Session sharing is temporarily disabled` and printed no URL.
The shell oracle contained `native_process_lifetime_unqualified` and the
marker file was absent. The developer `~/.a3s` fingerprint was unchanged.

```
cargo test -p aster --bin aster -- --test-threads=1
```

43 passed, 0 failed, 0.03s.

```
cargo test -p aster --test workspace -- --test-threads=1
```

2 passed, 0 failed, 9.93s.

```
rustup run 1.97.1 rustfmt --edition 2024 --check apps/aster/src/main.rs apps/aster/tests/cli_p0.rs apps/aster/tests/workspace.rs
```

Exited 0.

## Explicitly not claimed

No commit, tag, or published installer. No Linux or Windows run. No
`cargo build --release`. No live provider. The macOS native shell stays
refused. The six ACP boundary cases in `crates/code/acp/tests/boundary_e2e.rs`
were not re-run in this pass. Studio and AsterDream packaging were not part
of this check.
