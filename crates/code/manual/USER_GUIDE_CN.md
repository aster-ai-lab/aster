# A3S Code 用户与开发者指南

本仓库交付本地 Rust SDK `a3s-code-core` 和宿主使用的 `a3s-code-acp` 伴随进程。
这些包保留在根 Cargo 工作区中，使用仓库锁定的工具链，不发布额外语言 SDK。

## 1. 接入本地 Rust SDK

位于 `apps/<host>` 的 Rust 宿主使用本地路径依赖：

```toml
[dependencies]
a3s-code-core = { path = "../../crates/code/core" }
```

[Core 的公开实现与示例](../core/src/lib.rs)是 API 依据。外部宿主沿用 ACP 进程边界。
终端 `aster` 与 ACP 必须一起构建、分发；Studio 构建还包含 Office。

## 2. 用 ACL 配置模型

```acl
default_model = "openai/my-model"
providers "openai" {
  api_key = env("OPENAI_API_KEY")
  base_url = "https://api.example/v1"
  models "my-model" {
    tool_call = true
    limit = { context = 32768, output = 4096 }
  }
}
```

端点由宿主实际支持的服务决定。凭据通过环境变量进入 ACL。
聊天模型不会隐式成为 embedding 后端。只有可信宿主或用户配置可授权源码外发和
可选后端；自动发现的工作区配置只能缩小权限。

## 3. 创建、使用与关闭会话

```rust,no_run
use a3s_code_core::{Agent, SessionOptions};

async fn run() -> anyhow::Result<()> {
    let agent = Agent::new("agent.acl").await?;
    let session = agent
        .session_builder("/repo")
        .options(SessionOptions::new())
        .build()
        .await?;
    let result = session.send("Find the authentication entry points.", None).await?;
    println!("{}", result.text);
    session.close().await;
    Ok(())
}
```

`send`、`stream` 共用会话的单次执行边界；重叠调用立即返回忙状态。
发起下一轮之前应完整消费或取消流。`steer` 和 `interrupt` 针对当前 Run，不扩大权限。
恢复会话读取已记录的状态，实时模型、MCP 等资源仍由宿主重新注入。

## 4. 查看实际可用的工具

`sdk_capabilities()` 返回完整合同目录，包含未启用的可选能力，不是运行时可用性检测。
应查看会话的 `tool_definitions()` 和 `presented_tool_definitions()`。

| 工具面 | 用途 | 边界 |
| --- | --- | --- |
| 工作区 | 读写、编辑、补丁、搜索、目录、Git、Shell、下载 | 后端能力、工作区作用域和权限 |
| 组合执行 | batch、task、program | 原有受控执行链、子任务上限和取消 |
| 联网 | web_fetch、web_search | 网络授权、结果大小、重定向和超时 |
| 扩展 | MCP、Skill、宿主工具 | 宿主明确选择与作用域生命周期 |

模型选择的调用经过权限、确认、Hook、预算、沙箱和输出上限。
宿主直接调用是独立的受信控制入口；需要同等权限与确认时使用 governed 接口。
模型可见工具列表不授予执行权限。

## 5. 检索与记忆

`search` 支持路径与内容检索、BM25、语义检索和混合融合。语义检索需要显式选择后端。
运行时约束文本准入、切块、批次和结果大小，并在返回命中前复核当前源码摘要。
关闭会话会取消、等待并释放它拥有的检索资源。

Rust 宿主通过 `EmbeddingProvider`、`WorkspaceRetrievalOptions` 和
`SessionOptions::with_workspace_retrieval` 注入后端。标准 ACP 的可信 `local_cpu` 选择
使用内置轻量后端；内部 Rust 组合接口若已安装工厂则优先使用，失败时明确拒绝。
已交付的 stdio 代理没有通过环境变量加载工厂的入口。
远程路线仍需明确授权源码外发。详见[检索运维说明](WORKSPACE_RETRIEVAL_OPERATIONS.md)。

记忆保留已授权的事实，工作区检索索引当前源码。两者的来源、留存与授权边界不同。
参见[持久记忆](DURABLE_MEMORY.md)和[持久工程任务](DURABLE_ENGINEERING.md)。

## 6. 构建与宿主能力

Core 默认使用 `local-code`。高级工作流、S3、遥测通过显式 feature 选择，参见
[构建配置](RELEASE_PROFILES.md)。

独立 ACP 默认使用 API/HTTP 搜索。需要浏览器搜索时运行：

```sh
cargo build --locked -p aster -p a3s-code-acp --features a3s-code-acp/headless-search
```

桌面开发、构建与打包配方显式选择浏览器能力；只有实际进入浏览器搜索阶段才准备运行时。

Office 工具由 ACP 宿主在 `session/new` 中明确请求。以下是请求元数据，不是产品配置文件：

```json
{"_meta":{"a3sHost":{"officeTools":true}}}
```

缺省或 false 不注册原生 Office 工具，类型错误明确拒绝。Studio 明确声明办公支持。
源快照审阅模式仍不暴露工具；注册 Office 不授予工作区读写权限。
普通会话保留 `output_check` 的有界文本与数值检查。
Office 源读取覆盖 DOCX/XLSX/PPTX；当前不可用的 PDF/OCR 操作会明确拒绝。

## 7. 验证与证据

工具返回或模型自述都不能独自证明完成。需要保存实际执行回执、当前源码身份、必要检查
以及绑定修改摘要的验证证据。权限拒绝和模型失败分别记录。

从根工作区运行相关包的聚焦测试。测试结果只适用于记录的源码与构建产物，不能沿用为
后续工作区改动的验收结论。

## 8. 参考

- [Rust SDK API 设计](SDK_API_DESIGN.md)
- [能力合同及启用边界](SDK_CAPABILITY_MATRIX.md)
- [高级宿主集成](ADVANCED_DEVELOPER_MANUAL_CN.md)
- [作用域能力架构](SCOPED_CAPABILITY_ARCHITECTURE.md)
- [检索运维说明](WORKSPACE_RETRIEVAL_OPERATIONS.md)
- [能力验证](CAPABILITY_VERIFICATION.md)
