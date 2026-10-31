---
description: "Magpie 兼容本地 AI 网关的分阶段任务"
---

# Tasks: Magpie 兼容的本地 AI 网关

**Input**: Design documents from `specs/030-magpie-compatible-local-gateway/`

**Prerequisites**: plan.md (including Constitution Check), spec.md, research.md, contracts/gateway.md

**House rule**: every behavior task carries its hermetic test in the same task. A task without that test is not done.

## Phase A: 基线与对照框架

- [x] T001 [US1] 保留 `apps/gateway/src/gateway.rs` 的 `new_embedded` 和 `apps/studio/src-tauri/src/local_api_gateway.rs` 内嵌生命周期；原生构造与真实 401/200 测试通过。
- [x] T002 [US1] 支持 `provider/model`、`group/id` 并保留 `provider:model` 与裸路由 ID；文档和寻址测试通过。
- [ ] T003 [US1] 建立 `apps/studio/tests/e2e/local-gateway/` 的 Magpie 对照 harness：固定 0.1.2141、脱敏日志、请求/响应/SSE 比较，不把实时供应商作为默认依赖。
- [ ] T004 [US1] 将 `contracts/gateway.md` 中 P1 用例映射为独立测试清单；每项缺口必须有失败测试或已通过的现有测试链接。

## Phase B: 协议与流式语义

- [ ] T010 [US1] OpenAI Chat Completions 与 `/v1/models`：模型目录、JSON、错误、usage 和模型寻址对照。
- [ ] T011 [US1] OpenAI Responses：非流式、SSE、工具调用、推理与终止事件对照。
- [ ] T012 [US1] Anthropic Messages 与 count_tokens：请求/响应、签名、工具与错误对照。
- [ ] T013 [US1] Gemini `/v1beta` 模型、generateContent、streamGenerateContent 和 countTokens：实现并验证原生 Rust 路径，不恢复 ElysiaJS。
- [ ] T014 [US1] 跨协议转换：验证流式工具参数、推理签名、取消、超时与首响应前后失败；承诺后重放为 0。
- [ ] T015 [US1] 对所有 P1 协议做 Tauri/桌面真实生命周期验证：启动、重启、停止、开机恢复、端口冲突与错误提示。

## Phase C: 客户端密钥与配额

- [ ] T020 [US3] 定义 `GatewayClientKey` 的 ACL 表示、封闭状态、轮换与撤销；现有 aster 密钥迁移且不写回旧格式。
- [ ] T021 [US3] 实现模型 allowlist 和 401/403 语义；未授权上游调用为 0。
- [ ] T022 [US3] 实现日/周/月 token 与成本配额、重置信息和 429；使用封闭周期类型。
- [ ] T023 [US3] 配额预留/提交/释放与并发测试；失败尝试不重复扣减，超限请求不触达上游。
- [ ] T024 [US3] 在现有设置中实现命名密钥管理，不引入原生 select；Tauri 命令只开放给所需窗口。

## Phase D: 多账户与路由组

- [ ] T030 [US2] 扩展 `providers*.rs` 为显式多账户来源，凭据不入 Debug/日志/用量；旧配置无损迁移。
- [ ] T031 [US2] 实现 fixed/order/rotate/usage/pace 路由与封闭策略类型；每个策略有独立选择测试。
- [ ] T032 [US2] 实现 weight/free/fast 路由及价格/延迟声明来源；无声明时拒绝，不猜价格或速度。
- [ ] T033 [US2] 嵌套路由组与循环/空组拒绝；只展开显式候选。
- [ ] T034 [US2] 会话粘性与 prompt-cache 友好选择；跨组不串账户。
- [ ] T035 [US2] 区分 401/403 与可重试错误；只在前响应阶段切换，承诺后重放为 0。
- [ ] T036 [US2] 实际账户/密钥归因到 attempt 与最终 request，覆盖首选失败、备用成功。

## Phase E: 目录、预算与用量

- [ ] T040 [US4] 动态模型目录刷新、禁用/启用迁移和按 Agent 可见性；供应商之间互不影响。
- [ ] T041 [US4] 将本地预算映射为明确的客户端/账户/全局配额语义；不支持的通知语义不得伪称停止。
- [ ] T042 [US4] 用量 attempt/request 持久化：token、成本、缓存、重试、输出速度、状态和时间。
- [ ] T043 [US4] 用量按客户端、模型、账户、上游密钥聚合与导出；原始凭据出现次数为 0。
- [ ] T044 [US4] 会话与配额窗口、历史身份和重启恢复；删除/轮换密钥不改历史归因。

## Phase F: 插件、隐私与 Agent 配置

- [ ] T050 [US5] 独立隔离插件宿主与能力契约，覆盖 `onRequest`、`onEvent`、`onResponse` 和超时/异常；不得恢复 ElysiaJS。
- [ ] T051 [US5] 内存脱敏与流式恢复，包括工具参数和签名稳定；失败关闭且明文不落盘。
- [ ] T052 [US5] Agent Profile 与 Library 复用 `src/renderer/pages/code/cliConfig/`：原子写、格式保留、回滚与新旧模型兼容。
- [ ] T053 [US5] 插件/隐私/Agent 配置的真实 UI 与文件系统端到端测试，覆盖异常中断恢复。

## Phase G: 共享、同步与发布门

- [ ] T060 [US3][US5] 局域网共享与远程客户端：独立密钥、授权、协议不重复转换和断线错误。
- [ ] T061 [US5] 备份与同步恢复：WebDAV/S3/GitHub 的数据边界、凭据保护和冲突处理；不恢复旧远程配对实现。
- [ ] T070 [US1][US2][US3][US4][US5] 运行完整离线契约矩阵与现有 Aster 回归；ElysiaJS 依赖扫描为空。
- [ ] T071 [US1][US2][US3][US4][US5] 在声明支持的 macOS、Windows、Linux 做原生端到端；记录平台、工具链、结果和未测项。
- [ ] T072 [US1][US2][US3][US4][US5] 记录性能与资源基线，不把 Magpie 的数字当作 Aster 的结果。
- [ ] T073 [US1][US2][US3][US4][US5] 写 `docs/evidence/local-gateway/<date-slug>/OUTCOME.md`：命令、计数、日志、契约覆盖与 Explicitly not claimed。
- [ ] T074 Stage files individually and verify `git diff --cached --name-only` before any commit the user asked for.

## Final Verification Gate

- [ ] 所有 P1 契约用例通过；P2 未完成项明确列出，不宣称完美替代。
- [ ] `cargo fmt --all -- --check` clean for Rust edits；owning package focused tests 与 Clippy 通过。
- [ ] 前端定向测试、Tauri 权限测试、锁文件一致性通过。
- [ ] No new crates.io `aster-*` dependency and no second Cargo workspace/lockfile.
- [ ] No native `<select>` added on a product surface.
- [ ] Product configuration is ACL；被管理 Agent 文件没有加入产品配置发现链。
- [ ] 未授权/超限上游调用为 0；响应承诺后重放为 0；原始凭据泄露为 0。
