# Implementation Plan: 项目与任务工作区 Wiki

**Branch**: 当前工作区；不创建分支、不提交 | **Spec**: [spec.md](spec.md)

**Gate context**: 工作页 Product regime + 原 OKF 支持 crate。当前优先级替代 020；用户已授权实施。

## Summary

从工作页侧边栏项目/任务分组的“更多”打开类似设置、铺满应用内容区的卡片目录；选择对象后直接复用原知识页的百科、产物、记忆与最后的看板、多标签编辑器、图谱与来源预览。生成结构化页面和适用图示，随文件变更增量更新。普通任务自行判断是否调用 `search_wiki` 或 `search_memory`，默认跨当前实例已保存的合法工作区检索，也可限定当前范围，不保留独立问答。复用原检索、代码理解、记忆 owner、OKF 维护和 Core/ACP；主要新增是精确绑定、来源适配和目录/详情的产品体验。

## Constitution Check

PASS 指计划符合约束，实施证据尚未产生。

| Principle | Status | Evidence in the plan |
| --- | --- | --- |
| I. Fail closed | PASS | 无 scope、越权、陈旧来源/候选均拒绝或报告覆盖缺口 |
| II. Evidence retained | PASS | T011 记录实际命令、来源/二进制身份、失败与未验范围 |
| III. Hermetic tests | PASS | T001–T010 每个行为任务携带 owning-package 正反例 |
| IV. Closed sets | PASS | workspace/personal 模式、操作、状态与来源锚点类型化 |
| V. Workspace / APIs | PASS | 原根工作区、本地 API；新增小型适配，无外语 SDK |
| VI. Local schema | PASS | 原维护任务/发布存储内演进，无新数据库 |
| VII. Ratchets | PASS | 不放宽 personal 根限制、权限/egress 或运行时单权威 |
| VIII. Transports | PASS | 原 ACP / RuntimeClient；无 UI 直连模型/文件系统 |
| IX. Configuration | PASS | ACL 继续控制预算和后端，Markdown/manifest 只作数据 |
| XIII. Repository | PASS | 规划在父仓库，Studio 代码仅在既有私有子模块 |
| XIV. Names | PASS | 项目/任务/文件来自原绑定，主题名由模型结合来源提供 |
| XV. Meaning | PASS | 模型理解内容，禁止路径/关键词启发式推导业务语义 |
| XVI. Pickers | PASS | 原设计系统控件，不加 native select |
| XVII. Dialogs | PASS | 设置式铺满内容区覆盖层，原地进出、焦点/后台交互隔离、reduced-motion |

## Project Structure

```text
specs/021-workspace-wiki/
├── spec.md
├── plan.md
├── research.md
├── data-model.md
├── contracts/workspace-wiki.md
└── tasks.md
```

### Source Code

| Layer | Extend existing code | Add only what is missing |
| --- | --- | --- |
| 绑定/文件 | Studio `src/shared/data/api/schemas/agentWorkspaces.ts`、`src-tauri/src/agent_plane/domain.rs` / `store.rs`、`workspace_entries.rs`、`workspace_watch.rs`、`workspace_snapshot.rs` | 宿主 workspace Wiki 绑定/代次、当前读取范围与输出位置；保持原项目/任务权威 |
| 来源与理解 | Core `src/workspace/retrieval/`、`workspace/source_egress.rs`、`code_intelligence/service.rs`；原 document readers | 有界来源快照及精确锚点；LSP 可选，不装新转换/向量服务 |
| 维护/发布 | `crates/okf/src/personal/maintenance.rs`、`personal.rs` | 最小抽取共用原语，小型 workspace source adapter 与 sibling tests；保留 personal API/约束 |
| 受控执行 | `crates/code/acp/src/knowledge_compile.rs`、`host_context.rs`、原会话/Run 路径 | 类型化 workspace compile scope，原执行/预算/取消及产物摘要校验 |
| 索引/检索 | Core `src/workspace/retrieval/wiki.rs`、ACP `src/wiki_search.rs` / `memory_search.rs`、原 workspace retrieval / memory owner | 原索引的有界跨任务检索、去重与当前性；宿主解析绑定并隔离实例，既有离线向量异步构建，不混入 personal/team |
| 桌面原生 | Studio `src-tauri/src/agent_plane/runtime_client.rs`、原知识命令与 host 注册/permissions/capabilities | 类型化 Wiki 命令，扩展原维护/状态路径，无第二模型客户端 |
| 目录与入口 | Studio `src/renderer/pages/agents/WorkbenchHome.tsx`、`src/renderer/components/layout/AppShell.tsx` | 两个分组更多始终可用；一套 project/task 卡片目录与同层 Wiki 详情；同步 DESIGN、shared 类型和 i18n |
| 覆盖层交互 | Studio `src/renderer/services/settingsSurfaceService.ts`、`components/command/CommandProvider.tsx`、`hooks/useWindowRuntime.ts` 及原 Settings 键盘/关闭路径 | 复用既有后台保活与命令隔离，显式 Wiki 身份；不复制设置运行时或把 Wiki 伪装成 settings route |
| 组件复用 | Studio `src/renderer/pages/knowledge/KnowledgeWorkbench.tsx`、`noteReading.ts`、`knowledgeReveal.ts`、`components/Preview/MermaidPreview.tsx` | 复用可独立部分，不复制整个个人知识页或 renderer 维护运行时 |

表中 Core 相对 `crates/code/core/`；Studio 相对私有 `apps/studio/`。

## Research

详见 [research.md](research.md)、[data-model.md](data-model.md) 和 [交互/协议合同](contracts/workspace-wiki.md)。personal 维护保留原 `wiki/local` 约束；工作区维护复用原 OKF owner、原生 PlaneStore 与已保存绑定。按 2026-10-27 用户修订，普通任务可自行调用 `search_wiki` 与 `search_memory` 跨当前实例已保存的工作区检索。原异步 catalog/持久 FTS、记忆存储与离线向量继续由原 owner 拥有，不增建服务或模型客户端。

## Delivery phases and estimates

假设一名熟悉 Rust/React 的工程师，每人日约 6 小时有效开发；净估算不含 019 依赖等待。M0 完成后按复用缺口重估。

| 阶段 | 用户可见交付 / 退出条件 | Tasks | 人日 |
| --- | --- | --- | --- |
| M0 工作区范围 | 项目、独立任务、子任务精确绑定；无权限/改绑处理清楚 | T001 | 2–3 |
| M1 来源与维护适配 | 代码/资料可作为有出处的只读快照；原 personal Wiki 不回归 | T002–T003 | 3–4 |
| M2 首次 Wiki | 真实维护任务产出概览、目录、主题/流程、图示与合法引用 | T004–T005 | 3–4 |
| M3 目录、阅读和检索 | 分组更多、全页卡片目录、原知识页图谱／编辑器、范围锁定检索与来源预览 | T006–T008 | 3–4 |
| M4 持续更新 | 增量过期/更新、同范围复用、归档／恢复／目录变化／重启正确 | T009–T010 | 2–3 |
| M5 资格 | 真实模型与原生完整旅程、独立正反例、配对构建及证据 | T011 | 2–3 |

**原始估算合计 15–21 人日，约 3–5 个工作周，不作为当前剩余工期。** M3 演示生成、阅读、图谱和任务检索，M4 补齐持续理解，M5 才能声明本期完成。

## Dependency and release gates

- M1/M2 可先在原 OKF/Core/ACP 进行。桌面集成依赖 019 已合格的 RuntimeClient、会话/取消/单写者边界；不等待 020。
- 文本 Wiki 理解不需要执行项目业务代码；编译保持只读源及受限候选写入，不以放宽 macOS Shell 资格为前置手段。
- 项目文件本身可以持续编辑；生成在快照上进行，发布复核代次，不能把 personal 源修改锁直接施加到用户开发工作区。
- 首版机器生成的 Wiki 页校验后可自动刷新；不可借共享维护代码改变个人手写笔记的审阅/保存逻辑。
- 绑定、UI、来源、编译、检索和监视共享既有状态，按完整纵向切片交付；不先上线空 Wiki 按钮或假进度。

## Verification Plan

- Owning-package：`cargo test --locked -p aster-okf workspace_wiki`、`cargo test --locked -p aster-core --lib workspace_wiki`、`cargo test --locked -p aster-acp workspace_wiki`、`cargo test --locked -p aster-studio workspace_wiki`；新测试按该前缀组织，并补跑受影响 personal/retrieval/compile 既有测试。这些是计划命令，尚未执行。
- Renderer/shared：在 Studio 子模块执行现有 Vitest 对本期精确路径的聚焦测试；覆盖分组更多常显、全内容区覆盖、搜索/分页/卡片/空态、同层详情与返回、来源预览、焦点、后台快捷键隔离、窄窗口及中英文。保留原对象选择/管理操作可达。
- 独立夹具：小代码项目验证入口和跨文件关系；纯资料目录验证资料到产物关系；混合工作区验证双类引用；不同目录/同目录异权限验证隔离。
- 增量：改、删、重命名、源变化中发布、无变化重开、输出回灌、取消、模型失败、项目归档／恢复、合法目录变化及重启；记录真实生成请求，证明不重复触发。
- 质量：固定问题集核验回答的来源合法性与要点，保留不完整/错误回答；结构化引用校验不能冒充语义完全正确。脚本模型证明协议，真实模型单列内容质量与耗时/用量。
- 原生：更多→搜索卡片→项目生成→项目内/独立任务阅读→图谱／多标签编辑→来源预览→返回目录→关闭恢复，加源改变/更新和普通任务检索；验证目录浏览零新增生成与后台草稿不丢。保留当前源码/配对 ACP/桌面身份。未执行平台/格式不计通过。
- 仓库：`cargo fmt --all -- --check`、依赖/运行时权威检查、受影响类型/i18n 和 CI。既有失败单列，不放宽权限、egress、个人目录或架构基线。

## Requirement-to-task coverage

| Requirement | Producing tasks |
| --- | --- |
| FR-001 | T001, T010 |
| FR-002 | T001, T007 |
| FR-003 | T002 |
| FR-004 | T004 |
| FR-005 | T003, T005 |
| FR-006 | T002, T004, T006, T008 |
| FR-007 | T005, T006, T008 |
| FR-008 | T009 |
| FR-009 | T003, T009, T010 |
| FR-010 | T007, T008 |
| FR-011 | T003, T009 |
| FR-012 | T001, T003, T005, T010 |
| FR-013 | T011 |
| FR-014 | T001, T007, T011 |

## Deferred scope

先把核心阅读、编辑、图谱、任务检索、出处和更新做好。独立百科问答已按用户要求删除；2026-10-27 修订纳入当前实例跨任务百科/记忆检索及原文档工具转换。跨实例全局索引、聊天来源导入、远程公开发布、可编辑知识图谱、任意格式转换保证以及 020 业务验收不进入本轮实现。后续依据真实工作区的理解正确性、首次可读时间、增量成本和重复使用情况排期。
