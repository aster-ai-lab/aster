# OUTCOME: ksandbox process Bash sandbox

Date: 2026-11-07

## Change

Aster Core default `BashSandbox` wraps locked **ksandbox**
(`kense-labs/ksandbox` git rev `6c7c8bdd…`): Seatbelt / namespaces /
AppContainer. Aster overlays write-deny on `.aster` (with `.aster/loops`
carve-out). MicroVM **kbox** stays orthogonal and is not on this path.
Aster CLI package version for the next release is **0.2.0**.

Optimizations in this pass:

- macOS `shell_note` documents Seatbelt setsid descendant gap
- Session install e2e asserts in-workspace allow + `.aster` / outside deny
- macOS background / bash oracles expect real execution (not refuse-only)

## Commands run

```text
cargo build --locked -p aster -p aster-acp
cargo test -p aster-core --lib sandbox:: -- --test-threads=4
cargo test -p aster-core --lib installed_default_sandbox_enforces -- --nocapture
cargo test -p aster-core --lib native_background_denies -- --nocapture
cargo test -p aster --test cli_p0 -- --nocapture
cargo test -p aster-acp --test shipped_tui_e2e -- --test-threads=1
# live (isolated HOME + ASTER_CONFIG deepseek-only ACL):
# target/debug/aster -p '…printf ksandbox-live-ok > ksandbox-live.txt…' \
#   --permission-mode auto --trust --model deepseek/deepseek-flash
```

## Results

| Gate | Result |
| --- | --- |
| `aster-core` `sandbox::` | **30 passed** |
| Session boundary install | **pass** |
| Background outside + network deny | **2 passed** |
| CLI P0 (`shipped_cli_p0_oracles`) | **pass** (incl. `shell-workspace-boundary`) |
| `shipped_tui_e2e` hermetic | **18 passed**, 1 ignored (live bound-goal) |
| Live DeepSeek sandboxed bash | **sentinel `ksandbox-live-ok` written**; no `native_process_lifetime_unqualified`; process exit 1 from completion-gate verification (not sandbox refuse) |

Artifacts: `artifacts/`.

## Explicitly not claimed

- macOS Seatbelt does **not** reclaim `setsid()` descendants; escape suites
  record `setsid qualification=unsupported` without claiming containment.
- Live turn did not satisfy the completion-gate verification/waiver digest;
  sandbox execution itself succeeded (workspace file present).
- `apps/cloud` still vendors a refuse-only native stub; not migrated.
- Full `cargo test --workspace` and full Layer-C TUI recovery matrix were not
  re-run in this pass.
- kbox MicroVM integration remains out of scope.
