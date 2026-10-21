# Desktop spawn to one bash

## Result

AsterDream `AcpClient::spawn` of a real `a3s-code-acp`, then `initialize`,
`session/new`, and one scripted `bash`, settles both ways.

With no parent `ASTER_SANDBOX_DIR` and no sibling `sandbox/`, the next
model request contains `native_process_lifetime_unqualified`,
`side-effect.txt` is absent, and `macos-hvf-host-service` stays at the
baseline (0). The agent log says `sandbox companion directory is
unavailable`. The placed binary is a hardlink under `/private/var/tmp`,
so an existing `target/debug/sandbox` is not what discovery sees.

With a sibling `sandbox/` symlink to
`~/Library/Caches/aster/sandbox/dev-20261018-core-bash`, and again with
the parent variable set to that directory while five empty stub files
sit beside the binary, `host-visible.txt` is `E2E-TOKEN\n`, the tool
result contains `Linux`, and the host-service count rises from 0 to 1
and returns to 0 after the agent exits. Any `exit_code` scalar in that
model body is `0`.

Former's spawn writes gateway env. The same no-companion bash refuses
there too: token present, file absent, host-service count stayed 0.
Former does not boot HVF in this set.

## Commands

From the repository root. `a3s-code-acp` was already at `target/debug`.

```
ASTER_TEST_OCI_ASSETS=$HOME/Library/Caches/aster/sandbox/dev-20261018-core-bash \
  cargo test -p aster-dream --locked --lib sandbox_session -- --ignored --test-threads=1 --nocapture
```

Two verification runs: 3 passed, 0 failed, 785 filtered, 35.70s and 33.39s.
Each success oracle printed `hvf_baseline=0 hvf_max=1 hvf_after=0`.
The refusal oracle printed `hvf_baseline=0 hvf_max=0`.

```
cargo test -p a3s-desktop --locked --lib sandbox_session -- --ignored --test-threads=1 --nocapture
```

Two verification runs: 1 passed, 0 failed, 815 filtered, 1.61s and 1.54s.
The refusal oracle printed `hvf_baseline=0 hvf_max=0`.

`rustfmt --edition 2024 --check` on both `sandbox_session_e2e.rs` files
and both `acp.rs` files exited 0.

## Explicitly not claimed

No GUI session. No live model. The Chinese sentence a model uses to
restate this error is not an oracle. `mediated` was not booted. The OCI
image was not copied into either app bundle. setsid, uid map,
credentials, backslash paths, deadline, and cancel were not re-run.
Native macOS shell stays refused. A normal `cargo test` does not run
these ignored tests.
