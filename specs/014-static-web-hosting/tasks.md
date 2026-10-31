# Tasks: 完成静态 Web 托管服务

**Input**: [spec.md](spec.md), [plan.md](plan.md)
**House rule**: 行为及对应 sibling hermetic tests 同任务；仓库/authority proof 同场景
memory/live PG；未实际验证不勾选。根/Cloud workspace 分别验证。
用户已要求停止开发并提交当前阶段成果；不推送。

## Phase 1: Behavior and tests

- [x] T001 [US2] 在 Gateway `config/acl.rs`、`static_object/manifest.rs` 等现有
  边界消费实际 Cloud WEB0.1 静态目标（含 sha256:、delivery）；增加 sibling
  `config/static_bundle_contract_tests.rs` 和负控测试，固定 fixture 来源并先 red 后 green。
  覆盖规范/旧 digest、未知算法、字段/策略、凭据与 endpoint 拒绝。FR-001/005。
  2026-10-24：真实 Cloud fixture red→green；10 项新契约场景纳入 57 项静态回归、
  340 项 config 回归全部通过。只是契约消费切片，不提升 WEB0 产品门。
- [ ] T002 [US2] 在 Gateway `static_object/`、`gateway/builders.rs`、managed
  bootstrap/snapshot 所有者中绑定同部署对象权威的只读 adapter；缺 authority
  继续拒绝，已准入情况下托管激活。sibling tests 证明 bounded read、租户/发布
  隔离、损坏/短读/超时、无快照 endpoint/密钥，实 listener 证明 GET/HEAD/304/Range、
  SPA、middleware、cache 和更新/drain。FR-004/005/008。
- [ ] T003 [US1/US4] 扩展 Cloud 现有 `WebBundleObjectStore`/publication 接入
  真实 materialized manifest entry；完善幂等、冲突、quota、损坏、retention/reclaim
  场景。涉及 repository/authority 的同场景 memory/live PG；对象端同一 shared client
  local 与真实 HTTPS S3。沿既有 retention，不另建 worker。FR-003/008。
- [ ] T004 [US1/US4] 替换 Cloud `build_flow/build_plan.rs` Web OCI stub 及
  Unavailable materialization；通过既有 Box/Execution/Artifact 端口运行固定 React/Vue
  构建、实际字节/manifest/出处发布。同任务 hermetic tests 验证取消/重放、输出
  限额、路径/符号链接/secret 拒绝、缺物化结果明确失败；实 Box 测试另留证。FR-002/003/008。

## Phase 2: Wiring and management

- [ ] T005 [US3] 在 Cloud Assets/Applications/DeveloperWorkflows/Edge 的既有
  authority 完成发布/绑定/撤销/更新/精确回滚、Preview 到期及安全策略；必要连续
  migration 同步计数/版本。补齐带 guard/scopes 的 REST/OpenAPI、Rust SDK/MCP
  并以同场景 memory/live PG + 跨接口证据验证授权先于 replay、租户、CAS、失效、
  snapshot 和 owner cleanup。不得恢复退役 TS CLI。FR-006/008。

## Phase 3: Full product evidence

- [ ] T006 [US1–US4] 在 `apps/cloud/tools/` 的既有 conformance 体系补充并实际
  执行 WEB0.6：精确 Cloud/Runtime/Box/Gateway/HTTPS S3，React/Vue，两次发布、
  旧 revision 回滚、构建取消、对象损坏/不可用、进程/节点失效、租户/权限负控、
  有界负载、浏览器安全/可访问性 smoke、撤销/drain/零残留；gate runner 自身有
  hermetic tests，所有原始结果保留到 evidence。FR-005/007/008，SC-002/003。
- [ ] T007 [US1–US4] 运行 Final Verification Gate，写
  `docs/evidence/web/2026-10-24-static-web-hosting/OUTCOME.md`，记录命令、版本、
  计数、负结果及 Explicitly not claimed；根据实际证据更新 WEB0 门状态，不能把
  fixture 或跳过标为 Verified。SC-004。

## Final Verification Gate

- [ ] Gateway 与 Cloud 所属 package focused tests 通过（T001–T005）
- [ ] 所有新 repository/authority 场景同 memory + live PostgreSQL，单线程（T003–T005）
- [ ] 六个 WEB0 门逐项有相应证据，完整真实链与恢复/回滚/清理通过（T006）
- [ ] 根 `cargo fmt --all -- --check`；Cloud workspace 对应 fmt；其他会话失败单列（T007）
- [ ] 无新增 crates.io Aster 依赖、无新 workspace/锁文件；适用依赖/漂移守卫通过（T007）
- [ ] 无新增 native select；产品配置均 ACL；guard/scopes/owner 边界守卫通过（T007）
- [ ] evidence 中结论与实际结果一致；没有推送、生产部署或修改其他会话文件（T007）
