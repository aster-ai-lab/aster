# Feature Specification: Studio 原生处理优化

**Feature Branch**: 沿用当前工作分支，不另建用户未要求的分支
**Created**: 2026-10-30
**Status**: Ready for implementation
**Input**: 分析 Studio 中仍由 JS 执行的功能，持久化优化计划，先完成快赢部分。
**Gate context**: `apps/studio` product regime。首批统一 DOCX 生成；XLSX 生产接线须先通过大表资格验证。

## User Scenarios & Testing

### User Story 1 — 复用原生文件生成 (P1)

用户将识别文字和会议记录导出为 DOCX。文件生成复用现有 `office_tool` 与 `a3s-office`，删除重复的 DOCX JSZip 写入器。XLSX 原生生产接线延后。

**Independent Test**: 原生命令测试实际生成并重新打开 DOCX/XLSX；renderer 测试实际导出流程及主机请求。

**Acceptance Scenarios**:

1. 多行中文、空行、XML 特殊字符和首尾空格按原顺序写入 DOCX 段落。
2. 表格字符串（包括数字样式及 `=...`）保持字符串，不推断成数字或公式。
3. 超限输入拒绝并保留明确错误，不截断，不返回部分结果。

### User Story 2 — 避免逐单元格启动进程 (P1)

原生及现有 Node 验证路径使用 Office 已有的批量操作协议，避免每个单元格启动一个子进程。

**Independent Test**: 实际批量文件往返检查所有单元格；记录固定输入下的进程数及耗时。超时/清理沿用现有受控进程实现。

### User Story 3 — 可继续执行的优化计划 (P2)

后续迁移有明确顺序、边界和验收条件，不把“可改成 Rust”当作已证明的性能收益。

**Independent Test**: 人工交叉检查 spec、plan、tasks 与来源文件；实现状态必须有 evidence 才能勾选。

### Edge Cases

- DOCX 空文本、CRLF、尾部空行必须重新打开核对。
- XLSX 空表、空单元格、中文、前导零、公式样式文本必须保留类型和位置。
- 已有原生限额：文本合计 1,000,000 字节，500 行、26 列，输出 25 MiB，作业 60 秒。超限必须拒绝；不得静默截断。
- Office 单次 batch 最多 10,000 个 mutation；较大合法请求分批且共享同一作业截止时间。
- 缺少引擎、子进程失败或超时不得回退到另一套生成逻辑。

## Requirements

- **FR-001**: 两个 DOCX 生产导出调用 MUST 接入现有原生写入器；原生失败时 MUST 返回现有失败状态。XLSX MUST 保留当前生产生成器，直到原生大表性能和字符串类型验证都通过。
- **FR-002**: MUST 保留文字、空行、顺序和字符串单元格类型；MUST NOT 推断公式或丢弃内容。
- **FR-003**: MUST 沿用现有输入/输出限额、进程超时和临时目录清理；超限 MUST 明确拒绝。
- **FR-004**: MUST 复用 Office 现有 batch 协议；MUST NOT 为每个单元格创建进程，也不增加 Rust crate、运行时或配置格式。
- **FR-005**: MUST 删除本次替换后无调用者的 JSZip 生成器；MUST NOT 删除其他仍使用 JSZip 的功能依赖。
- **FR-006**: 后续阶段 MUST 在独立验收后推进；MUST NOT 声称未测量的速度、内存或体积收益。

### Key Entities

- Office input：文字或字符串行列，宿主验证后交给现有 Office CLI。
- Office output：原生生成的 OOXML 字节，不更改用户原始文件。

## House Checklist

- [ ] 原生与 renderer 的小型 hermetic 测试（T001–T003）
- [x] 不增加枚举或扩大命令集合
- [x] 不增加生产 unwrap/expect
- [x] 不增加外部 A3S 依赖或 Cargo 工作区
- [x] 不增加产品配置
- [ ] 留存 `docs/evidence/desktop/2026-10-30-native-office-quick-wins/OUTCOME.md`（T004）

## Success Criteria

- **SC-001**: 新增的原生/renderer 验收用例全部通过；旧文件生成相关回归结果完整记录。
- **SC-002**: 普通合法表格只启动 create + batch 两个进程；大于单批上限时按批数增长，不按单元格数增长。
- **SC-003**: 生成文件重新打开并逐段落/逐单元格与输入核对，不以 ZIP 魔数或关键词代替往返验收。
- **SC-004**: 计划、任务、证据链接一致，待做阶段保持未完成。

## Assumptions

- 首批复用现有原生输入限额；新接入的导出不得在超限时生成残缺文件。
- 当前用户工作区已有独立的 Office 质量工作（028）；本计划使用 029，避免覆盖。
- UI 修复、GitHub issues/PR 处理和签名 DMG 仍是本会话独立待完成工作。

## Explicitly Not Claimed

- 不迁移整套 React UI，不改 Office 公式语义，不扩展原生输入限额。
- 不在本批重写 PNG/PDF/Excel 解析、搜索、导入导出平台或 Python 沙箱。
- 不声称已经完成全量原生化或已有性能收益。
- 不声称 XLSX 生产路径已经原生化。一万零十格的原生逐格 batch 在原有 20 秒测试时限下失败，独立复跑同样失败；整表 CSV/TSV 导入会识别数据类型，不能直接替代字符串写入。
