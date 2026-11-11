# agent-desktop 全量 E2E 与本批优化验收（2026-11-11）

配套：上一批 `../2026-11-11-remediation-stage3/OUTCOME.md`、总计划 `../2026-11-11-ui-ux-page-audit/REMEDIATION-PLAN.md`。本文只记录**实际改动、实际命令、实际计数、实测结果**。

## 1. 基准与可追溯构建

- Studio 子模块：本批提交 `8c4df145e7`（父提交 `9561729c30`）。
- 父仓库：本轮期间被并发会话从 `49e0fd209` 推进到 `e2f4eb3d9`；并发改动全程未触碰。
- 构建：`node apps/studio/scripts/e2e-fresh-build.mjs --build-renderer` EXIT=0，stamp 为
  `rootCommit=e2f4eb3d951cc9da052a9051908bd30fd4b36423`、
  `studioCommit=8c4df145e72f86ba84d95e953724a01516823b35`、
  `nativeBinarySha256=c5971883434755463c48c54271465932c17033da8fd0b97f269cabcd59462bd4`、
  `rendererEntrySha256=5fba987da1a6cdb1a75d25bfe2e4442d4717446479b42dbc3a64238ff6d04cd2`、
  `builtAt=2026-11-11T08:06:33.061Z`。
  **先提交子模块再重建 stamp**，保证窗口服务的 renderer 与 `studioCommit` 一致（第一次构建时 stamp 仍指向旧 commit，已作废重跑）。
- 窗口：`STUDIO_RENDERER_DEV=0 STUDIO_RENDERER_PORT=4320 STUDIO_RENDERER_DIST=.studio/dist/e2e-latest ASTER_TENANT_SLUG=aster just studio aster` → `w-61786` / pid 7549。端口 4318 被旧 dev server 占用、4319 属上一批窗口，故启用独立 4320；**未杀任何现存进程、未复用旧窗口**。
- **stamp 时效性说明**：E2E 全部场景在 `builtAt=08:06:33Z` 之后、父仓库提交之前跑完，当时 stamp 与窗口一致。提交后 `e2e-fresh-build.mjs --verify` 会报 stale（`rootCommit` 前进到 `286aab73a`、并发会话重编使 `nativeBinarySha256` 改变），这是 stamp 绑定「构建时刻」的预期行为，**不表示本轮 E2E 跑在过期构建上**；若需再次真窗口验收，必须重新 `--build` 并重启窗口。

## 2. 本批优化（`8c4df145e7`，7 文件 +64/−18）

| 项 | 改动 | 原因 |
| --- | --- | --- |
| A2 命令菜单误报 | 6 处 `reportMenuFailure()` 收敛为 2 处：只保留「整个 native 菜单展示失败」与「popup 菜单失败」。额外菜单项解析失败的 4 处恢复为仅 logger。 | 那 4 处失败时菜单**照常打开**、用户操作仍成功，弹「操作失败，请重试」是假警报且无可重试对象，反违反 I1/I4。 |
| A3 知识库并发竞态 | `installBundle` 的 `busyId === bundle.id` 改为 `busyId`（与 `removeBundle` 一致）；`CloudKnowledgeHome` 新增 `busy` prop，两个重试按钮加 `disabled` / `aria-busy` / disabled 样式。 | 上一批新增的重试按钮放大了竞态：下载 A 时点 B 会覆盖 `busyId`/`download`，A 的进度回调继续写状态、A 的 `finally` 提前解除 busy。 |
| A4 reduce 兜底收窄 | `[style*='animation']` → `[style*='animation:']`。 | 只匹配真正的内联 CSS 声明，避免把 `--animation-name` 之类自定义属性当成运行中的动画。 |
| B3 页面加载态 | `AsterPageLoading` 在 wordmark 之外增加可见 `<span class="page-loader__label">`，文本与 `aria-label` 完全一致（Label-in-Name）；新增 `.page-loader__content` / `__label` CSS 与 is-leaving 淡出联动；新增回归测试。 | 原来只有 logo，视觉上像卡死的静止帧；`role=status` 已有但只对屏幕阅读器可见。 |

### 更正（不改写已发生的改动）

上一批 OUTCOME §2 的「修复全站无焦点环的根因」措辞过强，且一次口头风险清单把组件级 `outline:none` 误判为缺陷，已在 `../2026-11-11-remediation-stage3/OUTCOME.md` 新增 **§10 复核更正**：那些位置大多是 `:focus-visible` 规则自带的自定义焦点指示（background / box-shadow）或富文本编辑器的文本光标，**保持原样才正确**；真实待审计面是 2163 个 `.tsx` 中 24 个含 `outline-none` 却无 `focus-visible` 字样的文件，属静态可疑清单，未逐元素确认。本批**没有**为了「修缺陷」去改这些 CSS，避免制造双环回归。

## 3. 验证命令与结果

```bash
git diff --check
node scripts/check-motion-contract.mjs
node_modules/.bin/tsx scripts/i18n-check.ts
node_modules/.bin/vitest run --project renderer \
  src/renderer/components/layout/__tests__/PageEntry.test.tsx \
  src/renderer/components/command/__tests__/CommandMenus.test.tsx \
  src/renderer/pages/agents/__tests__/WorkScheduledTasks.test.tsx
```

| 检查 | 结果 |
| --- | --- |
| `git diff --check` | EXIT=0 |
| `check-motion-contract.mjs` | `scanned 56 CSS files` / `OK` |
| `i18n-check.ts` | `已校验 25308 条翻译`，EXIT=0（本批未改 i18n） |
| `PageEntry.test.tsx` | 新增「可见加载文字与 aria-label 一致」用例**通过**；既有 `stays on screen while the view is not ready…` 仍失败（`.aster-stroke` 期望 6 得 0）。用 `git stash push -- .` 在干净 HEAD 复跑，同一用例同样失败 → 既有，非本批引入。 |
| `CommandMenus.test.tsx` / `WorkScheduledTasks.test.tsx` | 通过 |

## 4. E2E 方法（`e2e-scenarios.py`）

严格 **headless 语义交付**：`snapshot` / `find` / `click` / `right-click` / `wait` / `get` / `screenshot`；**无 `--headed`、无 `focus-window`、无坐标、无鼠标、无物理键盘**（`press` 在 macOS 是物理输入，脚本不使用）。

相比上一批新增的能力（来自 `agent-desktop skills get desktop` / `platform` 指南）：

- `right-click <ref>` 在 macOS headless 走 `AXShowMenu`，可测命令菜单；`snapshot --surface menu` 读取菜单 surface。
- `wait --text` / `wait <ms>` 用于异步稳定，替代「点击后立刻截图」的竞态。
- AX 节点带 `role` / `name` / `description` / `states` / `available_actions` / `ref_id`，脚本据此统计 `role=status` / `alert` / `textfield` 数量、扫描内部诊断标记、挑选可右键节点、捕捉瞬时加载文字。
- 每个动作记录 `ok` + `data.disposition.delivery` + `error.code`；控件找不到记 `CONTROL_NOT_FOUND`，**不假绿**。
- 每个页面场景记录两份 AX：`transient_ax`（点击后立刻，用于观察加载态与可见 label）与 `ax`（settle 后，用于页面截图与扫描）。

场景清单：`S00` 首启弹窗、`S01` 工作台、`S02` 任务搜索、`S03` 自动化页、`S04` 自动化反馈、`S05` 技能、`S06` 连接器、`S08` 设置、`S09` 模型选择器、`S10` 知识库、`S11` 小程序、`S12` 还原工作台、`S13` 右键命令菜单（放最后，避免菜单 surface 遮挡其它页）。

## 5. E2E 结果

共跑 3 次，前两次日志原样保留为 `scenarios-run1.jsonl` / `scenarios-run2.jsonl`，不覆盖失败证据：

| 运行 | 场景 | action_ok | 判定 |
| --- | --- | --- | --- |
| run1 | 13 | 2 | **无效**：`S02` 打开的搜索对话框替换了窗口 AX 树（58→14 节点），后续 10 个场景全部连锁 `CONTROL_NOT_FOUND`。属 harness 未关闭对话框，不是应用缺陷；判无效后修脚本重跑。 |
| run2 | 14 | 7 | 加 `dismiss_overlay()` 后恢复；但页面场景未先回工作台，且控件名是猜测的（模型实际是 `combobox` 不是 `button`），`S09–S11` 仍失败。 |
| run3（定稿） | **15** | **9** | 按真实 AX 控件名重排场景并每场景前 `restore()`；14 张截图，最终 AX 回到 58 节点，无残留浮层。 |

定稿结果文件：`scenarios-run3.jsonl`、`scenarios.json`、`01`–`14` 共 14 张 PNG。逐场景关键数据：

| 场景 | AX 节点 | 动作 | 观察 |
| --- | --- | --- | --- |
| S00 首启弹窗 | 59 | 点「继续」成功 | 弹窗关闭后回到完整工作台 |
| S01 工作台 | 59 | — | 侧栏/composer/任务行语义齐全 |
| S02 搜索对话框 | 14 | 点「搜索任务」成功 | dialog surface 正确替换窗口树 |
| S02b 搜索输入 | 17 | `set-value` **`delivered_verified`** | 出现真实结果「➜ aster git:(main) ✗ aster · 6小时前」，匹配词高亮 |
| S03 自动化页 | 58 | 点「自动化」成功 | AX 尾部为「已暂停/已完成/已错过」，页面确实切换 |
| S04 测试运行反馈 | 58 | `NO_TASK_TO_RUN` | 空态无任务，**不宣称验证 toast** |
| S05 技能页 | 114 | 成功 | 卡片/安装/上一页/下一页 |
| S06 连接器页 | 103 | 成功 | 同构布局 |
| S08 设置页 | 138 | 成功 | 分组表单与开关 |
| S09 模型选择器 | 91 | 成功 | `combobox "DeepSeek Flash"` 打开，出现 GPT-5.6 Luna 等模型 |
| S10 更多选项 | 65 | 成功 | 上传附件 / 自动审查 / 产物追溯 / 关闭 |
| S11 授权模式 | 64 | 成功 | 5 个模式，高风险项红色标注 |
| S12 还原工作台 | 58 | — | 干净还原 |
| S14 composer 输入 | 58 | `ACTION_FAILED` | Tauri 富文本 composer 拒绝 headless `set-value`；未点击发送，未创建任务 |
| S13 右键菜单 | — | `ACTION_FAILED` | 4 个候选均失败；出现的是应用内任务详情 HoverCard，非系统菜单 surface |

- 全场景 `internal_markers=[]`：未出现 `completion gate` / `effect_digest` / `reasoning effort default` / `workspace mutation`。**但该扫描只覆盖 AX 的 name/description/value**，见 `ux-analysis.md` 跨页面观察第 2 条的盲点说明。
- 全场景 `status_nodes=0`、`alert_nodes=0`、`loading_label_seen=false`：本轮未触发加载/错误/慢路由，因此**新增的可见加载文字与 aria-live 只有单测证据，无真窗口行为实证**。
- 工作台侧栏没有「知识库」「小程序」入口，这两个路由本轮**未覆盖**（run2 曾以猜测控件名尝试并如实记为 `CONTROL_NOT_FOUND`，定稿已移除假控件）。
- 逐截图 UX 分析发现两处此前未记录的缺陷：`06` 连接器卡片把「该条目没有可识别的本地接入配置」当描述外露；`13` 任务详情浮层把词元/工具调用/失败工具/记忆条目等内部记账与用户可见信息平铺。详见 `ux-analysis.md`。

## 6. 逐截图 UX 分析

见 `ux-analysis.md`（按视觉层级、交互反馈、动效、可访问性、一致性逐张评估）。

## 7. Explicitly not claimed

- **未做** Stage 1 三条结构性改动（`runAction`、全局 motion token 提升、用户状态轴类型化）、Stage 2 的 ESLint catch 规则与 `state-freshness` 检查、Stage 3 剩余的设置页 27 处静默 catch / `ModelSettings` 三态 / 10 组重复 keyframes / 600ms 浮层吞点击 / `grid-template-rows` 布局动画 / 消息搜索 `aria-busy`。
- **未做** B1（工作台自动化仍是内存原型，`workSchedule.ts` 的 `memory` 刷新即丢、「测试运行」按 enabled 推导结果、从不真实执行）与 B2（`TaskErrorNote` / `ErrorBlock` 双错误路径收敛、错误卡标题与重试）。二者属产品语义与较大重构，需单独决策。
- **未做** Stage 4 量化：反馈时延 ≤100ms 打点、reduce 开/关两态对比、零 CLS 量化、GitHub Actions 实跑、真模型驱动 222 项消息流机制、`page×component` 覆盖率 Gate B。
- **未验证** 需要故障注入的路径：命令菜单失败 toast（改动已收敛但真窗口未注入失败）、知识库安装/移除失败与重试按钮 disabled、mini-app 加载失败重试、错误卡内部诊断脱敏。
- **未验证** 键盘焦点环（`press` 是物理输入，本轮无 `--headed` 授权）与 `prefers-reduced-motion` 开启态。
- **未宣称修复** 既有失败：`PageEntry` 的 `.aster-stroke` 用例、上一批的 6 个 vitest 失败、309 条 typecheck 错误、8 个文件的 oxfmt 漂移、`KnowledgeNotesPage` 的 51 个环境问题失败。
- 页面截图只证明**该时刻该窗口**的渲染与 AX 状态，不等价于 222 项消息流形态全覆盖。

## 8. 落地

- **Studio 子模块**：`8c4df145e7 fix(ux): remove false alarms, serialize downloads, label page loaders`，已 `git push origin HEAD:main`（`9561729c30..8c4df145e7`），`ls-remote` 返回 `8c4df145e72f`。
- **父仓库**：`just studio-land-check` 在 bump 前后各一次 EXIT=0（`worktree is clean`、`pointer advances 9561729c30cf -> 8c4df145e72f`、`delivery commits … are clean`）。本次提交只暂存 `apps/studio` gitlink、`2026-11-11-remediation-stage3/OUTCOME.md`（新增 §10 更正）与 `2026-11-11-agent-desktop-e2e/` 整个证据目录；`git diff --cached --name-status` 核对无并发会话文件。
- **并发隔离**：`crates/code/acp/**`、`crates/terminal/**` 的 staged rename、`docs/evidence/code/2026-11-10-acp-cancel-terminal/` 全程未 add / 未 commit / 未修改。父仓库 `main` 在本轮期间由并发会话从 `49e0fd209` 推进到 `e2f4eb3d9`，推送为 fast-forward，会一并带上这些已提交的前序 commit。
- **环境限制**：本机 Node 22 低于 `engines >=24.12`，门禁走等价入口（`node_modules/.bin/vitest` / `tsx` / `oxfmt`）；全局 `lefthook` 缺失只打印警告，commit/push EXIT 均为 0。`agent-desktop` 全程 headless，未使用 `--headed` 物理输入。
