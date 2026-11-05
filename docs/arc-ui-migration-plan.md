# Aster 当前用户可达界面的 Arc UI 迁移计划

日期：2026-11-01。状态：方案已持久化；尚未开始本计划的组件替换与验收。执行进度和接续入口见第 11 节。

目标：逐项覆盖当前 Aster 用户可达的界面，使适合采用 Arc UI 的控件和组合尽量采用其实现，并收敛原实现。完成条件包括功能、可访问性、性能、原生交互和旧代码删除；安装数量、改名数量和截图数量都不是完成条件。

范围按当前本地 Aster 产品确定：桌面 `aster` tenant、它实际承载的 Office／插件／预览界面，以及 CLI／TUI 的一致性边界。其他 tenant、云管理后台、独立文档站和仓库中没有产品入口的演示代码不自动成为迁移范围。被本地入口实际使用的跨包代码仍在范围内。共享 UI 的其他消费者需要兼容回归，但不因此展开后台重设计。

本文件是跨批次路线图，沿用 [总体机制路线图](xerj-replacement.md) 的记录方式。每个实现批次按 [SDD 流程](spec-driven-development.md) 形成有边界的 spec／plan／tasks，验收留在 `docs/evidence/`；不把本文件当成实现通过的证据。

## 1. 已核实的前提

| 事实 | 对计划的影响 | 来源 |
| --- | --- | --- |
| Arc 指 `uiarc.dev` 的 React 源码组件库 | 在现有 React 实现内采用源码，保留 Aster 的统一组件入口 | [官方安装说明](https://uiarc.dev/docs/installation) |
| Aster 已使用 React、Motion、Radix 和共享 UI 包 | 不先引入另一套运行时、主题系统或包管理流程 | [UI 包](../packages/ui/package.json)、[Studio 包](../apps/studio/package.json) |
| DESIGN.md 已吸收部分 Arc 交互规则 | 先识别已有能力，再替换不足；不能把已有改动重复记为本次成果 | [设计约定](../DESIGN.md)、[历史研究与结果](evidence/local-model-control/2026-10-28/OUTCOME.md) |
| 2026-10-28 的本地 Arc 目录有 371 项；官网当前展示 237 个组件和 138 个 blocks | 上游目录与 Aster 界面是两个分母；实施时冻结所用源码身份，不能拿上游目录评审数声称 Aster 覆盖完成 | [本地目录](evidence/local-model-control/2026-10-28/arc-ui-research/catalog.json)、[官网](https://uiarc.dev/)、[当前目录](https://uiarc.dev/r/catalog.json) |
| 当前 Aster 跳过 `OnboardingPage`，默认进入工作页 | 现有首次模型引导不能作为当前可达页面或首批试点；将来恢复入口需单列行为变更 | [MainApp](../apps/studio/src/renderer/windows/main/MainApp.tsx) |
| 源码中存在多种 renderer window，但当前 Rsbuild 入口指向 main | 源码存在不证明发行包可达；辅助窗口逐一验证构建产物、宿主创建路径和权限 | [窗口说明](../apps/studio/src/renderer/windows/README.md)、[构建入口](../apps/studio/rsbuild.config.ts) |
| main 入口按查询参数启动文件编辑器、Wiki、applet、通知、邮件撰写和视觉贴图等窗口 | 路由扫描之外还必须检查这些独立根；同一入口文件不等于同一运行上下文 | [main 入口](../apps/studio/src/renderer/windows/main/entryPoint.tsx) |
| Office 有自己的控件、工具栏、焦点与 portal 实现 | 仅替换 Studio 外框不能宣称覆盖 Office；其可达控件也要逐项处理 | [Office primitives](../packages/office/src/internal/design-system/primitives/index.ts)、[Office Dialog](../packages/office/src/internal/design-system/primitives/dialog/dialog.tsx) |

当前目录的 `generatedAt` 为 `2026-10-31T13:39:48.671Z`，由官方目录页面读取。此次没有冻结完整新版目录；直接下载曾返回 HTTP 403，因此不声明已完成新旧目录逐项差异审查。已有研究继续保留原身份，不覆盖历史文件。

## 2. 替换的架构和边界

```text
Aster 页面、窗口和业务组件
        │ 原有 props / ref / 事件 / data-ui
        ▼
@aster-ai-lab/ui 现有 primitives / composites / console 入口
        │ 在现有 owner 内采用并适配 Arc 源码
        ▼
Aster tokens、主题、portal、焦点、Motion/CSS

业务状态、配置、模型调用、ACP、权限和编辑器引擎继续由原 owner 提供
```

1. **在原 owner 内替换。** 通用按钮仍由现有 button 实现拥有。`Button.tsx` 这类纯 re-export 不算第二套按钮。确有职责不同的组合保留有意义的入口，不为目录整齐而合并业务。
2. **逐项允许实质替换。** 功能匹配时采用 Arc 的真实组件源码；需要保留 Aster 能力时在现有实现内适配。避免只改名称、颜色便宣称采用，也避免在每页新建 `ArcButton`、`NewDialog` 等并行基础实现。
3. **状态只有一个 owner。** 保存、复制、停止、导入、安装和审批使用原操作的真实 Promise／事件。控件展示状态，不另建模型服务、权限状态机、剪贴板服务或假运行进度。
4. **保留功能引擎，覆盖可见控件。** Office、Monaco／CodeMirror、终端、流式 Markdown、虚拟列表和图分析继续由原引擎承担功能。其工具栏、选择器、菜单、弹层、反馈、列表外框仍需迁移决策，不能以“复杂组件”整体豁免。
5. **源码权属不变。** Studio 修改落在私有 `apps/studio` 子模块；父仓库仅跟踪 gitlink。根 `packages/ui`、`packages/office` 等修改在各自 owner；不迁出仓库、不创建新工作树，不手改 dist 或生成文件。
6. **控制共享影响面。** 更换共享组件前查完导出和所有消费者。React 库不依赖桌面 shell；平台操作由调用者传入现有边界。

### 五种迁移决策

| 决策 | 使用条件 | 完成证据 |
| --- | --- | --- |
| 采用 Arc 源码 | API／能力匹配，来源和许可明确 | 实际源码、适配 diff、行为用例、原实现删除 |
| 适配 Arc 源码 | 存在可说明的 Aster 能力差异 | 差异列表逐条通过；原导出继续只有一个实现 owner |
| 使用共享 Arc 控件重组 | 页面／业务组件没有合适的整块替代 | 子控件完成迁移，业务数据与操作真实，旧重复 markup／CSS 删除 |
| 保留功能内核 | 文档排版、表格计算、编辑、终端、虚拟化等引擎 | 可见子控件已分别决策，内核保留原因和兼容用例明确 |
| 保留原生或外部边界 | TUI、系统控件、第三方网页／插件自有 DOM | 宿主可控外层完成决策；不可替换范围、理由和验证明确 |

“暂时没改”不等于保留决策；许可未确认、入口未证实和能力未核对均保留为未完成项。

## 3. 全量可达性清单的建立方式

### 3.1 分母与记录

先冻结当前父仓库、Studio 子模块、相关工作区未提交源码和构建产物的身份。当前 checkout 有并行改动，不能只记录 HEAD 就声称对应实际界面。

建立两张关联清单，放在首批的 `specs/<该批次>/contracts/`：

- **界面清单**：每个真实入口 × 运行条件 × 可交互区域 × 有意义的状态。记录菜单／命令／宿主入口、tenant、平台、所需资源或许可、直接组件、弹层和可观察操作。状态包括正常、加载、空、失败、未知、无权限、离线、取消、只读、过期及脏数据关闭；只对实际可能出现的状态建记录。
- **组件清单**：定义路径、导出、真实调用者、变体、Arc 对应项及其来源／许可、迁移决策、批次、原实现删除条件、测试与原生证据。共享组件定义去重，实例和状态覆盖单独计算。

清单是开发记录，不是新增产品配置。路径和组件身份来自源码和运行时注册，不按界面文字或组件名字猜业务含义。复用现有 `data-ui` AST／查询机制建立源码关联，不新建第二套运行时组件注册中心，也不通过自动生成大量标识追踪用户内容。

记录状态使用固定集合：`待核对`、`已决策`、`实现中`、`待验收`、`已验收`、`有证据的范围外`。范围外必须有理由和入口证明，不能用改分母消除遗漏。

### 3.2 必须追踪的入口

| 入口来源 | 检查内容 | 现有 owner |
| --- | --- | --- |
| 当前构建与宿主 | 发行资源、初始化页面、窗口创建、平台分支、capability；确认代码实际会被加载 | [Rsbuild](../apps/studio/rsbuild.config.ts)、[宿主](../apps/studio/src-tauri/src/lib.rs)、[capabilities](../apps/studio/src-tauri/capabilities) |
| 路由与重定向 | 路由、search 参数、父路由 guard、设置导航过滤、错误页；隐藏菜单不自动等于不可达 | [routes](../apps/studio/src/renderer/routes)、[侧栏](../apps/studio/src/renderer/utils/sidebar.ts)、[设置导航](../apps/studio/src/renderer/components/settingsMenu.ts) |
| 工作台与扩展注册 | 内建页面、资源入口、工作区子页、插件贡献、安装后新增内容 | [builtins](../apps/studio/src/renderer/windows/builtinWorkbenchPlugins.ts)、[插件宿主](../apps/studio/src/renderer/components/plugins) |
| 命令与上下文动作 | 快捷键、右键、命令面板、文件拖入、链接跳转、通知、系统菜单／托盘触发的界面 | [命令定义](../apps/studio/src/shared/utils/command/definitions.ts)、[命令菜单](../apps/studio/src/shared/utils/command/menus.ts)、[会话动作](../apps/studio/src/renderer/components/chat/actions/actionRegistry.ts) |
| 独立窗口 | main 的分支根、分离窗口、applet、编辑器、Wiki、通知、视觉贴图；检查窗口内各自 providers 和弹层 | [入口分支](../apps/studio/src/renderer/windows/main/entryPoint.tsx)、[windows](../apps/studio/src/renderer/windows) |
| 消息和预览分派 | 每一种实际消息／工具／产物、扩展名插件、折叠详情、失败分支 | [任务消息](../apps/studio/src/renderer/pages/agents/taskConversation/TaskConversationMessage.tsx)、[聊天工具](../apps/studio/src/renderer/components/chat/messages/tools)、[预览注册](../apps/studio/src/renderer/components/FilePreview/filePreviewRegistry.ts) |
| 全局服务与异常 | 程序式 popup、confirm、toast、更新、退出保护、启动失败、路由／窗口异常 | [PopupHost](../apps/studio/src/renderer/components/PopupHost/PopupHost.tsx)、[ToastHost](../apps/studio/src/renderer/components/ToastHost.tsx)、[窗口异常](../apps/studio/src/renderer/components/WindowFatalFallback.tsx) |
| 跨包和隔离 DOM | Office、实际引入的 form／flow、第一方 applet、iframe／Shadow DOM；区分宿主与客体控制权 | [Office](../packages/office/src/internal)、[form](../packages/form/src/react)、[flow](../crates/flow/packages/ui/src/react)、[别名来源](../apps/studio/scripts/flowDesignerAlias.ts) |
| CLI／TUI | 首次配置、账号与模型选择、会话、工具审批、错误、帮助等终端入口 | [CLI](../apps/aster)、[pager](../crates/terminal/crates/codegen/aster-pager) |

静态追踪补齐 lazy import、动态注册和 portal；随后用真实 Aster 构建走入口并核对 DOM／窗口。静态可达、原生实际到达、状态已验收分列记录。无法到达的旧窗口先查清构建和宿主原因，不为迁移恢复退休宿主。

### 3.3 当前已读到的路由边界

以下是清单种子，表示已核对的源码边界，不表示所有入口已做原生操作验收。

| 当前边界 | 源码结论与处理 |
| --- | --- |
| `/app/agents` | 当前 Aster 默认入口；工作区、任务、会话、资源面板都要继续向下追踪 |
| `/app/code`、`/app/files`、`/app/notes`、`/app/plugins` | 保留在候选可达主流程；逐项记录侧栏、命令或链接入口与详情状态 |
| `/app/plugin`、`/app/mini-app/`、`/app/mini-app/$appId` | 插件／mini app 的条件入口；安装／激活／缺失／拒绝均需覆盖，客体 DOM 单列 |
| `/app/browser`、`/app/file-preview`、`/app/documents`、`/app/capabilities`、`/app/launchpad`、`/app/release-notes` | 有源码入口，不能只因不在 rail 而排除；继续证明命令、资源、预览、菜单或通知的入口链 |
| `/app/chat`、`/app/messages`、`/app/mail`、`/app/org`、`/app/schedule` | 当前 Aster 的父路由／侧栏规则会重定向这些目的地；同名能力若由其他区域承载，按其真实入口追踪，不把原路由误记为已迁移 |
| `/app/knowledge` | 路由重定向到工作页；实际工作区 Wiki／Analysis 仍在范围内，不能随旧路由一起排除 |
| `/settings/general`、`appearance`、`shortcut`、`usage` | 当前 Aster 设置导航包含；通用项、全部分节和搜索定位入口分别追踪 |
| `/settings/provider`、`model`、`websearch`、`file-processing`、`api-gateway` | 当前 Aster 设置导航包含；首批业务试点优先选 provider／model |
| `/settings/data`、`archive`、`about`、`search` | 当前包含／允许；数据页所有 panel、导入导出／备份／恢复弹窗、通知与法律窗口都要下钻 |
| `/settings/channels`、`traces`、`dependencies` | 当前 Aster 设置导航显式过滤，SettingsPage 对无匹配项会回到 general；对应运行组件若从插件页等另有入口，仍需迁移 |
| 其他 settings 路由 | `browser`、`code-review`、`device-connections`、`evolution`、`local-models`、`mcp` 及全部子路由、`notifications`、`ocr`、`prompts`、`quick-assistant`、`scheduled-tasks` 及详情、`screenshot`、`selection-assistant`、`skills` 及详情、`worktree` 均先核对 SettingsPage 的导航 guard 与实际替代入口，不能直接按文件计为当前可达 |
| settings 的纯重定向 | `/settings/`、`/settings/system`、`/settings/code-execution` 只记路由行为；不创建不存在的独立页面迁移任务 |
| 首次 provider onboarding | 当前 Aster 不挂载；恢复引导及快捷账号勾选属于另一个明确功能批次，未来可复用本次控件，不混算覆盖 |

## 4. 必须纳入的组件族与替换方式

Arc 对应名称基于保留的官方目录和已读文档，均是候选映射，不代表已引入。实施前核对每项当前源码、tier、依赖和许可。表中 Pro 候选不能直接放入 MIT 的共享组件导出。

| 组件族／用户区域 | Arc 候选与迁移方式 | 必须保留／验证 |
| --- | --- | --- |
| 普通、图标、链接、分组、复制、异步按钮 | button、copy-button、action-button；改现有 button／复制实现；高密度行用轻量反馈 | ref／asChild／href／router、disabled／loading、快捷键、稳定命中区域、错误恢复；不每字创建 Motion 节点 |
| 输入、密码、搜索、数字、标签、快捷键录入 | input、textarea、password-field、search-field、number-field、tag-input、shortcut-recorder | 中文 IME、粘贴、撤销、受控值、错误关联、脱敏、不吞热键；密钥不进日志或清单 |
| 单选／多选／模型／资源／树选择器 | select、combobox、multi-select、tree-view；在 SearchSelect、combobox、实体选择器 owner 中适配 | 搜索、分组、图标、meta、disabled 原因、清除、多选、远程／大列表、portal、键盘；有功能差异不得降级 |
| 开关、复选、单选、分段、滑块 | switch、checkbox、radio-group、segmented-control、slider | 持久化时机、disabled、roving focus、RTL、Home／End、实时系统减弱动效；复用已有高亮能力 |
| 日期、时间、范围、颜色、emoji | calendar、date-picker、date-range-picker、time-picker、color-picker；emoji 许可单列 | 时区、locale、空值、范围边界、键盘和画布位置；不添加原来没有的选择功能 |
| Tooltip、菜单、右键、Popover、HoverCard | tooltip、dropdown-menu、context-menu、popover、hover-card | 嵌套 Esc 顺序、指针／键盘等价、锚点跟随、窗口边界、滚动与缩放、焦点返回 |
| Dialog、确认、侧面板、移动窄屏弹层 | dialog、popover、bottom-sheet；现有 Dialog／ConfirmDialog／PopConfirm 保持职责 | 桌面 modal 原位出现；dirty guard、异步提交、嵌套 Office popup、返回焦点；不能照搬 drawer 的侧滑 |
| 通知、错误、空态、加载、进度 | alert、toast／toast-stack、empty-state、skeleton、progress | 真实状态、无障碍播报频率、重试、持久错误、未知进度；复用现有 toast／popup 服务 |
| 侧栏、Tab、面包屑、命令面板、分隔／缩放 | tabs、breadcrumb、command-palette、resizable-panels、scroll-area | 多 Tab KeepAlive、分离／关闭、重排、脏状态、原生拖拽区域、滚动位置、快捷键；不迁移路由架构 |
| 表单布局、卡片、标题、详情、说明、状态标识 | card、avatar、badge、stepper 等共享基础件重组现有 composites／console | 信息密度、中文长文案、必要说明与错误位置；不凭采用 Arc 加重复标签 |
| 列表、表格、树、筛选、分页、批量操作、拖排 | sortable-data-table、tree-table、tree-view、filter-toolbar、pagination、reorderable-list | 原数据分页／排序／选择语义、虚拟化、选择锚点、批量权限、刷新和失败；保留 TanStack／virtua 等有效能力 |
| 模型与服务设置 | 以上表单、选择器、反馈组合；provider／model／路由／网关／文档处理实际页面 | 现有 ACL／宿主持久化、真实检测结果、模型身份、协议和可用性；保留本地账号导入现有能力和失败原因 |
| 任务输入和会话 | chat-thread、code-block、file-dropzone；输入器／工具栏在原 composer owner 中迁移 | 草稿、附件、历史、IME、流式续接、停止、重试、复制、编辑和滚动锚定 |
| Agent 状态、计划、工具、审批、来源和产物 | 免费基础件先覆盖；agent-status、tool-call、agent-plan、source-cards、code-suggestion、agent-run／agent-approvals 为 Pro 候选 | 真 ACP／宿主事件、等待审批、拒绝、取消、超时、部分失败、真实文件与来源；不重建 agent runtime |
| 工作区 Wiki／Analysis／图与时间线 | timeline、筛选、播放／详情等控件；保留当前事件投影和图布局内核 | 真实事件含义、选中资源、缩放、搜索、播放、20,000 事件场景、离屏暂停；不换成演示 sunburst |
| 文件、资料库、知识、笔记、归档、数据导入导出 | 树／列表／预览／表单／确认组合；按真实入口迁移 | 空库、缺失、只读、路径与权限、非 ASCII 名称、失败恢复、实际文件内容和脏关闭 |
| HTML／图片／Markdown／PDF／Word／表格／演示／文本预览 | 覆盖 FilePreview registry 的每种 plugin 外层、工具栏、菜单、错误与加载 | 扩展名分派、选区引用、下载、缩放、复制、可读性；保留原内容 renderer 与加载边界 |
| Office 内部 UI | 逐项替换其 design-system primitives，再迁移文档／表格／演示／PDF／Markdown 工具栏与弹窗 | selection、composition、格式／公式／排版／分页／打印、批注、导入导出、协作、dirty guard；不得换成简化富文本演示 |
| 第一方 applet、插件市场和安装／授权 UI | 共享表单、列表、菜单、状态、确认；每个实际注册的视图逐项列入 | 沙箱／capability、安装来源、授权范围、版本与恢复；第三方客体 DOM 只按既有主题接口接入 |
| 实际可达的 form／flow 控件 | React widget registry 中适配共享控件；保留表单求值与工作流图内核 | schema 值类型、校验、动态数据源、表达式、撤销、图交互；只读源码追踪不能把别名存在当作已可达 |
| 快捷助手、划词、截图、VisualPin、独立窗口 | 经当前宿主证明可达后，复用已迁移控件；截图画布、贴图定位功能保留 | 热键、剪贴板、屏幕坐标、多显示器／DPI、焦点和窗口关闭；只加载所需样式，不把全套主窗口依赖加进轻窗口 |
| 启动、更新、错误、退出、隐私／法律通知 | 共享状态／确认／通知；pre-React 启动层可复用生成后的静态设计值 | JS／chunk 失败时仍有反馈、无需 theme provider 的致命错误、更新签名与关闭保护；不增加重型启动依赖 |
| CLI／TUI 与系统原生表面 | 终端用原 Rust renderer 对齐信息结构、状态和选择行为；系统文件对话框／权限提示保留平台实现 | Arc React 源码不能直接运行于 ratatui；原生菜单、IME、窗口控制、系统权限不做 HTML 仿制替代；明确记录边界 |

### 三项必须先解决的技术差异

- **主题语义。** Arc 与 Aster 存在同名、不同义的 token，尤其 `--accent`。采用源码时将角色映射到现有 token／product roles；不能加载一套全局 Arc foundation 覆盖 Aster。主题仍由现有 ThemeProvider／宿主偏好负责，保留当前 `.dark` 与启动层同步；不再建一套 `data-theme` 偏好存储。修改 [token 源](../packages/ui/src/styles/tokens)、[语义别名](../packages/ui/src/styles/shadcn.css)、[产品角色](../packages/ui/src/styles/product.css)，通过现有生成流程得到 `theme.css`。Arc 默认的大圆角、留白与字体不能无条件覆盖桌面密度。
- **弹层运动。** Arc Dialog 默认包含位移、blur 和 backdrop blur；按 Aster 原位 modal 契约调整，保留焦点语义，移除违反约定或未经性能证明的效果。[Arc Dialog 文档](https://uiarc.dev/components/dialog) 明确描述了这些默认效果。采用组件不构成放宽 constitution XVI／XVII 的理由。
- **选择器能力。** 现有 [SearchSelect](../packages/ui/src/components/console/SearchSelect.tsx) 已有多选、group／meta、搜索、portal 和 dismissal 能力；Arc Combobox 是候选基础，不是现成的功能对等证明。保留有用能力和现有虚拟化，未知或失效选项按原边界处理，不通过 label 猜模型身份。

## 5. 分批执行与出场条件

顺序按复用面与功能风险安排。先完成一个可操作流程，再推广共享实现。每批完成都需要清单记录、测试和旧实现收敛，不能把清理无限延期。

| 批次 | 依赖 | 具体任务与主要文件 | 出场条件 |
| --- | --- | --- | --- |
| A0 可达性与基线 | 无 | 冻结身份；完成第 3 节两张清单；核对每个路由和查询参数窗口；从真实 Aster 构建记录关键流程、截图、访问性和性能基线；建立每项决策 | 所有发现的入口有去向；没有用“隐藏”“复杂”批量排除；未知入口标明并核对；分母、平台、源码和构建身份可重现 |
| A1 主题与基础控件试点 | A0 首批清单 | 明确 DESIGN.md 中“行为参考”向受控源码采用的扩展；处理 token／motion／portal 兼容；在现有 primitives 改 Button、Input、Checkbox／Switch、Tooltip 等首批依赖；保留导出和 facade | 现有消费者 API 与 data-ui 兼容；主题切换无闪烁；键盘与减弱动效用例通过；无全局 foundation 污染；仅有用的依赖进入构建 |
| A2 模型设置完整流程 | A1 | `ProviderSettings`、`ModelSettings`、实际模型选择器；保存／检测／失败恢复／确认／删除；所需 Dialog、Popover、SearchSelect 一并适配 | 从进入设置到保存、重开读取、真实失败恢复和键盘完成；现有账号导入路径逐项保持；不恢复当前关闭的 onboarding |
| A3 全局交互与工作台 | A2 | Shell／侧栏／tabs／菜单／命令／Toast／PopupHost／加载与异常；迁移其余高复用 primitives 和 composites | 多 Tab／分离窗口、嵌套弹层、焦点恢复和脏退出通过；异常页不依赖已失败的 provider；重复基础控件 owner 已收敛 |
| A4 任务与 Agent 流程 | A3 | composer、taskConversation、消息／工具渲染分派、WorkingStatus、审批、计划、diff／review、产物／来源、工作区资源面板 | 用真实协议输入测试流式、停止、取消、拒绝、重连与部分失败；权限决定无动画延迟；大输出与长会话保持锚点和响应；无假数据和第二套状态机 |
| A5 全部资源与设置 | A3；会话交叉入口依赖 A4 | 文件、资料库、Wiki、笔记、插件、归档、数据导入导出、全部当前可达设置及详情／弹窗；Analysis 外层控件 | 按清单逐项完成状态与操作；大列表、批量操作、搜索／筛选保持语义；每种资源的错误、只读和恢复通过 |
| A6 Office 与复合引擎界面 | A3、A5 的预览基础 | Office design-system／编辑工具栏／弹窗；全部预览 plugin；实际可达 form／flow widget；Analysis 时间线／图控制 | Office 所有可达控件有决策，编辑／选区／IME／导出／重开无回归；保留引擎的边界有证据；20,000 事件契约通过；生成资源由源重新构建 |
| A7 辅助窗口与平台表面 | A3；编辑窗口依赖 A6 | 经 A0 证明存在的快捷／划词／截图／VisualPin／applet／Wiki／编辑器／通知等窗口；pre-React 启动和更新／退出路径 | 实际发行资源可加载；独立 root 的主题／i18n／portal 正确；截图像素定位、原生焦点／拖拽／DPI 行为通过；TUI 与原生保留项记录完整 |
| A8 全量收敛与交付 | A4—A7 | 复查路由／命令／注册集差异；删除所有已替换且无调用者的旧控件、CSS、动效、依赖和临时兼容；更新文档和许可清单 | 两张清单全部有有效终态，所有可替换项已采用／适配／重组并验收；没有无理由保留；跨平台证据和实际打包结果齐全 |

A4、A5 的无交叉部分可在共享基础稳定后独立推进；这是依赖拆分，不预设增加并行 agent。每批按实际清单规模再切小批，避免一次改完全部共享组件造成无法定位的回归。

### 第一批实现任务的最小完整范围

1. A0 先核对 `/settings/provider`、`/settings/model` 的全部入口、子区域和状态，并完成全产品入口清单；不以首批试点替代总分母。
2. 记录当前组件 API、现有行为测试、真实保存／重开路径、失败恢复及性能；核对所有共享调用者。
3. 冻结所需免费 Arc 源码身份及 MIT notices，在原 owner 内适配 tokens 与事件；先迁移这条流程所需的完整控件集合。
4. 添加少量实际风险用例：输入法提交、嵌套 picker／dialog 的键盘与焦点、保存失败保留草稿、大模型列表不降级；复用原测试，避免重复实现业务逻辑。
5. 运行 owning package 检查与真实宿主流程，对照基线；删除这条流程已经失去调用者的旧样式与实现。
6. 在证据中记录迁移条目、保留差异、删除项、失败项及未测平台；通过后再推广剩余基础控件。

## 6. 许可、来源和依赖

Arc 免费部分使用 MIT，需保留版权和许可。Pro 允许用于终端产品，但限制以组件库、UI kit、registry 等形式提供源码；开源终端产品也需保留独立 Pro notices，不能将其声明为项目自身开源许可。[Arc 许可](https://uiarc.dev/license)

- 首批用免费源码和现有依赖推进。Pro 候选记录在清单，不自动下载、不购买，不将 Pro 源码作为 MIT UI 包的组成部分发布或共享。
- 若需要 Pro，实施前核实已有席位和所选代码的使用／分发位置；`private: true` 或私有子模块不是自动满足全部许可条件的证明。许可不明确的项不能算已采用，其他免费批次继续。
- 每个实际复制项记录官方 URL、取得日期、上游可用版本／提交或源文件摘要、文件列表、许可、依赖、Aster 修改。复用 [现有 provenance 目录](provenance) 和 owning package 的 notices 方式，不引入下载器或运行时远程加载。
- Arc 安装例子中的 `@latest` 不成为可重复构建依据；纳入源码后由当前仓库锁文件和构建流程管理。新依赖必须对应真实能力，已有 Motion／Radix／Lucide 能满足时直接复用。
- 第三方插件／网页不注入全局样式，不扩大 capability 或跨 iframe 访问权。第一方插件按各自源码 owner 迁移；只能控制主题接口时记录该边界。

## 7. 验证与性能验收

### 7.1 行为与访问性

每个迁移组件覆盖其实际变体；每个界面覆盖第 3 节的有效状态。所有权相关操作继续走原 service／host／ACP，不用动画模拟验证成功。

| 风险 | 必须证明的行为 |
| --- | --- |
| 键盘与输入法 | Tab／Shift+Tab、箭头、Enter／Space、Esc、Home／End、快捷键；中文 composition 未结束不提交、不选错项；长标签、RTL、缩放与字体回退可用 |
| 弹层和焦点 | trap／恢复／初始焦点、嵌套 Esc、关闭后 inert 与滚动锁释放；同一窗口内 portal 归属、编辑器选区和菜单位置正确 |
| 操作状态 | 防止真实重复提交；pending 对应操作寿命；失败可重试且保留输入；取消／关闭不制造成功；停止和审批立即进入业务路径 |
| 主题与语义 | 明暗／系统主题、运行中更改、contrast、focus ring、aria name／role／live；保留现有 data-ui 稳定契约及合法自定义 CSS 入口 |
| 动效生命周期 | 系统减弱动效在挂载期间改变立即生效；关闭、settled、后台、离屏、隐藏 Tab 后停止无用 rAF／timer；恢复不重放全部入场 |
| 数据和权限 | 模型／路径／资源仍按原边界验证；只读／拒绝／缺失有原有原因；不得因更换 picker 推断身份、放宽权限或暴露密钥 |
| Office／复合组件 | 选区、IME、dirty guard、撤销、协作、粘贴、导入导出和重开一致；用实际编辑器，不用简化 stub 替代关键路径 |
| 原生 | 真实宿主窗口、capability、平台焦点与窗口控制、多显示器／DPI、截图区域；浏览器通过不能代表所有原生平台通过 |

### 7.2 性能

目标是提高反馈清晰度并守住实际性能；目前没有证据证明换用 Arc 会自动变快。

1. A0 固定同一构建模式、硬件、数据集、窗口大小与操作；记录冷／热启动、关键 chunk 大小、交互到响应、长任务、掉帧、React commit、DOM 数量、内存和空闲 CPU。基线重复运行后冻结各流程预算，不能实现后调宽预算求通过。
2. 场景至少包括：大型模型搜索、长会话与工具输出、资源虚拟列表、多个保留 Tab、嵌套弹层、Office 文档，以及现有 20,000 事件 Analysis 场景。数据集沿用 owning 测试契约，实测后记录规模，不能用空页面证明大数据性能。
3. Arc action-button 的精细标签动画只用于少量明确操作；高密度控件沿用轻量 CSS。模态去掉未经证明的大面积 blur，保持点击目标与阅读位置稳定。宽高测量不在无关状态重复触发。
4. 保留现有虚拟化／按需加载；不为动效展开所有工具详情，不让每 token 或每帧重渲整棵会话／图。播放时钟走现有 motion value／局部更新约定。
5. Analysis 继续遵守 [既有运动与负载契约](../specs/026-agent-execution-understanding/contracts/analysis-motion.md)：20,000 事件、游标操作不产生重复业务请求、隐藏后停止工作；不能为 Arc 演示结构退回全图逐帧重排。
6. 性能超预算时在同一 owner 中简化效果，保留功能和可访问性；记录真实结果，不用“视觉更流畅”替代数字。报告分位数必须有相应样本，单次数据只能称单次结果。

### 7.3 沿用的检查与证据

实施时在 owning package 运行相关现有命令；以下是计划检查，不是此次已通过的声明：

- 根 UI 包：`bun run --cwd packages/ui typecheck`、`bun run --cwd packages/ui test`，主题变更运行 `theme:build`／`theme:check`；既有 stories 随状态扩充。
- Studio 子模块：按其现有 pnpm 流程运行 `test:pkg:ui`、聚焦 `test:renderer`、`typecheck:web`、`test:lint`、`format:check` 及相关 i18n／scripts 检查；保留 `ui:contract:query` 与 AST 语义测试。修改基础件后检查其他真实消费者。
- 实际修改 Office／form／flow 时，运行各自 package 已定义的 test／typecheck／build 及对应已有交互回归；不得用直接改生成 bundle 省略构建。
- 原生路径用仓库根 pinned toolchain、`just build-studio` 及受影响 Rust package 的聚焦测试／Clippy；修改 Rust 时运行 `cargo fmt --all -- --check`，精确记录外部已有失败，不能把它们悄悄修进 UI 迁移。
- 实际 GUI 操作复用现有 A3S Test／E2E 和证据流程，覆盖原生宿主；现有 shrink-only allowlist 只能收缩，不增加 native `<select>`／`<datalist>` 或 Radix Select 债务来换通过。
- 跨平台按照既有 CI 支持矩阵记录 OS、架构、WebView、features、源码与二进制身份；Windows／Linux／macOS 未做原生运行的格子明确为未测。共享基础件影响其他 tenant 时覆盖其回归，不复制 Aster 的账号规则过去。
- 每批写 `docs/evidence/desktop/<日期>-arc-ui-<批次>/OUTCOME.md`，保存条目、命令、结果、截图／交互日志、性能、源码与产物身份、许可变更、删除项和 Explicitly not claimed。最终交付仍走既有完整性／发布流程，本计划不新增发布操作。

## 8. 完成判定、清理与回退

最终分开报告：入口核对数／总入口、状态验收数／有效状态、共享组件决策与验收数、采用／适配／重组数、保留内核数、原生／外部边界数、未完成数。当前不填尚未建立的数量，不给估算完成率。

完成必须同时满足：

1. 当前 Aster 路由、命令、窗口、注册与运行时观察能对上清单；动态安装的扩展至少覆盖宿主支持的贡献类型和状态，用户任意第三方插件自有 UI 不伪称全量迁移。
2. 每个当前可达控件有具体决策；适合的项已完成实际替换／重组，保留项逐一有能力或平台理由。复杂页面不能整页保留而不检查子控件。
3. 每个实现拥有行为／访问性／性能／所需原生证据；截图、Storybook 或静态搜索单独不构成通过。
4. 每批检查当前引用、导出、依赖和生成源；所有无消费者的已替换实现、样式、动效 helper 和临时兼容分支删除。兼容 facade 若仍承载外部调用，保持单一 re-export，不能强删 API。
5. 目录更新采用增量复核，不移动已冻结验收分母；新增产品入口在所属批次加入清单，不留到所谓“下一代 UI”。
6. 回退按一个有边界的迁移批次恢复原 owner 实现和消费者，不加长期双实现开关，不回退用户已保存数据或复制用户账号状态。UI 迁移不附带持久化 schema 改造。

## 9. Constitution Check（计划约束）

本表表示方案可按约束实施，不表示实现已通过检查。

| 原则 | 计划状态 | 落点 |
| --- | --- | --- |
| I Fail closed | 符合 | 未知值／失效资源保留拒绝与原因；UI 不扩大权限 |
| II Evidence retained | 符合 | 每批独立 OUTCOME，清单与实际产物身份可对照 |
| III 路径对应测试 | 符合 | UI／Studio／Office owning hermetic tests 与实际原生交互 |
| IV Closed sets | 符合 | 记录状态固定；不新增猜测业务类型的逻辑 |
| V 工作区与 API | 符合 | 保留 Rust workspace、组件导出和实际能力；UI 不依赖桌面 shell |
| VI Schema | 不涉及 | 不附带存储迁移 |
| VII Shrink-only | 符合 | 不扩大 native picker／facade 等已有 allowlist |
| VIII 可替换传输 | 不涉及改造 | 保留原 service／ACP 边界 |
| IX ACL | 符合 | 不新增产品配置语言；开发清单属于验收与源码资料 |
| X 命令与测试归属 | 符合 | 保留 capability；新行为在 owning 测试项目验证 |
| XI／XII 判断与复核 | 符合 | 先证明可达性与能力，实测性能，明确保留项和反例 |
| XIII 单仓库 | 符合 | Studio 保持授权私有子模块，父仓库不收其源码；不新建仓库／工作树 |
| XIV／XV 名称与含义 | 符合 | 从现有精确注册／类型／组件边界取身份，不按文字猜模型或权限 |
| XVI 绘制选择器 | 符合 | 使用现有 drawn picker owner；不引入 native select／datalist |
| XVII 原位弹窗 | 符合 | Arc 默认位移／filter 必须适配，沿用现有 modal 契约 |

## 10. 此次规划的验证范围

已读取当前路由定义、侧栏与设置 guard、main 的条件启动根、窗口说明、构建入口、共享 UI 导出与主题、Office 原语和预览分派，以及既有 Arc 研究、constitution 与 SDD 约定；已复核官方安装、目录、Dialog 和许可页面。

本次持久化包含本计划及 DESIGN.md 中的导航链接，不改变产品源码、依赖、配置或许可；没有安装 Arc、恢复入口、进行 GUI 全量巡检、运行前端／原生测试或证明性能收益。A0 的全量组件级清单与原生入口核验是下一批交付物，不能把上述源码种子当成已经完成的全量审计。

## 11. 执行进度与接续

本文件是全产品迁移方案和批次进度的统一入口；各有边界批次的 spec／plan／tasks 从这里链接，实际验收结果链接到 `docs/evidence/`。后续更新本节，不另存一份内容相同的总方案。仅在对应任务和验收条件全部满足后勾选；既有 Arc 改动不重复记为本计划完成项。

- [x] 总体方案：范围、源码入口种子、组件族映射、A0—A8 依赖、许可边界、性能与验收要求已记录。
- [ ] A0：完成全产品可达界面／组件清单，冻结源码与构建身份，建立行为和性能基线。
- [ ] A1：完成主题、动效、portal 适配与首批基础控件，验证共享调用者兼容。
- [ ] A2：完成模型设置全流程试点及其测试、原生操作和旧实现清理。
- [ ] A3：完成全局控件、工作台、命令、弹层和异常路径。
- [ ] A4：完成任务与 Agent 会话、工具、审批、产物和来源界面。
- [ ] A5：完成资源与全部当前可达设置、详情、弹窗及有效状态。
- [ ] A6：完成 Office、预览、Analysis 和实际可达 form／flow 控件。
- [ ] A7：完成辅助窗口、启动／更新／退出与平台交互核验。
- [ ] A8：完成全量复核、旧代码／依赖收敛、跨平台验收和交付记录。

**相关有边界批次（不替代 A0–A8 勾选）**：
- Arc foundation consistency（紧凑尺寸与共享 hover plate）：`specs/040-arc-foundation-consistency/`，证据 `docs/evidence/desktop/2026-11-03-arc-foundation/`。
- Compact radii + dialog/toast 微动效对齐（进行中）：`specs/041-arc-ui-compact-motion/`，清单 `contracts/radius-motion-inventory.json`。
- Near-term ArcUI compliance B0–B4（债表清零、SearchSelect 迁完产品 Radix Select、分段 highlight、搜索/侧栏 chrome、UI 包卫生）：证据 `docs/evidence/desktop/2026-11-03-arc-ui-compliance/OUTCOME.md`；A0 种子清单见同目录 `a0-inventory-seed.md`。
- Full compliance push（手写 tab/nav highlight、popup 原地动效去 travel、搜索/工具栏 chrome、Office 外框决策种子）：证据 `docs/evidence/desktop/2026-11-03-arc-ui-full-compliance/OUTCOME.md`。

**下一步**：继续清工具栏/筛选 `rounded-full` 与剩余 ad-hoc 搜索；A6 按 `office-chrome-decisions.md` 逐项迁 Office 外框；用种子清单推进 A0。未完成 A0/A1 勾选前，不宣称全量组件替换或性能收益已通过。

**接续要求**：每次实际推进后更新批次状态、所属 spec 路径、证据路径和准确的下一项；失败和未测平台原样记录。界面入口或源码发生变化时增量核对清单，不用当前聊天上下文代替持久记录。
