# Feature Specification: 完成静态 Web 托管服务

**Feature Branch**: 当前工作树（未要求创建分支或提交）
**Created**: 2026-10-24
**Status**: Implementing
**Input**: 用户目标：「完成静态Web托管服务」
**Gate context**: `apps/cloud/ROADMAP.md` 的 WEB0.1–WEB0.6；架构为
`apps/cloud/docs/static-web-hosting-architecture.md`。保持整个目标，不能以
Gateway 单独可读本地文件、组件测试或一个演示站点替代完成。

## User Scenarios & Testing *(mandatory)*

### User Story 1 - 发布不可变静态站点 (Priority: P0)

租户接受固定源码/锁文件/工具链/资源边界的构建计划，在 Box 有限任务中构建
React/Vue 等静态产物，经 Artifacts 校验后进入唯一对象权威，再经 Edge/Gateway
公开访问；没有每站点常驻 Service。

**Why this priority**: 当前存在组件，却不能完成可信源码到公开字节的交付。
**Independent Test**: Cloud 构建/产物组件的 hermetic 场景；涉及仓库的同一场景在
memory 与 live PostgreSQL 运行；精确 Cloud/Runtime/Box/S3/Gateway 实进程链另留证据。

**Acceptance Scenarios**:
1. 固定 React/Vue fixture 经实际 Box build 发布，GET/HEAD 返回 manifest 对应字节。
2. 缺锁文件、摘要漂移、路径/符号链接逃逸、超限或 secret-bearing 输出拒绝发布。
3. 相同请求重放采用同一不可变结果；相同发布身份不同字节冲突，旧站点仍可访问。

### User Story 2 - 安全、隔离的 Web 请求 (Priority: P0)

Gateway 使用同一部署对象权威的只读端口，仅为已准入快照中的对象提供 GET/HEAD。

**Why this priority**: Cloud 当前投影与 Gateway 的 digest/交付契约不一致，托管模式
尚未绑定对象权威；放松校验不能解决缺少所有者的问题。
**Independent Test**: Gateway 实 listener + 固定 Cloud ACL 契约；digest/size、路径、
tenant、授权、超时/有界读取、HEAD、cache、条件请求和 Range 的反例。

**Acceptance Scenarios**:
1. Cloud 发出的规范 `sha256:` 引用和封闭 delivery policy 能被 Gateway 准确消费。
2. 无对象权威、错误租户/摘要/大小、未知策略或凭据/endpoint 快照均拒绝激活。
3. SPA fallback 不掩盖 `/api`、缺失静态资源、授权、完整性或方法错误。
4. middleware 在 cache/304/HEAD 之前执行；MIME/nosniff/CSP/cache 保持声明策略。

### User Story 3 - 管理、绑定、更新与回滚 (Priority: P1)

租户通过带权限的 REST/OpenAPI、Rust SDK、Management MCP 管理已有 Web release
和 Applications/Agent/Preview 绑定。更新采用完整快照；回滚复用以前的不可变发布。

**Independent Test**: 同一 memory/PG authority campaign 覆盖授权在重放前执行、
跨租户隐藏、CAS/幂等、撤销、应用/Preview 生命周期；Gateway 实 listener 覆盖在途
请求 drain、新快照失败保留旧发布，以及精确旧发布回滚。

### User Story 4 - 可证明的运行与清理 (Priority: P1)

发布者能在对象故障、构建取消、进程/节点丢失和升级后恢复；撤销流量并 drain 后，
由现有 retention/cleanup 所有者回收不再引用对象。

**Independent Test**: WEB0.6 实进程故障/恢复/负载/升级回滚/浏览器 smoke/无残留
campaign；每一项要求具名原始证据，跳过或只有 fixture 不算验收。

### Edge Cases

- Cloud `sha256:` 引用与旧 Gateway 裸 hex 的兼容必须显式处理，未知算法仍拒绝。
- 对象不可信长度、损坏、短读、超时、换 snapshot 后 cache、stale release、HEAD
  无 body、条件请求和 Range 均必须保留完整性和租户边界。
- 同路径 API/静态路由、不同租户相同文件/摘要、Preview 到期和正在 drain 的旧发布
  不得引入第二路由或清理权威。

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001 / WEB0.1**: MUST 统一 Web release/build/manifest/UI-binding/静态目标
  契约；未知字段/算法/策略和凭据注入 MUST 拒绝。T001。
- **FR-002 / WEB0.2**: MUST 在实际 Box 有限构建任务执行已接受固定计划，校验输出
  字节/出处/资源边界，支持取消、重放和清理；组件 OCI stub MUST 不冒充完成。T004。
- **FR-003 / WEB0.3**: MUST 经唯一对象客户端/权威 create-only 发布所有 manifest
  entry，并验证 quota/损坏/重放/retention；缺字节 MUST 失败，不能静默跳过。T003/T004。
- **FR-004 / WEB0.4**: MUST 绑定可信部署级只读对象权威；快照不得带 endpoint/密钥。
  无权威/越权/摘要漂移 MUST fail closed，不能取消现有托管模式围栏来假装接线。T002。
- **FR-005 / WEB0.4**: MUST 通过现有 middleware 提供安全 GET/HEAD、条件/Range、
  MIME/安全头、bounded cache、正确 SPA 与 snapshot drain。T001/T002/T006。
- **FR-006 / WEB0.5**: MUST 通过已有所有者完成 Application/Agent/Preview 绑定及
  带 guard/scopes 的 REST/OpenAPI/Rust SDK/MCP；跨租户/过期/撤销/错误 release
  MUST 拒绝。T005。
- **FR-007 / WEB0.6**: MUST 留存精确版本实际 Box/S3/Gateway/Cloud 的构建到浏览器
  全链、升级/回滚、故障/恢复、负载和零残留证据；组件通过不得提升产品 Verified。T006。
- **FR-008**: MUST 使用已有 Assets/Artifacts/Executions/Flow/Workloads/Fleet/Edge
  所有者；禁止第二控制面、队列、对象客户端权威或每站点 Nginx/Service。T002–T006。

### Key Entities

沿用 WebBuildProfile、BuildRun、WebReleaseProfile、WebBundleManifest、
ApplicationUiBinding、Edge Route/GatewaySnapshot；分别服从现有所属上下文。

## House Checklist *(mandatory — from Agents.md)*

- [ ] 每个新行为有所属 package 的 sibling hermetic tests（T001–T006）
- [ ] 新闭集枚举保持 parse/as_str；未知值拒绝（T001–T005）
- [ ] 生产 unwrap/expect 需要 construction-level infallibility 说明（T001–T006）
- [ ] 内部 Aster 依赖保持本仓库 path；三 workspace 不混并（T007）
- [ ] 产品配置只用 aster-acl；不将对象凭据写入流量快照（T001/T002/T007）
- [ ] authority/repository proof 同场景 memory/live PG（T003–T005）
- [ ] 证据 `docs/evidence/web/2026-10-24-static-web-hosting/OUTCOME.md`（T007）

## Success Criteria *(mandatory)*

- **SC-001**: Cloud 生成的真实静态目标被 Gateway 解析，验证策略和全部引用，不需
  手工改写快照；非法变体反例通过。T001。
- **SC-002**: 所有 US1–US4 场景有实际实现和对应测试/运行证据；WEB0.1–6 逐项可审计。
- **SC-003**: 指定精确版本的 React/Vue 两条发布、更新、回滚、取消/恢复、隔离与清理
  实进程链通过；不能以单机目录 serve 替代 Box/S3 产品链。T006。
- **SC-004**: focused tests、相关 architecture/PG campaigns、fmt、依赖/漂移守卫通过；
  pre-existing 或外部失败单独记录且不偷改 ratchet。T007。

## Assumptions

- 用户本次明确目标授权推进 WEB0；现有 Held 状态不成为停止实现的理由，其他方向
  的优先级与证据不擅自修改。
- 默认采用现有静态托管架构，不新增管理前端，不额外要求域名即可验证 listener。
- 当前工作树有其他会话修改，逐文件隔离；不提交、不推送、不部署到未指定生产主机。

## Explicitly Not Claimed

- SSR/BFF/动态服务或跨整个 OS 的 GA；这些仍为原 Workload Service 产品。
- 尚未实际执行的外部提供商/硬件/生产门不宣称通过；不是移出本目标。
- 不恢复已退役 TS CLI 或另建项目/仓库。Cloud 管理面采用现行 Rust SDK/MCP。
