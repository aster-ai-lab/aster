# Tasks: Core 检索融合收敛

**Status**: Implementing；只服务完整 Core 替代的一个收敛批次，不缩小总目标。

- [x] **T001** 固定 caller/政策与原 release baseline；记录三处 source、原 workload/fixture/grader hashes、主机与结果。布局变更依赖此证据；FR-002/FR-004。证据：`baseline-identity.json` 与原 `fusion-workspace-baseline.json`，先运行后改布局。
- [x] **T002** 新增 legacy 同分／重复 channel 反例，保留旧实现真实 red；frozen expected score/rank/metadata 与边界测试同批，不复制业务算法；FR-001/FR-002/FR-003。证据：`legacy-rrf-red.log` 两项真实失败；新增 sibling kernel tests 5 项通过。
- [x] **T003** 实现唯一私有融合 owner，接通三个 Core facade 并删除重复循环；保留公开 API、precision、single-channel、latest revision 与原排序政策；依赖 T001/T002；FR-001/FR-002/FR-003。证据：`RankFusion` 三处实际 caller，`core-rrf-tests.log` 4 项通过；其余资格仍按 T004/T005。
- [ ] **T004** 原 Core retrieval/memory tests、固定 eval、actual ACP target 与前后原 release workload；核对 hash/参数/计数并保留 D05 FAIL；依赖 T003；FR-002/FR-004/FR-005。
- [ ] **T005** scoped fmt/diff、owning Clippy all-targets、Core minimal、paired build 与 caller review；既有失败和未测平台单列；依赖 T004；FR-001–FR-005。
- [ ] **T006** 复开 evidence、回写 033 机制账本及 OUTCOME；无 stage/commit/release，不把本批次宣布为完整替代；依赖 T005；FR-005。

## Final Verification Gate

- [ ] 三处 loop 收敛且实际 caller 使用同一 owner；T003/T005。
- [ ] 原政策/公开 API/输入/grader 保持；tie 修复有真实反例；T002/T004。
- [ ] 原 release gate 前后同参数；失败保留，无“加速=少测”；T001/T004。
- [ ] Owning tests/fmt/Clippy/minimal/paired build 与未测范围准确；T004/T005。
- [ ] D05 和其它 owner 未完成项继续阻止总目标完成；T006。
- [ ] 无新 deps/workspace/runtime/config/repo/worktree，无权限扩大；T005。
