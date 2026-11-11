# Stage 3 交互反馈与动效修复验收（2026-11-11）

配套计划：`../2026-11-11-ui-ux-page-audit/REMEDIATION-PLAN.md`。按仓库纪律，计划与验收分开记录；本文只写**实际改动、实际命令、实际计数、实测结果**。

## 1. 基准与起点

- Studio 子模块基线：`0a22f9b34e`（`origin/main`）。
- 父仓库：本轮开始时 `70e5f305c`，期间并发会话推进到 `3ebe8a65f`；`crates/code/acp/**` 与 `docs/evidence/code/2026-11-10-acp-cancel-terminal/` 属于并发会话，本轮**未触碰**。
- 起点是上一轮遗留的 3 个未提交文件（B2 与回退 composer），本轮先完成其验证，再继续 B1/B3/B4。
- 改动规模：`15 files changed, 180 insertions(+), 62 deletions(-)`（Studio 子模块）。

## 2. 实际改动

### B1 · 操作 → 反馈通道（R1）

| 文件 | 改动 |
| --- | --- |
| `components/command/CommandMenus.tsx` | 6 处只写 logger 的静默 catch 全部接 `toast.error`（`key: 'command-menu'` 去重，避免右键反复失败刷屏）。用户主动触发却「完全没反应」的两条 native 路径也在内。 |
| `pages/knowledge/KnowledgeNotesPage.tsx` | `installFailed/removeFailed: boolean` → `failedInstall: CloudKnowledgeBundle \| null` / `failedRemoveId: string \| null`，记住失败对象以支持重试；重试开始时先清除旧提示。 |
| `pages/knowledge/CloudKnowledgeHome.tsx` | 失败提示由灰色 `role="status"` 改为 `role="alert"` 的错误条，并直接复用 `onInstall` / `onRemove` 提供「重试」按钮。 |
| `pages/agents/WorkScheduledTasks.tsx` | 保存 / 启停 / 测试运行 / 删除四类操作补成功 toast；删除二次点击态文案由「删除」改为「确认删除」；表单校验错误仍走原有内联 `role="alert"`。 |
| `pages/agents/WorkbenchHome.tsx`（上一轮遗留） | 回退 composer 发送按钮补 `disabled`、`aria-busy` 与 `try/finally` 单飞，避免连点与永久 busy。 |

### B2 · 任务会话动效（R2/R3，上一轮遗留，本轮验证）

| 文件 | 改动 |
| --- | --- |
| `pages/agents/taskConversation/TaskConversationMessage.tsx` | copy timer 改 `useRef` 并在卸载时清理；复制成功 `<Check>` 加 `data-copy-check`；推理预览改为「仅贴底或初始态才跟随」；IntersectionObserver 与 `live` 解耦，离屏暂停在非 live 也生效。 |
| `pages/agents/taskConversation/task-conversation.css` | artifact loading/deferred 高度与 settled 对齐（`clamp(320px, 58vh, 620px)`），消除 120px→620px CLS；从 `animation:none` 覆盖中移除 `.task-state-glyph`。 |

### B3 · 可访问性与失败可恢复

| 文件 | 改动 |
| --- | --- |
| `components/GlobalSearch/GlobalSearchResults.tsx` | `GlobalSearchState` 由裸 `<div>` 改为 `role="status" aria-live="polite"`，错误态 `role="alert" aria-live="assertive"`；`GlobalSearchPanel.tsx` 两处错误态传 `alert`。 |
| `pages/miniApps/MiniAppPage.tsx` | 加载态补 `role="status"` + sr-only 文案；错误态补 `role="alert"` 与调用 `refetch()` 的重试按钮。 |

### B4 · 动效与焦点的全局根因兜底

| 文件 | 改动 |
| --- | --- |
| `assets/styles/index.css` | `*:focus { outline-style: none }` → `*:focus:not(:focus-visible) { ... }`：只抑制指针焦点，键盘焦点保留浏览器默认 `:focus-visible` 轮廓，修复全站「无焦点环」的根因。 |
| `assets/styles/index.css` | reduce 块新增 `[style*='animation'] { animation: none !important }`：覆盖 `react-spinners@0.14.1` BeatLoader 等**内联**动画（class 级 `motion-reduce:` 无法命中）。 |
| `components/Spinner.tsx` | 非 aster 分支的 `motion/react` 无限动画接 `useReducedMotion()`，reduce 下静止在首帧。 |

### i18n 与测试

- `i18n/locales/{zh-cn,en-us}.json`：新增 6 个 key（`command.menu.failed`、`workbench.schedule.{confirm_delete,deleted,run_recorded,saved,toggled}`），各 6 行，按仓库强制的字典序插入。
- `pages/agents/__tests__/WorkScheduledTasks.test.tsx`：确认态文案变更后，同步 2 处断言到新设计值。**未删除、未跳过后、未放宽任何断言**。

## 3. 关键决策

1. **不新增 `runAction` 抽象**。计划 Stage 1 #1 提议新建统一操作通道；复核后确认仓库已有 `toast.loading({ promise })` 门面（内置 pending→success/error 三态）、`toast.success/error`、`aria-busy` 模式（`ArchiveSettings` / `CapabilitiesPage` 为正例）。按 `AGENTS.md` 避免单用途抽取，本轮复用既有门面，**Stage 1 #1 仍未实施**，理由与替代方案记录在此，供后续决策。
2. **I2「显示值 == 真实值」优先于好听的反馈**。工作台自动化页的「测试运行」只写本地运行记录（`manualWorkTaskRunStatus` 按 enabled 推导），并不真实执行任务；因此 toast 文案是「已记录一次测试运行」，不谎报「执行成功」。该页面本身是内存原型，未在本轮改成持久化实现。
3. **确认态用显式文案而非仅变色**。`confirmingDelete` 原来只把「删除」变红，第二次点击容易被误认为是第一次；改为「确认删除」后同步测试断言。
4. **不为了绿而改测试、不放宽门禁**。所有失败都用干净 HEAD 复跑归因（见 §5）。

## 4. Commands

```bash
# 静态与门禁
git diff --check
node apps/studio/scripts/check-motion-contract.mjs
node_modules/.bin/tsx scripts/i18n-check.ts
node_modules/.bin/oxfmt --check <15 changed files>
node --max-old-space-size=8192 node_modules/typescript/bin/tsc --noEmit -p tsconfig.web.json --composite false

# 回归（8 个相关套件）
node_modules/.bin/vitest run --project renderer \
  src/renderer/components/command/__tests__/CommandMenus.test.tsx \
  src/renderer/pages/agents/__tests__/WorkScheduledTasks.test.tsx \
  src/renderer/components/GlobalSearch/__tests__/GlobalSearchPanel.test.tsx \
  src/renderer/components/GlobalSearch/__tests__/GlobalSearchResults.test.tsx \
  src/renderer/pages/miniApps/__tests__/MiniAppPage.test.tsx \
  src/renderer/components/__tests__/Spinner.test.tsx \
  src/renderer/pages/agents/__tests__/WorkbenchHome.test.tsx \
  src/renderer/pages/agents/taskConversation/__tests__/TaskConversationMessage.test.tsx

# 归因：干净 HEAD 上复跑同一批失败用例 / 同一批格式检查
git stash push -- . && <same tests / oxfmt --check> && git stash pop

# 真窗口前置
node apps/studio/scripts/e2e-fresh-build.mjs --build-renderer
STUDIO_RENDERER_DEV=0 STUDIO_RENDERER_PORT=4319 \
  STUDIO_RENDERER_DIST=.studio/dist/e2e-latest ASTER_TENANT_SLUG=aster just studio aster
```

`--build-renderer` 输出 stamp：`rootCommit=3ebe8a65f…`、`studioCommit=0a22f9b34e…`、`rendererEntrySha256=260001a49a0344971b1bf0ba99e3bb3508bae2c8029b766083ab1ac2cce38cef`、`builtAt=2026-11-11T07:13:41.357Z`。

## 5. Counts 与实测结果

| 检查 | 结果 |
| --- | --- |
| `git diff --check` | EXIT=0 |
| `check-motion-contract.mjs` | `scanned 56 CSS files` / `OK` |
| `i18n-check.ts` | 首次因新 key 破坏字典序 EXIT=1；按字典序重排后 `已校验 25308 条翻译`，EXIT=0；两文件 diff 仅 `12 insertions(+)` |
| vitest 8 套件 | `Test Files 4 failed \| 4 passed (8)`；`Tests 6 failed \| 366 passed (372)` |
| 干净 HEAD 复跑 | 同 6 个失败用例全部复现；`WorkScheduledTasks` 在 baseline 为通过（确认该 1 例由本轮文案改动引入，已同步断言后转绿） |
| `KnowledgeNotesPage.test.tsx` | 带改动 `51 failed \| 15 passed (66)`；baseline 同一批失败 `51` 例，均为 `@aster-ai-lab/ui` mock 缺 `SelectionHighlight` 的既有环境问题，**未新增失败** |
| `tsc --noEmit -p tsconfig.web.json` | EXIT=2，全仓库 309 条既有错误；本轮改动文件中仅 `TaskConversationMessage.tsx(198,199,223)` 3 条 remark plugin 类型错误，位于本轮未触及的行 |
| `oxfmt --check` | 8 个文件报格式问题；干净 HEAD 上**同一批 8 个**同样报，且未改动的 `services/toast.ts` 也报 → 仓库既有格式漂移，本轮未新增 |
| 静默 catch 修复数 | 6 / 6（`CommandMenus.tsx`） |
| i18n 新增 key | 6 个 × 2 语言 = 12 行 |
| 测试断言同步 | 2 处 |

6 个既有失败用例：`Spinner.test.tsx` 1、`GlobalSearchResults.test.tsx` 2、`WorkbenchHome.test.tsx` 2、`TaskConversationMessage.test.tsx` 1。全部在 `git stash` 后的干净 HEAD 上复现，与本轮改动无因果关系。

## 6. 失败归因方法

没有把任何一个失败直接标为「历史问题」了事：两次用 `git stash push -- .` 把 15 个改动文件整体收起，在 `0a22f9b34e` 干净树上跑同一批 vitest 用例与同一批 `oxfmt --check`，再 `git stash pop` 恢复并核对 `git status` 文件数（15）。`WorkScheduledTasks` 正是因为 baseline 通过而被识别为本轮引入，随后同步断言修复。

## 7. 真窗口证据

按 `AGENTS.md:83-93`，先用 `e2e-fresh-build.mjs --build-renderer` 构建，再用 `STUDIO_RENDERER_DIST=.studio/dist/e2e-latest` 启动窗口。端口 4318 被一个**旧** dev server 占用，为不触碰任何现存进程，改用 `STUDIO_RENDERER_PORT=4319` 启动；**未复用任何旧窗口**。窗口 `w-61022`（pid 25878）来自本次构建的 renderer 与本次编译的 native host。

采集脚本 `visual-check.py`：全程 headless 语义交付（`snapshot` / `find` / `click <ref>` / `screenshot`），无 `--headed`、无坐标、无鼠标、无 `focus-window`。逐场景结果见 `visual-check-results.json`。

| 场景 | 结果 |
| --- | --- |
| 首启权限弹窗 | `00-onboarding.png`。**第一次采集无效**：弹窗遮住主界面，AX 仅 9 个节点，4 张截图字节数完全相同；已判定为无效证据、删除重复截图，在脚本中显式点「继续」关闭弹窗后重采。 |
| 工作台首页 | `01-workbench.png`：`snapshot_ok=true`，AX 41 节点，内部诊断标记 0，主界面与 composer 正常渲染。 |
| 自动化页 | `02-automation.png`：`control_found=true / click_ok=true`，真实进入「自动化」页（空态「暂无定时任务」），AX 41 节点，内部诊断标记 0。 |
| 测试运行反馈 | `03-automation-feedback.png`：`control_found=false / toast_visible=false`。该页没有任何任务，因此不存在「测试运行」按钮；**不宣称验证了新增 toast**。 |
| 技能页 | `04-secondary.png`：`click_ok=true`，AX 42 节点，内部诊断标记 0；截图捕获在页面加载态（仅 logo），**不作为完整页面渲染证据**。 |

4 个场景的 AX 全文扫描均为 0 个内部诊断标记（`completion gate` / `effect_digest` / `reasoning effort default` / `/Users/`）。

**重要边界**：侧栏「自动化」进入的是设置版 `TasksSettings.tsx`，**不是**本轮修改的工作台版 `WorkScheduledTasks.tsx`；本轮未找到工作台版的可达入口，因此新增 toast 与「确认删除」文案**只有单测证据，没有真窗口截图证据**。命令菜单失败 toast、知识库安装/移除失败重试、mini-app 加载失败重试都需要故障注入，本轮未做，只有代码与类型检查证据。

## 8. Explicitly not claimed

- **未实施 Stage 1**：`runAction` 统一操作通道（见 §3 决策 1）、全局 motion token 从 `.arc` scope 提升、用户可见状态轴与内部记账轴的类型化拆分（跨 Rust → ACP → 渲染层）。
- **未实施 Stage 2 剩余门禁**：ESLint「catch 必须报告」规则、`state-freshness:check`。已有的 `motion-contract:check`（上一轮）未改动。
- **未实施 Stage 3 剩余项**：设置页 27 处静默 catch、`ModelSettings` 默认模型三态、`TasksSettings` 按钮级 pending、知识库检索侧吞异常群、10 组重复 `@keyframes` 清理、消息搜索 `aria-busy`、600ms 浮层吞点击、`grid-template-rows` 布局动画、Spinner 之外 9 个 BeatLoader 使用点逐个改造（本轮以全局内联动画兜底覆盖）。
- **未实施 Stage 4 量化**：反馈时延 ≤100ms 打点、reduce 开/关两态对比、零 CLS 量化、GitHub Actions 上 `ci:basic-check` 实跑、真窗口 AX 全文内部诊断复扫、222 项消息流形态全覆盖。
- **未宣称修复** 6 个既有测试失败、309 条 typecheck 错误、8 个文件既有格式漂移、`KnowledgeNotesPage` 的 51 个既有失败。
- **未获得**真实模型驱动「触发所有机制」的行为实证（沿用 `../2026-11-11-live-model-e2e/OUTCOME.md` 记录的限制）。
- 工作台自动化页仍是**内存原型**（`workSchedule.ts` 的 `memory` 变量），本轮只补反馈，未实现持久化或真实调度执行。
- 不宣称 `format:check` / `typecheck` / `ci:basic-check` 当前为绿；本轮只证明未使它们变差。

## 9. 落地

- **Studio 子模块**：`9561729c30 fix(ux): give every action visible feedback and honour reduced motion`，已 `git push origin HEAD:main`；`ls-remote origin refs/heads/main` 返回 `9561729c30cf57c2cda56f3274394dc4016a8ccd`（快进自 `0a22f9b34e`）。
- **父仓库**：`just studio-land-check` 在 bump 前后各跑一次，均 EXIT=0（`apps/studio worktree is clean`、`pointer advances 0a22f9b34ea8 -> 9561729c30cf`、`delivery commits … are clean`）。本次提交**只暂存** `apps/studio` gitlink 与 `docs/evidence/studio/2026-11-11-remediation-stage3/`；`git diff --cached --name-status` 核对无任何并发会话文件。
- **并发会话隔离**：`crates/code/acp/**` 与 `docs/evidence/code/2026-11-10-acp-cancel-terminal/` 全程未 add、未 commit、未修改。父仓库 `main` 在本轮期间由并发会话从 `70e5f305c` 推进到 `3ebe8a65f`；推送为 fast-forward，会一并带上这些已提交的前序 commit。
- **环境限制**：本机 Node 22 低于 `engines >=24.12`，`pnpm` 门禁使用等价入口（`node_modules/.bin/vitest`、`node_modules/.bin/tsx`、`node_modules/.bin/oxfmt`）；全局 `lefthook` 缺失只打印警告，commit/push EXIT 均为 0。
