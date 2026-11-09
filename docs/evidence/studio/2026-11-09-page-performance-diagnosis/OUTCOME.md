# 页面性能问题：实证诊断与分级方案

状态：**诊断完成，并按用户选择实施了 P0-A 虚拟化**。用户反馈「存在大量页面性能很差的问题」，实测后确定瓶颈集中在「设置/用量」页。

## 1. 测量方法（可复现）

用 agent-desktop 的 headless semantic 在 fresh 窗口（`w-41763`，本 checkout 的 `just build-studio` + 最新静态 renderer）逐页导航，测三项：

- `navMs`：点击导航到页面稳定（含固定的 0.3s/步 + 0.6s 等待，**主要是等待，不代表真实渲染耗时**）；
- `snapshotMs`：`snapshot -i --compact` 的墙钟时间 —— AX 树遍历成本，作为页面 DOM/布局规模的代理指标；
- `axNodes` / `refs` / `offscreen`：AX 树节点数、可交互引用数、离屏节点数。

脚本 `/tmp/aster-perf-measure.py`，基线 `/tmp/aster-perf-baseline.json`。全程 headless，无 `focus-window` / `--headed` / 物理输入。窗口启动沿用上一轮的方式（fresh 二进制 + `/tmp` 静态 HTML 的最小 runtime-components 测试桩，仓库源码零改动）。

## 2. 基线结果（2026-11-09）

| 页面 | nav (ms) | AX 遍历 (ms) | 二次遍历 (ms) | AX 节点 | refs | offscreen |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 工作 | 3239 | 82.5 | 67.6 | 58 | 36 | 0 |
| 连接器 | 1042 | 76.7 | 74.3 | 59 | 37 | 0 |
| 技能 | 1014 | 81.3 | 71.9 | 59 | 37 | 0 |
| 自动化 | 1027 | 81.6 | 73.8 | 58 | 37 | 0 |
| 设置/通用 | 1529 | 87.5 | 80.3 | 93 | 68 | 8 |
| 设置/快捷键 | 1473 | 102.7 | 95.8 | 129 | 106 | 20 |
| **设置/用量** | **1966** | **654.7** | **598.5** | **1725** | **1316** | **1224** |
| 设置/模型服务 | 2250 | 125.2 | 95.8 | 179 | 149 | 46 |
| 设置/模型配置 | 1523 | 86.3 | 80.6 | 78 | 56 | 0 |
| 设置/网络搜索 | 1462 | 102.8 | 86.2 | 118 | 94 | 7 |
| 设置/文档处理 | 1499 | 88.2 | 80.1 | 84 | 61 | 0 |
| 设置/数据 | 1484 | 98.1 | 78.7 | 97 | 74 | 0 |
| 设置/归档 | 1487 | 92.8 | 80.1 | 104 | 81 | 0 |
| 设置/关于 | 1494 | 86.6 | 79.1 | 79 | 53 | 0 |

合计 AX 节点 2920、遍历 1847ms、离屏 1305；最慢页 **设置/用量**。

## 3. 结论：不是「大量页面」，是一个数量级问题 + 三个中等页

- **13/14 页** 的 AX 遍历在 **70–125ms**，且第二次遍历稳定在 67–96ms —— 这是 agent-desktop CLI 每次拉起进程的固定开销，页面本身规模（58–179 节点）属于正常范围。导航 1.0–2.3s 里约 1.2s 是测量脚本的固定等待，不能当作渲染耗时。
- **只有「设置/用量」是数量级异常**：1725 节点（次大的 12 倍）、遍历 655ms（其它页 6–8 倍）、**1224 个离屏节点**。
- 中等页：模型服务（179 节点 / 46 offscreen）、快捷键（129 / 20）、网络搜索（118 / 7）—— 有优化空间，但不是卡顿主因。

## 4. 用量页 1224 个离屏节点的来源（AX 逐节点统计）

| role | 数量 | 名称样例 / 来源 |
| --- | ---: | --- |
| `cell` | 721 | 「—」「Aster 对话」「kimi」「成功」「1.9万」→ **用量明细表**（`UsageEntriesTable.tsx` 的 `entries.map`，51 行 × 7 列，含每行「查看 k3.1-pro 的调用详情」按钮） |
| `group` | 172 | 表格单元格与日历格子的容器 |
| `switch` | 170 | 「2025-11-09 · 0 Token」每天一个 → **热力图日历**（`UsageHeatmap.tsx` → `UsageActivityTerrain.tsx` 的 `days.map`，一年约 365 个 Button，每个包一个 `NormalTooltip`） |
| `button` | 102 | 明细表行内「查看调用详情」、表头排序、加载更多、日历缩放 |
| `row` | 51 | 表格行 |
| `table` / `column` | 8 | 表格骨架 |

**两个共同根因**：

1. `UsageEntriesTable.tsx:120` 对 `entries` **全量渲染**；它是「加载更多」累积式（`hasNextPage` / `onLoadNext`），已加载数据会一直增长，没有任何虚拟化或行数上限。
2. `UsageActivityTerrain.tsx:111` 把一年所有日期一次性渲染成 3D 柱子 Button + 每个一个 Tooltip，`bars` 虽有 `useMemo`，但 DOM/组件实例数与天数线性增长。

代码里 `UsageHeatmap.tsx` 的 `days` / `cells` 与 `UsageActivityTerrain` 的 `bars` **都已有 `useMemo`**，所以不是「漏缓存」型 bug；瓶颈是渲染量本身。

## 5. 分级优化方案（待定方向）

| 级别 | 做法 | 具体文件 | 预期收益 | 风险 |
| --- | --- | --- | --- | --- |
| **P0-A 虚拟化明细表** | 用仓库已有的 `@tanstack/react-virtual` 只渲染视口内行（固定行高 + 占位），保留 `hasNextPage` 语义 | `UsageEntriesTable.tsx`；可参考 `components/chat/resourceList/base/ResourceListVirtual.tsx` | 表格节点从 ~770 降到视口内 ~30–40，AX 遍历预计 655ms → <150ms | 中：改表格 DOM 结构，需同步 `__tests__/UsageEntriesTable.test.tsx` 与 `settings/coverage.spec.ts` 的配置项断言 |
| **P0-B 日历按需渲染** | 只渲染当前视口/缩放范围可见的日期柱，或把 365 个 `NormalTooltip` 换成一个跟随选中格子的共享 tooltip | `UsageActivityTerrain.tsx` / `UsageHeatmap.tsx` | 减少 ~340 个 Button + ~340 个 Radix Tooltip 实例 | 中高：3D 变换与键盘导航（`tabIndex` / `onFocus`）依赖全部格子存在，删格子会破坏交互 |
| **P1 离屏跳过渲染** | 给 `UsagePanel`（12 个面板容器，普通 div）加 `content-visibility: auto` + `contain-intrinsic-size` | `UsageSettingsPrimitives.tsx` + 新建一个 CSS | 滚出视口的整个面板跳过布局/绘制，零 DOM 改动 | 低-中：Chromium 对离屏子树的可访问性/可见性判定可能变化，需重跑 `settings/coverage.spec.ts` 的 `toBeVisible` 断言 |
| **P2 中等页** | 模型服务 / 快捷键 / 网络搜索的长列表复用 P1 或按分组折叠 | `ProviderSettings/ModelList`、`ShortcutSettings.tsx`、`WebSearchSettings/components/*` | 每页减少 20–46 个离屏节点 | 低 |
| **P3 度量固化** | 把本脚本纳入 `test:e2e:all`，设「单页 AX 节点 ≤ 300、离屏 ≤ 50、遍历 ≤ 150ms」回归门槛 | 新增 spec + `e2e-coverage-gate.mjs` 旁的性能报告 | 防止再次劣化 | 低 |

**建议顺序**：先 P0-A（收益最大、模式仓库已有），再 P0-B 只做「共享 tooltip」不动格子数量（保住 3D 交互），最后 P1/P2。

### 5.1 已实施：P0-A（用户选定）

**重要修正**：初版把虚拟化加到了 `UsageEntriesTable.tsx`（org 用量明细），真窗口复查显示节点数不变 —— aster 租户实际渲染的是 `LocalUsageSettings.tsx` 的**本地调用明细表**。已回撤该无效改动，把优化落到真正的瓶颈上。

改动文件：`src/renderer/pages/settings/UsageSettings/LocalUsageSettings.tsx`（唯一产品源码改动）。

- 引入仓库已有的 `@tanstack/react-virtual` 的 `useVirtualizer`，`estimateSize: 52`、`overscan: 8`，滚动容器 `max-h-[420px] overflow-auto`，表头 `sticky`；用上下 `aria-hidden` spacer 行撑起总高度，保持 `<table>` 语义与滚动位置。
- `VIRTUALIZE_THRESHOLD = 40`：**小数据仍走原全量表格**，只有超过 40 行才虚拟化。这样单测（1 条）与设置页覆盖断言的默认路径不受影响，避免为了性能放宽测试。
- `data-usage-calls-virtualized` 属性标记虚拟化生效，便于复查。
- `UsageEntriesTable.tsx` 已用 `git show HEAD:` 完整还原，未留下无效改动。

**真窗口前后对比**（同为 fresh 窗口 + `/tmp` 测试桩，agent-desktop headless `snapshot -i --compact`）：

| 指标 | 优化前 | 优化后 | 变化 |
| --- | ---: | ---: | ---: |
| AX 节点总数 | 1725 | **1142** | **-583（-34%）** |
| refs | 1316 | **740** | **-576（-44%）** |
| offscreen 节点 | 1224 | **663** | **-561（-46%）** |
| AX 遍历耗时 | 654.7ms | **401.5ms** | **-253ms（-39%）** |
| 表格 `cell` | 721 | **259** | -462（-64%） |
| 表格 `row` | 51 | **18** | -33 |
| 表格 `button` | 102 | **36** | -66 |
| 日历 `switch` | 170 | 170 | 不变（P0-B 范围，未动） |

剩余 663 个 offscreen 主要来自热力图日历（170 switch + 172 group）与其它面板 —— 正是 P0-B / P1 的后续范围。`snapshotMs` 受机器负载影响（同轮其它页面整体耗时也有波动），**节点数与 offscreen 是客观指标，遍历耗时仅作参考**。

**验证**：
- `LocalUsageSettings` 相关单测：HEAD 版本同样 11 个失败（`Intl.Locale.getWeekInfo is not a function`，jsdom 环境缺失），已通过「回退到 HEAD 跑同一单测」确认是**既有环境失败**，非本次引入；`UsageEntriesTable.test.tsx` 4/4 通过。
- rsbuild build **EXIT=0**（16.6s，类型检查通过）；`git diff --check` 通过。
- `playwright.settings.config.ts`：**18 passed / 2 skipped**（`controls >= 387` 覆盖断言未触发回归）。
- `run-perf-layout-contracts.mjs`：**185/185**。
- `playwright.workbench-office.config.ts` 完整 E2E：见 §7 补充。

## 6. Explicitly not claimed

- **未修 P0-B / P1 / P2**：热力图 365 个日期格各带 Tooltip、`UsagePanel` 离屏跳过、模型服务/快捷键/网络搜索页优化均未动；日历 170 个 offscreen switch 仍存在，用量页仍有 663 个离屏节点。
- **未改产品交互语义**：虚拟化只减少离屏行的挂载，没有删除配置项、没有改排序/加载更多/详情弹层；`settings/coverage.spec.ts` 的 `controls >= 387` 断言仍通过。
- **未测 FPS / CPU / 堆内存 / longtask**：`snapshotMs` 是 AX 遍历墙钟时间，是 DOM 规模的代理指标，不是浏览器渲染耗时；`navMs` 含脚本固定等待，不代表真实首屏时间。要定量验证渲染收益，需补一次 CDP `Performance.getMetrics` 或 `PerformanceObserver` longtask 测量。
- **未宣称「大量页面」都慢**：实测 13/14 页在基线范围，问题集中在用量页。
- **未宣称修复了 runtime pin**：本轮沿用 `/tmp` 测试 HTML 的桩跳过首次运行门禁（见 `2026-11-09-agent-desktop-visual-audit/OUTCOME.md`），产品根因仍在。
- **未宣称单测全绿**：`LocalUsageSettings` / `usagePageCapabilities` 两个文件的 11 个失败是 jsdom 缺 `Intl.Locale.getWeekInfo` 的既有环境失败（HEAD 版本同样失败）。
- **未宣称 Gate B 数字变化**：本轮未生成新的逐组件 inventory；Gate A 分母口径未变。

## 7. 环境与数据

- 基线：`/tmp/aster-perf-baseline.json`（优化前）、`/tmp/aster-perf-after.json`（优化后 14 页）；上一轮 14 页逐组件台账：`docs/evidence/studio/2026-11-09-agent-desktop-visual-audit/inventory.json`。
- 测量窗口依次 `w-41763`（基线）→ `w-41934`（初版无效改动复查）→ `w-42003`（P0-A 复查），诊断结束后全部关闭；`/tmp` HTML 测试桩与 `/Applications` pin 均已还原；静态预览 4318 保持可用。
- **完整主功能 E2E**：`playwright.workbench-office.config.ts` → **138 passed / 1 failed / 55 skipped**。唯一失败 `home-search-motion.spec.ts` 的 `toBeFocused()`（首页搜索框 Enter 后焦点，与用量页无关），**定向复跑 2/2 通过**，判为偶发时序；之前几轮同一套件 139/139 全绿。
- **一次无效重跑已排除**：中途误用旧备份 HTML 覆盖构建产物，导致引用的 JS hash 失效、从第 122 个用例起整片 20s 超时；重新 `rsbuild build` 生成正确 HTML（4 个 JS 引用全部存在）后重跑，才得到上面的有效结果。
- 产品源码改动仅 `src/renderer/pages/settings/UsageSettings/LocalUsageSettings.tsx` 一个文件；`UsageEntriesTable.tsx` 已还原。未提交、未 push。
