# Studio 能力与 UI/UX 复用约束

> **2026-11-03 范围更新**：第三方 harness 不属于 Aster 产品范围；本任务不再扩展其托管或 CLI 接入。以 [tasks.md 的产品范围收敛](tasks.md) 为准。历史参考与未完成状态保留。

**Audit**: 2026-11-01；源码与调用点复核。新增网关导航回归另记于本节与 evidence；尚无全量原生 GUI 验收。补正上一版只列目录、没有确定页面归属的问题。以下约束适用于完整任务清单及其增量，不另建一套 Magpie 管理中心。

## 已确认的复用与真实缺口

表中 Studio 路径均相对 `apps/studio/src/`。

| 工作 | 既有入口/组件与能力 | 本次只补什么 | 关联任务 |
| --- | --- | --- | --- |
| Provider 与上游多 key | `renderer/pages/settings/ProviderSettings/ProviderSettingsPage.tsx`；`ConnectionSettings/ProviderApiKeyListDrawer.tsx` 已有新增/编辑/启停/删除；`useProvider` mutations | 接通网关全部 key、账户候选/状态/代理；扩现有 drawer，不新建上游 key 管理页 | T026 T030 T038 |
| 订阅登录 | 同目录 `ProviderSpecific/LoginOauthPanel.tsx` 已有 native OAuth、取消/登出/缓存刷新，token 不到 renderer | 增加明确的订阅 adapter 与账户选择；不使用产品 AccountCenter 恢复 Aster 账号系统 | T037 T038 T039 |
| 模型发现/导入 | `ProviderSettings/ModelList/ModelSyncPreviewPanel.tsx`；`hooks/useProviderDeepLinkImport.ts` 已有预览确认、create/update/add-key | 扩参考来源、字段、格式保留和失败处理；保留一套 preview/选择/应用链 | T016 T040 T047 T130 |
| 路由计划 | `renderer/pages/settings/ModelSettings/LocalModelRoutes.tsx` 已有列表、排序、模型选择、草稿、side panel、保存失败状态 | 扩策略/nested/intent/effort 字段，复用 `DefaultModelSelector`/`SearchSelect`；修复入口挂载缺口 | T007 T031 T032 T033 T034 T100 T136 |
| 本地预算 | 同目录 `LocalModelBudgets.tsx` 已有周期、scope、token/成本、notify/stop、草稿及状态读取 | 接通 Gateway 与现有预算判断/预留，扩 client/account scope 和窗口；不再造预算编辑器 | T022 T023 T024 T025 T041 |
| 用量 | `/settings/usage` 在 Aster 已挂载 `UsageSettings/LocalUsageSettings.tsx`；`useLocalUsage.ts`、`localUsage.ts` 已有 aggregate/page/CSV、request/attempt/credential/gatewayClient 字段、未知消费展示 | 接入外部 Gateway 同一事实源，扩缺少的分组/账户维度、路由与窗口；不新增第二 dashboard/CSV 管道 | T036 T042 T043 T044 T045 T137 |
| 网关设置 | `ToolSettings/ApiGatewaySettings/ApiGatewaySettings.tsx`、`LocalGatewayModelLinks.tsx` 与 `LocalGatewayClients.tsx`；生命周期、端口、默认密钥、命名客户端与原管理入口跳转 | 命名客户端复用同一密钥区域、`PageSidePanel`、`SearchSelect`、`Switch` 和 `CopyButton`；模型/路由/预算/消费仍跳原入口。周期预算与完整撤销/历史继续接线 | T015 T020 T021 T024 |
| 外部 Agent | `/app/code`→`CodeCliPage.tsx`→`useCodeCliPageViewProps.ts` 已接配置、网关模型、安装/升级/移除；`useConfigPanelController`/`useConfigDraftController`/`CliConfigEditor` 已有草稿和验证 | 扩现有 adapters、字段/文件保留、Profile/模型可见性/撤销，沿用当前配置/启动体验 | T052 T053 T104 T105 T106 |
| 插件与能力 | `renderer/pages/plugins/PluginStorePage.tsx`、`pages/capabilities/CapabilitiesPage.tsx`、`PluginConfigDialog`、workbench plugin slots 已有市场/已安装/配置/扩展机制 | 复用现有页面框架、贡献点和 lifecycle；OpenCode/pi/middleware 做兼容 adapter。现有代码混合 member authority 与本地来源分支。Capabilities 已有 local catalog 分支；分别验证本地来源与权限，不能只据页面壳断言完成，也不复制 managed member authority | T109 T110 T050 T111 T112 T113 |
| Runtime 安装 | `settings/DependenciesSettings/RuntimeComponentsSection.tsx` 已接 `runtime.components.list/install/remove`、安装状态/进度/错误 | runtime 探针确认后只扩组件目录和对应 native 实现，不新建下载器/进度系统 | T109 T113 T129 T135 |
| Library/skills/MCP | 现有 capabilities、resourceCatalog、MCP/skills 管理与插件内容 owner | 增加向外部 Agent 分发/撤销的 adapter。`PersonalLibraryPage` 是个人资料/知识库，不将其误作 Magpie 配置 Library | T107 T111 T118 |
| 搜索 | `settings/WebSearchSettings/components/WebSearchProviderSetting.tsx`、`WebSearchApiKeyList.tsx`、provider check/persist hooks 已有多 key、端点、验证、独立 capability 配置 | 扩顺序/冷却/quota 和外部工具消费；复用设置与 key 编辑，不再建搜索源页面 | T114 T115 T119 |
| 设备连接 | `/settings/device-connections` 的现有页面已有 LAN/地址/设备/权限操作外壳 | 复用信息架构和控件；其旧 `api_gateway.remote.*` 后端不能当新网关已可用，替换为经授权的新 native 适配，不恢复旧配对服务 | T060 T120 T126 |
| 备份/恢复 | `settings/DataSettings/` 与 `services/BackupService.ts` 已有 local/WebDAV/S3/恢复交互 | 补原生网关真实成员、冲突/回滚和凭据边界；复用这些表单与状态，不新建同步中心 | T061 T121 T122 T123 |
| 隐私 | 现有 wire/Sentry 是能力基础；用户隐私政策弹窗不是脱敏设置页 | 在现有网关设置中只新增缺少的脱敏字段/检查区，检查记录沿用本地用量请求详情；不复制 Magpie Insights 导航或误用隐私政策 UI | T051 T101 T102 T103 |

## 35 项能力的页面归属与当前差额

以下按冻结能力表顺序逐项映射。页面基础、组件测试和完整原生验收是三种证据。
本表没有把任何一行标成全部完成。协议能力通过可用模型、连接配置和请求详情表达。
不为每个 HTTP 协议另建管理页。

| 能力 | Studio 中的入口与操作 | 当前 UI 证据与差额 |
| --- | --- | --- |
| CAP01 网关生命周期 | 设置 → API 网关。查看状态，启动、停止、重启，复制地址。 | 页面与 hook 已有。复核发现 Aster 导航/搜索隐藏页面，深链会回到通用；本轮补通导航。原生重启恢复与端口冲突仍须验收。 |
| CAP02 推理协议 | 模型服务中配置 endpoint。Code 选择网关，调用详情显示协议与失败原因。 | 公开 Responses HTTP、Messages 和 count_tokens 已接统一控制，原生及已实现的 Chat 转换复用已有能力。计数明确区分上游值/估算且不产生生成账单。Gemini 原生内核与 Gateway 入口专项、官方 SDK 回环已推进；生产 ProviderStore/私有 HTTP/UI、Cloud 用量契约、Responses WS 与 Code 卡片的实际能力投影仍未接通，分布式并行回归仍待诊断。 |
| CAP03 服务商与模型 | 设置 → 模型服务。预设/自定义、模型发现、模型编辑与能力选择。 | 已有页面、模型列表与同步预览。完整 provider 预设、输入类型、effort 与真实能力投影仍待接通。 |
| CAP04 外部配置导入 | 模型服务的导入预览。选择新增、更新或添加 key，再确认应用。 | 复用现有深链导入与预览。完整 CC Switch/Claude/Codex/Alma/OpenCode 来源尚未验收。 |
| CAP05 多 key 与账户候选 | 服务商连接设置中的 key 抽屉。新增、编辑、启停、删除。 | 抽屉已有。网关仍需使用全部 key，并显示候选状态、冷却与账户代理。 |
| CAP06 订阅登录 | 服务商授权面板。登录、取消、刷新、登出与选择账户。 | 已有部分 native OAuth 面板。完整订阅清单、自动发现和 quota 窗口仍待适配。 |
| CAP07 路由组与策略 | 设置 → 模型配置 → 路由计划。侧栏编辑候选并调整顺序。 | 已有固定模型与顺序回退。rotate/usage/pace/smart/weight/free/fast、嵌套与完整粘性未呈现。 |
| CAP08 条件与意图路由 | 同一路由编辑器增加显式条件与 effort。用量详情解释实际选择。 | 目标落点已确定。条件编辑、意图分类与实时解释尚未形成可用链。 |
| CAP09 客户端密钥与限制 | API 网关中的客户端列表。按客户端编辑模型范围、额度、并发与撤销。 | 命名客户端、模型/接口授权、轮换和撤销已接入原密钥区域；默认客户端在该区域编辑总并发，命名客户端用原侧栏编辑，跨模型/接口共用占用；额度跳原模型配置预算页并预选客户端，支持日周月和提醒/停止；完整矩阵、历史及真实桌面仍待验收。上游 key 留在服务商页。 |
| CAP10 用量、成本与账户配额 | 设置 → 用量。筛选、聚合与查看明细。服务商面板显示账户余额窗口。 | 本地用量已有。外部网关与本地事实源仍须统一。完整账户维度、价格来源与配额刷新尚未验收。 |
| CAP11 会话与路由观测 | Code 的会话入口与用量请求详情。续开会话，查看上下文和路由尝试。 | 已有部分会话/用量基础。成本、cache 构成与路由解释尚未完整接通。 |
| CAP12 请求脱敏与恢复 | API 网关中的隐私检查区。请求详情显示处理结果。 | wire/Sentry 是底层基础。网关设置尚无完整脱敏交互与恢复验收。 |
| CAP13 脱敏规则与试用 | 同一隐私检查区编辑 Mask/Warn/Block、例外和精确规则，试用后查看结果。 | 尚缺对应完整 UI。隐私政策弹窗不承载此功能。 |
| CAP14 每 Agent 模型配置 | Code → 选择工具 → 配置。设置模型、effort、可见模型，再保存配置文件。 | 已有配置侧栏与草稿控制器。完整 Agent 字段、文件保留和接管撤销待逐项验收。 |
| CAP15 Profile 与 Library | Code 中切换 Profile。能力页管理 skills/MCP/指令，并向工具分发或撤销。 | 既有配置与能力 owner 可复用。全 Agent 快照、当前 Profile 标记和仅撤销托管内容仍待实现。 |
| CAP16 安装与会话续开 | Code 工具列表与详情。检测版本，安装、升级、移除和启动。 | 已有部分适配。完整 Agent 清单、安装来源及 Windows/WSL 独立实例未验收。 |
| CAP17 第三方授权插件 | 插件页管理 OpenCode/pi adapter。环境依赖页安装 runtime。 | 自有插件页与安装进度已有。Magpie 对应插件授权、刷新与 check-in 尚未等价。 |
| CAP18 请求中间件 | 插件配置抽屉编辑顺序与选项。请求详情显示执行失败。 | 复用插件配置与生命周期。hook 协议、预置变换和错误交互仍待实现。 |
| CAP19 插件扩展与页面 | 既有插件市场、已安装列表、配置抽屉和工作区扩展槽。 | 已有自有插件外壳与 AIX catalogue。完整兼容 adapter、崩溃恢复和能力限制未验收。 |
| CAP20 搜索与抓取 | 设置 → 网络搜索。编辑搜索源和多 key，测试连接。能力页配置外部工具。 | 搜索设置已有。顺序、冷却、quota 与外部工具消费仍待接通。 |
| CAP21 图像与视频 | 模型服务声明能力。沿用 Studio 工具/产物预览与 Code 调用入口。 | 现有图像能力不代表完整生成/编辑/视频兼容。异步状态、下载与取消尚待逐 provider 验收。 |
| CAP22 对外 MCP | 能力页的连接器管理与 Code 工具分发。API 网关提供外部连接参数。 | 现有 MCP 基础可复用。对外 router、同名工具消歧与客户端可见性未接通。旧设置页不可达不等于缺少能力页。 |
| CAP23 远程共享 | 设备连接的地址、设备列表与权限控件。Code 选择远端来源。 | 旧设备连接页面存在，Aster 当前导航未列出。需替换旧 IPC 后复用控件，不能据旧配对 UI 宣称新网关可用。 |
| CAP24 备份与同步 | 设置 → 数据。沿用本地/WebDAV/S3 备份与恢复面板。 | 通用备份已有。网关真实成员、受保护的 GitHub 同步、冲突与恢复仍待实现。 |
| CAP25 桌面、CLI/TUI 与无头入口 | 桌面沿用设置与 Code。CLI/TUI/serve 共用同一配置和运行状态。 | 当前入口分散。完整配置权威与读写语义待统一。命令行能力本身不需要镜像成多套菜单。 |
| CAP26 后台与主题 | 设置 → 通用/外观。使用开机启动、托盘、保持唤醒和主题控件。 | 这些通用控件已有。网关开机恢复与各平台后台行为仍待验收。 |
| CAP27 Docker/server/NAS | 复用网关连接设置、环境依赖和数据目录。部署通过既有命令与发行流程。 | 容器持久卷、升级/卸载与所需 companions 未完成。桌面 UI 不能证明部署包完整。 |
| CAP28 跨平台与性能 | 使用同一套 Studio 布局与状态。不可用操作显示原因与可行下一步。 | 未完成原生安装、各架构、Windows/WSL/Linux 与性能验收。源码/浏览器截图不能代替这些证据。 |
| CAP29 decision/System One | 模型服务显示非 chat 能力。路由编辑器配置 decision-only 分类器。 | Gateway 有平台入口。模型选择、choice/noul/score 与路由 UI 尚未完整接通。 |
| CAP30 限额、自查与诊断 | API 网关显示客户端状态。用量页显示额度、窗口、路由、并发与历史详情。 | 已有分散底层数据。尚缺完整 quota 卡片、刷新/等待和自查体验。 |
| CAP31 音频 | 模型服务声明音频能力。Code/外部工具沿用连接与产物入口。 | 不把音频当成一个独立网关设置页。统一授权、计量、状态和取消尚未完成。 |
| CAP32 rerank | 模型服务中显示 rerank 能力。通过现有工具调用与请求详情验收。 | Gateway 平台入口已有。宿主能力投影、可见性和实际调用未完整验收。 |
| CAP33 Responses WebSocket | Code/外部 SDK 使用同一网关连接。请求详情保留会话与取消状态。 | 缺完整 WS native 契约与体验。不会因为 HTTP 地址相同而标记支持。 |
| CAP34 Agent/Ollama 目录发现 | Code 的模型选择器读取真实可见目录，配置目标 Agent。 | 缺完整元数据和无 bearer 客户端的认证适配。UI 不自动宣称所有模型均可调用。 |
| CAP35 Magpie 迁移 | 设置 → 数据中的显式导入。预览模型、路由、Profile 与 Library 冲突，再应用。 | 通用导入/恢复外壳已有。专用迁移、冲突处理和失败回滚尚未完成。 |

### 用户旅程的验收顺序

1. 打开模型服务，导入或配置真实上游。
2. 在模型配置中选择路由与预算。
3. 启动 API 网关，创建客户端授权。
4. 在 Code 中选择 Agent，预览并保存配置。
5. 发起请求，在用量中定位同一个请求与实际上游。
6. 返回原编辑器，修改配置并验证持久化结果。

这六步是目标体验，尚未全量接通。验收还需覆盖保存失败、端口冲突、权限拒绝、
未知用量、客户端撤销、取消、窄窗口与键盘焦点。

## 不能把组件文件存在算成页面可用

2026-11-03：Aster 的原密钥区域已接入命名客户端。默认客户端密钥保持兼容；额外客户端通过原侧栏创建、改名、选择模型/路由和接口、轮换或撤销。密钥只在签发时返回，关闭侧栏后清除临时值。编辑失败保留草稿，应用失败仍交付已签发密钥并显示实际停止状态。只改名称不重启网关；该轮授权变化通过停止/重启生效，当时将不中断在途请求的授权快照更新列为后续。918 项前端/共享测试覆盖实际组件、共享 schema、bridge、原 Code 与路由/预算组件；59 项原生 Gateway、63 项 ModelControl 及 1 项权限测试通过。原生请求验证四种 alias、模型/接口拒绝、轮换/撤销、同址连接复用及同一账本的客户端身份。这不代表真实桌面或完整替代验收，见 [named client evidence](../../docs/evidence/local-gateway/2026-11-03-named-gateway-clients/OUTCOME.md)。


2026-11-03 后续：客户端授权改为复用 Gateway 的嵌入宿主热更新，不重建同址监听器、默认 verifier 或模型绑定；在途请求保留原快照，新请求立即应用新范围、密钥 generation 和撤销状态。实际 native 客户端命令贯通创建、权限收窄、轮换和撤销，并验证挂起的 SSE 最终成功及同一账本两条记录、20 tokens。Gemini CLI 既有后缀与端点交集另有回归；普通托管网关仍拒绝手工更新。没有新增 UI、配置 owner 或依赖。周期额度、并发、完整历史和真实桌面仍未验收，见 [authorization reload evidence](../../docs/evidence/local-gateway/2026-11-03-client-authorization-reload/OUTCOME.md)。


2026-11-03 周期预算后续：客户端行的预算按钮跳原 `/settings/model?view=budgets` 并携带稳定 UUID，路由边界只接收 UUID/兼容默认身份。原 `LocalModelBudgets` 同一侧栏预选该客户端，也可选择全局、服务商或默认客户端；列表失败不改已有 scope，名称只用于显示。沿用 Native Preferences 和原账本的 begin/settle/window，不把额度存进客户端记录。补周一重置、硬停止选项和 429/Retry-After，时间用已保存 UTC offset。真实 Chat 六组日周月 Token/USD 请求证明轮换不清零、其他客户端隔离和额度修改即时生效；928 项 renderer/shared 回归通过。旧 Completions/Embeddings 分支尚未统一计量，页脚明确覆盖范围；全协议硬额度、并发和真实桌面仍开放。见 [periodic budget evidence](../../docs/evidence/local-gateway/2026-11-03-client-periodic-budgets/OUTCOME.md)。


2026-11-03 非 Chat 接线后续：Completions/Embeddings 已移入既有 ModelControl、Core metering 与原预算/账本。API 网关继续用原目录表达显式 operation，embedding 不显示虚构输出上限，text-only/embedding-only 不混入 Chat；原预算覆盖说明更新为六类推理入口。没有新页面、侧栏或额度副本。929 项 renderer/shared 与 67 项公开 Gateway 回归通过；真实 ProviderStore/host HTTP 及命名客户端消费已有证据。生产额度上界、总并发、完整矩阵和真实桌面仍须继续验收，见 [shared accounting evidence](../../docs/evidence/local-gateway/2026-11-03-non-chat-shared-accounting/OUTCOME.md)。


2026-11-03 客户端总并发后续：默认客户端的上限位于原客户端区域，命名客户端沿用原编辑侧栏和 `InputNumber`。空值只取消客户端总上限，原路由限额仍有效。Native Preferences、共享 IPC schema、bridge 和原 Gateway admission guard 贯通；轮换、策略更新和从未设上限改为有上限均保留同一客户端在途占用。取消释放有实际 HTTP 场景，不另建 limiter 服务或 UI 配置页。70 项 Gateway native 与 932 项 renderer/shared 回归通过；真实桌面、完整生产额度上界与性能门禁仍开放。见 [client concurrency evidence](../../docs/evidence/local-gateway/2026-11-03-client-concurrency/OUTCOME.md)。


2026-11-03 模型额度上界后续：模型输入/上下文限制继续在原服务商模型编辑侧栏维护，价格沿用原价格编辑器；预算页只更新既有覆盖说明，提示硬停止所需的输入限制、请求输出上限和完整价格，不新增额度字段。实际 ProviderStore→Gateway→ModelControl→账本已覆盖六接口 × Token/USD/CNY 的 18 组首笔放行/次笔拒绝；普通文本使用已配置的模型上界，token ID 输入可用已知长度。批量与生成候选计入预留；缺少事实、后台或服务端工具执行保持未知。全局预算仍逐候选准入，首个候选预留过大可选择能放入额度的后备候选；未知旧消费不能绕过。完整生产/桌面验收与更紧的输入计数仍开放。见 [provider bounds evidence](../../docs/evidence/local-gateway/2026-11-03-provider-consumption-bounds/OUTCOME.md)。


2026-11-03 模型检查故障接线后续：原服务商页的模型检查继续使用既有 native 命令与错误反馈。用量库损坏/宿主未提供 ModelControl 时，原生检查现在返回原有 422 ModelCheckError，前端保留 AI_REQUEST_FAILED 和状态码，不改走旧 HTTP 检查或直接 curl；不新增提示卡、检查按钮或备用计量链。正常状态可检查未启用模型，消费进入原账本，预算拒绝不触达上游且不修改配置。真实损坏 SQLite、同一原生控制器实现和既有 bridge 回归已覆盖；真实桌面仍待验收。见 [probe admission evidence](../../docs/evidence/local-gateway/2026-11-03-probe-accounting-admission/OUTCOME.md)。

1. 新发现：`settingsMenu.ts` 同时隐藏 `/settings/api-gateway` 的导航与搜索。`SettingsPage` 对无导航项的 Aster 页面重定向到通用。Code 的配置侧栏却仍指向这个地址。本轮为 Aster 增加同一设置导航项并解除这一个页面的隐藏；其余租户沿用原导航。
2. 基线缺口：`/settings/model` 将 `view=routes/budgets` 作为 section 传入 `ModelSettings`，但原 `ModelSettings.tsx` 未挂载 `LocalModelRoutes`/`LocalModelBudgets`。2026-11-01 已接通既有组件和 `SegmentedControl`，仅在 Aster 非 compact 页面显示；managed 与 onboarding 保留既有入口。
3. `LocalGatewayModelLinks` 已跳向这些 view。定向组件测试已覆盖 section 切换、后续深链及仅切页不写配置，相关 3 文件共 22 tests 通过；这不等于网关点击/保存/native 已验收。T031/T024 仍须完成“从网关点击→正确编辑器→保存→返回仍有同一数据”的实际路由测试及后端策略接线，按 `DESIGN.md` §10 保持同页。
4. PluginStore 的 member pins/namespace 与 AIX catalogue、Capabilities 的 local catalog 分支须分别验证。Capabilities 源码已区分 local catalog；它不证明 Magpie 的插件协议或全部安装链已兼容。
5. DeviceConnections 的 UI 与旧 IPC 名称不证明新 Rust 网关有对应实现。复用控件与入口，禁止把已退休服务重新接回。

## 底层同样只补接线与差额

- `src-tauri/src/model_control.rs`、`model_control_http.rs` 已有内部 Chat/Messages/Responses dispatch。公开 Gateway 的 Reserved 不等于 Studio 内部不存在协议能力。T011/T012/T018 先验证这些真实实现和 Core `llm/metering/wire` 的可复用部分，再补公开准入，不能再写一套业务转换。
- `model_accounting.rs` 已有 attempt/request 身份、client 字段、begin/mark_dispatched/settle/budget_state/recover_interrupted；Core `budget/local.rs` 已有 scope/period/resource/window/admit。公开 Chat/Responses/Messages/Gemini/Completions/Embeddings 已接入这些 owner；计数预检不产生生成账本记录。T022/T023/T025/T042 继续补完整协议与额度上界、命名客户端和 UI 全流程，不另加 reservation store。
- `model_control_http` 是内部、有宿主 token 的边界。不能为了复用把 internal token 当外部 key，或外部→内部→上游重复转换/计量；每个请求必须证明只进行一次业务调度和一致的消费归因。
- `RequestRequirements::refusal` 已复用现有三条 codec 边；公开 Chat 使用原 ProviderStore 的入口协议投影，内部偏好 Responses 不改变原公开 Chat endpoint。完整协议组合仍须验证实际 endpoint、认证和原始 usage，不能全局取消协议检查。接线顺序与反例见 plan.md §5.2。
- `settingsMenu.ts` 已将 API 网关移入原有「连接」分组，并移除页内重复标题。导航和保存返回流程通过既有浏览器用例；`LocalGatewayModelLinks` 的启用模型总数仍不能冒充客户端可调用数量。

## 融合实施约束

具体实施顺序、唯一 owner、交互规则和性能门见 [plan.md](plan.md) §4.1、§4.2、§5.1、§7.1–§7.2。本文件仍是 35 项能力的页面归属清单，不另建 UI 计划或重复的设置导航。先接通同一真实执行链，再按完整用户流程扩展这些原入口。

## UI/UX 验收约束

- 以根 `DESIGN.md` §6、§8–§11 为依据。页面归属/导航/主题/字号/间距沿用 Studio，业务兼容不复制 Magpie 菜单。
- 使用 `SettingsPrimitives`、`ProviderSettingsDrawer`、`PageSidePanel`、`SearchSelect`/既有 model picker、`SegmentedControl`、toast/popup。新选择用现有允许的 picker，不因某旧页含 Select 扩大 ratchet。
- 失败保留草稿；保存完成前不关闭或显示成功；loading、empty、unavailable、retry 和权限拒绝沿用当前状态表达；键盘、焦点返回、reduced motion 与窄窗口必须覆盖。
- 网关只承担连接与自身策略入口。Provider 的上游 key 与网关客户端 key 是两种权限对象，可复用外观，不能共用 credential 列表/API。
- 不调整全局 ActivityBar、主框架、设置浮层或任务工作区来容纳网关功能。截图中框架异常另按既有窗口规范处理。
- 每个 UI 任务须说明：已有入口/挂载点、组件、数据 hook、native owner、实际缺项、保留的用户流程和已有测试；不能只写“复用现有 UI”。

## 定向回归

优先扩现有 ProviderApiKeyListDrawer/useProviderDeepLinkImport、LocalModelControls/ModelSettings、LocalUsageSettings/localUsage、CodeCliPage/useConfigPanelController/useConfigDraftController、PluginStorePage/CapabilitiesPage、RuntimeComponentsSection、WebSearchProviderSetting、DeviceConnectionsSettings、DataSettings 测试。用户流程变化再用既有 Playwright settings/code/plugin fixture 验证路由和实际持久化；不把组件单测作为入口或原生后端已接线的证据。


2026-11-03 Code 命名连接后续：文件型 CLI 在原网关配置对话框的模型选择之前使用同一 SearchSelect 选择默认/命名客户端，再填写签发密钥。客户端授权目录来自原 `api_gateway.clients.list`；管理仍跳原网关设置，Code 不再提供创建/轮换或预算编辑副本。原每工具配置记录保存 consumer key、UUID 和 generation，原模型目录按 target/endpoint/CLI 能力取交集，保存、启用和启动经原 CLI draft writer/native writer。保存失败保留输入；列表失败可在同一表单重试；列表未就绪不改写启动配置，旧 raw 命名 key 也不会被隐式默认密钥覆盖。真实共享控件和真实配置生成器已有测试；原生终端启动、桌面布局/键盘完整旅程、运行时专用适配器和平台/性能仍未验收。见 [Code client evidence](../../docs/evidence/local-gateway/2026-11-03-code-gateway-clients/OUTCOME.md)。


2026-11-03 Hermes 入口后续：Aster 的原 Code「启动」进入同一个 LaunchDialog，沿用目录与终端选择、当前模型/命名客户端及 native writer/launcher，不增加 Dashboard 页面或后台服务。旧 Dashboard cache 不控制 Aster Hermes 的运行/停止显示、配置和升级；卸载复用原 clear/restore。其他租户保持旧入口。页面路由、旧状态和恢复顺序已有回归，实际 renderer Hermes 草稿与原生隔离进程已有证据；真实 Hermes/桌面/其他 OS 与性能仍待完成。DeepSeek Harness/OpenClaw 旧 IPC 缺原生实现已登记，不能把原按钮或 schema 视为功能可用。见 [Hermes native entry](../../docs/evidence/local-gateway/2026-11-03-hermes-native-launch/OUTCOME.md)。

2026-11-03 Hermes 活动 profile 后续：继续复用 Code 配置侧栏、原草稿/原始文件编辑与 LaunchDialog，不增加 profile 管理页面、模型选择器或配置仓库。原生 read/write/prepare_restore/run 在每次操作开始解析有效 profile；原文件路径仍由共享目标契约产生，草稿版本绑定 profile，切换后旧草稿即使内容相同或文件都未创建也返回原 config-conflict。sticky profile 的原始字节、归属与部分恢复保留在原 recovery owner 的独立记录中；默认 profile 继续使用原记录。显式 profile HERMES_HOME 不跟随另一个 sticky profile，失效/删除/无身份目录及损坏标记拒绝操作，其他 CLI 不受其影响。已验证真实 writer、ToolStore、原启动脚本与隔离替身读取正确 profile；实际桌面、官方 Hermes 模型请求、provider catalog 与全机器人传播/撤销仍开放。见 [Hermes active profile evidence](../../docs/evidence/local-gateway/2026-11-03-hermes-active-profile/OUTCOME.md)。
