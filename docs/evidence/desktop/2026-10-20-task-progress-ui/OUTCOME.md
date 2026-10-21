# 紧凑任务计划栏

2026-10-20。修改位于 `apps/work` 的 `fix/planning-mode-ui-20261020` 分支，尚未提交或推送。

## 改动

- 用输入框顶部的可折叠任务栏替换悬浮进度胶囊及窄悬停卡片。任务栏进入现有 `topContent` 布局测量，保留白色背景和正文占位。
- 按用户追加要求，将收起按钮压至 32px，移除原有额外下边距和底部内边距；下方保留 1px 分隔线。
- 展开后每行至少 36px，默认单行显示。点击某行可查看完整原文，不裁剪任务数据，也不从长句猜测简短标题。
- 保留明确的待处理、进行中、完成、失败及取消状态；完成后仍可查看清单。审批等界面替换输入框时，进度继续可见。
- 使用现有 Button、Scrollbar 和状态投影，补齐折叠状态、进度描述及键盘操作测试。规划指令要求短动作标题，详细验收条件放正文；模型生成质量没有在本次视觉复测中重新评估。

## 实际桌面 CUA

通过 `mcp__cua_repl` 操作原生 AsterDream R5 的可访问性控件，复用隔离测试会话中已经完成的三个只读核验步骤。最终前端入口为 `index.9de3265824.js`，未向应用注入 JavaScript。

- [收起态](collapsed-32px.png)：任务计划及 `3 / 3 已完成` 同行显示。
- [展开列表](expanded-steps.png)：三个步骤在输入框上方展开。
- [展开长步骤](expanded-long-step.png)：第一步完整内容换行显示，其余步骤保持单行；点击总标题可以收起列表。
- [步骤可访问性记录](expanded-task-accessibility.txt)保留完整标题及展开状态。

视觉复测没有发送新任务、运行命令或修改 fixture。[文件读回记录](fixture-after-visual-check.json)保留三个文件的字节数、哈希与修改时间。[源文件与运行时身份](source-and-runtime.json)记录本次构建的源文件哈希。

## 验证

| 检查 | 结果 |
| --- | --- |
| AgentRightPane、AgentComposerSlot、AgentComposer、权限指令四组测试 | 267 / 267 通过，见 [日志](task-bar-tests.log)。覆盖点击展开、长文本、键盘操作、完成数的可访问性描述、未开始步骤保持待处理，以及审批替换输入框时保留进度。 |
| 前端生产构建 | 通过，见 [日志](task-bar-build.log)；动态导入警告仍存在。 |
| 本次八个文件的 oxlint | 通过，见 [日志](task-bar-focused-oxlint.log)。 |
| 本次八个文件的 ESLint | 无错误，保留三条原有警告，见 [日志](task-bar-eslint.log)。 |
| 本次八个文件的 oxfmt、git diff --check | 通过。 |
| 全仓 oxlint --deny-warnings | 未通过，129 个错误、58 个警告，见 [日志](task-bar-lint.log)。 |
| web TypeScript 检查 | 未通过，278 个错误；本次八个文件没有报告错误，见 [日志](task-bar-typecheck.log)。 |

未重新构建原生宿主及 ACP，未重跑完整工作区 CI。A3S Test GUI 服务未配置，本次证据来自直接 CUA，不声称 ACL 套件通过。实时步骤推进、规划审阅、停止及文件写入的前一轮验证见[原有记录](../2026-10-20-planning-mode-cua/OUTCOME.md)。
