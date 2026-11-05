# Core 机制与收敛账本

本文件跟踪用户当前目标，完成条件是机制完整、唯一、有效且高效。固定版的 MCP 22 项及其它入口用于找到行为；反馈提交／界面／传输适配由原宿主设施承担，不自动变成新的 Core 机制。此表仍在审计，没有完成百分比；路径与库存在都不算 PASS。

## 机制分母初表

| 机制 | 对照来源 | 原 owner／复用候选 | 资格与收敛状态 |
| --- | --- | --- | --- |
| 来源准入、分块、revision 与原文复核 | autoindex、search、def、code、source reads | Core `workspace`／retrieval；宿主资料／Office owner | 032 继续实际更新、删除、scope、读取预算与归因资格。 |
| 数据格式、schema 与字段 | autoindex extractors、查询 AST、mapping | 原 Office／OCR／宿主 ingest + Core／vec typed schema | 全格式、字段和 schema 行为尚未审计完整。 |
| lexical／BM25／analyzer | query AST、search | `aster-vec` FTS 与 Core catalog facade | 需对照 native／portable／query-time scanner，确定是否存在可合并的排名实现。 |
| embedding／模型身份／批次 | semantic search、memory、vector fields | 现有 `EmbeddingProvider`／`EmbeddingExecutor` + 本地 provider | light bundle 身份已补齐，18 owning tests 通过；真实 typed memory 可构建／回填，但同义查询融合排名失败，未通过完整资格。 |
| vector exact／ANN／过滤／CAS | knn、semantic、vector storage | 原 `aster-vec`、`aster-durable-memory::vector` | 保留必要算法选择，但收敛重复 kernel／index owner；需测量与完整调用图。 |
| RRF／linear／重排／局部 judge | hybrid／memory recall、rerank／judge | 原 Core fusion 与共享 vec 机制 | 多处融合实现待比较；不能为每个入口复制排序／模型服务。 |
| 批量编排、每项错误与输出预算 | batch_search、max_tokens／locators | Core 现有工具编排／结果策略 | 新接口只委托同一机制；顺序、取消、字节／真实 token 与 coverage 需独立资格。 |
| 代码定义、结构化引用与来源 | def、code_search、passage output | 原 code_intelligence／source owner | 需验证 definition-first、语言／kind、license 与当前 source span。 |
| typed query／SQL／聚合 | QueryNode、search、aggregation／SQL implementation | 原 vec typed query／filter owner | `query-owners.json` 的 44 个 QueryNode 变体及有序字段已与固定源码 AST 双向核对；支持类型、默认值、rewrites、SQL、聚合及实际语义仍未完成。 |
| 自然语言到 typed query | plan／ask | Core 模型与现有 typed validation | 意图由模型／typed 输入决定，不搬运上游规则型意图 matcher。 |
| 记忆 CRUD／语义／recency／dedup | memory store／recall | 原权威 MemoryStore／repository；现有 typed refresh／vector | v1 手写向量列表与 typed 机制必须收敛；不迁移／提升原 activation 政策。 |
| 图关系、证据、时态与记忆耦合 | brain ego／link／unlink／overview、graph recall | 原宿主 SAG／memory owner + Core 共享查询原语 | 需逐项验证，有 SAG 不代表已有全部时态／图语义。 |
| dataset 生命周期／watch／resume／预算 | autoindex、map、budget、remove | 原 Core manifest 与宿主 ingest／catalog owner | 不重复建立索引目录／作业 owner；完整 add/edit/delete/rename/restart 需取证。 |
| Corpus／pack／身份／签名／许可 | code_search、Hub／Corpus、pack implementation | 原宿主资料 owner，共享 Core 检索 | 原 source owner 持久化，Core 使用获准来源；缺口待完整合同。 |
| 持久性、WAL、snapshot／restore、迁移 | storage／engine／native operations | 原 vec／memory repository／source owner | 权威数据、派生索引与备份不能混称；恢复、升级和损坏需真实测试。 |
| 分片／副本／metadata consensus | 固定版 cluster 实际交付代码 | 待审既有共享组件与宿主部署 owner | 上游 partial 与已交付行为分开；不能虚构已支持或先新增协调层。 |
| 取消、deadline、资源与 native 生命周期 | 全部路径 | 原 Core／ACP／宿主 owner、tracing／metrics | 032 阶段观测进行中；CPU／queue／worker settle 与前台响应分开。 |

## 已读源码的重复候选

### D01：v1 记忆语义列表

- 初审时 `crates/code/core/src/durable_memory_vec.rs` 定义 `SemanticEmbed`、`Mutex<Vec<(String, Vec<f32>)>>`、put 时 append、手写 dot/zip（文档声称 cosine）与 `rrf_merge`。其 setter 没有本仓库生产调用者，公开 SDK 仍可能使用，不能直接删 API。
- Core 已有 `DurableMemorySemanticRecall`、有界 refresh／checkpoint／embedding cache，底层使用 `aster-durable-memory::vector::VectorIndex`。权威记录／activation 政策须先对照。
- 必须完成：旧公开入口以兼容 facade 委托共享机制；旧记录、更新、delete／prune、重启、失败和只读政策都有实际回归。不能给旧路径再复制一套 backfill 或 vector list。
- 旧 `SemanticEmbed`／`put`／`recall` 是同步公开 API，provider 没有 descriptor；typed refresh 是有 runtime／取消约束的异步 API。不能靠在同步入口嵌套 Tokio `block_on`、新建第二 runtime 或假定 provider 身份实现兼容。先核对同步基础原语及实际异步调用者，再收敛；旧记录的无 activation 状态与 typed Active-only 政策也不能直接等同。
- [037](../../037-memory-vector-convergence/spec.md) 已删除旧私有向量列表与 dot/sort，改为同一个 `InMemoryVectorIndex`；仅添加 blocking ports，原 async trait 在既有 worker 上委托同一 admission/snapshot/exact owner。没有第二 runtime／索引算法／权威库或伪造的模型 revision。原同步公开 API 保持，未来 put 的派生生命周期保留；provider 更换不复用旧向量。
- 037 实际 old-code 4 个反例均 red（非 unit 误排、失败写入 phantom vector、同内容重试被吞、NaN 准入）；最终旧路径 44 项通过（含 typed engine error-code）。另有 shared unit 6、公开 default 19、sqlite 特性 27、workspace 185、typed memory integration 18 项通过；owning Clippy/minimal 与原 release workload gates PASS。最终 CLI/ACP build ID 一致，实际 ACP 回放 8 PASS／1 原 RRF FAIL；完整证据见 `docs/evidence/code/2026-11-01-memory-vector-convergence/OUTCOME.md`，不等同于 legacy 语义的 ACP 启用。
- `2026-11-01-memory-async-boundary` 已把 ACP 实际使用的 lazy adapter 的首次 open/import 和 collection 读写移到既有 Tokio blocking pool，direct/lazy 复用相同 native bodies；沿用同一 Weak registry 与最终释放协议，没有新 runtime／权威库。两个旧源码真实锁等待反例一起与单独均失败；修复后 Core memory 53、ACP management 2、原 Memory/File retention lifecycle 14 项通过，minimal、owning Core/ACP Clippy/sqlite 与 CLI/ACP 配套构建通过。取消等待后实际 worker 仍持有 collection，完成写入后才释放；不宣称取消已终止同步工作。证据见 `docs/evidence/code/2026-11-01-memory-async-boundary/OUTCOME.md`。
- `2026-11-01-memory-retention-convergence` 已删除 InMemory/File/SQLite 的三份 age/protection/quota 选择循环，统一委托 `ValidatedPrunePolicy`；Native Vec 的原 no-op prune 已接入同一策略和原 primary/vector owner。真实旧源码复现 native 无效果、NaN 误删、时间溢出 panic、语义残留与 SQLite 损坏行误删；修复后 shared Memory default 63／sqlite 84（重叠 cohort）、Native 58、default/sqlite 各 5 个适配器与实际维护 owner tests、原维护 7、ACP management 2 项通过。Native single delete／clear／prune 共用一次物理批量提交/checkpoint，公开方法签名和配置默认值保留。证据与完整尝试见 `docs/evidence/code/2026-11-01-memory-retention-convergence/OUTCOME.md`。
- `2026-11-01-memory-sqlite-read-errors` 已收敛五个非 prune 调用点的错误传播：store_and_return／search／tags／recent／important 通过原 row decoder 与 blocking connection owner 返回 typed source，tags 不再复制连接 bridge。真实旧源码 4 个用例一起和各自单独均 red；修复后 owning default 63、sqlite／sqlite-vec 各 88（重叠）、Core adapter／checkpoint 6、ACP real stdio management 1 项通过，owning Clippy 全 targets 与配套构建通过。排序、ANY-tag 和结果 limit 保留，失败去重扫描不写入 DB／Markdown。证据见 `docs/evidence/code/2026-11-01-memory-sqlite-read-errors/OUTCOME.md`；不代表所有 SQLite 边界均已合格。
- 优化前的原固定 5,000×256-byte primary diagnostic 未进入清理阶段：逐条 put/checkpoint 的播种观测停在 revision 2048，完整性能样本为 0。原 source／参数未改；没有把部分播种当成 M 档或速度提升。写入端 checkpoint、WAL／资源与宿主生命周期须先单独核验；当前同步 put 的持久性行为未削弱。
- `2026-11-01-memory-checkpoint-chain` 已在原 StorageHandle owner 内每 32 代复用 full snapshot／原 manifest 后清理，保留逐次 flush、WAL 同步、格式和 no-op generation；没有第二 checkpoint loop。两个实际旧源码单独反例均 red；修复后 engine unit 241、公开全特性契约 38、Core native 58／维护适配器 12、ACP real stdio 1 项通过，owning Clippy／minimal／配套构建通过。原 5,000×256-byte source／断言未改，现完整跑通一份样本：删除 2,000、保留 3,000；共享主机 seed 374.343s、prune 172.745ms。不把单次 primary diagnostic 当 p95／速度比／语义 M 档。旧长链递归读取、逐次写入的 WAL／checkpoint／index-cache 成本及 best-effort 清理残留仍需处理；完整证据见 `docs/evidence/code/2026-11-01-memory-checkpoint-chain/OUTCOME.md`。
- `2026-11-01-memory-wal-checkpoint` 已在同一 engine checkpoint owner 接入 Always 阈值，native 使用 collection-local Always／128 operations／8 MiB，删除 put 的逐条显式 snapshot/index-cache flush，保留管理和关闭 flush；不再重置宿主已初始化的默认配置。同一 WAL 编码边界在文件变更前限制累计 committed bytes，不超过重开预算；原维护 owner 为下一帧留空间并允许删除恢复快照超限。十个不同的真实反例分阶段转绿，编译失败／SIGTERM 不计入；最终 engine unit 250、公开 async/jieba 40（含 durability 10）、Core native 61／边界维护 14、ACP real stdio 1 项通过，Clippy／minimal／配套构建／owning format 通过。原 5000×256-byte 断言未改，删除 2000／保留 3000；完整单次 seed 63.490s、prune 68.490ms。不同负载的共享主机单样本不证明稳定速度比／p95／语义 M 档；小 replay ceiling 保守地每次 checkpoint，旧长链 reader 仍待处理。前两条性能／逐次 flush 描述是历史阶段状态，当前结果与完整尝试见 `docs/evidence/code/2026-11-01-memory-wal-checkpoint/OUTCOME.md`。
- `2026-11-01-memory-legacy-chain` 已将原 snapshot reader 的递归祖先读取改为迭代，并在全部 delta 重放中复用一张文档合并表，删除逐代全表重建。真实旧源码在原写入器生成的 2048 代链／2 MiB worker 栈上溢出，单独重跑同样失败；相同反例在候选恢复全部 schema／文档。最终 engine unit 254、公开 async/jieba 40、Core native 61／边界维护 14、ACP real stdio 1 项通过，Clippy／minimal／配套构建／owning format 通过；全仓 fmt 仍是另外 9 个文件。保留格式、checksum／metadata／schema／generation 校验、full 文档顺序与 delta 删除／重插／最后写入语义。解码后的 delta 仍暂存内存，未宣称任意旧链的总工作预算或取消已合格，也未把前阶段计时当本次构建性能。前两阶段关于递归读取的描述是历史状态；完整证据见 `docs/evidence/code/2026-11-01-memory-legacy-chain/OUTCOME.md`。
- `2026-11-01-memory-canonical-dedup` 已让 direct/lazy MemoryStore 和旧 JSON 导入委托同一 canonical 写入 owner，复用原 fingerprint／merge，返回真实持久化 ID／内容，保留 typed 同 ID 替换、短片段独立及 raw SDK exact no-op 政策。旧 digest Vec／第二 hygiene mutex 已删除，派生 key/ID lookup 由原 mutation mutex 持有，重开／失败后从 primary 重建，更新／删除／prune／clear 与历史多 ID 共用同一维护。真实原源码与中间候选的 ID、metadata、短片段、重开、导入、故障及标签反例均保留；最终 native 75、边界维护 15、ACP real stdio 2 通过。复杂／空标签通过原辅助数据的可选字段保真，普通标签字段集合不变；旧严格管理读取器对该扩展的回退兼容尚未取得发布资格。完整证据见 `docs/evidence/code/2026-11-01-memory-canonical-dedup/OUTCOME.md`。
- `2026-11-01-memory-schema-identity` 已修复默认配套 ACP 与独立 SDK 因 JSON map 构建特性不同而无法重开的真实故障。同一 schema owner 为 digest、full/delta writer 规范化参数对象键，保留字段／数组顺序；解码保留旧编码顺序，在原 metadata 边界精确校验旧 digest，错误 digest 仍拒绝，升级继续用原 checkpoint。旧 SDK／旧 ACP 失败可复现；旧 SDK／新 ACP、新 SDK／旧 ACP、新／新均实际通过。最终 engine 258／公开契约 40、Core 75／边界 15、ACP real stdio 2、Memory sqlite-vec/preserve_order 88、Clippy／minimal／配套构建通过。原 5000×256-byte 输入／断言未改，候选两次删除 2000／保留 3000；同机顺序对照 seed 基线 66.609s、候选 74.358s，CPU 合计 5.173s／4.679s。候选墙钟更高，不能宣称效率改善、p95 或语义 M 档合格；效率资格继续开放。详见 `docs/evidence/code/2026-11-01-memory-schema-identity/OUTCOME.md`。
- D01 未结案：自动存量回填／restart 模型身份、既有刷新 owner 的 source adapter、ACP/host 语义启用／coverage、主数据发布后错误的语义重建及 M 档效率仍须完成；复杂标签旧版管理读取兼容、native access／零 limit／旧 raw codec、SQLite 可选 sqlite-vec ID filter／session-log 等剩余边界继续跟踪。直接 VecMemoryStore 的 async 兼容 trait 与同步析构不在 lazy-operation executor 隔离资格内；独立 Core 默认仍是 FileMemoryStore，ACP 显式注入 lazy native adapter。最新 follow-up 在 schema-identity 证据的 `FOLLOWUPS.md`。不得新建另一套 backfill／dedup／checkpoint pipeline，或把本次功能归并当完整记忆／效率资格。

### D02：工作区向量 adapter 是复用关系

- `workspace/retrieval/memory_vector_adapter.rs` 的 `MemoryVectorIndexAdapter` 已委托 `aster_durable_memory::vector::InMemoryVectorIndex`。
- 工作区／会话保留独立数据和权限是必要 owner 边界，不是新的相似算法。保留此委托关系，不再新建 workspace vector 实现。仍需 Q30 及 close／revision 资格。

### D03：模型身份是复用阻塞点

- `LightTierProvider::descriptor()` 原返回 `revision=None`；`DurableMemorySemanticBindingV1` 要求 revision。
- 真实 integration regression 已失败，错误字段为 `binding.embedding.revision`；见 `model-binding-red.log`。
- 修复方向是在现有 provider 绑定整个已验证 bundle 的身份，维持 typed memory 的准入约束。不能放宽约束或为缺身份的 provider 新建另一条记忆语义流程。
- 已修复为 model／tokenizer／config 三份固定 assets 的身份；真实构建和回填通过，原模型字节未变。6 项 native 回归中 5 通过，新增记忆 Top-1 probe 失败；不能把该 probe 当已通过的记忆资格。

### D04：其它向量与融合候选仍需实际调用图

- `crates/embedding/src/vector/{linear,hnsw,backend}.rs`、`crates/memory/src/vector/`、`crates/vec/src/index/` 具有相关算法／索引。先查全部生产消费者、metric／normalization／filter／revision／预算及公开 API，再收敛到既有最低 owner；不同算法本身不能仅凭名称判重复。
- Core 的 `durable_memory_vec::rrf_merge`、`durable_memory/fusion.rs` 与 workspace `hybrid_rank.rs` 已通过 [034](../../034-retrieval-fusion-convergence/spec.md) 委托同一个私有 `rank_fusion::RankFusion`，三处独立累计循环已删除。保留各自 precision／source／revision／normalization／tie 政策；旧 helper 的同分不确定与同 channel 重复计分反例已从 red 转 green。完整资格继续记录，不能由 grep 代替实际调用。
- `aster-vec::multi_query` 和 `aster-search::Aggregator` 另有融合／校准；共同源码与政策差异仍须审计。minimal Core 不被强制引入 vec，也不为一个公式迁移 crate feature 结构。这些候选没有因 Core 三入口收敛而结案。
- `HnswBackend` 的公开 test flush seam 已删除，原 sibling tests 直接调用私有 flush，18 tests 保持通过；这不代表其它向量 backend 已收敛。

### D05：真实记忆融合的已确认质量失败

- 原 bundled provider 的相同三文本单测确认：查询 `parse the config file` 与 `read configuration settings` 比天气文本更接近。
- 实际 Core typed memory preview 却把 `weather` 的 hybrid 分数归一到 1.0，把 `settings` 的 semantic 分数排到第二位（0.504065）。原始回执见 `docs/evidence/code/2026-11-01-acp-retrieval-optimization/memory-fusion-diagnostic/bundled-memory-preview.json`。
- 原 repository 词法按 token 交集打分；该冻结 probe 的 lexical floor 为 0，天气文本的 `the` 命中进入 RRF。这个证据说明融合不能只凭单通道模型单测判有效。
- 后续必须沿 T026/T027 的固定集与原 profile 契约处理，复用同一排名 owner；不能调高 probe 的 lexical floor、缩减 semantic candidates、换 query／gold 或单独给这条记录加规则来转绿。
- 新的实际 EmbeddingExecutor → 原 InMemoryVectorIndex 观测给出 settings=0.4223333、weather=0.0249560；见 `docs/evidence/code/2026-11-01-retrieval-fusion-convergence/semantic-baseline/direct-semantic.json`。模型和向量索引的顺序均正确，弱词法命中在 RRF 中获得几乎同等的额外 rank 贡献。
- 既有 RRF v1 是明确的持久 binding profile，不能悄改其公式或复用原 profile 名称宣传新算法。后续所需 linear／其它策略应复用同一归并 owner，先固定全局主路径和对照输入，再检验完整记忆固定集；当前没有推广新策略或按 query 选择最佳模式。
- [036](../../036-memory-score-fusion/spec.md) 已在同一 Core fusion owner 实现显式 `linear-unit.v1`，旧 RRF default／序列化／行为不变；不同 profile 使用不同 serving-generation 身份。实际同义反例在新 profile 下 settings=1、weather=0.6508383，Candidate 仍未进入索引／结果；旧 RRF probe 继续失败并保留。
- 036 使用原 frozen fixture 的全部 10 memory documents/queries、同一 Light provider、实际 InMemoryRepository／原 typed projection 和既有 metrics；RRF 与 linear 的 NDCG@10 均 0.9130929754、Recall@10 均 1。两个 query 的相关记忆仍不是 Top-1，不能宣称整体排名提升或进一步调参。raw outputs 与评分见 `docs/evidence/code/2026-11-01-memory-score-fusion/native-memory/`。这是 Core-v2 ActiveRecall owner 的实际资格，尚不等于 legacy memory／ACP 产品路由接通或全 60 查询资格。

## QueryNode 结构核对范围

`query-owners.json` 是手写 owner 决策，不是生成文件。`crates/code/core/tests/xerj_query_inventory.rs` 用已有 syn 读取固定源码，核对完整 enum 的 44 个变体与每项字段顺序，并拒绝缺失、重复、unnamed 与无效声明。两项 native structural tests 已通过。没有用源码顶部的旧分组表或正则推定分母。

此结构检查不解析／执行 Xerj 的产品配置，也不实现运行时 QueryNode 或另一套查询引擎。所有 query 行为资格仍为 NOT_RUN；候选 owner 是现有 vec，embedding／hybrid facade 还须按 D04 收敛。未审计的支持类型、解析器、其它 surface 与机制分母继续保留，T004 不勾选完成。

## 每项收敛的退出门

1. 全调用者、公开 API、源数据与政策差异已核对；记录保留的唯一机制／owner。
2. 旧入口委托同一实现，重复算法／索引／刷新 owner 已删除或合并；没有镜像权威库。
3. 真实实现通过固定质量、来源、生命周期与故障反例；输入／grader／超时不变。
4. 发布构建测量证明冷／热、并发、CPU／内存／I/O 和取消；原生平台缺证据保持未通过。
