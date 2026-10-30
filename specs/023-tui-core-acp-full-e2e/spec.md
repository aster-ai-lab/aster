# Feature Specification: Current Core and ACP in Aster TUI

**Feature Branch**: Current shared worktree; no branch requested.
**Created**: 2026-10-27
**Status**: Implementation authorized by the active user goal.
**Input**: 基于源码最新 a3s code 和 a3s code acp 集成到 aster tui，并完成全功能端到端测试。审查代码不可过拟合优化。
**Pinned live campaign (2026-10-28)**: 用户明确要求使用 `./.a3s/config.acl` 中的 Kimi K3.1 Pro，模型 ID 已从实际 ACP 初始化确认是 `kimi/k3.1-pro`，也是当前默认值。最终真实 TUI 活动固定此型号；不以其他型号或重建前二进制的结果替代。原始 ACL 保持不变，凭据仅通过现有环境交接工具传给隔离进程。
**Gate context**: Product regime. `apps/aster`, `crates/terminal`, `crates/code/core`, and `crates/code/acp`. This extends the shared-agent integration beyond the earlier connection-only acceptance. The current local Core and ACP manifests both declare 9.1.6; uncommitted source changes are part of the integration input.

**Discovered shared cause**: The credential handoff regression found quoted-key loss in the ACL parser/generator. The fix includes `crates/acl` and its identical API/Cloud submodule copies, with their owning tests and the unchanged drift ratchet. It does not add control-plane behavior or migrations.

**User-confirmed scope (2026-10-28)**: “默认及已配置能力全部测通，未启用能力验证明确拒绝”. The source inventory's 18 baseline groups and every already configured activation require positive behavior evidence. The 12 advanced groups remain inventoried; an unconfigured activation requires an actual explicit refusal, without installing or enabling that host merely to increase coverage. A refusal proves the disabled boundary, not successful execution of the capability.

**User-confirmed cancellation recovery (2026-10-28)**: “保留现有边界，复核后新建会话继续”. An interrupted mutating operation retains its uncertainty and the original session's refusal. The TUI offers review of the operation/workspace followed by an explicit `/new`; the new session continues with the configured model while original receipts remain unchanged and the cancelled command stays stopped. Ordinary resume still preserves its original session identity. No new Core/OCI reconciliation protocol is part of this choice.

## User Scenarios & Testing

### User Story 1 — Run the current source pair (Priority: P1)

The terminal starts the current ACP companion and executes the current Core, without silently choosing the retired in-process agent when a required companion is absent or incompatible.

**Independent Test**: Build both binaries in one Cargo graph; inspect their actual build identities and run the shipped TUI PTY handshake. Inject missing and mismatched companions and require precise refusal before a model request.

**Acceptance Scenarios**:

1. A clean isolated profile starts interactive and headless sessions using the same source-built ACP companion.
2. Missing, invalid, or mismatched companion identity refuses; a configured explicit companion is handled by its documented override contract.
3. ACL provider/model and trust decisions reach Core; appearance preferences cannot select another model or grant authority.

### User Story 2 — Use the complete integrated feature surface (Priority: P1)

The user can exercise the latest Core/ACP functionality through the terminal. Source declarations, protocol support, client commands, configuration, and actual behavior agree.

**Independent Test**: A source-owned capability and feature matrix maps every current Core capability, ACP host method and advertised command, and shipped terminal entry to implementation and end-to-end evidence. Each integration gap remains open until implemented and tested; an unavailable method is not counted as a completed feature.

**Acceptance Scenarios**:

1. Text, streamed reasoning/output, model selection, context/usage, attachments, and structured interaction render correctly.
2. Files, search/retrieval, edit/patch, shell, Git and artifacts perform real operations in an isolated workspace. Approval, refusal, cancellation, and sandbox boundaries are observed through the terminal.
3. Session creation, listing, resume, history, compaction, forks, interruption and recovery preserve identity and do not replay effects.
4. Goals, planning, delegation, background tasks, durable memory, MCP, Skills and configured optional capabilities are connected to the current Core contract rather than incompatible legacy DTOs.
5. Both terminal render modes, keyboard workflows, resize, errors and shutdown are covered. Every optional or host-owned Core capability retains an explicit activation and verification path in the matrix. Configured activations execute; disabled activations explicitly refuse.

### User Story 3 — Trust the full-function acceptance (Priority: P1)

The integration has evidence from the actual terminal and ACP processes. A mock model can make a workflow deterministic, but product code cannot recognize a test oracle to pass it.

**Independent Test**: Scripted local models drive actual ACP/tool loops and PTY interactions; a live configured model supplies a separate real-model campaign. The runner inspects both visible terminal state and actual workspace/session effects and reaps its process trees.

**Acceptance Scenarios**:

1. Positive cases have corresponding rejected, cancelled, malformed, duplicate or resumed cases where relevant.
2. Tests vary workspace names, content and input shapes, and assert outcomes rather than re-deriving implementation values.
3. No production prompt, model-ID or fixture-name matching, test-only bypass of permissions, fabricated success, or workload-specific optimization is introduced.
4. Every final feature-matrix row has direct evidence at its claimed scope. Ignored, indirect, missing, flaky or stale evidence is not marked passed.

### Edge Cases

- Concurrent dirty source changes invalidate a previously built pair's evidence; final hashes and identities are rechecked.
- A cancelled write or shell operation must not be retried because a terminal notification was lost.
- Legacy and Core memory/goal/task contracts with similar names are not treated as interchangeable.
- A model or optional runtime failure surfaces as a typed failure and cannot enable a feature or widen permissions.
- Oversized streams and tool results stay bounded; test timeouts release all owned children.

## Requirements

### Functional Requirements

- **FR-001**: Build and deliver `aster` and `a3s-code-acp` together from current local sources and preserve paired identity enforcement.
- **FR-002**: Inventory the complete current Core capability, ACP protocol and terminal surface, including optional and host-owned contracts. Required enabled gaps stay incomplete; disabled capabilities need actual refusal evidence.
- **FR-003**: Fix integration mismatches at their owning shared boundary with hermetic tests; do not substitute legacy semantics or weaken gates.
- **FR-004**: Verify every terminal feature family through real terminal/ACP execution, including stateful outcomes, errors, cancellation and restart where relevant.
- **FR-005**: Run a separate live-model terminal campaign using authorized existing credentials, without copying user state into fixtures or recording secrets.
- **FR-006**: Preserve ACL, trust separation, sandboxing, bounded I/O, error propagation, upstream package APIs and notices, and local root-workspace dependencies.
- **FR-007**: Audit connected code for overfitting, unnecessary branches/abstractions, fake coverage and unmeasured performance changes. Every finding needs a concrete reproducible case.
- **FR-008**: Retain feature-level evidence, command results, source/binary identities and cleanup proof; completion requires a requirement-by-requirement audit of the full objective.

### Key Entities

- **Source pair**: Current Aster and ACP binaries, their shared build identity and input hashes.
- **Feature matrix**: Inventory derived from explicit source contracts, with terminal entry, protocol/host activation, cases and evidence.
- **Campaign**: Hermetic or live-model run over an isolated terminal/workspace with owned lifecycle.

## House Checklist

- [x] New behavior is paired with an owning-package hermetic test in the plan.
- [x] Protocol and capability decisions use declared closed sets, not name or intent guessing.
- [x] New production panic paths are prohibited without an infallibility argument.
- [x] All internal A3S dependencies stay local in the root workspace.
- [x] Product configuration remains ACL parsed by `a3s-acl`.
- [x] Evidence target: `docs/evidence/code/2026-10-27-tui-core-acp/OUTCOME.md`.

## Success Criteria

- **SC-001**: Both final binaries build and their source/build identities agree with retained current inputs.
- **SC-002**: The complete feature matrix has no missing, incomplete, indirect-only or unverified required row; every supported activation is exercised.
- **SC-003**: Actual PTY campaigns and actual ACP stdio cases pass, including negative and lifecycle cases, with zero surviving owned processes.
- **SC-004**: The live-model terminal campaign completes real coding operations and verifies their artifacts and session recovery.
- **SC-005**: Applicable tests, workspace guards and format gates pass. Residual failures remain open and are not hidden by exclusions or adjusted ratchets.
- **SC-006**: The final adversarial review and completion audit find no unresolved requirement or confirmed integration defect.

## Assumptions

- “Latest source” initially means the authoritative current worktree, including its 9.1.6 changes. A new upstream source instruction must be incorporated explicitly rather than overwriting this dirty tree.
- Full end-to-end coverage can combine hermetic model endpoints with an actual live-model campaign, but unit tests or connection checks alone do not satisfy it.
- The matrix determines how a capability is used through TUI, ACP, model tools or host configuration; not every SDK operation needs a new menu item.

## Explicitly Not Claimed

- No source replacement with upstream binaries, new language SDK, second repository, credential migration, desktop rewrite, or automatic release.
- This specification and earlier connection tests are not proof of completed integration or full-function acceptance.
