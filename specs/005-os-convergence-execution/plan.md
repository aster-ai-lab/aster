# Implementation Plan: Aster OS 技术收敛执行（W0–W3）

**Branch**: `main` | **Spec**: [`specs/005-os-convergence-execution/spec.md`] | **Input**: [`specs/005-os-convergence-execution/spec.md`]

**Gate context**: 收敛计划（`apps/cloud/docs/convergence/plan.md`，上游子
模块）Phase 0 出口与 GA-2 验收链的 aster 侧执行；无新增产品门。

## Summary

落地史诗 specs/005 的四个执行相位：A（已完成）runtime 导出面诚实化与
backend fmt 修复；B main 全绿（会话协调落库在途修复 + 后端 check 复现）；
C Phase 0 收口（三项决策、T0.4.2 runbook 首执行、aging 回填，经上游 PR
+ gitlink 指针提升）；D GA-2 aster 侧支撑（保留绿 run 管道与证据托管）。
本计划只规划本仓库可落地的改动；`apps/cloud`、`apps/api` 的内容性修改
全部发生在 kense-labs 上游仓库，aster 侧仅指针提升与证据记录。

## Constitution Check

*GATE: Re-read `.specify/memory/constitution.md`. A NON-NEGOTIABLE
violation stops the plan.*

| Principle | Status | Evidence in the plan |
| --- | --- | --- |
| I. Fail closed | PASS | US-7 命名拒绝（`usage_export.not_wired` / `analytics.not_wired`，501）；FR-005 决策到期→下游故事拒绝启动；FR-010 保留管道私有子模块不可读时命名失败 |
| II. Evidence retained | PASS | `docs/evidence/development/2026-10-21-runtime-export-honesty/OUTCOME.md`（已落）；每相位各落 `docs/evidence/<domain>/<date-slug>/` |
| III. Hermetic tests | PASS | `apps/runtime/src/http/router/usage_export_tests.rs`（产品路径，5 passed）；US-5 三副本同套回归（共享副本路径）；不要求 live PG |
| IV. Closed sets | PASS | 不新增枚举；错误码沿用 router.rs 既有点命名闭集；拒绝集合在消息中命名 |
| V. One workspace / published APIs | PASS | `apps/runtime` 留根 workspace；`publish = false` 不变；上游包名保留（US-6 仅文档注释对齐既有包名） |
| VI. Local schema changes | N-A | 无 store schema 变更（US-5 填充既有 `payload` 字段，不改形状） |
| VII. Shrink-only ratchets | PASS | 不触碰任何 allowlist；`tests/crate_drift_report.py` 必须保持绿（FR-007）；fmt job 加强而非放宽 |
| VIII. Replaceable transports | N-A | — |
| IX. One configuration language | PASS | 不新增配置；D-A 若产生产品侧信任配置，必须 ACL（FR-012，spec 已约束） |
| XIII. One repository | PASS | 不新建仓库；上游修改经既有 kense-labs 仓库 PR；不创建 sibling checkout |
| XIV. Names are not guessed | N-A | 既有常量与闭集 |
| XV. Meaning is not matched | N-A | — |
| XVI. Pickers are drawn | N-A | 无 UI |
| XVII. Dialogs stay in place | N-A | 无 UI |

## Project Structure

### Documentation (this feature)

```text
specs/005-os-convergence-execution/
├── spec.md        # 需求与故事（含 US-6 证据修正）
├── plan.md        # 本文件
└── research.md    # D-A/D-B/D-D 决策材料 + D-F issue 与 US-5 PR 草稿
```

### Source Code

| Layer | Path | New files | 相位 |
| --- | --- | --- | --- |
| 数据面服务 | `apps/runtime/src/http/` | `router/usage_export_tests.rs`（新）；`router.rs`、`lib.rs`（改） | A ✅ |
| CI | `.github/workflows/backend.yml` | fmt job 补 `rustup component add rustfmt` | A ✅ |
| 共享 crate | `crates/lane/src/queue.rs` | payload 保真回归测试（随对账三副本同套） | C（阻塞上游双 PR） |
| 根工程 | `Cargo.toml`、`ci.yml` | 无新文件；落库在途修复（并发会话协调） | B |
| 证据 | `docs/evidence/development/` | `2026-10-21-runtime-export-honesty/`（已落）+ 后续相位 | A–D |

## Research

每个所选模式的仓内先例：

- **命名拒绝端点**：`apps/runtime/src/http/router.rs` 既有
  `route.not_ready`、`entitlement.denied`、`quota.exceeded` 点命名闭集；
  US-7 直接沿用，不引入新 pattern。
- **兄弟测试文件**：`apps/studio/src-tauri/src/agent_plane/*_tests.rs` 由
  模块 `#[cfg(test)] mod …;` 声明——`router/usage_export_tests.rs` 同法。
- **三副本对账**：`tests/crate_drift_report.py`（漂移棘轮）+ 宪法共享副
  本制度；先例 `docs/evidence/development/20261019-crate-drift-ratchet/`。
- **证据格式**：`docs/evidence/development/*` 的 OUTCOME.md 结构（命令、
  计数、Explicitly not claimed）。
- **指针提升提交**：宪法 Constraints 逐文件 stage；先例见
  `docs/migration.md` 的 backend import 记录。

## Verification Plan

- Hermetic test：`cargo test --locked -p aster-gateway-runtime`
  （当前 5 passed / 0 failed）。
- Ratchet 必须保持绿：`tests/crate_drift_report.py`（US-5 落地时）、
  `tests/workspace_dependencies.py`（US-6 残余注册时）。
- Phase B 判据：main 三 workflow 绿（远端保留 run 链接入证据）；本地
  `cargo check --locked --workspace --all-targets` 与包级 fmt 通过。
- 明确不运行：GHA 远端工作流（本会话不可触发）、上游控制面双适配器
  campaign（在上游仓库履行）、live PostgreSQL（产品/共享副本路径不需要）。

## Implementation Phases

| 相位 | 内容 | 状态 | 阻塞 |
| --- | --- | --- | --- |
| A | US-7、FR-002、US-6 修正、证据、research.md | ✅ 2026-10-21 | — |
| B | 在途修复落库（会话协调）；后端 check 复现；main 全绿 | 进行中 | 并发会话在途树（fmt 红 8 文件归属其名下） |
| C | D-A/D-B/D-D 拍板（材料已备）；T0.4.2 runbook；aging 回填；US-5 对账 | 待启动 | owner 决策、kense-labs 双 PR、D-F |
| D | 保留绿 run 管道；GA-2 指针提升与证据托管 | 待启动 | D-B、上游 GA-2 进展 |
