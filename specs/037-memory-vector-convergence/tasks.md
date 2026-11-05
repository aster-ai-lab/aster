# Tasks: 旧记忆向量机制收敛

- [x] T001 固定 source/API/limits 与现有运行状态；read-only analyze 后新增真实反例，运行旧代码保留 red。`legacy-red.log` 实际 0 PASS／4 FAIL；baseline source/原 frozen fixture/metrics hashes 已保留。
- [x] T002 原 InMemoryVectorIndex blocking ports 与 async delegation，同批 sibling sync/cross-port/边界 tests。`shared-vector-tests.log` 6 个 unit cases PASS（含 2 个新增实际 port cases）；被 filter 排除的 integration targets 不计资格。
- [x] T003 删除 Core 私有向量列表/手写 similarity；接通 put/edit/delete/clear/provider-change，持久失败可重试，新 library/put 边界保留 typed error/source，可选语义错误按原合同降级；同批真实 collection tests。最终 `core-final-tests.log` 44 PASS（含 engine error-code）；此前 43 PASS、初次 42 PASS／1 个新 fixture 编码错误、单独复现与真实生产者修正全部保留。旧源码 red 4 项均转 green；没有改变原四个反例、frozen 质量输入或 gold。
- [x] T004 focused default/sqlite/Core regression、minimal、scoped fmt/diff 与 owning Clippy；失败单列，原 gold/grader 保持。shared unit 6、default integration 19、sqlite feature integration 27、workspace 185、typed memory integration 18 PASS；三 owning packages Clippy/sqlite、minimal PASS；功能改动五个源文件 fmt PASS，全仓格式既有失败独立记录。额外 CLI 导入编译修正的 Clippy／paired build 跟随 T005。
- [x] T005 paired build、caller/adversarial review、重新打开证据并回写 033/OUTCOME；未完成 lifecycle/ACP/performance 仍开放。原 release workload 全 gates PASS；最终 paired build／build ID 一致，实际 ACP 8 PASS／1 原 RRF FAIL。两个先前 CLI 构建失败及一行 trait-import 修正、context-ID FAIL 和 root fmt 782 路径保留；无发布／安装。

## Final gate

- [x] 同一 shared exact owner，旧列表/计算消失，无新 runtime/projection/database/config/dependency。
- [x] 同步/异步和旧 mutation 真实回归；source/activation/公开 API 不被偷换。
- [x] 各检查有终态、失败保留；完整目标保持开放，D01 不提前结案。
