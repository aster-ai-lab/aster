# Implementation Plan: Magpie 兼容的本地 AI 网关

**Branch**: 当前工作分支 | **Spec**: [spec.md](spec.md)

**Gate context**: `apps/studio` 与 `apps/gateway`；根 workspace 的 product/hermetic 规则；分阶段交付，不把一次性大改称为完成。

## Summary

保留已经落地的内嵌 `aster-gateway` 生命周期、原生权限、供应商凭据传递和双向模型寻址，先建立 Magpie 对照测试，再按协议、客户端密钥、账户路由、目录用量、扩展与共享逐层补齐。每个阶段必须先得到可独立验收的业务闭环，未通过契约矩阵的阶段不得宣称可替代。

## Constitution Check

*GATE: Re-read `.specify/memory/constitution.md`. A NON-NEGOTIABLE violation stops the plan.*

| Principle | Status | Evidence in the plan |
| --- | --- | --- |
| I. Fail closed | PASS | 未授权/超限不触达上游；配置错误拒绝启动；响应承诺后禁止重放；脱敏失败不发送 |
| II. Evidence retained | PASS | 每阶段写入 `docs/evidence/local-gateway/<date-slug>/OUTCOME.md`，保留对照日志与未声称事项 |
| III. Hermetic tests | PASS | 离线协议、路由、配额、目录和配置测试；真实供应商为可选独立验收 |
| IV. Closed sets | PASS | 协议、策略、密钥状态、错误与配额周期使用带 `parse()`/`as_str()` 的封闭类型 |
| V. One workspace / published APIs | PASS | 仅根 workspace 的 `apps/gateway`、`apps/studio` 与既有 crate 公共 API |
| VI. Local schema changes | PASS | 新持久化只归本地配置/用量拥有者；不做 Cloud migration，不引入第二锁文件 |
| VII. Shrink-only ratchets | PASS | 不扩大 allowlist；ElysiaJS 依赖保持删除 |
| VIII. Replaceable transports | PASS | 传输实现留在 aster-gateway/宿主，客户端协议不泄漏传输选择 |
| IX. One configuration language | PASS | 新产品配置只用 ACL；Agent 原生文件仅作为被管理数据，不发现或合并为产品配置 |
| XIII. One repository | PASS | 不新建仓库；使用当前根仓库和既有 Studio 私有子模块 |
| XIV. Names are not guessed | PASS | 模型、账户、策略和错误名来自显式配置、Magpie 契约或上游响应 |
| XV. Meaning is not matched | PASS | 路由策略和协议由封闭类型决定，不用词表/正则猜测意图；意图路由只调用显式模型 |
| XVI. Pickers are drawn | PASS | 沿用 DESIGN.md 的 picker，不新增原生 select |
| XVII. Dialogs stay in place | N-A | 本计划不改变弹窗运动契约 |

## Project Structure

### Documentation (this feature)

```text
specs/030-magpie-compatible-local-gateway/
├── spec.md
├── plan.md
├── research.md
├── tasks.md
└── contracts/
    └── gateway.md
```

### Source Code

| Layer | Path | New files or change area |
| --- | --- | --- |
| Gateway data plane | `apps/gateway/src/` | 嵌入式快照、协议适配、路由与授权复用；不另造平行网关 |
| Desktop host | `apps/studio/src-tauri/src/local_api_gateway.rs` | 配置投影、生命周期与 native command 编排 |
| Desktop native modules | `apps/studio/src-tauri/src/` | 客户端密钥、账户、目录、用量、共享和插件的 owning modules |
| Provider authority | `apps/studio/src-tauri/src/providers*.rs` | 多账户与模型目录的唯一来源，避免第二套 provider store |
| Existing model control | `apps/studio/src-tauri/src/model_control*.rs` | 预算、路由、计量与现有本地策略复用 |
| Renderer | `apps/studio/src/renderer/` | 密钥、账户、路由和用量管理；沿用现有设置与 picker |
| Contracts/tests | 上述 owning sibling tests 与 `apps/studio/tests/e2e/` | 离线对照、Tauri 权限、真实 UI 生命周期 |

## Research

详见 [research.md](research.md)。实现约束：

- `apps/gateway/src/gateway.rs` 的 `new_embedded` 已支持宿主完整快照与 supplier credentials，继续作为唯一嵌入入口。
- `apps/studio/src-tauri/src/local_api_gateway.rs` 已提供启动、停止、重启、状态和配置投影，不在 renderer 重建生命周期。
- `providers.rs` 与 `model_control.rs` 已拥有模型、路由、预算和计量，新增业务必须复用其权威来源。
- `apps/gateway` 已有原生 inference 与翻译能力，优先补齐宿主投影和契约，不复制第二套转换器。
- ElysiaJS 已删除，后续插件隔离与网关数据面必须分开，不能借此恢复旧服务。

## Phases and backlog

| 阶段 | 功能与顺序 | 验收条件 | 状态 |
| --- | --- | --- | --- |
| A / P0 | 固定 Magpie 版本与契约矩阵；保留现有内嵌与双向寻址 | 所有差距有测试或明确后续任务；当前冒号兼容不回退 | 基线已完成，矩阵待补 |
| B / P1 | OpenAI Chat/Responses、Anthropic、Gemini 的模型发现和完整流式协议 | 离线双方对照；工具/推理/终止/错误一致；401 不触达上游 | 待做 |
| C / P1 | 命名客户端密钥、模型授权、周期 token/成本配额 | 轮换撤销、403/429、并发精确扣减、无超限上游调用 | 待做 |
| D / P1 | 多账户池、全部路由策略、嵌套组、粘性与可重试切换 | 策略矩阵通过；承诺后重放为 0；实际账户归因正确 | 待做 |
| E / P2 | 动态目录、用量、成本、缓存、会话与导出 | 目录只随声明刷新；最终请求不重复计数；secret 不泄露 | 待做 |
| F / P2 | 插件中间件、隐私脱敏、Agent Profile 与 Library | 插件失败受控；脱敏双向恢复且不落盘；配置格式保留 | 待做 |
| G / P2 | 局域网共享、远程客户端、备份同步 | 独立密钥授权、远端协议不重复转换、同步恢复一致 | 待做 |
| H / 发布门 | 全协议与全平台原生端到端、性能基线、证据 | 声明平台全部实测；未测项明确；无“完美替代”提前声明 | 待做 |

依赖顺序：A → B → C/D → E → F/G → H。C 与 D 可并行，E 依赖实际账户归因；F/G 依赖核心授权与用量稳定。

## Verification Plan

- **对照优先**：相同请求同时驱动 Magpie 与 Aster，保存脱敏后的请求/响应、事件流和状态，不只检查关键字。
- **协议测试**：非流式、SSE、工具调用、推理、取消、超时、首响应前后失败。
- **路由测试**：每个策略、嵌套组、配额边界、重试条件、会话粘性和实际账户。
- **授权测试**：密钥轮换撤销、模型拒绝、周期重置、并发配额和原子扣减。
- **存储测试**：配置/用量重启恢复、失败不损坏、历史凭据身份不被新值覆盖。
- **桌面测试**：Tauri 权限、启动/重启/停止/开机恢复、真实 UI 和 `/v1/models`。
- **通用检查**：`cargo fmt --all -- --check`、owning package focused tests、Clippy、前端定向测试、锁文件一致性。
- **明确不做默认门槛**：无真实付费供应商、无全仓既有失败清零、无未运行平台声明。

## Analyze verdict

计划与规格包含协议、路由、客户端密钥、目录、用量、插件、共享与证据阶段。每项 Magpie 核心差距都有 owning task 和独立验收；旧 Aster 寻址与现有原生生命周期不得回退。未完成 H 阶段前，产品只能声明“基础兼容”，不能声明“完美替代”。
