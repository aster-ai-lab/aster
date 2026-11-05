# Research: 已有实现、证据与待测点

**Date**: 2026-11-01。以下是源码检查结果，不是新增运行成绩。

## 1. 可复用路径

| 领域 | 已有实现 | 本计划如何复用 |
| --- | --- | --- |
| 能力声明／工具注册 | `crates/code/acp/src/host_context.rs`、`agent/session_open.rs`、`crates/code/core/src/tools/builtin/mod.rs` | 保留现有 typed host opt-in 和动态工具注册，不另建注册中心。 |
| 工作区检索 | Core `tools/builtin/search.rs`、`workspace/retrieval/{hybrid_query,hybrid_rank,rerank}.rs` | 继续一套 search 工具和原有四通道、RRF／可选 MMR；优化共享路径。 |
| 索引 owner | Core `workspace/retrieval/{runtime,persistent}.rs` 及 `persistent/tests.rs` | 沿用 manifest watcher、generation、delta、reopen，不另造扫描器。 |
| 语义向量 | Core `workspace/retrieval/{semantic_runtime,semantic_projection,memory_vector_adapter}.rs`；`crates/memory/src/vector/{in_memory,search}.rs` | 当前精确、会话内向量是合法基线；按实际规模测量。 |
| 本地模型 | `crates/code/acp/src/agent/local_embedding.rs`、`crates/embedding/src/{adapter,provider,assets}.rs` | 已有进程内缓存保留；模型字节和校验沿用当前契约。 |
| 宿主桥 | ACP `document_read.rs`、`memory_search.rs`、`wiki_search.rs`；Studio `agent_plane/turns.rs` | 沿既有 transport、run access 和 owner 清理路径增补测试／具体缺桥。 |
| 宿主跨工作区搜索 | Studio `agent_plane/workspace_{wiki,memory}_search.rs` 和 sibling tests | 原有 profile／binding 查询、分页、预算和 provenance；先测再优化 fanout。 |
| 资料／SAG | Studio `local_documents/library.rs`、`library_knowledge.rs`；`crates/sag/src/lib.rs` | 宿主继续提供资料扫描、解析、模型抽取、索引与 source fence。 |
| 文档原生处理 | ACP `office_read.rs`；Studio `agent_plane/{document_read,workspace_wiki_documents}.rs`、`document_processing` | 复用 native Office／PDF／OCR；不把 probe stub 当真实处理器。 |
| ACP 本地记忆 | Core `durable_memory_vec.rs`、其 management／tests；ACP `agent/memory_management.rs` | 保持现有权威存储、管理语义和锁行为。 |
| 另一套 typed durable 语义能力 | Core `durable_memory/{binding,semantic,semantic_binding,semantic_refresh}.rs`、`memory/semantic_refresh.rs`、`agent_api/session_options.rs` | 先对照可复用算法／接口，不直接迁移存量 MemoryStore，不假定 Candidate 等同 Active。 |
| 实际 ACP 用例 | ACP `tests/shipped_tui_e2e/retrieval.rs`、`src/agent/retrieval_seam_tests.rs`、`memory_search/tests.rs`、`wiki_search/host_tests.rs` | 保留已有证据 oracle；补真实 process 和宿主路由缺少的路径。 |
| 既有度量 | Core `examples/workspace_retrieval_benchmark.rs`、`workspace_persistent_index_production.rs`、`workspace_retrieval_aster.rs`；`tests/retrieval_bench/metrics.py` | 保留算法／资源 gate，区分控制 provider 与真实 provider，复用指标公式。 |

源码表中的 Studio 路径均以 `apps/studio/src-tauri/src/` 为前缀。它是私有 gitlink 内的源码，实施时在其原有工作流中修改和验证，不能加入父仓库普通文件。

## 2. 已确认事实

1. 既有 ACP 检索 slice：17 个相关 unit tests + 3 个真实 stdio 测试通过；Core 局部 5,000 文件样本查询最大值 23.609 ms，完整 ACP 冷序列约 5.459–6.858 s。均为 debug、局部样本，不是发布 SLO。
2. bundled model 在 ACP 中已使用 `OnceLock` 复用。已有测试确认同一 Arc、预取消与宿主 factory 优先；该修复不需要再规划一套模型缓存。
3. `tests/retrieval_bench/run_quality.py` 解析 `--engine`，但 main 只运行 `obvious_ranker`／ripgrep，engine 路径仅写入结果字段。必须先修复真实执行路径或拒绝 unsupported engine，才能使用其成绩评估产品。
4. `workspace_retrieval_aster.rs` 是固定语料／orthogonal-vector control 程序，并不是可直接按 query 调用的通用 JSON 排序 worker。不能假定它能接收质量脚本现有参数。
5. `workspace_retrieval_benchmark.rs` 使用 deterministic provider，有既定 release gate：25,000×384 exact p95 ≤30 ms；hybrid p95 ≤100 ms；rerank 新增 p95 ≤10 ms、scratch ≤4 MiB。它证明该夹具的性能／结构性质，不证明真实语义相关性。
6. `crates/embedding/src/adapter.rs` 的 async embed 内直接执行同步 `encode_batch`。这是需要测 executor 响应性的候选点；尚未证明它是当前冷启动的主要耗时。
7. `VecMemoryStore::open` 的 semantic provider 为 None；setter 不回填旧记录；语义 vector list 为进程内派生状态。仅接 setter 无法完成存量／重启资格。
8. Core 已有 typed durable semantic binding、refresh checkpoint、cache 和 maintenance owner。它与 ACP 绑定的 `LazyFileMemoryStore` 不是同一个接口，不能把“底层有 API”算作已接 ACP，也不能再实现一套相同 refresh 框架。
9. Studio Wiki／memory 查询已有 3 秒总预算、分页、最多 12 hits／48 KiB 和完整性字段；逐 owner 查询及 memory 各层调用是候选成本，不能直接取消限额追求完整覆盖。
10. `library.rs` 的热查询目前会 scan／sync，并在 `with_index` 的进程全局 map mutex 内执行索引操作。需要测扫描量和锁等待再决定优化；不得用无效化机制不明的缓存换取速度。
11. Studio `tests/workspace_document_stdio.rs` 是 `harness=false` probe，内部 `document_processing` 明确返回不可用。此 probe 不能替代真实 PDF／OCR 正向验收。`document_tools_native` 是另一实际 target，实施前核对其依赖和覆盖。
12. 当前通用 CI 是 macOS／Linux，CLI 发布已有 Windows runner；若新用例沿用 `shipped_tui_e2e.rs` 的 cfg(unix)，不能宣称 Windows 已测。规划要求将无 shell 的 ACP contract cases 放到可原生运行的独立 target。

## 3. 需要通过测量决定的事项

| 决策 | 默认方案 | 改变默认所需证据 | 决策任务 |
| --- | --- | --- | --- |
| 启动瓶颈 | 保留异步索引、现有模型缓存 | 单调时钟阶段分解 + release baseline 定位阻塞关键路径 | T004、T005、T009 |
| 本地编码执行方式 | 保持 API；按有界批次测响应性 | 确认 CPU 同步调用使取消／会话响应超目标，再放入既有 blocking owner 路径 | T010 |
| 跨工作区 fanout | 保留原分页／总预算／owner | 1／8／32 个 owner 的排队、CPU、I/O 与覆盖率对照，证明具体瓶颈 | T017、T018 |
| SAG Agent 入口 | 优先使用既有宿主桥 | 实际 Agent 原文检索旅程无法通过既有 operation 到达宿主 SAG；才添加 contracts 中的最小候选桥 | T006、T016 |
| 记忆语义复用 | 现有 MemoryStore 为权威，语义为派生读投影 | 对照 typed durable API 与旧存储的 ID／revision／激活政策，选无破坏兼容路径并冻结 | T021 |
| Rerank 推广 | 现有 RRF-only 默认 | 固定质量集有收益、exact symbol 与当前引用不退化，且不越既有资源 gate | T026、T027 |
| ANN／跨会话缓存 | 本期保持现状，输出决策 | 大规模发布负载明确不达目标，且成本分解表明相应方案有价值；另立后续 bounded slice | T028 |

## 4. 排除与保留

- 不依据用户意图猜词路由工具；使用模型选择和宿主 typed capabilities。
- 不按外部项目 feature list 扩建通用数据库接口；先为已有 Agent 旅程闭环。
- 不把宿主实现判作架构缺口，也不把其 UI 可用判作 ACP 已验收。
- 既有 full fmt 差异和 runtime-boundary 版本文档不一致分别登记处理；不修改 grader／allowlist。
- 本轮不执行产品基准或新用例，旧 evidence 继续保持原结果；实施从 T001 重新记录准确源码与二进制身份。
