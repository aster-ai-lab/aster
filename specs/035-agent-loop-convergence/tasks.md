# Tasks: 自主循环与长程任务去重机制融合

**Input**: [spec.md](spec.md)、[plan.md](plan.md)、[research.md](research.md)。
**Status**: FOLLOWUP_EVALUATED_WITH_LIMITS。T001–T013首轮结果保留；T014–T017为当前目标，未通过门不勾选。
**Prerequisites**: Constitution Check 无 VIOLATES；当前 checkout 的在途改动清单已知。

每项行为变更在同一任务提交范围内携带 owning-package hermetic test。以下任务默认串行，不标记 `[P]`；不需要另建 agent、工作区、配置或持久化账本。

## Phase 0 — 冻结和复现（2 项）

- [x] **T001 [US1–US4] 冻结真实基线和调用图。**
  - 范围：`crates/effect/src/coding.rs` 及全部 `impl ToolRunner`；Core `fact_control/`、`agent_api/conversation_runtime/`；ACP `unattended_e2e.rs`。
  - 记录 HEAD、相关文件内容哈希、未提交 diff、toolchain、OS/架构、features、现有 Cargo lock 等待。禁止只以 HEAD 代表 dirty source。
  - 确认父任务、ChildRun、恢复、批次和 generic runner 的准备/终态路径。列出 trait 方法、结构体字面量、公开 re-export 和旧日志读取者。
  - 在改生产代码前运行相关原测试，并保存固定测量输入/基线结果或可信基线二进制。没有可用构建时记录原因，不复用身份不明的旧产物。
  - 完成证据：reviewed-path 清单、baseline 命令和计数、三项复现的环境。依赖：无。要求：FR-001/FR-007/FR-008/FR-011。

- [x] **T002 [US1–US3] 为 R1–R3 建立真实 red 或反证。**
  - 在 `crates/effect/src/coding/convergence_tests.rs`、Core `fact_control/receipts/tests/convergence_preparation.rs`、`agent_api/tests/fact_log/convergence.rs` 新增最小用例，分别注册到原 sibling module。
  - R1 必须经过 effect 调度调用真实 Core 准备 adapter；证明钩子 canary 与回执是否冲突。只直调 `prepare_tools` 不足以证明覆盖缺陷。
  - R2 必须记录压缩实际发生、保留工具对数量、实际工具/模型次数和终态；外层 watchdog 只防测试挂死，不充当产品停止证据。
  - R3 使用同参数 7 个新观察和固定观察负对照；不得复制检测逻辑到测试。
  - red 必须是目标断言失败，编译错误/构建锁/缺少 sandbox 不算产品 red。被推翻的项记录证据并撤销对应修复范围。
  - 同时确认兼容接线方案：是否能复用现有入口；若需默认 trait 方法扩展，列明所有 adapter 影响，不改必填公开字段。
  - 完成证据：R1/R2/R3 各自的真实结果与接口决策。依赖：T001。覆盖：C02/C07/C10；要求：FR-001。

## Phase 1 — 回执保真与兼容接线（2 项）

- [x] **T003 [US1] 在原准备路径修复重复拒绝。**
  - 修改 `crates/effect/src/coding.rs::prepare_tool_batch` 和 Core `fact_control/receipts.rs::prepare_tools`、`executor_tools.rs` 的必要接线。
  - 重复拒绝已确定时，沿宿主准备流程生成绑定原调用的拒绝；禁止先执行钩子再直接覆盖完整空效果。
  - 已经产生的 hook denial、schema failure、policy deny、取消和不确定效果保持原义。不得改写为“工具未执行所以一定无副作用”。
  - 同任务通过 C01/C02/C03；保留 `argument_validation`、`execution_availability`、`permission_diagnostics` 的原测试。
  - 完成证据：目标 red 转 green、hook/tool 次数、原始回执和完成门结果。依赖：T002。要求：FR-002/FR-006/FR-008。

- [x] **T004 [US1] 验证全部 adapter、批次和旧调用兼容。**
  - 核对 `crates/effect/src/lib.rs` 的显式导出、全部 `ToolRunner` 实现，以及 Core `fact_control/receipts/tests.rs` 的 request 构造。
  - 如 T003 新增默认 trait 方法，使用只实现原必需方法的 runner、原 request/result 字面量及原调用形式编译并运行。测试正常路径仍调用原 prepare。
  - 在 `coding/convergence_tests.rs` 与 Core sibling tests 验证钩子参数改写、重新批准、准备/执行取消及同批重复。
  - 拒绝路径不跳过调用身份绑定；不引入 effect → Core 依赖，不从错误文字判断分支。
  - 完成证据：C04/C05/C16 的相关断言、adapter 清单全部有结论。依赖：T003。要求：FR-002/FR-007/FR-008。

## Phase 2 — 统计与新信息融合（3 项）

- [x] **T005 [US2] 让原去重统计跨压缩和重放保持。**
  - 在 `crates/effect/src/coding.rs` 的现有 scheduler/fold 维护可重建统计，替换单纯依赖 retained messages 的次数计算。
  - `compaction.done` 不刷新额度；真实用户输入和现有进展重置规则保持。不得加入第二份 store、后台任务或每次调用完整重扫的新检测器。
  - 旧日志缺少新增可选数据时使用明确兼容规则；未知观察不计新进展，不改变原文件的历史含义。
  - 在 `coding/convergence_tests.rs`、原 `revalidation_tests.rs` 和 Core `fact_log/convergence.rs` 同批通过 C06/C07/C09。比较连续执行与从相同事实重放的输出。
  - 完成证据：实际压缩、阈值、生效区间、调用数和终态对照。依赖：T004。要求：FR-003/FR-008。

- [x] **T006 [US3] 将现有新信息判据接入工具准入。**
  - 读取并复用 `fact_control/receipts/novelty.rs::information_identity`；Core 继续拥有结果封装解析。若需共享纯比较原语，只放到 `crates/effect/src/coding/tool_effects.rs` 的通用层。
  - 在原准备/回执接线传递可信观察，沿用原 threshold；不让 effect 解析 Core 私有封装，不复制第二套观察哈希计算。
  - 同参数持续产生新观察时允许预算内继续。只改变调用 ID、耗时、宿主提示或工具自报 metadata 时不刷新额度。
  - 同任务通过 C08/C10/C11/C12/C13，保留原 `receipts/tests/novel_information.rs` 和 `fact_log/revalidation.rs`。
  - 完成证据：7 次新观察全部实际执行；稳定结果有界；JSON/图像与 outcome 边界有断言。依赖：T005。要求：FR-004/FR-005/FR-007。

- [x] **T007 [US2–US4] 接通父/子事实路径并清理重复决策。**
  - 复查 `fact_control.rs`、`receipts/refusal.rs`、`history_seed.rs`、`agent_api/conversation_runtime/owned_fact.rs`、`child_run.rs`。
  - 展示提示只能消费既有事实/投影，不能拥有另一套停止额度。JSON 可解析性、工具消息信任和内部哈希不进入模型上下文的约束保持。
  - 保留拒绝循环与 A/B 工作区状态循环的不同责任；不删除它们来追求表面代码合并。旧循环的公开兼容类型也不随本任务清理。
  - 同任务通过 C14/C15/C16/C17，并运行父/子已有 refusal、mutation、productive 回归。
  - 完成证据：真实路径图与 scoped diff 核对，零新 recovery controller/配置/持久化表；实际模型和工具调用可解释。依赖：T006。要求：FR-006/FR-009。

## Phase 3 — 自主循环与实际入口（2 项）

- [x] **T008 [US4] 验证 Goal 额度、等待和恢复的原契约。**
  - 运行并仅为缺口补充 `agent_api/goal_facade/tests/execution/{refusal,novelty,automatic,waits}.rs` 及原 child/recovery tests。
  - C18：相同观察跨新 Run 不算新进展，Memory/File session store 重开后仍停滞。
  - C19/C20：真正接受的步骤可推进；已接受新计划只获得原有一次预算内机会。
  - C21：Unknown/Incomplete、宿主等待、硬预算、deadline 和取消不扩权、不盲目重放，也不伪报 Goal 完成。
  - 如接线修复必须改 `goal_evidence.rs` 或 continuation，行为改变与对应 hermetic test 同批；禁止新增恢复额度。
  - 完成证据：实际 slice admissions、settlements、provider 调用和完成事实。依赖：T007。要求：FR-006/FR-009。

- [x] **T009 [US4] 通过真实 ACP stdio 验证接线。**
  - 复用 `crates/code/acp/tests/unattended_e2e.rs` 和 `long_horizon` 本地受控模型/fixture，在 sibling 文件补 C22 的强制压缩与持续新观察对照。
  - 使用当前源构建的 ACP；相关 CLI build 同时构建 `aster` 与 `aster-acp`。记录二进制身份，禁止用旧程序冒充新实现。
  - 断言实际模型请求、工具副作用、压缩、拒绝码、最终交付、Goal 未完成和无伪成功 End；原 state/refusal cases 保持通过。
  - 新的测试控制分支有测试侧自证；不修改既有冻结 grader/任务/超时。不具备后端条件时记录环境阻塞。
  - 完成证据：C21/C22 的原始 wire/receipt/产物，`acp_conformance_stdio` 结果。依赖：T008。要求：FR-009/FR-011。

## Phase 4 — 消耗和工程验证（2 项）

- [x] **T010 [US4] 执行固定输入的消耗对照。**
  - 复用 T001 基线和现有 Core/ACP harness；按 plan 的五类 workload 比较，正向长轨迹覆盖 50/200/500 档。
  - 报告完成/失败/未知、实际模型/工具次数、压缩次数、provider usage、重复上下文及耗时。缺失 usage 保持未知，假模型 token 不当作实际费用。
  - 正向产物必须相同；失败不能从消耗报告删除。计量新增逻辑若有，附计数/缺失 usage 的 hermetic self-check。
  - 真实模型实验与受控测量分开。未运行时记 NOT_RUN；如运行，先冻结独立压缩 cohort，复用原配对流程，不改旧任务和原配置取得通过。
  - 完成证据：逐任务 baseline/candidate 结果、输入/二进制身份、失败与环境归因。依赖：T009。要求：FR-010/FR-011。

- [x] **T011 [US1–US4] 完成格式、lint、feature 和原生平台检查。**
  - 执行 plan 的 owning-package tests、`cargo fmt --all -- --check`、effect/Core/ACP 的 `clippy --all-targets --no-deps` 和 `tests/workspace_dependencies.py`。
  - Core 覆盖 default、minimal；advanced-harness 做兼容编译及受影响支持组合的 lint。不假设两个 crate 已继承 workspace lints，逐项检查新 Rust。
  - 用 `--list` 核对 filters 命中，0 tests 不算 PASS。全批失败先 SOLO 复现，不加 lint allowlist 或放宽冻结检查。
  - 如新 tests 未进入现有 CI，接入 `.github/workflows/qualify-code.yml` 的原 macOS/Linux/Windows jobs，不新建 pipeline。
  - 完成证据：命令、测试数、features、原生 OS/架构和未测组合；交叉编译不代替原生运行。依赖：T010。要求：FR-007/FR-008/FR-009。

## Phase 5 — 证据和交付复核（1 项）

- [x] **T012 [US1–US4] 复开代码、测试和证据后交付。**
  - 重读真实 callers、默认 adapter、Core override、状态投影与拒绝/完成门；核对 FR-001–FR-011、C01–C22 和全部任务的实际结果。
  - 在 `docs/evidence/code/2026-11-01-agent-loop-convergence/OUTCOME.md` 写 Result、What landed、实际命令/计数、推翻项、既有失败、精确 follow-ups 和 Explicitly not claimed。
  - 检查 source/binary/input 身份、首轮与重试分离、受影响文件 diff；任何未通过/未测项继续保持未完成或注明范围，不能用本规划替代验收。
  - 不自动提交。只有用户请求提交时，逐文件 stage 并核对 `git diff --cached --name-only`，不包含其它任务改动。
  - 完成证据：可复开的 OUTCOME 与原始 artifacts 索引。依赖：T011。要求：FR-001–FR-011。

## Dependencies

```text
T001 → T002 → T003 → T004 → T005 → T006 → T007 → T008 → T009 → T010 → T011 → T012
```

R1/R2/R3 若被推翻，删除对应无效实现分支，保留有效的回归和调查结论；不绕过其它任务的回执、兼容或测量门。

T011已执行全部本机检查并记录失败/未测；勾选表示检查执行完成，不表示所有检查通过。T012/T013报告和逐attempt评估已交付。接受门的未通过项保持未勾选，见OUTCOME精确后续。

## Final Verification Gate

- [x] R1–R3 各自有真实复现或反证，不能把编译/环境故障算产品 red。
- [ ] C01–C22 均有对应断言和实际结果；缺口未冒充已覆盖。
- [ ] effect/Core/ACP owning-package focused tests 通过；旧实现的必要正向行为保持。
- [ ] `cargo fmt --all -- --check` 与拥有者支持 features 的 clippy 完成。
- [x] 公开 Rust 调用、默认 adapter、旧日志与显式 exports 兼容。
- [x] 零独立恢复状态机、额外恢复额度、第二份账本、命令词表或额外模型裁判。
- [x] 无新 crates.io `aster-*` 依赖；产品配置如有必须 ACL，本规格首轮不新增配置。
- [x] 无产品 UI/native select、workspace 迁移、SDK 或私有 Studio 源码副本。
- [x] 原生平台与未测组合明确，现有冻结任务/grader/模型/超时保持。
- [x] 实际消耗与任务正确性分别报告；NOT_RUN、Unknown、失败不伪报收益。
- [x] `OUTCOME.md` 与源、日志、产物相符，未来训练工作不计入本轮已完成。

## 用户追加：GLM 真实任务验收

- [x] **T013 依 real-llm-plan.md 执行并评估。** 固定 `glm-01/glm-5.3`；保留所有首次尝试和中断。校准六轮任务、五个原 Rust seeds、机制覆盖分别报告；不能把未触发的压缩/新观察分支记为真实模型已覆盖，也不能声称没有 baseline pair 的相对费用收益。


执行结论：真实GLM G1/G2完整覆盖未取得，G3/G5未运行；不将T013的执行完成解释为所有规划cohort资格通过。下一轮资格工作从现有失败、未覆盖项与同模型固定输入出发。


## 用户授权的优化续篇（串行执行）

- [x] **T014** 冻结当前源，按续篇US5补真实LiveCompletion red，复用原投影修验证反馈，覆盖压缩、二次编辑、当前验证通过/未覆盖、图片及原始事实不变。
- [x] **T015** 复核恢复/截断路径，扩展原有界relay的原生Messages usage/stop reason观察与正负自测；不增加模型调用额度或放宽契约。
- [x] **T016** 固定GLM与原五个Rust seeds、单Run cohort和grader，分别保留首轮、失败、身份、实际压缩/新观察、完整流程/代码正确率及请求/时间/usage；用各自可信冻结pair作优化前参考，不以改预算求通过。未合格项继续修复/定位，不能缩小目标。
- [x] **T017** owning-package tests、default/minimal/advanced受影响组合、all-target clippy、fmt、依赖边界及scoped adversarial review；逐条完成审计并写本轮OUTCOME，未证明的要求保持未完成。

- [x] **T018 实测失败归因修复。** 保留原有ForcedStop类型，在完成门拒绝时同时保留截断原因与原验证/未知副作用债务；不把截断转成功，不增加恢复调用。先测试真实gate_surface_result red，保留typed downcast、原错误前缀、失败事件及未追加完成事实；按原输入/验收器独立注册复验，不覆盖首轮2404失败。

- [x] **T019 实测压缩耗尽修复。** 保留单Run首轮15请求/6次实际压缩/其中5次摘要max_tokens失败证据；在原摘要路径明确只整理既有事实，不代做任务；两个入口共用原typed截断判据，不接纳非空截断摘要，fact路径仍沿原确定性fallback且不增调用。真实red/green覆盖两入口；保留原7版本、4K cap、grader和预算独立复测。

- [x] **T020 独立容量归因诊断。** 原4K最终cohort完整保留（Rust5/5、单Run机制7/7但最终截断）；仅测试适配器新增8192输出条件，模型/原请求/336记录/原grade/超时不变。先逐字节验证只改入口参数与事实额度记录，再独立冻结pair+test并记录身份。至多1次新条件尝试，报告任务/流程/机制和实际usage，不计入原4K通过、不宣称整个共享checkout隔离因果；生产Core不因本项增加恢复/配置/状态机制。

- [x] **T021 现有上下文预算独立对照。** 16K/8K原case报告完全正确但600秒超时，8次压缩；普通前缀约15K已接近/超过16K的80%触发线。只把新条件的现有context配置改为32768，保留8192输出/40请求/原600s及1800s/原模型数据grader及>=2真实压缩门。原16K两条件不改、不替换结果。先逐字节audit适配器差异，冻结pair/test，同样只1attempt；报告完整结果后不再扩展条件或增加生产控制机制。

## 本轮执行完成与资格状态（不等同）

T014–T021的实现、受控检查、固定真实尝试和评估均执行完成。勾选表示执行/定位及证据交付完成，不把失败条件改判通过：Rust固定五例5/5；长数据16K/4K最终截断，16K/8K产物正确但600秒超时，32K/8K正常结束但第7批小计算错。三组实际机制门均触发，完整任务资格均未取得。

- [ ] 长数据报告正确、当前绑定、正常结束和所有保护/机制门在同一条件下同时合格。
- [ ] 全工作区格式及分组入口稳定性全部通过。
- [ ] Windows/Linux原生宿主资格完成。

上述未取得项保留为后续资格工作，不由本轮已完成检查或独立条件替代。完整结果、全部首次失败、消耗未知值及源/二进制身份见 docs/evidence/code/2026-11-01-agent-loop-verification-feedback/OUTCOME.md。没有新增恢复额度、第二套控制器、第二份持久化状态或模型裁判。

## 通用优化与防过拟合续篇（用户目标 ACTIVE）

- [x] **T022 冻结优化前版本和实验契约。** 当前 checkout 留存 scoped source/diff、CLI/ACP pair 与精确 test binaries；沿原 strict candidate 规则检查身份。原4K数据任务、原生文件多轮任务与五个原工程 seeds 在生产改动前注册，不按输出选择样本。对照缺陷和环境失败均留存。实际 baseline97a072cc，首次不匹配构建保留；共享其它源变化不作隔离因果证明。
- [ ] **T023 统一压缩预算。** 先复现事实入口忽略目标预算及 fallback 不收缩的真实失败。复用 `CompactionBudget`、原 token estimator 和原 seal；计入实际固定提示与保留尾部。默认 trait 扩展如必要，保留已有 adapter 的调用兼容。成功/失败/截断和 Unicode 各有真实实现测试。
- [ ] **T024 避免无效再次压缩。** 根据真实可回收历史和上次压缩结果审查触发行为，修复已证实路径；不增加恢复额度、模型路由判断或任务关键词。保留连续执行、旧日志、重放、后台输出、父/子任务和多工具批次验证。
- [ ] **T025 固定GLM与独立任务验证。** 原模型 `glm-01/glm-5.3`；冻结原输入/预算/验收器；包括原失败数据任务、五个原工程 seeds 和原生文件多轮任务。另注册独立任务验证跨领域状态保留。记录正确性、流程、原机制门、完整 usage、请求与耗时。所有已启动首次尝试保留。压缩次数减少不自动计为原强制压缩资格通过。
- [ ] **T026 工程门与对抗审查。** effect/Core/ACP focused tests、default/minimal/advanced 受影响组合、all-target clippy、fmt 和依赖边界。逐条检查无任务名称/答案/seed/模型特判、无原 grader 改动、无权限扩大、无第二套机制。复开产物后交付 `docs/evidence/code/2026-11-01-agent-loop-general-optimization/OUTCOME.md`；证据不足则目标保持 ACTIVE。

## 正常完成优先的效率续篇（用户目标 ACTIVE）

- [ ] **T027 修复真实完成状态衔接。** 保留上一轮冻结8例的全部首次结果。先以公开Session和真实ACP通知复现宿主验证未进入模型反馈、拒绝确认缺少终态、已打开工具行再次打开三处失败。复用原完成门、原PermissionDenied和原painted_tools；阻塞宿主回调留在线程池。覆盖缺失验证、明确测试契约、旧确认和取消；不让字节一致性替代语义检查，不增加恢复或配置。
- [ ] **T028 以正确且正常结束为前提复验。** 固定glm-01/glm-5.3、原输入/提示/预算/验收器和独立holdout；冻结新的CLI/ACP与精确测试入口，原8例每例至多1次新版本尝试。按完整任务比较请求、native usage和耗时；失败、缺失usage、强制压缩资格单列，不能用更快失败声明优化。不能覆盖前一轮数据或换更容易任务。
- [ ] **T029 复核内核融合及可泛化范围。** 既有事实日志、去重阈值、可信新观察和完成门保留唯一权威；核对无任务/文件/字段/公式/seed/模型特判，无重复批处理提示、第二套停止控制器和独立账本。完成Core默认/minimal、受影响advanced、ACP、clippy、现有CI接入与scoped格式检查，复开产物和证据后报告效果。未取得正常完成或成本改善时目标继续ACTIVE。

## 原始批量回复保真与效率续篇（用户目标 ACTIVE）

- [x] **T030 复现批量历史重复并修复唯一投影。** 同一原始模型回复的公开文字和私有续接被按每个工具结果重复发送；先保存1/2/3/8成员、完整/保留后缀和重复压缩的真实red，再按原始fact.seq恢复一条assistant回复与所有有序工具结果。复用原日志读取者、消息类型及检查点的分组契约；不按文字或ID相似性去重，不改原始事实、调用、结果、错误、图片和用户边界。保留旧压缩器的独立文本输入兼容，不放松真实provider入口对孤立结果的拒绝。279项相关检查通过；字节减少不能直接当作native token收益。
- [ ] **T031 固定真实模型验证批量效率。** `glm-01/glm-5.3`、原配置hash、所有原任务、预算、超时和grader保持；保存SDK原两例的前后首次尝试和原8例新版本首次尝试。冻结CLI/ACP及精确测试身份。SDK编辑批次由宿主立即结算，须明确其回放覆盖局限。只对正确且正常完成的任务比较native usage/耗时；缺失usage保持未知。
- [ ] **T032 核对融合与最终工程证据。** 复查原始来源序号、不同回复相同文字/重用ID、不同每调用元数据、私有续接预算、图片/错误与用户边界；运行Core默认/minimal/advanced、ACP受影响检查、clippy、格式和现有CI过滤器。记录已有失败、共享源因果局限和未测原生平台。交付 `docs/evidence/code/2026-11-01-agent-loop-batch-history/OUTCOME.md`；未完成的长程资格和整体性能目标继续保持未完成。

T031/T032本轮实验和审查已执行并交付证据，资格门仍未通过，故保持未勾选：原8例6通过2失败，Rust五例均成功配对的请求83→64、任务执行耗时1115.040→872.313秒；整组native usage不完整，完整usage的两例合计反增2.13%。主23工具定义指纹相同，辅助单工具定义跨版本变动51字节，所以只能报告集成观测，不能将速度差异归因于本补丁。批量历史修复本身与冻结源一致，没有改原任务、模型、预算或grader。

- [ ] **T033 定位长数据首次保真缺口。** 复用原事实日志、compactor和测试侧有界证据，逐段核对原始读取、公有模型checkpoint、摘要/sealed/kept及后续消费。所有原始读取都有回执；模型称早期数据缺失不能直接证明摘要丢失，更不能当作从未读取。先获得真实red再改生产路径；不按任务字段、名字、公式或关键词挑选需要保留的信息，不把私有续接写进公开证据。
- [ ] **T034 补严格效率对照和完整计量。** 在当前checkout先冻结完整源和辅助工具面，构建仅含已审查优化差异的可信baseline/candidate；若无法稳定则记录阻塞，不擅建worktree。保持原输入、模型、generation options、工具定义、预算、超时和grader。只对双方正确且正常结束的任务比较请求、实际native usage和耗时；未知不推算，所有失败/首次尝试保留，不作结果驱动重跑。
- [ ] **T035 复核现有历史恢复能力再决定接线。** 先审查原始公开观察是否可由现有受治理工具/宿主API在原权限内恢复，避免第二账本、重复工具路由和新恢复控制器。新权限边界或其它实质扩权必须先列明具体失败、可能性和代码成本，再按AGENTS.md取得批准。不得用跨会话访问、私有推理导出、额外恢复额度或放宽完成门取得通过。


2026-11-02 公有恢复实验（R5）已评估：真实 Session red→green，默认/minimal各299项通过；既有有界 ArtifactStore 与受治理 read/search 可恢复摘要遗漏的公有文本。但原两项长数据 baseline/candidate 均失败，候选恢复出现多次归档往返并接近600秒上限，不能验收为效率改进。完整首次结果见 `docs/evidence/code/2026-11-02-compaction-source/OUTCOME.md`。没有增加工具、控制器、权限、预算或永久保留根。

- [x] **T036 复现并接通现有公有观察恢复。** 已覆盖原文/pruning顺序、lossless/微预算/容量拒绝/淘汰、远端内容adapter不落本地副本、真实受治理read和save/resume。此项表示恢复接线已验证，不表示R5性能获准。
- [x] **T037 修复归档搜索的既有输出和分页契约并完成首次实验。** R5实际调用count却收到内容，游标也被忽略。先保存真实ToolRegistry red，再共享现有grep输出枚举与PageRequest；覆盖content/count/files_with_matches/summary、空结果、错误参数和输出预算下的继续读取。使用原GLM、holdout→feed顺序、原输入/提示/预算/超时/grader保存候选首次结果；失败保持失败。
- [ ] **T038 测量固定上下文开销再收敛机制。** 主调用约43761字节工具定义与18970字节系统文本，原16K测试的固定开销约15K native prompt tokens，不能靠反复压缩回收。先按实际模型面量化重复/冗长来源；任何精简必须保留参数、权限、检验与任务完成语义，不按任务/文件/模型挑选工具。当前链式原文恢复没有通过长数据效率门，后续只基于实际证据调整；不叠加恢复控制器。

R3/R4五个成功工程配对的补充计量：按确切system指纹区分已有记忆提取后，主执行native1430366→1089179（−23.85%），五组主执行均有完整用量；缺失的是记忆请求。该事后诊断不替代原整任务验收口径，未知记忆成本不能作零，完整usage两例反增2.13%的原结论保留。T034仍未完成。


T037工程检查默认368通过/minimal364通过，四项clippy均0；原GLM两例仍失败，故只完成契约修复与实验，不是长程效率验收。独立任务600089ms/26请求/7压缩；feed192447ms/9请求/3压缩且只观察3/4发布版本（原任务要求7），最后native响应max_tokens；均无产物，不能算提速。见 `docs/evidence/code/2026-11-02-artifact-search-contract/OUTCOME.md`。

T038测量发现已有 `ToolPresentationProfileV1::code()`，先复用它，不新增工具选择器。隔离SDK19工具的同源描述/schema值36293→6784字节（−81.3%）仅是表示开销，不是native用量收益，也不是ACP23工具面。
- [ ] **T039 分层验证既有Code模式。** 先原样运行已有两文件Adaptive→Code真实控制，模型明确glm-01/glm-5.3、同一冻结SDK二进制、原prompt/input/grader/限制，一次首次执行；不得并发另一真实模型性能样本。随后另行登记复杂长程任务的profile对照，保留所有原Adaptive失败。profile只能作为公开实验变量，不放宽嵌套权限/沙箱/任务完成门，不改默认、不加模型/任务关键词路由。简单控制即使通过，也不能勾选复杂长程或整体token验收；不完整记忆/用量继续未知。


T037收尾又复现并修复既有允许最小1KiB输出预算下的首匹配超限；默认368/minimal364再次通过。此修复在原真实cohort结束后，原真实结果仍绑定原冻结candidate，新分支仅有独立工程检查，未结果驱动重跑。

T039简单控制已完成：原泛用测试误调Flash专用断言，首个fixture尝试在LLM调用前失败，两模式任务均NOT RUN；仅改泛用real_agent读取原loader已验证的显式模型，不改共享loader、Flash专用门、prompt/input/grader/预算。保留该失败并预登记入口修正后，glm-01/glm-5.3两模式首次实际任务都成功。同一冻结SDK二进制，Adaptive9367→Code9089执行阶段上报tokens（−2.97%），14852→14015ms（−5.64%）；N=1且仅两文件，不代表稳定/复杂长程收益，不是完整背景记忆账单。见 `docs/evidence/code/2026-11-02-existing-code-profile-control/OUTCOME.md`。T039复杂长程部分与T034保持未完成，未切默认/添加选择器。


## 原事实公开归档收敛（2026-11-02）

T039预审发现原六轮native任务明确禁止运行代码，原feed/holdout的直接调用/回执机制门也不覆盖program的嵌套回执。保留这些原契约，不通过修改prompt/grader或要求模型发出未展示的调用来套Code模式。已有Code的复杂任务资格仍只考虑允许代码且覆盖嵌套治理的适用场景，不能代替原native/长数据资格。

- [x] **T040 复现并收敛连续压缩的恢复视图。** 两项真实red证明最新archive在两次有损压缩后缺少早期原文，其中一项旧对象已被既有容量淘汰。复用现有checkpoint投影与原始事实读者，把原公开观察平铺到现有有界ArtifactStore；删掉重复的folded结果解码器。手动/自动共享原构造与预算，legacy只有Message时保留当前窗口来源，不伪造更早事实。覆盖重用调用ID、Unicode/空格路径、私有续接不导出、原日志不变、lossless/容量/remote adapter和save/resume。相关默认/minimal各307通过，Core默认/minimal/advanced与ACP all-target clippy均0。
- [ ] **T041 原任务固定条件端到端资格。** `glm-01/glm-5.3`与原配置hash；原holdout→feed，每例新候选首次尝试一次。原输入/prompt/grader/16K或24Kcontext/4K输出/40请求/600s prompt/1800s outer不变。新四成员冻结同一完整源身份；R6失败结果仅作历史参考，不当作隔离性能对照。原长数据失败保持，正常完成/完整native使用量未齐全时不能报告效率通过。见 `docs/evidence/code/2026-11-02-flat-public-history/`。

- [x] **T042 修复恢复元数据污染目标。** 先保留真实red和六次交接回归，再将归档引用置于独立框架段；仅当新对象实际保留时替换精确生成段，保留用户正文中的同名文字和容量拒绝时旧引用。默认/minimal各308通过、Core三种组合与ACP clippy均0、scoped格式通过；原R8两例失败保持，R9尚未真实LLM资格，不声明效率。见 `docs/evidence/code/2026-11-02-compaction-source-boundary/OUTCOME.md`。

- [x] **T043 删除静态工具说明的明确重复。** `task` 的完整动态目录只保留在原 `agent` 参数中；通用工具契约精简措辞，原权限/字段/分页/字节/编辑与验证规则保持。真实red→green、原prompt测试不改，默认/minimal各514通过1既有ignored、四项clippy均0、scoped格式通过。19工具参数schema逐字节相同，SDK固定表示48696→46919字节（−3.65%，非native用量）。独立候选构建遇到外部测试源漂移，身份冻结拒绝且LLM未运行；不声明原长任务完成或提速。见 `docs/evidence/code/2026-11-02-static-contract-cost/OUTCOME.md`。

- [x] **T044 修复摘要正文预算顺序。** 真实 native/folded red 表明：正文小于 allowance 的75%仍会被稍后删除的长Goal挤掉。复用原 section remover→原 cap→原 seal，调整内置摘要模板只请求宿主保留的正文；保留短尾原本通过的控制和首次fixture失败，不改原grader。默认516通过1项外部新测试失败1ignored，minimal517通过1ignored，四项clippy与ACP相关控制通过；整体工程门未因此豁免。见 `docs/evidence/code/2026-11-02-summary-body-budget/OUTCOME.md`。
- [ ] **T045 验收正常完成和真实成本。** R9/R10/R11同源冻结的原GLM首次cohort：独立任务572580ms通过、5/5读取、9次压缩、31请求；feed600067ms超时、7/7读取、9次完成压缩、23请求且第10次摘要未完成，无产物。前者已知495323 tokens另1条memory未知，后者已知306450另1条summary未知。完成性1/2，不能宣称整体提速/省token。所有首次尝试、源码漂移拒绝、原失败和未知计量保留；不以改变预算、模型、任务输入/提示或grader取得通过。
- [ ] **T046 测量不可压缩开销并减少无效往返。** 先用实际ACP工具面与原预算量化固定前缀、可用正文、摘要/恢复的关键路径。审查现有摘要fallback与公开工作结果投影；main的公开文字在`call.text`，不能从顶层`payload.text`空值推断没有工作状态。仅在真实实现red后改既有所有者，禁止新恢复控制器/账本、task/field/seed/model特判或regex猜业务意义。若精简工具注释，必须逐项保留语义与完整validation schema，并用独立复杂任务验证正常结束；原8例资格和T034完整严格效率比较继续开放。

- [x] **T047 精简原生工具说明而不改变契约。** 十处Core源文件只精简Tool/参数说明，保留完整验证结构、默认值、示例、字段名/顺序及独有操作限制；schema-aware对照保留真实名为description的参数。同一SDK19固定表示46919→42194字节（−10.07%，非native用量）。拒收旧库零变化测量并修正Cargo产物定位、保留所有失败；共享task.rs后台结算修改不覆盖、不归为本补丁。默认/minimal各758通过2ignored，四项clippy最终均0，ACP相关控制通过。见 `docs/evidence/code/2026-11-02-native-tool-docs/OUTCOME.md`。
- [x] **T048 修复无损交接的宿主开销重复计入。** 保留实际red [1,1]→green [0,0]：原文已符合正文额度时，Goal/标题等宿主恢复内容不再逼出额外模型调用，逐字保留原输入；原contains保护不变，超额/有损路径仍按原机制生成。无新控制器/额度/模型设置。十项压缩预算控制、四窗口失败/截断和64Unicode变体通过。见 `docs/evidence/code/2026-11-02-lossless-handoff-overhead/OUTCOME.md`。
- [ ] **T049 原条件长任务与泛化资格。** R12/R13匹配CLI/ACP与三个原runner已同源冻结，原GLM、配置hash、任务输入/提示/grader/输出和请求上限/期限不变；holdout→feed各一次首次尝试。只有两项关键原任务均通过，才继续同冻结原六轮native及2401..2405五个Rust seeds；否则记NOT RUN并保留全范围未完成，不用更容易任务替换。只在正确正常结束的任务上评价native与耗时，所有未知用量保留未知。首次cohort两项均600s超时无产物：holdout5/5读取、36请求、已知598239加未知summary36；feed6/7读取、24请求、已知291169加未知main24。六项补充测试按原条件NOT RUN，无重跑。微回归与字节减少不代表整体优化通过。

- [x] **T050 证明并修复失败回退的公开状态丢失。** 先通过真实compact_handoff和CompactionSource.attach复现：重复Goal在预算截断前未移除；替换框架两行归档notice错误吞掉其后折叠正文。保留原obligations、原始事实和有界归档；不加控制器/账本/预算/任务字段特判。不更改已冻结真实cohort，先做实际red→green，再决定新的资格验证。见 `docs/evidence/code/2026-11-02-compaction-fallback-state/preregistration.json`。

T050实际证据：首个notice删除red成立，短状态fallback控制原本通过并保留；补充大于半尾但小于原正文额度的状态后，两项均真实red，最小生产修改后green。原相关default313通过；扩展capability339通过2失败（Knowledge绑定/UI固定digest），不改期望、不豁免。初次minimal被并行后台helper误用Scope而非handle的API编译错误阻断，只修正这一调用并继续检查。见R14 OUTCOME，工程回归不代表整体性能资格。

- [ ] **T051 R14正常完成与未见数据资格。** 所有原模型/任务/输入/提示/grader/预算/期限保持，先完成检查并冻结匹配CLI/ACP和3个原runner；原holdout→feed各首次一次，无结果驱动重跑。两项均通过才运行新的未见opaque数据（原contract/request/notes字节不变、原oracle），再执行原六轮native及5个Rust seeds；任一关键失败则剩余NOT RUN。保留旁路检查失败、并行源差异和未知usage，不把静态字节或两项微回归当整体优化。见 `docs/evidence/code/2026-11-02-compaction-fallback-state/qualification-preregistration.json`。

T050收尾边界：旧notice若已截断，原查找会越行吃到后续checkpoint的右括号；真实red后把匹配限定在notice行，第三项green。最终default/minimal各340通过2项相同capability失败，三项本补丁回归均通过；四clippy0、ACP103通过4ignored、5文件格式通过、全仓格式15其他路径失败。R14真实模型0次，99个原ACP测试/辅助路径hash不变，原/未见数据与oracle重开核对通过，T051仍开放。

T051真实首次终态（candidate-4）：holdout385973ms通过，18请求、3压缩，已知native239762+未知summary14；feed600082ms超时，25请求、10压缩，7/7读取但无产物，已知297873+未知summary25。七项补充资格NOT RUN，仍未完成。用户要求逐次报告具体差值和百分比：见holdout-descriptive-deltas.json（同任务历史成功观察，非同源A/B）、feed-diagnostic-deltas.json（两次失败，仅资源诊断）。新增TUI全面正确/高效要求按017交接衔接，不以SDK通过代替UI，不新增并行生命周期控制器。
