# Implementation Plan: 完成静态 Web 托管服务

**Branch**: 当前工作树 | **Spec/Input**: [spec.md](spec.md)
**Gate context**: WEB0.1–WEB0.6，保持完整出口。

## Summary

先用真实 Cloud ACL 固定跨进程契约，再绑定受信部署级只读对象端口；补齐真实
Box 构建输出到现有 Artifacts publication，再由现有 Assets/Applications/Edge
管理绑定、快照更新与回滚。最终以精确版本的实际 Box/S3/Cloud/Gateway 证据验收。

## Constitution Check

| Principle | Status | Evidence in the plan |
| --- | --- | --- |
| I. Fail closed | PASS | T001 未知策略/算法拒绝；T002 无 authority 不激活；T004 缺字节不跳过 |
| II. Evidence retained | PASS | T007 保留 OUTCOME 和每个 gate 的原始输出 |
| III. Tests follow regime | PASS | Gateway hermetic/listener；Cloud repository 同 memory/live PG |
| IV. Closed sets | PASS | ACL delivery/digest/绑定闭集，不做意义猜测 |
| V. Workspaces/published APIs | PASS | Gateway 在根 workspace；Cloud 保留自己的 workspace，跨 context 用 published |
| VI. Local schema | PASS | 必要 schema 只在 Cloud 连续迁移，同步两个版本常量并双适配器证明 |
| VII. Shrink-only ratchets | PASS | 不增长 allowlist；缺口通过真实 owner wiring 消除 |
| VIII. Replaceable transports | PASS | Gateway 只读 port；部署 bootstrap 与快照产品意图分离 |
| IX. One configuration language | PASS | 只用 aster-acl；无产品配置兼容解析器 |
| X. Tests/controllers | PASS | 新测试 sibling；HTTP guard/scopes/#[controller] |
| XI/XII. Understanding/adversarial review | PASS | 对契约、字节来源、隔离与恢复尝试破坏，不以组件证据完成任务 |
| XIII. One repository | PASS | 仅本树与现有 Cloud 子模块，不创建仓库/外部 checkout |
| XIV/XV. Names/meaning | PASS | 只用明确 asset/release/tenant 身份和封闭策略，不推断用户含义 |
| XVI/XVII. Pickers/dialogs | N-A | 不新增产品 UI |

**Procedure reconciliation**: `.claude/commands/speckit.plan.md` §4 仍写“不要规划
apps/cloud”，并引用已退役 apps/work/apps/desktop。按该 skill 指向的
`docs/spec-driven-development.md` Codex surface 规则，以根 AGENTS/constitution
当前 control-plane regime 和用户明确 WEB0 目标为准；本计划修改现有 Cloud 所有者，
不添加新的 bounded context。这不是 NON-NEGOTIABLE 豁免。

## Project Structure

| Layer | Existing owner / paths | Planned extension |
| --- | --- | --- |
| Gateway contract | `apps/gateway/src/config/acl.rs`, `config/static_bundle.rs`, `static_object/` | Cloud digest/delivery exact admission，独立契约测试 |
| Gateway object read/runtime | `static_object/port.rs`, `gateway/builders.rs`, `managed_snapshot/` | 部署级 authority composition；只读、bounded、tenant/release 固定；拒绝快照端点 |
| Object publication | Cloud `modules/artifacts/{application,infrastructure}/web_*` | 用现有 ImmutableObjectClient 完成物化、发布、quota/retention |
| Build | Cloud `artifacts/infrastructure/build_flow`, `executions/application/web_build_task.rs`, Node Agent/Box existing build ports | 真正运行固定 build command 并输出字节，不采用 OCI stub |
| Binding/management | Cloud `assets`, `applications`, `developer_workflows`, `edge`, `crates/sdk`, contracts | 所属 authority、REST/OpenAPI/Rust SDK/MCP 和既有清理生命周期 |
| Evidence | `docs/evidence/web/2026-10-24-static-web-hosting/` | 逐 gate 原始输出、版本、负控和未证范围 |

## Research / prior art

- `apps/cloud/crates/contracts/src/web_static_bundle_target_acl.rs` 已输出规范
  `sha256:` 及固定 delivery block；Gateway 目前 digest 验证仅接受 64 字符 hex。
- `apps/gateway/src/config/mode.rs` 明确拒绝 Cloud-managed static target；不得
  仅删除这一拒绝。先建立受信、只读对象 authority，再允许对应配置。
- Gateway 已有 `static_object/port.rs`、目录 adapter、cache/Range/HEAD/listener，
  继续复用，不创建第二请求服务。
- Cloud `infrastructure/immutable_object/` 为唯一对象客户端；
  `WebBundleObjectStore` 使用 `web-bundles/<object_namespace>/objects/<path>`，
  读端必须精确匹配现有键并绑定 manifest，而非凭 digest 扫描或猜键。
- `HostedWebBundleBuildFlowPublication` 当前注入 Unavailable materialization，
  `build_flow/build_plan.rs` 的 web_hosted_component_plan 仍是 OCI stub。
- `WebBuildProfile`、`WebReleaseProfile`、UI binding 和 Gateway ACL 均已有规范，
  新管理操作沿现有 owner，而非新增 StaticSites 服务。
- T002 进一步检查 cache：`static_object/serve.rs` 当前给所有非 entry/fallback
  路径一年 immutable，不能证明每个路径已按内容命名。必须在产品出口前收紧为
  已证明不变的路径/发布命名空间，避免根路径站点更新后浏览器沿用旧资源；不能仅
  因 delivery 字段已被解析就宣称整个缓存契约完成。

## Verification Plan

1. T001 先固定实际 Cloud fixture，证明旧实现失败，再通过 digest/delivery 消费和
   非法字段/策略测试；根 `cargo test -p aster-gateway --lib <focused filter>`。
2. T002 运行只读 authority / 真实 listener 的隔离、损坏、超限、故障、cache/drain。
3. T003–T005 运行所属 package hermetic + 同一 memory/PG scenario；PG 采用显式
   `ASTER_CLOUD_TEST_POSTGRES_URL`、单线程。不从环境输出凭据。
4. T006 实际 Box/S3/Cloud/Gateway React/Vue 发布与恢复/升级/清理；保留精确命令、
   revision、状态、计数和负控，完整通关前保持目标 active。
5. T007 focused/architecture tests、各 workspace fmt、依赖/漂移等适用守卫。
   不将其他会话修改一起格式化/提交；不以未运行的检查冒充通过。

## Clarify / analyze

2026-10-24 clarify：规格无待用户决策 marker，沿既有架构推进。
2026-10-24 read-only analyze：FR-001..008、SC-001..004 均映射至 tasks；当前无
宪法或任务覆盖 blocker。外部资源可用性不作为推迟 hermetic 实现的理由。
