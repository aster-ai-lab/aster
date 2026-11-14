# Aster 项目概览

> 这是给智能体快速上手的**导航文件**，不是规则文件。
> **权威规则与契约在 [`AGENTS.md`](AGENTS.md)**；两者冲突时以 `AGENTS.md` 为准。
> 子目录还有各自的 `AGENTS.md` / `CLAUDE.md`，进入对应路径前应先读。

## 一句话

Aster 是一个 Rust 工作区，包含终端编码工具（Aster CLI）、可嵌入的 Agent 执行内核（Core / ACP），以及共享的桌面宿主（Aster Studio）。CLI 与 Studio 都通过 ACP 调用 Core；Core 管理模型、工具与完成判定，Effect 用事实日志驱动执行与恢复。

## 三条产品线（版本彼此独立）

| 产品线 | 源码 | 版本来源 | 发布 tag |
| --- | --- | --- | --- |
| Aster CLI（`aster` + `aster-acp` 成对交付） | `apps/aster`, `crates/code/acp` | `apps/aster/Cargo.toml` | `aster-cli-vX.Y.Z` |
| Core / ACP 源码与平台归档 | `crates/code/core`, `crates/code/acp` | 两个 Cargo.toml 必须一致 | `aster-acp-vX.Y.Z` |
| Aster Studio（桌面版） | `apps/studio` | 独立 | `vX.Y.Z`（仓库 Latest 归它） |

发布契约见 [`docs/release-governance.md`](docs/release-governance.md)：CLI 与 Core/ACP 一律 `--latest=false`，滚动通道是 `aster-cli-stable`，版本化 Release 与 tag 有保留窗口要求。

## 目录地图

| 路径 | 内容 |
| --- | --- |
| `apps/aster/` | CLI 二进制、`src/cli_pair_update.rs` 更新器、`tests/` Rust 集成测试 |
| `apps/studio/` | 桌面宿主（独立子模块，落地前需 `just studio-land-check`） |
| `apps/web/`, `packages/` | 站点与前端共享包 |
| `crates/code/core`, `crates/code/acp` | Agent 内核与 ACP 协议层 |
| `crates/terminal/` | TUI 渲染（pager / prompt widget / PTY harness） |
| `crates/effect`, `crates/flow`, `crates/power`, `crates/memory` 等 | 执行、编排、推理、记忆等基础 crate |
| `scripts/` | `install.sh` / `install.ps1` 安装器与发布脚本 |
| `.github/workflows/` | 构建、资格校验（`qualify-*`）、发布（`release-*`）流水线 |
| `tests/` | Python 治理测试（如 `tests/release_governance.py`） |
| `docs/release-governance.md`, `docs/evidence/` | 发布契约与行为变更的证据记录 |
| `.runtime/` | **本地评估产物，非源码**；改动不影响构建输入 |

## 常用命令

先看 `just --list`。常用入口：

- `just build` — 把 `aster` 与 `aster-acp` 构建进同一 `target/`（默认走 `cargo build --locked`，可能自动启用 sccache）
- `just code *args` — 启动终端，保留 shell 引号转发参数
- `just build-studio` / `just studio` — 桌面宿主构建与热重载启动
- `just studio-land-check` — Studio 落地前检查（脏树、gitlink 回退等）
- `just studio-contracts` — 渲染器契约测试
- `cargo fmt --all -- --check`、`cargo clippy --locked -p <package> --all-targets --no-deps` — Rust 检查

工具链固定在 `rust-toolchain.toml`（Rust 1.97.1 + clippy/rustfmt + minimal）。**不要**改动 ABI / libc / linker 基线，也不要引入 `target-cpu=native` 之类的机器局部 flag。

## 测试与证据口径

- 行为变更需要在 `docs/evidence/<domain>/<date-slug>/OUTCOME.md` 留下命令、计数与「Explicitly not claimed」小节；没跑过的检查不能写成通过。
- PTY / 静态 / Rust 套件是 suite-level 证据，**不等于** agent-desktop 可视化 E2E；判定必须用独立 oracle（relay prompt 计数与 digest、磁盘字节、AX + PNG），屏幕关键字单独不算通过。
- GUI E2E 前必须从当前 checkout 重新构建被测应用；`ps` 不可用的会话里 N lane 用例应保持 BLOCKED，不得用其它结果顶替。
- 覆盖率分母与 catalog hash 是冻结口径，不要为了好看换分母或删失败项。

## 智能体上手顺序

1. 读 `AGENTS.md`（权威规则），再读目标路径自己的 `AGENTS.md` / `CLAUDE.md`。
2. 用 `rg` / `rg --files` 从入口和直接依赖定位，不要先扫全仓。
3. 改动前看该路径的 regime 与已有改动；不要顺手重构相邻代码。
4. 按 `AGENTS.md` 的 Verification 章节跑检查；Rust 改动前初始化 `apps/studio`。
5. 提交与推送分开，推送需单独授权。

---

本文件只做导航。具体规则、验证清单、Studio 落地约束、Rust 实现规范，一律以 `AGENTS.md` 及其引用的文档为准。
