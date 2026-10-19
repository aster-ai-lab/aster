# T01 提交源码审计

提交：`/Users/roylin/code/tasks/T01-workspace/pipeline.py`，623 行，SHA-256 `a0f468639ddac231ba38fb8452ab7946b4651a2ee62ead69341bac042d5b7b4b`。只读审阅；未修改提交、未操作 UI、未向被测模型发消息。

结论：主体算法符合要求；存在一个已实际复现的负零格式边界缺陷。原有 8 个独立场景全部通过；新增负零场景失败，合计 **8/9 本地场景通过**。这不是官方隐藏评分。

| 检查项 | 结论与依据 |
| --- | --- |
| 单文件、仅标准库 | 导入只有 `__future__`、argparse、csv、datetime、decimal、hashlib、json、os；未见动态导入、网络、子进程或自写模块依赖。文件读取均来自参数输入目录，写入为输出 results.json。 |
| 无公开答案硬编码 | 固定量为市场、字段、比例尺度及 schema；结果由订单、事件、参考表、政策重新计算。未发现公开结果表或外部答案读取。隔离单文件及数据变体也已通过。 |
| 行互斥及三重优化 | 按 row_key 分组，每组只从上一层 DP 扩展，保证至多选一个。每个精确成本保留最大收益及最小字典序动作元组；最终升序遍历成本且只在收益严格提升时替换，保证最大收益、最小成本、最小字典序。另对提交的 optimize 做 270 个暴力枚举对照，全部一致。 |
| 金额、比率、事件 | 金额用 Decimal 精确缩放，金额与折扣落为整数；比率用整数商余数处理半入。事件先按非空行首次 ID 去重，再按时间/ID 执行；优先级依次为未知行、未知类型、已取消、非法金额、应用；`type(amount) is int` 排除布尔值。 |
| 聚合、确定性与溯源 | 原始分组集保留全取消组，指标只计 active 行。列表显式排序，JSON 键排序且禁止 NaN，原始字节计算 SHA-256；相应运行证据见主目录 `submission-evaluation.json`。 |

## 已复现 finding：对账负零丢失

位置：提交第 71–94 行 `format_scaled_int/format_fixed`、第 334–344 行 `reference_decimal`。将舍入结果转为整数后，`-0` 变成 `0`，符号消失。

合法最小输入：一个 Consumer 组，Sales=`1.000`、Profit=`-0.001`、Shipping Cost=`0`，参考表 Total_Profit=`-0.00`。在临时复制输入上执行未改的单文件 CLI，退出码为 0；对照独立 `reference.py` 的全部结构化结果，仅以下两处不同：

| 输出位置 | 独立期望 | 提交输出 |
| --- | --- | --- |
| reconciliation，segment_performance.csv / Consumer / Total_Profit，actual | `-0.00` | `0.00` |
| 同一对象，reference | `-0.00` | `0.00` |

依据：题面第 5 节要求仅在最后做 ROUND_HALF_UP，并输出固定精度字符串。本次独立 oracle 使用 Decimal `quantize(Decimal('0.01'), rounding=ROUND_HALF_UP)` 后固定精度格式化，保留 `-0.00`；没有规定将负零统一为正零。此处金额整数和 match 布尔值仍一致，但严格字符串比较失败。完整证据见 [negative-zero-evaluation.json](negative-zero-evaluation.json)；提交 SHA-256 前后相同。

## 未声称

没有穷举全部合法输入，也未做超大订单量或极大预算压力测试；当前证据不能保证官方未知隐藏用例全部通过。发现的边界未回写或修补被测提交。
