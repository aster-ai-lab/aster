# Gateway Contract: Magpie 兼容对照表

**Reference**: Magpie 0.1.2141。每项必须有离线 fixture、现有通过测试或明确未完成状态。协议语义一致不要求供应商错误文本逐字一致。

## Identity and addressing

| Item | Required behavior |
| --- | --- |
| Model | `provider/model` 寻址到声明的供应商模型；`provider:model` 为旧 Aster 别名，结果相同 |
| Group | `group/id` 使用声明的路由组；裸 `id` 为旧 Aster 别名 |
| Unknown model/group | 返回 model-not-found，不猜测替代模型 |
| Client key | 使用配置生成的密钥；支持后续命名密钥轮换与撤销，不硬编码 `magpie` 为通用密钥 |
| Upstream credential | 由宿主按实际 target 注入，不进入 ACL、日志、Debug 或用量记录 |

## HTTP surface

| Priority | Method & path | Contract |
| --- | --- | --- |
| P1 | `GET /v1/models` | 返回 `object=list` 与可授权的模型 ID；无凭据 401 |
| P1 | `POST /v1/chat/completions` | OpenAI JSON、SSE、工具、usage、错误与取消 |
| P1 | `POST /v1/responses` | OpenAI Responses JSON/SSE、工具、推理与终止 |
| P1 | `POST /v1/messages` | Anthropic Messages、SSE、工具、推理签名与终止 |
| P1 | `POST /v1/messages/count_tokens` | Anthropic token 估算/上游语义与错误 |
| P1 | `POST /v1beta/models/{model}:generateContent` | Gemini JSON、工具与错误 |
| P1 | `POST /v1beta/models/{model}:streamGenerateContent?alt=sse` | Gemini SSE、工具、推理与终止 |
| P1 | `POST /v1beta/models/{model}:countTokens` | Gemini token 计数与错误 |
| P2 | `POST /v1/embeddings` | 声明支持时的 embeddings 与模型权限 |
| P2 | `POST /v1/completions` | OpenAI Completions 与 usage |
| P2 | Image/video/MCP | 独立阶段验收，不能用旧 ElysiaJS 端点冒充完成 |

## Authentication and errors

| Condition | Expected behavior |
| --- | --- |
| Missing/invalid client key | 401；不读取请求上游，不产生供应商调用 |
| Key lacks model | 403；不调用该模型上游 |
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

## Routing

- `fixed/order/rotate/usage/pace/weight/free/fast` 均为显式封闭策略；未实现策略拒绝配置，不静默回退为 fixed。
- 嵌套组只展开显式成员；空组与循环引用拒绝。
- 401/403 不等同于可重试失败；可重试集合与状态码必须由策略声明。
- 会话粘性只影响同一会话的候选选择，不跨会话泄漏账户或密钥。
- 实际回答账户与密钥是用量归因来源，不是请求时首选账户。

## Persistence and privacy

- 客户端密钥、账户、路由、目录和用量重启后保持一致。
- 配额预留与提交必须原子化；并发下不得超扣或重复扣减。
- 原始凭据不得进入 ACL、普通配置输出、日志、Debug、用量、导出或遥测。
- 脱敏只在内存保存映射；要求脱敏但引擎不可用时拒绝发送。
- Agent 原生配置文件的写入保持注释、顺序和格式；不被纳入 Aster 产品配置发现。
