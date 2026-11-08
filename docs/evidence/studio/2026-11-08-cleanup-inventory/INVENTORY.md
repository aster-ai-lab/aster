# 可清理功能盘点（2026-11-08）

**盘点阶段只读；随后按用户「继续完成深度清理」指令执行了 A / B1 / B4 三组，执行结果见 §E。** 全部结论来自 `apps/studio` 的扫描，按「零风险 / 需产品确认 / 高风险挂起」分级。上位约束见 `tests/e2e/COVERAGE_UPLIFT_PLAN.md` §5：不做假绿、任何口径变化要在 OUTCOME 写明前后数字。

> ⚠️ **与覆盖率口径的联动**：Gate A 分母已统一为 **485**。删除任何 spec 都会缩小分母（例如删 2 个过期 spec → 483），必须在新 OUTCOME 里同时给出前后数字，不能借清理把覆盖率「做上去」。

## A. 零风险，可直接清（已下线且无引用）

| # | 项 | 证据 | 建议动作 |
| --- | --- | --- | --- |
| A1 | 插件商店孤儿 i18n key | 初查 `npx tsx scripts/i18n-check-unused.ts` 报 `plugins.*` 34 个未使用、`plugins.card.` 源码 0 引用 | **已并入 B1**：删掉商店后孤儿增至 53 个，用官方 `--clean --groups plugins` 一次清完（见 §E） |
| A2 | `workspace-terminal.css` 里 6 段 `[data-workspace-terminal-toolbar]` 规则（约 117–155 行） | 组件已删除，只剩 CSS；本轮 E2E 已把「清除」改为 Ctrl+L、「新建终端」改为工作区标签菜单 | 删规则，保留其余终端样式 |
| A3 | 2 个**明确过期**的 `test.skip(true)` spec | `composer-capture-spacing.spec.ts:6`、`composer-image-preview.spec.ts:14` 自述「控件已下线，Capture 接管」；对应零引用组件 `components/composer/recording/ComposerRecordingControl.tsx` | 改写为 Capture 契约用例，或经评审删除（删则分母 485→483） |
| A4 | `playwright.config.ts` 与 `playwright.workbench-office.config.ts` 配置重复 | 前者是 `pnpm test:e2e` 入口、注释自称「renderer workbench-office suite」；后者被 runner 编排。两份的 timeout / reporter / outputDir 已出现漂移 | 保留入口文件，改为 `export { default } from './playwright.workbench-office.config'`，消除配置漂移 |
| A5 | 3 条 CSS / 测试里的死代码注释 | 如 `workspace-terminal.css` 的 `.terminal-toolbar-action` hover/focus 规则随 A2 一并失效 | 随 A2 清理 |

A 组不触碰任何仍在渲染的产品路径，删完应能通过 `pnpm i18n:check`、`git diff --check` 和现有 E2E。

## B. 需产品确认（整条功能已下线，但涉及面较大）

### B1. 插件商店整条（最值得清的一条）

- **证据**：`PluginStorePage.tsx` 仍被 `builtinWorkbenchPlugins.ts:45` 懒加载注册，`routeTree.gen.ts` 仍有 `/app/plugins`；但侧栏只剩「连接器 / 技能 / 自动化」，**没有入口**。本轮已把 3 个 E2E 用例从插件商店迁到 `data-ui="capabilities.page"`（技能能力市场页），静态预览直接访问 `/app/plugins` 也只渲染默认 `/app/agents`。
- **可清范围**：`pages/plugins/PluginStorePage.tsx` + `__tests__/PluginStorePage.test.tsx` + `pluginStoreData.ts` + `PluginArtwork.tsx` / `PluginCardArt.tsx` / `PluginConfigDialog.tsx`（零引用）+ `routes/app/plugins.tsx` + `builtinWorkbenchPlugins.ts` 的 `plugins` 注册 + `tests/e2e/plugins/`（5 文件）+ 相关 i18n key + 重新生成 `routeTree.gen.ts`。
- **不能整目录删**：`pages/plugins/` 下的 `AixAppletWindow.tsx`、`pluginLaunch.ts`、`pluginCallable.ts`、`pluginOkf.ts`、`PluginOkfConfig.tsx` 仍被 applet/OKF 功能使用，需先逐个核对引用。
- **决策点**：能力市场是否已**完全**取代插件商店？若是，整条清；若还保留「内置插件详情」入口（本轮 `first-run-components-motion.spec.ts` 仍在测 builtin plugin details），需保留对应数据读取路径。

### B2. 零引用页面/组件 37 个（需逐个复核）

粗扫 `src/renderer/**/*.tsx`（1312 个，排除测试）得到 37 个文件名在其它源码中 0 次出现，明显属于下线功能残留：

- 旧首页板块：`pages/agents/ConstructionCrew.tsx`、`CreationBoard.tsx`、`WorkScheduledTasks.tsx`、`WritingCenterPage.tsx`；
- 云智能体：`pages/agents/components/CloudAgentPanel.tsx`、`CloudRunHistory.tsx`、`CloudStreamingText.tsx`、`ExecutionTimeline.tsx`、`SkillPublishButton.tsx`；
- 知识/组织：`pages/knowledge/KnowledgeNotebook.tsx`、`TeamEncyclopedia.tsx`、`pages/org/OrgCreateAccount.tsx`；
- 其它：`components/collection/*`、`components/devtools/*`、`components/popups/LocalModelDownloadPopup.tsx`、`components/icons/*` 若干、`pages/settings/*` 若干。

**注意**：该扫描按「文件名出现在其它文件」判定，可能漏掉 barrel re-export、路由自动发现、字符串动态 import，因此**必须逐个用 LSP/ts-morph 复核引用后再删**，不能按清单批量删除。

### B3. 1786 个孤儿 i18n key（14%）

`npx tsx scripts/i18n-check-unused.ts` 报 1786 / 12751 未使用，分组：`platform.* 602`、`knowledge.* 323`、`workbench.* 229`、`settings.* 137`、`paintings.* 126`、`documents.* 84`、`translate.* 73`、`org.* 45`、`plugins.* 34`、`workspace_wiki.* 49` 等。

- 但除 `plugins.card.*` 外，这些前缀在源码里**都有引用**（`platform.` 166 文件、`knowledge.` 354 文件…），很可能是 `t(\`${prefix}.${x}\`)` 动态拼接导致检查器误判。
- 建议：先给 `i18n-check-unused.ts` 增加 `--strict`（排除动态拼接/模板字符串前缀）再分批清；**不要整组删**，否则会误删运行时文案。

### B4. 26 个未编排 E2E 目录 / 294 文件 —— **复核后结论改为「保留」**

`tests/e2e` 共 32 个目录，只有 7 个被 `e2e-runner.mjs` 编排。以「目录名是否出现在编排 spec / scripts / config 里」扫描，11 个目录零引用（`code-mode`、`contract-review`、`intelligent-ui`、`interactive-coding`、`office-catalog`、`task-catalog`、`task-management`、`team-admin`、`team-encyclopedia`、`team-workbench`、`workspace-browser`）。但打开 README 后确认它们是**带用途说明的历史验收 campaign / 研究清单**（Python 脚本、ACL、board-vectors、frozen inputs），不是无主死代码，删掉会丢研究记录 → **保留**。

`tests/e2e/regression/` 原被列为「可优先归档」，复核发现它有完整的 CI 控制器（`scripts/e2e/regression/`：reporter、phases、cases）+ 4 个单测 + `playwright.regression.config.ts` + `tsconfig.e2e.json` + `package.json` 脚本，且被编排 spec 引用 28 处 → **保留**，它只是未纳入默认矩阵，不是废弃。

**本轮实际只清了纯缓存**：`tests/e2e/**/*.pyc` 17 个 + 3 个空 `__pycache__` 目录。

## C. 高风险，此前的明确挂起项（不建议现在动）

### C1. 本地网关数据面（用户曾选「整条本地网关」，但需 redesign）

上一轮已删 22 个路径（spec030 11、Magpie 对照 harness 9、docs 3、e2e spec、ApiGatewaySettings 6 文件、路由、导航专项测试），并清理了中英各 60 个孤儿 key；`routeTree.gen.ts` 已重生成、api-gateway 残留 0。**但数据面仍在**：

- `src/shared/types/apiGateway.ts`、`src/shared/data/types/apiGatewayPairedDevice.ts`、`src/shared/data/api/schemas/apiGatewayPairedDevices.ts`、`src/shared/ipc/schemas/apiGateway.ts` + `ipcSchemas.ts`、`src/shared/data/preference/preferenceTypes.ts` + `preferenceSchemas.ts`、`src/shared/types/codeCli.ts`；
- `src/renderer/hooks/useApiGateway.ts`（4 个非 Magpie 消费者）、`useCodeCli.ts`、`src/renderer/data/studioLocalProviderPlane.ts`（Gemini）、`src/renderer/shell/studioMainBridge.js` 的 IPC；
- `src/renderer/pages/code/hooks/*` 约 4 个 controller、`pages/code` 约 20 文件；
- Rust 侧 `local_api_gateway*.rs` 全族与 10 条 IPC（在 crates 目录）。

**阻塞原因**：Code 页 CLI 配置、设备连接、`ApiGatewayRequiredDialog` 都依赖它；硬删会编译失败。需要先设计「Code CLI 走非 gateway provider 路径」的替代方案，再整条删。本轮**不碰**。

### C2. Cherry → Aster 品牌名残留

`src/renderer` 下 12+ 文件含 `cherry`（`types/newMessage.ts`、`types/messageExport.ts`、`utils/errorDiagnosis.ts`、`utils/knowledge.ts`、`utils/file/*`、`shell/*.test.ts` 等），多数是类型名 / 历史常量而不是用户可见文案。上一轮已把 3 条 zh-cn 可见文案统一为 Aster 让 `i18n:check` 转绿。**全量替换风险高**，需先区分「标识符」与「UI 文案」，且可能涉及持久化数据兼容。

## D. 清理顺序建议

1. **A 组**（零风险，一次提交跑全量回归）→ 2. **B1 插件商店**（若产品确认能力市场完全取代，收益最大）→ 3. **B4 只清纯缓存** → 4. **B2/B3 逐个复核后分批** → 5. **C1/C2 等专门方案**。

每一步都要产出：被删路径清单、`i18n:check` / `git diff --check` / 相关单测结果、Gate A 分母前后数字（若删了 spec）、以及「被删功能是否曾被任一页面入口引用」的证据。

## E. 本轮实际执行（2026-11-08 深度清理）

| 组 | 已做 | 未做 / 原因 |
| --- | --- | --- |
| A | 删 6 段 `[data-workspace-terminal-toolbar]` 死 CSS（保留仍在用的 `.workspace-terminal__follow:focus-visible`）；删 2 个过期 `test.skip(true)` spec（`composer-capture-spacing` / `composer-image-preview`）；`playwright.config.ts` 改为 re-export `playwright.workbench-office.config`，消除 timeout/reporter/trace 漂移与 HTML report 服务不退出 | `ComposerRecordingControl.tsx` **保留**（有 `__tests__/ComposerRecordingControl.test.tsx` 引用，不是死代码） |
| B1 | 删 8 个插件商店专属文件（`PluginStorePage` / `PluginArtwork` / `PluginCardArt` / `pluginCallable` / `pluginStoreData` + 2 个专属测试 + `routes/app/plugins.tsx`）；`builtinWorkbenchPlugins.ts` 的 `plugins` 改 `null`（保留 `SidebarAppId` 与 preference 迁移）；`pnpm exec rsbuild build` 重生成 `routeTree.gen.ts`，`/app/plugins` 残留 0；用官方 `i18n-check-unused.ts --clean --groups plugins` 清 **53** 个孤儿 key（两个 locale 各 53） | 保留 `AixAppletWindow` / `PluginConfigDialog` / `PluginOkfConfig` / `pluginOkf` / `pluginLaunch`（被能力市场、applet、aixApplets 测试引用） |
| B2 | 严格倒排索引复核 1310 个 tsx 得 46 个零引用，其中 ~20 个是 TanStack 路由自动发现 / shim / bench 的天然误判；其余无明确下线证据 | **未删任何组件**，需产品逐个确认 |
| B3 | 已清 `plugins.*` 53 个 | 剩余约 1754 个孤儿**未清**：源码里有 74 个 `workbench.*`、72 个 `org.*`、37 个 `settings.*` 等动态模板前缀，检查器会误报，须先给 `i18n-check-unused.ts` 加动态拼接识别 |
| B4 | 清 17 个 `.pyc` + 3 个空 `__pycache__` | 11 个零引用 harness 目录与 `regression` 套件**保留**（有 README / CI 控制器 / 单测，属研究资产） |
| C | 不碰 | 本地网关数据面、Cherry 全量替换需专门 redesign |

**分母联动**：删掉 2 个 spec 后 `workbench-office` 声明从 196 → **194**（42 文件），全量分母从 485 → **483**；通过数不变（139），跳过从 57 → 55。按 `COVERAGE_UPLIFT_PLAN.md` 的 485 口径需同步改为 483，OUTCOME 必须给前后数字。
