# Implementation Plan: 记忆融合保留原始相关度

**Checkout**: 当前 checkout，无新 branch/worktree。无委派。

## Design

现有 RankFusion 增加可选已准入 score 贡献，RRF wrapper 仍走原公式，同一 Entry/去重路径。typed memory 仅选择贡献，不复制累计算法。新 private bool policy 字段默认 false、false 不序列化，builder 显式选择 linear；binding 的 profile 与 policy 一致性由既有恢复入口校验。原 binding 的序列化不变，选择新 profile 时复用原 serving-generation digest 失效规则。

query 入口先收取 repository response；仅新 profile 验证 lexical 分数，再执行原 floor/filter。全零融合不除零；保留 score=0 和原 ties。不改变 semantic 验证、activation、retention、取消或降级机制。

固定评估只 seed frozen fixture 的原 memory owner documents，使用实际 InMemoryRepository、原 semantic projection、LightTierProvider 和固定时钟；输出实际 preview 与 binding，Python 复用原 metrics 评分并核对 ID/revision/content/digest，无测试替身排序或 query 词表。

## Constitution Check

| Principle | Status | Evidence / obligation |
| --- | --- | --- |
| I Fail closed | PASS | 新 profile 在 owner 边界拒绝非法 score，恢复拒绝 policy/profile 不一致；T002/T003。 |
| II Evidence | PASS | 保存原 fixture/model/grader hashes、对照、失败；T001/T004。 |
| III Hermetic | PASS | sibling tests 和实际本地模型，无网络／付费调用；T003/T004。 |
| IV Closed sets | PASS | bool field，未知字段／错误类型拒绝；profile 精确匹配。 |
| V Workspace/API | PASS | 原默认行为与公开签名保持，显式 additive API；无依赖迁移。 |
| VI Schema | PASS | 新 policy 字段可读旧数据，false 序列化字节不变。 |
| VII Ratchets | PASS | 原金标、阈值与 RRF default 不改；T004。 |
| VIII Transports | PASS | Core host API，无 ACP wire 扩展。 |
| IX ACL | N-A | 无新产品配置，沿既有 host typed policy。 |
| X Tests | PASS | sibling unit / public integration；T003/T004。 |
| XI Judgment | PASS | 不把一条 probe 当完整质量资格。 |
| XII Review | PASS | caller/数据/profile 已读；T005 复核边界。 |
| XIII Repository | PASS | 当前 checkout，无新 repo/worktree/subagents。 |
| XIV Names | PASS | 精确 node ID/profile，不猜名称。 |
| XV Meaning | PASS | 无 query 分类／词表／按查询挑算法。 |
| XVI Pickers | N-A | 无 UI。 |
| XVII Dialogs | N-A | 无 UI。 |

## Verification

Core kernel、durable_memory sibling tests、原 durable_memory_semantic_eval；ACP target 的原 probe（保留已知 RRF FAIL）与新 linear probe/10-query actual owner eval。原 Python metrics 不变；输出证据在 `docs/evidence/code/2026-11-01-memory-score-fusion/`。owning all-targets Clippy/no-deps、minimal check、paired build、scoped fmt/diff；不声称 Windows/Linux native qualification。
