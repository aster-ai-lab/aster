# 第一性原理修复计划：交互反馈与动效（2026-11-11）

配套审计：`UI-UX-AUDIT.md`（9 个页面组、10 项跨页面系统性问题）。**本文不按那 30 条症状逐条开任务**，而是回答：为什么会系统性地产生这些症状，以及最少需要几条结构性约束才能让它们不再复发。

基准：`apps/studio` 干净 checkout `cb3df20eed`（父仓库 `81f657b7b`）。本轮只读，未改任何代码。

---

## 一、第一性原理推导

### 1. 用户真正需要什么

用户点一下按钮、发一条消息、悬停一个工具行时，只需要四件事：

1. **系统收到了**（≤100ms 的可见确认）
2. **现在在做什么**（进展可见）
3. **做完了 / 没做成**（结果可见）
4. **没做成时能恢复**（可操作的下一步）

动效在这四件事里的**唯一合法用途**是帮助理解状态变化，而不是装饰。由此得到两个结论：

- **反馈是数据问题，不是组件问题**：只要「操作 → 状态 → 反馈」不是一条强制通道，就一定会有的页面做全（ArchiveSettings / Capabilities / FilesPage），有的页面完全没有（WorkScheduledTasks / CommandMenus / 回退 composer）。逐页去补 toast 是治标；缺的是「不经过统一通道就不允许发起异步操作」。
- **动效是策略问题，不是样式问题**：只要组件能自己写 `animation: xxx 600ms linear infinite`、自己决定要不要处理 `prefers-reduced-motion`，就一定会时长漂移、漏保护、甚至写出 `@keyframes` 未闭合吞掉 5 条规则而没人发现。逐文件补 reduce 块是治标；缺的是「任何动效必须来自单一权威层，且 reduce 是默认」。

### 2. 为什么问题能反复进入主干

`/Users/roylin/code/aster/.github/workflows/ci.yml:174` 仍在调用 `pnpm run test:contracts:perf-layout`，而 `apps/studio/package.json` 里**没有这个 script**（等价入口是 `node scripts/run-perf-layout-contracts.mjs`）。也就是说 `desktop-frontend` job 的最后一步是坏的；`ci:basic-check` / `ci:test-check` 这两个真正有用的门禁并没有接进 CI 的桌面前端流程。

**没有可靠的机器约束 → 全靠开发者自觉 → 自觉程度因页面而异 → 审计里看到的参差不齐不是偶然，是必然。**

### 3. 根因树（4 个根因解释了全部症状）

| 根因 | 直接后果 | 派生症状（对应审计条目） |
| --- | --- | --- |
| **R1 没有强制的「操作 → 反馈」通道** | 每个页面自行决定给不给反馈 | 自动化页零反馈；命令菜单 6 处 catch 只写 logger；知识库检索/安装吞异常；设置页静默 catch 群；回退 composer 无 busy；模型配置页只有失败 toast；小程序运行页错误态无重试；外链失败静默 |
| **R2 显示值允许与真实值分离** | UI 可以故意滞后于状态 | `useMinimumDisplayDuration` 扣留 1000/1200ms；400ms preparing 延迟；600/1500/650/500ms hover 延迟集群；830ms 交互滚动；「正在处理」与「先恢复可用模型」并存 |
| **R3 动效没有单一权威层** | 组件各写各的，无编译/CI 约束 | token 双轨（`.arc` 变量 vs renderer 硬编码，`var(..., fallback)` 说明变量未生效）；5 个 CSS 文件无 reduce；Spinner/BeatLoader 不受兜底；`@keyframes` 未闭合吞规则；动画定义了未接线 / 被 `animation:none` 覆盖；`grid-template-rows`/height 布局动画 → CLS；全局 `*:focus{outline:none}` |
| **R4 用户可见状态与内部记账共用一根轴** | 内部校验失败被翻译成用户任务失败 | 错误卡直出 `completion gate`/`reasoning effort default`；「已删除并确认不存在」后判「本轮执行未完成」；执行轨迹与成果叙事同权重平铺；内部解析失败表现为空白 |

> 审计里所有 30+ 条症状都能映射到 R1–R4。反过来，只要这四条不变，补完这一轮，下一轮新增页面还会再产生同样的问题。

---

## 二、四条不变量（可机器检查的形式化定义）

计划的全部目标就是让下面四条成为**不变量**，而不是建议。

| ID | 不变量 | 可检查形式 | 违反即失败 |
| --- | --- | --- | --- |
| **I1** | 任何用户可触发的异步操作必须经过统一 `runAction`，自动具备 pending / success / error 三态与 a11y 播报 | ESLint：禁止 `catch` 块内没有 `notify*`/`toast*`/`throw`/错误 state 赋值 | CI 门禁 |
| **I2** | UI 显示值 == 最新真实值；防闪烁靠预留空间，不靠扣住数据 | 禁止 `useMinimumDisplayDuration` 类 hook 用于**状态文本**；禁止用 `setTimeout` 延迟状态落地 | ESLint + 代码审查清单 |
| **I3** | 任何 `animation:` / `transition:` 必须引用全局 motion token；自动继承 reduce；不得改布局属性、不得拦截指针 | `motion-contract:check` 脚本：裸时长字面值、未闭合 `@keyframes`、`animation` 在 reduce 下未停、布局属性动画、`pointer-events` 缺失的 fixed 浮层 | CI 门禁 |
| **I4** | 用户可见成败只由用户可观察结果决定；内部诊断只能进显式详情/复制日志 | `isInternalProcessErrorPayload` 覆盖所有错误出口（当前任务页与通用路径已各有一套，需收敛为单一入口） | 单测 + 真窗口 AX |

### 不变量的边界

- I2 **不禁止**有意的防闪烁措施，但要求改用「预留尺寸 + 骨架」而不是「扣住真实值」。审计里 Capabilities 300ms 最小 spinner 属于用占位保持稳定，与 I2 不冲突；`useMinimumDisplayDuration` 扣住**状态文本**才违反 I2。
- I3 **不禁止**旋转用 `linear`（旋转是 linear 的合理例外），但 shimmer/scan/淡入应走 token。

---

## 三、分阶段计划（按根因分批，不按页面分批）

### Stage 0 · 止损（零风险、不引入新抽象，先让最疼的地方不疼）

| 动作 | 文件 | 出场条件 |
| --- | --- | --- |
| 补 `@keyframes task-pending-shimmer` 闭合 `}` | `task-conversation.css:3255` | CSS 结构检查通过，代码块折叠箭头 180ms 过渡与 `-90°` 生效 |
| `useSmoothScrollAnimation` 接 `useReducedMotion`；reduce → 瞬时；正常 `DEFAULT_FRAMES` 50 → 15（≈250ms） | `list/useSmoothScrollAnimation.ts:16,40` | 单测覆盖 reduce 分支；`just studio-contracts` 等价入口 185/185 保持绿 |
| 取消 `ThinkingBlock` / `ToolBlockGroup` 对 `useMinimumDisplayDuration` 的状态扣留，改 200ms trailing throttle + 立即首帧；保留 error/waiting bypass | `blocks/ThinkingBlock.tsx:16,95-100`、`blocks/ToolBlockGroup.tsx:66,394-399`、hook 本身 | 现有 `useMinimumDisplayDuration.test.ts` 更新为新语义；相关 renderer 套件全绿 |
| 修 CI 门禁脚本名漂移：`ci.yml:174` → 等价入口；并把 `ci:basic-check` / `ci:test-check` 接进 desktop-frontend | `.github/workflows/ci.yml` | CI 上桌面前端 job 实际 EXIT=0 且真的跑了 lint/typecheck/vitest |
| 600ms 聚合预览浮层改 `pointer-events: none`（需局部滚动的区域单独开启） | `TaskConversationMessage.tsx:1269-1275`；`task-conversation.css:2600-2602` | 真窗口：浮层出现后单击锚点一次即生效 |

> Stage 0 只是止血，**不算修复**。它的价值是让 Stage 1–2 在一条可信的基线上做。

### Stage 1 · 建立单一权威层（本计划的核心，三个改动）

| # | 改动 | 落点 | 解决 |
| --- | --- | --- | --- |
| 1 | **`runAction` 统一操作通道**：`runAction(fn, {onSuccess, onError, pendingLabel, successLabel})`，内部统一 pending/disabled/`aria-busy`/toast/live region；页面只声明业务副作用 | `src/renderer/services/`（与已有 `toast.ts` / `popup/presets.ts` 同层，复用现有门面，不引入新库） | R1 |
| 2 | **全局 motion token + reduce 默认**：把 `arc/foundation.css:99-112` 的 `--duration-*`/`--ease-*` 提升到 `:root`（去掉 `.arc` scope），renderer 侧硬编码时长替换为 token；`:root[data-reduce-motion]` 与 `@media (prefers-reduced-motion: reduce)` 做全局 `animation:none; transition:none`，组件无需各自声明 | `packages/ui/src/components/arc/foundation.css`、`renderer/assets/styles/index.css`、`workbenchMotion.css` | R3 |
| 3 | **用户状态轴类型化**：把「用户可见结果」与「内部记账」分成两个字段，UI 只消费前者；失败的内部记账只写日志 + 可选的轻提示，不否定已完成的结果 | 跨 Rust `loop_runtime.rs` → ACP → `TopicStreamStatus` → 渲染层 | R4 |

Stage 1 出场条件：三条不变量各有至少一个真实调用点迁移完成（选 ArchiveSettings 作 I1 正例对照组、任务会话错误卡作 I4 正例），且 `just studio-contracts` 等价入口 185/185 保持绿。

### Stage 2 · 把不变量变成 CI 门禁（防止复发，这是「根本」的关键一步）

复用仓库已有的 `scripts/*check*.ts|mjs` 模式（`check-doc-links.ts`、`verify-doc-structure.ts`、`check-bundle-budget.mjs`、`plugins:contract:check`），新增三个轻量检查，接进 `ci:basic-check`：

| 检查 | 规则 | 理由 |
| --- | --- | --- |
| `motion-contract:check` | ① `@keyframes` 大括号配对、内部只含 `from/to/%` ② CSS 中禁止裸 `animation:`/`transition:` 时长字面值（必须 `var(--duration-*)`）③ 含 `animation:` 的文件必须有 reduce 覆盖或全局兜底 ④ 禁止 `grid-template-rows`/`height`/`margin` 过渡 ⑤ `position: fixed` 且 `z-index >= 50` 的浮层必须有 `pointer-events` 声明 | 直接拦下 F1 那类语法缺陷和 token 漂移 |
| ESLint 规则（在现有 `eslint.config.mjs` 上加 `no-restricted-syntax` 或自定义 rule）：catch 块必须报告（toast / notify / throw / error state） | 拦静默 catch |
| `state-freshness:check`（可用单测而非静态脚本）：断言状态文本渲染值等于真实值，不允许 `setTimeout` 扣留状态 | 拦 I2 回归 |

Stage 2 出场条件：三个检查在 `pnpm ci:basic-check` 中运行；**故意注入一个违规（例如临时加一个裸 `animation: fade 600ms`）能被门禁拒绝**——不做这个负向验证，就不算门禁可用。

### Stage 3 · 按违规清单迁移（此时才按页面推进，但由工具驱动而非人工记忆）

用 Stage 2 的检查输出生成违规清单，按**用户影响 × 迁移成本**排序，分批迁移：

| 批次 | 范围 | 理由 |
| --- | --- | --- |
| B1 | 自动化页、命令菜单、回退 composer、知识库检索/安装 | R1 的 S1 缺口集中，改动局部、收益最大 |
| B2 | 任务会话页动效：600/1500/650/500ms 延迟集群 → 200ms token；复制/工具状态动画接线（去掉覆盖它的 `animation:none`）；artifact 高度统一消除 CLS；推理预览改为「仅在贴底时跟随」；离屏暂停与 `live` 解耦 |
| B3 | 设置页静默 catch 群、模型配置页三态、小程序运行页重试、消息搜索 `aria-live` |
| B4 | 清理重复 keyframes 定义、统一 `focus-visible` 全局基线、Spinner/BeatLoader 纳入 reduce 兜底 |

每批次出场条件：该批次违规清单归零 + 相关 vitest 套件全绿 + 真窗口截图复核（见 Stage 4）。

### Stage 4 · 验收（可运行、可量化，不靠"看起来好了"）

| 维度 | 方法 | 门槛 |
| --- | --- | --- |
| 静态 | `pnpm ci:basic-check` + `pnpm ci:test-check` 在 CI 真跑 | 0 violations |
| 契约 | `just studio-contracts` 等价入口（9 文件 / 185 用例） | 185/185 |
| 真窗口 | 按 `AGENTS.md:83-93` 先 `e2e-fresh-build.mjs --build`，重启窗口后用 `agent-desktop` 跑固定场景清单（工作台、任务会话、设置、连接器、自动化、知识库、小程序、搜索/多选） | 每页截图 + AX 快照归档到 evidence |
| 反馈时延 | 在 `runAction` 里埋 `performance.now()` 打点，真窗口读日志 | 操作 → 首个可见反馈 ≤100ms；无「零反馈」操作 |
| Reduce Motion | 同一场景跑「系统减弱动态 开 / 关」两态截图 + AX 对比 | 开启态下 0 个运行中的 animation；滚动瞬时；无布局位移 |
| 布局稳定 | 长回复 + 产物加载场景截图序列 | 无 120px→620px 跳变；折叠不引起下方内容位移 |
| 内部细节 | 真窗口 AX 全文扫描 | 无 `completion gate` / `effect_digest` / `/Users/` / `reasoning effort default` 出现在可见节点 |

---

## 四、反目标（明确不做，避免范围膨胀）

- 不重写设计系统、不引入新动画库（现有 `motion` + `@aster-ai-lab/ui` 够用）。
- 不做像素级视觉重设计、不换配色与排版。
- 不一次性大爆炸迁移所有 CSS；Stage 0/1/2 先立约束，Stage 3 分批。
- 不为了"通过门禁"删除或放宽既有测试（`never weaken a test or gate to obtain a pass`）。
- 不把装饰性动画一律删光：reduce 下静止、正常下保留合规动效。
- 不在本轮修改任何产品代码；本文件是计划，验收记录另建 `OUTCOME.md`（constitution：计划与验收不是同一份记录）。

## 五、与现有证据/纪律的关系

- 症状来源：`UI-UX-AUDIT.md`、`2026-11-11-message-flow-visual-e2e/OUTCOME.md`、`2026-11-11-live-model-e2e/OUTCOME.md`。
- 落地纪律：任何真窗口验收前必须 `e2e-fresh-build.mjs --build` + 匹配 renderer；Studio 改动先作为子模块提交推送，父仓库过 `studio_land_guard.py check` 再 bump gitlink。
- 触及 disclosures / `MessageVirtualList` / `ModelSelector` 的改动保持 `just studio-contracts` 绿。
- 行为变更在 `docs/evidence/studio/<date-slug>/OUTCOME.md` 记录 commands / counts / Explicitly not claimed。

## 六、Explicitly not claimed

- 本计划**未修复任何问题**，也**未验证** Stage 1–4 的可行性到代码级；只做了根因归纳与现有护栏能力核对。
- `motion-contract:check` / `runAction` 是**拟新增**的抽象，仓库里目前不存在；实施前需按 AGENTS.md 评估「是否有真实复用」，避免单用途抽取。
- 未估算工时，未排期；批次顺序按依赖给出，不含时间承诺。
- 标注为推断的项：Spinner/BeatLoader 在 reduce 下不关闭（未读第三方包实现）、CSS 层叠被覆盖的运行时结果、`@keyframes` 吞规则的浏览器行为（结构缺陷已确认）。
- 已知环境限制：`ci.yml:174` 脚本名漂移；本机 Node 22 低于 `engines >=24.12`，`pnpm` 相关门禁需按本机已验证的等价入口（`node_modules/.bin/vitest`、`node scripts/run-perf-layout-contracts.mjs`）落地。
