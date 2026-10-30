# Tasks: ACP 长程真实 LLM E2E 与优化

**Input**: [spec.md](spec.md)、[plan.md](plan.md)、[cases.md](cases.md)、[journeys.md](journeys.md)
**Prerequisites**: Constitution Check 通过；read-only analyze 无未解决 blocker。实施已获用户授权。未完成任务不打勾，不提交/推送。

## Phase 1 — 基础行为与测试系统自证

- [ ] T001 [US6] 在 `docs/evidence/code/<date>-acp-long-horizon/` 记录当前源码/子模块/dirty、工具版本、二进制与 capability preflight；发现可信 ACL、精确模型标识和可用执行后端，不导出密钥/用户状态。测试身份错配、配置不全和零模型调用拒绝：A01–A10、M02/M09。候选身份漂移须重新冻结。
- [ ] T002 [US6] 在 ACP `tests/` 的小型 sibling support 中实现 attempt 结果、证据关联、封闭状态/归因、预算和有界回执采集；测试未知/重复 ID、skip/blocked/unknown usage、截断与重试分母：M07/M08/M10。JSON 仅结果；运行配置 ACL；不添加新产品协议。
- [ ] T003 [US1/US6] 在根测试树复用 std-only 工程、独立 checker 和命名 barrier；只提取必要的已有本地 provider/ACP 进程 helper。测试错产物、改 checker、错过故障屏障和两种合法修复路径：M01/M03/M04/M05/M06。没有自证不能信任后续成绩。
- [ ] T004 [US4/US6] 扩展 `crates/code/acp/tests/acp_conformance_stdio.rs` 或小型 `long_horizon_stdio.rs`（拟新增），通过真实二进制测 B01–B10。每个新边界附 parser/终态/背压 owning-package 小测试；未知容量或不可观察终态先点名实现缺口，不编造通过。
- [ ] T005 [US1] 在 `long_horizon_stdio.rs` 和未来 `long_horizon_real_llm.rs` 共用任务 fixture/oracle，实施 C01–C10、D01–D06、K01–K10；H1 先跑 LH01/LH02/LH03 再绑定真实 LLM。正反对照必须包含一次修多个文件合法通过、失败后校验、旧验证失效和无最终答复；不得强迫不必要的固定工具顺序。

## Phase 2 — 安全、恢复、能力与宿主接线

- [ ] T006 [US3] 在现有 `approval_rules_stdio.rs`、`workspace_layer_trust_stdio.rs`、`consumer_deny.rs`、`plugin_workspace_stdio.rs` 扩展 E01–E10、D08–D10 和 LH07；每个拒绝证明执行计数/受保护字节/网络 canary。新增行为回落对应权限、config 或 tool owning-package 测试，不新增匹配用户意图的词表。
- [ ] T007 [US2/US5] 在现有 `unattended_e2e.rs`、ACP sibling terminal/cancel 测试贯通 H01–H10、D07 和 LH04；用屏障测 EOF/End/迟到回调、审批/表单取消与真实后代清理。每个超时/终态修复有最小 race 回归；保留 0=unlimited 的既有交互轮数和审批语义。
- [ ] T008 [US2/US3] 复用 Core checkpoint/lease 的既有小测试，在 ACP wire 实施 I01–I10、F01–F10、LH05/LH06；对每个崩溃窗口记录提交点和 effect ledger。错误快照/外来 workspace 在资源激活前拒绝；不确定非幂等副作用不盲重放。新接纳或版本分支有 sibling hermetic test。
- [ ] T009 [US4] 扩展 `client_fs_stdio.rs`、Office/检索/插件现有 owning tests，贯通 J01–J10 和 LH09；默认/feature、host bool、dirty buffer、stale index、浏览器惰性和失败清理均有对照。不为消除失败扩大能力或网络来源。
- [ ] T010 [US5] 用小型资源 sampler 和既有 owner ledger 实施 L01–L06/L10 及 LH10；sampler/分位数/斜率/retention gap 自身有固定输入测试。先测基线再 ratchet；8h/24h 只有在明确档位与预算后启动，不用短循环代替实际耐久声明。
- [ ] T011 [US1/US6] 复用现有 provider client/协议 fixture，实施 G01–G10 和 LH08。若需 fault relay，真实凭证仅留现有可信配置和测试父进程内存，ACP 使用无秘密的本地 fixture 凭证；不得把用户 ACL 中的明文秘密复制到仓库/证据。relay 的上游为精确预声明地址，认证不入日志；有损坏帧、限流、取消、重试计数、零外发和“实际请求确实到真实模型”的小测试。任何新依赖须先证明现有 HTTP/provider client 不能满足。
- [ ] T012 [US4] 在 `crates/code/acp/tests/a3s-test/long_horizon.acl`（拟新增）及既有 TUI fixture 实施 LH11/L07；先实际观察稳定路径，`a3s-test check` 后执行。接线改动有 owning-package 测试；PTTY/ConPTY、授权、停止/恢复和打开当前产物都有可见断言。未观察路径不伪造 ACL。
- [ ] T013 [US4] 在私有 `apps/studio` 测试树实施 LH12/L08/L09；遵守其 AGENTS 和 GUI skill，先核验背景控制 capability、独占 lane 与当前候选。metadata/状态接线有私有 hermetic test；绑定丢失即停止，私有源和 fixture 不复制到父仓库。

## Phase 3 — 真实模型、缺陷修复与资格证据

- [ ] T014 [US6] 增加两种 ACP feature 的 owning-package、E2E 与覆盖率入口并检查既有 CI；测试覆盖汇总不能把 Core 依赖、测试文件、未执行 variants 混入 ACP 分母。锁定工具链若不支持 branch coverage 则明确 UNMEASURED；报告器有固定计数样例测试；不得降低 allowlist 或覆盖门。
- [ ] T015 [US1–US6] 固定实际可用模型精确 ID、provider 协议、配置摘要、采样参数、预算、平台/feature/backend 和预注册 seed；先一个 R1 诊断 cohort，再执行选定 R1/R2、U 旅程的发布级重复与全部适用故障 variants。每个需真实推理的场景必须有实际 provider 回执；接纳前拒绝场景应证明模型调用 0，不能为了“真实 LLM”绕过边界。所有失败 SOLO 复现并独立归因；有界重试不覆盖首轮。测试本身的每个新判断有 C0 自证。
- [ ] T016 [US1–US6] 对每个确认产品/测试缺陷按所属 crate 最小修复，**先留失败证据和小 hermetic 回归、再修复、再跑对应 H/R/U 回归**；核对所有调用者与权限，不修改 fixture 降难度，不把故障粗暴归为模型上限。修复后重新冻结 cohort，不能混合前后版本统计。
- [ ] T017 [US6] 记录分层 `OUTCOME.md`、逐 case/variant traceability、执行/阻塞/失败与模型区间、实际覆盖率、资源/预算、构建/当前源身份和 Explicitly not claimed。验证报告器把缺证据/未运行从通过中排除；回归所有已改路径、formatter、依赖与必要 workspace checks。没有实现或执行的格子不能打勾；不提交/推送/部署。

## Requirement → task / case / journey

| Requirement | Tasks | Cases | Journeys |
| --- | --- | --- | --- |
| FR-001 | T001 T002 T017 | A01 A02 A10 M02 | LH01 LH11 LH12 |
| FR-002 | T003 T004 T015 | B01 B02 B03 B10 | LH01 LH04 LH05 |
| FR-003 | T002 T004 T008 | B02 B05 B09 F10 I08 | LH04 LH05 LH06 |
| FR-004 | T003 T005 T015 | C01 C02 M04 | LH01 LH02 LH03 |
| FR-005 | T005 T007 T010 | C03 C05 H05 H06 H07 H10 | LH02 LH04 LH10 |
| FR-006 | T005 T016 | D01 D03 D04 K01 K02 K03 K04 K05 | LH01 LH03 LH09 |
| FR-007 | T006 T009 | E01 E03 E04 E05 E06 E07 E08 E09 | LH07 LH09 |
| FR-008 | T004 T008 | B04 B05 F01 F02 F03 F04 F05 F06 F07 F08 F09 F10 | LH06 |
| FR-009 | T011 T015 | G01 G02 G03 G04 G05 G06 G07 G08 G09 G10 | LH08 |
| FR-010 | T007 T008 T010 | H01 H02 H03 H04 H05 I10 L01 | LH04 LH05 LH10 |
| FR-011 | T005 T008 T009 | C08 I01 I02 I03 I04 I05 I06 I07 I08 I09 I10 | LH02 LH05 LH06 |
| FR-012 | T001 T009 T012 T013 | A08 A09 J03 J04 J05 J06 J07 J08 J09 J10 | LH09 LH11 LH12 |
| FR-013 | T002 T012 T013 T017 | K06 K07 K08 K09 K10 L07 L08 L09 L10 M08 | LH01 LH09 LH11 LH12 |
| FR-014 | T002 T010 T014 T015 | H06 H07 L01 L02 L03 L04 L05 L06 M07 M10 | LH08 LH10 |
| FR-015 | T002 T003 T016 T017 | M01 M02 M03 M04 M05 M06 M07 M08 M09 M10 | LH01 LH03 LH07 |
| FR-016 | T001 T009 T011 T013 T014 T017 | A05 A06 A09 | LH07 LH11 LH12 |

## Final Verification Gate

- [ ] T001–T004：每条新路径有 owning-package / oracle 自证，M01–M10 都有正反控制。
- [ ] T005–T011：所有宣称已覆盖的 case 有实际 target、variant、屏障、oracle、预算、终态和 cleanup 证据。
- [ ] T014–T017：Rust formatter、依赖边界、适用 workspace checks 已执行并逐项披露；覆盖率口径独立。
- [ ] T006/T009/T011：无跨语言 SDK/新 workspace/新产品配置语言/放宽安全白名单；ACL 和显式权限仍生效。
- [ ] T012/T013：未新增 native select 或其它 UI 改造；真实 TUI/GUI 成绩不被下层成绩替代。
- [ ] T015/T016：模型、网关、产品、规范和基础设施归因有原始证据；失败保留、首轮和重试分开。
- [ ] T001/T013/T017：私有 Studio 源留子模块，不覆盖其它会话修改；当前配对身份和装配字节一致。
- [ ] T017：所有必需目标组合无未披露未完成项；不把 BLOCKED、NOT_RUN 或 PLANNED 当完成，不虚报全绿。
