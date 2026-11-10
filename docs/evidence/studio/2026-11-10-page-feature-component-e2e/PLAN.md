# Aster Studio 逐页面 · 逐功能 · 逐组件 端到端测试计划

状态：**规划稿，未实施**。本轮只做只读侦察与计划编写，未新增/修改任何测试、配置或门禁，未运行任何用例，未构建桌面应用。所有数字均标注来源；凡未经本轮执行验证的，一律写作「盘点值 / 待重测」，不作为验收依据。

范围：`aster` 单租户的 Aster Studio 桌面应用（Tauri native host + renderer），macOS 为主验证平台。CLI / ACP Rust 侧行为、发布治理脚本、纯组件变体与性能布局契约各有归属，不搬进本计划。

---

## 0. 文档定位与口径校准

`apps/studio/tests/e2e/` 下已有三份规划稿：`PLAN.md`（B1–B7 基建 + 37 条场景级用例）、`PAGE_COMPONENT_E2E_PLAN.md`（15 页 × 1522 组件台账）、`COVERAGE_UPLIFT_PLAN.md`（S0–S4 覆盖率路径）。三份都带 2026-11-09 归档说明，且都自述「规划稿，未实施」。

本文件是**唯一上位计划**：补齐缺失的「逐功能」层，把三层用统一 ID 串成一份可核对台账，并校准失效口径。三份旧稿作为历史决策记录保留，本轮**不修改 `apps/studio` 子模块内的任何文件**——该子模块当前是已推送的干净 gitlink，改动它会违反 `AGENTS.md:94-98` 与 `.cursor/rules/studio-land-discipline.mdc` 的「Studio 脏树不得交付」要求。

### 0.1 必须校准的六条

| # | 旧稿口径 | 本轮核对结果 | 本计划采用 |
| --- | --- | --- | --- |
| 1 | 双租户隔离、`toyou` 51 例门禁、Gate A 上限 89.4%、95% 目标挂起 | `AGENTS.md:200` 明确 `aster` 无账号、托管认证只属 `toyou`；三份旧稿自己的 2026-11-09 归档说明已宣布该口径失效 | **本计划按 `aster` 单租户范围**，删除双租户矩阵与 51 例门禁结论。P0「登录与双租户隔离」整条撤除，改为「无账号身份边界 + 单实例数据隔离」。Gate A 分母与上限须按单租户重算，不沿用 483 / 89.4%。若日后恢复多租户，须另起范围变更并重新冻结分母（`AGENTS.md:193` 仍要求租户身份与数据隔离） |
| 2 | 页面基线 15 页 / 1522 组件，含「设置/API 网关 63」 | 该页已下线；`scripts/e2e-coverage-gate.mjs:29` 仍硬编码 `BASELINE_COMPONENTS = 1522` | **1522 / 1446 达标线作废**。基线由 S0 重新导出后按新总数 × 0.95 计算，并在 OUTCOME 写明前后数字 |
| 3 | Gate B `UNMEASURABLE`：`ax-page-audit.mjs` 不输出 `components[].coveredBy` | **已过时**：`scripts/ax-page-audit.mjs:133,221` 已 push `components[]` 且每项含 `coveredBy: []`；`scripts/e2e-coverage-gate.mjs:141-163` 已能读取并给出 `measurable / coverage / zeroCoverage` | Gate B 的**数据链路已具备**，缺的是「通过的 spec → 组件 key」回填与真窗口重测。S0 只需补回填，不再改 schema |
| 4 | CI 唯一门禁 `pnpm run test:contracts:perf-layout` | `.github/workflows/ci.yml:178` 仍调用它，但 `apps/studio/package.json` 的 `scripts` 中**不存在**该名字（本轮 Grep 确认）；等价入口为 `just studio-contracts`（父仓库）或 `node scripts/run-perf-layout-contracts.mjs` | 列为 **S0-B1 阻断项**：修 CI 调用或补 script 别名，验证后写入 OUTCOME。在此之前不得声称契约门禁在 CI 上 green |
| 5 | 侧栏 14 个应用项全部可达 | `src/renderer/utils/sidebar.ts:137,139`：`TEMPORARILY_HIDDEN_ACTIVITY_BAR_APP_IDS = {assistants, plugins}`（全部租户）、`LOCAL_HIDDEN_APP_IDS = {messages, org, code_tools}`（aster 专属） | aster 下 Activity Bar **可见 9 项**；其余 5 项路由仍注册但隐藏/重定向，按「条件可达」单独测，不计入默认页面分母 |
| 6 | 目标「功能覆盖率 ≥95%」写死 | 该目标来自旧稿且绑定已作废的 1522 分母；`constitution.md §II` 要求口径变更留痕 | 改为**三层各自的可核对验收**（§9），分母由 S0 重测确定，不预设 95% 数字 |

### 0.2 仍然有效的硬约束（`AGENTS.md`、`constitution.md`、`tests/e2e/README.md`）

- **全新构建**：任何 `agent-desktop` / CUA / headed 真窗口 E2E 之前，从**本 checkout** 执行 `just build-studio`（native + companions）+ 匹配的 renderer（`just studio` 热更新，或 `STUDIO_RENDERER_DIST=…` 的 rsbuild build/preview）。禁止复用未验证来源的窗口；构建与启动命令写进 OUTCOME。`scripts/e2e-fresh-build.mjs` 的 stamp 校验是现成门禁。
- **隔离与无焦点**：owned isolated application、单租户单窗口、`workers:1`、`fullyParallel:false`、固定 `locale`。`agent-desktop` 一律 headless 语义投递（`snapshot` / `find` / `click <ref>`）；禁用 `focus-window`、坐标、鼠标、裸 `press escape`；键盘走 `press --app … --window-id …`。禁止新增第二套 native driver，禁止用前景自动化替代后台 CUA。
- **真实验证**：断言用户可观察结果或磁盘 / IPC 事实，不断言 mock 占位符。renderer 桩**不证明** native window / guest / permission / companion 行为；原生用例须保留 observations、input receipts、artifacts、cleanup，空产物或身份未知即判失败。
- **门禁**：触及 disclosures / `MessageVirtualList` / scroll-position memory / `ModelSelector` 的改动，须 `just studio-contracts`（9 个 pinned 文件）全绿；落地前 `just studio-land-check`。两 target 相互独立，只跑后者不能证明契约 green。
- **证据**：行为变更在 `docs/evidence/<domain>/<date-slug>/OUTCOME.md` 记录 commands、counts、**Explicitly not claimed**；跳过 / 未执行不计通过，失败不得改名 skip，不得缩小分母凑数，失败按「本次引入 / 既有 / 环境」分类。
- **前端测试规范**：`apps/studio/docs/references/testing/frontend-testing.md` §1/§4/§5/§9 —— 写用例前先说清要防的回归；`getByRole` / `getByLabel` 优先，`data-ui` 用 `~=`；只 mock 最窄外部边界；E2E 只做跨边界关键流；等可观察条件而非固定 timeout；禁组件树快照。

---

## 1. 三层模型与统一 ID

旧稿把「场景」（`PLAN.md` 的 P0/P1/P2）与「页面 × 组件台账」（`PAGE_COMPONENT_E2E_PLAN.md`）分成两套编号，中间缺**逐功能**层，导致「1522 个组件被命中」不等于「某个功能真的能用」。本计划用三层 ID 建立可追溯关系：

| 层 | ID | 含义 | 判定单位 |
| --- | --- | --- | --- |
| 页面 | `P01`… | 一个可导航到的页面 / 独立窗口表面 | 页面能真实打开且 AX 树完整 |
| 功能 | `F01`… | 跨组件、用户可感知的一条能力 | 该功能至少一条**通过**的用例断言可观察结果 |
| 组件 | `C01`… | 一个可复用组件族（不是单个 DOM 节点） | 组件族内关键控件被稳定选择器命中 |
| 场景 | `SC-xx` | 跨页面 / 跨进程的端到端旅程 | 复用旧稿 `PLAN.md` 的 P0/P1/P2 编号，避免重复造号 |
| 用例 | `P01.F03.02` / `C07.04` / `SC-P0-04` | 具体到一条用例 | 通过 / 失败 / 跳过 / 未执行 四态之一 |

**台账文件**：`page-feature-component-inventory.json`（S0 生成，放在本 evidence 目录）。结构为 `pages[]`、`features[]`、`components[]`、`cases[]` 四张表；`components[]` 沿用 `ax-page-audit.mjs` 的 `{page, role, name, states, selector, coveredBy[]}`，另加 `featureIds[]`。`coveredBy[]` 由**通过的**用例按稳定选择器回填，跳过与未执行一律不回填。

三层关系：`P` 承载 `F`，`F` 由 `C` 实现，`C` 被 `cases` 命中。任一功能为「已覆盖」的必要条件是：其归属页面可达、至少一条用例通过、且该用例所属执行面足以证明该功能（§2）。**仅组件被命中不判功能通过**。

---

## 2. 执行面与「能证明什么」

| 面 | 现有入口 | 可证明 | 不可证明 |
| --- | --- | --- | --- |
| **L0 台账** | `scripts/ax-page-audit.mjs --out …`（真窗口 AX，headless 语义投递） | 页面可达性、组件 role/name/states、零覆盖清单 | 功能是否可用 |
| **L1 组件级** | Playwright，10 个 `playwright.*.config.ts`；`getByRole` + `data-ui` | 组件可访问名、状态切换、键盘可达、焦点返回 | native window / guest / permission / companion |
| **L2 功能级** | 既有 fixture：`support/taskConversation.ts` `installFixture`、`workbench-office/fixture.ts` `installWorkbench`、`settings/support.ts`、`task-turn-state/fixture.ts` | 单页内跨组件流程、渲染层状态机 | 跨进程持久化、真实 ACP / 模型 / 文件字节 |
| **L3 场景级（真宿主）** | `tests/e2e/interactive-coding/`（真 ACP + 真模型）、`tests/e2e/task-catalog/`（后台 CUA + input receipts）、`tests/e2e/office/`（`aster-test` journeys）、`tests/e2e/agent-desktop/core_navigation.py`、`tests/e2e/settings/cua_suite.py`、`connectors/nativeHarness.ts` | 真实窗口、真实 IPC / 磁盘 / 权限、跨进程、跨窗口、真实产物 | — |
| **契约层（门禁，不计覆盖）** | `just studio-contracts` → `scripts/perf-layout-contracts.json` 的 9 个 pinned vitest 文件 | disclosures 延迟挂载、虚拟列表 / 滚动记忆、单一 ModelSelector | 产品级 E2E 行为 |

**关键纪律**：338/339 个既有 Playwright 用例通过 `addInitScript` 拦截 DataApi / IPC 跑在内存桩上（旧稿 `PLAN.md` §1 盘点值，未经本轮复核）。因此 L1/L2 的结果**必须**在 OUTCOME 标注「renderer 桩」，只有 L3 能支撑发布级结论。真正能造宿主状态的现有 harness 是 `connectors/nativeHarness.ts`（`playwright.connectors.config.ts`，端口 4598 硬编码待改 env）。

---

## 3. 逐页面清单（P 层）

页面清单以 `src/renderer/utils/sidebar.ts` 的 `SIDEBAR_APP_DEFINITIONS`、`src/renderer/windows/builtinWorkbenchPlugins.ts` 的 `PAGES` 表、`src/renderer/routes/`、`src/renderer/windows/README.md` 为依据；「可见」列由 §0.1 第 5 条的隐藏集合推导，**最终可达性以 S0 的 L0 真窗口审计为准**。

### 3.1 Activity Bar 页面

| ID | 应用 / 路由 | 页面实现 | 可见 | 优先级 |
| --- | --- | --- | --- | --- |
| P01 | `agents` `/app/agents`（默认落地页） | `pages/agents/AgentPage.tsx`（WorkbenchHome / TaskWorkspace / SessionCard） | 是 | P0 |
| P02 | `schedule` `/app/schedule` | `pages/org/OrgPage.tsx`（initialSection: schedule）/ `SchedulePage.tsx` | 是 | P1 |
| P03 | `mail` `/app/mail` | `components/workInbox/InboxHome.tsx`（initialLane: mail）/ `MailPage.tsx` | 是 | P1 |
| P04 | `mini_app` `/app/mini-app`、`/app/mini-app/$appId` | `MiniAppsPage.tsx` / `MiniAppPage.tsx`（MiniAppTabsPool、`split_open` 分屏） | 是 | P1 |
| P05 | `knowledge` `/app/knowledge` | `pages/knowledge/KnowledgePage.tsx` | 是 | P0 |
| P06 | `files` `/app/files` | `pages/files/FilesPage.tsx` | 是 | P1 |
| P07 | `notes` `/app/notes` | `pages/notes/NotesPage.tsx` | 是 | P2 |
| P08 | `capabilities` `/app/capabilities` | `pages/capabilities/CapabilitiesPage.tsx`（ConnectorItemDialog / SkillUploadDialog / SkillUseDialog） | 是 | P0 |
| P09 | `documents` `/app/documents` | `PersonalLibraryPage.tsx`（aster）/ `DocumentsPage.tsx` | 是 | P1 |
| P10 | `messages` `/app/messages` | `MessagesPage.tsx` | aster 隐藏 | P2 条件 |
| P11 | `org` `/app/org` | `OrgPage.tsx` | aster 隐藏 | P2 条件 |
| P12 | `assistants` `/app/chat` | `pages/home/HomePage.tsx` | 全租户暂隐 | P2 条件 |
| P13 | `code_tools` `/app/code` | `CodeCliPage.tsx`（aster 下 `PAGES` 为 `null`） | aster 隐藏 | 不测，仅验证重定向 |
| P14 | `plugins` `/app/plugins` | 无（`PAGES.plugins = null`，重定向 `/app/agents`） | 全租户暂隐 | 仅验证重定向 |

### 3.2 会话与独立路由页面

| ID | 路由 | 说明 | 优先级 |
| --- | --- | --- | --- |
| P15 | `/app/agents?agentId=&sessionId=` | 任务会话页：消息流、工具披露、审批、引用、上下文用量 | **P0** |
| P16 | `/settings/*` overlay | Settings 不是普通 tab，是覆盖在当前工作页上的浮层；`settingsMenu.ts` 的 `settingsNav` / `settingsMenu` 为索引，约 8 组（personal / connections / agent / capabilities / automation / coding / system / developer）。含设置内搜索结果页 `settingsSearch/SearchResultsPage.tsx` 与 `?section/?panel/?focusId` 深链 | **P0** |
| P17 | `/app/launchpad` | `LaunchpadPage.tsx` 命令启动台 | P1 |
| P18 | `/app/browser?url=` | `BrowserPage.tsx`，webview 导航 | P2 |
| P19 | `/app/file-preview` | `FilePreviewPage.tsx` | P1 |
| P20 | `/app/release-notes` | `ReleaseNotesPage.tsx` | P2 |
| P21 | `/app/plugin` | 通用 `PluginRoute` / `parsePluginPageSearch` | P2 条件 |

> P16 的子页清单（general / appearance / shortcut / usage / provider / model / websearch / file-processing / data / archive / about / traces / dependencies / mcp 子页 / skills / scheduled-tasks / screenshot / ocr / browser / notifications / worktree / local-models / evolution / prompts / search 等）**不在此手写穷举**：由 S0 的 L0 审计按 `settingsMenu.ts` 导出实际可达子页，逐个分配 `P16.nn`。旧稿的 15 页 AX 基线只覆盖侧栏可达的 11 个设置子页，漏了这些，必须重测。

### 3.3 独立窗口表面（必须真窗口验证）

| ID | 窗口 | 依据 |
| --- | --- | --- |
| P30 | 主窗口 `main` | `src-tauri/src/lib.rs` `WebviewWindowBuilder`、`tauri.conf.json` `app.windows: []`（全部运行时创建） |
| P31 | `subWindow` detached tab | `windows/subWindow/SubWindowAppShell.tsx`、`AppShellTabBar.tsx` 的 `detachTab` |
| P32 | quickAssistant（Home / Chat / Translate） | `windows/quickAssistant/`、`quick_assistant.toggle` |
| P33 | screenshot / selection-action / selection-toolbar | `windows/screenshot/CaptureOverlay.tsx`、`windows/selection/*` |
| P34 | applet / aix-applet / file-editor / agent-wiki / mail-compose / notice | `src-tauri/src/{applet_window,aix_applet_window,file_editor_window,agent_wiki_window,mail_compose_window,notice_window}.rs` |
| P35 | `studio-embed` / `studio-notice` 工作区浏览器 | `src-tauri/src/workspace_browser.rs` |
| P36 | migrationV2 / userDataRelocation | `windows/migrationV2/`、`windows/userDataRelocation/` |

> 待确认：P32–P36 的 Rust 侧创建入口与 label 命名在 `lib.rs` 未显式列出（可能由 WindowManager 动态创建）。S0-B2 需定位后再写具体用例，不得凭命名猜测 label。

---

## 4. 逐功能矩阵（F 层，本计划新增）

每条功能绑定「主要页面」「实现线索」「必须断言的可观察结果」「最低执行面」「防的回归」。**最低执行面**是验收下限：标 L3 的功能用 L2 桩通过不得记为覆盖。

| ID | 功能 | 主要页面 | 实现线索（相对 `apps/studio/`） | 必须断言 | 面 | 优先级 |
| --- | --- | --- | --- | --- | --- | --- |
| F01 | 会话/草稿生命周期与导航保持 | P01/P15 | `components/chat/resourceList/*`、`utils/chat/sessionListHelpers.ts` | 草稿跨页面导航保留、tab 关闭不丢、空草稿不落盘、切 Tab 后滚动与状态保持（MemoryRouter + `<Activity>` KeepAlive） | L2 + L3 抽样 | P0 |
| F02 | Composer 输入 / 斜杠命令 / 附件拖入 / 队列 | P01/P15 | `components/composer/ComposerCore.tsx`、`tools/definitions/*`、`ComposerFileDropOverlay.tsx`、`QueuedFollowupsDock.tsx` | 中文输入法、Shift+Enter、粘贴 HTML、空白不落盘；每个协议命令的 `promptText` 与 UI 标签一致；拖入文件可见且可移除；附件面板开合不改草稿 | L2 | P0 |
| F03 | 流式消息 / 工具披露 / 引用 | P15 | `components/chat/messages/tools/shared/ToolDisclosure.tsx`、`toolRendererRegistry.tsx`、`blocks/ToolBlockGroup.tsx`、`chat/citations/` | `aria-busy` → `Stop` 出现 → 完成终态不重放；折叠内容延迟挂载、关闭卸载；12 次调用聚合、失败行单行；引用可跳转到源 | L2 + 契约门禁 | P0 |
| F04 | 消息虚拟列表与滚动记忆 | P15 | `list/MessageVirtualList.tsx`、`chatVirtualizerRuntime.tsx`、`useScrollPositionMemory.ts`、`MessageListSearch.tsx` | 长会话节点回收、切走再切回位置恢复、`scrollToKey` 命中、回到底部、搜索命中可跳转 | L2 + 契约门禁 | P0 |
| F05 | 模型选择整链 | P01/P15 | `components/ModelSelector/`、`WorkbenchModelPicker.tsx`、`ModelSpeedControl` | 唯一 ModelSelector 实例、搜索/标签过滤/键盘导航、选中模型在 paint 前置顶、思考强度变更与还原、切换后用量与发送链路一致 | L2 + 契约门禁 | P0 |
| F06 | 工具审批批准 / 拒绝 | P15 | `composer/variants/PermissionRequestComposer.tsx`、`WorkbenchPermissionPicker.tsx`、`AppletPermissionDialog.tsx` | 批准后结果真实回写、拒绝后状态明确且无成功假象、两次终态可区分；需 live native ACP | **L3** | P0 |
| F07 | 真实 ACP turn 与连接器/MCP 调用 | P08/P15 | `tests/e2e/connectors/nativeHarness.ts`、`interactive-coding/` | 真实回复与工具记录落盘、重启仍在；stdio/SSE/HTTP 成功 → 断开有错误态 → 重连可恢复；记录 input receipts | **L3** | P0 |
| F08 | 项目 / 工作区 / 侧栏重排固定 | P01 | `pages/agents/WorkbenchProject*.tsx`、`WorkbenchScopePicker.tsx`、`services/SidebarShortcutService.ts` | 新建/切换项目、范围筛选、必填校验、reorder / setPinned 后重启仍生效；侧栏收起后导航控件消失 | L2 + L3 抽样 | P1 |
| F09 | 多标签与窗口编排 | P01/P31 | `layout/{TabsProvider,TabRouter,AppShellTabBar}.tsx`、`useTabDrag.ts`、`tabDragRubberBand.ts` | Ctrl+Tab / 关闭 / 关最后一个关窗、拖拽重排、detach 到独立窗口、新窗口焦点与标题 | **L3** | P1 |
| F10 | 命令面板 / 全局搜索 / 设置搜索 / 深链 | P16/P17 | `GlobalSearchPopup.tsx`、`WorkbenchCommandPalette.tsx`、`settingsSearch/*`、`shared/utils/command/definitions.ts` | 全局搜索恢复/开会话、空结果态可清除、`?focusId` 滚动定位、快捷键重绑与冲突提示 | L2 | P1 |
| F11 | 偏好与跨进程持久化 | P16 | `shared/data/{preference,bootConfig,cache}/`、`src-tauri/src/{preferences,data_sqlite}.rs` | 偏好写入 IPC 回读、跨页即时生效、**退出进程后重启**从磁盘恢复（非 localStorage 假象） | **L3** | P0 |
| F12 | 知识库 / 文件 / 文档真实字节产物 | P05/P06/P09/P19 | `pages/knowledge/`、`pages/files/`、`documentTools/*`、`src-tauri/src/document_result_handoff.rs` | 创建→索引→重启→召回+引用；DOCX/XLSX/PPTX/PDF/PNG 校验真实字节与内联预览，不是文件名 | **L3** | P1 |
| F13 | 首次运行 / 运行时组件 / 系统权限 | P30 | `layout/{FirstRunRuntimeComponentsDialog,FirstRunPermissionsDialog}.tsx`、`src-tauri/src/{runtime_components,macos_permissions}.rs` | 插件检查→安装→退出对话框→权限设置→主页可用；真实匿名安装 3 个 runtime 组件、摘要一致 | **L3** | P0 |
| F14 | 更新 / 退出守卫 / 单实例 | P20/P30 | `components/UpdateFab.tsx`、`src-tauri/src/{app_updater,main_exit,single_instance}.rs`、`MainExitGuard.tsx` | 真实更新通道与清单、DB 迁移后旧数据可读、二次启动激活已运行窗口后退出、退出守卫保留进行中任务 | **L3** | P0 |
| F15 | 截图 / 录屏 / 视觉标注 | P33 | `src-tauri/src/{screenshot,visual_feedback,capture}/` | 每屏一个 pooled 截图窗口、置顶 pin、导出/保存的真实文件、脱敏生效 | **L3** | P2 |
| F16 | 错误态 / 离线 / 崩溃恢复 / 无障碍 | 全页 | `RouteErrorFallback.tsx`、`frontend-testing.md §4` | 空/加载/拒绝/离线/重试成功文案完整且固定 locale；杀进程重启后草稿保留、进行中 turn 有明确终态；键盘可达、焦点返回、name/role/value 完整、reduce-motion 终态一致 | L2 + L3 抽样 | P1 |
| F17 | 外观矩阵 | 全页 | `packages/ui/`、`AppShell` 主题 | 深浅色/高对比、100%/125%/200% 缩放、窄窗口无重叠溢出、chromium + webkit 关键流一致 | L2 | P2 |

> **F06 / F07 / F09 / F11 / F12 / F13 / F14 / F15 是旧稿标记的 0-spec 或仅桩覆盖区**，也是「名义 E2E ≠ 真实 E2E」的主要缺口。这些必须落在真宿主上，且每条给出本次构建 hash、启动命令、真实产物与 cleanup。

---

## 5. 逐组件台账（C 层）

按**组件族**出断言，不为上千个控件各写一条用例；族内控件由 L0 inventory 逐个核对零覆盖。

| ID | 组件族 | 关键路径（相对 `apps/studio/src/renderer/`） | 组件级断言要点 |
| --- | --- | --- | --- |
| C01 | AppShell / TabBar / 窗口框 | `components/layout/{AppShell,AppShellTabBar,TabRouter,TabsProvider,WindowResizeHandles,trafficLights}.tsx` | tab `aria-current`、拖拽区、resize 手柄、overlay 层级 |
| C02 | Sidebar / 快捷方式 | `components/Sidebar/SidebarList.tsx`、`services/SidebarShortcutService.ts` | 每项可访问名唯一、pin 状态、收起后控件消失 |
| C03 | 命令与搜索 | `components/command/*`、`GlobalSearch/GlobalSearchPopup.tsx`、`QuickPanel/` | 可访问名、键盘导航、焦点返回 |
| C04 | Composer 核心 | `components/composer/{ComposerCore,ConversationComposerStage,ComposerToolRuntime,QueuedFollowupsDock,ComposerFileDropOverlay}.tsx` | 输入态、工具条开关、队列、拖放层 |
| C05 | Composer 工具定义 | `components/composer/tools/definitions/` 下 11 个 `*Tool.tsx`、`toolbarManifests.tsx` | 每项触发后可观察面板/标签变化，不改草稿 |
| **C06** | **ModelSelector（契约）** | `components/ModelSelector/`、`WorkbenchModelPicker.tsx`、`ModelSpeedControl` | **绑定 pinned 契约**：单一实例、虚拟列表唯一纵向滚动、过滤单行横向、选中前置顶 |
| **C07** | **消息虚拟列表（契约）** | `components/chat/messages/list/{MessageVirtualList,chatVirtualizerRuntime,useScrollPositionMemory,MessageListSearch,SelectionBox}.tsx` | **绑定 pinned 契约**：`scrollTo*` handle、位置恢复、多选 |
| **C08** | **Disclosures（契约）** | `components/chat/primitives/Disclosure.tsx`、`messages/tools/shared/ToolDisclosure.tsx`、`useMessageDisclosureState.ts`、`blocks/__tests__/ThinkingBlock.test.tsx` | **绑定 pinned 契约**：折叠不挂载、展开挂载、关闭再卸载；跨虚拟列表回收保留展开态 |
| C09 | 资源列表 | `components/chat/resourceList/{Agent,Assistant,Session,Topic}ResourceList.tsx`、`base/ResourceList*.tsx` | 分组、重排、workdir 分组、drop payload 归一 |
| C10 | 工作区与会话卡 | `pages/agents/{WorkbenchProject*,WorkbenchScopePicker,WorkbenchSession*,WorkspaceWikiCatalog}.tsx` | 行名唯一、状态筛选、菜单命令 |
| C11 | 引用 / 上下文用量 | `components/chat/citations/`、`chat/contextUsage/`、`AgentCitationsPanel.tsx` | 图例占比之和≈总量、无明细态明确提示、进度条画实测值 |
| C12 | 权限姿态 | `composer/variants/PermissionRequestComposer.tsx`、`WorkbenchPermissionPicker.tsx`、`AppletPermissionDialog.tsx` | 全姿势互达、Escape、缩放、reload 保持 |
| C13 | 设置组件 | `components/settingsMenu.ts`、`pages/settings/settingsSearch/*`、`ShortcutSettings.tsx` | 每行 `aria-label` 带命令名、focus 深链、录制冲突提示 |
| C14 | 虚拟列表原语 | `components/VirtualList/{Dynamic,Grouped,GroupedSortable}VirtualList.tsx` | 回收、排序、节点数累加 |
| C15 | 编辑器 / 预览 | `RichEditor/`、`CodeBlockView/`、`FilePreview/`、`FileTree/` | 粘贴、图片上传、预览字节 |
| C16 | 账户 / 首启 / 更新 | `components/account/*`、`Oauth/OauthButton.tsx`、`layout/FirstRun*.tsx`、`UpdateFab.tsx`、`MainExitGuard.tsx` | 对话框可访问名、取消路径、版本信息 |
| C17 | 收件箱 / 小程序 | `components/workInbox/`、`MiniApp/`、`MiniAppTabsPool` | 泳道、分屏、tab 池 |
| C18 | 产物与 Office | `pages/agents/{WorkbenchApplets,WorkbenchAppletWindow,PlanEditorWindow,WorkbenchFileEditor}.tsx`、`documentTools/*`、`paperloom/*` | 打开/关闭/恢复、下载字节 |
| C19 | 设计系统原语 | `packages/ui/`（button / switch / combobox / radiobutton / textfield） | 禁止原生 `<select>`；同名控件带行上下文；状态位可由 AX 读取 |
| C20 | 插件扩展点 | `components/plugins/{ExtensionPointHost,PluginWorkbenchLayout,PluginSidebarRegistryProvider,AixWorkbenchRuntime}.tsx` | 扩展点挂载、侧栏注册、运行时代理 |

**契约绑定（C06/C07/C08）**：`apps/studio/scripts/perf-layout-contracts.json` pin 了 9 个 vitest 文件，由 `just studio-contracts`（父仓库）→ `scripts/studio_land_guard.py contracts` 执行；等价入口 `node scripts/run-perf-layout-contracts.mjs`。修改这 9 个路径须同步更新 manifest。契约层是**门禁**，不计入三层覆盖率分子。

---

## 6. 三层交叉矩阵

下表是「逐页面 × 逐功能 × 逐组件」的实质绑定。每个单元格给出该页面必须执行的功能 ID 与组件族 ID；场景级用例（SC）单独列出。实现时按 `Pxx.Fyy.nn` 编号并入既有 spec 目录，**不新建套件**。

| 页面 | 优先级 | 必须覆盖的功能 | 必须命中的组件族 | 场景级用例（复用 `PLAN.md`） |
| --- | --- | --- | --- | --- |
| P01 工作台 | P0 | F01 F02 F05 F08 F10 F16 F17 | C01 C02 C03 C04 C05 C06 C09 C10 C13 C19 | SC-P0-01/04/06/09、SC-P1-01/04/15、SC-P2-02/03 |
| P15 任务会话 | **P0** | F01 F02 F03 F04 F05 F06 F07 F16 F17 | C04 C05 C06 **C07 C08** C11 C12 C14 C18 C19 | SC-P0-04/05、SC-P1-06/07/13、SC-P2-01 |
| P16 Settings overlay | **P0** | F10 F11 F16 F17 | C01 C13 C19 | SC-P0-06/07/08、SC-P1-08/09/10、SC-P2-04 |
| P05 知识库 | P0 | F12 F16 | C09 C14 C15 C19 C20 | SC-P1-11 |
| P08 能力/连接器 | P0 | F07 F16 | C02 C19 C20 | SC-P0-12 |
| P30 主窗口 | P0 | F13 F14 F16 | C01 C16 C19 | SC-P0-01/02/03/07/08/09/13 |
| P06 文件 | P1 | F12 F16 | C09 C14 C15 C19 | SC-P1-12 |
| P09 文档/资料库 | P1 | F12 F16 | C09 C15 C18 C19 | SC-P1-12 |
| P19 文件预览 | P1 | F12 | C15 C18 | SC-P1-12 |
| P17 Launchpad | P1 | F10 | C03 C19 | — |
| P02 日程 / P03 邮件 | P1 | F01 F16 | C09 C17 C19 | — |
| P04 小程序 | P1 | F09 F16 | C17 C19 | — |
| P31 detached tab | P1 | F09 | C01 | — |
| P33 截图 / 划词 | P2 | F15 | C01 C19 | — |
| P32/P34/P35/P36 其它窗口 | P2 | F09 F14 F15 | C01 C16 | — |
| P07/P10–P14/P18/P20/P21 | P2 条件 | F16（+ 重定向验证） | C02 C19 | — |

> 旧稿 `PAGE_COMPONENT_E2E_PLAN.md` 的 15 页 / 1522 组件矩阵只对应上表的 P01、P08、P16 三个子集与部分子页；**P15 会话页、P30 主窗口、全部独立窗口、隐藏路由的条件可达性均不在旧基线内**。这是本计划相对旧稿最主要的覆盖增量。

---

## 7. 批次与执行顺序

| 批次 | 范围 | 依赖 | 出场条件 |
| --- | --- | --- | --- |
| **S0 口径与基建校准** | ① 修 `ci.yml:178` 的 `test:contracts:perf-layout` 漂移并验证 CI 契约门禁；② 用 `ax-page-audit.mjs --out` 在**全新构建**的真窗口重导页面清单（含 P16 全部可达子页、P30+ 窗口 surface），得到新基线总数；③ 打通「通过用例 → `coveredBy`」回填；④ 按单租户重算 `e2e-coverage-gate.mjs` 的 `DECLARED_TESTS`（当前 483），并核对 `BASELINE_COMPONENTS`（1522，仅在未提供 inventory 时占位）与新基线一致，口径变更留前后数字；⑤ 定位 P32–P36 的 Rust 窗口 label；⑥ 确认 `e2e-fresh-build.mjs` 硬编码路径与 `just build-studio` 的 `CARGO_TARGET_DIR` 一致；⑦ 逐条处置 skip / fixme（含旧稿的 15 处），写明解除条件 | 无 | 基线可复现、`Gate B measurable=true`、0 个无理由永久跳过、`test:e2e:all` 能编排全部 config、CI 契约门禁实际通过 |
| **S1 P0 真宿主** | P01/P15/P16/P30 的 F01–F07、F11、F13、F14；复用 `interactive-coding/`、`task-catalog/`、`office/`、`agent-desktop/core_navigation.py` | S0 | 每条给出构建 hash、启动命令、真实产物、input receipts、cleanup；renderer 桩结果单独标注 |
| **S2 逐功能覆盖** | 上表全部 F 的 L2 部分（P1/P2 页面），沿用既有 fixture 与 config，固定 locale | S0、S1 的 P0 关键流 | 每个 `Pxx.Fyy` 至少一条通过用例；跨引擎（chromium + webkit）关键流一致 |
| **S3 逐组件补盲** | 按 C01–C20 出组件组断言，优先 C06/C07/C08 契约组件与设置大页；跑 L0 零覆盖清单 | S0 | 每个组件族无未说明的零覆盖行；`just studio-contracts` 全绿 |
| **S4 宿主与窗口** | P30–P36 全部窗口、F09/F15、原生菜单、系统权限、单实例、真实文件对话框、跨窗口焦点与剪贴板 | S1 | 真窗口 AX 完整、observations 齐全；不用 `focus-window` / 坐标 / 鼠标 |
| **S5 回归固化与验收** | 把本轮与旧会话修复项写成「修复前会失败」的最小断言；跑全量矩阵与 §9 验收 | S1–S4 | 三层验收通过、OUTCOME 完整、`Explicitly not claimed` 齐备 |

**运行节奏建议**（沿用旧稿 §7，属建议非既成事实）：PR / 日常跑 S2 快集 + 触及契约组件时的 `just studio-contracts`；夜间指定 runner 跑 S1/S4 真宿主；发布前跑 S1–S5 全量并写发布 OUTCOME。**当前 9 个 CI workflow 均不调用 Playwright / CUA / `agent-desktop`**，全部 E2E 仍只能本地跑；把 S2 快集接入 CI 需另行评估私有子模块 `STUDIO_READ_TOKEN` 与 macOS runner 依赖。

---

## 8. 环境、数据与已知阻塞

**构建与启动（写进每批 OUTCOME 的原文）**：
```
just build-studio aster
STUDIO_RENDERER_DIST=<当前树构建产物> STUDIO_RENDERER_PORT=4318 node apps/studio/scripts/e2e-preview.mjs
node apps/studio/scripts/e2e-fresh-build.mjs --build / --verify / --print
```
`just studio` 热更新或显式 `STUDIO_RENDERER_DIST=…` 的 rsbuild build/preview 均可，但必须与 native host 来自同一 checkout。**不得打开或复用来源未验证的已有窗口**。

**隔离**：owned isolated application（旧稿用 `interactive-coding-e2e-20261101` + 独立 renderer port）、单租户单 data dir、`workers:1`、`fullyParallel:false`、`locale:'zh-CN'`；`ASTER_TEST_PID` / `ASTER_TEST_CUA_LOG` 用于后台投递回执，缺回执即拒绝。端口 4598（connectors）应改为 env 可覆盖。

**可复用数据与 mock**：`tests/e2e/settings/fixtures/page-baseline.json`、`interactive-coding/fixture/`（seed / campaign-inputs / frozen-campaign）、`task-catalog/catalog.json`、`tests/fixtures/{office,terminal}/`、根 `tests/e2e/{json_render_model,knowledge_model,context_compaction_model,network_boundaries}.py`（127.0.0.1 冻结模型）。凭据经现有 provider 解析，**不复制 account / session 文件、不记录 auth token / private token**（`interactive-coding/README.md`、`task-management/native-server.mjs` 明文）。

**已知阻塞（必须在 OUTCOME 如实记录，不得绕过伪装通过）**：
1. `docs/evidence/studio/2026-11-09-agent-desktop-e2e/OUTCOME.md` 记录真窗口被 `FirstRunRuntimeComponentsDialog` 的 `Runtime plugin pin identity mismatch` 挡住、侧栏 0 matches，导致逐页 AX 场景全部无法执行；当前用 `STUDIO_E2E_STUB_RUNTIME=1` 或 `/tmp` 桩绕过，**产品根因未修**。S0 的真窗口重测以此为第一阻塞项。
2. 旧稿记录 `just build-studio` 曾被 SwiftPM `sandbox-exec: sandbox_apply: Operation not permitted` 挡住；本轮未验证当前环境是否仍受阻，S0 需先跑一次并记录实际结果。
3. `agent-desktop` / `aster-test` / `agent-browser` 是外部 CLI，版本与 macOS TCC 权限不由仓库控制；多份 OUTCOME 记录了 `screenshot --app` / `press --app --headed` 在此环境失败。
4. `--headed` 口径有张力：`AGENTS.md:83` 把 headed live-window 场景纳入 fresh-build 要求，而 `PAGE_COMPONENT_E2E_PLAN.md:40` 与 `COVERAGE_UPLIFT_PLAN.md:139` 禁止 `--headed`；`nav01_work.py` 的键盘却用 `press --headed`。**本计划采用**：窗口发现/点击一律 headless 语义投递；确需键盘事件投递时仅用 `press --app … --window-id …` 并在 OUTCOME 标注，禁止 `focus-window` 与坐标。S0 需统一这一口径。

---

## 9. 验收标准与证据

不预设「≥95%」这类绑定作废分母的数字。**分母在 S0 重测后写入台账并冻结**；此后口径变更必须在 OUTCOME 写明前后数字。三层各自判定：

1. **P 层**：L0 审计导出的每个「默认可达」页面都能真实打开且 AX 树完整；条件可达页面给出隐藏/重定向断言；独立窗口给出创建、焦点、关闭、恢复四态。
2. **F 层**：每条 `Pxx.Fyy` 至少一条**通过**的用例，且执行面 ≥ 该功能的「最低执行面」；标 L3 的功能用桩通过不计覆盖。
3. **C 层**：`components[]` 中 `coveredBy.length > 0` 的比例 ≥ 目标值（由 S0 冻结）；零覆盖组件逐条列 `role + name + selector + reason`；C06/C07/C08 的 `just studio-contracts` 9 文件全绿。
4. **四态记账**：每批 OUTCOME 给出通过 / 失败 / 跳过 / 未执行四元组；跳过与未执行不计通过，失败按「本次引入 / 既有 / 环境」分类。
5. **真宿主证据**：每条 L3 用例附构建 hash、启动命令、窗口来源、input receipts、artifacts、cleanup；空产物、身份未知、终态缺失判失败。
6. **门禁**：`just studio-contracts`、`just studio-land-check`、S0 修复后的 CI 契约步骤，以及各 `pnpm playwright test --config …` 的**实际结果**（不得声称未跑的检查通过）。
7. **OUTCOME 必须含 `Explicitly not claimed`**，至少覆盖：不声称 renderer 桩证明 native 行为；不声称零覆盖已消除；不声称发布就绪；不声称本计划中的用例已运行或通过。

**本目录当前只有 `PLAN.md`**。按 `constitution.md` 的「Planning and acceptance are not two copies of the same record」，执行后每批次另建 `docs/evidence/studio/<date>-<slug>/OUTCOME.md` 记录实际结果，不回填改写本计划。

---

## 10. 明确不做 / 本轮未宣称

- 不新增测试框架、不新建第二套 native driver、不新建 spec 目录；复用 10 个 Playwright config、`e2e-runner.mjs`、`e2e-fresh-build.mjs`、`ax-page-audit.mjs`、`e2e-coverage-gate.mjs` 与既有 fixture / campaign。
- 不修改 `apps/studio` 子模块内的三份旧稿（避免脏树违反 land discipline），不在本轮改任何门禁脚本、config 或测试代码。
- 不做组件树快照、整页截图基线、坐标点击；不把 `regression/` 的 21-case 迁移清单、`test.skip(true)`、未编排 harness 计为覆盖。
- 不把 1522 / 1446 / 483 / 89.4% 等旧基线数字作为验收依据；不沿用双租户用例。
- **本轮未运行任何用例、未构建桌面应用、未验证任何门禁通过**；文中「本轮核对结果」仅指对源码与配置文件的只读检查。所有执行结果留待各批次 OUTCOME 记录。
