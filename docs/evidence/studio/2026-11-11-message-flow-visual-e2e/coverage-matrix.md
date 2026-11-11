# 222 项渲染形态 · 真窗口可达性矩阵（2026-11-11）

逐项清单见 `../2026-11-11-message-flow-ui-inventory/INVENTORY.md`，本文件按相同编号一一对应。分类依据是「当前 checkout 下，要让它出现在 agent-desktop 驱动的原生窗口里，需要什么前提」，不是实测结果。

## 口径与四类

| 类 | 含义 | 能否在本轮截图 |
| --- | --- | --- |
| **A** | 现有会话数据 + 纯 UI 操作（点击/悬停/搜索/多选/菜单）即可出现 | 能 |
| **B** | 需要真实模型 / ACP 任务执行到特定工具或状态 | 需另起任务，且部分需凭据与真模型 |
| **C** | 需要向 renderer 注入构造好的消息 part / 状态（真窗口无 `addInitScript` 通道） | 需先扩展 `e2e-preview.mjs` 注入 fixture 并重建 renderer |
| **D** | 代码明确不可见、纯内部状态、错误兜底或无法从 UI 分辨 | 不能，只能静态代码验收 |

### A 类（逐项编号）

- **表 A 行形态**：R01 用户气泡、R02 用户 header、R03 附件外置行、R04 助手普通、R05 workbench 隐藏身份、R06 任务页 transcript、R07 编辑态、R08 多选态、R13 流式行（发消息触发）、R18 错误卡行、R19 完成过程组折叠、R20 活动过程组、R21 连续工具聚合、R24 单工具行、R27 tail 附加区、R31 送达/来源徽标。
- **表 B 内容块**：B01 text 正文、B02 任务页 prose、B03/B04 reasoning 两种、B08 通用工具 part、B09 任务页工具行、B10 data-error、B11 data-form（历史会话中已存在）、B31 连续图片组、B32 连续工具组、B34 过程组外壳、B35 占位块、B38 hover 活动预览、B39 文本内围栏/代码、B40 引用 pill/上标。
- **表 C3 工具状态**：1 折叠、2 展开、3 单次工具行、4 N 次聚合、8 失败行（历史会话）、19 耗时、23 子任务聚合、26 无数据兜底。
- **表 D 容器**：L8 回到底部、L9 导航按钮组、L10 锚点轨、L12 多选、L13 拖拽框选、L15 消息内搜索、L16 时间戳/分隔、L17 错误卡、L18 生成中占位（发消息时）。
- **表 E 覆盖层**：S3 Stop（发消息时）、S11 WorkingStatus（发消息时）、S14 token/成本详情、S15 引用面板、S16 右侧详情面板、S17 任务进度条、S21 送达徽标、S13 速率（发消息时）。
- **表 F 行内操作**：A1–A20 全部（消息菜单栏 hover 可观察，具体显隐取决于 host 是否注入对应 action）。

### B 类（逐项编号）

- **表 C1 全部 32 项**工具渲染器（T01–T32）：每一项都需要对应工具被真实调用；部分工具（Team*/Worktree*/Workflow/SendMessage 等）还需特定 agent 配置。
- **表 C2 全部 15 类**旁路工具 UI：需对应工具或审批/表单流程被触发。
- **表 C3 剩余状态**：6 进行中、7 成功、9 失败行单行布局、10–12 审批等待/优先级/被拒、13 取消、17 流式参数、21 进度环、22 子代理流程行、24 压缩锚点、25 thinking 头、27 deferred 结果。
- **表 A**：R10–R12 多模型组 4 布局、R14 awaiting、R15 待确认、R16 暂停、R17 空占位、R22/R23 任务页工具聚合、R25 文件活动行、R26 WorkingStatus 的细节 popover、R28 子代理列表、R29 会话结果/报告产物、R30 渲染错误兜底。
- **表 B**：B05 code、B06/B07 file、B12 video、B13 compact、B14 压缩锚点、B15 会话重置、B16 fork、B17 翻译、B18 retry、B36 块错误兜底、B37 子代理/报告、B41 内联产物、B42 暂停空 markdown。
- **表 D**：L3 历史分页、L4 live tail、L7 阅读冻结、L11 消息大纲、L14 全选等分页、L20 会话重挂载。
- **表 E**：S1 生成中、S5 完成、S6 中断、S7 错误、S8 压缩 5 态、S9 API 重试、S10 重试块、S12 等待确认、S18 审批浮层、S19 表单挂起、S20 AskUserQuestion、S22 会话不可用。
- **表 G 全部 13 项**产物卡片：需任务真实产出文件 / Office / 图片 / diff / 审查结论。

### C 类（逐项编号）

需构造消息数据，真窗口当前没有注入通道：R09 上下文边界分隔（需 `data-clear` part）、R30 渲染错误兜底（需注入抛错块）、B06/B07 附件 part、B11 表单 5 种子状态（closed/answered/pending/invalid/legacy）、B14 压缩锚点多态、B17 翻译块、B18 retry、B28 引用来源 part、B33 连续 video、B37 报告产物、C3 14/15/16 延迟挂载与 Suspense（视觉上不可分辨，需计时观测）、L2 初始骨架、L5 虚拟回收（视觉不可分辨）、L6 滚动恢复（需会话切换）、L19 底部 inset。

### D 类（逐项编号）

- **代码明确返回 null 的 12 种 part**：B19 `data-context-usage`、B20 `data-agent-task-event`、B21 `data-task-plan`、B22 `data-knowledge-scope`、B23 `data-review-source`、B24 `data-review-progress`、B25 `data-workbench-proof`、B26 `data-clear`（由 frame 画分隔线，本身不渲染）、B27 `data-agent-plan`、B28 `source-url`/`source-document`/`step-start`、B29 `data-citation`、B30 未知 part type。
- **纯内部 / 非视觉状态**：S2 `aria-busy`（需读 AX 属性而非截图）、C3 14/15/16 延迟挂载时机、L5 虚拟回收、`data-task-turn-state` 等 DOM 属性。
- **无对应 UI**：按日历日期分组分隔、已删除/撤回占位、`system` 角色分支（代码中不存在）。

## 实测回填（2026-11-11，窗口 `w-56826`）

`capture-scenarios.py` 共 16 个场景，`captures.json` 记录每个场景的 `action_ok`、AX refs 与截图。**只有 `action_ok=True` 才能算「该形态被实际触发」**；`False` 的截图只是基态，不得计为通过。

| 场景 | action | 实际覆盖编号 | 判定 |
| --- | --- | --- | --- |
| 01 transcript 全景 | 基线 | R06、R18、R19、R21、B02、B10、L16、S14、F02/F07/F10/F11 | 已截图 + 已分析 |
| 02 reasoning 展开 | False | — | **未验证**（截图与 01 字节相同） |
| 03/04/05/06 工具聚合展开 | False | — | **未验证**（截图为同一会话不同滚动位置） |
| 07 更多操作菜单 | False | — | **未验证**（菜单未出现在截图中） |
| 08 模型选择器 | True | C06、F04 | 已验证 |
| 09 授权模式 | True | C12、E18 | 已验证 |
| 10 Composer 更多选项 | True | C05、F11 | 已验证 |
| 11 搜索任务对话框 | True | L15、F16 | 已验证（AX refs=7，modal 隔离正确） |
| 12 轮次跳转锚点 | True | L10、R31 | 已验证 |
| 13 任务工作区面板 | True | S16、P1 | 已验证 |
| 14 工作区快捷菜单 | False | — | **未验证**（与 13 截图字节相同） |
| 15 设置页 | True | P16 | 已验证 |
| 16 新建任务空态 | True | L1、R17 | 已验证 |

**统计**：16 张截图、`action_ok=True` 8 个、`False` 7 个、基线 1 个；`restored=False` 16 个（恢复判定要求「新建任务 + 描述任务」同时出现在 AX 前 28 项，条件过严，不代表状态未回位；窗口终态已确认回到工作台，refs=39）。

**A 类实际完成度**：矩阵中 A 类约 70 项，本轮**实触发并留证 18 项编号**（上表「实际覆盖编号」去重后），其余 A 类项或因动作未生效、或因需 hover/`--headed`（本轮禁用）未执行。**B/C/D 类全部未执行**，原因见上文四类定义。完整 222 项**未覆盖**，本轮不构成完整 E2E 验收。
