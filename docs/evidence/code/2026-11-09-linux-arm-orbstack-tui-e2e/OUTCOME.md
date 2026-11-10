# Linux ARM（OrbStack）Aster TUI e2e

## Verdict

**pass** — 在 OrbStack Linux aarch64 虚拟机内，对当前 checkout 编译的 `aster` / `aster-acp` 跑通 `shipped_tui_e2e`：**19 passed / 0 failed / 1 ignored**（`shipped_bound_goal_real_llm`）。

## Environment

| Item | Value |
| --- | --- |
| Host | macOS arm64 + OrbStack 2.2.5 |
| VM | `aster-tui-linux-arm` · Ubuntu 24.04.5 (noble) · aarch64 |
| Kernel | `Linux aster-tui-linux-arm 7.2.12-orbstack-00394-gbecdd6130068 aarch64` |
| VM limits | memory 28672 MiB（链接 CLI 时从 8192 上调）, CPU 6, disk 64G |
| Toolchain | rustc/cargo `1.97.1`（`rust-toolchain.toml`） |
| Target dir | `$HOME/aster-linux-arm-target`（与宿主机 `target/` 隔离） |
| Pair under test | `aster 0.3.1` + `aster-acp 9.2.1`（本树 Linux aarch64 构建） |

## Commands

```bash
# VM
orb create -a arm64 --cpus 4 --memory 8G --disk 64G ubuntu:24.04 aster-tui-linux-arm
# later raised:
orbctl config set memory_mib 32768
orbctl config set machine.aster-tui-linux-arm.memory_mib 28672
orbctl config set machine.aster-tui-linux-arm.cpu 6
orbctl stop && orbctl start && orb start aster-tui-linux-arm

orb -m aster-tui-linux-arm bash -lc '
  sudo apt-get install -y build-essential pkg-config libssl-dev curl ca-certificates \
    git python3 clang cmake protobuf-compiler libprotobuf-dev file
  curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain none
  . "$HOME/.cargo/env"
  cd /Users/roylin/code/aster
  rustup toolchain install 1.97.1
  export CARGO_TARGET_DIR="$HOME/aster-linux-arm-target"
  cargo test -p aster-acp --test shipped_tui_e2e --locked --no-run
  CARGO_PROFILE_DEV_DEBUG=0 cargo build -p aster --locked -j 2
  # stage into hardcoded e2e path ../../../target/debug/aster (backup Mach-O first)
  cargo test -p aster-acp --test shipped_tui_e2e --locked -- --nocapture --test-threads=1
'
```

## Results

| Suite | Result | Notes |
| --- | --- | --- |
| `aster-acp` `--test shipped_tui_e2e` | **ok** | 19 passed; 1 ignored (`shipped_bound_goal_real_llm`); **149.12s** |
| Oracle lines (`result=pass`) | 151 | Full log: `shipped_tui_e2e.log` |

Covered surfaces include PTY launch/resume/palette/commands, permission reject/cancel, prefix-fork, wire/session identity, retrieval stdio cases, and readiness helpers.

## Issues hit and mitigations

1. **8 GiB VM OOM on `aster` link** (`ld` SIGKILL). Raised OrbStack machine memory to 28 GiB and rebuilt with `CARGO_PROFILE_DEV_DEBUG=0`.
2. Missing **`protoc`** / **`file`** packages — installed via apt.
3. Published `aster-cli-v0.3.1-linux.tar.gz` is **x86_64 only** — not usable on this ARM VM; built from source instead.
4. E2E hardcodes `target/debug/aster`; Linux ELF was staged over the host Mach-O temporarily, then host `aster` / `aster-acp` were restored to Mach-O after the run.

## Explicitly not claimed

- Live DeepSeek / `shipped_bound_goal_real_llm` (ignored by design).
- macOS agent-desktop **visual** TUI harness inside Linux.
- Published linux-arm64 CLI archive (none in the 0.3.1 release assets checked).
- Full workspace / Studio GUI e2e on Linux.
- That host `target/debug/aster-acp` after restore matches the exact pre-run Studio-staged bit-identity beyond `file` Mach-O + `--version` (restored from `target/debug/agent/aster-acp`, which reported `9.2.0`).
