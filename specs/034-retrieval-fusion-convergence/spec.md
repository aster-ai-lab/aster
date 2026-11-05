# Feature Specification: Core 检索融合收敛

**Feature Branch**: 当前 checkout，无新 branch／worktree。
**Created**: 2026-11-01
**Status**: Implementing
**Input**: Core 机制完整替代 Xerj、无重复实现、有效且高效。
**Gate context**: Product/Core；承接 032 FR-015 与 033 D04/D05。此批次合并三处 Core RRF 循环，不重建数据库／查询服务。整体目标不因此缩小；记忆质量失败和共享库其它候选继续要求解决。

## User Scenarios & Testing

### User Story 1 — 三个 Core 入口使用同一融合机制（Priority: P1）

工作区、typed durable memory 与旧 MemoryStore 使用同一按来源排名归并／去重实现，各自保留来源、准入与输出政策。

**Why this priority**: 三处循环独立维护相同的 RRF 公式与重复记录归并，旧实现还遗漏已声明的同分排序。
**Independent Test**: `aster-core` sibling tests、原 retrieval／durable-memory tests 与实际 ACP qualification target。

**Acceptance Scenarios**:

1. **Given** 同一 ID 来自多个 channel，**When** 融合，**Then** 每个 channel 最多贡献一次；rank=0 不准入，不产生结果或分数。
2. **Given** 工作区候选，**When** 复用共享机制，**Then** exact tier、channel rank 顺序、path/span/ID 同分顺序和来源核验不变。
3. **Given** typed memory 的较晚 semantic revision，**When** 与 lexical 归并，**Then** 保留该已复核 revision、原 f32 归一化、channel/updated_at/ID 同分政策与单通道 passthrough。
4. **Given** 旧 MemoryStore 融合同分，**When** 截断，**Then** 执行原文档规定的第一通道 rank 优先，再以第二通道 rank／ID 确定结果，不受 HashMap 顺序影响。

### User Story 2 — 收敛保留质量与性能事实（Priority: P1）

固定输入和现有 grader 验证收敛；独立保留已经发现的记忆误排，不靠改门限或变更 profile 名称转绿。

**Why this priority**: 去重必须保持已发布数值／策略，修复相关性需要独立校准，不能把重构当质量改进。
**Independent Test**: 原 release `workspace_retrieval_benchmark` 前后对照、固定 workspace quality 和实际 provider 的 memory probe。

**Acceptance Scenarios**:

1. **Given** 原 25k×384 exact／512-chunk hybrid workloads，**When** 收敛前后运行，**Then** 原输入、采样、门限和输出验证不变；保留完整通过或失败记录。
2. **Given** 当前 `rrf-k60.v1` 记忆 probe，**When** 运行，**Then** 不更改 query、候选数、lexical floor、模型或 gold；既有 Top-1 FAIL 继续阻止完整目标完成。

### Edge Cases

- 空输入、单通道、重复 channel、零 rank、不同来源的同一 ID、超大 rank 的加法。
- 同分截断和输入顺序；typed memory 的旧 lexical node 与新 semantic node。
- Core minimal 不启用 vec；不能为去重强制引入 vec 或另一个依赖／runtime。

## Requirements

- **FR-001**: MUST 删除三处 Core 各自的累计／重复归并循环，统一到一个 crate-private owner；domain facade 不复制算法。
- **FR-002**: MUST 保留公开 API、配置、source／activation／retention、channel metadata、现有 f32/f64 精度和 profile 身份；仅修复旧路径已记录却未执行的同分顺序。
- **FR-003**: MUST 一次准入 rank 为非零 typed 值，内部复用；每 ID/channel 一次贡献。预算继续由现有入口限制，不另建 registry 或权威库。
- **FR-004**: MUST 在 ownership-layout 改动前记录原 release workload；前后现有质量／资源／latency gates 不放宽，保留失败。
- **FR-005**: MUST 保留 D05 和其它库的待收敛候选，不能把 Core 三个入口收敛宣布为完整替代或全库去重。

### Key Entities

- **RankFusion / fused entry**：Core 内部按 ID/channel 合并排名与原有 payload；没有 I/O、模型调用、权限或后台 owner。
- **Domain facade**：原 workspace/memory 入口负责 provenance、revision、归一化与特有同分政策。

## House Checklist

- [ ] Owning sibling tests 与行为同批；T002–T005。
- [ ] 无新增产品 enum／配置，公开 API 保持；T003/T005。
- [ ] 新生产代码无无依据 panic，输入 rank 仅准入一次；T003/T005。
- [ ] 无新 dependency／crate／workspace／lockfile／runtime；T003/T005。
- [ ] Evidence：`docs/evidence/code/2026-11-01-retrieval-fusion-convergence/OUTCOME.md`；T006。

## Success Criteria

- **SC-001**: 三处 Core caller 委托唯一融合 owner，重复累计代码消失；实际调用图与 scoped diff 复开核对。
- **SC-002**: 原 Core/ACP tests 和新增去重／同分反例通过；D05 probe 的独立已知失败原样记录。
- **SC-003**: 原 release workload 参数一致、通过既有 gates；噪声／平台限制独立报告。

## Assumptions

- 当前 Core typed-memory query cap 为 100；不提高限额，不改变精度或配置。
- 同分以外的分数／排序应保持；语义质量策略的提升继续沿冻结评估处理。

## Explicitly Not Claimed

- 记忆 RRF 质量失败已修复、全部共享库融合已收敛、032/033 或总目标已完成。
- 新融合策略、模型、ANN、缓存、持久化 schema、API、宿主路由或发布。
