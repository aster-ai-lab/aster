# Frozen capability inventory

**Inventory revision**: 2026-11-01；上游 `9162277b91c9327773b5a1e0413d59f24de6f3af`。

来源：[README](https://github.com/yetone/magpie/blob/9162277b91c9327773b5a1e0413d59f24de6f3af/README.md)、[reference](https://github.com/yetone/magpie/blob/9162277b91c9327773b5a1e0413d59f24de6f3af/docs/reference.md)。Plugins/intent/privacy/MCP/remote/docker 的站点文档为补充，Phase A 记录其快照摘要、日期与和二进制 0.1.2141 的差异。

这是需求与任务映射，不是完成证明。状态只有“有基础”“明确未准入”“待对齐验收”；有基础仍需走完整用户场景。A 阶段继续检查每节细项，发现遗漏新增稳定 CAP ID，不以合并行删除范围。Agent 独立逐行见 [agents.md](agents.md)。

| ID | 业务能力及必须核对的子项 | 当前状态 | Owning tasks | 阶段 |
| --- | --- | --- | --- | --- |
| CAP01 | 网关 start/stop/restart/status、恢复 enabled intent、端口冲突、旧模型 alias | 有基础 | T001 T002 T015 | A/B |
| CAP02 | models、Chat、Responses、Messages/count_tokens、Gemini generate/stream/count、跨协议工具/推理/usage/取消、原生 passthrough | 六类推理入口接同一宿主，计数独立授权；完整协议/SDK、平台、错误与性能矩阵仍待验收 | T010 T011 T012 T013 T014 T017 T018 T019 | B |
| CAP03 | Provider 预设/自定义、协议与能力、动态模型、真实 upstream 名称、模型等价 alias、输入类型与 effort | 有基础 | T016 T040 T046 T047 T130 | B/E |
| CAP04 | 从 CC Switch/Claude/Codex/Alma/OpenCode 导入、网站配置导入链接、显示预览并确认、格式/来源校验 | 待对齐验收 | T130 | E |
| CAP05 | 每 provider 多 key、账户模型、启停、轮换、每 key/账户代理、实际身份与失败冷却 | 部分 key 存储已有，网关只导出一个 | T026 T030 T036 T038 | C/D |
| CAP06 | Claude 本地 CLI/MCP bridge、ChatGPT、Copilot、Gemini Code Assist、Antigravity、Grok 及冻结参考其余订阅；登录、发现开关、刷新、quota windows、账户切换 | 有部分账户基础 | T030 T037 T038 T039 | D |
| CAP07 | order/rotate/usage/pace/smart、配额/重置选择、nested、会话/cache 粘性、前响应 failover | 仅 Fixed/Ordered 本地配置 | T031 T032 T033 T034 T035 T036 | D |
| CAP08 | 按 agent/长度/图像/reasoning 的显式条件、模型驱动 intent、turn 粘性、effort override、实时路由解释 | 待对齐验收 | T100 | D |
| CAP09 | 命名客户端 key、模型范围、撤销/轮换、日周月 token/cost、并发和未知 usage、历史 key 身份 | 命名 key、热更新、周期预算/并发、六接口共享计量与模型检查准入已接通；Code 文件型 CLI 可选命名客户端；历史、完整生产/桌面矩阵仍开放 | T020 T021 T022 T023 T024 T025 | C |
| CAP10 | 实际 key/账户用量、token/cache/reasoning、价格来源/自定义价格/币种、余额窗口、低使用量提醒、warm-up/check-in | 有 accounting 基础 | T039 T041 T042 T043 T044 T046 | D/E |
| CAP11 | 会话标题/成本/续开、context 占用和 cache 构成、账单聚合、OTLP、路由观测 | 有部分会话/观测基础 | T036 T043 T045 T100 | D/E |
| CAP12 | 请求脱敏和响应恢复、所有 chunk 切分/tool arguments、稳定映射/签名/cache、内存映射与失败关闭 | 有 wire 基础，未完成等价接线 | T051 T101 | F |
| CAP13 | secret/PII 开关、Mask/Warn/Block、Never mask、自定义精确值/机械规则、可选本地模型、Try it/记录页 | 待对齐验收 | T102 T103 | F |
| CAP14 | 每 Agent 选模型/effort/小模型、独立可见模型列表、保留注释/顺序/缩进、原子写、接管撤销、环境/安装路径 | 文件型 CLI 命名连接与 API 别名已接；Aster Hermes 原生入口及活动 profile 读写/启动/独立恢复切片已验；完整 Agent、参数、平台与桌面矩阵未完成 | T047 T053 T104 T105 | E/G |
| CAP15 | Profile 全 Agent 快照/切换/当前标记；Library 的指令、MCP、skills、subagents 分发与仅撤销托管内容 | 待对齐验收 | T052 T107 | G |
| CAP16 | 安装检测、版本/升级来源、命令提示、会话续开、Windows/WSL 独立实例 | 有部分管理基础 | T045 T106 T127 | E/G/K |
| CAP17 | OpenCode 与 pi auth provider；npm/git/local 包、登录/刷新/models/usage/check-in、runtime 安装/修复 | 自有插件系统不等于兼容 | T109 T110 T111 | H |
| CAP18 | async request hook、onRequest/onEvent/onResponse、中间件顺序/选项、异常/超时、预置 param/model/system/word/think 变换 | 待对齐验收 | T050 T112 | H |
| CAP19 | 插件工具、Agent 扩展、页面/发现/状态；安装/升级/撤销/崩溃恢复和最小能力 | 有自身 lifecycle 基础 | T111 T113 | H |
| CAP20 | 模型搜索/搜索 API、有序搜索源、启停不丢 key、多 key rotate/order、冷却、key quota、web_fetch | 有 search providers 基础 | T114 T115 T119 | I |
| CAP21 | 图像生成/编辑、视觉、视频（含支持的订阅/API）、CLI 直接调用、产物下载/路径、异步状态/取消 | 需核对真实 provider 与外部入口 | T116 T117 T119 | I |
| CAP22 | 对外 tools MCP、服务连接/发现、MCP router、工具同名消歧与按客户端可见性 | 有 MCP 基础 | T118 | I |
| CAP23 | LAN 开关/绑定、每客户端授权、远端 gateway 的 providers/accounts/groups 访问、本机 Agent 接线、不重复翻译 | 有绑定选项，完整链未验收 | T060 T120 | J |
| CAP24 | 文件备份/导入、WebDAV/S3/GitHub 保护的同步、网关成员、冲突/恢复、凭据边界 | 通用备份不证明此项完成 | T061 T121 T122 T123 | J |
| CAP25 | 桌面/托盘/普通窗口、CLI/TUI、无头 serve、web 管理的同一权威与读写语义 | 有分散入口基础 | T124 T125 T126 T128 | K |
| CAP26 | 开机恢复、后台服务、保持唤醒、明暗/系统主题、Linux Omarchy 适配 | 待逐平台对齐验收 | T128 T129 | K |
| CAP27 | Docker/server/NAS、持久卷、升级/卸载保留与显式清理、安装包所需 runtime/companion | 待对齐验收 | T129 T132 T135 | K/L |
| CAP28 | macOS/Linux/Windows、适用架构和 WSL native、迁移、性能/资源、最终安装实测、逐项证据 | 未完成全量资格验收 | T070 T071 T072 T073 T131 T132 T133 T134 T135 | L |
| CAP29 | decision/System One 模型、choice/noul/score、decision-only group、组分类器与非 chat catalog | Gateway 有平台入口，参考语义待验收 | T136 T100 | D |
| CAP30 | 客户端自查 limit、quota cards/refresh/wait、route/concurrency、paged usage/history、工具外消费提交、管理诊断及 Codex 专用模型目录 | 有分散底层能力，完整入口待验收 | T137 T043 T045 T126 | E/K |
| CAP31 | audio transcriptions/translations 的 multipart 原字节转发，以及 speech 的 audio/JSON/SSE、模型/费用/脱敏/取消 | 普通 proxy 已准入；inference-policy 路由拒绝，统一授权/计量与宿主投影待接线 | T138 T016 T018 | B/I |
| CAP32 | rerank query/documents、索引/score、top_n/return_documents、模型能力、usage/错误 | Platform 准入与 request profile 已有；宿主模型投影、统一授权/计量及真实调用待验收 | T139 T016 T019 | B |
| CAP33 | 新版本 Responses WebSocket 握手/事件/continuation/工具/取消，与 HTTP/SSE 共用授权/配额 | 缺完整 native 契约验证 | T140 T011 T018 | B |
| CAP34 | Ollama-compatible catalog、agent-specific discovery、完整元数据与只支持无 key 客户端的认证适配 | 缺端到端替代证明 | T141 T047 T105 | B/E/G |
| CAP35 | 既有 Magpie 用户配置/模型/路由/Profile/Library/可迁移用量的显式导出导入、预览、冲突与失败回滚 | 未实现完整迁移 | T142 T130 T132 | E/J/L |

## 完成与差异规则

- 每行拆到独立 fixture 时记录 `CAP ID → 子项 → binary/doc revision → test → result → evidence`，不能一个成功 endpoint 覆盖整行。
- 每 provider/订阅有实际模型、headers、token refresh、工具和 quota 的适用说明。离线成功不算实时账户通过。
- 只有参考明确不支持的平台/操作，或产品书面排除并调整声明，才标 N/A；缺设备/凭据/开发能力都记未验收。
- weight/free/fast 是旧规格要求，须单独确定它们是参考能力还是 Aster 扩展，不改名替代 smart，也不静默删除。
- G3 只能绑定此 frozen inventory 及其已记录的补项。上游后续变化另做差异审查。
- 用户于 2026-11-01 明确要求完整替代。T143 冻结当次官方最新 v0.1.2523 的增量；0.1.2141 保留回归。旧主表遗漏的 6 个 Agent、audio/rerank、WebSocket 和 Ollama catalog 均为必须核对项，不能以原任务缺少专用行而排除。
