# 全球订单审计、事件处理与预算优化（107-V1）

你接手了五个市场的订单分析系统。财务要求先核对原始报表，再处理乱序到达的售后事件，生成最新经营指标，并在有限预算内选择收益最高的一组经营改进动作。

请编写 **`pipeline.py`**，仅使用 Python 标准库。运行接口固定为：

```text
python pipeline.py --input input --output output
```

程序必须创建 `output/results.json`，UTF-8 编码。输入目录、输出目录可包含空格；不依赖当前工作目录。程序可以任意组织内部函数，但最终只提交单文件 `pipeline.py`，不能依赖其他自写模块或预先生成的答案文件。

评分器会更换数据、事件和政策后重跑，并检查全部结构化结果。因此不能把公开数据的结果写死在程序中。输入文件不得修改。相同输入重复运行，`results.json` 必须字节完全相同。不要在结果中加入时间戳、绝对路径或随机值。

## 1. 输入与金额口径

`input/` 中有 11 个文件：五个 `*_orders.csv`、`product_info.csv`、`segment_performance.csv`、`shipping_by_mode.csv`、`priority_codebook.txt`、`policy.json`、`adjustments.jsonl`。

- 市场取自文件名，固定为 `Africa`、`Asia_Pacific`、`Europe`、`LATAM`、`USCA`。CSV 的 `Region` 是市场内部地区，不能用它代替市场。
- **每一条 CSV 记录均需保留**。`Order ID` 可以重复，不是本题行主键。
- 行主键 `row_key = 源文件名 + ':' + 六位补零记录序号`，例如 `Africa_orders.csv:000001`。序号从 1 开始，只计数据记录、不计表头和空行，不是物理文本行号。
- 金额统一为 **千分之一货币单位的整数**，字段后缀为 `_milli`。例如 `1.23 → 1230`，`1.237 → 1237`。输入金额最多三位小数，必须精确转换，不能先舍入到分。
- 折扣统一为百万分之一整数，后缀 `_ppm`：`0.2 → 200000`。输入折扣最多六位小数。
- 比率输出基点整数，后缀 `_bp`：`0.1689 → 1689`；用 `ROUND_HALF_UP` 四舍五入。分母为零时输出 JSON `null`。
- 本题定义 **贡献利润 = Profit − Shipping Cost**，仅为本改编任务的统一核算假设，不声称是原数据的真实会计口径。政策改进收益也按下面的模拟公式定义。
- 输入日期为 ISO `YYYY-MM-DD`；事件时间为可按字符串排序的 UTC ISO 时间，格式一致。政策成本均为正整数、预算为非负整数，比率为 0–10000。订单数、政策数值和事件内容可能变化，文件格式不变。

## 2. 原始明细标准化

用 `Product ID` 关联唯一的产品目录。`normalized_rows` 按 `row_key` 升序，是对象列表；每个对象必须且只包含：

| 字段 | 类型及来源 |
|---|---|
| row_key | 上述行主键，字符串 |
| source_file, source_row | 源 CSV 文件名；记录序号整数 |
| order_id, product_id | Order ID；Product ID，字符串 |
| market | 文件名对应市场 |
| category, sub_category | 产品目录 Category、Sub-Category |
| segment, ship_mode, priority | Segment、Ship Mode、Order Priority |
| order_date, ship_date | 原始 ISO 日期字符串 |
| delivery_days | Ship Date − Order Date 的自然日数，整数 |
| quantity | Quantity，整数 |
| sales_milli, profit_milli, shipping_milli | 三个原始金额的精确整数 |
| discount_ppm | 原始折扣的精确整数 |

不要把订单行压缩成订单级别，不能用汇总参考表代替原始明细计算。

## 3. 售后事件账本

`adjustments.jsonl` 每条非空记录包含 `event_id`、`row_key`、`timestamp`、`type`、`amount_milli`。

先按文件中非空记录的顺序去重：相同 `event_id` 仅保留第一次出现的记录，即使后面的载荷不同也忽略。再将保留的事件按 `(timestamp, event_id)` 升序执行。审计表最终按原始非空记录顺序输出。

状态初始为有效 `active=true`，金额初始取标准化明细。每个唯一事件按下列**优先级顺序**确定状态，命中即停止：

1. 行主键不存在：`unknown_row`。
2. 类型不是 `cancel`、`refund`、`shipping_credit`：`unknown_type`。
3. 目标行此前已取消：`order_cancelled`。
4. 金额不符合下述规则：`invalid_amount`。
5. 否则：`applied`，应用变更。

被去重的原始记录状态始终为 `duplicate_event`，不参加上述处理。

事件规则：

- `cancel`：金额必须为整数 0；将 `active=false`，销售额、利润、运费均设为 0。
- `refund`：金额必须是正整数，且不超过**当前**销售额；销售额和利润同时减去该金额，运费不变。
- `shipping_credit`：金额必须是正整数，且不超过**当前**运费；仅运费减去该金额，利润不变。
- JSON 布尔值不视为整数。拒绝的事件不改变任何数据。

`event_audit` 是对象列表，每个对象只含 `input_line`（从 1 开始的非空记录编号）、`event_id`、`row_key`、`status`。

`effective_rows` 是标准化行的副本，按 `row_key` 升序，仅增加布尔字段 `active`，并更新三个金额。取消行仍保留在列表中，数量和其他属性保持原值。

## 4. 最新经营指标

定义一组通用指标：

| 字段 | 计算方法（仅统计 active=true 的行） |
|---|---|
| row_count | 行数 |
| quantity | Quantity 之和 |
| sales_milli, profit_milli, shipping_milli | 对应金额之和 |
| contribution_milli | 利润之和减运费之和 |
| profit_margin_bp | ROUND_HALF_UP(利润之和 / 销售额之和 × 10000) |
| contribution_margin_bp | ROUND_HALF_UP(贡献利润之和 / 销售额之和 × 10000) |
| high_discount_count | discount_ppm 严格大于 200000 的行数，与政策阈值独立 |
| negative_contribution_count | 利润严格小于运费的行数 |

`aggregates` 是对象，包含五个列表：

- `market`：按 `market` 分组。
- `market_category`：按 `market, category` 分组。
- `segment`：按 `segment` 分组。
- `ship_mode`：按 `ship_mode` 分组。
- `market_month`：按 `market, month` 分组；`month` 是 Order Date 的 `YYYY-MM`。

每个聚合对象只包含分组字段及上述全部通用指标。分组集合依据**事件处理前**明细中实际出现的组合确定，即使某组合全部取消也输出零指标，比率为 null。每个列表按分组字段元组的字符串字典序升序。

`summary` 包含全部明细的通用指标，另外增加：

- `input_row_count`：原始数据记录总数。
- `unique_order_id_count`：原始明细中不同 Order ID 的数量。
- `duplicate_order_id_extra_rows`：总行数减不同 Order ID 数量。
- `cancelled_row_count`：最终 active=false 的行数。
- `event_status_counts`：各审计状态到出现次数的映射，只包含出现过的状态。

## 5. 原报表对账

必须用**事件处理前**的标准化明细重算两个参考汇总表。参考表可能过期或有误，不能直接假定它正确。

- `segment_performance.csv`：按 Segment 核对 `Order_Count`（0 位）、`Total_Sales`（2 位货币单位）、`Total_Profit`（2 位货币单位）、`Profit_Margin`（利润 / 销售额 ×100，2 位百分数）、`Avg_Discount`（原始折扣的行平均值，4 位）。
- `shipping_by_mode.csv`：按 Ship_Mode 核对 `Order_Count`（0 位）、`Total_Shipping_Cost`（2 位货币单位）、`Avg_Shipping_Cost`（运费的行平均值，2 位货币单位）、`Avg_Delivery_Days`（自然日的行平均值，2 位）。

对参考表中的每个分组和字段输出一个对象。先以精确值完成计算，仅最后按规定小数位 `ROUND_HALF_UP`。没有相关行时金额、平均数和比率按 0 计算，仅限本对账步骤。

`reconciliation` 每个对象只含 `source_file`、`group`、`field`、`actual`、`reference`、`match`。actual 与 reference 都是固定小数位的字符串，match 是两者是否完全相同的布尔值。按 `(source_file, group, field)` 升序。

## 6. 预算约束下的最优行动方案

根据最终有效明细和 `policy.json` 为每行生成以下候选：

### shipping_switch

当 Ship Mode 不是 `Standard Class` 且优先级属于政策 `shipping_priorities`：

- `gain_milli = floor(当前运费 × shipping_saving_bp / 10000)`。
- `cost_units` 取 `shipping_cost_by_market[market]`。

### discount_review

当折扣严格大于政策 `discount_threshold_ppm`，且当前贡献利润严格小于 0：

- `gain_milli = floor(当前销售额 × discount_recovery_bp / 10000)`。
- `cost_units` 取 `discount_cost_by_market[market]`。

收益不大于 0 的候选不输出。`action_candidates` 每个对象只含：

`action_id`（`row_key + '|' + 类型`）、`row_key`、`type`、`cost_units`、`gain_milli`。

候选按 `action_id` 字典序升序。两个候选可能属于同一订单行。

从全部候选中选出一个集合：总成本不超过 `budget_units`；**每个 row_key 至多选一个候选**。允许不花完预算，也允许不选。

依次使用以下目标，不能用贪心近似：

1. 最大化总 `gain_milli`。
2. 收益相同，最小化总 `cost_units`。
3. 收益和成本都相同，将各方案的 action_id 升序排列，选择字典序最小的列表（按 Python 字符串和列表比较规则）。

`selected_actions` 是入选候选的完整对象列表，按 action_id 升序。`optimization` 只含：

- `budget_units`、`spent_units`、`gain_milli`，整数。
- `selected_action_ids`，action_id 升序列表。
- `projected_contribution_milli`，当前总贡献利润加最优模拟收益。

动作仅用于场景评估，**不回写** effective_rows 或 aggregates。

## 7. 输出契约与溯源

results.json 顶层必须且只包含：

```text
schema_version = "107-v1"
normalized_rows
event_audit
effective_rows
aggregates
reconciliation
summary
action_candidates
selected_actions
optimization
input_sha256
```

`input_sha256` 为输入目录全部普通文件的“文件名 → 文件原始字节 SHA-256 小写十六进制摘要”映射。不要对内容重编码再计算摘要。

按上述字段类型输出；不要把整数写成浮点数，不要输出 NaN/Infinity，不要添加未经要求的字段。JSON 对象的键顺序不限；列表顺序必须符合规定。

## 完成标准

提交 `pipeline.py`，并对公开 input 成功生成 output/results.json。评分会检查：公开数据全流程、重复执行一致性，以及修改预算/政策、数据/参考表、乱序重复事件、最优解与并列解等隐藏情形。
