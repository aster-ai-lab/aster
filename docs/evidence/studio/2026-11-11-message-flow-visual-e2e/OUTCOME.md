# 任务消息流可视化 E2E 与 UX 审计（2026-11-11）

## 1. 前置与执行方式

| 项 | 命令 / 结果 |
| --- | --- |
| 停旧实例 | 旧窗口 `w-52438` 的 `just studio` 后台任务（运行 11h42m）已停止；4318 释放、窗口数 0 |
| 重建 | `cd /Users/roylin/code/aster && node apps/studio/scripts/e2e-fresh-build.mjs --build` → EXIT=0，3m17s；内部 `just build-studio aster` + `rsbuild build`（`ready built in 1m 12.7s`） |
| Stamp | `rootCommit=e47845b4d95f11cff4cffdf52f2ad9df64dcc47a`、`studioCommit=42303bb224`、`nativeBinarySha256=45f662f79c3f…`、`rendererEntrySha256=342fc8623675…`、`rendererDist=.studio/dist/e2e-latest`、`builtAt=2026-11-11T01:07:38.269Z`；`--verify` = `verified e47845b4d/42303bb22` |
| 启动 | `STUDIO_RENDERER_DEV=0 STUDIO_RENDERER_DIST=.studio/dist/e2e-latest ASTER_TENANT_SLUG=aster just studio aster` |
| 窗口 | `agent-desktop list-windows --app Aster` → `w-56826`、pid 29192、1440×920、`visible=true`、`accessible=true` |
| 权限 | accessibility=granted、screen_recording=granted、automation=unknown |
| 取证 | `python3 docs/evidence/studio/2026-11-11-message-flow-visual-e2e/capture-scenarios.py w-56826` → 16 场景，全程 headless semantic（`find`/`click`/`snapshot`/`screenshot`），未用 `--headed`、`focus-window`、坐标、鼠标、`press` |

**并发漂移（如实记录）**：重建时 stamp 绑到 `e47845b4d`；期间父仓库 HEAD 又从 `f9d44b326` 前进到 `e47845b4d`（并发会话提交）。`apps/studio` 子模块工作树仍有 7 个未提交文件，父 gitlink 为 `07ffdfcf5` 而子模块 checkout 在 `42303bb224`。构建产物来自构建那一刻的 checkout。

## 2. 覆盖范围（不假绿）

- 16 张截图（`screens/01..16`，3016×1976 / 3104×2064 PNG），`captures.json` 记录每个场景的 `action_ok` 与 AX 摘要。
- **`action_ok=True` 8 个**：08 模型选择器、09 授权模式、10 Composer 更多选项、11 搜索对话框、12 轮次跳转、13 工作区面板、15 设置页、16 新建任务空态。
- **`action_ok=False` 7 个**：02–07、14。这些只拍到基态或同一会话的滚动位置（01/02 字节完全相同、05/06 相同、13/14 相同），**不得计为已验证该形态**。
- 01 为全景基线。
- 相对 `INVENTORY.md` 的 222 项：本轮实触发并留证约 18 个编号；A 类其余、B/C/D 类全部未执行。逐项分类与原因见 `coverage-matrix.md`。**这不是完整 E2E 验收。**

## 3. P0 发现：上一轮的内部文本收口对任务会话页无效

`01-transcript-overview.png`、`03`、`07`、`11` 均显示：本次重建后的窗口里，红色错误卡仍完整显示

> 本轮执行未完成
> `completion gate: workspace mutation f9066392… (README.md, index.html, server.py, test_server.py) has no bound Passed verification and no host waiver. Assistant text does not count. Bind a verification_report.effect_digest …`

**根因（代码确认）**：任务会话页的 assistant 消息走 `TaskConversationMessage.tsx` 的 `TaskErrorNote`（第 1987–2023 行），不是上一轮修改的 `blocks/ErrorBlock.tsx`。`TaskErrorNote` 的 `detail` 由 `getSafeProviderErrorMessage({ message, responseBody: error.responseBody, data: error.data })`（第 2007 行）生成后直接渲染为 `<p className="task-error-note__detail">`，**没有经过 `isInternalProcessErrorPayload` 或任何净化**。

即：`MessageContent.tsx:27` 的两套渲染器分流，导致上一轮只修了通用路径；任务页这条路径仍是「错误载荷自由文本直接进可见面」。上一轮 `ErrorBlock.test.tsx` 的 35/35 与 185/185 都是真的，但**测不到这条路径**。本轮真窗口 E2E 才是第一个把它暴露出来的验证。

建议修复：让 `TaskErrorNote` 复用同一收口（对 `serialized` 调用 `isInternalProcessErrorPayload`，命中时用 `error.agent.rpc.internal.message`），并补一条 `TaskConversationMessage` 的对应断言。**本轮未改代码**（任务范围是测试与分析）。

## 4. 逐图 UX 审计

评判依据：仓库 `docs/references/testing/frontend-testing.md §4`（语义角色、固定 locale、禁止实现细节断言）、`ax-page-audit.mjs` 的 R1–R6（可访问名、重复名、offscreen、单 dialog）；业界 WCAG 2.2 AA（1.1.1 / 1.3.1 / 1.4.3 / 3.3.1 / 4.1.2）、Nielsen 十大启发式（#1 状态可见、#2 匹配现实、#8 极简、#9 错误识别与恢复）、Apple HIG 的清晰与渐进披露。对比度为目测，未做像素量化。

### 01 / 03 / 07 / 11 · 任务会话页（用户此前吐槽的同一场景）

| 维度 | 判定 | 依据 |
| --- | --- | --- |
| 错误处理 | **不符合** | 违反 Nielsen #9 与 WCAG 3.3.1：错误文案是内部诊断（digest、`verification_report.effect_digest`、`host waiver`），用户无法据此判断或恢复；且与上一行「已删除并确认不存在…」直接矛盾（Nielsen #1 状态可见性）。上一行说完成、下一行判「本轮执行未完成」，是最严重的体验断裂 |
| 信息层级 | **不符合** | 违反渐进披露 / Nielsen #8：成果叙事与执行轨迹平铺，无主次。「询问用户 <0.1s」「列出文件 Directory: …/01a19bdb-3e19-70fa-…」「4 次工具调用 0.2s」「7.1 词元/秒 消耗 12.1 万词元」全与正文同权重；内部 UUID 与用户对任务无关 |
| 冗余与一致性 | **不符合** | 同一句「请提供要删除的文件路径或选中这些文件；目前消息中没有可识别的文件。」连续出现两次、两个「表单已提交」卡片；WAI 上无状态区分（已提交/待回答/已过期），用户无法判断哪个是当前有效输入 |
| 可读性 | **不符合** | 01/03 中错误卡正文被 `line-clamp-3` 截断，末行文字压在卡片底边，关键诊断读不全；小字浅灰与白底对比度目测偏低（WCAG 1.4.3 风险，未量化） |
| 可访问性 | 部分不符合 | AX 里 `image \| 暂未获得上下文用量` 用 `image` role 承载状态文本，违反 WCAG 1.1.1 / 4.1.2（应作 `status` 或文本）；其余工具按钮可访问名完整（如「17 次工具调用 1 失败 · bash 1.0s」），符合 R1 |
| 视觉层级 | 符合 | 红色左边框 + `CircleAlert` 图标明确标识错误区；用户气泡右对齐、助手内容左对齐，角色区分清楚 |
| 状态反馈 | 部分符合 | 底部「正在连接任务模型… 历史记录可正常查看，连接可用模型后即可继续发送消息」解释了为何不能发送，符合 Nielsen #1；但它与「本轮执行未完成」并存，两个终局状态互相冲突 |

### 11 · 搜索任务对话框

符合：居中模态 + 背景变暗、`esc` 快捷键提示、空态「输入任务名称」、打开后 AX `ref_count=7`（背景被隔离，符合 WAI-ARIA dialog）。
可改进：无显式关闭按钮（仅 `esc` / 点外部），违反「提供多种退出路径」；对话框位置偏上，输入框与空态之间留白偏大。

### 13 · 任务工作区右侧面板

符合：三栏分区清晰，文件树有分组（最近 / 会话工作区）、缩进、类型图标，可扫描性良好（Nielsen #6  Recognition rather than recall）。
可改进：对话区被压到约 1/3 宽，长列表文本换行密集；「最新消息」浮动按钮悬在对话中部，与内容争夺视觉焦点；右栏无收起/宽度调节的明显入口。

### 15 · 设置页

符合：分组（偏好 / 智能 / 系统）清晰，开关统一右对齐、标签左对齐、卡片留白一致，是本次截图中最符合规范的一屏。
问题：**P1** 左侧「归档」呈选中高亮，但右侧内容仍是「通用」——选中态与内容不一致，疑似 hover 残留或状态未同步（违反 Nielsen #1）。**P2** 工作区路径 `/Users/roylin/aster` 直接暴露绝对路径；「保留最近消息数」为禁用态但未给出禁用原因；部分说明文字字号小、灰度浅，对比度存疑。

### 16 · 新建任务空态

符合：中央大标题 + composer 形成明确视觉焦点，留白充足，空态不空洞（Apple HIG / Nielsen #10）。
可改进：除输入框占位符外没有任何示例、能力说明或快捷入口，首次使用者缺少行动指引；「最近」列表仍在，但与空态之间缺少视觉分隔。

### 08 / 09 / 10 / 12 · 弹层与导航（AX 已确认，截图未逐张细评）

08 模型选择器、09 授权模式、10 Composer 更多选项、12 轮次跳转均 `action_ok=True` 且 refs 明显变化（167 / 153 / 151 / 70），说明形态被触发；本轮**未逐张完成像素级 UX 评判**，不宣称通过。

## 5. 缺陷清单（按严重度）

| ID | 严重度 | 问题 | 位置 / 证据 |
| --- | --- | --- | --- |
| D1 | **P0** | 任务页错误卡直出内部门禁全文，且与「已删除并确认不存在」矛盾 | `TaskConversationMessage.tsx:1987-2023`；01/03/07/11 |
| D2 | **P1** | 内部诊断改写失败状态时否定了已完成的用户可见结果（状态轴错位） | `crates/code/core/src/agent/loop_runtime.rs:576-581` → ACP → UI；01 |
| D3 | **P1** | 设置页导航选中态与内容不一致 | 15-settings.png |
| D4 | **P1** | 错误卡正文被 `line-clamp-3` 截断 | 01/03 |
| D5 | **P2** | `image` role 承载状态文本 | AX 快照 `暂未获得上下文用量` |
| D6 | **P2** | 执行轨迹（工具次数、UUID、词元速率）默认全铺，未渐进披露 | 01/03/07 |
| D7 | **P2** | 同一提问文案与「表单已提交」重复出现且无状态区分 | 01/03 |
| D8 | **P2** | 绝对路径 `/Users/...` 出现在设置页与文件工具行 | 15、01 |
| D9 | **P2** | 对话框无显式关闭按钮、空态无行动指引 | 11、16 |

## 6. 四元组与 Explicitly not claimed

通过 16 场景截图取证（其中 8 个动作真正生效）；失败 0；跳过 0；**未执行 222 项中的绝大部分**（B/C/D 类全未执行，A 类仅约 18 个编号留证）。

- **未声称完成 222 项完整 E2E 覆盖**；`action_ok=False` 的 7 个场景明确标为未验证。
- **未声称所有截图都经过像素级 UX 评判**：08/09/10/12 仅确认形态触发，未细评。
- **未声称对比度、点击热区、键盘可达性、屏幕阅读器播报通过**：本轮只做了截图目视 + AX 结构摘要，未做量化或辅助技术实测。
- **未声称修复了 D1–D9 任何一项**：本轮未修改任何产品代码。
- **未声称 `just studio-contracts` / vitest / typecheck 在本轮通过**：本轮未运行。
- **未声称窗口来自当前 HEAD**：stamp 绑定 `e47845b4d`，并发会话期间 HEAD 有漂移，已在 §1 记录。
- **未声称子模块状态干净**：7 个未提交文件仍在，未 commit / 未 push / 未 bump 父 gitlink。
