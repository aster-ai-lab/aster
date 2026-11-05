# Tasks: Xerj 完整替代契约

**Input**: [spec.md](spec.md)、[plan.md](plan.md)；完整总目标保留于 [总计划](../../docs/xerj-replacement.md)。

**Status**: Implementing。这里的勾选只表明契约批次任务完成，不表示对应产品能力 PASS。所有产品回放初始 NOT_RUN。

## Phase 1: 固定源与保真数据

- [x] **T001 [US1] 固定源与许可。** 核对 `docs/evidence/code/2026-11-01-xerj-replacement/` 的发布 commit、manifest、LICENSE、MCP schema 与源码；记录 main 差异、22 项最大集合及条件，不能靠上游宣传分母。FR-001/FR-002。
- [x] **T002 [US1] 生成器与拒绝测试。** 增加 `tests/xerj_replacement/inventory.py` 和 sibling `test_inventory.py`；验证有界读取、schema digest、名称／required／property／完整对象摘要、源顺序、精确 owner 双向覆盖，坏源／未知／重复／漏项均拒绝；`--check` 比较完整生成物而非 keyword。FR-001/FR-002/FR-003。
- [x] **T003 [US1/US2] MCP 精确 owner 与缺口。** 手写 `contracts/mcp-owners.json`，逐项读取 22 个工具的完整 schema，核对 `aster_paths` 候选源码；生成 `mcp-inventory.json`。required／参数不删，Hub 条件明确，入口未完成保持 NOT_RUN。T002 tests 同批验证，不推定近似工具即兼容。依赖 T001/T002；FR-002/FR-003/FR-004/FR-005。
- [ ] **T004 [US1/US2] 其它对照面与机制归并。** 按固定 CLI、HTTP／gRPC route、Query AST、extractor、存储／运维／集群与 Corpus schema 分别展开，写入 `contracts/mechanisms.md`，与真实声明结构做双向完整核对并追踪唯一 owner／重复候选／收敛路径；复用 T002 验证模式，每个新增 parser 带小型反例。保留未核查的分母，不用 MCP 清单替代全部。依赖 T001；FR-001/FR-002/FR-007。已完成 QueryNode 44 变体及有序字段的 AST structural check；支持类型与其它对照面仍开放。

## Phase 2: 实际合同与回放

- [ ] **T005 [US2/US3] 冻结黑盒 cases。** `contracts/` 按 surface 固定请求、正／反例、fixture／来源身份、scope、预算、错误、动态字段及浮点比较规则；先核对 owner，实际能力缺口指向后续 owning implementation，不自动生成占位 handler。比较器带已知正负 controls，不改既有质量 grader。依赖 T003/T004；FR-004/FR-005/FR-006。
- [ ] **T006 [US3] 双目标 process adapter。** 复用 032 `acp_engine.py` 和实际 stdio／host helper 接对应真实目标；只有实际请求／回执可进入结果。坏 executable、协议、身份、缺失结果、超时与双方失败均不 PASS，测试覆盖清理与不外发。依赖 T005；FR-004/FR-006。
- [ ] **T007 [US2/US3] 实际逐项 replay。** 依赖已经完成的 owning implementations，按同一冻结输入运行全部 surface 的适用正反例，保留参考及候选 raw outputs 和副作用证据。参考未支持／缺 key／资源记录真实状态；负例拒绝不能给正例通过。MCP、CLI、HTTP、gRPC、数据／运维均独立分母；未运行阻止完整替代声明。依赖 T006 与相应实现；FR-004/FR-005/FR-006/FR-007/FR-009。

## Phase 3: 复核与证据

- [ ] **T008 [US1/US3] 交叉审查与 gates。** 复开清单、原文、fixture 和生成物，核对数目／顺序／参数／状态／身份；运行 focused tests，相关 Rust 的 owning Clippy、fmt 与 paired build，原生平台与 tenant 分开；保留既有失败，不扩 allowlist。依赖 T007；FR-008。
- [ ] **T009 [US1/US2/US3] 执行记录。** 写 `docs/evidence/code/2026-11-01-xerj-replacement/OUTCOME.md`，明确清单覆盖、真实 replay、缺口、NOT_RUN／BLOCKED 与后续 owner；回写总计划，不宣布单批即整体完成。无需 stage／commit／发布。依赖 T008；FR-007/FR-008。

## Final Verification Gate

- [ ] 生成物完整可复算，所有 source／schema digest 保真；T001–T004/T008。
- [ ] 每项声明、实际入口、资格状态分开；未审计 surface 未隐去；T004/T007/T009。
- [ ] Owning tests 与新行为同批，比较器通过反例；T002–T008。
- [ ] Rust 若改动，fmt／owning Clippy／paired build 与原生状态准确；T008。
- [ ] 无新 crates.io Aster 依赖、workspace／lockfile／SDK／repo／worktree；T008。
- [ ] 无 native select 或权限扩大；配置仍只 ACL；T005/T008。
- [ ] 无未经授权外发、无 Xerj sidecar 冒充实现；T006/T007。
- [ ] 证据复开核对，总体目标没有缩成清单批次；T009。
