---
description: "Magpie 全功能业务对齐任务；每项行为同时交付 hermetic test"
---

# Tasks: Magpie 全功能业务对齐

**Updated**: 2026-11-03。

> **2026-11-03 产品范围收敛（优先于下列历史对齐计划）**：用户明确 Aster 不需要支持第三方 harness。本任务只建设 Aster 自身 Agent/ACP 及模型、供应商、协议、网关、用量等通用能力；不再为 Aster 开发 OpenClaw、DeepSeek Harness、Hermes 或其他第三方 CLI 的托管、配置同步、安装及启动界面。原第三方运行器对齐条目停止推进，不标记为已实现或已验收。冻结参考清单和历史证据保持原样；共享宿主中其他租户的现有代码不等于 Aster 的产品功能。当前入口与构建收敛证据见 [OUTCOME](../../docs/evidence/desktop/2026-11-03-harness-retirement/OUTCOME.md)。
**Input**: [spec.md](spec.md)、[plan.md](plan.md)、[research.md](research.md)、[data-model.md](data-model.md)、[功能清单](contracts/capabilities.md)、[Agent 清单](contracts/agents.md)、[Provider 清单](contracts/providers.md)、[协议契约](contracts/gateway.md)。

**House rule**: 行为、真实实现的 hermetic test、调用方、权限、迁移和文档一起完成；未记录验证不得勾选。Rust 单元测试新增 sibling 文件，公开 API 用 tests/。这里的候选测试文件是待创建目标，不表示已经存在。

**Studio reuse gate**: 所有 UI/数据接线任务必须遵守 [ui-reuse.md](ui-reuse.md) 的原入口/组件/hook/native owner 映射，先扩现有测试。组件存在但未挂载记接线缺口；不重建已有编辑器、账本或 reservation 引擎，不以复用 managed UI 为由引入本地产品账号。

**编号/依赖**：保留原 T001–T074 的既有编号与语义，新增 T100 起。T005–T007、T016–T019、T025–T026、T037–T039、T045–T047 是原空号补项。T001/T002 的 [x] 仅保留原历史，T006 复核。未标 [P]：各阶段共享配置、模型、entrypoint 与用量状态，不默认并行修改。各阶段按列出顺序，显式“前置”优先。

## Phase A — 基线、清单与对照

- [x] T001 [US1] 保留 apps/gateway/src/gateway.rs 的 new_embedded 与 apps/studio/src-tauri/src/local_api_gateway.rs 生命周期；已有真实鉴权 401/Chat 200 测试，T006 复核。
- [x] T002 [US1] provider/model、group/id 与 legacy provider:model/bare route alias 已实现；寻址测试与文档保留，T006 复核。
- [x] T005 [US1] 先在 research.md 固定 Magpie 0.1.2141 executable 版本/哈希、OS/架构、启动参数和文档 revision；记录新旧差异，保留旧 corpus；不得读取真实账户作为默认 fixture。2026-11-01：官方 asset/tag/hash 与隔离版本验证通过；来源见 Studio tests/e2e/local-gateway/reference.json，证据见 docs/evidence/local-gateway/2026-11-01-alignment-progress/OUTCOME.md；T003/T007 的未验收差异仍保留。
- [ ] T143 [US1][US8] 按用户 2026-11-01 的“必须实现 Magpie 完全替代”要求，冻结当次官方最新 v0.1.2523 的 release/tag commit/asset/hash、文档和完整功能增量，保留 0.1.2141 的全部 corpus/证据；复用隔离 harness 增加该版本，逐项补 owning task/fixture/native 要求。不得静默换旧 fixture，也不把后续动态 main 当移动验收目标。
- [ ] T007 [US1][US8] 按 contracts/capabilities.md、agents.md、providers.md、ui-reuse.md 与 frozen reference 补齐 endpoint/provider/账户/Agent/平台和具体 UI owner；核对实际路由挂载/数据来源，记录并验证 ModelSettings 的 routes/budgets 挂载修正、managed 插件数据前置、旧 remote IPC；每项写 test/evidence 目标，校验数量/顺序，未测不标 N/A。
- [ ] T003 [US1] 在 apps/studio/tests/e2e/local-gateway/ 建双方对照 harness，复用现有 runner；本地 upstream 覆盖一个 JSON/SSE 成功与错误案例，固定归一化字段，保存脱敏差异；无 Magpie 时明确 corpus-only。
- [ ] T004 [US1] 将 contracts/gateway.md 映射到 owning Gateway/Studio tests；为每个 P1 缺口记录可失败 fixture，校验事件/工具/usage/错误而非关键字；旧授权回归单独保留。
- [ ] T006 [US1] 复核 T001/T002 并修正 apps/studio/docs/references/api-gateway/README.md 对 Reserved 的过度声明；对照 standards.rs、entrypoint.rs 与现有拒绝 tests，记录修改前基线；文档链接/构建检查通过。

## Phase B — 协议与流式，G1

前置：A。先完成 plan.md U0 的本地执行/计量接线，再按 T010→T011→T012→T013 逐条交付。每条同时验证已有 UI、真实外部请求和同一用量事实；其余在相关协议完成后接入。

2026-11-01，T011/T014 的输出/传输切片已推进：复用同一 accumulator 处理 JSON/SSE、工具/推理/拒绝、trailing usage 和有界 SSE；已补内容完整的并行工具历史/参数映射、自产推理/拒绝结果的历史回传；修正跨协议上游路径并验证四种 JSON/SSE 主备组合。转换和计量共用 8 MiB 有界 SSE 分帧，转换用量改读上游原始字段，并保留客户端响应寿命内的配额 guard。证据见根仓库 `docs/evidence/local-gateway/2026-11-01-alignment-progress/OUTCOME.md`。公共 Responses 仍 Reserved；网关导航/搜索/深链已恢复，48 个 renderer tests、网关→既有路由/预算→保存返回及 13 个设置入口的浏览器流程已通过。它们未证明原生请求/硬预算生效。完整 HTTP dispatch、原生路径/状态/内容、能力/计量与 T018 验证未完成，不能据此勾选任务或声明 G1。

- [ ] T010 [US1] apps/gateway/src/inference/openai_request.rs、entrypoint.rs 完成 models/Chat 的真实 listener 回归；hermetic cases 覆盖 JSON/SSE、工具、usage、错误、边界 body、401 不触达上游及所有 alias；不改固定 grader。
2026-11-01 后续 HTTP 切片：公开 POST Responses 已从 Reserved 接入同一宿主 ModelControl，旧分支撤回对应 grant，预算/账本/候选/取消不新增 owner。真实 HTTP 反例已复现原 404 和原生 SSE 400；补充原生/Chat 转换 JSON/SSE、工具/推理、状态续接、预算、取消及已输出后不重放。另修复宿主 SSE 多空行、保留脱敏后的 Responses 失败事件，并按原契约隐藏 principal 的未授权/缺失资源。公开宿主 20 项、共享模型控制 47 项、ProviderStore 7 项、Gateway Responses 38 项及 inference 262 项检查已通过，计数有重叠；详见根仓库 `docs/evidence/local-gateway/2026-11-01-responses-http/OUTCOME.md`。T011/T018/T140 与所有 G gate 保持未完成。

- [ ] T011 [US1] standards.rs、openai_request.rs、translation.rs、entrypoint.rs 实现 Responses 准入/dispatch；Studio 本地模式接既有 ModelControl，复用协议 codec 和真实用量观察，不追加一套本地调度/计量实现；sibling tests 覆盖原生与 translated JSON/SSE、tool IDs/参数、reasoning、终止、usage、previous_response_id 的支持或明确拒绝，不静默丢字段。

2026-11-01，Messages HTTP 切片：公开 `/v1/messages` 和独立授权的 `/v1/messages/count_tokens` 已接同一 ModelControl/ProviderStore。原生及 Chat 转换的 JSON/SSE、system/tool/thinking/signature、原始用量、取消、预算、重试与已发工具事件后不重放已验证；counter 只尝试同模型 key，明确区分 upstream/estimated，不产生生成账本或 spool 消费。修复客户端和 Provider 配置导致的重复 `anthropic-version`，保持其他宿主接口原鉴权范围。最终公开网关 33 项、共享控制 48 项、实际 ProviderStore 7 项通过；保留 Gateway inference 262、translation 43、usage 82 与 principal 4 项记录，计数有重叠。两包 Clippy、ToYou 编译和定向格式通过；根格式与全量文档检查仍有其他路径失败。证据见根仓库 `docs/evidence/local-gateway/2026-11-01-messages-http/OUTCOME.md`。完整 T012/T017/T018、Gemini、WS、SDK/平台/性能与各 G gate 均未完成，96 项任务勾选状态不变。

- [ ] T012 [US1] 同一 Gateway 入口实现 Messages/count_tokens；真实 HTTP/SSE fixture 覆盖 system/content blocks、tool_use/result、thinking/signature、stop/error、headers 和 token 计数，不仅复用 Chat 名称。
- [ ] T013 [US1] 同一 Gateway 入口实现 Gemini models/generateContent/streamGenerateContent/countTokens；native Rust fixture 覆盖路径 model、query、parts/tools、thought/signature、finish/usage/error，禁止恢复 ElysiaJS。

2026-11-01，T013/T016 的原生上游内核前置：Core WireMeter/SseMeter 新增 Gemini，分开归入工具输入与思考输出，保留缓存和最终用量，EOF 后核验已见候选终止。Studio 既有 ModelControl 接通原生 URL/method、查询参数、x-goog-api-key、内容/签名、能力/状态绑定、预算和取消，未增加第二套执行/计量。补充反例后修复未完成/矛盾 JSON 响应仍被当成功，以及损坏用量快照覆盖既有消费的问题；快照整体通过后提交的约束扩到四种协议，显式总量可恢复确定的缺失分量。固定本轮 10 个实现/测试源码哈希后，Core 计量 22 项、公共 SSE 回归 6 项、共享控制 57 项、现有公开网关 33 项、两包 Clippy 与 ToYou 编译均通过，计数有重叠。根格式检查仍有其他路径失败。公开 Gemini 路径仍 Reserved；ProviderStore/私有 HTTP 挂载、模型目录、计数、跨协议 codec 与 UI 能力投影尚未接通，T013/T016/T017/T018 和各 G gate 均保持未完成。证据见根仓库 `docs/evidence/local-gateway/2026-11-01-gemini-native-kernel/OUTCOME.md`。

- [ ] T014 [US1] Gateway translation/dispatch 实测所有声明支持的 caller→upstream 组合；全 chunk 切分、并行工具、推理、取消、超时、首响应前后故障；记录承诺后重放 0，并验证 upstream 任务释放。
2026-11-01，T013 的 Gateway 原生边界切片：新增 URL 模型/方法描述符，复用既有鉴权、目标选择、HTTP/SSE 和用量观察器；计数与目录使用独立 grant，计数不生成消费。有效原生内容保留字节，路径别名、实际 API ID、查询密钥、Google 错误与分页均有专门 fixture。最终 20 项专项包含官方 Google SDK 1.52.0 到真实测试监听器的 JSON/SSE、目录、计数、工具/签名和 401/403 验证；基础推理 414 项、路由边界 1 项、用量复跑 159 项通过，计数有重叠，含 wire 的全目标 Clippy 通过。保留用量首轮 6 项失败和分布式模块并行 14 项失败；单项通过尚未解释并行拒绝，不能宣称全回归通过。Cloud 用量协议暂不能接收新 endpoint，相关生成与 principal 路由保持关闭。生产 Studio ProviderStore/私有 HTTP/UI 尚未接通，完整任务与平台/真实账户验收未完成。96 项任务状态和 G1/G2/G3 不变。证据见 `docs/evidence/local-gateway/2026-11-01-gemini-public-boundary/OUTCOME.md`。

- [ ] T016 [US1][US4] Gateway config 与 Studio providers.rs 接通协议、输入类型、tool/reasoning/effort 能力；未知/不支持组合在入口明确拒绝；fixture 验证不靠模型名猜能力。

2026-11-02，T013/T017 的 Studio 原生宿主前置切片：既有 LocalModelsController 和 embedded Gateway 绑定已加入 Gemini 模型/路由 generation、stream、count；同一 ModelControl 负责真实重试、预算与账本。官方 Google SDK 1.52.0 经真实测试监听器调用宿主，验证目录分页、两种别名、实际 API ID、JSON/SSE 工具签名、计数与 401/403；两次生成各 23 tokens，计数不产生生成消费。公开入口 42 项、共享控制 51 项、内置链路 8 项、translation/estimator 45 项、Core 计量 35 项通过，计数有重叠；三包全目标 Clippy、ToYou 编译和定向格式通过。新增反例先确认 Core 缓存上限误含工具输入，再修复 base prompt 的直接/推导计数约束。保留路由语法、新断言及无关 Core 测试编译的初次失败，根格式仍有 8 个其他路径失败。生产 ProviderStore/Google runtime/UI 继续关闭：Core 尚不能保留并重放 opaque 原生工具上下文，不能把测试 lookup 当作产品可用；候选数量、protobuf 字段能力分类、跨协议、Cloud 与平台/真实账户仍待完成。96 项勾选与 G1/G2/G3 不变。证据见 `docs/evidence/local-gateway/2026-11-02-gemini-native-host/OUTCOME.md`。

2026-11-02，T013/T016 的 Core 原生前置：显式 `protocol = "gemini"` 已接既有工厂、HTTP/retry/SSE meter 和消息 serde；保留有序 parts、opaque signature、原生 call IDs、工具/图片结果和既有文本/工具投影，按真实 provider/model 与投影摘要绑定，其他协议在上游 0 调用时拒绝该状态。旧昵称的默认协议保持不变。launcher 复用私有宿主与 ModelControl，以宿主凭据写 ACL，真实 provider ID 用既有 headers 传递且不转发给 vendor。真实 loopback fixture 经生成 ACL、Core 客户端和宿主完成两轮调用；恢复消息并换端口重建宿主后签名仍正确，两条成功记录共 46 tokens。原生 21 项、既有文字工具恢复 10 项、compaction 36 项、宿主链路 9 项通过；Core/Studio 全目标 Clippy、CLI/ACP 编译与 ToYou 编译通过。新反例先复现放弃响应头 future 后未取消传输、原生示例被文字恢复推断为额外工具调用，再修复；正常完成/失败不取消父 token 也有断言。更广 LLM 回归为 649 通过、4 失败、8 ignored，4 项各自单跑仍失败，涉及既有 effort 推断与 OpenAI/Codex CR 分帧；未改原断言、输入或超时。生产 ProviderStore/Google UI、完整能力投影、状态密钥亲和、多候选、Cloud/平台/真实账户与各 G gate 仍未完成，96 项勾选不变。证据见 `docs/evidence/local-gateway/2026-11-02-gemini-core-native/OUTCOME.md`。

2026-11-02，T013/T017 原生能力准入修复：共用入口改为借用式类型投影，补齐字段别名、工具结果图片和 opaque 状态绑定，冲突别名与无法分类的媒体在联网/记账前拒绝，原始转发 JSON 保持不变。共享控制 55 项、公开网关（含官方 SDK）42 项、宿主 9 项、原安装能力回归 1 项通过；Studio 全目标 Clippy、ToYou 编译和定向格式通过。生产 ProviderStore/Google UI、多密钥续接、完整模态/协议兼容及原有失败仍待完成，任务勾选与 G gate 不变。证据见 `docs/evidence/local-gateway/2026-11-02-gemini-capability-admission/OUTCOME.md`。

2026-11-02，T013/T015/T017 的生产提供商接线：Gemini 已接既有 ProviderStore、ACL 重开、原生分页目录、连接探测、内置文本与前端本地适配器；同一公开网关已用真实保存配置完成 JSON→SSE 签名工具续轮并核对原账本。最终 Gemini 33 项、前端 38 项通过，Studio Clippy 与 ToYou 编译通过。完整提供商回归仍有 2 项旧断言失败，全量前端类型/根格式在其他路径失败；多密钥续接、Code 网关能力快照和真实桌面流程未完成，所有勾选与 G gate 不变。证据见 `docs/evidence/local-gateway/2026-11-02-gemini-provider-store/OUTCOME.md`。

2026-11-02，T013/T017/T044 的 Gemini 状态密钥亲和：复用原 SQLite 账本保存作用域、状态引用和实际 vendor/key 绑定的指纹，已知签名可在主密钥失败后继续使用成功密钥，数据库重开后仍有效；替换同 ID 的实际密钥、混入冲突历史会在新上游调用前拒绝。状态写入在 JSON/SSE 输出前提交，写入失败不切钥重放且保留已观察消费；计数沿用同一绑定，不生成消费。路由先做能力/预算准入，实际尝试后才去重同一绑定，既允许可用别名也避免重复请求。最终共享控制 63 项、账本 14 项、公开网关（含真实 ProviderStore 单/多 key 和官方 SDK）43 项、Core/宿主 9 项共 129 个不同测试通过；Clippy、ToYou 编译与定向格式通过。15 个固定输入保持原哈希，最终根格式仍有 6 个其他路径失败。真实账号/OAuth、完整能力/跨协议/桌面/平台/性能和各 G gate 未验收，96 项勾选不变。证据见 `docs/evidence/local-gateway/2026-11-02-gemini-state-affinity/OUTCOME.md`。

- [ ] T017 [US1] Messages/Gemini count 和 models discovery 的独立授权/错误/计数来源；fixture 覆盖无 alias、未知模型、缺 tokenizer/上游失败及 query，估算与真实值可区分。
- [ ] T018 [US1][US3] 协议准入联动 inference authorization、managed/principal 与 reserved tests；新增成功请求与无 grant/超限/隐藏资源/快照撤销反例，保持既有控制面契约且不修改 control-plane；不得仅删 Reserved 断言。
- [ ] T019 [US1] completions/embeddings 及非 chat 模型的宿主投影，验证选对 endpoint/模型、维度/usage/错误；未声明能力拒绝；Chat-only catalog 不得伪称 embeddings 已可用。
- [ ] T139 [US1][US4] 为冻结来源的 `/v1/rerank` 与适用 alias 实现实际 native Rust 入口、模型投影/授权和计量，复用现有 rerank provider；fixture 比较 query/documents、排序索引/relevance score、top_n/return_documents、usage/错误，验证不作为 chat 派发及未知能力/越权上游 0。
- [ ] T140 [US1][US3] 在 T011/T018 的同一 Responses owner 上实现 frozen 新版本的 WebSocket 行为；真实 WS 客户端验证握手前鉴权、create/continuation/tool/推理事件、usage、断开/取消、会话隔离和已承诺后重放 0；兼容 HTTP/SSE，不能用普通透传绕过 inference grant/配额。
- [ ] T141 [US1][US4][US5] 同一 Gateway catalog owner 提供 Magpie 的 Ollama-compatible `/api/version`、`/api/tags`、`/api/show` 与 agent-specific discovery 元数据；验证精确模型能力/context/effort、仅目录用途及 `/api/chat`/`generate` 的参考拒绝契约。对不支持 bearer 的客户端提供经验证的配置/认证适配，不能全局免鉴权；端到端读取目录并真实调用所选模型。
- [ ] T015 [US1] Studio local_api_gateway.rs、useApiGateway.ts、ApiGatewaySettings 与 permissions/capabilities 完成 native 生命周期：start/restart/stop/开机恢复/端口冲突/坏配置/模型变化；native 提供同一运行/协议能力快照，Code、目录和网关 UI 读取它，替换固定手写支持列表；UI 测试和真实 HTTP 请求同时验证状态，不以窗口启动验收。

2026-11-02，T015/T017/T105 的 native 协议快照与 Code 接线：既有网关命令/共享缓存提供实际监听地址、基础协议准入及 stored/API ID，投影复用 ProviderStore、ModelControl 兼容判断和运行中实际路由/grant；Code 不再固定宣称三种协议。模型选择、预览、保存和启动沿用同一已准入模型 map，避免重读原模型资料后写错协议/API 别名；上游原生协议优先，缺 ModelControl 或停用模型时不误报可用。Gemini CLI 的既有 `@cherry` 后缀已在原调用路由接通，目录仍仅显示原两个标准别名；配置统计卡片和编辑入口保留原含义。最终公开 HTTP/SDK 49 项、共享控制 63 项、宿主 9 项共 121 个不同 Rust 测试、完整 Code 与相关前端 716 项通过，Clippy、ToYou 编译和定向 lint/格式通过。全量类型检查仍有 263 条其他路径错误，根格式有 8 个其他文件失败；i18n 原有键顺序及 18 个其他键的占位符问题保留，网关文案不再承诺任意 CLI/全部模型。15 个固定输入不变。完整生命周期、虚拟路由模型/非 chat 投影、真实 CLI/安装版/账号/平台/性能与 G gate 未验收，96 项勾选不变。证据见 `docs/evidence/local-gateway/2026-11-02-gateway-native-snapshot/OUTCOME.md`。

## Phase C — 客户端身份、密钥、授权和硬配额

前置：A；HTTP 接线依赖 B。先 T020/T026 确定身份，再 quota。每个切片同时接既有 UI 和真实调用，按 plan.md §5.1 验收，不把 UI 留到整个阶段末尾。

- [ ] T020 [US3] Studio Preferences/local_api_gateway.rs 与 Gateway credentials/grants 实现 named client key id、generation、轮换/撤销；保存 verifier，原单 key 兼容迁移；tests 覆盖旧 key 失效、新 key 授权、重启和 Debug/导出无 synthetic secret。
- [ ] T026 [US2][US3] Studio providers_keys.rs/providers.rs 复用现有 multi-key identity，并让 compiled target 保留 key/account id；tests 覆盖启停、legacy key 迁移、顺序和不同 key 不串凭据；不建立第二 store。
- [ ] T021 [US3] Gateway model/endpoint grants 与 Studio key 配置贯通；tests 覆盖未授权上游调用 0、未知/隐藏资源按现有拒绝契约处理；可区分的本地场景按参考返回 403，不削弱 principal 的 404 隐藏契约。
- [ ] T022 [US3] Studio model_control/model_accounting 和 Gateway 入口实现 client/account/global 日/周/月 token/cost policies；typed period/timezone/currency，tests 覆盖边界重置、价格未知/消费上界不明时硬预算拒绝和 429/reset 信息。
- [ ] T023 [US3] 将外部 Gateway 经宿主类型化适配接到现有 ModelControl dispatch 与 accounting begin/mark_dispatched/settle/recover_interrupted、Core budget/local admit；消除本地模式直连 provider 的平行执行链。按 plan.md §5.2 保留已有跨协议 JSON/SSE，复用既有 codec，实际 endpoint/认证/原始 usage 按上游协议；不能只转内部 HTTP 或取消协议检查。传入实际 client/source/session/request 身份，不冒充 built-in 或外泄 internal token；只扩已证实缺少的状态/接线。真实 UI+HTTP/并发 tests 验证相同预算、最后额度、重试、缺 usage、取消/断流、四种 alias 与 stored/apiModelId 不同，证明一次业务选择/转换、一次 request final 与每个实际 attempt 仅结算一次。
- [ ] T025 [US3][US4] reservation/identity 的重启恢复与轮换历史；临时 store/注入 clock 测试跨周期、崩溃中断、重复提交和已删除 key；原始 secret 不入日志/报表。
- [ ] T024 [US3] ApiGatewaySettings 只接客户端 key 的创建/轮换/撤销/模型范围，关联预算跳 ModelSettings 的 LocalModelBudgets；补现有预算组件挂载并按原 side panel 扩 scope/周期，沿用 IPC/permissions；路由测试覆盖网关跳转→编辑保存→返回共享数据，授权外不生效。

## Phase D — 订阅、多账户和路由

前置：B、T020/T026；预算相关依赖 C。顺序先身份和 provider adapters，再选择策略。

- [ ] T030 [US2][US4] Studio providers*.rs 将多 key/账户候选显式导入 gateway_models；来源遵守 Core account_import opt-in；fixture 覆盖不同账户、租户/instance、缺授权、旧 ACL 和不串 headers/secret。
- [ ] T037 [US2][US4] 每个冻结订阅建立独立 adapter/fixture：Claude 本地 CLI+caller tools MCP bridge、Codex/ChatGPT、Copilot、Gemini/Antigravity、Grok/Cursor、Devin/Qoder/Qoder CN 及 T007 确认其余来源；覆盖登录/登出/模型/工具/非流式适配；真实凭据另验，不读开发者默认状态。
- [ ] T038 [US2][US4] providers 与既有登录 adapter 完成 refresh、每 key/account proxy、project/organization/seat headers、过期/冷却/退出；fixture 模拟 rotated token 和失败恢复，读文件导入的副作用须在执行前明示，不能复制未知用户状态。
- [ ] T031 [US2] Core local_policy.rs/Gateway 扩 fixed/order/rotate/usage/pace/smart；Studio 在 ModelSettings 挂载已有 LocalModelRoutes，扩其 SearchSelect/DefaultModelSelector/草稿侧面板，不在网关页复制；策略 fake-clock/quota fixture 与实际入口路由测试通过，smart/pace 无观测按显式 fallback。
- [ ] T032 [US2] 同一策略链实现 weight/free/fast；T005 核实参考，weight 保留精确约束，fast 用真实 TTFT/速度观测，free/price 用明确来源；缺信息拒绝/显式 fallback；tests 不硬编码赢家。
- [ ] T033 [US2] 兼容扩展 local_policy 的 model/group refs，Gateway 只展开显式成员；tests 覆盖 nested/空组/循环/未知引用/展开上界/legacy schema；公开 Rust API 不被破坏。
- [ ] T034 [US2] Gateway session/turn/cache affinity 与 stays 策略；tests 覆盖 auto/session/turn/off/strict、不同组/会话隔离、重试和限额；strict 不偷偷切账户。
- [ ] T035 [US2] Gateway failover 使用 typed status/code 与参考 retry set；真实两上游 tests 覆盖 401/403、429/5xx、连接/超时、首语义响应前后、空/损坏响应；承诺后不重放。
- [ ] T036 [US2][US4] dispatch→usage→UI 传真实 account/key/attempt id；测试首选失败/备用成功/多重试，request 计一次，所有实际 attempt 消费留存；路由解释不含凭据。
- [ ] T039 [US4] Studio provider quota/window adapters、提醒与 opt-in warm-up/check-in，复用现有 agent_plane/schedule.rs 时间逻辑；fake clock/subprocess fixture 覆盖五小时/周窗口、credits policy、禁用/错过/重启，真实消耗另记证据。
- [ ] T100 [US2] local_policy/Gateway/路由 UI 完成显式 agent/长度/图像/reasoning 条件、模型 intent classifier、effort override 和 turn trace；fixture 覆盖超时/非法分类、工具续轮不重选、subagent 隔离；禁止正则/词表猜意图。
- [ ] T136 [US1][US2] 前置 T031/T033/T100，盘点并对齐 frozen reference 的 decision/System One 模型与组；复用 Gateway standards/平台入口，fixture 覆盖 choice/noul/score、组路由与 quota、拒绝混合 chat/decision 和不适用规则；不改变既有平台模型契约。

## Phase E — 目录、价格、用量和会话，G2

前置：B–D；元数据盘点可提前，实际归因必须等 T036。

- [ ] T040 [US4] ProviderStore/model registry 与活跃 Gateway 快照刷新，先保留启停/真实 id/旧 alias；fixture 验证供应商局部变化、失败保留有效快照、在途流/下一请求 revision 和重启，不静默空 catalog。
- [ ] T130 [US4] Studio provider import 边界支持冻结参考来源（CC Switch/Claude/Codex/Alma/OpenCode）与网站配置链接；复用当前 provider registry，不加入产品配置发现；临时文件 tests 覆盖预览确认/忽略/重复/collision/headers、循环指回网关/仅 env key 的拒绝和损坏输入。
- [ ] T046 [US4] providers_pricing/model_accounting 对齐自定义与目录价格、币种/版本、cache read/write、reasoning、有效价格；fixture 验证未知不是 0、token tiers、partial estimate 和实际 provider/key 账单。
- [ ] T047 [US4][US5] catalog/cliConfig/模型 picker 接通 per-Agent visibility、only-picked/show-new、group-only 列表与 canonical model alias；tests 验证展示与授权区分、订阅例外、模型改名不串历史。
- [ ] T041 [US3][US4] model_control 的预算语义与 C 的硬配额统一：提醒/停止/未知消费有明确定义；真实 gateway fixture 证明超过硬限额不调用上游，通知不得冒充强制停止。
- [ ] T042 [US4] 在已有 model_accounting/localModelUsage 的 request/attempt/credential/gatewayClient、usage/cost/cache/TTFT/速度/status/time 结构上接外部调用，只补确切缺列/状态；沿用 LocalUsageSettings/useLocalUsage/localUsage.ts 与原 CSV；Gateway transport/spool 只提供对应追踪，不形成第二份本地消费汇总。内置与外部各发实际请求核对一次 final 和所有真实 attempt；临时库 tests 覆盖失败消费、未知 usage、重复完成、重启/取消，不另建账本。
- [ ] T044 [US4] 同一 owner 接通窗口/历史身份/会话与 key 删除轮换；tests 覆盖旧数据、迁移中断、窗口重置和真实账户归因不被后续配置改写。
- [ ] T043 [US4] 用量页/导出/OTLP 按 client/model/account/key 聚合，复用现有观测输出；fixture 比较请求与尝试合计、cache/reasoning、脱敏与导出失败，synthetic secret 出现 0。
- [ ] T045 [US4][US8] 会话标题/成本/context 构成/cache 命中与续开，复用现有 session/terminal launch；fixture 覆盖无 ancestry/remote-only/未知名、重命名、未安装终端、空格/非 ASCII 路径和真实启动参数；不猜会话名称。
- [ ] T137 [US3][US4][US7] 既有 Gateway/Studio 管理面提供功能等价的客户端 limit、quota cards/refresh/wait、route/concurrency、分页 history、外部 usage 提交、Codex 目录及管理诊断；fixture 覆盖 client 只读自身、管理权限、分页/刷新限流/取消、usage 去重与不可信提交不能修改权威费用；不要求复制 Go 专属 profiling 数据格式。

## Phase F — 隐私与可检查性

前置：B、C、E。顺序先出站闭环，再设置页。

- [ ] T051 [US5] Gateway wire/Sentry 与 Studio gateway 编译/调用链接通 opt-in mandatory masking；真实 upstream fixture 验证请求 secret 只占位符、回复/tool arguments 回原值、不可用拒绝，生成 ACL/日志/磁盘无 synthetic secret。
- [ ] T101 [US5] wire 流式有界恢复与稳定映射生命周期；全切分点/UTF-8/SSE/tool JSON/signature/cache fixture，覆盖跨 turn、重启后不可恢复、上限、取消和长流；禁止逐 chunk replace 冒充实现。
- [ ] T102 [US5] 现有隐私策略扩展 secrets/PII、Mask/Warn/Block、Never mask、自定义明确值/机械模式和可选本地模型；tests 验证优先级/示例域名例外/缺模型失败；人名地址走模型，不扩猜名词表。
- [ ] T103 [US5] 现有设置/用量界面接通 Try it、类型/计数/placeholder/请求记录与开关；UI/IPC 权限 tests 证明历史不含原始值、取消不保存、显示和实发一致。

## Phase G — 外部 Agent、Profile、Library 与版本管理

前置：B、C、E；插件扩展注入在 H 完成后验收。逐 Agent 任务 T105 可拆成顺序子切片，保持清单不丢行。

- [ ] T104 [US5] cliConfig/parser/builders/adapters/file 与既有 native 写文件 API 支持最小字段编辑；各 JSON/JSONC/TOML/YAML/dotenv fixture 保留注释/顺序/缩进/无关字节，原子替换/权限错误/并发冲突可恢复；不创建第二套编辑器。
- [ ] T105 [US5][US8] 按 contracts/agents.md 的原顺序为每个 Agent 完成 exact config path/fields、安装实例检测、模型/effort/小模型/特殊 flags 与接管撤销；每项有 fixture/status；已有 9 adapters 同样复核，剩余不得以通用 URL 当完成。
- [ ] T052 [US5] cliConfig/Preferences 实现全 Agent Profile 保存/应用/删除/当前标记及模型可见性；真实多文件 fixture 覆盖中途失败回滚、用户并发编辑和新旧 model id，不复制登录态。
- [ ] T107 [US5] Library 分发 instructions/MCP/skills/subagents，复用现有 capabilities 与配置 adapters；temp-HOME tests 覆盖 idempotent apply、source 更新、同名冲突和仅删除托管字段/文件，用户内容保持原样。
- [ ] T106 [US8] 现有 managed-tool 状态/安装管理路径接通版本检测、安装来源与升级、缺工具命令；stub executable fixture 验证 argv/env/cwd/错误/取消，不使用 shell 猜安装方式，不升级来源不明的程序。
- [ ] T053 [US5][US8] 原生 UI+文件系统 e2e 完成选择→预览→写入→启动→切 Profile→撤销；验证注释、旧字段、实际请求和恢复；按 Agent/OS 记录 native/live 缺口。

## Phase H — 插件兼容与中间件

前置：B、C、D、E；先 T109 探针再承诺 runtime 实现。

- [ ] T109 [US5] crates/use plugin_lifecycle/plugin_runtime 与 Studio capabilities 做最小兼容探针；用固定本地 fixture 运行一个 OpenCode provider、一个 pi provider、一个 middleware，断言模型/响应/事件和超时结果，记录 Bun/OS/架构/进程隔离是否足够和最小差距；不引入第二网关或默认无界执行。
- [ ] T110 [US5] 同一生命周期接通 npm/git/local provider auth、models、fetch、refresh、usage/check-in；fixture 覆盖多账户/流式/headers、token 轮换、授权撤销、包缺失与 runtime 停止，provider/key 归因贯通。
- [ ] T050 [US5] 隔离宿主的 async request/onRequest/onEvent/onResponse hooks，声明顺序、时间/大小边界与异常语义；真实 helper fixture 覆盖超时/崩溃/撤销/输出损坏，可选 hook 保留原请求且不可绕过 mandatory privacy/auth。
- [ ] T112 [US5] 冻结预置 middleware 的 param override、model map、system prompt、word guard、think tags 兼容；真实请求/事件 tests 覆盖条件/删除/拒绝/模型名恢复与多中间件组合，不复制第二种产品配置语言。
- [ ] T111 [US5] 插件工具、Agent 扩展与页面接到现有 lifecycle/capability grants；fixture 覆盖 install/upgrade/disable/remove、同名冲突、断电恢复、页面权限与未授权副作用 0。
- [ ] T113 [US5] 插件发现/配置/状态复用 PluginStorePage/CapabilitiesPage/PluginConfigDialog 与既有 contributions，local Aster 走本地来源而非 member 登录；runtime 用 RuntimeComponentsSection/runtime.components.* 扩目录，UI/native tests 验证缺 runtime/错架构/无网络/取消/撤销和最小窗口权限。

## Phase I — 搜索、媒体、外部 MCP

前置：B、C、E；插件工具在 H 后验收。

- [ ] T114 [US6] crates/search providers 的配置接线覆盖冻结搜索源、源排序/启停、每源多 key rotate/order、错误冷却与 quota；local HTTP fixture 验证失败切换、禁用保留 key 和真实身份归因。
- [ ] T115 [US6] Core tools/MCP 与 Gateway 外部入口接通 web_search/web_fetch，包括可搜索模型 fallback；tests 验证有界 URL/body/redirect、来源与错误、取消、不受权限外源影响，不维护第二份 search implementation。
- [ ] T116 [US6] 复用真实 image/video provider 与 task adapters 完成生成/编辑、异步状态/取消/下载；fixture 覆盖支持的 Grok/Minimax 等冻结来源、产物类型/多文件/超时/费用，实际 live 另验。
- [ ] T138 [US1][US6] Gateway/Studio 复用现有 audio provider，实现 transcriptions/translations 的 multipart 与 speech 的 JSON→audio/JSON/SSE；fixture 校验音频字节不变、只改声明 model 字段、厂商扩展字段/响应类型、voice/speed、usage/费用、脱敏/取消与有界上传。三种端点独立授权与模型投影；不能以聊天内语音输入控件或图片工具验收。
- [ ] T117 [US6] 外部视觉/analyze_image 接通统一工具配置、能力与计量；tests 覆盖路径/输入大小/错误模型/图像内容、无窗口场景，visual feedback/OCR 不能冒充通用视觉。
- [ ] T118 [US6] 既有 MCP 实现暴露 tools server 与 router，接通服务发现、tool schema、同名消歧、client visibility/auth、取消；真实 stdio/HTTP 客户端 fixture 验证与内置工具相同计量，无推理 key 管理权限。
- [ ] T119 [US6][US7] apps/aster 的工具 CLI 复用同一 MCP/调用层，支持 schema 参数、JSON/text 输出与媒体 -o；tests 验证未知/缺参数调用上游 0、已有文件不覆盖、空格/非 ASCII 输出路径和远端模式。

## Phase J — LAN、远端与同步恢复

前置：C–E；Profile/Agent 恢复还依赖 G。

- [ ] T060 [US3][US5] local_api_gateway/Gateway 接 LAN bind、client key 与传输保护，沿用 DeviceConnectionsSettings 的地址/设备信息架构；旧 remote IPC 仅复用 UI 外壳后替换，不恢复配对服务；真实客户端/UI tests 覆盖开关/401/allowlist/quota/TLS/停止和默认 loopback。
- [ ] T120 [US5][US7] 现有 remote packages 与 Gateway client 实现远端 catalog/providers/groups 访问及本机 Agent 配置；两 gateway fixture 验证一次翻译、断线/撤销、远端 secret 不复制、本机/远端身份正确。
- [ ] T121 [US5] BackupService/原生 owner 定义网关备份成员和恢复顺序；临时目录 round-trip 比较 routes/catalog/Profile/Library/usage/identity，默认 raw secret 排除、损坏/错租户/未知 schema/覆盖冲突拒绝。
- [ ] T061 [US5] 网关配置与备份适配纳入实际恢复/失败回滚；fixture 验证部分写失败、旧版数据、重复恢复、凭据失效和已运行流，不能用聊天数据备份验收。
- [ ] T122 [US5] 现有 WebDAV/S3 同步路径接通网关成员、revision/冲突、离线/认证/大包错误；local protocol fixture 验证上载→另一隔离实例恢复、取消不覆盖、无 secret 泄露。
- [ ] T123 [US5] 同步 owner 支持参考要求的 GitHub 保护备份；测试密钥缺失/解密失败/冲突/未授权写和跨设备恢复；执行凭据保护，不把 raw tokens 放进仓库或日志。
- [ ] T142 [US4][US5][US8] 为 Magpie 用户提供显式导出/导入迁移：复用 Provider/数据导入预览与 native store owner，逐项映射 providers/keys refs、groups、Profiles/Library、模型可见性与可迁移用量；外部格式只在导入边界读取，不加入产品配置发现，不复制登录/session 状态。合成 fixture 覆盖旧/当前 schema、未知项明确报告、同名冲突、部分失败回滚、重启和导入后实际请求，迁移前后的原导出均保留。

## Phase K — CLI/TUI、无头和平台入口

前置：G2；各扩展仅在 F–J 对应能力完成后开放。无头复用不能复制 Studio 业务逻辑。

- [ ] T124 [US7] apps/gateway 独立 serve、apps/aster 管理 CLI 共用本地 owner API；将真正需共享逻辑放既有合适 crate/module 并保留公开入口；真实进程 tests 验证无 Tauri 时 key/provider/route/quota/Profile 读写及同 revision 冲突。
- [ ] T125 [US7] 现有 terminal authored source 接通 Agent/模型/effort/Profile/catalog 管理；按生成流程更新 codegen；TUI hermetic/真实进程测试键盘选择、错误、退出与服务生命周期，不新建平行 TUI。
- [ ] T126 [US7] 独立 serve 的 web 管理复用同一业务 API 与现有 UI，管理鉴权独立于 inference grant；HTTP tests 覆盖无权限/错误 key、配置修改、TLS/reverse proxy 配置和桌面读取一致。
- [ ] T127 [US8] 既有 Windows/进程适配支持 WSL distro 检测、文件写入、Agent 启动和 session 归因；portable fixtures 验证 path/argv；native Windows+WSL 验证双发行版、非 ASCII/空格、停止/取消，缺环境不标已通过。
- [ ] T128 [US7][US8] 现有 Studio tray/autostart/theme 接通 gateway 状态、Profile 快捷操作、保持唤醒、明暗/系统/Omarchy 适用行为；native/UI tests 验证开关/重启/权限拒绝和退出释放。
- [ ] T129 [US7][US8] 现有部署/CLI 流程支持后台服务与 Docker/server/NAS；真实进程/容器 fixture 验证持久卷、重建/升级、缺 runtime、停止进程树；install/uninstall 默认保留数据，显式 purge 才清理授权目录。

## Phase L — 全量资格、交付和证据，G3

前置：A–K。除 T074 的条件性 staging 外，所有任务是 G3 必需。

- [ ] T070 [US1][US2][US3][US4][US5][US6][US7][US8] 运行全离线契约和现有受影响回归，保存每 test 的结果/数量；ElysiaJS 扫描为空；批量失败逐项单独重跑区分实现/fixture/环境，不改 grader。
- [ ] T071 [US8] 在声明平台/架构运行 native HTTP/SSE、桌面、CLI/TUI、安装/恢复/取消；记录 Rust/Node/pnpm/features/Agent/provider 版本与 Windows/WSL 证据；现有 macOS/Linux CI 不代称全平台。
- [ ] T132 [US3][US5][US8] 真实旧数据→新版本→失败恢复/回滚资格；fixture 覆盖单 key/多 key、旧 route/schema、Agent 接管和 secret 存储、同步部分失败；不承诺未支持的二进制降级。
- [ ] T072 [US1][US2][US6] 按 plan.md §7.2 冻结同一 fixture/workload、样本数与阈值，测量启动、RSS、p50/p95、TTFT、并发、长 SSE/取消和插件/脱敏开销；补 100 provider/1,000 model/100,000 usage 的 UI/查询场景，核对一次活动查询、不全量读历史与后台刷新范围。使用既有 Criterion/Playwright/native 观测，记录环境/原始数据，不套 Magpie 数字，不改 workload/阈值获得通过。
- [ ] T135 [US8] 最终 desktop/CLI 归档按 owning source contract 验证 companions/runtime/模型/字体/renderer/capture；隔离前缀安装后真实调用网关/工具；macOS CLI 保留 sandbox smoke，缺成员有负例，不创建/上传 Release。
- [ ] T131 [US8] 在现有 .github/workflows/ci.yml 与 release/qualification workflows 纳入已确认的功能/native 检查入口；测试条件和归档门能检测缺文件/未测平台，不新增并行 CI 体系或扩大 allowlist。
- [ ] T133 [US1][US7][US8] 更新 Gateway/Studio/CLI 使用文档、权限/配置/迁移/恢复说明与各平台限制；实际命令/help/endpoint 对照，链接检查通过，删除提前宣称而非隐藏未完成项。
- [ ] T134 [US1][US8] 逐行审查 contracts/capabilities.md、agents.md、provider/endpoint 子清单：来源→任务→实现→fixture→native/live→evidence；未验收/失败行禁止 G3；记录反证与 exact follow-up，不用统计百分比代替场景。
- [ ] T073 [US1][US2][US3][US4][US5][US6][US7][US8] docs/evidence/local-gateway/<date-slug>/OUTCOME.md 保存命令、计数、diff、脱敏日志、平台/性能/归档、Result/What landed/Explicitly not claimed；真实账户缺证据就保留限制。
- [ ] T074 仅在用户另行要求 commit 时逐文件 stage 并检查 git diff --cached --name-only；不提交其他会话修改；未请求 commit 时作为明确不适用的操作，不影响 G3 行为验收。

## Final Verification Gate

- [ ] 冻结来源和差异已记录；全部功能/Agent/provider/endpoint 子项有证据，未测试不算完成。
- [ ] owning package focused tests 有真实非零执行；修改 Rust 后 cargo fmt --all -- --check 与支持 features 的 clippy --locked --all-targets --no-deps 通过。
- [ ] 前端定向 tests、typecheck、原生 Tauri 权限与受影响 tenant 回归通过。
- [ ] 未授权/超限上游调用 0；承诺后重放 0；logs/Debug/usage/export 的 synthetic raw secret 0。
- [ ] 单 key/多 key/ACL/Agent 原生文件迁移与部分失败恢复通过，用户非托管字段无损。
- [ ] 管理与推理权限分离；内置/外部工具、桌面/CLI/TUI/无头使用同一配置权威。
- [ ] 参考兼容与 Aster 扩展分开记录；无未测平台/账户/性能提前声明。
- [ ] 无新 crates.io aster-* 依赖、第二 root workspace/lockfile、ElysiaJS 或原生 select/datalist。
- [ ] 产品配置仍为 aster-acl；Agent 原生文件不进入配置发现；Studio 仍为私有子模块。
- [ ] 最终归档成员由 source contract 定义，隔离安装后真实执行通过；发布另按 release governance。

### 2026-11-02：Code 路由目录接线（验收未关闭）

本轮为 T015/T031/T105 增加实际接线：Gateway status 从同一 ModelControl 活跃策略投影 fixed/ordered 路由，保留名称、ID 与 canonical `group/<id>`，按活跃 alias/service/endpoint grant 过滤；每个唯一模型最多三次本地能力查询，目录不调用上游。Code 复用 ModelSelector 和原 ModelSettings 路由编辑器；配置预览/读回、Claude 角色、当前连接和启动协调共用显式 runtime 路由地址，路由变更刷新共享快照。新增红/绿回归证明并修复“原始配置保留新路由但启动仍传旧模型/供应商”。744 个前端测试、51 个 native Gateway 测试（含原 Google SDK fixture）、Studio Clippy 与 ToYou lib 编译通过；15 份冻结输入哈希不变。全量类型检查仍有当前 32 文件范围外的 263 个错误，root fmt 仍有 8 个其他路径失败。证据：`docs/evidence/local-gateway/2026-11-02-code-route-catalog/OUTCOME.md` 与 verification.json。

验收限制：九种文件配置型 CLI 的配置写入/读回测试使用 mock host I/O，不证明真实落盘或进程启动；本次检查未找到当前宿主的 `code_cli.run` 处理器，下一步补齐原生 Code CLI read/write/run 并验证真实 configure→launch→gateway 请求。运行型 Antigravity/OpenClaw/DeepSeek Harness 需显式路由适配后才开放路由选项。运行中的新增路由/协议变化仍需重启 Gateway；刷新目录不等于热重编译 grant。协议可用也不等于 tools/reasoning/modalities 已全部验收。T015/T031/T033/T105 与完整 Magpie 替代门禁保持未完成，不修改验收复选框。

### 2026-11-02：原生 Code CLI 配置与启动（资格复验待完成）

继续 T015/T031/T033/T105：现有 Code 页面四个 IPC route 已接入受主窗口 capability 与既有 host token 保护的 native handler。共享 JSON 保留原有 13 个配置 target、9 种文件配置型 CLI 和 14 个 executable 名称；原生按 target 解析路径、限制文件字节、先暂存再提交并在后续失败时回滚。启动复用 ToolStore/PATH 解析和原终端选择；Gateway 使用显式 stored model/route identity 校验运行快照。未增加第二套配置页面、模型选择器或安装器。

当前 22 文件范围对应的 701 项前端测试通过；扩大范围 714 项中 713 通过，范围外 cua 英文描述缺失的一项原断言单独重跑仍失败。全量类型检查有范围外 263 个错误；root fmt 有 9 个其他路径失败。15 份冻结输入哈希不变。较早检查通过 17 项原生配置/启动/真实 IPC 测试及 1 项独立进程→实际 Gateway HTTP→用量记录测试，后者使用合成 CLI fixture，不能代替官方 CLI 二进制或桌面终端验收。最终原生复验、全 Gateway（含原 SDK fixture）、Clippy 与 ToYou 编译仍待完成：当前 rustc 在过程宏动态库加载期间等待 macOS dyld 的 fcntl 文件检查，尚未开始测试；保留原进程和排队检查，不以等待代替通过。证据、源文件/冻结输入哈希、诊断与继续使用的运行句柄见 `docs/evidence/local-gateway/2026-11-02-native-code-cli/OUTCOME.md` 和 verification.json。

后续先收集上述运行句柄的最终结果，再验收官方 CLI/桌面终端与 native Windows/Linux/WSL，并补运行型 adapter、其他终端、热更新等缺口。完整替代门禁与既有验收复选框保持未完成；未执行安装、发布、stage 或 commit。

### 2026-11-02：配置 JSONC 保留与原生复验（验收仍开放）

T104/T105 的 JSON 切片已接入原有 Code 配置流程：七个 JSON target 的生成和撤销、草稿修改及既有 Format 按钮复用现有 jsonc-parser；仅修改变化的对象字段，未变化的注释、转义/数字原始写法、缩进与换行保留，新文件维持原格式。原始空/损坏文件仍可在原编辑器读取修复，重复键及等价转义键在 JSONC 解析边界拒绝，避免修改首个键而 CLI 仍读取最后一个键。未新增编辑器、配置页或依赖。新增 24 例；全 Code/桥接/共享预置及 IPC 回归 725 项通过，五文件类型错误为零，全量类型检查仍有范围外 263 个错误，15 份冻结输入哈希不变。原始失败与 fixture 键名修正说明见 `docs/evidence/local-gateway/2026-11-02-cli-jsonc-preservation/OUTCOME.md`。

上一切片的最终原生检查已越过 macOS 动态库等待：首次编译在范围外 capabilities_toolchain.rs 的 4 处旧 API 调用失败；当前 ToolchainPlugin 已提供对应方法，重查后 18 项 Code CLI 配置/真实 IPC/启动参数测试与 53 项 Gateway 测试通过，两组名称无重叠，共 71 项。包含原 Google SDK fixture、stored/API alias 校验、真实独立 CLI fixture 进程→网关→用量账本链路。Clippy 与 ToYou 编译仍沿原句柄排队，最终结果更新在 native-code-cli 证据中。官方 CLI、桌面终端和其他 OS 的实际运行仍未验收。

T104 仍需其余格式、变更数组的格式保留、并发编辑和所有权/撤销；T105 的完整 Agent 清单、热更新及各 G gate 继续保持未完成，不改变验收勾选状态。

本切片资格补记：18 项 Code CLI 与 53 项 Gateway 测试最终均通过；Studio 全目标 Clippy 与 ToYou lib 编译也已通过。Clippy 的共享 lib.rs 原有诊断按位置保留并读上下文，未自动修复；没有诊断指向新增 CLI 模块或 typed catalog resolver。725 项前端测试、5 文件源码及 15 份冻结输入哈希已经重新打开比较，详情与原始失败记录均在上述两份 evidence 中。根格式/全量类型的其他路径错误、官方 CLI/桌面终端/平台/性能资格以及所有完整替代门禁仍未关闭。


### 2026-11-02：Code CLI TOML 保留与构建缓存（验收未关闭）

继续 T104/T105/T053：Codex/Kimi 的生成、设置更新和清除配置沿用现有 Code 草稿/预览/保存入口，改为保留原 TOML 注释、格式、字面值及 64 位整数；保留原严格入站解析。补齐 24 个实际适配器/编辑器回归，并修复源片段覆盖检查的重复扫描。固定 50 次采样、5 次预热、60 秒超时的 Node 基准中，1 万用户键配置的保留式编辑 p50 从 1283.29 ms 降至 140.65 ms；原超时失败和全部原始统计均保留，不等同 UI 长任务或 G3 性能验收。依赖补丁从固定上游源码和源补丁重建，安装版本与重建结果逐字节一致。

产物检查发现成功的缓存构建仍含旧算法，已将 PNPM 锁文件/工作区配置接入既有构建缓存依赖。冷构建、修复后的常规构建及再次热构建均通过；最终 2365 个 renderer 文件哈希与实际补丁片段复验一致。785 项相关前端测试通过，ESLint/Oxlint、定向源码格式、冻结离线依赖安装（禁生命周期脚本）及依赖一致性检查通过；全量类型检查仍有相同 263 个既有诊断，国际化及三个已有格式问题仍未通过。15 份冻结输入不变。证据：`docs/evidence/local-gateway/2026-11-02-cli-toml-preservation/OUTCOME.md` 与 verification.json。

本轮不关闭任何任务复选框：配置并发/完整接管撤销、其他格式资格、官方 CLI 与跨平台原生实测、UI 性能、完整 Magpie 替代及发布门禁仍未完成。未 stage、Git commit 或发布。


### 2026-11-02：Code CLI 乐观并发与草稿保留（资格待完成）

继续 T104/T105/T053：既有 native read 返回文件字节 SHA-256 或 missing；write/delete 必须携带来源 revision。草稿、原始编辑、设置更新和凭据重建沿用该 revision；九个 clear adapter 改为转换同一次读取的快照，避免二次读取掩盖冲突。宿主校验准备/提交时的内容，串行化同一宿主的读写；回滚检测到外部改动时保留外部文件并报错。既有弹窗、切换与启动错误路径使用既有翻译提示，失败保留未保存草稿，没有新增编辑器或配置 owner。

原生旧版反例已复现覆盖外部修改（0 通过/1 失败），新版本 26 项 Code CLI 原生测试通过；包含实际 Tauri IPC、文件生命周期、整批 auth 删除冲突、八路保存仅一个成功、提交中冲突及回滚保护。793 项相关前端测试、renderer 生产构建、定向 ESLint/Oxlint/格式和 i18n 检查通过。39 个 Studio 源码文件检查后哈希稳定，15 份冻结输入不变。全量类型仍有范围外的 263 个诊断，根格式在十个其他路径失败。Gateway 联动、all-target Clippy 与 ToYou lib 编译的串行资格任务仍在运行，尚不计为通过。证据：`docs/evidence/local-gateway/2026-11-02-cli-config-conflicts/OUTCOME.md` 与 verification.json。

这不是跨进程原子 compare-and-swap；不协作的外部写入仍可能落在检查与替换之间，保留外部改动时的回滚也可能不完整。完整接管/撤销原配置、持久恢复、真实桌面/官方 CLI/平台/性能与全部替代门禁仍待验收。不更改复选框，不 stage、Git commit、安装或发布。

资格补记：同一串行原生任务现已结束，53 项 Gateway 测试、Studio 全目标 Clippy 和 ToYou lib 编译均 exit 0；与 26 项 Code CLI 测试名称无重叠，共 79 项原生通过。39 个源码与 15 份冻结输入的最终哈希不变；完整日志和最终资格状态已更新到上述 evidence。此处不追改先前“待完成”的历史检查点，也不关闭完整任务或替代门禁。

### 2026-11-02：T143 完整源码清单与近期行为增量（未关闭）

官方固定版本的新旧源码已全部下载并逐 blob/目录验证。旧 executable 基线→v0.1.2523 为 2,251 个文件变化；规划 source→v0.1.2523 的全部 53 个文件已归入 13 组，逐组补 owning task、具体行为、既有 UI owner 和 fixture/native 要求，见 [source audit](../../docs/evidence/local-gateway/2026-11-02-reference-source-audit/OUTCOME.md) 与 semantic-delta.json。补正 research 中“当前参考执行器未验收”的过时结论，保留既有真实运行和固定 429 失败，不改输入/超时。AG41/AG51 的专用 Code adapter 明确缺失。完整清单不等于功能对齐；旧基线的其余语义审计及新适配尚未完成，96 个任务仍为 3 勾选/93 开放，70 Agent 与 35 CAP 的原 ID、名称和顺序保持不变。

### 2026-11-02：T104 dotenv 字节保留与真实语法（未关闭）

修复原共用写入器改动 CRLF/末尾换行、清除首行键时丢失 UTF-8 BOM、重写赋值展示及引号/反斜杠失真的问题；显式区分 Gemini 的 Node dotenv 与 Hermes 的 Python dotenv。预览、更新、提取、验证和清除沿用原 Code 入口及 native revision 保存；清除共享同一 source-span 实现，删除裸声明遮蔽的旧托管凭据。新增 31 个 hermetic cases，相关前端 824 项/56 文件通过；Node 120 输入的四种操作、实际 Python dotenv 1.2.4 和固定 Hermes 变量解析函数的 433 次交叉检查通过。后者运行在 Python 3.13.15，仅资格该解析函数，不等于当前 Hermes 3.14 应用/官方 CLI/真实请求通过。renderer 构建、定向 lint/格式/i18n 通过；全量类型仍为范围外 263 项，15 份冻结输入不变。最终源码的固定 Node 微基准中，一万用户键 p50 为旧 11.75 ms、当前 Node 7.53 ms、Python 4.07 ms；此前样本与源码哈希另保留，不等于 UI 或 G3 性能验收。

完整证据、失败/fixture/新 oracle 缺陷的更正记录与所有原始样本见 [dotenv evidence](../../docs/evidence/local-gateway/2026-11-02-cli-dotenv-preservation/OUTCOME.md)。YAML/变更数组保留、原配置接管撤销、各 Agent 实际版本/加载器/启动及所有替代门禁仍未完成；96 项复选框不变，不新增 UI 或配置 owner，不 stage/commit/安装/发布。

### 2026-11-02：T104 YAML 源码保留与真实加载器（未关闭）

Hermes/MiniMax 的生成、更新和清除继续复用原 Code 草稿、预览、保存与 native revision 路径。使用已安装 YAML 库的 CST 接口合并托管字段，保留未改动用户字段的缩进、行内集合、注释、数字写法、BOM、指令、文档结尾及 LF/CRLF/CR；无变化的草稿逐字节一致。修复空段注释丢失、MiniMax 空 provider 段和 YAML 1.1 自动生成 `!!omap` 的问题；新字符串显式引用并转义会被 YAML 1.1 折叠的 Unicode 换行。清除会留下悬空别名时拒绝保存；语法错误只报稳定代码，不含配置原文。重复键保留原库判定，通过一次线性校验；普通 JS 读回和编辑器共用同一解析入口。

新增 26 个 hermetic cases；相关前端 **850 项/57 文件通过**。固定 Hermes 代码与 ruamel.yaml 0.18.16 的实际加载器通过 **600 次检查**（144 个格式输入和 6 个 Unicode 输入，各四种操作），运行环境为 Python 3.13.15；不等于 Hermes 3.14 应用或官方 MiniMax CLI 验收。定向 lint/格式/i18n 和 renderer 构建通过；7 个源码文件及 15 份冻结输入哈希复验不变；全量类型仍有相同文件/代码分布的范围外 263 个诊断。

固定 5 次预热、50 次采样和 60 秒超时的 Node 基准保留全部结果：初版超时；线性校验检查点的万键 p50 为旧 859.01 ms、当前 132.65 ms；最终源码复验仍超时，因此不宣称最终性能通过。没有修改输入、样本数或超时取得通过。证据：[YAML evidence](../../docs/evidence/local-gateway/2026-11-02-cli-yaml-preservation/OUTCOME.md) 与 verification.json。变更数组、完整原配置接管/撤销、实际 Agent/平台、UI 性能与完整替代门禁继续开放；96 项复选框不变，未新增 UI、依赖或配置 owner，未 stage/commit/安装/发布。

### 2026-11-02：原配置恢复记录与卸载顺序（逐字段撤销未完成）

既有 native CLI 写入器在租户/实例 profile 下记录首次原文件、原权限、实际应用字节和前一次 revision；接管记录先于配置提交，恢复记录最后删除，继续使用原事务提交、冲突与回滚。配置切换保留首次原始状态，已记录且没有额外修改的文件可原样恢复，原本不存在的文件可删除；无记录时不按字段名、供应商前缀或 OAuth 字段猜测归属。记录仅限既有 9 个文件型 CLI/13 个 target，读写有界，Unix 文件权限为 0600；不新增配置权威或通用凭据库。

Code 的原清除、停用、使用自身配置和卸载入口继续使用同一 `code_cli.write_config`，增加互斥恢复意图；删除九份旧字段清理逻辑与未使用的 Codex OAuth 推断。卸载先恢复文件，恢复失败不卸载或清空当前选择；恢复成功、二进制卸载失败时按文件实际状态清空活跃连接。已有样式、弹窗、错误翻译和模型选择器继续复用。

当前前端 **811 项/57 文件**通过；原生 **37 项 Code CLI + 53 项 Gateway** 名称无重叠，共 90 项通过；工具环境 14 项测试、两包 all-target Clippy、ToYou lib 编译和 root fmt 通过。工具安装模块两处不存在的 `Value::Array` 调用已修为当前 `Value::List`。全量类型仍为相同文件/代码分布的范围外 263 项诊断。旧清除断言要求删除已有用户设置或推断登录模式，现由实际 native 文件恢复、冲突、权限和 Profile 隔离用例接替；不改变冻结输入或使用通过数表示覆盖率。

这仍是整文件恢复切片：外部修改会阻止恢复/后续覆盖；逐字段所有权、保留接管期间新增的非托管内容（含高级原始编辑）、旧无记录配置迁移、官方 Agent/桌面/其他 OS、备份/导入成员与凭据保护及性能尚未完成。不把 host 重建/中断状态模拟称为真实应用崩溃或断电验收。完整证据见 [恢复记录](../../docs/evidence/local-gateway/2026-11-02-cli-config-restoration/OUTCOME.md)；96 项任务的编号、顺序和勾选状态不变，未 stage/commit/安装/发布。


### 2026-11-02：保留外部修改的逐字段恢复（所有权验收仍开放）

继续 T104/T105/T053，解除接管沿用原 Code 清除、使用自身配置、停用与卸载入口。优先执行原整文件恢复；遇到外部修改时，由同一 native owner 返回 before/applied/current 及恢复记录、各文件 revision，renderer 复用既有 JSONC/TOML/YAML/dotenv 写入器撤销记录中的字段差异。原生核对恢复记录未更新、全部目标齐全且逐文件未再变化，继续同一暂存/提交/回滚，并最后删除记录；不新增页面、编辑器、配置 owner 或依赖。

无关的外部用户字段和注释保留；删除字段时丢失的 JSONC/TOML/YAML 注释转为文件开头的独立注释，dotenv 行尾注释保留在原处。已自行恢复的值不重写；重叠字段、用户改动过的托管数组或新建容器、安全字符串路径不能表达的 YAML 键均拒绝整批恢复。真实加载器检查另发现原 YAML 空段无法恢复，已修复 null 节点生成并加回归。

最终 **832 项/58 文件**前端通过；**42 Code CLI + 52 Gateway + 1 单独启用的 Google SDK 用例 = 95 项**原生通过，名称无重叠；Studio all-target Clippy、ToYou lib、root fmt、定向 lint/格式/i18n 与 renderer 构建通过。Node dotenv 106、固定 Hermes Python dotenv 106、Hermes YAML 144，共 **356 次真实加载器检查**通过；后两项运行 Python 3.13.15，不等于 Hermes 3.14 应用或官方 CLI 验收。全量类型仍有相同文件/代码分布的范围外 263 项诊断；17 个修改源码与 15 份冻结输入复验不变。

这仍按已记录的 before/applied 差异恢复，不等于明确的 managed-path 所有权；经 Aster 高级原始编辑保存的用户改动仍可能包含在恢复差异中，需后续独立所有权记录。外部改动后的直接重新接管、无记录迁移、变更数组保留、官方 CLI/桌面/平台、崩溃断电与性能门禁仍未完成。证据：[逐字段恢复](../../docs/evidence/local-gateway/2026-11-02-cli-field-release/OUTCOME.md)。96 项任务仍为 3 勾选/93 开放，Agent/CAP 清单不变；未 stage/commit/安装/发布。

### 2026-11-02 — 明确托管路径并保留高级原始编辑

T104/T105/T052 的现有配置草稿、原始编辑、解除接管及 native 恢复记录现在贯通 adapter 生成路径的所有权。元数据在用户原始编辑前捕获，并在模型、凭据及控件重建时保留；解除接管只反转受管字段，保留未托管手工字段、生成容器内用户字段与注释。纯生成记录仍走精确恢复，部分记录走既有预览/版本校验/事务；历史无所有权记录拒绝新增原始编辑保存，并保留草稿给出翻译说明。修正了保留用户内容时不应将当前 0600 权限恢复成旧 0644 的反例。没有新增页面、编辑器、依赖或配置 owner。

855 项 renderer/shared、49 项 Code CLI、52 项 Gateway、显式执行的 1 项 SDK、1 项视觉权限回归通过；原生不同测试共 103 项，不重复计算隔离重跑。最终配置另通过 106 项 Node dotenv、106 项 Python dotenv、144 项 Hermes YAML 加载器检查（合计 356）。Clippy、格式、局部前端 lint、当前 i18n 与 renderer 构建通过。完整 typecheck 仍有 257 项本轮路径外错误，较前一 263 项基线只减少六项。当前 ToYou 复验仍在 screencapturekit 构建脚本运行中，未声明完成。

固定输入、5 次预热/50 次采样/60 秒超时的最终源码配对基准中，万键 YAML 所有权收集 p50 从 139.68 ms 降至 85.21 ms；这不含完整生成、格式化、IPC、文件写入或 UI，之前最终 YAML 格式化基准超时仍开放。早期所有权基准的 YAML 源码哈希与最终文件不同，已保留旧结果，并以相同输入、样本数和超时复验最终源码。全部失败、命令、计数、哈希与未声称事项见 [ownership evidence](../../docs/evidence/local-gateway/2026-11-02-cli-config-ownership/OUTCOME.md)。官方 Agent/桌面/跨平台、历史所有权迁移、外部修改后重新接管、变更数组、备份导入与完整替代门禁继续开放。96 项任务编号、顺序与 3 勾选/93 开放状态、Agent/CAP 契约保持不变；未 stage/commit/安装/发布。

### 2026-11-02 — Antigravity CLI 配置与运行时连接

AG66/T105 复用既有 Code 配置侧栏、模型选择、原始编辑、启动弹窗与 native 恢复 owner，新增独立的 `~/.gemini/antigravity-cli/settings.json` adapter。写入 Gemini provider、自定义模型目录及选择，保留用户模型、未知字段和注释；启动时经既有 IPC 传入 API key/endpoint 环境，并用实际 custom model key 传 `--model`。自己的登录不注入运行时参数。现在共有 10 个文件 adapter、14 个目标，没有新页面、编辑器或依赖声明。

最终 renderer/shared 865 项、62 个文件通过，构建、i18n 与局部 lint/format 通过；完整 typecheck 仍是原有 257 项相同文件/错误码分布，本轮无新增。ToYou lib 编译 exit 0。首轮原生运行 51 项通过（含三个新用例），唯一失败是旧契约测试仍断言 13 个目标/9 个文件 CLI；已更新为 14/10，并增加 Antigravity 的独立路径、语言和单一目标归属断言。该用例的隔离复验、完整 CLI/既有网关链路/SDK、Clippy 和根格式检查仍在后续流水线执行中，未计作通过。

首次 `--locked` 在测试启动前发现既有依赖声明未同步；Cargo 离线同步仅给 Studio 的锁文件依赖列表补入已声明的 `tracing-appender`，没有新增 package。之前所有权切片的原 ToYou delivery 检查也已终止并记录成功完成日志。详见 [Antigravity evidence](../../docs/evidence/local-gateway/2026-11-02-antigravity-cli/OUTCOME.md) 与 [ownership evidence](../../docs/evidence/local-gateway/2026-11-02-cli-config-ownership/OUTCOME.md)。

官方 CLI、桌面 GUI、Windows/Linux/WSL、外部 shell profile、移除模型后的目录清理和 Agent 专用身份/计量仍未验收；完整替代及 G3 继续开放。96 个任务编号、顺序和 3 勾选/93 开放状态不变；Agent 主表 70 个名称/顺序与附表 6 行不变，仅 AG66 进度更新；CAP 契约不变。未 stage/commit/安装用户工具/发布。

### 2026-11-02 — 官方 Antigravity CLI 原生文字、工具与计量

补齐 AG66/T105 的官方 CLI 验证：固定 1.4.12 macOS ARM64 发行资产及提取后二进制 SHA-256，并通过严格签名验证。生产 adapter 的实际草稿经既有 native writer、ToolStore 和启动脚本落地，真实 Gateway/ModelControl/账本承接请求；仅官方子进程使用临时 HOME、合成 key 和本机网络沙箱。三项沙箱正反控制通过，不改用户 profile 或已安装 CLI。

首轮正例因共享 fixture 的 8,192 输出上限被官方 CLI 默认 65,536 请求拒绝，原始失败保留，并作为显式负例继续验证零上游/零计量；正例声明兼容能力后，文字场景 1 请求/11 tokens，真实 read_file 工具往返 2 请求/22 tokens。断言核对实际文件内容、parametersJsonSchema、thoughtSignature 及既有账本唯一记录。没有放宽能力 guard、超时或冻结 grader。11 项 Gemini 集成测试（含此 opt-in 官方 CLI 与既有官方 Google SDK）通过，Clippy all-targets 和根格式检查 exit 0。

仍需完成 T105/T127 的输出额度感知模型选择/配置 UX、附加内置轻量模型的可用性与策略、其他 Agent/功能/平台、外部身份认证、shell profile、目录清理、Agent 身份与计量及 G3 性能门禁。本轮不重新声明前端 typecheck 或桌面 UI 已通过。96 项任务保持 3 勾选/93 开放，Agent 名称和顺序及 CAP 契约不变；仅 AG66 的证据进度更新。命令、原始失败与哈希见 [official CLI evidence](../../docs/evidence/local-gateway/2026-11-02-antigravity-official/OUTCOME.md)。

### 2026-11-02 — 模型额度进入既有选择、保存和启动流程

补齐 T105/T127 的已知额度缺口：原生目录在既有查询中按协议投影输出上限；混合协议路由不会用 OpenAI 的较高上限冒充 Gemini 的额度。模型选择、草稿和既有保存检查、启动前置检查共用一条 Antigravity 兼容规则；1.4.12 所需 65,536 输出额度未知或不足时明确拒绝。配置错误在模型选择处可见，高级编辑器折叠或没有生成文件时仍能显示，展开后不会重复出现。IPC schema 原先丢弃的路由和额度字段已用失败回归补齐。无新页面、选择器、权限、依赖或持久化设置。

879 项前端/共享测试（63 文件）、11 项目录和 11 项 Gemini 原生测试通过，后者含固定官方 CLI 和既有官方 Google SDK。Clippy all-targets、根格式、局部 Oxlint/ESLint/Oxfmt、i18n 与 renderer 生产构建通过。全量 typecheck 从基线 266 项变为 253 项，本轮文件均无诊断；其他位置的并发变化不归因为本轮修复。15 个冻结输入及此前官方 CLI 证据保持原字节。

完整替代和 G3 不关闭。真实桌面、其他 Agent/平台/版本、辅助模型、shell profile、目录清理、Agent 身份与计量仍待验收；源审计还提示直接模型别名的草稿与启动 ID 可能不一致，下一步需完整复现并在共享 resolver/matching owner 修复。96 个任务保持 3 勾选/93 开放；Agent 名称和顺序及 CAP 契约不变。见 [output limit evidence](../../docs/evidence/local-gateway/2026-11-02-antigravity-output-limits/OUTCOME.md)。

### 2026-11-02 — 直接模型别名贯通既有配置、原始编辑和启动

以相同输入切换原始/修复后的生产 resolver，确认旧草稿写入内部 `local-alias`，而启动使用 `probe`。现在复用既有 `getRawModelId`，十种文件配置型 CLI 的草稿、当前连接归属、配置编辑与启动均采用 API ID，内部选择和显示名称保持原有语义。直接配置编辑复用页面已加载的模型 map，等待现有查询；仅改显示名称不重读 CLI 文件，Antigravity 的纯模型配置不额外读取凭据。原始配置中保留同一 API 模型时也保留内部选择，避免保存后把内部别名换成不存在的记录。既有外部配置/原始覆盖语义保留，没有新页面、依赖声明、查询或选择器。

893 项前端/共享测试（63 文件）、53 项原生 Code CLI 测试通过；后者包含固定官方 1.4.12 的直接别名请求，使用同一 JSON oracle、真实 native writer、ToolStore、隔离启动脚本及合成本机 Gemini 服务。53 项不重复计入单独预跑的官方案例。模型读取失败在任何 CLI 文件 IPC 前拒绝，只有确实缺少记录时保留旧式 wire ID 回退。Aster Clippy、根 Rust 格式、局部 ESLint/Oxlint/Oxfmt、i18n 及生产构建通过；全量 typecheck 和全局 docs gate 的失败与覆盖限制单列在证据中，不据此宣称全部通过。

既有错误别名文件需在现有 UI 重新应用配置，不启动时静默覆盖用户原始文件。官方直接模式不经本地网关账本，本轮不增加计量资格结论。真实桌面、外部认证、辅助模型、其余平台/版本/Agent 和 G3 仍待验收。96 项任务保持 3 勾选/93 开放，Agent 名称和顺序及 CAP 契约保持不变。见 [direct alias evidence](../../docs/evidence/local-gateway/2026-11-02-cli-model-alias/OUTCOME.md)。

### 2026-11-03 — 命名客户端进入原密钥区域与同一授权/账本

T020/T021/T024 已增加命名客户端的创建、改名、模型/路由与接口范围、轮换和撤销；只扩现有 API 网关密钥区域，沿用 `PageSidePanel`、`SearchSelect`、`Switch`、`CopyButton`。模型选择使用内部稳定引用，由原编译 owner 统一生成四种 alias；不新建模型页、预算页、账本或凭据 vault。新密钥仅在创建/轮换后返回，Native Preferences 保存 verifier、稳定 ID、generation 和授权；列表/Debug 不含签发密钥。缺失 scope 拒绝，显式 null 才代表全部模型。原默认密钥和 Code 的默认客户端保持兼容。

协议转接不再把客户端 endpoint 范围扩大为全部。Gateway 的既有认证结果通过显式可信 target 和同一个 attempt identity 传到私有宿主；重复准备请求也保留该身份，普通上游移除该内部 header，伪造调用方 header 不改变归因。公共/私有权限、原有协议、工具、预算、取消和真实官方客户端继续复验。仅改名称不重启；哈希和配置编译离开 async 执行线程；应用失败返回签发密钥及错误，并避免共享缓存继续显示旧的运行状态。

918 项前端/共享测试（67 文件）和 123 项不同的 Studio 原生检查通过：59 Gateway（含官方 Google SDK 和 Antigravity）、63 ModelControl、1 权限。新命名客户端场景在同址连接复用下验证四种 alias、越权零上游、旧 key 失效和撤销，实际 8 次调用由原账本记录 80 tokens，按两个稳定 client ID 分别归因。两包 Clippy、根 Rust 格式、ToYou 编译、局部前端 lint/i18n 及生产构建通过。完整 typecheck 仍有 253 项错误、无本轮文件诊断；Gateway 全库 2208 通过、3 失败、7 ignored，三项失败已逐项单跑；全局 docs gate 仍失败。未修改原 OCR/资源隐藏断言来换取通过。

T020–T025 和完整替代继续开放：默认客户端 verifier/generation 的完整迁移、客户端周期预算、Code 的命名客户端选择、更多协议/历史及真实桌面/跨平台验收仍缺。授权变化当前会停止/重启网关；下一步应复用既有 Gateway reload，在不破坏在途请求的条件下发布凭据/授权快照。96 个任务保持 3 勾选/93 开放，Agent 名称/顺序不变；CAP09 仅更新实现进度，需求与任务映射不变。详见 [named client evidence](../../docs/evidence/local-gateway/2026-11-03-named-gateway-clients/OUTCOME.md)。

后续复验完成：修正路径断言的 `PathBuf`/字符串类型错误后，隔离契约测试通过，完整 Code CLI 52 项、既有 Gateway CLI/账本链路 1 项、显式运行的 Google SDK 1 项通过（54 个不同原生测试，隔离契约不重复计数）。all-target Clippy 与根格式检查 exit 0；上述官方 CLI/完整行为/平台/性能及完整替代边界继续开放。原失败日志均保留在同一 evidence 中。

最终兼容性复验补充：缺少 `URL.canParse` 会让有效端点校验抛异常，已由先失败的回归复现，并改用已安装的 Zod URL/协议约束。最终前端为 866 项通过，62 个文件；完整 typecheck 的 257 项文件/错误码分布不变，本轮路径无错误。并发翻译更新后的排序错误由既有 sync 恢复，翻译内容保持相同，当前 i18n 与构建通过；Native 54 项的 Rust 与文件契约源码未再改动。


### 2026-11-03：客户端授权热更新（完整 C 阶段仍开放）

沿用原 API 网关客户端侧栏与 Gateway 原子运行时替换。创建、修改授权、轮换和撤销不重建原监听器、默认 verifier 或模型绑定；在途请求沿用原快照，新请求使用新权限。新增嵌入宿主更新入口只接受 `new_embedded` 构造的实例，普通托管网关仍须经过原快照入口。配置投影、模型读取和哈希继续在既有 blocking worker；应用失败停网关并保留一次性签发结果。端点权限取现有 grant 的交集，并保留 Gemini CLI 既有后缀，没有新增 UI、依赖、配置或账本 owner。

59 项 Gateway 原生测试、另行启用的 2 项 SDK/官方 CLI 检查、63 项 ModelControl、74 项 Gateway 生命周期、8 项真实托管监听/快照集成和 17 项 renderer 测试通过。实际 native 客户端命令在挂起 SSE 期间完成新增、收窄权限、轮换和撤销；新请求取得预期 200/403/401，旧流最终成功，同一 UUID 的两条账本共 20 tokens。两包 all-target Clippy、ToYou lib 检查、root fmt 及定向 locale/格式通过。

15 份冻结输入和上一轮 81 份证据 artifact 哈希复验不变；96 项任务仍为 3 勾选/93 开放。周期额度、并发、完整历史、默认 verifier 迁移、Code 命名客户端选择、协议矩阵、真实桌面/平台与性能仍开放；此前全量类型、全 Gateway 库与全仓文档失败不因本轮定向通过而视为解决。证据：[授权热更新](../../docs/evidence/local-gateway/2026-11-03-client-authorization-reload/OUTCOME.md)。未 stage/commit/安装/重启应用/发布，完整替代目标保持 active。


### 2026-11-03：客户端日周月预算接线（全协议硬额度仍开放）

T022/T024 继续复用原 Native Preferences、Core budget/local 与 model_accounting。原预算页增加命名/默认客户端范围、周周期和提醒/停止选择，客户端行只跳同一预算页并预选稳定身份；不新建预算编辑器、quota store 或客户端内嵌额度副本。周周期按保存的固定 UTC offset 在周一零点重置，跨年及正负 offset 有实际内核测试；重置提示包含时间。超限返回 429、Retry-After 与 X-Should-Retry:false，稳定 budget_denied/reset 字段保留，原账户探针调用方的旧 402 预期随契约更新。

六组真实 Chat HTTP 场景覆盖日/周/月 × Token/USD：首笔消费、轮换后仍超限、其他客户端独立、经原 ACL writer 提高额度后立即生效；每组实际调用 3 次，客户端 A 的消费为 20 个对应单位，拒绝不触达上游。Core 9 项、公开 Gateway 60 项及另启用的 2 项 SDK/官方 CLI、ModelControl 63 项、accounting 14 项通过；renderer/shared 928 项/68 文件通过。两包 all-target Clippy、ToYou lib、root fmt、定向 lint/格式、i18n 与 renderer 构建通过。全量类型仍为相同文件/代码分布的 253 项诊断，最终 web 检查 231 项，修改文件没有主诊断。

扩大 Provider 调用方检查为 97 通过/2 失败/1 ignored；两项失败独立复现：managed credential 拒绝预期和 real OpenAI probe 的 invalid response，不改这两项现有断言或 fixture 取得通过。新测试首次误用偏好 scope，被原边界拒绝，改用原本地 UUID；旧内核拒绝 weekly、旧 HTTP 402 与旧账户探针 402 预期的失败记录均保留。

Completions/Embeddings 仍走旧分支，尚未受统一硬预算保护，预算页明确列出覆盖范围；需要继续接入同一执行/计量 owner，不能把新 UI 当作全协议客户端额度。客户端总并发、account scope、完整协议/SDK矩阵、历史/恢复、默认 verifier 迁移、真实桌面/其他 OS 和性能门禁仍未完成。15 份冻结输入及前两轮 81/40 份证据 artifact 保持不变。96 项任务的编号、顺序和 3 勾选/93 开放不变，完整目标保持 active。证据：[周期预算](../../docs/evidence/local-gateway/2026-11-03-client-periodic-budgets/OUTCOME.md)。未 stage/commit/安装/重启应用/发布。


### 2026-11-03 — Completions / Embeddings 共用执行、预算与账本

T011/T012/T018/T022/T024/T025 将两条旧直接上游分支接入既有 ModelControl、Core wire metering 与 model_accounting。显式操作元数据决定 embedding/text-completion 导出、目录与 setup probe；不按模型名猜测。四种 alias、原输入和响应方言保留；非法输入在上游前拒绝，客户端稳定身份来自原授权，取消保留未知消费，已交付流内容后不重放。没有新增 UI、依赖、预算存储或计量引擎；原预算覆盖说明更新为六类共享推理入口。

相同有效请求在修复前得到 200、违反 429 预算拒绝预期，失败日志保留。修复后 67 项公开 Gateway、25 项 Core wire、63 项 ModelControl、9 项 Agent plane caller 与另启用的 2 项 SDK/官方 CLI 通过；929 项 renderer/shared（68 文件）通过。两包 all-targets Clippy、root fmt、ToYou lib、定向格式/lint、i18n 与 renderer 构建通过。Provider 扩大检查为 98 通过/2 失败/1 ignored，两项失败与上一轮相同，现有断言/fixture 未改；全量 typecheck 仍为同分布 253 项诊断，不宣称全绿。

8 次命名客户端请求→8 次上游→8 条同一身份账本、36 tokens；真实 ProviderStore 的创建/更新、目录、host transport、probe 与账本另有实际调用。完整生产额度上界、客户端总并发、account scope、历史、完整协议/SDK/币种矩阵、Code 命名客户端选择、真实桌面/其他平台和性能仍开放。15 份冻结输入及前三轮 81/40/92 份证据 artifact 保持原字节；96 项任务仍为 3 勾选/93 开放，CAP 名称/需求/顺序/任务映射不变。完整目标保持 active，未 stage/commit/安装/重启应用/发布。证据：[共享计量接线](../../docs/evidence/local-gateway/2026-11-03-non-chat-shared-accounting/OUTCOME.md)。


### 2026-11-03 — 默认与命名客户端总并发接入原准入 guard

T020/T021/T024/T025 继续复用原 API 网关客户端区域、PageSidePanel、InputNumber、Native Preferences 与 IPC。默认客户端编辑同一区域的上限，命名客户端编辑原侧栏；不另建 limiter 服务、页面或周期额度副本。可选正 u32/null 在边界解析，旧记录可读，保存失败保留草稿。原 Gateway admission guard 追加稳定 credential UUID 的总占用，原路由计数/预留/退款保持不变；轮换、热更新启用及升降上限保留在途计数。

修复前两个真实 HTTP 案例都得到 200 而非 429；修复后默认/命名各覆盖 11 个入口的拒绝，单组上游 1 次/账本 1 条，轮换不清空占用、客户端独立、升降立即生效、取消后在 1 秒截止内释放。Gateway limiter 26、配置 45、授权 11、Studio 网关 70 及另启用 SDK/官方 CLI 2 项通过；932 项 renderer/shared（68 文件）通过。两包 all-targets Clippy、ToYou lib、root fmt、定向 lint/格式、i18n 与 renderer 构建通过。类型仍为同分布 253 项诊断；全仓 docs 仍有 244 处坏链接及 9 项 frontmatter 错误，不宣称全部通过。

15 份冻结输入、前四轮 81/40/92/77 份 artifact 及固定官方 CLI 输入保持原字节。任务编号、顺序和 3 勾选/93 开放不变，CAP 需求/名称/顺序/映射不变，仅更新 CAP09 进度。生产额度上界、完整协议/SDK/币种矩阵、account scope、历史、Code 命名客户端选择、客户端自查、真实桌面/其他 OS 和性能仍开放；完整目标保持 active。未 stage/commit/安装/重启用户应用/发布。证据：[客户端总并发](../../docs/evidence/local-gateway/2026-11-03-client-concurrency/OUTCOME.md)。


### 2026-11-03 — 既有模型限制接入消费上界与预算路由

T022/T023/T024 复用原服务商模型输入/上下文限制与价格、Core PricingSnapshot、ModelControl 和同一账本生成保守预留。实际 wire 请求的批次、n/best_of、Gemini 候选及明确输出上限计入；token ID 输入可用已知长度。缺事实、价格不全、后台或服务端工具执行保持未知；调用方 bounds 不能设定预留。原预算页只改已有说明，原模型侧栏继续维护限制/价格，不新增页面、预算字段或计量引擎。

18 组真实 ProviderStore→Gateway HTTP→账本覆盖六类操作 × Token/USD/CNY：首笔成功、次笔生成上游前拒绝，18 次真实上游/18 条账本、54 tokens；另有四组未知事实反例与 Native 非法覆盖不变测试。接线前 unknown_bound 的真实失败保留。全局预算另复现首选预留 72 放不进 25、后备 20 却被提前终止，现改为逐候选同一事务准入；首选 0 次、后备 1 次、唯一账本实际消费 10。原未知失败消费仍约束后备的回归通过。

Core metering 14、Gateway 72、ModelControl 67、accounting 14、另启用官方 SDK/CLI 2 项通过；932 项 renderer/shared（68 文件）通过。两包 all-targets Clippy、ToYou lib、root fmt、locale 格式/i18n 与 renderer 构建通过。Provider 99 通过/2 失败/1 ignored，两项旧失败完整函数未改；类型仍为同分布 253 项诊断，docs 244 坏链接/9 frontmatter 错误，不宣称全绿。

预留依赖正确的模型限制/价格配置，不是供应商物理上限或账单的独立证明；更紧输入计数、无限输出/服务端工具/后台的硬额度、全部流/取消/恢复及账户/周期交叉矩阵、真实供应商/桌面/其他 OS 与性能仍开放。15 个冻结输入及前五轮 81/40/92/77/90 份 artifact 保持原字节。96 项任务仍为 3 勾选/93 开放；CAP 需求/名称/顺序/映射不变，只更新 CAP09 进度。未 stage/commit/安装/重启用户应用/发布，完整目标保持 active。证据：[消费上界](../../docs/evidence/local-gateway/2026-11-03-provider-consumption-bounds/OUTCOME.md)。


### 2026-11-03 — 原生模型检查在计量故障时关闭直连旁路

T023 的 native ai_model_check 在共享 ModelControl 不可用时仍有同步 curl fallback；旧 HTTP 本地检查入口已经拒绝该路径。先用真实损坏 SQLite 和缺宿主 controller 复现旧代码仍成功返回 latency，再删除未计量分支，复用原 422 ModelCheckError；托管组织路径、Codex、原文本生成及原 UI 错误通道保持各自既有规则。健康的 disabled 配置检查仍经同一预算/账本，预留 20、消费 3，额度 21 下第二笔拒绝，上游 1 次/账本 1 条，实际 wire ID 与凭据身份正确，配置不启用不重写。

2 项新原生回归包含在 Provider 101 通过/2 失败/1 ignored 中；ModelControl 67、Gateway 72、renderer/shared 933（68 文件）通过。Studio all-targets Clippy、ToYou lib、root fmt、定向 lint/格式与 renderer 构建通过。类型仍为同分布 253 项诊断；docs 244 坏链接/9 frontmatter 错误，不宣称全绿。两项旧 Provider 失败在独立临时根复现，函数/断言/fixture 未改；旧 Aster 拒绝 BYOK 与 FR-007、旧 Chat 探针与出厂 Responses 目录的分歧另行记录，不修改产品契约来迎合旧断言。

15 个冻结输入和前六轮 81/40/92/77/90/81 份 artifact 保持原字节；任务编号、顺序和 3 勾选/93 开放不变，CAP 需求/名称/顺序/映射不变，仅更新 CAP09 进度。旧测试契约对齐、完整客户端/账户/周期/流/恢复矩阵、真实供应商与桌面/其他 OS/性能/G3 继续开放。未 stage/commit/安装/重启用户应用/发布，完整目标保持 active。证据：[探针计量准入](../../docs/evidence/local-gateway/2026-11-03-probe-accounting-admission/OUTCOME.md)。


### 2026-11-03 — Code 文件型 CLI 选择命名客户端（完整替代仍开放）

T020/T021/T024/T105 的 Code 接线复用原网关配置对话框、SearchSelect、密码输入、模型选择和原始文件编辑器。原 Code 每工具/服务商配置记录保存稳定 client UUID、签发 generation 和调用方密钥；网关客户端记录仍只保存 verifier，上游 provider 凭据不合并。模型目录按客户端模型/路由、接口授权和具体 CLI 能力取交集，四种 alias 继续由既有 owner 解析。选择、预览、保存、启用、当前连接识别和启动使用同一连接；默认客户端只能经显式选择切回，命名客户端缺失/撤销/轮换或读取失败时不回退默认密钥。

实际配置保存回归先失败，确认旧 upsert 会丢弃客户端字段。真实共享控件测试覆盖选择客户端、输入密钥、保存失败保留草稿和重试；实际 CLI draft writer 的启动测试验证写入所选 key、正确 wire alias、列表已显示后发生撤销时零写文件/零启动。旧原始文件含另一命名 key 时先要求明确选择；客户端元数据未完成加载时，旧实现仍会写默认连接的失败记录保留，修复后拒绝配置改写。前端回归与检查、具体计数及原始日志见 [Code client evidence](../../docs/evidence/local-gateway/2026-11-03-code-gateway-clients/OUTCOME.md)。

本轮没有新页面、凭据管理服务、预算/计量 owner、依赖或 Rust 宿主入口。Code 作为客户端需要保存连接密钥，UI 已明确说明该密钥会随原 Code 配置保存；这不等于网关能重新读取已签发的秘密。运行时专用 Agent 的命名连接、所有 Agent/协议/版本、真实桌面、跨平台及 G3 继续开放。96 项任务保持 3 勾选/93 开放；CAP 需求、名称、顺序和任务映射不变，仅更新 CAP09/CAP14 进度。没有安装、重启用户应用、提交或发布，完整替代目标保持 active。


### 2026-11-03 — Aster Hermes 主入口使用既有原生 CLI 链路

T053/T104/T105 复核发现 Hermes 已有文件 adapter、原生 writer 与 terminal launcher，页面却优先调用原生 bridge 不支持的 `hermes_dashboard.start`，并受旧 Dashboard 状态锁定。现在默认 Aster 的 Hermes 使用原启动对话框和同一目录/终端、模型与命名客户端连接；旧运行状态不显示错误的托管控制或锁住配置/升级。卸载先执行原配置恢复，不调用不可用的 Dashboard stop。其他租户原 Dashboard 分支保留。

两个页面回归在修复前真实失败。修复后 969 项 renderer/shared（72 文件）通过；53 项原生 Code CLI 检查通过、1 个官方 CLI opt-in 保持 ignored。原生新场景经实际 config writer、ToolStore 和 launcher 运行隔离替身进程，证明带中文/空格/引号的 HERMES_HOME 与密钥文件传递正确，不启动用户终端。renderer 的 Hermes 场景使用实际 YAML/dotenv adapter、key resolver、writer 和解析器；native IPC 为边界 fixture，完整真实 Hermes 模型调用未声称完成。Clippy all-targets exit 0（保留其他文件的现有警告）、root fmt、定向 lint/format 和 renderer 构建通过；全量类型仍为同文件/错误码分布的 253 项诊断。

AG27/AG42 的实际缺口已确认：现有 DeepSeek Harness/OpenClaw renderer IPC 在原生 bridge 返回 ROUTE_NOT_FOUND，不能只补客户端选择就视为生效。核对冻结参考 Git blob，dsh 需要版本化 profile patch、.env 与独立 credential store；OpenClaw 需要其 HOME/STATE_DIR/PROFILE/CONFIG_PATH 与 provider/model 配置契约。这些原生适配及两者完整 UI/runtime 仍属于原 T105，未重连退休服务或缩减需求。 同一冻结参考还要求 Hermes 的 provider catalog、活动 profile、多机器人同步与对应撤销；本轮只接通主 CLI 入口，这些项目仍未验收。AG33 仅更新已验证进度，Agent ID/名称/顺序与任务映射不变，96 项任务保持 3 勾选/93 开放。完整替代、真实桌面/Agent/供应商、跨平台和 G3 继续开放；未安装、重启应用、提交或发布。证据：[Hermes native entry](../../docs/evidence/local-gateway/2026-11-03-hermes-native-launch/OUTCOME.md)。

## 2026-11-03 Hermes 活动 profile 原生接线（T052/T053/T104/T105 进度，未关闭完整任务）

修复前的原生回归复现：sticky profile 读到根配置、相同/缺失文件的旧草稿在切换后仍可保存，以及无效 profile 没有拒绝读取。现在沿用原 Code 配置与终端入口，在原生操作开始捕获有效 profile，并将草稿 revision 绑定到 profile；旧草稿经原 config-conflict 路径拒绝，未产生文件或恢复记录。sticky profiles 在原 recovery owner 中分别保留原始字节和归属，支持各自预览、部分恢复与撤销；默认配置保留既有记录。原生 launcher 传递所解析的 HERMES_HOME，实际隔离替身从该目录读取 YAML 与所选客户端密钥文件。平台默认目录同步使用现有环境入口，保留显式 HERMES_HOME、Windows LOCALAPPDATA 和数据目录 suffix。

完整 Agent Profile 的 UI 保存/应用/删除、provider/model catalog、所有机器人同步、官方 Hermes/供应商请求、真实桌面及跨平台验收仍开放。显式 HERMES_HOME 的旧恢复记录保持原行为，不将 sticky profile 切片当成全部 profile 管理。冻结 Agent/CAP 身份、任务映射与 checkbox 状态不变。证据：[Hermes active profile](../../docs/evidence/local-gateway/2026-11-03-hermes-active-profile/OUTCOME.md)。
