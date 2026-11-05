# Feature Specification: 旧记忆向量机制收敛

**Created**: 2026-11-01
**Status**: Implemented and qualified for this convergence slice; D01 integration/lifecycle remains open
**Scope**: Product/Core + existing shared `aster-durable-memory::vector` owner；033 D01/D04 与 032 T021 的兼容原语阶段。完整替代目标不缩小。

## Requirements

- FR-001：旧 `SemanticEmbed`、`VecMemoryStore::put/recall/set_semantic_provider` 公开签名保持；权威记录仍在原 collection，不搬入 V2 repository，不推定 Active／已验证。
- FR-002：删除旧 `Mutex<Vec<(String, Vec<f32>)>>` 和手写 dot/cosine/sort。同步与异步入口委托现有 `InMemoryVectorIndex` 的同一验证、normalization、snapshot、预算与 exact search owner；无 nested runtime、block_on、第二模型服务或新依赖。
- FR-003：旧文档声称 cosine，实际 dot/zip 会误排非 unit 向量并接受维度错误。使用原 owner 的真实 cosine／有限值／dimension 验证；错误只降级旧语义通道，不破坏已存记录或改变 RRF profile。
- FR-004：put 的去重状态与派生向量不能在持久写入成功前发布；失败后可用同内容重试。edit／delete／clear 使用同一个派生 owner；失败的 embedding 更新移除旧向量，provider 替换清除上一代派生状态。
  原 put 已公开承诺 insert-or-replace；使用现有 collection upsert 实现该合同，替换后释放旧 digest。同内容 no-op 只刷新实际已存 ID 的派生向量，不给被去重跳过的新 ID 建立 phantom entry。
- FR-005：查询 embedding 不在旧 vectors slot lock 内执行；同步调用者可直接使用 blocking port，异步 trait 继续用既有 spawn_blocking。一次调用读取一个完整 immutable revision；不新增并发协调层。
- FR-006：保留当前旧 setter 的 future-put 生命周期，不在兼容阶段再写一套存量回填／持久缓存／刷新调度。完整目标仍要求后续把 source adapter 接到已有刷新 owner，补齐 restart/backfill、真实 provider 和 ACP/host 路由；这些未完成项必须继续开放，不能据此关闭 D01 或 032 T021–T025。

## Acceptance

1. 真实旧实现的非 unit cosine、失败写入 phantom vector、失败后同内容重试反例先 red；不改现有 query/gold/模型或门限。
2. 无 Tokio runtime 的 shared-index tests 与 async trait 在同一个实例上交替读写，status/revision/history 和结果一致，边界拒绝不改已发布状态。
3. 实际 VecMemoryStore／临时 collection 测新维度／NaN／provider change／edit/delete/clear；既有管理和 RRF tests 保持。
4. owning tests/Clippy/fmt/minimal/paired build 有终态记录；保留既有失败与未测平台，性能不由去重代码存在推定。
