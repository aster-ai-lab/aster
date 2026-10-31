# Implementation Plan: 本地 9.1.6 集成与 Aster 任务评测

**Branch**：当前工作区 | **Spec**：[spec.md](spec.md)
**Gate context**：Product regime；本地 Core/ACP、私有 Studio、Office 和测试工具。

## Summary

先核验已有本地路径依赖与配套构建，避免添加重复集成层。扩展现有 task-catalog 的验收范围，使用独立 Aster fork、精确上传白名单和强制后台 CUA。先跑少量真实任务确认操作链，再完成 28 项，按实际失败修复上游 owning module。保留原输入、失败和模型开销，不以历史结果或宽松评分代替当前通过。

## Constitution Check

| Principle | Status | Evidence in the plan |
| --- | --- | --- |
| I. Fail closed | PASS | 身份不符、后台投递不明、无产物/终态/评分、OCR 不可用均不通过 |
| II. Evidence retained | PASS | T009 的 OUTCOME 与每次尝试回执，原始敏感输出只留本地 |
| III. Hermetic tests | PASS | T002–T005、T008 各自携带拒绝及近似错误反例 |
| IV. Closed sets | PASS | 使用明确 case ID、类型化结果与 CUA schema；未知值拒绝 |
| V. One workspace / APIs | PASS | 根 Cargo/锁文件、local paths、保留 Rust API |
| VI. Local schema changes | N-A | 不计划数据库迁移；若实测需要则在 owning context 另行证明 |
| VII. Shrink-only ratchets | PASS | 不放宽现有权限、输入白名单或评分门槛 |
| VIII. Replaceable transports | PASS | 原 ACP/RuntimeClient 与已有 CUA 驱动，不新增执行循环 |
| IX. One configuration language | PASS | 宿主 ACL 由 aster-acl 解析；不用旧测试代理的手工 ACL parser |
| XIII. One repository | PASS | 测试夹具仅在本仓库 target 下；不在同级 tasks 目录建仓库或写结果 |
| XIV. Names are not guessed | PASS | 显式目录、catalog ID、模型元数据与精确二进制来源 |
| XV. Meaning is not matched | PASS | 不以关键词列表推断成功；检查实体/行关联、数值和原始证据 |
| XVI. Pickers are drawn | N-A | 不计划新增产品选择器；如需修复沿用现有控件 |
| XVII. Dialogs stay in place | N-A | 不计划修改对话框结构或动效 |

## Project Structure

| Layer | Existing path / prior art | Planned change |
| --- | --- | --- |
| 根构建 | `justfile`、`tests/workspace_dependencies.py` | 使用配套构建与来源检查；仅在实际不一致时修复 |
| SDK / ACP | `crates/code/core`、`crates/code/acp` | 保持 9.1.6 本地依赖；按实测失败补 owning-module 测试与修复 |
| Studio 本地接入 | `apps/studio/src-tauri/Cargo.toml`、`src-tauri/src/agent_plane/launcher.rs` | 核对 Core path、明确 sidecar、实际进程和 ACL；不复制私有源码到父库 |
| 实例隔离 | `apps/studio/scripts/studio-dev.mjs`、`src-tauri/src/tenant_profile.rs` | 复用 `just fork aster` 和独立 profile，不停止用户实例 |
| 后台测试 | `apps/studio/tests/e2e/task-catalog/cua_guard.py`、`settings/cua_driver.py` | 复用后台保护；新路径不能使用旧的坐标例外、截图或前台恢复 |
| Office 评测 | `apps/studio/tests/e2e/task-catalog/{catalog,common,grade,window}.*` | 保留 25 项及白名单，补缺失内容检查和扫描件基准，不信任仅关键词评分 |
| 代码评测 | `../tasks/{T01,git-leak-recovery,regex-log}` 与既有 reference-alignment | 在测试层显式加入 3 项；只复制受测输入和准备好的环境，解法/测试不暴露给模型 |
| 证据 | `docs/evidence/desktop/2026-10-26-local-916-task-evaluation` | 脱敏身份、计数、断言和指标；原始产物置于 `target/aster-task-benchmark-20261026` |

## Research

见 [research.md](research.md)。测试执行和回执合同见 [contracts/evaluation.md](contracts/evaluation.md)。[tasks.md](tasks.md) 是待执行清单，不预填通过。

## Verification Plan

1. `cargo metadata --locked --offline` 与根依赖检查：同名 Core/ACP 只能来自本地 9.1.6。
2. 固定 pnpm 版本及锁文件，按根 `just build-studio aster` 构建 host/ACP/Office；检查实际路径、摘要和本轮 source fingerprint。
3. 后台 guard、输入准备及内容检查的 Python 小测试；修改 Rust/renderer 时补 owning-package 测试和相应格式/类型门。
4. 独立 fork 中先验证 CAT-MAIL、一个表格任务和一个扫描件任务的真实链路；动作都以新快照寻址、带后台回执。
5. 28 项实际执行、独立内容验收、预算与原始输入摘要核对。首次失败单独复现，修复后复验受影响集合；最终结果必须指向同一合格构建。
6. 保留全部失败、未完成和计量缺失。未解决基础设施或 provider 缺口不标通过；不运行无关全仓修复，不改写历史证据。

## Clarifications and limits

任务路径已由用户确认，无待决路径标记。OCR 和新评分器是执行工作，不作为豁免。现有 020 产品规格已延期且不属于本期；本期只评测真实任务并修复造成失败的既有接入或运行行为。私有 Studio 的更改仍留在其子仓库。
