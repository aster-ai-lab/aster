# 真实模型驱动的消息流 E2E（2026-11-11）

## 1. 授权与前置

用户明确授权两项：**允许 `--headed` 物理输入**（前置 Aster 窗口、抢焦点、真实键盘事件）；**「可达的逐个跑并标注，不假绿」**作为「触发所有机制」的口径。

| 项 | 结果 |
| --- | --- |
| 旧实例 | 后台任务 `mumh4P`（窗口 `w-56826`）已停止；旧窗口全部 `visible=false` / `WINDOW_NOT_FOUND` |
| 重建 | `node apps/studio/scripts/e2e-fresh-build.mjs --build` → EXIT=0，1m39s（`ready built in 1m 8.5s`） |
| 启动 | `STUDIO_RENDERER_DEV=0 STUDIO_RENDERER_DIST=.studio/dist/e2e-latest ASTER_TENANT_SLUG=aster just studio aster` |
| 窗口 | `w-57385`，pid 79380，1440×920，`visible=true`、`accessible=true` |
| 前台影响 | 记录 `foreground_before=iTerm` → `foreground_after=iTerm`；物理输入只在 Aster 窗口内进行，未持续占用焦点 |
| 模型配置 | 本机 `.aster/config.acl` 配有多个真实 provider（deepseek / glm / kimi / gateway）；Studio 模型选择器可见 19 个模型（标签「全部 19」） |

## 2. 决定性实验：真模型链路被产品侧配置阻断

| 步骤 | 实测结果 |
| --- | --- |
| headless 语义输入 | `set-value` / `type` 对 Tauri 富文本 composer 均返回 `ACTION_FAILED`；`press --app --window-id` 返回 `STALE_REF / exact_window_focus_unconfirmed`（与 2026-11-09 历史证据一致） |
| `--headed` 输入 | `type --headed` 第一次报 `ACTION_FAILED` 但 value 实际已写入，第二次 `ok=true`；`click 发送 --headed` 成功。**真实任务确实被发送** |
| 任务 1（deepseek-flash） | 已处理 0.1 秒 → 错误卡：`The current ACP model openai/deepseek-flash does not support reasoning effort default; nothing was sent.` |
| 任务 2（deepseek-flash，重发） | 同一错误，0.1 秒 |
| 切换模型 | 从 `deepseek-flash` 切到 `K3.1 Pro`（模型项是 `statictext`，需完整 AX 树而非 `-i` 才能取到 ref） |
| 任务 3（k3.1-pro） | 同一错误，仅模型名变化：`openai/k3.1-pro does not support reasoning effort default; nothing was sent.` |

**结论：请求从未到达模型（`nothing was sent`），因此没有任何真实模型回复、思考、工具执行或产物生成。**

### 根因（代码确认）

`apps/studio/src-tauri/src/agent_plane/reasoning_effort.rs:96-102`：把 `effort.as_str()` 拿去 ACP 模型目录的 `meta.reasoningEfforts` 词表里查，不含就 `Err(...)`；`apply_turn_settings()`（`:113-131`）随即返回错误，`turns.rs` 在发 prompt 前中止整轮。

`default` 是「用户未选择推理强度」的**哨兵值**，不该作为 wire 值送去校验。UI 侧恰好对这两个模型都不显示推理强度滑杆（见 `2026-11-10-reasoning-effort-fallback` 的判定链），用户无从改它，所以不是 UI 能绕过的配置问题。

`turns.rs:7220-7255` 的 `unsupported_current_model_effort_is_refused_before_acp_prompt` 测试断言的是「请求 `low` 而模型只支持 `high` 时拒绝」——那是合理的 fail-closed 契约；本轮场景是**哨兵 `default` 被当真值传下**，属于不同层次的缺陷，不能靠删测试解决。

修复方向（本轮未实施，需授权改 Rust native 并重建）：在 `turns.rs` / `workbench_turns.rs` 构造 `Option<ReasoningEffort>` 时把 `Default` 归一为 `None`，或在 `apply_turn_settings` 入口跳过 `default` 的校验与 `session/set_model`。

## 3. 实际触发并留证的渲染形态

虽然模型请求被拒，三轮真实 turn 仍触发了一批**错误/恢复路径**的 UI，共 10 张截图（`screens/` 与根目录 `live-*.png`、`model-list.png`）：

| 编号 | 形态 | 证据 |
| --- | --- | --- |
| R01 | 用户消息气泡（右对齐、含完整输入文字） | `T1-00-composer.png`、`T1-final.png` |
| R06 | 任务页 transcript 流水行 | 全部截图 |
| R18 / B10 | 任务页错误卡 `TaskErrorNote`（红色左边框 + `CircleAlert` + 「发生未知错误」+ 英文 detail） | `T1-final.png`、`live-final.png`、`T1-01-t005s.png` |
| R19 | 完成过程组折叠、工具聚合组（历史会话） | `T1-01-t005s.png` 顶部 |
| S1 / S11 | 「正在处理 5.0s」WorkingStatus | `T1-01-t005s.png` |
| S3 / S22 | 「暂停」+ 黑色停止按钮、「先恢复可用模型」恢复条（前往模型设置 / 重试） | `T1-01-t005s.png` |
| S17 | 「任务计划 0 / 3 已完成」折叠卡 | `T1-01-t005s.png` |
| S14 | 「已处理 0.1 秒 / 18.9 秒 / 20.8 秒」耗时 | 全部 final 截图 |
| C06 | 模型选择器（19 个模型、Kimi Coding Plan 分组、搜索框） | `model-list.png` |
| A2/A7/A9/A11 | 错误卡下的复制 / 保存笔记 / 删除 / 更多操作 | `T1-final.png` |
| E18 | 「完全访问」授权模式、工作区 `aster` 指示 | `T1-00-composer.png` |
| — | 切换模型的预防性提示「在对话中途更改模型会降低缓存命中率，且可能改变计费方式。」 | `T1-00-composer.png` 顶部 tooltip |

**未触发**：thinking / reasoning 块、任何工具渲染器（32 个）、表单、产物卡片、引用、压缩、审批、多模型组、流式 token 速率、成功态回复——因为请求根本没发到模型。222 项清单里本轮真实触发约 **12 个编号**，其余仍为未执行。

## 4. 逐图 UX 分析

依据同上一轮（WCAG 2.2 AA、Nielsen 启发式、Apple HIG、仓库 `frontend-testing.md §4`）。对比度为目测，未量化。

### `T1-basic-reply-00-composer.png` · 输入态

- **符合**：输入框文字完整可见；发送按钮变为实心黑圆箭头，可发送性明确；模型名 `k3.1-pro`、授权模式、工作区指示齐全。
- **符合（亮点）**：切换模型后顶部出现「在对话中途更改模型会降低缓存命中率，且可能改变计费方式」——这是很好的预防性反馈，符合 Nielsen #5 防错与 #10 帮助。
- **不符合**：背景里两张历史错误卡仍完整显示英文诊断，与当前可发送态并存，形成噪声（Nielsen #8）。

### `T1-basic-reply-01-t005s.png` · 发送后 5 秒（模型不可用恢复态）

- **不符合（严重）**：**状态自相矛盾**——「正在处理 5.0s」与「先恢复可用模型 / 连接可用模型后即可继续发送消息」同时出现；错误卡说 `nothing was sent`，状态行却说正在处理。用户无法判断当前到底在跑还是已停（Nielsen #1、#9）。
- **不符合**：「回车 排入下一条，其回车 改这一轮」两段快捷键提示叠在一起，文字重叠、语义不可读（WCAG 1.4.3 / 1.3.1）。
- **不符合**：恢复条给出「前往模型设置」「重试」两个动作，但根因是内部 `reasoning effort default`，两个动作都不能解决；用户会陷入无效重试循环（Nielsen #9）。
- **符合**：「任务计划 0/3 已完成」可折叠、状态行有 spinner、暂停/停止按钮可辨识，反馈渠道齐全。
- **可改进**：composer 在恢复态变成「选择模型」下拉，但没有说明为什么要重新选。

### `T1-basic-reply-final.png` / `live-final.png` · 终态

- **不符合**：三轮失败卡片完全相同且连续堆叠（已处理 0.1 秒 ×2、18.9 秒 ×1），没有去重、折叠或「同类错误合并」；「发生未知错误」+ 英文 detail 对用户既不可诊断也不可恢复（WCAG 3.3.1、Nielsen #9）。
- **不符合**：`The current ACP model ... does not support reasoning effort default` 把 ACP、模型 id、内部参数名全部暴露给终端用户；正确做法是给「当前模型的推理设置不受支持，请在模型设置中调整」这类可操作中文文案，原文进详情/复制日志。
- **符合**：红色左边框 + 图标的视觉层级清晰；用户气泡与助手区分明确；行内操作图标齐全。
- **观察**：这里的标题是「发生未知错误」（`classifyError` 走 unknown），与用户此前截图的「本轮执行未完成」不同——说明 `TaskErrorNote` 的标题随分类浮动，同一类失败在不同载荷下称谓不一致（一致性问题）。

### `model-list.png` · 模型选择器

- **符合**：分组（深度求索 / Kimi Coding Plan）、当前选中勾选、搜索框、标签计数（全部 19）、「配置自定义模型」入口齐全，可扫描性好。
- **可改进**：「全部 19」与分组计数并列，但当前只显示 6 个模型，滚动/分页提示不明显。

### `live-final.png` · 第一次任务（历史错误态）

- 与上一轮 `01-transcript-overview.png` 同类：错误卡与「已删除并确认不存在」矛盾、执行轨迹全铺。本轮复现了**同一问题在重建后依然存在**，且这次是**由真实任务触发**的，不是历史数据。

### 未逐张细评

`live-00-before-send.png`、`live-01-streaming.png`、`live-04-streaming.png`、`T1-03-t015s.png`、`T1-06-t030s.png` 为阶段快照，本轮只用于确认 refs 变化，未完成像素级 UX 评判，不宣称通过。

## 5. 覆盖账本

| 项 | 数量 |
| --- | --- |
| 真实任务发起 | 3 次（2 个模型） |
| 成功到达模型的请求 | **0** |
| 截图 | 10 张 |
| 逐图细评 | 5 张（另 5 张为阶段快照） |
| 相对 222 项真实触发编号 | 约 12 |
| 未执行 | 其余全部（根因：模型链路被阻断 + B/C/D 类需故障注入或 fixture 通道） |

## 6. 环境与状态恢复

- 模型偏好已从 `K3.1 Pro` **切回用户原先的 `deepseek-flash`**（已确认 composer 显示 `deepseek-flash`）。
- 三轮测试任务留在会话里（三条「只回复 OK」用户消息与三张错误卡），未删除，避免抹掉证据。
- 窗口 `w-57385` 与 `just studio` 后台任务 `3IrLiV` 仍在运行。
- 父仓库 HEAD 在重建期间被并发会话推进（stamp 绑定 `e47845b4d` 后又有提交），已如实记录；`apps/studio` 子模块仍有 7 个未提交文件，未 commit / 未 push / 未 bump gitlink。
- 本轮**未修改任何产品代码**。

## 7. Explicitly not claimed

- **未声称真实模型成功回复**：三次请求全部在 ACP 前置校验被拒，`nothing was sent`。
- **未声称触发了「所有机制」**：thinking、工具渲染器、表单、产物、引用、压缩、审批、成功态均未触发。
- **未声称 222 项完整覆盖**：真实触发约 12 个编号。
- **未声称修复了 reasoning-effort 阻断**：只定位到 `reasoning_effort.rs:96` 与调用链，未改代码。
- **未声称 10 张截图全部完成 UX 细评**：5 张细评、5 张阶段快照。
- **未声称对比度/键盘可达性/屏幕阅读器实测通过**：仅目视 + AX 结构摘要。
- **未声称 `just studio-contracts` / vitest / clippy 在本轮通过**：本轮未运行。
- **未声称物理输入没有副作用**：已记录前台应用前后均为 iTerm，但 `--headed` 期间确有可能短暂激活 Aster 窗口。

## 8. 追加：三处修复与真窗口实证（同日稍后）

用户反馈「明显存在大量问题」后，本轮不再只做诊断，直接修根因。

| # | 根因 | 改动 | 验证 |
| --- | --- | --- | --- |
| F1 | 任务页 assistant 错误卡走独立的 `TaskErrorNote`，绕过了上一轮 `ErrorBlock` 的收口 | `TaskConversationMessage.tsx`：detail 计算最高优先级改为 `isInternalProcessErrorPayload(serialized) → error.agent.rpc.internal.message`；import 补齐 | 新增 `TaskConversationMessage.test.tsx` 集成断言（结构化 `data.message` + `responseBody.error.message`），**1/1 通过** |
| F2 | 收口规则只认 `completion gate`，真窗口里满屏的 `ACP model ... does not support reasoning effort ... nothing was sent` 会漏 | `userFacingText.ts` 的 `isInternalProcessError` 增加 `does not support reasoning effort` / `nothing was sent` / `ACP model|runtime` | 新增 1 条单测，**4/4 通过**（`-t` 子集）；`git diff --check` EXIT=0 |
| F3 | `apply_turn_settings` 把哨兵 `default` 当 wire 值，导致所有模型请求在 `session/set_model` 前被拒 | `src-tauri/src/agent_plane/reasoning_effort.rs`：入口把 `ReasoningEffort::Default` 归一为 `None`；补测试断言「default 不阻断 turn 且不发 set_model」 | `rustfmt --check` 单文件 EXIT=0；`e2e-fresh-build.mjs --build` **EXIT=0**（Rust 编译通过）；`cargo test -p aster-studio --lib agent_plane::reasoning_effort` **无法编译**，原因是 5 个 `include_bytes!` fixture 文件被移走（与改动无关，属环境缺失），**未声称该测试通过** |

**真窗口实证**：`e2e-fresh-build.mjs --build`（1m39s）→ `--build-renderer`（1m13s）→ 重启 `just studio` → 窗口 `w-57813` → 截图 `fix-verification.png`。三张历史错误卡的 detail 全部从英文机器诊断变为中文友好文案「请重试，或换一种方式描述你的需求。」。这是 F1+F2 生效的可见证据，不依赖任何模型请求。

**仍未确认**：F3 的行为修复（模型成功回复）**未获实证**——重启后 `--headed` 输入持续 `TIMEOUT`（剪贴板兜底也未写入 value），无法再发任务；`cargo test` 又被缺失 fixture 挡住。全量 TS 套件 `178 tests / 1 failed`，唯一失败是历史记录过的 `opens an edit as a diff and can switch to the split view`（`diff radiogroup`，位于 3637 行，与错误卡改动无交集），未放宽任何断言。

**状态**：`apps/studio` 子模块现有 **10 个未提交文件**（此前 7 + 本轮 3），未 commit / 未 push / 未 bump 父 gitlink；`cargo fmt --all -- --check` 全仓有预存在格式差异（含 `crates/use/tool-acquire/*` 等），仅保证本文件单文件 rustfmt 通过。窗口 `w-57813` 与后台任务 `igLDM4` 仍在运行。

## 9. 错误卡片按业界最佳实践重做（同日 11:0x）

### 依据

Nielsen #1（状态可见）、#9（帮助用户识别、诊断并从错误中恢复）、#8（极简设计）；WCAG 2.2 AA 1.4.3 对比度 / 1.4.11 非文本对比 / 3.3.1 错误识别 / 4.1.2 名称角色值；Carbon / Material / Ant Design 的 inline notification 模式：**图标 + 明确标题 + 一句恢复指引 + 真实动作 + 渐进披露**，默认不放原始堆栈/诊断。

### 改动

| 文件 | 内容 |
| --- | --- |
| `TaskConversationMessage.tsx` 的 `TaskErrorNote` | 改为紧凑 inline alert：标题行 + 恢复说明 + 右上角「复制错误日志」按钮 + 「▸ 详细信息」受控折叠；**原始诊断默认完全不渲染**（不是 CSS 隐藏），仅用户主动展开时出现；`role="alert"` + `aria-live="polite"`、`aria-expanded`、按钮 `aria-label`、图标 + 文字双重编码（不只靠颜色） |
| `task-conversation.css` 的 `.task-error-note*` | 去掉大面积红底与 3px 粗边框，改 2px 语义边 + 3% 淡背景；padding/字号/行高收紧；`width: fit-content; max-width: 100%` 不再撑满整行；`:focus-visible` 可见焦点环；`pre` 限高 8rem 可滚动 |

复用既有 i18n key（`error.copy_log` / `error.details` / `common.copied` / `error.agent.rpc.internal.message`），**未新增翻译 key**，避免 i18n 门禁风险；未新增依赖、未改动通用 `ErrorBlock`。

### 验证

- `TaskConversationMessage.test.tsx` 新增 1 条：`role=alert` / `aria-live=polite`、默认 DOM 不含 `reasoning effort default`、复制按钮存在、点击「详细信息」后 `aria-expanded=true` 且 `<pre>` 出现原文。
- 全量相关套件 **180 tests / 179 passed / 1 failed**；唯一失败是历史记录过的 `opens an edit as a diff and can switch to the split view`（3661 行 `diff radiogroup`，与错误卡无交集）。`git diff --check` EXIT=0。
- `e2e-fresh-build.mjs --build-renderer` ×2 均 EXIT=0；重启窗口 `w-57948` 后截图 `error-card-final.png`：三张卡由「撑满整行 + 英文诊断」变为「按内容收窄的紧凑中文 alert + 复制/详情动作」，AX 快照确认每张卡都有 `复制错误日志` 与 `▸ 详细信息` 按钮。中间态截图 `error-card-redesign.png` 保留作对比。

### 仍未解决（不做假绿）

- 标题仍是 `classifyError` 的 `error.diagnosis.unknown`（「发生未知错误」），信息量低；要按根因给具体标题需要结构化错误码，属 Rust/ACP 侧改造。
- 三张卡分属**三个不同 turn**（中间夹用户消息），是真实历史，不宜在组件内合并；若要降级旧错误的视觉权重，需在消息列表层按 turn 位置处理。
- 收窄后卡片约 170px、说明折两行，偏窄；可加 `min-width: min(22rem, 100%)` 微调（本轮未做，避免再次重建）。
- 没有「重试」按钮：错误卡拿不到可靠的重发回调，不做无响应按钮；当前恢复出口是用户自行重试 + 复制日志 + 查看详细信息。
- `cargo test` 仍被缺失 fixture 阻断，F3 的行为修复仍未获实证。

## 10. 其它 UI 渲染：本轮修复与优化地图（同日 12:0x）

用户追问「其它的 UI 渲染呢」，按同一套标准（WCAG 2.2 AA、Nielsen、渐进披露、内部细节不外露）继续处理。

### 本轮又改了 3 处（均为已定位的确定问题）

| # | 问题 | 改动 | 验证 |
| --- | --- | --- | --- |
| G1 | `AgentComposerContextUsage` 用 `role="img"` 承载「暂未获得上下文用量」状态文本（WCAG 4.1.2） | 改为 `role="status"`，同步更新 `AgentComposerContextUsage.test.tsx` 的 `getByRole` | 相关套件 160 tests / 159 passed，唯一失败为既有 `diff radiogroup`；真窗口 AX 中 `image` 泄漏归零 |
| G2 | `TaskDiffView.tsx:130` 把完整绝对路径写进「在工作区中打开」tooltip | 接入已有 `redactInternalPaths` | 单测 + `git diff --check` 通过 |
| G3 | `TaskConversationMessage.tsx:1665` 的 workspace path citation，`aria-label` 与 `title` 都直出 `/Users/roylin/code/aster`（功能回调仍用完整路径） | 两处展示文案接入 `redactInternalPaths` | **真窗口 AX 实证**：`link \| 在工作区中打开 …/aster`（原为 `…/Users/roylin/code/aster`） |

`--build-renderer` EXIT=0，窗口 `w-58172`，截图 `other-renders-final.png` / `other-renders-verification.png`。

### 复核纠正一条误判

上一轮 UX 审计写「搜索对话框无显式关闭按钮」——**该判断错误**：`WorkbenchHome.tsx:1365-1371` 已有 `aria-label=common.close` 的关闭按钮（显示 `<kbd>Esc</kbd>`）。撤销该条；仅保留「以 kbd 而非 X 图标呈现、对触屏用户不够直观」作为可改进建议。

### 其余形态的优化地图（未全部动手，按依赖分类）

| 组件族 | 已确认问题 | 依赖 | 建议 |
| --- | --- | --- | --- |
| 工具活动 / 聚合组（`ToolBlockGroup`、`TaskActivityGroup`、`WorkingStatus`） | 执行轨迹与成果叙事同权重平铺；目录 UUID、`N 次工具调用`、`词元/秒`、`消耗 X 万词元` 全可见 | **产品决策**：需定义「默认折叠轨迹」的交互 | 成果叙事默认可见，轨迹收进可折叠区；统计信息移到详情/侧栏 |
| 模型不可用恢复条 / WorkingStatus | 「正在处理 5.0s」与「先恢复可用模型」并存自相矛盾；快捷键提示「回车 排入下一条，其回车 改这一轮」文字重叠；「重试/前往模型设置」都解决不了 `reasoning effort default` 根因 | **状态机**：需统一 `TopicStreamStatus` 与 UI 呈现 | 单一权威状态；恢复动作必须指向真实可解的路径 |
| 表单 / 确认卡（`FormPartView`、`AskUserQuestionCard`） | 同一提问连续出现两次、两个「表单已提交」无「已回答/已关闭/已过期」区分 | 组件内可修 | 给已提交/已关闭态明确视觉与文案区分 |
| 错误卡标题 | 仍是 `error.diagnosis.unknown`（「发生未知错误」） | **Rust/ACP**：需结构化错误码 | 按 code 映射具体标题 |
| 新建任务空态 | 只有占位符，无示例/能力入口 | 组件内可修，需产品文案 | 加 2–3 个可点击示例 |
| 设置页 | 左侧「归档」高亮而内容是「通用」疑似选中态不一致 | 需复核是否 hover 残留 | 先复现再改，本轮未动 |
| 多模型组 / 产物 / 引用 / 压缩 / 审批 / 32 个工具渲染器 | 未实跑 | 真模型链路（F3 未获行为实证）+ 故障注入 | 链通后按 `coverage-matrix.md` 逐项补 |

**本轮未动**上述任何一项的产品行为，避免在真实模型链路仍不通、且多项需要产品决策时扩大范围。

## 11. 错误卡进一步压到 1 行（同日 12:1x）

用户问「能 1 行轻量化展示吗」。把 §9 的三行 alert 再压成**单行 inline notice**：

- `TaskConversationMessage.tsx` 的 `TaskErrorNote`：外层改 `flex-direction: column`，默认只渲染一条 `__row`；标题与恢复说明放进同一个 `<p className="task-error-note__summary">`（`white-space: nowrap` + `text-overflow: ellipsis`）；复制与展开改为两个纯图标按钮（`aria-label` / `title` 用 `error.copy_log` / `error.details`，`aria-expanded` 保留）；`<pre>` 仅在展开时作为第二行出现。
- `task-conversation.css`：padding 降到 `0.28rem 0.5rem`、行高 1.4、图标按钮 1.25rem、展开态 `ChevronDown` 旋转 180°；保留 `:focus-visible` 焦点环与 `role="alert"` + `aria-live="polite"`。
- **兼容既有断言**：`[data-task-error-title]` 仍是只含标题的 `<strong>`、`.task-error-note__detail` 仍是只含说明的 `<span>`、`.task-error-note__copy` / `.task-error-note__toggle` class 保留，`<pre>` 仍可被查询。

### 回归与修复

首轮全量跑出 **2 个失败**：一个是既有 `diff radiogroup`；另一个是 `shows a generated file link as its name, not a blocked token`，根因是上一轮 G3 把 workspace citation 的 `title` 也脱敏了，而该测试（以及 tooltip 的用途）要求 hover 时显示完整路径。**已修**：`aria-label` 脱敏、tooltip `title={path}` 保留完整路径（主动 hover 属渐进披露，不是默认暴露）。修复后回到 **160 tests / 159 passed**，唯一失败仍为既有 `diff radiogroup`；`git diff --check` EXIT=0。

### 真窗口实证

`--build-renderer` EXIT=0 → 窗口 `w-58243` → `error-card-one-line.png`：三张错误卡均为单行，形如
`⚠ 发生未知错误 请重试，或换一种方式描述你的需求。 [复制] [展开]`，卡片高度约一行、宽度按内容，默认无英文机器诊断。AX 快照确认三张卡各有 `复制错误日志` 与 `详细信息` 按钮（各 3 个）。

**未变**：标题仍是 `error.diagnosis.unknown`；三张卡分属不同 turn；真模型链路（F3）仍无行为实证；本轮全部改动仍未 commit / push。
