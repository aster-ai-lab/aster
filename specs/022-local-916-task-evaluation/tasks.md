---
description: "Local 9.1.6 integration and 28 native Aster evaluations"
---

# Tasks: 本地 9.1.6 集成与 Aster 任务评测

**Input**：[spec.md](spec.md)、[plan.md](plan.md)、[research.md](research.md)。
**House rule**：每项行为修改同时交付 owning-module 小测试；不预标共享窗口、工作区或运行状态的并行任务。

## Phase 1: 基线与真实接入

- [x] T001 [US1,US3] 核对 root/Studio dirty 基线、Core/ACP 本地 9.1.6、构建与实际运行路径；冻结 25 个 catalog ID 加 3 个额外 case、原输入摘要和精确 allow/deny；原始材料只读。修改准备逻辑时以临时文件测试答案泄漏、符号链接和越界拒绝。
- [ ] T002 [US1] 使用固定 pnpm 和锁文件、根 `just build-studio aster` 构建配套程序；核对 `agent_plane/launcher.rs` 的实际选择与摘要，运行 `tests/workspace_dependencies.py` 及受影响 launcher/协议测试。若需修复接入，测试必须证明错误显式路径拒绝、不回退旧 sidecar。
- [ ] T003 [US2] 使用原实例隔离启动独立 Aster fork，核实新 profile/窗口/进程且原用户进程未终止。沿用 `cua_guard.py` 和无坐标驱动；所有新路径在单测中拒绝前台/坐标/指针/不明投递与 addressed hotkey，原始事件必须证明后台投递。

## Phase 2: 评分与首条链路

- [ ] T004 [US3] 在私有 Studio `tests/e2e/task-catalog` 补齐 Office 缺失的内容检查与 OCR 依据；保持白名单和原任务要求。每项新检查都有正确产物与错数值、错行关系、损坏/空产物等反例，不以词语散落判语义成功。
- [ ] T005 [US3] 在同一测试目录复用既有 reference-alignment 设施接入 T01、git-leak-recovery、regex-log；夹具及输出仅位于本仓库 target。增加 T01 确定性/变体/独立最优解检查、正则边界与跨行检查、Git 合法历史与对象清理检查，拒绝参考解法或原件混入受测工作区。
- [ ] T006 [US1-US3] 先观察新 fork，再以真实模型运行 CAT-MAIL、一个表格任务及一个扫描件任务。验证输入、许可、消息、产物与 Run 收尾；稳定后复用为受保护批量流程，未知 UI 或过期 token 必须重新观察。

## Phase 3: 完整执行、修复与证据

- [ ] T007 [US3,US4] 完成 28 个实际任务，逐项保存当前构建与独立断言、后台动作、耗时/调用/token、原输入未变证明；失败不跳过，计量不可用不编造。
- [ ] T008 [US1-US4] 按原始失败单独复现，在 Core/ACP/Office 或私有 Studio owning module 修复；每项行为修复补 hermetic 反例，保留首次失败，不通过改变题目或评分消除失败。重新验证失败及受影响集合，最终结果绑定同一当前构建。
- [ ] T009 [US1-US4] 完成下方最终门禁，并写 `docs/evidence/desktop/2026-10-26-local-916-task-evaluation/OUTCOME.md`，列实际结果、源码/二进制身份、每项独立结果、效率、既有问题和未验范围。原始私有数据留 target；未经本轮明确请求，不提交/推送 Studio 改动。

## Requirement coverage

| Requirement | Tasks |
| --- | --- |
| FR-001 | T001,T002,T006,T009 |
| FR-002 | T003,T006,T007,T009 |
| FR-003 | T001,T004,T005,T007 |
| FR-004 | T006,T007,T009 |
| FR-005,FR-006 | T004,T006,T007,T008 |
| FR-007 | T002,T003,T006,T007,T008 |
| FR-008 | T005,T007,T008 |
| FR-009 | T007,T008,T009 |
| FR-010 | T001,T002,T008,T009 |

## Final Verification Gate

- [ ] 受影响 owning package 的 focused 测试通过；新评分与输入隔离的负例不能通过。
- [ ] Rust 修改执行 `cargo fmt --all -- --check` 并单列未改动的既有失败；前端修改执行所需类型、i18n、格式及 focused 测试，不清理他人改动。
- [ ] 无新的 crates.io A3S 依赖、第二 Cargo workspace/lockfile、SDK bindings 或私有源码复制。
- [ ] 无 native select；若产品配置有改动仍为 a3s-acl 解析的 ACL，不增加模型授予权限的路线。
- [ ] 28/28 当前独立结果通过；扫描件、额外代码任务、结构/内容/变体检查与预算均未被豁免。
- [ ] 后台动作全部有证据，无坐标、鼠标、前台或不明投递；原用户窗口和原任务材料未被修改。
- [ ] OUTCOME 保留失败与成功、版本/输入/输出身份、效率及 Explicitly Not Claimed；不把旧报告沿用为本次资格。
