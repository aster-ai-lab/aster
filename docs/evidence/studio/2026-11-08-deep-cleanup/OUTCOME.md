# 深度清理执行结果（2026-11-08）

上位清单：`docs/evidence/studio/2026-11-08-cleanup-inventory/INVENTORY.md`（A 零风险 / B 需确认 / C 高风险挂起）。本轮按用户「继续完成深度清理」指令执行 **A + B1 + B4（仅缓存）+ B3 的 `plugins.*` 组**；B2 死组件、B3 其余孤儿文案、C 组**未动**，原因见 §6。

## 1. 清理前后口径（不做假绿，分母变化显式留痕）

| 项 | 清理前 | 清理后 |
| --- | --- | --- |
| `workbench-office` 声明用例 | 196（44 文件） | **194（42 文件）** |
| 通过 / 失败 / 跳过 | 139 / 0 / 57 | **139 / 0 / 55** |
| 7 config 全量分母（已决策主口径） | 485 | **483** |
| Gate A 新口径推算 | 139/485 = 28.7% | **139/483 = 28.8%** |
| 70% 门槛所需通过 | 340 | **339** |
| 95% 门槛所需通过 | 461 | **459** |
| 门禁脚本当前输出（旧 197 保守口径） | 70.6% PASS | **70.6% PASS**，`executed 139/139 = 100%` |

> 门禁脚本 `scripts/e2e-coverage-gate.mjs` 仍写死 197（按用户「仅评审规划、暂不执行」的决策，S0 口径改造**未执行**），所以它把减少的 2 个声明算作 `did not run = 3`、`structural ceiling = 72.1%`。这是旧口径的算术产物，不是漏跑；**正式新口径数字以 483 为准**，待 S0 落地后由脚本动态给出。

## 2. 已删除文件

### A 组（零风险）
- `tests/e2e/workbench-office/composer-capture-spacing.spec.ts`（自述控件已下线）
- `tests/e2e/workbench-office/composer-image-preview.spec.ts`（自述控件已下线）
- `workspace-terminal.css` 中 6 段 `[data-workspace-terminal-toolbar]` 规则（保留仍在用的 `.workspace-terminal__follow:focus-visible`）

### B1 插件商店整条（8 个文件）
- `src/renderer/pages/plugins/PluginStorePage.tsx`
- `src/renderer/pages/plugins/PluginArtwork.tsx`
- `src/renderer/pages/plugins/PluginCardArt.tsx`
- `src/renderer/pages/plugins/pluginCallable.ts`
- `src/renderer/pages/plugins/pluginStoreData.ts`
- `src/renderer/pages/plugins/__tests__/PluginStorePage.test.tsx`
- `src/renderer/pages/plugins/__tests__/pluginStoreData.test.ts`
- `src/renderer/routes/app/plugins.tsx`

**保留**（被能力市场 / applet / aixApplets 测试引用）：`AixAppletWindow.tsx`、`PluginConfigDialog.tsx`、`PluginOkfConfig.tsx`、`pluginOkf.ts`、`pluginLaunch.ts`、`__tests__/AixAppletWindow.test.tsx`、`__tests__/PluginOkfConfig.test.tsx`、`__tests__/pluginExperience.test.tsx`。

### B4 纯缓存
- `tests/e2e/**/*.pyc` 17 个 + 3 个空 `__pycache__` 目录

### 文案
- 两个 locale 各删 **53** 个 `plugins.*` 孤儿 key（用官方 `npx tsx scripts/i18n-check-unused.ts --clean --groups plugins`），`en-us` / `zh-cn` 均从 12751 → **12698** key，集合一致。保留 42 个仍被 `homeComposerMode.tsx` 等引用的 `plugins.*` key（含 `plugins.title`）。

## 3. 已修改文件

| 文件 | 改动 |
| --- | --- |
| `src/renderer/windows/builtinWorkbenchPlugins.ts` | `plugins: lazy(...)` → `plugins: null`，保留 `SidebarAppId` 与 preference 迁移；`if (!component) return []` 会跳过 |
| `src/renderer/routeTree.gen.ts` | 由 `pnpm exec rsbuild build` 重新生成，`/app/plugins` 残留 **0** |
| `playwright.config.ts` | 从第二份 `defineConfig` 改为 `export { default } from './playwright.workbench-office.config'`，消除 timeout/reporter/trace 漂移与 HTML report 服务不退出 |
| `src/renderer/i18n/locales/{en-us,zh-cn}.json` | 删 53 个孤儿 key |
| `src/renderer/pages/agents/components/AgentRightPane/workspace-terminal.css` | 删 6 段死规则 |
| 规划文档 | `tests/e2e/COVERAGE_UPLIFT_PLAN.md`、`tests/e2e/PAGE_COMPONENT_E2E_PLAN.md` 的分母 485→483 全量同步；`INVENTORY.md` 补 §E 执行记录并修正 B4 结论 |

## 4. 验证结果（全部在最新构建产物上）

| 验证 | 结果 |
| --- | --- |
| `STUDIO_RENDERER_DIST=/tmp/aster-e2e-dist pnpm exec rsbuild build` | **exit 0，18.4s**（含插件商店删除后的类型/解析与 routeTree 重生成） |
| 源码残留扫描 | `PluginStorePage` / `pluginStoreData` / `PluginCardArt` / `PluginArtwork` / `pluginCallable` 引用 **0**；`routeTree.gen.ts` 中 `/app/plugins` **0** |
| `pnpm i18n:check` | **通过（25396 条翻译）** |
| `node scripts/run-perf-layout-contracts.mjs` | **9 文件 / 185 用例全过** |
| `vitest --project scripts i18n-check-values.test.ts` | **19/19** |
| `vitest --project renderer` 定向 6 文件（插件/能力市场/侧栏/录制） | **4 文件 89 通过，2 文件 6 失败**（见 §5，既有失败） |
| **完整 E2E**（`/tmp/aster-e2e-cleanup.log`） | **`PLAYWRIGHT_EXIT=0`，139 passed / 0 failed / 55 skipped**，6.4min |
| `git diff --check` | 通过 |

运行方式沿用稳定方案：`/tmp/aster-e2e-dist` 静态产物 + `node scripts/e2e-preview.mjs`（**不启用** `STUDIO_E2E_STUB_RUNTIME`）+ `--output=/tmp/aster-e2e-cleanup-results`，避免 dev server 监听源码树。

## 5. 6 个既有单测失败（非本次清理引入）

`vitest --project renderer` 定向 6 个相关文件时出现 6 个失败，分布在两个**本轮未修改**的测试文件：

- `src/renderer/pages/capabilities/__tests__/CapabilitiesPage.test.tsx` 5 个：3 个断言 `aria-label="capabilities.install"` 实得 `"capabilities.installing"`（安装状态机时序）；2 个断言 upload dialog overlay 的 class。
- `src/renderer/components/plugins/__tests__/pluginSidebarProviders.test.tsx` 1 个：找不到 ActivityBar 按钮 `review/entry`。

判据：本轮清理只删插件商店专属实现、路由与文案，未改这两个测试文件、未改 `pluginSidebarProviders` 相关源码；`CapabilitiesPage.tsx` 的既有 diff（按钮宽度 / 不可用条目改为纯文本）来自**更早的会话**，不属于本轮清理。这些失败与「插件商店页面是否存在」「`plugins.*` 文案是否存在」无因果关系。建议单独列任务跟进，不在清理轮里夹带修改。

## 6. 明确未清理（及原因）

| 项 | 原因 |
| --- | --- |
| 46 个严格零引用 tsx | 倒排索引复核后含 ~20 个 TanStack 路由自动发现 / shim / bench 的天然误判；其余无明确下线证据。**未删任何组件** |
| 剩余约 1754 个孤儿 i18n key | 源码里有 74 个 `workbench.*`、72 个 `org.*`、37 个 `settings.*` 等动态模板前缀，检查器会误报；须先给 `i18n-check-unused.ts` 加动态拼接识别再分批清。**只清了 `plugins.*` 53 个** |
| 11 个零引用 E2E harness 目录 + `tests/e2e/regression/` | 前者是带 README 的历史验收 campaign / 研究清单，后者有完整 CI 控制器 + 4 个单测 + config + tsconfig，属有效资产；**只清纯缓存** |
| 本地网关数据面 / Cherry→Aster 全量替换 | 盘点文档 C 组高风险项，需专门 redesign；本轮不碰 |

## 7. 状态

未提交、未 push、未 bump submodule gitlink。静态预览服务仍在 4318 运行（`/tmp/aster-e2e-dist`），可继续复跑。
