# Performance contract

## 1. 口径

下列新增数值是**拟定工程目标**，不是现有成绩。T005 在任何优化前，用发布构建冻结性能机、输入、样本数和以下 gate。若基线与目标差距过大，保留失败和工期影响，不抬高阈值换取通过。已有更严格的 gate 优先，测试超时也不放宽。

正确性、来源当前性和权限优先。加速不能以跳过来源复核、缩减既有查询分母、静默丢记录、改标注或扩大权限为代价。

## 2. 固定工作负载

| 档位 | 输入 | 目的 |
| --- | --- | --- |
| L0 | 512 个合成文本文件；无实际 LSP server／外部 MCP／账号探测 | 可重复拆解首次建会话和第一条检索。 |
| L1 | 5,000 文件；固定源字节与实际 chunk 数；真实 bundled provider | 冷建、热查、单文件改写、关闭重开。 |
| L2 | 沿用既有 25,000×384 exact-vector fixture；hybrid 沿既有 512-chunk fixture | 保留历史性能门，与真实模型质量分开。 |
| L3 | 约 25k／100k 实际 chunks 的有界 fixture；记实际 dimension／bytes | 观察规模曲线和现有限额；超过默认预算必须保留 partial／拒绝，不能偷偷提高产品上限。 |
| H8/H32 | 8／32 个宿主候选 owner，固定 publication、12 hits／48 KiB、现有 continuation | 测 fanout、冷 owner、覆盖率和 3 秒总预算。 |
| D | 固定 OOXML、文字 PDF、扫描 PDF、图像；page／node 范围固定 | 区分提取计算、缓存命中和 ACP transport。 |
| M | 5,000 条 memory，包含旧记录、同义正例、删除／无关负例 | 回填、热召回、重启及更新投影。 |

“文件数”“chunk 数”“向量数”不能混用。L1/L3 如因资格策略未收全，需要记录 eligible／indexed／failed／skipped，不能用较小的实际分母冒认目标规模。

## 3. 新目标与既有门

| 项目 | 拟定目标／必须保留的 gate | 测量范围 |
| --- | --- | --- |
| 首次可交互 | L0 model-free，process start → initialize → new_session response，p95 ≤1,500 ms | 不等待全文／语义 100% 覆盖；不得预热后称 cold。 |
| bundled cold 首会话 | 同流程，显式本地语义首次 load，p95 ≤3,000 ms | 单列 load、建 session、索引状态。 |
| 同进程第二会话 | L0，相同 workspace／另一个 workspace 分栏，p95 ≤500 ms | bundled model decode 次数增量 0，来源与 session 索引不串用。 |
| L1 语义建成 | model 已加载 → 当前 5,000 文件向量就绪，p95 ≤2,000 ms | 包括扫描／分块／embedding／publish；coverage 独立验证。 |
| 单路热查询 | L1／M，真实 provider，query-admit → result-complete，p95 ≤100 ms | 包含当前来源核验；不包含 LLM HTTP 与 UI render。 |
| 8 路查询 | L1，独立请求、固定 query 集，p95 ≤250 ms | 不通过不受限增加线程／内存换取通过。 |
| L3 单路查询 | 25k chunks p95 ≤100 ms；100k chunks p95 ≤400 ms | 真实 dimension 与现有限额内完整就绪的索引，包含来源校验。 |
| L3 8 路查询 | 25k chunks p95 ≤250 ms；100k chunks p95 ≤1,000 ms | 与单路同一查询集、相同覆盖；预算不足导致 partial 时不能计完整档通过。 |
| 原有 exact gate | 25,000×384 p95 ≤30 ms | `workspace_retrieval_benchmark.rs` 原夹具与原 grader。 |
| 原有 hybrid／rerank gate | p95 ≤100 ms；rerank 新增 p95 ≤10 ms、scratch ≤4 MiB、候选 ≤100 | 不改既有阈值；真实语义质量另测。 |
| H8 全热 owner 查询 | p95 ≤500 ms | 相同候选覆盖、顺序／归因与输出界限。 |
| H32 | 遵守已有 3 秒总预算，continuation／pending 正确；报告 p95 与覆盖率 | 不承诺一页查完所有 owner；拒绝减少查询分母换速度。 |
| 取消前台响应 | p95 ≤250 ms | 发送 `session/cancel` 通知至挂起的 `session/prompt` 返回 `stopReason=cancelled`；该通知没有 ACK。native settle 与 owner 清理另量，沿原期限。 |
| 同字节保存／热查 | 新文档 embedding 次数 =0；同进程 model decode 次数增量 =0 | Query embedding 可按原策略执行，权威来源 reread 仍允许且计数。 |
| 文档热读 | 相同 source digest／extractor／页范围命中后，新增原生提取次数 =0 | 全部回执内容／页数相同；OCR cold 记录 ms/page，不预设跨机器统一秒数。 |
| 资源生命周期 | session close 后该 session vector records=0；20 次开关后无 owner／队列持续增长 | bundled model 常驻单独列出；不要求 allocator 立即归还所有 RSS。 |

RSS 在同一机、同一热身档位测峰值与稳态；T005 冻结按平台的基线和噪声带。已有向量 128 MiB、catalog、候选、输出和 worker 限额保持。用 accounted bytes 判预算，用 RSS 观察进程成本，不能互相替代。

## 4. 测量规程

1. 同一 source／submodule 身份、相同 Rust features 和 `--release`；记录 CPU、内存、OS、arch、工具链和其它负载。性能机没有稳定性证据时标记 BLOCKED，普通共享 CI 不放脆弱 wall-clock 小断言。
2. cold 至少 20 次独立进程；warm 必须先等待目标来源代和所测通道完整 Ready，记录 coverage，再做 5 次热身和至少 100 次测量，固定 seed 打散相同 query 集。并发档为 1／4／8；不以逐轮改变输入／超时调参。partial／降级的查询单列，不能并入完整通道的热查询通过样本。
3. 各 profile 分开报告 p50／p95／max。阶段时钟用进程内单调 elapsed，跨进程以既有 request／session／run 关联；不相减不同 wall clock 伪造阶段耗时。
4. 把 init、model load、scan、chunk、embedding queue／compute、index publish、query embed／retrieve／rerank／source verify、host RPC 和 render 分开。只添加解释实际瓶颈所需字段，不建新的遥测平台；stdout 保持 ACP wire。
5. 不删最慢样本；超时、失败、partial 均保留。有效样本不足时不计算“通过”的 percentile。LLM HTTP／OCR 计算有单独范围，不能通过排除主要成本冒称整体更快。
6. 每次仅改变已定位的路径，并同时跑质量与资源对照；扩大测试只由新变更／失败／未决风险触发。

## 5. ANN／跨会话缓存决策

T028 只形成结论，不在本 epic 预置第二个后端。

- ANN：需要 L3 在既有限额内未达到上表对应规模／并发的 p95 门，并证明 retrieval scan 占主要开销；候选方案须保持过滤／revision／取消，且对 exact oracle recall@10 达到仓库已有 0.98 下限，才可提出后续实现。若该规模因原有资源预算无法完整 admitted，单列容量结果，不冒认为查询性能超时。
- 跨会话向量缓存：需要“相同资料反复重开”的重新 embedding 为主要用户开销，且能复用现有 owner／generation／model identity；先报告节省与存储／失效代价，再立独立 slice。
- 目标已满足时，结论可以是保持精确扫描和现有生命周期。没有 backend 变更也可以完成性能优化任务。
