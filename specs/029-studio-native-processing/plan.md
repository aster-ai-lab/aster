# Implementation Plan: Studio 原生处理优化

**Branch**: 当前工作分支 | **Spec**: [spec.md](spec.md)
**Gate context**: `apps/studio` product regime；首批 Office 生成器统一。

## Summary

先完成 DOCX 原生生成。现有 Office 写入器的逐格进程改成 batch，保留截止时间、输入限额和清理。XLSX 生产路径在大表测试失败后保持原实现，不能将内部 batch 改造当作生产迁移完成。

## Constitution Check

| Principle | Status | Evidence in the plan |
| --- | --- | --- |
| I. Fail closed | PASS | 沿用有界输入、输出与明确错误，不回退或截断 |
| II. Evidence retained | PASS | T004，独立 OUTCOME 与原始日志、往返文件 |
| III. Hermetic tests | PASS | T001–T003，真实 Office CLI 与离线 fixture |
| IV. Closed sets | PASS | 不新增命令；使用已有 create/batch 和 mutation 类型 |
| V. One workspace / public APIs | PASS | 根工作区，复用 Office CLI，不增加依赖 |
| VI. Local schema changes | N-A | 不改持久化 schema |
| VII. Shrink-only ratchets | PASS | 不扩大 allowlist |
| VIII. Replaceable transports | PASS | 渲染器只调用已有 Office 写入边界 |
| IX. One configuration language | N-A | 无产品配置 |
| XIII. One repository | PASS | 仅当前仓库与既有私有 Studio 子模块 |
| XIV. Names are not guessed | PASS | 路径及 mutation 名称来自 Office 现有协议 |
| XV. Meaning is not matched | PASS | 不用词表或正则推断用户意图/数据类型 |
| XVI. Pickers are drawn | N-A | 无 picker |
| XVII. Dialogs stay in place | N-A | 不改弹窗 |

## Project Structure

| Layer | Path | Change |
| --- | --- | --- |
| Renderer | `apps/studio/src/renderer/pages/agents/pdfToolsRun.ts` | 两个 DOCX 导出接入现有原生写入器；XLSX 保留 |
| Bridge | `apps/studio/src/renderer/pages/agents/pdfToolsOffice.ts` | 复用 batch；源代码验证路径指向根 target |
| Retired source | `apps/studio/src/renderer/pages/agents/pdfToolsDocx.ts` | 删除重复 DOCX 写入；未变更的 XLSX 写入移到 `pdfToolsXlsx.ts` |
| Native host | `apps/studio/src-tauri/src/office_tool.rs` | 复用 create/batch，替代逐单元格 set |
| Tests | 上述 owning sibling test files | 真实文件往返、边界及主机请求 |

## Research

- `src-tauri/src/office_tool.rs` 已有 capture/docx/xlsx 三个动作、私有临时目录、60 秒截止时间及 25 MiB 输出限额。
- `pdfToolsOffice.ts` 已有对应写入器；生产导出仍在 `pdfToolsRun.ts` 调用独立的 `pdfToolsDocx.ts`。
- 当前 XLSX 原生写入为一次 create 加每单元格一次 set。直接切换会扩大进程开销。
- `crates/office/crates/cli/src/office_native_cli/replay.rs` 已支持 schemaVersion=1 的 mutation batch，单批最多 10,000 项、输入最多 8 MiB。
- `NativeOfficeMutation::SetText` 保持字符串；`AddParagraph` 可保留多段落。不得用会推断数字/公式的导入替代。
- Node 路径只供现有源代码验证使用；正式 WebView 继续走 Tauri。保留两条实际实现路径的往返测试。

## Phases and backlog

| 阶段 | 功能 | 验收条件 | 状态 |
| --- | --- | --- | --- |
| A1 / P1 | DOCX 原生生成；现有 Office 写入器批量调用 | 真实文件逐项往返；明确拒绝；不逐格启动进程 | 进行中 |
| A2 / P1 | XLSX 生产迁移 | 10,010 格在现有 20 秒时限内完成，全部值和类型一致；需显式字符串的高效批量原生 API | 待做，当前 batch 未通过 |
| B / P1 | PNG 编解码、像素变换、裁剪滤镜、拼图 | 像素/颜色/透明度一致；大图内存与耗时基线；统一取消 | 待做 |
| C / P1 | PDFium WASM 与原生渲染统一；PDF 组装旋转 | 页序、尺寸、文字坐标及字体对照；保留交互文字层 | 待做 |
| D / P1 | Excel 预览解析、样式与图表 | 原始 workbook fixture 保真；不得丢公式、图表或日期格式 | 待做 |
| E / P2 | 公式补算 | 错误值、缓存、循环、日期系统和预算的等价测试 | 待做 |
| F / P1 | 笔记搜索接入已有原生搜索 | 先核对两个入口契约；匹配范围、评分、正则与分页不变 | 待做 |
| G / P2 | 对话导入、第三方导出 | 分支/顺序/附件往返；事务失败恢复；平台接口契约 | 待做 |
| H / P2 | 计划草稿等 localStorage 归并 | 旧数据迁移、跨窗口一致性、失败不丢草稿 | 待做 |
| I / P3 | 语法高亮 | 先证明大文本确有瓶颈，再比较 token、主题和 IPC 成本 | 待评估 |
| 非本计划 | Python 执行宿主 | Rust 仅能接管调度，仍需 Python 引擎及单独的隔离设计 | 不作为快赢 |

## Verification Plan

- Renderer：生成路径、host settlement、文档工具及 DOCX/XLSX 往返 focused tests。
- Native：`office_tool::tests`，真实 CLI 文件往返；边界拒绝测试不得调用外部服务。
- 度量：同一输入下记录原生进程数和耗时；不提前承诺比例。
- `cargo fmt --all -- --check`；owning Studio 双租户 Clippy `--all-targets --no-deps`；工作区依赖检查。
- `pnpm lint`、定向格式/类型对照及 renderer 构建；既有失败与本批失败分开记录。
- 不增加数据库或云服务测试；不宣称未运行的平台验证。

## Analyze verdict

2026-10-30：US1 对应 T001–T003，US2 对应 T001/T002/T004，US3 对应本计划与 T004/T005。无缺失验收项。阶段 A2 的大表测试失败后已收回生产接线，保留失败源码和日志；未改测试时限。允许完成 A1，A2 及后续阶段保持待办。
