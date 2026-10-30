# Research: 工作区 Wiki 的复用与缺口

**Date**: 2026-10-25；静态源码调研，未对用户项目运行模型。

**历史记录，非当前功能合同**：以下缺口和早期方案保留调研时点。2026-10-26/27 的用户修订及实现以 [spec.md](spec.md) 和 [协议合同](contracts/workspace-wiki.md) 为准：独立问答和目录新建卡片已删除，原生项目注册表成为单一权威，普通任务只检索当前工作区百科。实际 Wiki 位于绑定给出的 `sourceRoot/wiki/workspace/<身份>/`，由 source admission 排除，不作为新的来源。真实模型与性能验收见[当前证据](../../docs/evidence/desktop/2026-10-26-workspace-wiki/OUTCOME.md)。

## Existing mechanisms

| 需求 | 已有路径 | 本期决定 |
| --- | --- | --- |
| 项目与任务目录 | `apps/studio/src/shared/data/api/schemas/agentWorkspaces.ts`、`renderer/pages/agents/workbenchWorkspace.ts`、`sessionWorkspace.ts`、原生 `agent_plane/domain.rs` / `store.rs` | 项目/任务身份与真实 cwd 解析，移动/改绑用代次失效；不能把项目名拼成知识范围 |
| 工作区浏览/监视 | `src-tauri/src/workspace_entries.rs`、`workspace_watch.rs`、`workspace_snapshot.rs`、`workspace_document.rs` | 用已有有界目录与对账；识别原产品运行状态目录并排除，不扫描 app 根目录 |
| 源检索与准入 | `crates/code/core/src/workspace/retrieval/`、`workspace/source_egress.rs` | 复用原文件 manifest、块、准入与传输限制；语义 provider 不可用时原词法/源码路径仍可用 |
| 代码结构 | `crates/code/core/src/code_intelligence/service.rs` | 符号/引用增强理解，LSP 不可用有明确降级，不安装新解析/图谱体系 |
| 个人 Wiki 维护 | `crates/okf/src/personal/maintenance.rs`、`personal.rs` | 复用任务/快照/staging/来源校验/发布原语；增加工作区来源适配，保留个人 vault 的保护 |
| 受限编译 | `crates/code/acp/src/knowledge_compile.rs`、`host_context.rs` | 编译只访问合法快照与候选输出；扩展类型化 scope，不给任意原目录通用写权限 |
| 搜索已发布 Wiki | `crates/code/core/src/workspace/retrieval/wiki.rs`、`crates/code/acp/src/wiki_search.rs` | 继续用原 WikiIndex，不复制索引；增加当前 workspace 的显式绑定，默认不混搜个人/团队内容 |
| 原模型与运行 | `apps/studio/src-tauri/src/agent_plane/runtime_client.rs`、`turns.rs`；`renderer/pages/knowledge/knowledgeCompileAgent.ts` | 使用原会话/Run，不在 Wiki UI 直接调用模型；共享维护逻辑，避免复制整个 renderer runtime |
| 阅读与导航 | `renderer/pages/knowledge/KnowledgeWorkbench.tsx`、`noteReading.ts`、`knowledgeReveal.ts`、`components/Preview/MermaidPreview.tsx` | 复用 Markdown/Mermaid/来源打开组件；首期不增加可视化图谱编辑器 |
| 工作页入口 | `renderer/pages/agents/WorkbenchHome.tsx` 的 `workbench.projects.more` / `workbench.tasks.more` | 直接替换两处分组更多目录入口，项目“更多”在少量/空列表也可见；不新增任务 Wiki 标签作为首期入口 |
| 全页覆盖 | `renderer/components/layout/AppShell.tsx`、`renderer/services/settingsSurfaceService.ts`、`renderer/components/command/CommandProvider.tsx`、`renderer/hooks/useWindowRuntime.ts` | 复用设置页覆盖/保活/命令隔离模式，保留独立 Wiki 身份，审查所有后台交互调用方 |

以上 renderer/native 简写路径均相对 `apps/studio/src/`。

## Confirmed gaps

1. `is_personal_vault` 只接受 `wiki/local`，维护扫描只收 Markdown。任意代码/资料工作区不能直接传入该 API，也不应通过复制整仓或伪造路径绕过。
2. 当前 `search_wiki` 默认结合个人笔记/团队百科，`agent/local/team` 范围不表示工作页项目或任务。工作区问答需要显式精确绑定和隔离测试。
3. 任务 cwd 与项目放置目录可能不同，历史状态也可能包含已改绑的任务。2026-10-26 用户取消了侧边栏任务拖入/拖出项目；新建任务仍可选择项目，初始化绑定不搬动已有文件。目录树包含原生元数据和会话状态；只做递归扫描会把这些错误地当知识来源。
4. 当前页面出处以源路径为主。DeepWiki 式追溯还需原文件范围与内容代次，并支持引用过期后明确展示。
5. 工作区生成内容与个人手写笔记的覆盖策略不同；共享机制不能取消个人重要更新的审阅规则。
6. 当前 `WorkbenchHome.tsx` 两个“更多”分别打开固定 420px 的项目目录/任务历史弹窗；项目“更多”仅在超出预览时出现。它们需改为用户指定的全内容区卡片目录。任务历史当前只列收件箱/置顶任务，项目内任务应从项目详情可达。

## Scope decisions

- 核心交付是生成可阅读的 Wiki、问答与来源，而非只增加一个文件搜索框。
- 代码、文档及混合工作区都可进入；页面结构由模型基于内容决定，软件运行方式仅在有证据时出现。
- 项目覆盖合法项目工作区文件，含拥有读取权的子任务文件；任务只读自身真实范围。外部/独立目录不随项目归属自动导入。
- 同范围、同权限、同准入规则及源代次复用发布结果；不同根或权限不共用。首期不建设跨项目去重缓存。
- Wiki 输出放宿主拥有的既有 workspace 状态/知识产物位置，与原文件分开；由绑定定位，不在源码根自动创建容易冲突的 `wiki/`。
- 首次生成由用户打开后发起；已启用范围的变更去抖排队，受原预算约束。失败保留可读代次，明确重试。
- 生成页经来源/代次校验后可自动替换；个人笔记和原 personal 重要更新审阅维持原行为。
- 交互按用户截图修订：两个分组“更多” → 设置式全页弹层 → 卡片目录 → 同层 Wiki 详情。目录只读元数据与已发布摘要，选择对象后才具备问答范围；关闭保留后台任务/草稿，返回保留目录状态。
- 新建卡片仅复用原创建流程；不复制外站 stars、公开仓库推荐或 URL 导入。原项目/任务管理操作保留可达。

## Risks and assumptions

- 结构化工作区适配是主要新增工作量；不能把已有个人 Wiki 的页面数当作本功能已具备的证明。
- 019 正在收敛原运行时。Core/OKF 可先做；桌面接入只消费已合格 RuntimeClient/会话边界，避免第二账本或模型客户端。
- 大工作区需显式报告部分覆盖。目录/符号和检索可减少上下文，但不承诺一次全量理解。
- 未保存编辑、远程云工作区、实时外部系统和聊天导入本期不作为源；无需 Shell 执行业务代码即可理解文本。
- 规划可以现在完成；模型效果、原生 UI、耗时与成本均需后续实际夹具和试用证据。

## Clarification record

用户已经明确当前优先级、对象及入口布局：工作页项目/任务工作区 Wiki，通过侧边栏分组“更多”进入类似设置的全页卡片目录，参考提供的 DeepWiki 截图。早先任务 Wiki 标签方案被此选择替代。采用本地文件、现有模型、显式首次生成、持久页面与增量更新作为范围；本轮不需要额外权限或偏好回答。
