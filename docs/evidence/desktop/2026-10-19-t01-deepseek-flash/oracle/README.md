# T01 独立验收 oracle

此目录仅供评测方使用。不得向被测 AsterDream 会话提供脚本、期望数据或数值答案。未发现 T01 自带评分器或官方预期结果；这里是依据 `T01/instruction.md` 编写的独立验收工具，不代表官方隐藏评分。

## 产物

- `reference.py`：从输入重新计算全部 11 个顶层字段；金额和比率使用整数及 Decimal；预算使用每行至多一个动作的精确动态规划。
- `expected_public.json`：公开 input 的完整预期结构，不是交给被测模型的输入。
- `evaluate.py`：只复制提交的单文件，在包含空格的隔离目录内，从无关工作目录执行；逐项严格比较字段、类型、值及列表顺序。
- `harness_selfcheck.json`：评测工具自身的 8/8 冒烟测试记录；使用临时 oracle 包装程序，**不是 AsterDream 的成绩**。

2026-10-20：将已发现的合法 signed-zero 反例加入 `evaluate.py`，当前共九例。
历史八例报告和模型原提交保持不变。新增记录在
`docs/evidence/code/2026-10-20-task-orchestration/oracle-*-nine.json`；
`oracle-selfcheck-nine.json` 是工具自检，`oracle-baseline-nine.json` 是冻结
模型原提交的 8/9 复核。后续优化实验统一使用这九例。

## 逐项验收

| 要求 | 证据与判定 |
| --- | --- |
| 仅提交 `pipeline.py`，仅标准库 | 只复制这一个文件后运行 Python `-I`；AST 检查显式导入。另需人工检查动态导入、自读外部答案或其他取巧方式。 |
| CLI、空格路径、无关当前目录 | 9 个用例均传绝对输入和输出路径，路径含空格，cwd 与脚本、输入均不同。 |
| 输入保持不变 | 每次执行前后对输入文件做 SHA-256；还对原始公开 input 做前后校验。 |
| 逐行标准化 | 对所有原始记录的完整对象逐字段、逐类型比较；变体加入重复 Order ID、空行、三位金额、六位折扣。 |
| 事件去重、排序、状态优先级 | 公开 48 条事件；合成变体使用乱序、不同载荷重复、当前余额限制、布尔和浮点金额、未知行/类型与取消状态优先级。 |
| 最新聚合、summary、取消后保留空组 | 比较全部 5 个聚合列表及 summary；合成输入包含整组取消、空市场、有效零销售额组。 |
| 事件前对账 | 比较每个参考分组字段的固定精度字符串与 match；变体修改数据和参考表，加入无对应原始行的分组，并检查 `-0.001` 四舍五入后的 `-0.00`。 |
| 动作生成及精确最优解 | 比较所有候选、入选对象和 optimization；变体覆盖政策、零预算、同一行二选一、收益同成本择少、字典序并列、贪心反例。 |
| SHA-256、JSON 契约 | 逐顶层键及对象字段严格比较；拒绝重复 JSON 键、NaN、Infinity；布尔和整数不视为同型。变体额外加入含非 UTF-8 字节的普通文件，验证原始字节哈希。 |
| 字节一致性 | 每个用例连续运行两次，比较 results.json 原始字节。 |

## 使用

```sh
python3 docs/evidence/desktop/2026-10-19-t01-deepseek-flash/oracle/evaluate.py \
  --pipeline /absolute/path/to/submitted/pipeline.py \
  --input /Users/roylin/code/tasks/T01/input \
  --report docs/evidence/desktop/2026-10-19-t01-deepseek-flash/submission-evaluation.json
```

每个子进程最多 90 秒。报告记录提交 SHA-256、运行退出码、耗时、输出摘要、前 20 处差异与确定性。当前比较结果必须 9/9 通过、输入未变、显式导入均为标准库，方可称“通过本地验收”。

## 公开输入基线

- 原始行 1000；Order ID 去重后 867；多出的重复行 133。最终有效行 987，取消行 13。
- 事件：applied 37，duplicate_event 6，invalid_amount 2，order_cancelled 1，unknown_row 1，unknown_type 1。
- 有效 sales/profit/shipping：1,688,849,344 / 285,536,004 / 267,683,856 milli；贡献利润 17,852,148 milli。
- 候选 252；选中 16；预算及花费均 64；最优收益 3,165,191 milli；预测贡献利润 21,017,339 milli。
- 公开参考汇总的 31 个核对字段全部匹配。

## 自检与边界

`reference.py --self-test` 检查金额、负数半入、零分母、严格类型，以及 270 个动态规划和暴力枚举最优解的一致性。合成输入另对预设最优动作和全部事件状态次数做断言。`harness_selfcheck.json` 证明完整验收路径在系统 Python 3.9.6 上运行成功。

本工具没有穷举全部合法输入，不宣称通过官方未知隐藏用例。AST 导入检查无法完整证明不存在动态导入或其他外部依赖；提交源代码仍需人工审阅。没有运行提交前，工具自检不构成对 AsterDream 能力的结论。
