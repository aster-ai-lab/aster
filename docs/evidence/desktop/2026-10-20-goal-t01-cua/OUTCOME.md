# AsterDream /goal 与 T01

测试计划见 [PLAN.md](PLAN.md)。测试原生 AsterDream R5，使用 DeepSeek Flash、逐次确认及仓库内 `tests/T01`。交付程序通过独立验收；原生目标最终停在 `active / needs_input`，未宣称目标控制器完成。

## 已验证的交付

AsterDream 通过 `/goal` 读取任务、创建 `pipeline.py` 并运行生成 `output/results.json`。测试者没有代写或修订交付程序。提交文件 SHA-256 为 `ab17057be7a679c88be1d28d4d67456dd03dd7b3af10dbe557aee4cc95d2d2eb`。

- 公开输入的全部结构化结果与独立实现逐字段一致，见 [original-public-validation.json](original-public-validation.json)。1,000 条原始行、867 个不同订单 ID、48 条事件、13 条取消行；最优成本 64，收益 3,165,191 milli，入选 16 个动作。
- [独立验收程序](verify_t01.py)在读取交付源码前编写。隔离目录中只复制单文件，校验精确字段集合、类型、所有列表顺序、金额、事件、聚合、对账、候选及最优解。小规模候选再用穷举校验测试 oracle。
- [首批七组](submission-initial-validation.json)及[全取消分组与同时间事件](additional-edge-validation.json)全部通过；每组运行两次，16 次运行的同输入结果字节一致。输入、输出目录均包含空格，工作目录与程序及输入目录分离。
- 变体覆盖零预算、改变政策、三位金额及六位折扣、重复订单 ID、乱序及重复事件、布尔/非整数金额拒绝、事件错误优先级、并列收益的成本及字典序规则、空输入、全部取消后保留零分组与 null 比率。
- 未发现第三方导入；Python 3.13 独立验证通过，[Python 3.9.6 公开数据重跑](python39-validation.json)也通过，输出字节相同。

## 已观察的行为

- `/goal status` 不推进目标；停止当前执行后，目标保持 active、未被误标完成。`/goal pause`、`/goal resume` 的原生状态变化已观察到。
- 暂停后刷新桌面前端，会话及暂停状态恢复，见 [07-paused-after-reload.png](07-paused-after-reload.png)。恢复后继续读取输入并实现程序。
- 模型建立五步结构化清单。授权等待时清单及停止入口仍可见，见 [09-structured-progress-write-approval.png](09-structured-progress-write-approval.png)。
- 普通工具继续逐次确认。一个包含删除操作的临时目录命令被冻结权限策略拒绝；模型换成不含删除的方案继续，随后空格路径自测成功，见 [10-selftest-recovered-refusal.png](10-selftest-recovered-refusal.png)。
- 原生准入事实中的延续策略为自动执行、最多 8 个 slice、连续无进展上限 2、24 小时截止。没有测试耗尽这些上限的行为。
- 交付及自测之后，重复验证遇到冻结权限策略拒绝。测试者拒绝了最后一次重复核验请求；`/goal resume` 随后返回 `needs_input`，没有继续执行，见 [13-t01-needs-input.png](13-t01-needs-input.png) 和 [14-resume-still-needs-input.png](14-resume-still-needs-input.png)。停止并未被当作成功，目标也没有自动清除。
- 拒绝面板填写的补充说明没有传至 ACP：当前授权结果只携带允许或拒绝布尔值。这是宿主信息传递的限制，不能据此认定模型忽略了说明。

## 发现与建议

| 优先级 | 实测问题 | 建议 |
| --- | --- | --- |
| P1 | 暂停命令已持久成功，当前消息区却出现空白回复；刷新后显示暂停文本。 | 修复快速命令回复的流事件与持久结果对账，完成回执不能留下空白消息。 |
| P1 | `/goal resume` 已恢复执行，输入区目标摘要仍显示“已暂停”；切换回会话后恢复。 | 使用实际目标状态作为显示来源，并同步失效缓存；不能仅重放历史用户命令来判断状态。 |
| P2 | 公开输出及多项自测已经完成时，清单长时间保持 1/5、实现进行中。 | 在阶段完成时更新结构化进度。保留真实状态，避免根据工具名称或文字猜测完成。 |
| P2 | 正文反复展示 `Durable goal`、`Plan r1`、`Next: advance`、slice ID 等内部状态；暂停恢复主要依靠命令。 | 提供紧凑目标行，呈现当前状态、暂停/恢复和验证结果，技术细节放入按需展开的记录。 |
| P2 | 临时自测结果与正式结果都展示为 `results.json` 工作产物。 | 正式交付突出项目相对路径，区分临时核验文件，避免同名结果混淆。 |

上述是本次任务范围内的评估，依据[长任务的持久进度与验证实践](https://www.anthropic.com/engineering/effective-harnesses-for-long-running-agents)、[W3C 状态消息](https://www.w3.org/WAI/WCAG22/Understanding/status-messages.html)及[折叠交互](https://www.w3.org/WAI/ARIA/apg/patterns/disclosure/)，不是行业认证分数。

## 本轮同时修复的规划界面

- 完成的计划从输入区退出，正文历史保留。
- 新建任务隐藏旧会话计划及审阅操作，不继承已完成计划。
- 开启规划模式不再额外显示说明行；开关原有帮助说明保留。
- [实机新任务、规划开关开启截图](08-clean-home-plan-on.png)确认界面干净。[可访问性记录](08-clean-home-plan-on.ax.txt)确认不存在旧计划与额外说明行。
- 三组相关测试 264 项通过，见 [日志](planning-cleanup-tests.log)。前端生产构建通过；修改文件的 oxlint、格式、diff 检查通过；ESLint 无错误、三条既有警告。i18n 同步仅移除两条已不用的说明文案，校验通过。
- Work 文档结构与索引检查通过；链接及 frontmatter 检查被既有失效引用阻挡，见对应日志。没有修改无关文档使门禁变绿。

## Explicitly not claimed

未使用官方隐藏评分器，未宣称所有未知数据规模通过。未重建宿主和 ACP、未跑完整工作区 CI、未声称全仓 lint 或类型检查通过。A3S Test 的 Web 适配器版本不兼容且没有配置 GUI MCP，本次通过直接 CUA 测试，不声称 ACL 套件通过。目标状态同步的缺陷尚未修复。代码尚未提交或推送。

后续中文状态与目标摘要修复的证据单独记录在 [更新及界面修复报告](../../work/2026-10-20-updater-repair/OUTCOME.md)。本报告中的 R5 原始观察不因此改写为新版通过。
