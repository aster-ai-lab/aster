---
description: "Agent 执行理解与证据回放实施任务"
---

# Tasks: Agent 执行理解与证据回放

**Input**: [spec.md](spec.md)、[plan.md](plan.md)、[data-model.md](data-model.md)、[contracts/analysis-run.md](contracts/analysis-run.md)、[contracts/analysis-motion.md](contracts/analysis-motion.md)

**Prerequisites**: plan.md 及 Constitution Check、spec.md；当前只做任务分解，不表示任何任务已实施或通过。

**House rule**: 每个行为变更任务都在同一任务中携带 owning-package hermetic test。没有对应测试不得标记完成。优先测试真实实现，不复制业务逻辑到测试，不用 snapshot 代替合同断言。

**Scope guard**: 放射资源图是 P1 必做；交互式 AI 问答、聊天入口、Smart Context 和两轮证据选择均不在范围内。M4 仅允许单次 `AnalysisAnnotation` 生成。Arc 组件行为与微动效按 `analysis-motion` 合同全面落实，复用 house 组件，不引入第二设计系统。

## Format and dependencies

格式：`[ID] [P?] [Story] 实施内容 + 同任务测试 + 依赖`。

- **[P]** 仅表示不同文件、无共享状态且前置依赖已满足，可以并行。
- **US1** 执行回放；**US2** 原始证据与脱敏；**US3** 状态／决策；**US4** 放射资源图。
- Studio 相对路径均以 `apps/studio/` 为根；Evidence 以 Aster 仓库根为根。

| Phase | Tasks | Count | Gate |
| --- | --- | --- | --- |
| M0 黄金夹具 | T001–T004 | 4 | 完整、缺失、隐私和 20,000 事件夹具可用 |
| M1 事实与证据 | T005–T012 | 8 | `AnalysisRun`、deferred、Trace、AgentPlay 导出合同通过 |
| M2 回放与放射图 | T013–T022 | 10 | US1／US2／US4 和 SC-005 通过 |
| M3 状态与 Story | T023–T027 | 5 | US3、状态图和完整导出通过 |
| M4 单次语义注释 | T028–T031 | 4 | 无聊天；注释校验、隔离和取消通过 |
| Arc 微动效整合 | T032–T035 | 4 | FR-016、SC-006 和 9 类动效场景通过 |
| M5 资格与证据 | T036–T041 | 6 | SC-001–SC-006 有实际证据 |
| **Total** | **T001–T041** | **41** | 无当前阻塞；必须按依赖执行 |

## Phase 1: M0 黄金夹具与覆盖基线

- [x] T001 [US1/US3/US4] 在 `src/renderer/pages/knowledge/__tests__/fixtures/workspaceAnalysisRun.ts` 构造两个用户回合、主／子 Agent、Read／Edit、失败 Verify、确定性重试、模型变化、压缩、产物和 deferred 输出；在 `__tests__/workspaceAnalysisFixtures.test.ts` 断言原 `CherryUIMessage` 类型、源 ID、角色、父子关系和事件数量，禁止为填满模型修改原始类型。
- [x] T002 [P] [US2] 在 `__tests__/fixtures/workspaceAnalysisPrivacy.ts` 加入 prompt、response、reasoning、tool input、tool output、file content、command args、subagent prompt 共 8 个唯一秘密标记及敏感路径；在 `__tests__/workspaceAnalysisPrivacyFixture.test.ts` 断言 8 个标记均真实存在于输入，避免后续“零泄漏”测试因空输入而假通过。依赖无，可与 T001 并行。
- [x] T003 [P] [US1] 在 `components/chat/trace/__tests__/TracePage.test.tsx` 和 Trace fixture 中记录 `SpanEntity` 的模型／工具字段、cursor reset／完成语义和无重叠轮询基线；在 `research.md` 追加实测覆盖结论，不能证明 per-request 的字段明确标为消息级粒度。不得仅复制现有 UI 树快照。
- [x] T004 [US1/US2] 在 `__tests__/fixtures/workspaceAnalysisLong.ts` 确定性生成 20,000 个有稳定源身份的事件，并加入旧消息无 runtimeTiming、无 Token、无快照、永久 deferred miss、历史分页失败等缺失变体；在 `__tests__/workspaceAnalysisFixtures.test.ts` 断言总数、顺序和缺失类型，依赖 T001。

**M0 Gate**：四类夹具可重复生成；隐私测试输入确有 8 个秘密；长会话确有 20,000 事件；所有未知能力均有显式缺失表示。

## Phase 2: M1 统一事实、证据与脱敏导出

- [x] T005 [US1/US2] 新增 `pages/knowledge/workspaceAnalysisModel.ts`，定义 `AnalysisRun`、封闭事件／资源／状态集合、discriminated `EvidenceRef`、覆盖和错误码 Zod Schema；新增 `__tests__/workspaceAnalysisModel.test.ts` 覆盖未知枚举、重复 ID、非法 partIndex、跨 session ref、非法 path mode 和命名拒绝。依赖 T001–T004。
- [x] T006 [US1] 扩展 `pages/knowledge/workspaceAnalysisProjection.ts` 生成稳定事件 ID、源顺序、Agent／Task、真实指标和去重事件；扩展 `__tests__/workspaceAnalysisProjection.test.ts`，保留现有 3 个测试并增加分页 prepend、同事实多引用、未知 Token／耗时、模型变化和重试链接断言。依赖 T005。
- [x] T007 [P] [US1] 从 `components/chat/trace/TracePage.tsx` 提取 `useTraceData.ts` 并接回原页面，保持现有轮询间隔、cursor、取消和无重叠 IPC；新增 `components/chat/trace/__tests__/useTraceData.test.ts` 覆盖 reset、完成、stale trace、取消和连续失败，原 TracePage 行为测试必须保持。依赖 T003。
- [x] T008 [US1] 在 `WorkspaceAnalysisPage.tsx` 读取所选 session 的可选 `traceId`，把 `useTraceData` 原 spans 合并进 `AnalysisRun`，保留无 Trace 的消息级降级；在 `__tests__/WorkspaceAnalysisPage.test.tsx` 与投影测试中验证时间相同顺序、span 去重、无 traceId 和 session 切换。依赖 T006、T007。
- [x] T009 [P] [US2] 将 `AgentRightPane.tsx` 内 `findDeferredToolResult` 提取到 `renderer/utils/message/find.ts`，新增 `pages/knowledge/workspaceAnalysisEvidence.ts` 复用 `useToolResult`／`ai.tool.get_result`；在 `__tests__/workspaceAnalysisEvidence.test.ts` 和 AgentRightPane 既有测试中覆盖命中、永久 miss、不跨 session、out_of_scope 和不得把 miss 显示为空输出。依赖 T005、T006。
- [x] T010 [US1/US2] 在 `workspaceAnalysisProjection.ts` 与 `WorkspaceAnalysisPage.tsx` 加入 coverage、sourceRevision 和 stale-response 隔离；在对应 sibling 测试中覆盖加载中、分页失败、局部完整表述、源修订变化和旧消息／Trace／证据响应迟到。依赖 T008、T009。
- [x] T011 [P] [US2] 新增 `pages/knowledge/workspaceAnalysisAgentPlay.ts` 生成脱敏 `agentplay/v1`，无法机械确认的命令 verb 省略；新增 `__tests__/workspaceAnalysisAgentPlay.test.ts` 和固定上游 Schema fixture，断言 8 个秘密零出现、引用可解析、相对时间、path mode 与未知指标省略。仅在 `package.json`／`pnpm-lock.yaml` 增加 test-only `ajv`／`ajv-formats` 并运行 2020-12 Schema 校验，生产继续 Zod。依赖 T002、T006。
- [x] T012 [US2] 将 `pages/agents/planExport.ts` 的通用保存能力提取到 `renderer/utils/saveFile.ts` 并接回原调用，给分析页接入脱敏导出；在 `__tests__/saveFile.test.ts`、`workspaceAnalysisAgentPlay.test.ts` 和 planExport 既有测试中覆盖 desktop save、browser fallback、取消、Schema 失败不得报告保存成功。依赖 T010、T011。

**M1 Gate**：当前已有分析页行为不回归；稳定 ID、deferred miss、局部覆盖和脱敏导出均有正反例；不展示空的新地图或 Story 入口。

## Phase 3: M2 执行回放与必做放射资源图

- [x] T013 [US1] 新增 `pages/knowledge/workspaceAnalysisPlayback.ts` 管理唯一 selectedEventId、cursorSeq、filter、lens、playback、speed 和会话重置；新增 `__tests__/workspaceAnalysisPlayback.test.ts` 覆盖本地步进、过滤后边界、手动拖动退出跟随、100 步无请求和 stale session。依赖 T010。
- [x] T014 [US1] 在 `WorkspaceAnalysisPage.tsx` 接入覆盖摘要、全会话／当前镜头指标及任务／模型／子 Agent 控件，复用 `SearchSelect` 和分段控件；在 `__tests__/WorkspaceAnalysisPage.test.tsx` 验证镜头指标范围、无 native select、切换任务清空镜头和选择。依赖 T013。
- [x] T015 [P] [US1] 新增 `pages/knowledge/WorkspaceAnalysisTimeline.tsx` 和 sibling 测试，实现 Tasks／Events／Tokens／Context 四轨、步骤／真实时间轴、异常跳转和虚拟化事件列表；断言缺失测量为空轨道而非零值、20,000 事件不生成常驻全量 DOM。依赖 T013、T014。
- [x] T016 [P] [US1] 新增 `pages/knowledge/WorkspaceAnalysisPlaybackControls.tsx` 和 sibling 测试，实现 1×／4×／16×、播放／暂停、前后步进、Space、方向键、Shift 步进及“回到实时”；验证输入框／编辑器不拦截键盘、末尾停止和 reduced-motion。依赖 T013、T014。
- [x] T017 [P] [US4] 新增 `pages/knowledge/workspaceAnalysisResourceGraph.ts` 和 `__tests__/workspaceAnalysisResourceGraph.test.ts`，从真实事件生成中心任务、七类资源及 links；文件、命令 verb、MCP、模型、产物、子 Agent 和错误按精确身份去重并保留全部 eventIds，缺失分类不建占位节点。使用 T001 夹具达到至少 30 个真实节点，断言 label 来源和无重复计数。依赖 T005、T006。
- [x] T018 [US4] 新增 `pages/knowledge/workspaceAnalysisRadialLayout.ts` 和 sibling 测试，按固定类别顺序和稳定节点 ID 计算确定性径向坐标：中心 `(0,0)`，非空分类均分角度，外圈在所属分类扇区内排序；`innerRadius = max(120, min(width,height) × 0.18)`，`outerRadius = max(innerRadius + 160, min(width,height) × 0.38)`。`layoutRevision` 绑定 run 与视口，测试同 revision 坐标完全一致、resize 才重算、无随机 force 布局。依赖 T017。
- [x] T019 [US4] 新增 `pages/knowledge/WorkspaceAnalysisMap.tsx` 和 `__tests__/WorkspaceAnalysisMap.test.tsx`，用现有 `@xyflow/react` 渲染径向节点／links，支持拖动、缩放、空白取消和当前路径高亮；选择外圈节点时，若含当前 event 则保持，否则按 seq 选择其首个真实 event，并同步 T013 游标。依赖 T018。
- [x] T020 [US4] 在同一 `WorkspaceAnalysisMap.tsx` 增加筛选／镜头淡化、当前路径保留和可访问资源列表；扩展 `WorkspaceAnalysisMap.test.tsx` 断言筛选前后节点数量与全部坐标不变、无匹配时显示明确空镜头而非重排、键盘列表可选择相同节点。依赖 T019。
- [x] T021 [P] [US1/US2] 新增 `pages/knowledge/WorkspaceAnalysisInspector.tsx` 和 sibling 测试，展示当前结论、事实属性、证据和指标，按需解析 deferred 原文；验证播放不自动取原文、打开证据最多一次读取、miss 非空输出错误、原文件／子 Agent 跳转使用现有 owner。依赖 T009、T013。
- [x] T022 [US1/US2/US4] 在 `WorkspaceAnalysisPage.tsx` 集成 timeline、放射图、检查器和播放控件，并更新 `src/renderer/i18n/locales/`；在 `__tests__/WorkspaceAnalysisPage.test.tsx` 验证 Map／时间线／检查器同一选择、七类资源至少 30 节点、筛选坐标稳定、20,000 事件播放 100 步 0 历史／模型请求、stale session 和中英文。依赖 T014–T016、T020、T021。

**M2 Gate**：US1／US2／US4 全部通过；SC-002、SC-005 有自动化断言；放射图不是静态截图或装饰性占位，资源选择可直达真实证据。

## Phase 4: M3 状态图、Story 与完整导出

- [x] T023 [US3] 新增 `pages/knowledge/workspaceAnalysisStateGraph.ts` 和 sibling 测试，从真实 Diff／哈希／产物生成 states、phases、actions、transitions、links；覆盖有效迁移、连续编辑合并、读取／测试不迁移、失败／no-op 不迁移、回滚新状态、无快照降级和非 DAG 拒绝。依赖 T022。
- [x] T024 [US2/US3] 新增 `pages/knowledge/workspaceAnalysisTrajectory.ts` 和 sibling 测试，生成 `coding-agent-state-trajectory` v1；验证 ID／引用、原始语言、未知证据、缺原文标记和非法状态拒绝，未确认原文不得形成“完整证据”文档。依赖 T023。
- [x] T025 [P] [US3] 新增 `pages/knowledge/WorkspaceAnalysisStory.tsx` 和 sibling 测试，实现 Summary／Outcome、状态→阶段→动作→原文四级展开和“只看决策”；验证折叠不删除事件、无注释的确定性 Story、引用定位同一事件。依赖 T023。
- [x] T026 [P] [US3] 新增 `pages/knowledge/WorkspaceAnalysisStateMap.tsx` 和 sibling 测试，用 xyflow 实现版本树／执行顺序、side branch、rollback 和当前路径；验证无状态证据时不渲染伪造 DAG，键盘列表提供等价导航。依赖 T023。
- [x] T027 [US2/US3] 在 `WorkspaceAnalysisPage.tsx` 集成 Story／StateMap、共享选择和完整导出确认，并补齐 locales；在 `__tests__/WorkspaceAnalysisPage.test.tsx` 验证决策→执行下钻、原文确认前不保存、取消不报告成功、源变化清空旧注释。依赖 T024–T026。

**M3 Gate**：US3 通过；没有证据的历史只显示动作；完整导出和脱敏导出隐私语义不可互换。

## Phase 5: M4 单次语义注释（不做 AI 问答）

- [x] T028 [P] [US3] 新增 `src/shared/types/agentSessionAnalysis.ts` 的封闭内部目的标识，并在 `AgentPage.tsx`、`sessionWorkspace.ts`、`WorkbenchProjectPicker.tsx`、`WorkbenchProjectTray.tsx`、`WorkspaceWikiCatalog.tsx`、`WorkspaceBoard.tsx`、`WorkspaceAnalysisPage.tsx` 的既有内部会话过滤点排除；sibling 测试证明只按 session description 过滤，不按共享 workspace path 隐藏原项目。依赖 T027。
- [x] T029 [US3] 新增 `pages/knowledge/sessionAnalysisAgent.ts` 和 `WorkspaceAnalysisTurn.tsx`，复用独立 session 创建、精确 sender 与 ACP `sourceReview` 无工具路径，一次性提交有界节点／原文；在 sibling 测试中以固定模型响应验证无工具、超预算不发送、取消／切换丢弃迟到结果、原会话消息数不变。依赖 T028。
- [x] T030 [P] [US3] 新增 `pages/knowledge/workspaceAnalysisAnnotation.ts` 和 sibling 测试，只接受 `AnalysisAnnotation` 候选并校验 target、证据范围、sourceRevision、逐字 quote 和 `model_inferred`；覆盖未知 ID、越界引用、非逐字 quote、超预算、陈旧修订和部分发布拒绝。依赖 T027。
- [x] T031 [US3] 在 `WorkspaceAnalysisPage.tsx`、`WorkspaceAnalysisStory.tsx` 和 `WorkspaceAnalysisMap.tsx` 接入显式“生成语义注释”及证据回跳，不添加聊天输入；组件测试验证无模型时 M1–M3 全部可用、注释与观察事实视觉分离、引用可定位放射资源节点、迟到结果不污染新任务。依赖 T029、T030。

**M4 Gate**：只生成单次注释；没有聊天、追问或 Smart Context；原会话不被写入，内部分析助手不可见，任何非法注释均不发布。

## Phase 6: Arc 微动效与组件整合

- [x] T032 [US1/US4] 新增 `pages/knowledge/useWorkspaceAnalysisClock.ts` 和 sibling 测试，按 `agent-timeline`／`line-replay` 行为用 Motion／CSS variable 驱动共享 playhead：拖动 1:1、步进／跳转从当前位置重定向、无每帧 React render；验证播放中隐藏／离屏／暂停／结束／会话切换均停钟，手动播放与挂载后 reduced-motion 变化立即可用。依赖 T013、T018、T022。
- [x] T033 [US1/US2] 在 `WorkspaceAnalysisPage.tsx`、`WorkspaceAnalysisPlaybackControls.tsx`、`WorkspaceAnalysisInspector.tsx` 和 `WorkspaceAnalysisTimeline.tsx` 落实 house `SegmentedControl`、Button／CopyButton、EmptyState、rowMotion 与 `tool-call`／`agent-status`／`action-button` 行为；组件测试覆盖 controlled 选中底板、稳定目标、真实活动状态、首屏有界进入、筛选不重播、输出按需挂载、实际保存 Promise 与取消／失败不提前成功。依赖 T032。
- [x] T034 [US1/US4] 在 `WorkspaceAnalysisMap.tsx` 和 `WorkspaceAnalysisTimeline.tsx` 加入 Arc 相机与 `context-window` 行为：240ms 无 overshoot 镜头重定向、拖动接管、无自动旋转、筛选不排图，真实 compaction 单次 ≤360ms sweep；布局／组件测试验证连续输入、节点坐标、pre／post 文本、未知 Token 和 reduced-motion 即时终态，明确不使用 Sunburst。依赖 T032、T033。
- [x] T035 [US1/US2/US3/US4] 按 `contracts/analysis-motion.md` 在 Story、指标、加载、产物和导出完成 9 类验收矩阵；覆盖连续 5 次模式切换、打断／反转、真实 check 一次、数字固定槽位、权威修正直接切换、挂载后 reduced-motion、settled 后 0 活动循环和无每帧 React render。不得新增 motion token、第二组件系统或装饰性 autoplay。依赖 T033、T034。

**Arc Gate**：FR-016 与 SC-006 全部有行为断言；动效只解释真实事件，不修改业务事实，不妨碍键盘、停止、恢复或导出取消。

## Phase 7: M5 资格、性能与证据

- [x] T036 [US1/US2/US3/US4] 运行 plan.md 所列精确 renderer／shared 测试、受影响的 AgentRightPane／TracePage／planExport／内部过滤回归、Arc motion 组件测试、`pnpm lint`、`pnpm deps:check`、renderer build 和适用的 `cargo fmt --all -- --check`；记录实际命令与计数，不为匹配计划补空测试，不把真实模型调用作为 hermetic 条件。依赖 T001–T035。
- [x] T037 [US1/US2/US4] 在隔离 Aster 实例完成“失败测试→放射图错误节点→重试→deferred 原文”原生旅程；断言七类资源、至少 30 节点、当前路径、筛选坐标稳定、选择同步、真实相机打断和原证据身份，保留行为断言与截图并清理 owned 进程。用户后续确认以无界面 Chromium 的相机结果替代原生鼠标验收；原生指针未执行，不作为通过项。依赖 T036。
- [x] T038 [US3] 在隔离 Aster 实例完成“状态→阶段→决策→原始记录→执行事件”原生旅程，另验证无快照降级、完整导出确认、真实动作反馈和内部分析会话不污染任务列表；保留实际输出与未验范围。依赖 T036。
- [x] T039 [US1/US2/US3/US4] 验证 20,000 事件、播放 100 步 0 请求、分页失败、deferred miss、取消／重启、1440／1024／768／320 宽度、键盘、焦点、挂载后 reduced-motion、hidden／settled 停钟和中英文；分别记录功能、布局与性能证据，不用截图替代行为断言。依赖 T037、T038。
- [x] T040 [US1/US2/US3/US4] 写 `docs/evidence/desktop/<date>-agent-execution-understanding/OUTCOME.md`，包含 Result、What landed、逐 FR／SC 证据、命令与计数、放射图资源／坐标断言、Arc 触发器／owner／打断／reduced-motion 复核、隐私导出和 `Explicitly not claimed`；重新打开实际导出文件比对源数据。依赖 T036–T039。
- [x] T041 按用户要求分批提交，逐文件 stage 并检查 `git diff --cached --name-only`；Studio 源码与根仓库子模块指针分别提交，其他会话的进行中修改保持未提交。依赖 T040。

## Requirement and story coverage

| Scope | Producing tasks |
| --- | --- |
| US1 回放诊断 | T001、T003–T010、T013–T016、T021–T022、T032–T037、T039 |
| US2 证据与脱敏 | T002、T005–T012、T021–T022、T024、T027、T033、T035–T036、T040 |
| US3 状态／决策／单次注释 | T023–T031、T035–T036、T038–T040 |
| US4 放射资源图 | T001、T017–T020、T022、T031–T032、T034–T037、T039–T040 |
| FR-001–FR-004 | T005–T010、T021 |
| FR-005–FR-006 | T013–T016、T019–T022 |
| FR-007–FR-008 | T023、T030–T031 |
| FR-009 | T011–T012 |
| FR-010 | T024、T027 |
| FR-011 | T017–T020、T022、T034、T037 |
| FR-012 | T028–T029 |
| FR-013 | T005、T024、T030 |
| FR-014 | T015–T016、T019–T022、T025–T027、T032–T035、T039 |
| FR-015 | T011–T012、T028–T029、T036 |
| FR-016 | T032–T035、T037–T040 |
| SC-001 | T005–T031、T036 |
| SC-002 | T004、T015、T022、T032、T039 |
| SC-003 | T002、T011–T012、T040 |
| SC-004 | T037–T039 |
| SC-005 | T017–T020、T022、T034、T037、T040 |
| SC-006 | T032–T035、T037–T040 |

## Final Verification Gate

- [x] 所有适用行为任务的 owning-package focused test 通过，现有 `workspaceAnalysisProjection` 3 个测试保持通过
- [x] 20,000 事件、100 步 0 请求、8 个秘密零泄漏、7 类资源至少 30 节点和筛选坐标稳定均有实际断言
- [x] Arc 9 类动效场景、连续 5 次切换、挂载后 reduced-motion、hidden／settled 停钟和无每帧 React render 均有实际断言
- [x] `cargo fmt --all -- --check` 对 Rust edits 保持 clean；纯 TypeScript／docs 改动不虚构 Rust 行为验收
- [x] No new crates.io `aster-*` dependency、第二 workspace／lockfile、Python 服务、iframe Viewer、Arc 批量安装或第二组件系统
- [x] No native `<select>` added on a product surface；所有选择器均为 Aster 绘制式控件
- [x] Product configuration, if added, is ACL；当前没有新增配置
- [x] test-only Ajv 未进入生产 bundle，`pnpm deps:check` 与 renderer build 通过
- [x] 没有交互式 AI 问答、聊天输入、Smart Context、原会话写入、共享 workspace 误过滤、Sunburst 资源占比或装饰性 autoplay
- [x] `OUTCOME.md` 重开核对，所有未执行项写入 `Explicitly not claimed`
