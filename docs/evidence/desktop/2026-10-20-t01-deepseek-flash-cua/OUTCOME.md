# AsterDream / DeepSeek Flash：T01 CUA 评测

评测已完成。模型交付原稿通过 **8/9 个本地独立场景**；公开数据完整结果正确，负零字符串格式场景失败。未修改模型提交来补分，不代表官方隐藏评分。

本次输入来自仓库 `tests/T01`，不是历史评测工作区。任务说明与 11 个输入文件先逐字节复制到 `target/evaluations/T01-deepseek-flash-20261020-cua`；结束及评分后的 SHA-256 检查均确认原始输入和副本未变。

## 应用、模型与控制方式

- 受测应用是已配套构建的 `AsterDream-TaskOptimization-R5.app` 开发包。现有 R2 窗口处于停止状态；正常退出它后启动 R5，保留历史产物和安装版。
- 通过 CUA 原生应用 API 启动、操作和观察；使用窗口 `Raise` 置于前台，使用 AX 控件及应用定向键盘/剪贴板输入。没有使用屏幕坐标点击、移动鼠标或拖拽。工具未暴露独立的命名模式开关，本次以这些操作实现前台、不移动鼠标的控制方式。
- UI 选中 **DeepSeek Flash**；实际 ACP 参数为 `--model deepseek/deepseek-flash`，原生会话模型为 `deepseek::deepseek-flash`，运行配置端点为 `https://api.deepseek.com`。
- 实际 ACP SHA-256：`87e1ce16cc44e4efa8ea6b0fdeb2572079c6fc48a89f4e1e4ce077a1474b1780`。Host、ACP、Office 三个二进制在评测结束后均与启动前一致。
- 保留逐次确认审批；只批准本轮输入读取、程序编写、运行及自建测试产物清理。没有切换为放开权限模式，没有提供独立 oracle 或答案，也没有追加纠错提示。

设置与运行身份见 [setup.json](setup.json)、[runtime-identity.json](runtime-identity.json)。界面证据见 [发送前摘录](cua-before-send.txt)、[最终答复摘录](cua-final.txt)、[最终截图](cua-final.png)。证据仅保留本轮任务内容。

## 执行结果

原生会话 `01a1567c-5be8-7724-99c2-638cee0a066d` 于北京时间 **07:25:53.279–07:46:11.412** 执行，墙钟时间 **20 分 18.133 秒**，包含审批等待、CUA 观察及状态冲突重读时间，不能作为纯模型推理时延或性能对比。

共 84 次模型调用、87 个工具结果：75 个工具结果为成功、12 个失败、64 次审批答复。成功工具结果数量不等于通过的测试数量。失败包括 6 次有效参数权限拒绝、2 次跨调用临时文件不存在、1 次补丁格式错误、1 次 BusyBox `ls` 选项不支持及 2 次模型自建断言错误。模型自行恢复；期间新优化器曾产生错误结果，模型通过回归发现并自行修正。全程没有 `max_turn_requests` 停止。

最终 Host 为 `done`、助手消息为 `success`；Core 的终态是 `narrative`，`forced_stop` 为空，不将它表述为原生 `verified`。交付正确性由下述独立运行另行核验。运行统计见 [runtime-final.json](runtime-final.json)。没有可用的 token/费用总量，未估算。

模型实际运行 `python3 pipeline.py --input input --output output`，退出码 0。公开输出完整结构与独立 oracle 一致：1000 条原始明细、987 条 active 明细、13 条取消、48 条事件、31 项对账全部匹配、252 个候选、16 个入选动作；花费 64/64，最优收益 3,165,191 milli，预测贡献利润 21,017,339 milli。

## 独立验收

复用此前依据同一任务说明编写的九场景验收器，未把其源码、预期结果或数值提供给被测会话。仅复制提交单文件，在隔离目录中用 Python `-I`、含空格的输入/输出路径及无关当前目录执行；严格比较全部 JSON 字段、类型、列表顺序和值。八个通过场景均连续运行两次，输出字节一致，输入未变。

| 场景 | 结果 |
| --- | --- |
| 公开数据完整流程 | 通过 |
| 零预算 | 通过 |
| 修改政策、成本及预算 | 通过 |
| 修改明细、参考表、空行及原始字节溯源 | 通过 |
| 每行互斥动作及事件优先级/金额边界 | 通过 |
| 字典序并列解 | 通过 |
| 同收益最小成本 | 通过 |
| 贪心反例 | 通过 |
| 负零固定小数位字符串 | **失败** |

完整结果见 [submission-evaluation.json](submission-evaluation.json)。实际应用生成的公开 JSON 也独立比对全部结构，而非仅检查汇总数字；另对冻结提交的优化器做 270 个穷举对照实例，全部通过。见 [additional-verification.json](additional-verification.json)。

失败复现只需一条 Consumer 明细：Sales=`1.000`、Profit=`-0.001`、Shipping Cost=`0`，参考 Total_Profit=`-0.00`。规定精度及 ROUND_HALF_UP 下应保留 `-0.00`；提交把 `reconciliation.actual` 和 `reference` 都写成 `0.00`。程序退出码仍为 0，其余结构一致。独立复现两次，差异相同、字节一致、输入未变。见 [negative-zero-details.json](negative-zero-details.json)。

根因位于提交的 `format_fixed`、`quote_fixed`：先转整数，再由 `fixed_from_int` 判断负号，零值因此丢失符号。原稿已冻结，未修复。模型最终答复中的“未覆盖的部分：无”不能代替独立验收结论。

## 交付与复跑

- [冻结的 pipeline.py](submission/pipeline.py)，761 行、27,217 字节；SHA-256 `5695594a780841c343d053cf45dc113008a96927eccb45e0c6caeaa9d0de73ed`。
- [应用生成的 results.json](submission/output/results.json)，1,439,900 字节；SHA-256 `1410d96bf12818691780e94e03e6c58001060c9af862d092736f255b58c097e8`。
- 工作区内原始交付仍在 `target/evaluations/T01-deepseek-flash-20261020-cua/pipeline.py` 和 `output/results.json`，与冻结副本一致。

源码审阅仅发现标准库静态导入；所有文件读取来自给定输入，唯一程序写入为输出目录的 `results.json`。没有发现动态导入、网络/子进程调用、预置答案或其他自写模块依赖。优化器内部使用 64 位整数数组；本轮未穷举极端数值规模。

在仓库根目录复跑：

```sh
python3 docs/evidence/desktop/2026-10-19-t01-deepseek-flash/oracle/evaluate.py \
  --pipeline docs/evidence/desktop/2026-10-20-t01-deepseek-flash-cua/submission/pipeline.py \
  --input tests/T01/input \
  --report /tmp/t01-r5-recheck.json
```

预期退出码 1，表示九例中负零场景失败。未运行官方隐藏评分、全仓 CI 或多次模型统计实验；没有修改产品源码、提交代码、发布或替换安装版。AsterDream 保持在本轮最终答复界面。
