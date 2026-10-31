# Research: Magpie 兼容基线与实现依据

## Reference baseline

- 本机可执行文件：`/Users/roylin/.local/bin/magpie`，实测版本 **magpie 0.1.2141**。
- 官方仓库：`https://github.com/yetone/magpie`。
- 契约文档：仓库 `README.md` 与 `docs/reference.md`，于 2026-10-30 阅读。
- 本机 `/v1/models` 实测返回 `object=list`，模型 ID 使用 `provider/model`，并包含 `group/<id>`。
- 参考版本固定为 0.1.2141。升级时必须重新审查协议、策略和配置差异，不能静默跟随 `main`。

## Current Aster baseline (verified before planning)

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
| OpenAI/Anthropic/Gemini 与跨协议流式 | Gemini 缺失，流式/工具/推理对照不足 | 复用 aster-gateway 翻译与流式状态机，建立双方 fixture 对照 |
| 多账户订阅与故障切换 | 主要是 API key 和有限凭据 | 扩展 ProviderStore 为显式多账户权威，不读取未授权订阅 |
| 全部路由策略与嵌套组 | 固定/ordered 为主 | 用封闭策略类型投影 aster 路由，缺失语义先测试再实现 |
| 命名客户端密钥与模型/成本/token 配额 | 单一本地密钥 | 新建本地密钥授权与原子配额，保留现有密钥迁移 |
| 动态模型目录和按 Agent 可见性 | 启动时静态模型集合 | 复用 ProviderStore 刷新与本地策略，不另建目录 |
| 用量、成本、缓存、会话与配额窗口 | 本地预算未映射 | 复用 model_control 计量，区分 attempt 与最终 request |
| 插件与中间件 | 无 Magpie 兼容运行时 | 独立隔离宿主，明确能力边界，不恢复 ElysiaJS |
| 隐私脱敏 | 无双向脱敏 | 内存占位符映射，流式恢复，失败关闭且原文不落盘 |
| Agent Profile/Library | 覆盖部分 Agent | 扩展现有 CLI config adapters，原子写并保留格式 |
| 局域网共享、远程和同步 | 旧远程配对未迁移 | 使用新客户端密钥授权，旧实现不冒充兼容 |

## In-repo prior art

- `apps/gateway/src/inference/`：授权、路由、翻译、流式与 usage primitives。
- `apps/studio/src-tauri/src/model_control.rs`：本地策略、预算、调度与计量。
- `apps/studio/src-tauri/src/model_accounting/`：本地用量持久化与预算状态。
- `apps/studio/src-tauri/src/preferences.rs`：ACL preference snapshots、revision 和 commit。
- `apps/studio/src/renderer/pages/code/cliConfig/`：Agent 配置写入、模型寻址与连接匹配。
- `apps/studio/src/renderer/hooks/useApiGateway.ts`：现有 renderer 生命周期和 shared-cache running state。

## Decisions

1. **不复制 Magpie 代码或启动其进程**：业务契约对齐，数据面继续内嵌 aster-gateway。
2. **先对照再扩展**：每个缺口先变成可失败的契约测试，避免以名称相似代替语义一致。
3. **不恢复旧 Node 服务**：ElysiaJS 保持移除；插件隔离与网关服务分开评估。
4. **不静默迁移用户凭据**：旧 `cs-sk-*` 已在首次启动时替换；后续密钥体系必须保留新 aster 密钥及已写配置。
5. **不承诺未验证语义**：实时订阅、全平台、性能与“完美替代”只能在对应证据后声明。
