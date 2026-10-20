# 规划模式与任务追踪修复

2026-10-20。本次修改位于 `apps/work` 的 `fix/planning-mode-ui-20261020` 分支；尚未提交或推送。

## 原因与改动

- 原生执行已结束时，前端可能漏掉终态事件，保留生成中状态。现在从接收回执建立可取消的执行状态，并在重连、恢复会话时按执行 ID 与消息锚点核对已保存终态。读回失败时不推断完成。
- 截图中的计划操作栏位于透明的输入框外层，滚动时与正文重叠。现在复用输入框内部的 `topContent`，背景、布局测量和底部占位由现有输入框负责。
- SDK 的 `update_plan` 能力一直存在。原规划提示仅要求文字计划，前端也未识别 ACP 的显式 `TodoWrite` variant 与 `plan[].step` 字段。现在恢复原生结构化计划卡片及进度投影，保留失败、跳过、取消状态，未知原生状态拒绝展示为完成。
- 规划生成中显示固定阶段提示和停止操作；计划完成后提供“修改计划”“开始执行”。有未发送草稿时阻止开始执行，修改计划保留草稿；执行仍使用当前工具授权方式。
- 每轮提交时保存显式规划标记。开关只影响下一次发送，历史回合、正在运行的回合和已排队消息保持提交时的模式。只有最新完成的规划回合可开始执行。
- 执行阶段通过隐藏的回合指令更新结构化步骤。用户看到简洁确认消息；任务进度只使用明确状态，完成后的清单和进度保留可查。

交互参考了 GitHub 的[计划审阅、调整与开始实施流程](https://docs.github.com/en/copilot/how-tos/copilot-in-your-ide/use-copilot-agents/use-plan-mode)；阶段和进度采用不抢焦点的状态反馈，参考 [W3C status messages](https://www.w3.org/WAI/WCAG22/Understanding/status-messages.html)。

## 实际 CUA 结果

测试通过原生 AsterDream 窗口的可访问性控件操作，使用 DeepSeek Flash、“逐次确认”及隔离 fixture。界面访问 `127.0.0.1:4319`；最终加载前端入口 `index.bdca2b09a4.js`。原生宿主及 ACP 身份见 [runtime-identity.json](runtime-identity.json)。未向应用注入脚本。

| 场景 | 结果与证据 |
| --- | --- |
| 重叠修复 | [19-review-overlap-fixed.png](19-review-overlap-fixed.png)；操作区位于输入框内部，正文与操作区不重叠。 |
| 原生规划、只读检查与待处理步骤 | [21-native-plan-ready.ax.txt](21-native-plan-ready.ax.txt)；规划阶段文件与基线相同，见 [before-execution-files.json](before-execution-files.json)。 |
| 多轮中途关闭和开启 | [multi-turn-runtime.json](multi-turn-runtime.json) 与 13–15 号 AX 记录；规划中关闭仍完成规划，普通回合中开启仍按普通回合处理。 |
| 确认执行保持逐次审批 | [23-implementation-started.ax.txt](23-implementation-started.ax.txt)、[24-implementation-progress.ax.txt](24-implementation-progress.ax.txt)、[26-write-approval.ax.txt](26-write-approval.ax.txt)；分别批准 fixture 的 Edit、Write 及只读校验。 |
| 有草稿时停止 | [36-cancellable-with-draft.ax.txt](36-cancellable-with-draft.ax.txt)、[37-stopped-draft-preserved.ax.txt](37-stopped-draft-preserved.ax.txt)；停止可用，草稿保留，持久记录为 aborted，未提供执行按钮。 |
| 停止后重新规划 | [38-readonly-plan-ready.ax.txt](38-readonly-plan-ready.ax.txt)；三个步骤均为 pending，0/3 完成。 |
| 实时步骤追踪 | [40-readonly-progress.ax.txt](40-readonly-progress.ax.txt)、[41-readonly-second-step.ax.txt](41-readonly-second-step.ax.txt)、[42-readonly-third-step.ax.txt](42-readonly-third-step.ax.txt)、[43-readonly-finished-live.ax.txt](43-readonly-finished-live.ax.txt)；同一内联清单及进度连续显示 0/3、1/3、2/3、3/3，无需刷新。 |
| 完成、恢复和折叠 | [44-final-checklist-visible.png](44-final-checklist-visible.png)、[35-completed-after-reload.ax.txt](35-completed-after-reload.ax.txt)、[45-checklist-folded.ax.txt](45-checklist-folded.ax.txt)、[46-checklist-reopened.ax.txt](46-checklist-reopened.ax.txt)。 |

文件字节经独立读回核对：`note.txt` 为 `after-plan-mode\nKEEP_SECOND_LINE_20261020\n`，`result.txt` 为 `plan-mode-e2e-ok\n`；`sentinel.txt` 哈希和修改时间与初始值相同。追加只读核验后，三个文件的哈希和修改时间均未改变，见 [after-execution-files.json](after-execution-files.json)、[after-readonly-files.json](after-readonly-files.json)。原生回合标记、取消状态和完整步骤状态序列见 [final-turn-contracts.json](final-turn-contracts.json)。

早期构建曾出现一次内联清单仍显示 1/3、进度已为 3/3 的现象（34 号记录）；刷新后恢复。最终构建追加的逐步执行复测显示内联清单与进度一致。该单次现象保留为证据，未据此声称消除了所有终态时序问题。

## 自动验证

- 16 个聚焦测试文件：543 通过，1 个原有跳过。
- 输入框运行时的停止与计划操作区回归：2 通过，166 个未选中的测试跳过。
- 前端生产构建、修改文件格式检查及 `git diff --check` 通过；目标 ESLint 无错误，9 个现存警告。
- 完整输入框测试存在 18 个现存失败；使用 HEAD 生产组件重跑得到同样 18 个失败，任务新增的两个回归用例单独通过。
- 已运行规定的全量 `pnpm lint`、Web 类型检查与根工作区 `cargo fmt --all -- --check`，均受现存错误阻塞。任务没有修改 Rust；没有改动或提交其他工作中的文件。详细命令与日志保留在 `target/evaluations/planning-mode-cua-20261020/`。

## Explicitly not claimed

- 不声称全量 CI、全量 lint、全量类型检查或 Rust 格式检查通过。
- 不声称进行了原生宿主/ACP 重新构建、发布、提交、合并或推送。
- 本机 A3S Test CLI 与浏览器能力版本不匹配；这里是直接 CUA 验证，不声称 ACL 套件通过。
- 未进行暗色/窄屏全矩阵、长时间并发、断网压力测试，或所有模型的提示遵循验证。规划提示不会替代原有工具权限和沙箱边界。
