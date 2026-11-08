# 模型配置页残留的「路由计划 / 本地预算」tab（已清理）

用户截图：设置 → 模型配置，红框里仍有 `默认模型 | 路由计划 | 本地预算` 三个 tab，质疑「这些功能不是删除了吗」。

## 1. 为什么还在

**上次删除的范围是「本地网关」的设置页与导航入口**（`ApiGatewaySettings` 6 文件、spec030、Magpie 对照 harness、docs、e2e spec、路由与导航跳转等），并**没有**动「模型配置」页里的这组 tab。两者是不同链路：

- tab 由 `ModelSettings.tsx` 的 `localControls = !compact && studioIsAsterTenant()` 控制，只要非 compact 且是 aster 租户就渲染，**没有任何「功能已下线」判断**；
- 「路由计划」→ `LocalModelRoutes.tsx`，数据来自 `usePreference('feature.model_routing.routes')`；
- 「本地预算」→ `LocalModelBudgets.tsx`，数据来自 `usePreference('feature.model_routing.budgets')` + `invoke('local_model_budget_status')`；
- 后端仍完整：`src-tauri/src/lib.rs:252` 注册 `model_control::local_model_budget_status`，`preferenceSchemas.ts` 仍有 `feature.model_routing.*`，类型 `modelRouting.ts` 仍被 `apiGateway.ts` / `codeCli.ts` / `model.ts` 引用。

所以这不是「删了又回来」，而是**上一轮清理只删了网关设置页，漏了模型配置页里的这两个 tab 及其 UI 依赖**。本轮按用户意图把 UI 残留清掉。

## 2. 已清理

| 文件 | 改动 |
| --- | --- |
| `ModelSettings.tsx` | 删 `SegmentedControl` 三个 tab、`localSection` / `localControls` 状态、`section` / `gatewayClientId` props、切换 section 的 effect、`SettingGroup` 的 `hidden`、routes/budgets 条件渲染，以及相关 imports（`SegmentedControl` / `studioIsAsterTenant` / 两个组件 / `LocalModelSection`）。**默认模型四个设置项全部保留**（默认任务 / 快速 / 翻译 / 绘画模型） |
| `LocalModelRoutes.tsx` | 删 |
| `LocalModelBudgets.tsx` | 删 |
| `__tests__/LocalModelControls.test.tsx` | 删（专属测试） |
| `modelSettingsFocus.ts` | 删 `LocalModelSection`、`view` / `client` 参数与 zod 校验，只保留 `focus` |
| `routes/settings/model.tsx` | 简化为 `<ModelSettings focus={focus} />` |
| `__tests__/ModelSettings.test.tsx` | 删 `describe('local model section mounting')` 整块，换成一条「不再渲染退役 tab」回归断言 |
| `__tests__/modelSettingsFocus.test.ts` | 改为断言 `view` / `client` 被忽略、`focus` 仍生效 |
| `i18n/locales/{en-us,zh-cn}.json` | 各删 52 个 `settings.localRouting.*` / `settings.localBudget.*` 孤儿 key（12698 → 12646） |

## 3. 刻意保留（数据面，需单独 redesign）

- `feature.model_routing.routes` / `.budgets` 的 preference schema 与默认值；
- `src/shared/data/types/modelRouting.ts` 及其在 `apiGateway.ts`、`codeCli.ts`、`model.ts` 的引用；
- Rust `local_model_budget_status` IPC 与 `model_control.rs` 实现。

原因：这些仍被 Code CLI、设备连接、`ApiGatewayRequiredDialog` 等链路依赖，硬删会编译失败；上一轮清理已明确把「数据面删除」列为高风险挂起项。现在它们只是**没有 UI 入口的配置/接口**，不影响页面。若要彻底删，需要先设计 Code CLI 的非 gateway provider 路径，再整条下线。

## 4. 验证

| 检查 | 结果 |
| --- | --- |
| `pnpm i18n:check` | 通过（25292 条翻译） |
| `pnpm exec vitest run --project renderer`（ModelSettings + focus） | **2 文件 / 17 用例全过** |
| `pnpm exec rsbuild build` | **exit 0，19.9s**（dist 已更新） |
| `node scripts/run-perf-layout-contracts.mjs` | **9 文件 / 185 用例全过** |
| 完整 E2E | **`PLAYWRIGHT_EXIT=0`，139 passed / 0 failed / 55 skipped** |
| `git diff --check` | 通过 |
| 源码残留扫描 | `LocalModelRoutes` / `LocalModelBudgets` / `LocalModelSection` / `localRouting.tab` 引用 0 |

静态预览在 `http://127.0.0.1:4318`，刷新「模型配置」页即可看到只剩「默认模型」内容、不再有 tab 条。真实窗口走 `just dev` 热更新。

未提交、未 push。
