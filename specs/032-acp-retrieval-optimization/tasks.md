# Tasks: ACP 检索闭环与性能优化

**Input**: [spec.md](spec.md)、[plan.md](plan.md)、[research.md](research.md)、[contracts](contracts/qualification.md)。

**Status**: Implementing；任务完成状态以勾选与执行 evidence 为准。旧 provider cache slice 已完成，不计为本期任务完成。每个行为任务的测试与实现同批；测到已满足的路径保留实现，以证据完成任务。未安排多 agent 或新的 worktree。

## P0 — 测量可信（5 项）

- [x] **T001 [US1/US4] 能力与源码基线。** 核对 ACP `host_context.rs`／`agent/session_open.rs`、Core `tools/builtin/mod.rs`、Studio `agent_plane/turns.rs`，固定 C0/C1/H1、36 个 Q-ID、owner／handler 和实际 source／submodule／binary 身份。沿 tests/CI 的原始 guard 做先验记录，不能以 keyword 存在判断能力完成。产物为执行 evidence 的 inventory；覆盖 FR-001、FR-004、FR-014。
- [x] **T002 [US4] 真实 engine 路径。** 修 `tests/retrieval_bench/run_quality.py` 的 `--engine` 只记标签问题；在 ACP 新 `tests/retrieval_qualification_stdio.rs` 复用 actual-process helper，批量导出真实排名，Python 只调用既有指标。新 `tests/retrieval_bench/test_engine.py` 证明实际 executable 被执行、坏路径／坏协议失败、stand-in 不能充产品成绩；同时用实际 ACP binary 跑正例。依赖 T001；覆盖 FR-003、Q01/Q03/Q31。
- [x] **T003 [US4] 冻结质量集。** 保留 `tests/retrieval_bench/mini_corpus.py` 和旧 graders，新建 ACP `tests/fixtures/retrieval_qualification_v1.json`（至少 60 条、六类各 10 条），在 T002 target 验证 source hash、标注范围／ID、重复排名处理及指标 golden controls。先冻结匹配语义／限额／输入再比较产品；不能按结果改标注。依赖 T002；覆盖 FR-003/FR-011、Q31/Q32。
- [ ] **T004 [US2/US4] 阶段与资源观测。** 在 ACP bin 配置加载、`agent/{protocol,session_open}.rs` 与 Core 原 retrieval／embedding 指标补必要 structured tracing；宿主 query 用原请求 ID 关联。复用 `log_file` 和原 metrics，不建遥测框架。测试确认阶段因果、缺失值不记 0、stdout 无日志、无正文／凭证泄漏，资源计数匹配已知 fixture。依赖 T001；覆盖 FR-004、Q34。
- [ ] **T005 [US2/US4] 发布基线和 gate 冻结。** 用原 release examples + T002 实际链路跑 L0/L1/L2/H8/H32/M 的适用档，记录性能机、20 cold／100 warm 样本与基线问题；L3 先确认现有限额。将 contracts/performance 中拟定目标与来源固定到 attempt manifest；已有 gate 不改。以原始 samples 复算汇总并核对 count／unit，而非单次最大值。依赖 T003/T004；覆盖 FR-004/FR-011、Q34。

## P1 — ACP 调用可达（3 项）

- [ ] **T006 [US1] 声明／路由矩阵。** 对 ACP `host_context.rs`、`agent/session_open.rs`、`document_read.rs` 和 Core `SearchTool::definition` 增加 profile contract cases：absent／false／true、bridge 缺席、restricted review、degraded semantic。复用现有闭集，无缺口时只补测试；SAG 原文旅程实测不通才冻结候选 bridge。依赖 T001/T003；覆盖 FR-001/FR-002、Q02/Q04。
- [ ] **T007 [US1] Portable 实际搜索。** 在 T002 新 stdio target 运行 C0/C1 的 BM25／hybrid／semantic，保留已有 `shipped_tui_e2e/retrieval.rs` 的证据 oracle。固定 query／源范围、真实 provider、降级结果与工具列表一致；无 shell helper／cfg(unix) 依赖。每条实际 tool evidence 必须进入后续模型 transport 或 host-extension 回执。现有 workspace extension 固定 hybrid；BM25／纯 semantic 复用并移植既有 shipped stdio 的本地脚本模型工具调用路径，不能给 extension 贴上另一种模式标签或仅为基准扩大生产协议。依赖 T002/T006；覆盖 FR-001/FR-005、Q01–Q03/Q06。
- [ ] **T008 [US1] 宿主拒绝和生命周期。** 在 ACP `document_read.rs`／memory-search／wiki-search sibling tests 与 Studio `agent_plane/turns.rs` 相关 tests 覆盖缺 handler、deny、host disconnect、timeout、cancel、迟到回执。任何修复只落原 adapter／owner，并实际 stdio 重现；验证后端未执行及现有 owner 清理。依赖 T006；覆盖 FR-002/FR-007/FR-008、Q04/Q05/Q20/Q28。

## P2 — ACP 运行开销（4 项）

- [ ] **T009 [US2] 冷启动关键路径。** 根据 T005 的归因修改 ACP `agent/{protocol,session_open}.rs` 或定位到的 bin/Core 前置，保留已有 `local_embedding.rs` 缓存。必需 readiness 不跳过，非必需索引沿现有后台方式。前后跑 L0/C0/C1、同进程第二会话与两个不同 workspace，Q30 验证共享模型和独立索引；无实际瓶颈不重构。依赖 T005/T007；覆盖 FR-004/FR-006、Q30。
- [ ] **T010 [US2] 编码 executor 响应性。** 对 `crates/embedding/src/adapter.rs`／provider 与 Core `embedding/executor.rs` 测实际 bounded batch 占用；确认影响响应后复用现有 blocking owner／batch 边界取消模式。测试真实编码与计时 barrier：cancel 通知至挂起 prompt 返回 cancelled、实际 worker settle、排队界限、次序／向量 shape 均有效；保留 factory 和 API。依赖 T005；覆盖 FR-007、Q27。
- [ ] **T011 [US2] 增量／重复工作。** 核对 Core `workspace/retrieval/{runtime,persistent,semantic_projection}.rs`；以 provider input／source read 计数证明 repeated-query、同字节保存、edit／rename／delete 的工作量。扩展既有 `persistent/tests.rs`、`tests/lifecycle.rs`；只修已复现的增量／失效根因，不去掉 source verify。依赖 T005；覆盖 FR-005/FR-006、Q07–Q11。
- [ ] **T012 [US1/US2] 更新与取消闭环回归。** Core retrieval sibling tests、ACP `agent/retrieval_seam_tests.rs` 和 portable stdio target 组合验证 Q06–Q12/Q27/Q30：真实改源、构建中查询、关闭与迟到结果。检查旧 publication 数为 0、phase 准确、另一会话不受影响；新行为在原 owner 同批修。依赖 T009/T010/T011；覆盖 FR-005/FR-006/FR-007。

## P3 — 宿主检索闭环（8 项）

- [ ] **T013 [US1] Wiki／资料 publication。** Studio `agent_plane/workspace_wiki_search.rs`、binding/lifetime tests 与 ACP `wiki_search` host tests；通过实际 route 验 current／all、资料库 publication、真实 binding 变化、正在维护的部分覆盖。原 publication 和 source owner 不复制到 ACP。依赖 T006/T008；覆盖 FR-005/FR-008、Q13/Q14/Q29。
- [ ] **T014 [US1] OOXML 原生读取。** 复用 ACP `office_read.rs`／sibling tests 与 Studio `agent_plane/workspace_wiki_documents.rs`，实际 DOCX/XLSX/PPTX fixture 核对段落／单元格类型／公式／slide anchor、digest、next_node_path、输出限额。透过实际 ACP 读取，源字节不变；不能只检查字符串出现。依赖 T007/T008；覆盖 FR-005/FR-008、Q16。
- [ ] **T015 [US1] PDF／OCR 与提取复用。** 在 Studio 原 `document_processing`、`agent_plane/document_read.rs` 与实际 native test target 验文字层 PDF、扫描 PDF、图像及范围／下一页。区分 `workspace_document_stdio` probe stub；同 digest／extractor／页范围 extraction 次数增量为 0，改源需失效。缺模型／native companion 记 BLOCKED 正向资格；拒绝分支另验。依赖 T008/T014；覆盖 FR-005/FR-007/FR-008、Q17/Q18/Q20。
- [ ] **T016 [US1] SAG 原文到 Agent。** 使用 Studio `local_documents/library.rs`／SAG 原 index 测 Q19；若 T006 确认缺桥，按 host-operations 候选最小接口改 ACP `host_context.rs`／`document_read.rs`／动态工具注册和 Studio `agent_plane/turns.rs` 分派，增加 sibling＋实际 route tests。无缺桥不加工具；不迁移入库编排、不二次索引。依赖 T006/T008/T013；覆盖 FR-001/FR-002/FR-005/FR-008、Q04/Q05/Q19/Q20/Q29。
- [ ] **T017 [US1/US2] Wiki fanout 热路径。** 在 Studio `workspace_wiki_search.rs` 测 H8/H32 的每 owner 成本；只对证实瓶颈使用现有有界并发或热 publication 复用。测试 3 秒总预算、12 hits／48 KiB、独立 relevance score 归并、pending 和 cursor 无跳项／重复。T020 对照实际回执，性能已达门可只记录。依赖 T005/T013；覆盖 FR-005/FR-008、Q14。
- [ ] **T018 [US1/US2] Memory owner fanout。** Studio `workspace_memory_search.rs`／`local_memory.rs` 与 sibling tests 测每 owner／layer 调用；复用原活跃 owner，不恢复关闭任务。优化后验证相同 scope／候选、原记忆层归因、当前 revision、预算和 continuation。依赖 T005/T008；覆盖 FR-005/FR-008、Q15。
- [ ] **T019 [US2] 资料 scan／锁与热读。** 对 Studio `local_documents/library.rs` 的 scan、sync、`with_index` map lock 分别计量；仅在证明重复扫描／锁等待后复用现有有效 invalidation／snapshot、缩短锁内无关工作。sibling tests 覆盖外部文件修改、并发读取、library switch 和 owner 关闭；不得以陈旧缓存换速度。依赖 T005/T016；覆盖 FR-005/FR-006/FR-008、Q35。
- [ ] **T020 [US1/US2] 实际 ACP-to-host 旅程。** 在现有 ACP host fixture／Studio 原生 route target 接通 T013–T019 的正反例，校验 request/run 归属、原文／页／memory 引用及 cancel 清理。至少对获准 Wiki、memory、OOXML、PDF/OCR、SAG 各有实际端到端记录；环境缺失保留 BLOCKED，不用 stub 补位。依赖 T013–T019；覆盖 FR-005/FR-007/FR-008、Q13–Q20/Q28/Q29。

## P4 — 本地记忆语义（5 项）

- [ ] **T021 [US3] 兼容与显式准入决策。** 对照 Core `durable_memory_vec.rs`、`durable_memory/{semantic,semantic_binding,semantic_refresh}.rs`、`memory/semantic_refresh.rs`、`agent_api/session_options.rs`。保留原权威记录与 activation；冻结唯一共享机制与有界投影 owner；原接口不兼容时先收敛共享原语，薄 adapter 保持 API 和政策，不新写类似投影／刷新／排序流程。优先已有 typed policy，无入口才按 contracts 的最小 trusted ACL 候选改 Core `config/{mod,loader,acl_render}.rs`／`memory.rs` 与 ACP caller；同批默认关闭、render 往返、workspace 不能启用的测试。依赖 T003/T006；覆盖 FR-002/FR-009/FR-010、Q21。
- [ ] **T022 [US3] 存量与增量投影。** 在 T021 选定的 Core 原 owner 实现有界旧记录回填，复用已有 provider／refresh／vector 边界；add／update／delete／prune 同步当前派生状态。临时 store tests 使用真实 light provider 对同义正例和无关负例核验 ID／revision，不复制召回逻辑。依赖 T021/T010；覆盖 FR-009、Q22/Q24。
- [ ] **T023 [US3] 重启与失效恢复。** 在同一 memory owner 加验证／重建；模型／dimension／源 revision 变化不复用错误向量，部分失败与只读锁不改原数据。既有 schema 兼容、旧零 embedding 列、取消回填与重开测试同批；默认不加持久缓存 schema，确需变更仅在 owner 定义版本与恢复。依赖 T022；覆盖 FR-005/FR-007/FR-009、Q23–Q26。
- [ ] **T024 [US3] ACP 与宿主记忆接线。** ACP `agent/session_open.rs`／`agent/memory_management.rs`、Core MemoryStore search 与 Studio `local_memory.rs`／`workspace_memory_search.rs` 使用同一实际 memory owner 的召回，不创建镜像库。测试当前政策、scope／layer、语义通道状态和回退；查询管理与 Agent recall 不误认为同一接口已自动生效。依赖 T022/T023/T018；覆盖 FR-009/FR-010、Q22。
- [ ] **T025 [US3] 完整记忆资格。** 扩展 Core memory sibling tests、ACP `tests/memory_management.rs` 与真实 host chain，对 Q21–Q26 独立运行，并通过 T002 固定 quality oracle 与 M 档性能。关闭语义回到原词法；存量数据、生命周期与读写锁行为不变。依赖 T024；覆盖 FR-009/FR-010、Q21–Q26。

## P5 — 质量与规模决策（3 项）

- [ ] **T026 [US4] 真实语义质量／消融。** T002 实际 runner + 60-query fixture 比较事前固定主路径及 BM25／hybrid／semantic 消融，真实 provider／control 分栏；用 `tests/retrieval_bench/metrics.py` 复算，保留每类 NDCG／Recall／MRR／false positive、当前引用与输出 bytes。工作区子资格依赖 T003/T007，并随 T009–T012 复测；宿主资料子资格依赖 T020；记忆子资格依赖 T025。各自执行，整体完成要求三者全部完成。原 grader 不改，失败先复现再定位。覆盖 FR-003/FR-005/FR-011、Q32/Q33。
- [ ] **T027 [US4] RRF／MMR 选择。** 用 Core 原 `workspace/retrieval/rerank.rs`、`hybrid_types.rs`、examples 与 T026 固定输入做策略对照；只有质量收益且既有 exact／资源／latency gate 均过，才使用现有 opt-in 推广。保留同次 owning tests；无优势就保留默认。仅依赖 T026 的工作区子资格，不等待无关 OCR／记忆功能；覆盖 FR-011/FR-012、Q33。
- [ ] **T028 [US2/US4] 规模与后续决策。** 原 release benchmark、persistent production example、真实 ACP runner 跑 L1/L2/L3、1／4／8 并发、20 次 owner 开关。对 exact scan、重复 embedding、source verify、host RPC 分别归因；按已冻结 L3 规模／并发门输出 ANN／跨会话缓存 go/no-go 与成本。工作区部分依赖 T005/T012/T027，宿主部分依赖 T019，记忆部分依赖 T025；独立执行，整体完成仍要求全部适用部分。这里只决策，不提前上线新后端；不得提高限额或减分母。覆盖 FR-004/FR-012、Q34/Q35。

## P6 — 整体验收与交付（4 项）

- [ ] **T029 [US4] 变更审查与根 guard。** 对实际变更做公开 API／typed error／panic／blocking／平台检查；运行 owning features 的 Clippy `--all-targets --no-deps`、fmt、dependency／runtime-boundary guards。新源与测试分 sibling；既有失败单列，不扩大 allowlist／skip 或改 grader。依赖实施 tasks；覆盖 FR-013、Q36。
- [ ] **T030 [US1/US4] 原生平台与宿主 tenant。** 复用现有 macOS／Linux qualification 和 Windows 原生 runner；portable ACP 正反例覆盖空格／非 ASCII 路径。Studio 受影响模块分别 `tenant-aster`／`tenant-toyou`，源码只在私有 submodule；probe 和真实 native tests 分列。跨编译与 cfg(unix) skip 不计 native PASS。依赖 T029；覆盖 FR-013、Q36。
- [ ] **T031 [US1/US2/US4] 冻结发布复测与用户入口。** 同一最终 source/binary 身份重跑对应 36-case matrix、固定质量／性能门，构建 `aster`／`aster-acp` 配对；沿既有 TUI／Studio 测试入口各完成一条实际检索／来源／取消旅程。只重复变更影响或未决部分；不启动发布、安装或付费模型。依赖 T028/T030；覆盖 FR-011/FR-013、Q34–Q36。
- [ ] **T032 [US4] 证据与后续账本。** 写 `docs/evidence/code/<execution-date>-acp-retrieval-optimization/OUTCOME.md` 和逐 attempt logs／manifest；重新打开核对 Q-ID、计数、单位、来源、原文引用、source hashes。明确 PASS／FAIL／BLOCKED／NOT_RUN、旧失败、未测平台和 ANN／缓存决策。父仓库／Studio 产物分开，不 stage／commit／release；旧记录不覆盖。依赖 T031；覆盖 FR-014、Q36。

## Requirement → task coverage

| Requirement | Producing tasks |
| --- | --- |
| FR-001 | T001、T006、T007、T016 |
| FR-002 | T006、T008、T016、T021 |
| FR-003 | T002、T003、T026 |
| FR-004 | T001、T004、T005、T009、T028 |
| FR-005 | T007、T011–T020、T023、T026 |
| FR-006 | T009、T011、T012、T019 |
| FR-007 | T008、T010、T012、T015、T020、T023 |
| FR-008 | T008、T013–T020 |
| FR-009 | T021–T025 |
| FR-010 | T021、T024、T025 |
| FR-011 | T003、T005、T026、T027、T031 |
| FR-012 | T027、T028 |
| FR-013 | T029、T030、T031 |
| FR-014 | T001、T032 |
| FR-015 | T021–T025、T027、T029 |

## Final Verification Gate

- [ ] Owning package focused tests通过，新行为与其测试同批；T029/T031。
- [ ] Rust 格式与 Clippy 检查通过；若既有全仓失败，保留原失败及本批 scoped 检查，不宣称全绿；T029。
- [ ] 无新 crates.io `aster-*`、新 workspace、lockfile、SDK 或未经许可的 worktree；T001/T029。
- [ ] 无新增 native `<select>`，没有权限／allowlist／未知值容忍范围扩大；T006/T029。
- [ ] 产品配置如确需新增，只用 trusted ACL，旧字段／默认／层级兼容有测试；T021。
- [ ] 真 engine／provider／宿主路径可辨，输入／grader／预算冻结且原始结果可复算；T002–T005/T026/T031。
- [ ] 每个已声明的正向能力通过对应 matrix case；宿主缺席／环境 blocked 不冒充功能通过；T030–T032。
- [ ] CLI／ACP 同一来源配对构建；Studio 源码保持子模块边界；T030/T031。
- [ ] Evidence 重新打开核验计数、单位、source／binary identity，非本任务改动没有混入提交；T032。

## Start gate

规划允许从 T001 开始，不需要新的用户事实。实现 P2 需要 T005 的真实归因；T016 的新协议只在缺桥已证实时进入；P4 需要 T021 的兼容决策；ANN／缓存生产实现不在本任务清单内。不得将这些前置视为已通过。
