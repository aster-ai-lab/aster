# Implementation Plan: Core 检索融合收敛

**Branch**: 当前 checkout | **Spec**: [spec.md](spec.md)
**Gate context**: Root Product/Core；同步公开旧 API 与异步 typed／workspace API 均保留。

## Summary

将 Core 三个已有 RRF 归并循环收敛到私有 `rank_fusion`；它只拥有 ID/channel 去重、rank 准入与贡献累计。原 domain facade 保留源数据和排序政策。先运行原 release benchmark，再改变数据布局；不从单条失败 probe 推广新排名策略。

## Constitution Check

| Principle | Status | Evidence in the plan |
| --- | --- | --- |
| I. Fail closed | PASS | 非零 rank 一次准入；T002/T003。 |
| II. Evidence retained | PASS | 原 baseline、每次 attempt、D05 FAIL；T001/T006。 |
| III. Hermetic tests | PASS | sibling unit tests 与既有原实现资格；T002/T004。 |
| IV. Closed sets | N-A | 复用已有 channel types，无新产品 enum。 |
| V. One workspace / published APIs | PASS | 仅 Core 私有模块，minimal 不增加 vec 依赖；T003。 |
| VI. Local schema changes | N-A | 无存储 schema。 |
| VII. Shrink-only ratchets | PASS | 原 fixtures／gates／profiles 保持；T001/T004/T005。 |
| VIII. Replaceable transports | PASS | ACP 不改 wire；T004。 |
| IX. One configuration language | N-A | 不加产品配置。 |
| X. Commands / tests | PASS | 原 Cargo examples 与 sibling tests；T002/T004。 |
| XI. Independent judgment | PASS | 旧 tie 缺陷与已知质量失败分别处理。 |
| XII. First principles / review | PASS | 已读三处 caller 与政策，先 baseline/反例；T001/T002/T005。 |
| XIII. One repository | PASS | 无新 repo/worktree/自动委派。 |
| XIV. Names are not guessed | PASS | 使用原 ID/channel，排序只看 typed 数据。 |
| XV. Meaning is not matched | PASS | 不加 query 意图／词表规则。 |
| XVI. Pickers are drawn | N-A | 无 UI。 |
| XVII. Dialogs stay in place | N-A | 无 UI。 |

## Project Structure

| Layer | Existing path | Change |
| --- | --- | --- |
| Shared Core mechanism | `crates/code/core/src/lib.rs` | 私有 `rank_fusion.rs` 与 sibling `rank_fusion_tests.rs`。 |
| Workspace facade | `workspace/retrieval/hybrid_rank.rs` | 委托归并；保留 exact/path/span/ID 和 channel 顺序。 |
| Typed memory facade | `durable_memory/fusion.rs` | 委托归并；保留 raw single-channel、f32 max 归一化、较晚 semantic revision 与 ties。 |
| Legacy facade | `durable_memory_vec.rs` / sibling tests | 委托归并，执行已声明的第一通道 tie。 |
| Qualification | 原 examples／Core tests／ACP stdio | 不复制旧业务逻辑进新 tests、不改 fixed inputs/grader。 |

## Research

- 当前 Core 三处都固定 k=60，但拥有不同 domain payload/precision/ties；共享机制不能把这些差异当重复政策删除。
- workspace 原 ranks 使用 BTreeMap；共享条目保留首次 channel 顺序供累计，输出按原 channel 顺序排列。typed 只有 lexical/semantic 两路。新布局的性能先后由原 release workload核对。
- `aster-search::RankingConfig` 另含 provider-local 校准和 search-result identity；`aster-vec` multi-query 支持其它策略。它们仍是 033 的收敛候选；不能强迫 minimal Core 依赖 vec，或为一个公式迁移 crate feature 结构。
- D05 的失败与已发布 `rrf-k60.v1` 策略有关。本批次不悄改 profile 行为；完整目标仍要求后续有据的质量改进。

## Verification Plan

```sh
cargo run --locked --release --target-dir target-issue-fix -p aster-core --example workspace_retrieval_benchmark
cargo test --locked --target-dir target-issue-fix -p aster-core --lib rank_fusion
cargo test --locked --target-dir target-issue-fix -p aster-core --lib durable_memory
cargo test --locked --target-dir target-issue-fix -p aster-core --lib workspace::retrieval::
cargo test --locked --target-dir target-issue-fix -p aster-core --test durable_memory_semantic_eval
cargo test --locked --target-dir target-issue-fix -p aster-acp --test retrieval_qualification_stdio
cargo clippy --locked --target-dir target-issue-fix -p aster-core -p aster-acp --all-targets --no-deps
cargo build --locked --target-dir target-issue-fix -p aster -p aster-acp
```

- 测 Core minimal compilation；不声称 native Windows/Linux、完整 host profile 或 release 总体性能完成。
- 复开 source/callers、数值与 score/channel 元数据、inputs/fixture hashes。已知 D05 FAIL 仍失败时不能记录整 target PASS。
