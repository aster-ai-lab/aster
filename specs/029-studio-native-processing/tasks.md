# Tasks: Studio 原生处理优化

**Input**: [spec.md](spec.md)、[plan.md](plan.md)
**Scope**: 完成阶段 A1；A2 在大表资格验证失败后留在 backlog。

## Phase 1: Behavior and tests

- [ ] T001 [US1,US2] `apps/studio/src-tauri/src/office_tool.rs` 与 `office_tool_tests.rs`：真实 DOCX 多段落/空行/特殊字符往返、XLSX 字符串/空格/前导零/公式样式文本往返；先以现有实现运行并记录不等价处，再接入 Office batch。验证超限在启动进程前拒绝，合法大批次不逐单元格启动进程。
- [ ] T002 [US1,US2] `apps/studio/src/renderer/pages/agents/pdfToolsOffice.ts` 与 owning tests：源代码验证路径使用同一 batch 协议和根工作区 Office binary；保留超时、取消结算和输入拒绝；实际重新打开生成文件。

## Phase 2: Wiring

- [ ] T003 [US1] `pdfToolsRun.ts` 两个 DOCX 生产导出接入写入器；删除重复 DOCX 实现，XLSX 未变更代码移入 `pdfToolsXlsx.ts`。保留 MIME、文件名、文字标签与顺序；实际导出 fixture 往返，host 请求与失败传播测试。

## Phase 3: Evidence

- [ ] T004 [US1,US2,US3] `docs/evidence/desktop/2026-10-30-native-office-quick-wins/OUTCOME.md`：记录前后命令、测试数、同输入耗时/进程数、重新打开的文件、未验证项。更新 spec/plan 状态，不把待办标成完成。
- [ ] T005 [US3] 按此前授权提交时，私有 Studio 先提交，父仓库只提交 gitlink、计划及证据；逐项 stage 并核对清单，签名、DCO、正常推送，不覆盖用户原工作区。

## Final Verification Gate

- [ ] Owning focused tests 通过；每个批量失败已单独复核并记录。
- [ ] `cargo fmt --all -- --check` 干净。
- [ ] Studio 双租户 owning Clippy 和根依赖检查，结果留存。
- [ ] 无新 crates.io A3S 依赖、Cargo workspace 或锁文件。
- [ ] 无新 native select 或产品配置。
- [ ] 生成 DOCX/XLSX 已重新打开，与原始输入逐项对照。
- [ ] 不截断超限输入，deadline、输出边界、临时目录清理保留。
- [ ] evidence 归档清单已按字节和哈希重新打开核对。
