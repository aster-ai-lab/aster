# AsterDream /goal × T01 端到端验证

测试当前运行的 AsterDream R5，工作区为仓库内 `tests/T01`。模型沿用 DeepSeek Flash，工具授权沿用逐次确认。输入文件记录哈希与修改时间，任务输出由应用生成；测试者只做独立验收，不代写 `pipeline.py`。

## 顺序与验收

1. 在新任务中选择 T01，输入 `/goal`，观察发现性、命令提示、目标摘要和任务范围。验证不继承其他会话的执行状态。
2. 发送完成 T01 的持续目标。验证读取 instruction.md、分解工作、使用工具推进、审批等待与运行状态清晰，避免只承诺继续或未验证即宣布完成。
3. 在可控边界查看状态、暂停、恢复，检查会话切换后目标与进度保留。不得靠编辑持久数据伪造状态；若产品限制操作，记录限制。
4. 独立运行交付文件，检查公开数据的输出契约、精确金额、事件优先级、聚合、对账、最优解、重跑字节一致、带空格路径与变化数据。逐项区分通过、失败和未验证。
5. 核对 input 原始字节及修改时间，保存最小截图、可访问性记录、命令结果和缺陷清单。完成后检查目标终态与正常输入恢复。

## 行为与 UI/UX 对齐依据

- 明确目标、逐步推进、可恢复的进度记录、验证后完成，参考 [Anthropic 长任务 harness 实践](https://www.anthropic.com/engineering/effective-harnesses-for-long-running-agents)。
- 运行、等待授权、暂停、完成及失败状态应有可读反馈，参考 [W3C 状态消息](https://www.w3.org/WAI/WCAG22/Understanding/status-messages.html)。
- 可折叠步骤应支持明确按钮和键盘操作，参考 [W3C Disclosure](https://www.w3.org/WAI/ARIA/apg/patterns/disclosure/)。
- 将这些原则转为本次任务的验收项；不据一次测试声称全面符合行业规范或无障碍认证。

## 测试基础设施

A3S Test Web 适配器报告 `test.driver.web.version_unsupported`：安装的 A3S Browser 0.3.2，不满足 >=0.4.0,<0.5.0；当前工具集没有 A3S Test GUI MCP。本次直接通过 CUA 操作原生窗口，不注入应用脚本，不声称 ACL 套件通过。
