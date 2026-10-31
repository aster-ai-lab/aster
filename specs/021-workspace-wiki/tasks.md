---
description: "Task list for project and task workspace Wiki"
---

# Tasks: 项目与任务工作区 Wiki

**Input**: 本目录 spec.md / plan.md / research.md / data-model.md / contracts。

**Status**: 实施及验收中。ActivityBar 知识入口移除；原知识页用于产物／看板／百科／记忆，保留紧凑页头、原多标签编辑与统一空态。原生接口上的真实模型整批发布、运行中取消、失败候选恢复、未打开 Wiki 时后台增量、项目注册表归档／恢复，以及普通任务当前百科检索已通过；离线异步向量在 512 文件夹具中已实际命中。重启一致性及当前原生图谱、引用、编辑丢弃重开、记忆保存恢复旅程已通过；全库失败项仍需收尾，不据接口测试预先勾选整体验收。最新命令、耗时与限制见 `docs/evidence/desktop/2026-10-26-workspace-wiki/OUTCOME.md`。020 保持延期，不提交。

**2026-10-26 用户收敛**：取消百科独立问答，删除入口、组件、专用路由/会话/证据通道和仅供该功能的 ACP 查询模式；任务聊天沿用 `search_wiki`，只检索当前保存绑定的工作区百科。原图谱必须显示该工作区的百科页面/链接并打开原文件标签。T008 现只包含图谱、原百科引用来源与标签编辑，不再实现独立问答。继续完成后台多工作区增量、删除/恢复、目录变化和大项目性能；任务拖入/拖出项目仍禁用。历史验收记录保留，不能据旧问答证据宣称当前功能存在。

**House rule**: 每项行为与 owning-package hermetic 测试同任务完成；共享运行/发布状态，不预标并行。

**2026-10-27 收尾补充**：原 GUI 创建的项目候选经真实 DeepSeek 原作用域 Run 修复链接后，由原任务 owner 整代发布两页，12 个当前引用和原图谱的一条边通过校验，重启后绑定与页面摘要不变。修复了目录移走期间归档、目录放回后恢复会复活旧候选的缺陷，包含 store 重开、重复恢复和原生接口验收。原 Core egress/manifest、Wiki watcher 和实际 ACP 检索统一排除嵌套会话定位文件，保留合法用户 metadata 与秘密排除报告。5000 个小文本文件的实际离线向量就绪由 11814 ms 降至 815 ms；这不代表 51 MB 原 note-owner 冷查询也已优化。最终 350 Core workspace、6 ACP／15 Core／37 native Wiki、8 实际 stdio 用例通过。完整桌面旅程仍受 `cgWindowNotFound` 阻碍，全库红项保持可见，最终门禁不预勾选。命令、真实回执与限制见 `docs/evidence/desktop/2026-10-27-wiki-finalization/OUTCOME.md`。

**当前收尾顺序（2026-10-26 新目标）**：先完成 Studio 原启动 ACL 对离线 `local_cpu` 向量能力的接入及真实 ACP 证明；再将现有项目注册表收敛到原生 agent-plane 存储，原 renderer 元数据只作一次受保护的初始化输入，不保留第二个可写项目集合。项目删除为可恢复的注册关系删除，磁盘文件不删除；原生项目状态统一控制下属任务及 Wiki 的可用性，恢复保留原 Wiki 身份，单独归档的任务不随项目恢复。随后完成后台多范围/重启、整代发布/故障恢复、大项目基准、原生图谱和编辑完整旅程及受影响仓库检查。独立百科问答和任务跨项目拖拽继续删除，不以旧测试恢复这些功能。

## M0: 范围（1 task）

- [ ] T001 [US1-US2] 在 Studio 原项目/workspace/session owner 及 `src/shared/data/api/schemas/agentWorkspaces.ts` 补精确 Wiki 绑定与只读解析；为目录复用或补齐有界名称/已存摘要查询、分页、排序及 Wiki 状态投影，不建立对象副本；核对 `workbenchWorkspace.ts`、`sessionWorkspace.ts` 的独立/项目子任务目录及 019 边界。带 owner 测试：分页外命中、原集合范围、无 workspace、项目无需普通任务、独立/子任务、合法嵌套范围、外部目录、越权/跨租户、同目录异权限、迟到结果；不猜目录名或读取父级。

## M1: 来源及复用（2 tasks）

- [ ] T002 [US1-US3] 复用 `crates/code/core/src/workspace/retrieval/`、`workspace/source_egress.rs`、`code_intelligence/service.rs` 及 Studio 原 document readers，增加有界 workspace 源快照/范围锚点/覆盖报告。测试代码、Markdown、现有可读资料、无 LSP、秘密/运行状态/依赖排除、symlink/越界、超限、坏锚点及原文件摘要不变；输出不纳入源。
- [ ] T003 [US1-US4] 在 `crates/okf/src/personal/maintenance.rs` / `personal.rs` 最小抽取 snapshot/staging/citation/publication 共用原语并增加 workspace source adapter，仍用原维护任务与存储；保留 personal 公开 API 和目录/审阅约束。带测试：任意源 root 不能伪装 personal、快照只读、原子发布/上一代可读、用户源可继续编辑、无第二账本和原 personal 回归。

## M2: 生成（2 tasks）

- [ ] T004 [US1-US3] 在原 OKF prompt/manifest/来源校验路径加入 workspace 内容轮廓，输出概览/目录、证据支持的模块/主题/流程及适用 Mermaid；保存源范围与代次。带固定代码/纯资料/混合夹具，验证有效来源、引用缺失拒绝、无证据运行命令不编造、图边推断标记和来源提示注入不能扩权；不建图谱引擎。
- [ ] T005 [US1-US3] 在 ACP `knowledge_compile.rs`、`host_context.rs`、`agent/host_methods.rs` 和 Studio `agent_plane/runtime_client.rs` 接入 workspace compile 类型化范围及原 Run；更新 host 注册/permissions/capabilities 和 parser。带 ACP stdio/原生边界测试：scope mismatch、候选写边界、未知字段、取消/预算、无模型旁路与不调用 Shell/网络；编译会话不污染普通任务列表，不恢复专用问答会话。

## M3: 阅读、检索与图谱（3 tasks）

- [ ] T006 [US3] 扩展 Core `workspace/retrieval/wiki.rs` 与 ACP `wiki_search.rs` 的原 WikiIndex 范围绑定和来源当前性，复用 hybrid 原文件检索回答补充问题。带测试：not-ready、删源/旧引用、当前 snippets、同名文件异 workspace、同目录异权限、无 personal/team 混搜及无第二索引/模型客户端。
- [ ] T007 [US1-US2] 先更新根 `DESIGN.md` 的两个分组“更多”/铺满内容区的目录弹层规则，再在 Studio `WorkbenchHome.tsx`、`components/layout/AppShell.tsx` 及其现有命令/焦点隔离调用方接一套 project/task 卡片目录和同层 Wiki 详情；替换原 420px 弹窗，保持“更多”在少量/空集合可见。元数据/已存摘要搜索、有界分页、状态/时间/进入箭头，目录不含新建卡片，原工作页创建及对象选择/管理操作可达；复用 Wiki 阅读/Mermaid 部件。带测试：原集合范围、非当前页搜索、空/无匹配、目录浏览零模型生成、键盘/窄屏/长标题/中英文、后台快捷键隔离和关闭焦点/草稿恢复，不伪造 settings 身份。
- [ ] T008 [US3] 原百科图谱只展示当前工作区页面和链接，节点、来源与产物／百科／记忆均打开同一多标签编辑器。保留冻结来源校验、源变更提示、编辑草稿及返回目录状态；点击看板卡片直接打开该任务真实产物目录。带测试：正确 scope、图谱节点／边、来源锚点、迟到结果拒绝、保存冲突及关闭恢复。不保留“问这个工作区”、专用路由或独立聊天运行时。

## M4: 持续理解（2 tasks）

- [ ] T009 [US4] 复用 Studio `workspace_watch.rs` / 原监视对账及 OKF 维护状态，将源变化映射到过期页面、去抖排队、原预算内更新和明确重试；未知依赖保守使概览过期。带测试：变更/删除/重命名、生成中变更、原子激活、无变化零生成、输出回灌、取消/失败无无限重试、personal 重要更新审阅不变。
- [ ] T010 [US2-US4] 原生项目注册表只接受一次旧集合初始化；归档／恢复和合法目录变化控制任务、Wiki、计划及当前生成资格。恢复保留原身份、独立归档任务与源文件，目录缺失时明确拒绝。带重启、目录变化、外部目录、重复进入、旧异步回执和相同目录不同权限测试；禁止任务拖入、拖出项目，不复制权限或合并别的项目内容。

## M5: 资格及文档（1 task）

- [ ] T011 [US1-US4] 跑上述 owning-package/stdio/renderer/shared 与原 personal/retrieval 受影响回归、格式、依赖/运行时权威及类型/i18n 检查；固定小代码/纯资料/混合/多 workspace 夹具独立判定，执行真实模型与原生完整旅程。记录实际源码、配对 ACP/桌面、请求/时间/用量、首次失败与未验范围至 `docs/evidence/desktop/<实施日期>-workspace-wiki/OUTCOME.md`；补接入、覆盖、更新与恢复文档，不默认提交、发布或联系他人。

## Final Verification Gate

2026-10-27 补充资格：FR-014 的目录状态改为原发布 owner 的有界只读投影，区分未生成、生成中、待审阅、失败、已有、忙碌及不可用；不扫描来源或启动模型。SC-002 的纯资料与混合独立真实模型夹具分别整代发布 4／5 页，原文件摘要不变，36／46 个引用逐条通过原生来源校验。混合页明确代码只计算、不写管理报告。当前目录状态已在原生桌面显示；完整项目生成 GUI、双语言键盘焦点旅程和全库红项仍未全部验完，保持最终门禁未勾选。具体回执见本次 OUTCOME。

- [ ] T012 [US2-US4] 按最新目标让原 ACP 工作区会话异步准备原 aster-vec 持久索引，复用原 catalog/监视/增量发布和 session close；原 BM25/hybrid 实际命中当前来源，支持冷启动回退、更新、重复进入与秘密排除。编译候选会话跳过此预热与个人资源装配；验证首个搜索前自动发布、延迟接入后的更新、关闭后拒绝及原生真实旅程，不新增索引/调度/embedding 服务。

- [ ] 项目/任务范围准确且原文件不变，秘密和原运行状态不进入 Wiki（T001–T005）
- [ ] 概览/主题/图谱与检索有合法当前出处，过期/无依据可见（T004、T006、T008）
- [ ] 两个分组“更多”共用全页卡片目录/同层阅读/原模型运行时，列表搜索分页、详情返回、关闭恢复、中英文及键盘可用（T005、T007、T008）
- [ ] 增量、无变化零生成、复用、归档／恢复、目录变化、取消/失败/重启正确（T009、T010）
- [ ] personal Wiki 保护、ACL、egress、单权威和原有依赖约束不回归（T003、T005、T011）
- [ ] focused tests、`cargo fmt --all -- --check` 与受影响 CI 完成（T011）
- [ ] 实际原生/真实模型证据与未验范围齐全，不将静态预览当产品验收（T011）

## Latest UI and source-capture evidence — 2026-10-25

- T005/T006/T008 2026-10-26 integration: workspace-bound question/source routes now use the original hidden Session/Run/chat stream, original governed hybrid/WikiIndex retrieval, a tool-free answer session and original canonical source tabs. Seven native Q&A cases and 179 original turn cases passed before later snapshot literal-quoting/range and exact host-state exclusion changes; those final native changes still require rebuilding and running owning tests. The question pane has three passing original-runtime tests; the latest reader/editor/menu regression has 78 passes and shared schemas have 21 passes. i18n, focused formatting and lint checks are recorded in `OUTCOME.md`; full type/runtime-boundary gates remain red. The latest desktop/model/GUI journey is not yet qualified and no completion box is checked.

- T005/T006/T012 further foundation: the original ACP now exposes governed, model-free search of its current session workspace through `aster/workspace/search/v1`. The original hybrid lexical channel reuses the same published aster-vec generation with epoch/revision checks and current-source verification. **7 actual stdio cases pass**, including publication before the first search, actual persistent route metadata, changed/secret sources, foreign workspace refusal and restricted-review refusal. **24 Core hybrid / 12 persistent owner / 24 native Wiki cases pass**; the original Core model-free late-attachment case now checks the actual persistent route. Source-review construction also isolates ambient memory/instructions/skills/hooks and skips personal assets; **413 ACP unit cases pass / 8 existing ignored** before the later hybrid change. The detailed source-qualified receipts and earlier failures are in `OUTCOME.md`. Native Wiki Q&A, exact admitted-source/converted-document evidence selection, whole-generation activation and full GUI qualification remain incomplete, so no final gate is checked.

- T007/T008 UI refinement: Outputs / Board / Encyclopedia / Memory reuse the original knowledge surface and canonical file editor. Open file buffers stay mounted across resource switches; dirty closes are guarded. Board cards open the bound file tree in the same Wiki, with a separate original-task action. Project work is paged by the owner; task work uses structured plans or its exact current execution. All four empty states use the same centered inbox illustration.
- Durable memory editing stays in the existing Core/ACP/native owner, with a reviewed revision, field preservation and transient-layer write refusal. It is not a new file store or model runtime.
- T005 partial: the original ACP compile guard dispatches an explicit `workspaceRevision` to the same OKF workspace owner, rechecking the live job on every tool call without changing the candidate-only ceiling. All 8 parser/guard and actual-session cases pass, including all four permission modes and late writes. The earlier build blocker was repaired by preserving the original `LegacyHistory.display` payload as display-only replay; all 4 replay cases pass. Native generation/settlement/Run/UI wiring remains outstanding.
- T002/T005 task admission: `/api/agent-sessions/{id}/wiki-inputs` resolves the saved native binding twice, combines Core source admission with OKF document projections, and returns metadata/coverage without source text, directory creation or a model request. Three native cases and 15 shared schema/classification cases pass. Discovery reuses the original ignore walker with entry/deadline/cancellation limits and without Git subprocesses or source-file symlink traversal; its new owning Core bound/root regression is still queued on the shared build lock. Project admission, generation/settlement actions and UI integration remain outstanding.
- T003 partial: workspace evidence packets now enter the original OKF begin/finish/review/cancel engine and ledger, using explicit workspace binding and snapshot identities. Original sources remain outside the write owner; frozen evidence lives only in staging and the original history. Workspace machine updates may publish without the personal replacement-review rule; uncertain changes retain review. Seven new workspace maintenance cases plus the six document cases pass, and all 74 OKF tests pass. ACP/Studio Run admission, atomic generation activation and the UI generation action remain outstanding.
- T002 partial: bounded source capture and coverage have 9 passing Core tests. Input identity v2 excludes the coverage report, so excluded generated-output creation/rewrites cannot invalidate unchanged admitted input; actual source changes still invalidate it. The shared OKF document adapter now binds admitted bytes to immutable conversion passages and source/projection generations; PDF page markers are retained when supplied by the existing extractor, other documents expose only converted-block positions. Six new document cases and all 67 OKF regressions pass, including explicit truncation, stale/forged citation refusal and zero OCR callbacks. The original Office extractor's hidden XML truncation was fixed. Host compilation/source-packet integration and native Office paragraph/cell-specific navigation remain outstanding; this adapter is not yet invoked by the workspace Wiki UI. Native binding HTTP authority test passes.
- Latest checks and source-qualified native captures are retained in `docs/evidence/desktop/2026-10-25-workspace-wiki/OUTCOME.md`. No M0–M5 checkbox or real-model gate is completed by these UI checks.

- 2026-10-26 用户取消侧边栏任务拖入/拖出项目：已删除拖拽及目录搬迁代码，初始任务目录绑定保留，已有任务迁移请求被拒绝。原生 3 项及直接运行的已有任务拒绝用例通过，前端任务/项目 12 项通过；现有文件、Wiki 和长期记忆保持原位。详见 `OUTCOME.md` 最新记录；不改变整体 Wiki 完成状态。
