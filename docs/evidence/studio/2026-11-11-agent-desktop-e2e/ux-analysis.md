# 逐截图 UI/UX 分析（agent-desktop 真窗口，2026-11-11）

分析对象：`2026-11-11-agent-desktop-e2e/` 下 14 张截图，窗口 `w-61786`，构建 stamp 见 `OUTCOME.md §1`。

**方法**：每张截图按五个维度看——视觉层级、交互反馈、动效、可访问性、一致性；结论分「符合」「待改进」「缺陷」。动效是时间维度，静态截图**不能证明**动画合规，本轮只记录能从静态画面判断的部分（是否预留空间、是否有硬切痕迹），不宣称验证过时长/缓动/reduce。可访问性结论结合同场景 AX 快照（`scenarios-run3.jsonl`）的 `role` / `states` / `available_actions`。

## 逐张结论

| 截图 | 场景 / AX 节点 | 视觉层级与一致性 | 交互反馈 | 可访问性 | 结论 |
| --- | --- | --- | --- | --- | --- |
| `01-workbench.png` | 工作台 / 59 | 三栏：窄图标条 + 项目/最近侧栏 + 中央 wordmark 与 composer。留白充足、层级清晰，圆角/描边统一 | composer 占位文案、授权模式、模型、任务范围四个入口均可见，空态无禁用态硬切 | 侧栏 `navigation`、任务行 `button`、composer `textfield` 语义完整；`status/alert=0`（首页无进行中状态，合理） | **符合** |
| `02-search.png` | 搜索对话框 / 14 | 居中模态 + 背景遮罩，焦点唯一，符合命令面板范式 | 打开即时、有清除与「关闭」、键盘 `ESC` 提示 | dialog surface 替换窗口树（14 节点），语义聚焦正确 | **符合** |
| `02b-search-query.png` | 输入「Aster」/ 17 | 匹配词黄色高亮、单条结果带相对时间与跳转图标，密度低、可读性好 | `set-value` 返回 `delivered_verified`，结果即时出现，无加载闪烁 | **待改进**：结果区没有「N 个结果」的 `role=status` 播报，`status_nodes=0`；屏幕阅读器听不到结果数量变化 | **待改进** |
| `03-automation.png` | 自动化页 / 58 | 左筛选（全部状态/活跃/已暂停/已完成/已错过）+ 右空态，图标 + 标题 + 说明三段式，层级好 | 空态明确「暂无定时任务 / 创建一个任务，让助手按计划自动运行。」，新建按钮在右上角 | 筛选为可聚焦控件，空态为普通文本（无 `role=status`），可接受 | **符合** |
| `04-automation-feedback.png` | 同上（无任务）/ 58 | 与 `03` 一致 | 无「测试运行」按钮可点 → `NO_TASK_TO_RUN`，toast 未被触发；**不宣称验证了新增 toast** | 空态无状态播报 | **符合（未覆盖反馈路径）** |
| `05-skills.png` | 技能页 / 114 | 两列卡片网格：图标 + 标题 + 两行截断描述 + 安装按钮；分类 chips、搜索、市场/已安装、新建，密度高但整齐 | 安装按钮为常显次级按钮，未见 pending 态（本轮未点） | 卡片/按钮语义齐全 | **待改进**：AX 有「上一页/下一页」但截图视口内看不到分页控件，需滚动才能发现；描述统一截断两行，长名称信息损失 |
| `06-connectors.png` | 连接器页 / 103 | 与技能页完全同构，设计系统一致性好；品牌彩色图标提升可扫读性 | 安装按钮状态一致 | 同技能页 | **缺陷**：至少 2 张卡片把「该条目没有可识别的本地接入配置」这类**内部配置诊断**当作用户可见描述直接铺出（XSCT 模型选型 MCP、Chrome 开发者工具 MCP），与 I4「内部细节不外露」冲突 |
| `08-settings.png` | 设置 / 138 | 分组清晰（偏好/智能/系统），开关右对齐、说明文字次级灰色，表单节奏稳定 | 开关即时态、无遮挡 | 设置项多但语义树完整；截图里「工作区文件夹 /Users/roylin/.aster」是**有意展示的用户可配置项**，不是内部泄漏；AX 扫描 `absolute_path_nodes=0` 说明静态文本未进入 `name/value`，扫描存在盲点 | **符合** |
| `09-model-picker.png` | 模型下拉 / 91 | 浮层带标题、搜索、服务商分组 chips、模型列表；当前项浅蓝底，选中态明确 | 打开即时、无遮挡主操作 | 列表项语义可读 | **待改进**：面板中**未见推理强度滑杆/档位**（上一批修过档位丢失），需确认是否要展开某个模型详情才出现；本截图不能证明功能缺失，仅记录未观察到 |
| `10-more-options.png` | composer 更多选项 / 65 | 小浮层 4 项，开关右对齐、状态清楚，与整体圆角/描边一致 | 开关即时、有显式「关闭」项 | 开关 role/状态可读 | **符合** |
| `11-permission-mode.png` | 授权模式 / 64 | 5 个选项各有图标 + 一行说明；「完全访问」用红色文字 + 「高风险」标签，危险等级视觉区分到位；选中项浅蓝底 + ✓ | 选中态、说明、「下一轮生效」提示齐全 | 选项语义完整 | **符合**（业界最佳实践正例） |
| `12-restored-workbench.png` | 还原工作台 / 58 | 与 `01` 一致，restore 成功，无残留浮层 | 场景间状态隔离有效 | 节点数回到 58 | **符合** |
| `14-composer-value.png` | composer 输入尝试 / 58 | 仍是空 composer 首页 | `set-value` 返回 `ACTION_FAILED`，输入框仍为空 → **headless 语义写入对 Tauri 富文本 composer 不可用**（与既有记录一致）；未点击发送，未产生真实任务 | **待改进**：空 composer 时「发送」在画面上不可见，但 AX 树仍暴露 `button 发送`；若未正确 `disabled`/`aria-hidden`，屏幕阅读器会读到不可见的提交按钮 | **工具限制 + 待改进** |
| `13-context-menu.png` | 右键尝试 / — | 4 个候选 `right-click` 均 `ACTION_FAILED`；画面出现的是**任务详情 HoverCard**（不是系统菜单），说明 headless `AXShowMenu` 在此 WebArea 不生效 | 无菜单反馈 | `snapshot --surface menu` 返回 0 节点（应用内浮层不走 menu surface） | **误判更正（见下方 §更正）**：初稿把该浮层判为「内部记账平铺」缺陷；复核 `WorkbenchSessionCard.tsx` 与 `WorkbenchSessionCard.test.tsx` 后确认它是**有意的产品功能**（`role="region" name="任务统计"`、专用统计 API、每个指标的 tooltip 口径说明、完整单测），不属于错误路径的内部诊断外露。保留工具限制记录，**撤回缺陷判定**，未改动该功能 |

## 跨页面观察

1. **`status_nodes=0` / `alert_nodes=0` / `loading_label_seen=false` 出现在全部 14 个场景**。这不证明上一批的 `GlobalSearchState`、`AsterPageLoading` 改动无效——本轮没有触发加载态、错误态或慢路由；但必须承认：**新增的可见加载文字与 aria-live 状态只有单测证据，没有真窗口行为实证**。要拿到实证需要故障注入或慢速路由，本轮未做。
2. **内部诊断标记扫描为 0，但扫描存在盲点**：它只覆盖 AX 节点的 `name` / `description` / `value`。`08` 截图里肉眼可见 `/Users/roylin/.aster` 却记为 0，`06` 的「没有可识别的本地接入配置」是可见描述文本但不属于那 4 个技术标记词。所以「0 标记」只能说明**没有 `completion gate` / `effect_digest` / `reasoning effort default` / `workspace mutation` 这几类字符串**，不能推论「无内部细节外露」——上表保留的那处缺陷正是扫描规则外的。
3. **正面**：技能页与连接器页共用同一套卡片/分类/分页结构，授权模式的危险等级表达、搜索的即时结果 + 高亮 + ESC 提示、设置页的开关与分组，都符合业界常见规范；场景切换后 `12` 能干净还原，无残留浮层或状态串台。
4. **未覆盖**：错误卡、工具轨迹、流式推理、产物加载、键盘焦点环（需物理 `press`，本轮无 `--headed` 授权）、`prefers-reduced-motion` 开启态、真模型驱动的 222 项消息流形态。静态截图不评价动效时长与缓动。

## 更正（后续复核追加）

初稿把 `13-context-menu.png` 的任务详情浮层判为「I4 内部记账平铺」缺陷，属**误判**。复核依据：

- `src/renderer/pages/agents/WorkbenchSessionCard.tsx` 是独立组件，渲染 `role="region"`、可访问名「任务统计」，数据来自专用 API `/agent-sessions/:sessionId/statistics`。
- `src/renderer/pages/agents/__tests__/WorkbenchSessionCard.test.tsx:88-130` 明确断言 12 个指标标签、图标、数值与记忆分层全部存在；每个指标还有 `stat_hint.*` tooltip 解释统计口径，`workbench.session.card.partial_stats` 解释统计局限。
- 它是**用户主动 hover 打开的用量面板**，与用户此前反馈的「错误卡被动暴露 `completion gate` / `effect_digest` / UUID / 工具轨迹」性质不同：后者是失败路径的内部诊断，前者是有标注、有口径说明的显式功能。

因此**未修改该功能**，也不把它计入缺陷。本轮保留的有效新发现只有 `06` 连接器卡片的技术化不可用文案（已改为用户向文案）。`right-click` 在该 WebArea 的 `ACTION_FAILED` 仍是工具限制，不计为应用缺陷。
