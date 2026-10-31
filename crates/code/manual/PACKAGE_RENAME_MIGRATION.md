# 包命名外部迁移清单

本文面向调用 Aster Code Core、消费其事件或读取其持久化数据的宿主。此次调整把“认知包”统一改为“包”，并同步修改 Rust API、协议字符串和序列化字段。

这是一次破坏性改名：当前源码没有保留旧名别名。调用方需要显式迁移，不能只升级依赖后沿用旧代码或旧数据。精确代际绑定、宿主重新注入 provider、失败关闭等行为没有改变。

## 1. 修改 Rust 导入和类型

模块路径：

| 旧名称 | 新名称 |
| --- | --- |
| `aster_core::cognitive_context` | `aster_core::package_context` |

公开类型：

| 旧名称 | 新名称 |
| --- | --- |
| `CognitivePackageBindingV1` | `PackageBindingV1` |
| `CognitiveKnowledgeBindingV1` | `PackageKnowledgeBindingV1` |
| `CognitiveKnowledgeCitationV1` | `PackageKnowledgeCitationV1` |
| `CognitiveContextDocumentV1` | `PackageContextDocumentV1` |
| `CognitiveContextError` | `PackageContextError` |
| `CognitiveContextLimits` | `PackageContextLimits` |
| `CognitiveContextProvider` | `PackageContextProvider` |
| `CognitiveContextRequestV1` | `PackageContextRequestV1` |
| `CognitiveContextResponseV1` | `PackageContextResponseV1` |
| `CognitiveContextResult<T>` | `PackageContextResult<T>` |
| `CognitiveContextSession` | `PackageContextSession` |
| `RunCognitiveBindingError` | `RunPackageBindingError` |

这些类型仍从 `aster_core` 根模块重新导出；使用根模块导入时，只需修改类型名。

## 2. 修改宿主、会话和 Run 调用

| 旧调用或字段 | 新调用或字段 |
| --- | --- |
| `SessionOptions::cognitive_context` | `SessionOptions::package_context` |
| `SessionOptions::with_cognitive_context(...)` | `SessionOptions::with_package_context(...)` |
| `ContextProvider::cognitive_package_binding()` | `ContextProvider::package_binding()` |
| `AgentSession::cognitive_package_binding()` | `AgentSession::package_binding()` |
| `AgentSession::current_cognitive_package_binding()` | `AgentSession::current_package_binding()` |
| `InMemoryRunStore::bind_cognitive_package(...)` | `InMemoryRunStore::bind_package(...)` |
| `RunSnapshot::cognitive_package_binding` | `RunSnapshot::package_binding` |
| `SessionData::cognitive_package_binding` | `SessionData::package_binding` |
| `EvidenceRunStateV1::cognitive_binding_digest` | `EvidenceRunStateV1::package_binding_digest` |

最小调用示例：

```rust
use aster_core::{PackageContextSession, SessionOptions};

fn options_with_package(context: PackageContextSession) -> SessionOptions {
    SessionOptions::default().with_package_context(context)
}

fn current_binding(session: &aster_core::AgentSession) {
    let binding = session.current_package_binding();
    // 使用 binding；恢复会话时仍需由宿主重新注入对应 provider。
}
```

如果自定义了 `ContextProvider`，还要把实现中的 `cognitive_package_binding()` 改为 `package_binding()`，返回类型同步改为 `Option<&PackageBindingV1>`。

## 3. 修改能力目录和事件处理

| 旧名称 | 新名称 |
| --- | --- |
| 能力 ID `cognitive_packages` | 能力 ID `packages` |
| 操作 `session.cognitive_package_binding` | 操作 `session.package_binding` |
| 操作 `session.current_cognitive_package_binding` | 操作 `session.current_package_binding` |
| `AgentEvent::CognitiveContextBound` | `AgentEvent::PackageContextBound` |
| 常量 `COGNITIVE_CONTEXT_BOUND` | 常量 `PACKAGE_CONTEXT_BOUND` |
| 事件字符串 `cognitive_context_bound` | 事件字符串 `package_context_bound` |

检查事件分发、日志分类、指标标签、审计消费者和持久化事件解析器，确保它们识别 `package_context_bound`。评估证据中的摘要域名也要修改：

| 旧域名 | 新域名 |
| --- | --- |
| `aster.code.evidence.cognitive-binding.v1` | `aster.code.evidence.package-binding.v1` |

## 4. 修改协议常量和字符串

| 旧常量 | 新常量 | 新字符串值 |
| --- | --- | --- |
| `COGNITIVE_PACKAGE_BINDING_SCHEMA` | `PACKAGE_BINDING_SCHEMA` | `aster.code.package-session-binding.v1` |
| `COGNITIVE_KNOWLEDGE_BINDING_SCHEMA` | `PACKAGE_KNOWLEDGE_BINDING_SCHEMA` | `agentic.ontology.r0-knowledge-lease-binding.v1`（值不变） |
| `COGNITIVE_CONTEXT_REQUEST_SCHEMA` | `PACKAGE_CONTEXT_REQUEST_SCHEMA` | `aster.code.package-context-request.v1` |
| `COGNITIVE_CONTEXT_RESPONSE_SCHEMA` | `PACKAGE_CONTEXT_RESPONSE_SCHEMA` | `aster.code.package-context-response.v1` |
| `COGNITIVE_CONTEXT_REQUEST_DIGEST_DOMAIN` | `PACKAGE_CONTEXT_REQUEST_DIGEST_DOMAIN` | `aster.code.package-context-request.v1` |
| `COGNITIVE_CITATION_METADATA` | `PACKAGE_CITATION_METADATA` | `aster.package.citation` |
| `COGNITIVE_PACKAGE_BINDING_METADATA` | `PACKAGE_BINDING_METADATA` | `aster.package.binding` |

对应的旧字符串值分别为：

- `aster.code.cognitive-package-session-binding.v1`
- `aster.code.cognitive-context-request.v1`
- `aster.code.cognitive-context-response.v1`
- `aster.cognitive.citation`
- `aster.cognitive.package_binding`

还要检查以下上下文载体：

| 旧值 | 新值 |
| --- | --- |
| 元数据键 `aster.cognitive.source_digest` | 元数据键 `aster.package.source_digest` |
| provenance `aster-use-cognitive-package` | provenance `aster-use-package` |
| 文本前缀 `[cognitive citation=...]` | 文本前缀 `[package citation=...]` |

OKF 搜索、读取和引用 schema 值，以及 `aster.use.capability-snapshot.v1` 摘要域名保持不变。不要扩大迁移范围去修改这些未变化的合同。

## 5. 迁移持久化数据

先备份并停止写入，再迁移自己拥有的数据。不要直接改写仓库中的冻结协议夹具或历史证据。

需要检查的序列化字段和值：

- Session 与 Run 快照：`cognitive_package_binding` → `package_binding`。
- 评估证据：`cognitive_binding_digest` → `package_binding_digest`。
- 持久化事件：`cognitive_context_bound` → `package_context_bound`。
- 绑定对象内的 `schema`：`aster.code.cognitive-package-session-binding.v1` → `aster.code.package-session-binding.v1`。
- 上下文请求与响应内的 `schema`：分别改为 `aster.code.package-context-request.v1` 和 `aster.code.package-context-response.v1`。
- 上下文元数据、provenance 和证据摘要域名：按第 3、4 节同步修改。

不要假定旧摘要仍然有效。使用新协议重新计算受影响的请求、证据或其他派生摘要，并通过现有验证路径检查数据。恢复会话时，宿主必须重新注入与迁移后绑定完全一致的 provider；Code 不会从 Registry 或 `latest` 自动选择替代代际。

如果无法证明旧数据已完整迁移，创建新会话，不要在旧快照上混合新旧字段或 schema。

## 6. 更新测试和验收

调用方侧：

- [ ] 导入、类型、builder、provider、Session 和 Run 调用均已改用新名称。
- [ ] 能力目录、事件分发和日志消费者识别新 ID 与新事件名。
- [ ] 自有持久化数据完成字段和 schema 迁移，并已备份。
- [ ] 元数据、provenance、文本前缀及错误字符串断言已更新。
- [ ] 恢复会话测试验证宿主重新注入完全相同的绑定。
- [ ] 编译、单元测试和持久化恢复测试通过。

仓库内可用于复核的命令：

```bash
cargo test -p aster-core package_context --lib
cargo test -p aster-core agent_api::tests::package --lib
cargo test -p aster-core --test session_checkpoint_v1
cargo test -p aster-core --test r0_cross_project_contract
cargo test -p aster-core --tests --no-run
cargo fmt --all -- --check
```

## 7. 迁移 ACP 扩展命名空间

ACP 的扩展 method、通知、工具元数据和会话元数据统一使用 `aster/` 命名空间。按前缀迁移即可覆盖该命名空间下的全部现有和后续方法，不要只迁移文档中列出的几个示例。

| 旧形式 | 新形式 | 适用范围 |
| --- | --- | --- |
| `x.ai/` | `aster/` | 普通扩展 method、通知和 `_meta` 键 |
| `_x.ai/` | `_aster/` | 原始传输带一个前导下划线的扩展通知 |
| `__x.ai/` | `__aster/` | 原始传输带两个前导下划线的扩展通知 |
| `x.ai~1` | `aster~1` | JSON Pointer 中转义后的 `/` |

常见调用示例：

| 旧 method | 新 method |
| --- | --- |
| `x.ai/models/list` | `aster/models/list` |
| `x.ai/session/info` | `aster/session/info` |
| `x.ai/session/list` | `aster/session/list` |
| `x.ai/session/delete` | `aster/session/delete` |
| `x.ai/permission_posture` | `aster/permission_posture` |
| `x.ai/compact_conversation` | `aster/compact_conversation` |
| `x.ai/interject` | `aster/interject` |
| `x.ai/turn/outcome` | `aster/turn/outcome` |

元数据采用同一规则，例如 `x.ai/tool` → `aster/tool`、`x.ai/userMessageEcho` → `aster/userMessageEcho`、`x.ai/leaderClientId` → `aster/leaderClientId`、`x.ai/titleIsManual` → `aster/titleIsManual`。JSON Pointer 示例：`/params/_meta/x.ai~1leaderClientId` → `/params/_meta/aster~1leaderClientId`。

迁移时保留原有下划线数量。下划线属于传输包装，不属于业务 method；不要同时保留旧前缀作为隐式别名，也不要把一个扩展通知重复加前缀。

## 8. 迁移文档和示例地址

- `/docs web` 使用项目自有文档站 `https://aster-ai-lab.github.io/aster/docs/`。
- 配置示例使用保留域名 `example.com` 或 `docs.example.com`，不使用真实品牌域名作为占位符。
- ACP 覆盖索引包含 `aster/models/list`，生成页面由 MDX 源文件重新生成，不直接手改 HTML。
- 真实第三方 API、认证地址和禁止下载的安全用例属于外部连接或安全语义，不属于 ACP 命名空间。不要为了文字替换把这些生产地址改成失效的占位域名；如果产品决定移除对应 provider，应作为独立变更处理。

调用方侧追加验收：

- [ ] ACP method、通知、`_meta` 键和 JSON Pointer 已全部使用 `aster/` 前缀。
- [ ] 原始传输的下划线数量保持不变，没有重复前缀或旧前缀隐式回退。
- [ ] 文档链接使用项目文档站或保留示例域名，生成页面与源文件同步。
- [ ] ACP、Pager、Shell、Aster 主程序和网站相关测试通过。

仓库内追加复核命令：

```bash
cargo test -p aster-acp --lib
cargo test -p aster-acp --test protocol_commands_stdio
cargo test -p aster-pager slash::commands::docs --lib
cargo test -p aster-shell inject_capabilities --lib
cargo test -p aster-shell relay_ --lib
cargo test -p aster-shell session_rename --lib
cargo test -p aster --bin aster
cd apps/web && npm run build && npm test
```
