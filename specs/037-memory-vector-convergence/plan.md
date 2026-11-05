# Plan: 旧记忆向量机制收敛

当前 checkout，无 branch/worktree/委派。当前 `InMemoryVectorIndex` 已有同步 build/publish/remove/search kernel；只公开三个 blocking ports，普通 async trait 委托这些 ports 并继续在既有 blocking executor 上运行。CAS/clear 的原 owner 不变。异步 search 的 snapshot 在线程开始执行时读取；仍满足既有一次 immutable revision 合同，不声称 admission 时刻 snapshot。

Core 旧 facade 保留一个可 clone 的 index handle slot；第一次有效 embedding 决定该 session generation 的 dimension，使用原默认预算与 cosine。锁只用于取 handle／更新派生状态，不包围 provider 的 query embedding。provider replacement 使旧 handle 失效，不制造模型 revision 或迁移 primary data。新的错误保留 source；可选语义错误使用固定诊断降级，不能分支判断 rendered error text。

put 先检查已知 digest，完成原 collection insert/flush 后才写 digest 和派生 index。管理 edit/delete/clear 复用相同 update/remove 边界；无新权威 map，不保留第二份向量列表。原去重成功 no-op 政策保持，未来生命周期问题继续沿 T021–T025 收敛到已有 refresh，不能新增平行 backfill pipeline。

## Constitution check

| Principle | Status | Application |
| --- | --- | --- |
| I Fail closed | PASS | shared typed vector validation；可选失败清除旧 derived entry，primary errors 保留。 |
| II Evidence | PASS | old-code red、baseline hashes、所有 attempts 与 OUTCOME；T001–T005。 |
| III Tests | PASS | Product sibling / actual collection tests；共享 port 同实例 cross-port tests。 |
| IV Closed sets | PASS | 复用现有 vector descriptor/error types；无新产品 enum。 |
| V Workspace/API | PASS | additive blocking methods；旧 API/feature/minimal 保留。 |
| VI Schema | N-A | 无 primary/persistent schema 变更。 |
| VII Ratchets | PASS | 原 fixtures/grader/profile/限额不改，无 skip/allowlist 扩大。 |
| VIII Transports | PASS | 同步/异步薄入口委托同 kernel，无 runtime bridge。 |
| IX ACL | N-A | 无新产品配置。 |
| X Test files | PASS | 新 tests 在 sibling，既有 tests 不搬业务逻辑。 |
| XI Judgment | PASS | 原错误合同以实测核对；不把原语收敛当 full integration。 |
| XII Review | PASS | 所有 vectors/mutation/provider caller 已读；T005 adversarial review。 |
| XIII Repository | PASS | 同仓，无额外 worktree 或 agents。 |
| XIV Names | PASS | 原 record IDs，不推定实体。 |
| XV Meaning | PASS | 无 query matcher／词表。 |
| XVI Pickers | N-A | 无 UI。 |
| XVII Dialogs | N-A | 无 UI。 |

## Verification

`aster-durable-memory` default + existing sqlite feature vector tests；Core legacy memory/management、shared fusion、minimal；owning Clippy all-targets/no-deps、scoped/root format、paired CLI/ACP build。默认 RRF/原 context-ID 的已知失败不隐去。原生 macOS arm64；Windows/Linux 未运行不算 PASS。
