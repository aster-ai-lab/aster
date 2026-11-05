# Gateway Contract: Magpie 兼容对照表

**Reference**: Magpie 0.1.2141。新增需求文档 revision `9162277b91c9327773b5a1e0413d59f24de6f3af` 与二进制的差异由 T005/T007 审查。每项必须有离线 fixture、现有通过测试或明确未完成状态。协议语义一致不要求供应商错误文本逐字一致。这是目标契约，不是当前支持声明。

## Identity and addressing

| Item | Required behavior |
| --- | --- |
| Model | `provider/model` 寻址到声明的供应商模型；`provider:model` 为旧 Aster 别名，结果相同 |
| Group | `group/id` 使用声明的路由组；裸 `id` 为旧 Aster 别名 |
| Unknown model/group | 返回 model-not-found，不猜测替代模型 |
| Client key | 使用配置生成的密钥；支持后续命名密钥轮换与撤销，不硬编码 `magpie` 为通用密钥 |
| Upstream credential | 由现有 ProviderStore/授权来源拥有，宿主按实际 target 注入；不进入生成的网关 ACL、日志、Debug、用量或默认导出 |

## HTTP surface

| Priority | Method & path | Contract |
| --- | --- | --- |
| P1 | `GET /v1/models` | 返回 `object=list` 与可授权的模型 ID；无凭据 401 |
| P1 | `POST /v1/chat/completions` | OpenAI JSON、SSE、工具、usage、错误与取消 |
| P1 | `POST /v1/responses` | OpenAI Responses JSON/SSE、工具、推理与终止 |
| P1 | `GET /v1/responses` WebSocket upgrade | 冻结新版本的 Responses WS、continuation/tool/推理事件与取消；T140 独立验收 |
| P1 | `POST /v1/messages` | Anthropic Messages、SSE、工具、推理签名与终止 |
| P1 | `POST /v1/messages/count_tokens` | Anthropic token 估算/上游语义与错误 |
| P1 | `GET /v1beta/models` | Gemini 可见模型发现、模型权限、分页/查询语义 |
| P1 | `POST /v1beta/models/{model}:generateContent` | Gemini JSON、工具与错误 |
| P1 | `POST /v1beta/models/{model}:streamGenerateContent?alt=sse` | Gemini SSE、工具、推理与终止 |
| P1 | `POST /v1beta/models/{model}:countTokens` | Gemini token 计数与错误 |
| P2 | `POST /v1/embeddings` | 声明支持时的 embeddings 与模型权限 |
| P2 | `POST /v1/completions` | OpenAI Completions 与 usage |
| P2 | `POST /v1/rerank` | 独立 rerank 模型与 query/documents/indices/scores/usage，T139 |
| P2 | `POST /v1/audio/transcriptions`、`/translations` | multipart 音频原字节、model rewrite、vendor 格式/错误，T138 |
| P2 | `POST /v1/audio/speech` | JSON 输入、音频/JSON/SSE 响应、厂商字段及取消，T138 |
| P2 | `GET /api/version`、`GET /api/tags`、`POST /api/show` | Ollama-compatible discovery 元数据及不支持操作的明确拒绝，T141 |
| P2 | Image/video/MCP | 独立阶段验收，不能用旧 ElysiaJS 端点冒充完成 |
| P2 | Decision/System One | choice/noul/score、decision-only group、与 chat 不混用；T136 独立验收 |
| P2 | Management/introspection | limit/quota/route/concurrency/history/usage submit/Codex catalog/诊断；T137 验证等价入口、权限与分页，不要求 Magpie 路径拼写或 Go 数据格式 |

## Authentication and errors

| Condition | Expected behavior |
| --- | --- |
| Missing/invalid client key | 401；不读取请求上游，不产生供应商调用 |
| Key lacks model | 允许区分的已认证本地场景按参考返回 403；principal/跨租户场景保留既有 404 隐藏契约；均不调用上游 |
| Unknown model/group | model-not-found，不选其他模型 |
| Quota exhausted | 429 与重置信息；不消耗上游 |
| Upstream 401/403 | 供应商配置错误，不盲目切换 |
| Retryable upstream failure before first semantic response | 按策略最多尝试声明的候选；最终请求只计一次 |
| Failure after semantic response | 返回/终止当前流，不向其他候选重放 |
| Invalid config or unreadable store | 启动失败并说明原因；不写 enabled=true |
| Port conflict | 启动失败、running=false、保留原状态与错误文本 |

## Streaming and conversion

- 事件顺序必须与参考协议一致；每个事件只出现一次，终止事件只出现一次。
- 工具调用 ID、名称、参数增量和 JSON 完整性保持正确，跨协议不重复或丢参。
- 推理内容、签名与工具结果保持可用；不能用普通文本拼接伪装推理。
- 客户端取消时取消上游工作；新请求不能继承旧请求上下文。
- 非流式响应只在完整成功后提交；失败不得截断为成功空响应。
- 跨协议只转换一次，远程共享不得重复转换。
- 计量读取转换前的上游用量；协议兼容默认值不能成为账本实测值。未知用量保持未知，配额预留保持至客户端响应结束。

## Routing

- `fixed/order/rotate/usage/pace/smart/weight/free/fast` 是计划范围；frozen reference 明确 weight/fast，free 由 T005 核实为参考行为或 Aster 扩展。未实现策略拒绝配置，不静默回退为 fixed。
- 嵌套组只展开显式成员；空组与循环引用拒绝。
- 401/403 不等同于可重试失败；可重试集合与状态码必须由策略声明。
- 会话粘性只影响同一会话的候选选择，不跨会话泄漏账户或密钥。
- 实际回答账户与密钥是用量归因来源，不是请求时首选账户。

## Persistence and privacy

- 客户端密钥、账户、路由、目录和用量重启后保持一致。
- 配额预留与提交必须原子化；并发下不得超扣或重复扣减。
- 原始凭据只留现有授权凭据 owner；不得进入生成的网关 ACL、普通配置输出、日志、Debug、用量、默认导出或遥测。不能把此约束解释为重写现有 ProviderStore 存储。
- 脱敏只在内存保存映射；要求脱敏但引擎不可用时拒绝发送。
- Agent 原生配置文件的写入保持注释、顺序和格式；不被纳入 Aster 产品配置发现。

## Current admission versus target

| Surface | 2026-11-01 代码状态 | 开放要求 |
| --- | --- | --- |
| models/Chat/completions/embeddings | Gateway admission 已开放；宿主主要只导出 chat 模型 | T010/T019 验证实际模型和完整请求，不以 admission 代替能力 |
| Responses | POST HTTP 已准入，原生与 Chat 转换复用既有 codec/usage；Studio 接同一 ModelControl。WS 未实现 | T011/T018：完整协议、auth/grant、计量、managed/principal、快照撤销及失败路径一起通过；WS 另见 T140 |
| Messages/count_tokens | 两条 POST 已准入；Messages 原生及 Chat 转换的 JSON/SSE 接同一 ModelControl；计数独立授权，复用同模型原生 counter 或显式标记估算，不记录生成消费 | T012/T017/T018 的完整跨协议、SDK、目录/错误与快照撤销仍待验收；计数不等同于生成调用 |
| Gemini | Gateway 原生 generate/stream/count/models 边界的 20 项专项（含官方 SDK 回环）已通过；embed 仍 Reserved。Studio 原生内核已有，生产 ProviderStore/私有 HTTP/UI 尚未接通；Cloud spool/principal 场景关闭。分布式模块并行回归仍有失败 | T013/T017/T018 继续完成宿主接线、计数、codec、生产 SDK/平台及分布式回归诊断；Gateway 专项不代表 Studio 用户已能调用 |
| image/audio/rerank/MCP/decision | 各自存在不同 admission 或普通 proxy/平台约束 | T007/T116/T118/T136/T138/T139 逐端点核对，不能互相推定 |

## Differential fixture matrix

每个声明支持的协议及转换组合都记录以下 case；不适用时给出协议原因。

| Case family | 比较内容 | 反例 |
| --- | --- | --- |
| Discovery | 可见 ID、canonical/alias、能力、顺序/启停与按 Agent 视图 | 未授权/隐藏/未知模型不猜替代 |
| JSON | system/user/assistant、结构化内容、输出、finish、usage、headers | 无效/过大 body、未知字段的明确处理、非 JSON/截断上游 |
| SSE | 协议 start/delta/end、工具 ID/name/argument、reasoning/signature、usage、终止一次 | 每个 chunk 切分点、并行工具、缺 end、上游中断 |
| Stateful | Responses continuation、call/result 关联、会话/turn 身份 | 不支持的 stateful translation 明确拒绝，不吞字段 |
| Retry/cancel | typed status/错误、在途取消、首响应前后失败、资源释放 | 向下游发出语义事件（含 response/message/tool start）后重放为 0 |
| Auth/quota | client/key/model/endpoint、窗口/price revision、429/reset | 未授权和超限 upstream calls 为 0；不把未知消费当零 |
| Identity/usage | 最终 request 与全部 attempts、实际 key/account、cache/cost | 首选失败/备用成功、轮换/删除/重启历史稳定 |
| Privacy/plugins | 最终发出内容、占位符恢复、hook 顺序与权限 | 插件改 model 后再次授权；插件新增内容仍经过必需脱敏 |

两种承诺分别处理：HTTP headers 已提交后不能更换 HTTP 状态；任一已发协议语义事件之后不能重放到新上游。metadata/heartbeat 是否已形成协议语义由具体 fixture 明确，不靠“还没文本”判断。

## Evidence record

每个 case 至少关联 CAP/AG ID、task、reference binary/doc hash、request/corpus revision、fixture upstream、Aster revision/features、预期/实际状态与事件序列、upstream call count、test command/count、native/live/corpus-only 标志和脱敏 evidence 路径。这里只列目标，不生成伪造通过记录。
