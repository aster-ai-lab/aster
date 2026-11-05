# Research: Magpie 兼容基线与实现依据

## Reference baseline

- 历史本机可执行文件：`/Users/roylin/.local/bin/magpie` 曾实测 **magpie 0.1.2141**；2026-11-01 已观察到该安装漂移为 0.1.2277，不能继续将它当冻结参考。
- 官方仓库：`https://github.com/yetone/magpie`。
- 契约文档：仓库 `README.md` 与 `docs/reference.md`，于 2026-10-30 阅读。
- 本机 `/v1/models` 实测返回 `object=list`，模型 ID 使用 `provider/model`，并包含 `group/<id>`。
- 参考版本固定为 0.1.2141。升级时必须重新审查协议、策略和配置差异，不能静默跟随 `main`。

## Historical Aster baseline (2026-10-31，以下由本轮复核补正)

- `apps/gateway/src/gateway.rs` 提供 `Gateway::new_embedded`，完整宿主快照和 service-named supplier credentials。
- `apps/studio/src-tauri/src/local_api_gateway.rs` 内嵌生命周期、客户端密钥生成、模型/路由配置投影。
- `apps/studio/src-tauri/src/providers.rs` 提供启用模型、上游地址、协议与凭据。
- `apps/studio/src-tauri/src/lib.rs` 注册 `api_gateway`；`capabilities/*.json` 与 `permissions/api-gateway.toml` 允许主窗口调用。
- 已支持 `provider/model`、`group/id`，并保留 `provider:model` 与裸路由 ID。
- 已有真实测试：未授权 401、授权 Chat Completions 200、模型/上游密钥转发、路由 failover、前端生命周期。
- ElysiaJS 直接依赖、锁文件条目和补丁已删除。

## Gap-to-contract mapping

| Magpie capability | Current gap | Implementation direction |
| --- | --- | --- |
| OpenAI/Anthropic/Gemini 与跨协议流式 | Responses/Messages/Gemini 被实际准入层拒绝；流式/工具/推理对照不足 | 复用翻译与流式状态机，完成授权/计量后逐协议准入 |
| 多账户订阅与故障切换 | 已有多 key 存储，但网关只投影选中 key；有限账户接入未等于通用 provider | 扩展 ProviderStore 候选与实际身份，不重建凭据库、不读取未授权订阅 |
| 全部路由策略与嵌套组 | 固定/ordered 为主 | 用封闭策略类型投影 aster 路由，缺失语义先测试再实现 |
| 命名客户端密钥与模型/成本/token 配额 | 单一本地密钥 | 新建本地密钥授权与原子配额，保留现有密钥迁移 |
| 动态模型目录和按 Agent 可见性 | 启动时静态模型集合 | 复用 ProviderStore 刷新与本地策略，不另建目录 |
| 用量、成本、缓存、会话与配额窗口 | 本地预算未映射 | 复用 model_control 计量，区分 attempt 与最终 request |
| 插件与中间件 | 无 Magpie 兼容运行时 | 独立隔离宿主，明确能力边界，不恢复 ElysiaJS |
| 隐私脱敏 | wire 已有掩码/恢复，但 Studio 未启用 feature，流式跨 chunk/会话稳定性未对齐 | 接通已有能力，修复流式恢复和生命周期，失败关闭 |
| Agent Profile/Library | 覆盖部分 Agent | 扩展现有 CLI config adapters，原子写并保留格式 |
| 局域网共享、远程和同步 | 旧远程配对未迁移 | 使用新客户端密钥授权，旧实现不冒充兼容 |

## In-repo prior art

- `apps/gateway/src/inference/`：授权、路由、翻译、流式与 usage primitives。
- `apps/studio/src-tauri/src/model_control.rs`：本地策略、预算、调度与计量。
- `apps/studio/src-tauri/src/model_accounting.rs`、`model_accounting_usage.rs`：本地用量持久化与预算状态。
- `apps/studio/src-tauri/src/preferences.rs`：ACL preference snapshots、revision 和 commit。
- `apps/studio/src/renderer/pages/code/cliConfig/`：Agent 配置写入、模型寻址与连接匹配。
- `apps/studio/src/renderer/hooks/useApiGateway.ts`：现有 renderer 生命周期和 shared-cache running state。

## Decisions

1. **生产数据面不复制 Magpie 代码或启动其进程**：继续内嵌 aster-gateway；验收 harness 可启动固定版本的参考 executable 做对照。
2. **先对照再扩展**：每个缺口先变成可失败的契约测试，避免以名称相似代替语义一致。
3. **不恢复旧 Node 服务**：ElysiaJS 保持移除；插件隔离与网关服务分开评估。
4. **不静默迁移用户凭据**：旧 `cs-sk-*` 已在首次启动时替换；后续密钥体系必须保留新 aster 密钥及已写配置。
5. **不承诺未验证语义**：实时订阅、全平台、性能与“完美替代”只能在对应证据后声明。

## 2026-11-01 source audit and planning decisions

本次只读检查实现后规划，没有重新运行 Magpie、Gateway、GUI 或付费供应商。下列“存在”来自源码，不代表本次测试通过。

| Decision | 现有文件/事实 | 最小方案与拒绝的替代 |
| --- | --- | --- |
| 固定盘点来源 | git ls-remote 得到 main revision `9162277b91c9327773b5a1e0413d59f24de6f3af`；读取该 revision reference | 保留 binary 0.1.2141，新增文档差异审查；不把动态 main 当冻结基线 |
| 协议实现先于准入 | apps/gateway/src/inference/standards.rs 的 admission、entrypoint.rs 的 standard_not_admitted、inference_tests.rs 的 Messages/Responses 拒绝测试 | 每协议完整 gate；不能因 translation.rs 已有函数直接开放全部模式 |
| 复用多 key | Studio providers_keys.rs 的 keys/selected_key；providers.rs 的 gateway_models | 传递实际候选和 key/account ID；不建第二套密钥库 |
| 不编造 secret store | providers_persistence.rs 已持久化 ProviderStore 数据；Gateway compiled ACL 不包含上游 secret | 原生 owner 继续拥有凭据，导出/日志/用量不泄露；不引入通用 vault 或扩大到全产品凭据迁移 |
| 账户导入仍 opt-in | Core config/account_import.rs 为 Grok/Cursor/Codex 封闭集合；Studio 有派生 Codex | 增加明确来源与 consent；不默认扫描所有产品登录态 |
| 路由不是名称替换 | local_policy.rs 仅 Fixed/Ordered；route candidates 为 ModelReference | 兼容增加策略/组引用；各策略对照，不能把 fixed 当 smart |
| 复用计量 | Studio model_control.rs、model_accounting.rs、model_accounting_usage.rs | 扩 attempt/request 与 reservation；不新增平行记账服务 |
| 隐私存在但不等价 | Gateway wire.rs 的 SSE string-replace shortcut；Studio Cargo.toml 未启用 Gateway wire feature | 先接线与流式恢复；不把“没有完整对齐”写成“无任何实现” |
| 原生文件保留需真实验证 | cliConfig/adapters.ts 使用 JSON/TOML parse/stringify，9 个 file adapters | 逐 fixture 验证未改内容，修复共同编辑层；不维护第二套无头编辑器 |
| 插件能力先做探针 | crates/use/src/plugin_lifecycle 与 plugin_runtime 有现成生命周期/transport | T109 验证 OpenCode/pi/Bun 的实际缺口；不默认可兼容，不新增平行网关 |
| 工具接口先分清 | crates/search/src/providers、Core tools/mcp 有基础；Studio visual_mcp 是人工视觉反馈 | 复用真实 search/media adapter；视觉反馈不冒充图像生成/识别 |
| 时间计算复用 | Studio agent_plane/schedule.rs 有 cron/interval/one-shot 与 fake-time 边界 | warm-up/check-in 复用适合部分；不另起 scheduler 服务 |
| 备份成员需要证明 | Studio BackupService.ts 注明保留 v1 Data/IndexedDB/Local Storage/cache；有 WebDAV/S3 入口 | T121 定义网关真实成员并 round-trip；有 UI 不等于新原生 store 被备份 |
| 资格范围不能猜 | ci.yml 普通 check 仅 ubuntu/macOS；release-aster-desktop.yml build 为 darwin-aarch64 | 原生 Windows/WSL/其他架构另验证，不用 cross compile 代称 runtime |

### 上游当前文档补项

已读取 frozen reference 的 Agents、Routing groups、Signed-in agents as providers、Use，以及顶级章节。Agent 名称和顺序见 [agents.md](contracts/agents.md)。Routing 文档明确包含 weight/fast、stays 的 auto/session/turn/off/strict、group-only listing、decision-only group；不能仅按 README 的五种策略理解。旧 spec 的 free 仍由 T005 核实为 reference 或 Aster 扩展，不能静默删除。

账户盘点还需覆盖 Devin、Qoder/Qoder CN、Cursor、Factory 导入等细项，不仅是 README 列举的五个品牌。文件导入可能刷新/旋转登录 token 并影响原客户端登录，T038 必须在执行前说明实际副作用；本次不读取或导入任何真实文件。

插件补充来源：[plugins](https://usemagpie.ai/docs/plugins) 的 provider/models、auth/refresh/usage/check-in、async request、middleware、tools、Agent、pages、pi、host protocol；意图补充：[intent](https://usemagpie.ai/docs/intent)。站点文档可能随时间变化，T005/T007 冻结适用内容后再进入验收，不自动覆盖旧 golden。

### 不确定性及解决任务

| 未确认项 | 解决任务 | 影响 |
| --- | --- | --- |
| 文档 revision 与 0.1.2141 executable 的精确差异、binary hash | T005/T007 | 不阻塞计划；阻塞相应兼容声明 |
| 全 provider/endpoint/特殊 Agent 字段细项 | T007/T105/T130 | 当前清单是稳定索引，实施前补子项，不能丢记录 |
| 插件 runtime 的 Bun/OS/隔离/现有 API 是否足够 | T109 | 探针后估尾部，不先决定引新 runtime |
| Native Windows/WSL 与每种真实订阅资源 | T071/T127/T037 | 不影响离线开发；资源缺失仍记未验收 |
| 硬成本限额下可验证的最大消费 | T022/T023 | 不确定时拒绝该硬限额请求，不以未知=0 放行 |

## Studio reuse follow-up (2026-11-01)

复用审计与落点详见 [ui-reuse.md](ui-reuse.md)。上一版虽列出 backend owners，但 UI/UX 仅泛指 ApiGatewaySettings/pages/code，且把已有计量状态写成新建任务，复用不够具体，已补正：

- 根 DESIGN.md §10 明确网关链接既有模型、路由、预算、用量编辑器，不拥有重复配置或第二 dashboard。
- LocalModelRoutes/LocalModelBudgets 有组件和测试；实际 ModelSettings 未挂载它们。现有 view query 和网关跳转不能作为可达证明，T031/T024 补接线。
- LocalUsageSettings 已为 Aster 挂载；schema/CSV 有 request/attempt/credential/gatewayClient，accounting 已有 begin/settle/recovery，Core budget 有 admit。任务只补外部调用接线和差额。
- Studio model_control_http 已有内部 Messages/Responses 路径。公开网关 Reserved 与内部能力必须区分；复用不能绕过 internal token 或导致双重转换/计量。
- ProviderSettings 已有多 key drawer、OAuth panel、model sync preview、deep-link import；CodeCli 已有配置草稿/安装升级；WebSearch 已有多 key 编辑。原入口扩展，不重建。
- PluginStore/Capabilities 的 member 数据前置、DeviceConnections 的旧 remote IPC、PersonalLibrary 的资料语义都是复用边界，不能仅按名字或页面存在判定已适用。

以上是源码/调用点复核，本轮未运行 GUI、原生 endpoint 或组件 tests。

### Historical planning-only scope

不改 Rust/renderer、锁文件、workflow、submodule 源码或既有证据；不编译、不安装 runtime、不调用付费模型、不读取真实登录态、不发布、不提交。只做规划产物的一致性和源记录校验。

## Frozen executable and implementation evidence (2026-11-01)

T005 的可执行文件基线改为官方 release 的固定 CLI asset，不跟随用户安装或 `main`：

- Release：[`v0.1.2141`](https://github.com/yetone/magpie/releases/tag/v0.1.2141)，API 发布时间 `2026-10-30T04:33:24Z`。
- Tag/source commit：`eadeb89c48f2420818462227d7687cd7a989a1e5`。官方 API 的 tag 对象、asset 大小和 digest 已重新读取并校验。
- Asset：[`magpie-cli-darwin-arm64`](https://github.com/yetone/magpie/releases/download/v0.1.2141/magpie-cli-darwin-arm64)，59,034,178 bytes；SHA-256 `9b6d64ea0fb62a511536012bc849036feb31cab16078e42a4450be42e3c0c328`。忽略的下载缓存为 `target/local-gateway-reference/magpie-0.1.2141-darwin-arm64`，不把 binary 加入源码。
- 原生环境：macOS 26.6.2、arm64；Rust 1.97.1；Python 3.9.6；复用既有 `openai==2.47.0` / `httpx==0.28.1` 测试依赖。
- 运行：复制 asset 到全新临时 portable 目录，以 `serve` 启动；`MAGPIE_ADDR=127.0.0.1:<临时端口>`、`MAGPIE_NO_ACCOUNTS=1`、`DO_NOT_TRACK=1`，XDG config/cache 指向该临时目录；HOME 保持继承值，不改写。仅传入必要的系统环境和 system PATH。
- 隔离：macOS `sandbox-exec` 拒绝 HOME 下的文件读写，只允许回环网络；在启动 executable 前验证 HOME 不可读、临时 fixture 可读，再校验版本输出 `magpie 0.1.2141`。原生参考执行仅验证了 darwin-arm64。

完整机器可读来源在 Studio `tests/e2e/local-gateway/reference.json`。它同时固定 tag 文档与规划文档 revision `9162277b91c9327773b5a1e0413d59f24de6f3af` 的 README/reference SHA-256。README 的 revision diff 为 +3/-3 行；reference 为 +194/-18 行。新增文档涉及 QwenPaw、T3 的 Codex harness、Qwen Cloud、Vertex Claude、搜索来源、middleware onAccount、客户端 `/me`、Cheapest first 和账户切换/移除语义。文档新增不证明旧 binary 不支持该行为；T007/T032 仍须逐源码/fixture 核实，原规划要求均保留。

旧本机 models/agents 探测仅作历史线索，不能作为隔离验收证据。本次新 corpus 的所有供应商、模型、账户和密钥均为临时合成 fixture；保留原始 corpus 字节及其 hash。仓库格式化后，已逐对象及序列化 HTTP JSON 证明输入不变，记录两种字节 hash。

T003 已建立复用 Gateway unittest/SDK process fixture 的对照 harness，但保持未完成：Aster 的四个 corpus 场景和 401/上游 0 调用通过；双方 JSON/SSE 成功数据一致。参考 429 场景在固定五秒 transport timeout 内分别调用上游 4/5 次后超时；参考本机无 key 请求返回 200/上游 1 次，Aster 返回 401/上游 0 次。源 `internal/gateway/lan.go` 明确放行 loopback，不能据此降低 Aster 的鉴权要求；先前两端 local-auth 等价假设已被反证，后续须按这一来源校准 oracle，并保留反证。T035 继续验证 reference retry/commit 语义，不通过放宽 timeout 或修改 frozen corpus 隐藏差异。

构建、9 项既有 SDK 测试、11 项 managed HTTP 鉴权/限额测试、2 项 Reserved 测试、Studio 本地网关 3 项测试及 ActivityBar 3 项完整 Studio 几何测试的结果，见根 `docs/evidence/local-gateway/2026-11-01-alignment-progress/OUTCOME.md`。这些结果不代表其余协议、Agent、账户、平台或完整交付已验收。

## Full-replacement scope follow-up (2026-11-01)

用户再次明确“必须实现 Magpie 完全替代”。当次官方 latest API 指向 `v0.1.2523`（`2026-11-01T03:10:29Z`），T143 将其固定为本次现行快照并保留旧基线。它是 annotated tag，tag object 为 `bf0cea07e358fcc6220cb8dcad7aa580d599df29`，解引用后的 source commit 为 `fb3942ee67a719a4e4733cdb632873defd42f28b`；不能将 tag object SHA 当 source commit。

- 官方 CLI asset 61,767,538 bytes，SHA-256 `36fc69d1effba0286717bb8ee8c7efe3ff276683595768e01477f8f55bca9570`，已下载验证，未安装。
- 来源与启动候选记录在 Studio `tests/e2e/local-gateway/reference-0.1.2523.json`；版本选择封闭为 0.1.2141/0.1.2523，旧调用默认仍使用原基线。
- 新 source README 与规划文档的 README 字节完全一致；reference 的唯一文档差异是 QwenPaw effort。README 仍有 70 个显式 Agent，主表只列 64 个；AG65–AG70 补齐其余 6 个，保持原稳定 ID。
- `presets.go` 顶层静态预设声明由 58 增至 60；这不是 region/plan/plugin/partner 的完整展开数量。T007 继续逐成员盘点，不能拿这个计数替代供应商验收。
- audio 三端点、rerank、Responses WS、Ollama catalog 和 Magpie 导出迁移不能由旧泛化行验收，新增 T138–T142 与 CAP31–CAP35；共 96 个唯一 task ID、70 个主 Agent ID、35 个 CAP ID。
- 首次快照检查尚未通过 native qualification：先在 Aster management listener 就绪前失败，未启动参考服务；之后本机命令出现启动挂起。后续隔离参考执行器已在 darwin-arm64 真实启动 v0.1.2523，并通过目录就绪、JSON/SSE 成功和独立校准的鉴权检查；固定五秒的 429 对照仍超时，没有修改 corpus、期望或超时。完整兼容仍未通过。真实后续结果见 [alignment evidence 的 Current reference qualification](../../docs/evidence/local-gateway/2026-11-01-alignment-progress/OUTCOME.md#current-reference-qualification-and-scope-inventory)，不能继续将首次未启动概括成当前执行器未验收。

上述是扩大遗漏覆盖和保留基线的实施进展，不是协议或 Agent 已开发完成的声明。T143/T007/T003 均保持开放。

2026-11-02 完整源码复核：重新获取官方 release/tag/commit/tree，重开并验证 0.1.2141 的 6,772 个 blob 与 0.1.2523 的 7,305 个 blob；递归树均未截断，所有目录对象也按 Git 编码重算并匹配。旧二进制基线→新版为 2,251 个文件变化（新增 539、删除 6、修改 1,706），不能使用 GitHub compare 截断的 300-file 列表代替。规划源码 `9162277…`→新版的完整差异为 18 commits / 53 files，已逐文件归入 13 组行为，并记录 owning task、既有 UI owner、fixture/native 要求。范围包含 QwenPaw/ZCode、进程与价格查询、Bili headers、Kimi schema、视觉描述取消/去重及账本归因、Library 空记录、Windows 图标与 Unix handoff；不只是 README 的 QwenPaw 增量。全部证据与可离线复现的树/差异校验见 [source audit](../../docs/evidence/local-gateway/2026-11-02-reference-source-audit/OUTCOME.md)。旧基线的其余语义审计、实际适配和跨平台证明未完成，T143/T007/T003 继续开放；未切换旧 corpus 的默认基线。


## 现有 UI 融合与执行 owner 复核（2026-11-01）

用户要求复用现有 UI，并确保有效、高效且没有重复功能/设计。本次在原计划中补充
§4.1、§4.2、§5.1、§7.1–§7.2；不另建 UI 计划或任务体系。

| 已核对来源 | 事实 | 计划约束 |
| --- | --- | --- |
| Studio local_api_gateway.rs 的 compile_gateway/render_model | Gateway snapshot 生成直连 provider targets；grants 预先列出多个协议 | 公共准入和 UI 不能据此宣称所有协议可用；本地执行链优先收敛 |
| Studio model_control.rs 的 dispatch_using | 已有本地 route candidates、能力检查、accounting begin/mark_dispatched/settle | 接入同一执行 owner，不新增另一份本地调度或预算扣减 |
| model_control_http.rs 的 infer、DispatchInput、BuiltinSource | 内部入口校验 host token；built-in 来源闭集；controller 的 gateway_client_id 当前为 None | 传真实外部身份，禁止冒充内置调用或把 internal token 当外部 key |
| Core llm/metering/wire.rs、Gateway sse.rs | 已有 SSE/usage 实现，但 carry/事件边界和上限不同 | 复用已有 owner 并补边界契约，不能再加第三套，也不能通过缩小支持范围取巧 |
| useApiGatewayProvider.ts | renderer 固定声明三种 endpoint | 改由同一 native 能力快照驱动模型选择、Code 和连接说明 |
| useProvider.ts、useLocalUsage.ts、localUsage.ts | 既有 query 失效、generation、分页和每页 50 条的读取 | 继续用原缓存/分页，不增加平行 cache、全表前端汇总或组件各自轮询 |
| settingsMenu.ts 与 SettingsPage | 原网关页面同时被导航和搜索隐藏；深链会回到通用 | 已完成的导航修复与浏览器流程另留 evidence；后续每个入口都要实测 |
| ApiGatewaySettings 与 SettingsPage 的页面标题 | 当前页面出现同名标题重复 | 由原 settings shell 负责页面标题，子组件只展示功能内容 |
| model_control.rs 的 RequestRequirements::refusal 与 dispatch_using | 不同协议直接拒绝；endpoint 由 resolved model 提供，但认证、响应校验及 meter 当前按 input.protocol | U0 不能仅桥接内部 HTTP；复用既有 codec，分别保留客户端/上游协议，先证明原有 JSON/SSE 不回退，见 plan.md §5.2 |
| settingsMenu.ts 与 DESIGN.md §10 | API 网关目前在 agent 分组，设计要求归连接 | 在原导航修正分组并同步搜索/回归，不增加第二个导航项 |
| LocalGatewayModelLinks.tsx | 卡片读取启用模型总量，尚非按客户端实际授权/协议准入过滤 | 保持配置总量含义；如显示可调用量，则复用同一 native 可用性投影 |

性能数值是计划准入目标，不是测量结果。固定工作量、构建模式和阈值后再运行，
完整调用链必须先通过功能门。单一 owner 是每个业务对象/本地请求的权威，不要求
错误合并上游 key 与客户端 key，也不取消 standalone Gateway 原有产品职责。
