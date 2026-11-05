# Feature Specification: 记忆融合保留原始相关度

**Created**: 2026-11-01
**Status**: Implemented as opt-in; default rollout and complete replacement remain open
**Scope**: Product/Core；承接 033 D05。完整 Xerj 替代目标保持开放。

## Problem and acceptance

实际 bundled provider 对固定 query `parse the config file` 给 settings 0.4223333、weather 0.0249560。现有 RRF 丢失该差距，weather 的低相关 lexical 命中反而使其成为 Top-1。不能改查询、模型、候选上限、门限或既有金标来隐藏失败。

1. Host 可显式选择新的 `linear-unit.v1` profile；两路已准入的 unit score 等权相加，再沿原 max 归一化、revision/channel/tie 政策输出。复用唯一 Core fusion owner，不新增 ranker/index/repository/runtime。
2. 已发布 `rrf-k60.v1` 默认、序列化与分数保持。旧 session/binding 可以恢复；不同策略有不同 binding/serving-generation identity，profile/policy 不匹配拒绝恢复。
3. 新策略在 MemoryRepository 返回边界拒绝非有限或不在 [0,1] 的 lexical score；semantic 复用现有 query_verified 准入。空通道、零分、重复 ID/channel、Candidate 隔离与较晚 revision 有实际测试。
4. 在实现前固定 primary candidate 为等权 linear，不增加可调权重。使用原 development probe 和原 frozen fixture 的全部 10 memory docs/queries；不改输入、gold、模型或现有 metrics。保留 RRF 对照与所有失败。只有这些 owner case 的实际执行被计入，不据此宣称全 60 查询或 ACP 产品记忆路由已完成。

## Affected surfaces

- Core 私有 fusion kernel、typed memory facade/context、semantic policy/binding 与显式 exports。
- Sibling tests、既有 profile compatibility eval、新的 actual bundled-provider memory owner qualification。
- 无产品配置／wire／primary data 迁移；无新依赖。

## Success criteria

- 开发反例在新 profile 下 settings Top-1，旧 RRF 数值与既有失败保留。
- 固定 10 memory queries 的 actual rankings/source digests/profile 与原 metrics 有可复核产物；达不到原 gates 时记录 FAIL，不继续调参。
- owning Clippy、scoped fmt、Core minimal、paired CLI/ACP build 与 focused tests 有实际结果；完整替代仍须后续机制与性能资格。
