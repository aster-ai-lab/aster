# Linux x86_64（OrbStack）Aster TUI e2e

## Verdict

**pass** — 在 OrbStack Linux amd64 虚拟机内，对当前 checkout 编译的 `aster` / `aster-acp` 跑通 `shipped_tui_e2e`：**19 passed / 0 failed / 1 ignored**（`shipped_bound_goal_real_llm`）。

## Environment

| Item | Value |
| --- | --- |
| Host | macOS arm64 + OrbStack 2.2.5 |
| VM | `aster-tui-linux-x86` · Ubuntu 24.04 (noble) · **x86_64 / amd64** |
| Kernel | `Linux aster-tui-linux-x86 7.2.12-orbstack-00394-gbecdd6130068 x86_64` |
| VM resources | ~31 GiB available RAM, 17 CPUs visible, disk on shared OrbStack volume |
| Toolchain | rustc/cargo `1.97.1`（`rust-toolchain.toml`） |
| Target dir | `$HOME/aster-linux-x86-target`（与宿主机 `target/` 隔离） |
| Pair under test | `aster 0.3.2` + `aster-acp 9.2.1`（本树 Linux x86_64 ELF） |

## Commands

```bash
# VM
orb create -a amd64 --cpus 6 --memory 16G --disk 64G ubuntu:24.04 aster-tui-linux-x86

orb -m aster-tui-linux-x86 bash -lc '
  sudo apt-get install -y build-essential pkg-config libssl-dev curl ca-certificates \
    git python3 clang cmake protobuf-compiler libprotobuf-dev file
  curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain none
  . "$HOME/.cargo/env"
  cd /Users/roylin/code/aster
  rustup toolchain install 1.97.1
  export CARGO_TARGET_DIR="$HOME/aster-linux-x86-target"
  export CARGO_PROFILE_DEV_DEBUG=0
  cargo build -p aster -p aster-acp --locked -j 2
  cargo test -p aster-acp --test shipped_tui_e2e --locked --no-run -j 2
  export ASTER_TUI_BIN="$CARGO_TARGET_DIR/debug/aster"
  export ASTER_ACP_BIN="$CARGO_TARGET_DIR/debug/aster-acp"
  cargo test -p aster-acp --test shipped_tui_e2e --locked -- --nocapture --test-threads=1
'
```

## Results

| Suite | Result | Notes |
| --- | --- | --- |
| `aster-acp` `--test shipped_tui_e2e` | **ok** | 19 passed; 1 ignored (`shipped_bound_goal_real_llm`); **170.41s** |
| Oracle lines (`result=pass`) | 151 | Full log: `shipped_tui_e2e.log` |
| Pair build | **ok** | `build.log`; ELF x86-64; ~22m under qemu-amd64 |

Covered surfaces include PTY launch/resume/palette/commands, permission reject/cancel, prefix-fork, wire/session identity, retrieval stdio cases, and readiness helpers.

## Issues hit and mitigations

1. Apple Silicon host runs the amd64 VM under OrbStack emulation — full pair build took ~26 minutes; used `CARGO_PROFILE_DEV_DEBUG=0` and `-j 2`.
2. Avoided overwriting host Mach-O under `target/debug/` by isolating `CARGO_TARGET_DIR` and pointing the harness at `ASTER_TUI_BIN` / `ASTER_ACP_BIN`.

## Explicitly not claimed

- Live DeepSeek / `shipped_bound_goal_real_llm` (ignored by design).
- Native (non-emulated) Linux x86 hardware.
- Published `aster-cli` channel archive on this VM (built from the current tree instead).
- macOS agent-desktop visual TUI harness inside Linux.
- Full workspace / Studio GUI e2e on Linux.
