# 现有机制与设计取舍

**日期**：2026-11-01。状态：静态代码核对；三项缺口尚待动态复现。

## 复用清单

| 责任 | 现有来源 | 本次处理 |
| --- | --- | --- |
| 实际会话和子任务 | `crates/code/core/src/agent_api/conversation_runtime/owned_fact.rs`、`child_run.rs` | 两条路径均使用事实执行；借用的 AgentLoop 不等于执行旧消息循环。 |
| 工具准入 | `crates/effect/src/coding.rs::prepare_tool_batch` | 保留唯一调度准入 owner，修正准备后覆盖终态的方式。 |
| Core 工具准备 | `fact_control/executor_tools.rs`、`receipts.rs::prepare_tools` | 继续负责权限、钩子、参数改写和原 Run 绑定。 |
| 机械新信息 | `fact_control/receipts/novelty.rs` | 复用生效参数和原观察，不复制一套 hash 规则。 |
| 拒绝与状态循环 | `fact_control/receipts/refusal.rs` | 保留既有终止和状态连续性判据。 |
| 跨 Run 进展 | `fact_control/goal_evidence.rs`、`goal_control/facts/settle.rs` | 保留 V3 新信息、净状态、已接受步骤和不确定性减少。 |
| 一次重规划机会 | `goal_control/facts/run/continuation.rs` | 已存在；取消独立恢复计数器方案。 |
| 提示与压缩 | `fact_control/history_seed.rs::repeated_result_note`、`compaction_carry.rs` | 提示是展示；统计不能依赖提示或摘要。首轮不加常驻账本。 |
| 用量与重复上下文 | `harness_evidence/input.rs`、`agent_api/session_facade.rs::context_report` | 直接使用现有计量，不建设另一套观测服务。 |
| 协议验证 | `crates/code/acp/tests/unattended_e2e.rs` | 已有拒绝循环、写入循环和 Goal 停滞断言，按需补压缩场景。 |

## 静态发现与复现条件

| ID | 静态发现 | 最小复现与反证 | 当前结论 |
| --- | --- | --- | --- |
| R1 | `prepare_tool_batch` 调用 runner.prepare 后，以完整空效果覆盖重复拒绝。Core 的准备可能已执行输入钩子。 | 使用真实 Core 钩子写 canary 后拒绝，再触发调度器重复分支；核对原回执的 Incomplete 是否丢失。对照：拒绝在钩子前确定时，钩子应为 0 次。 | STATIC；T002 复现后才修。 |
| R2 | `duplicate_nudge` 扫描 retained messages；压缩替换消息后，早期重复可能不再计数。 | 阈值 3、足够大的稳定只读结果、每次只保留一个工具对、轮数 0、宿主外层 watchdog。必须证明实际压缩发生。 | STATIC；不得只测已触发拒绝后的压缩。 |
| R3 | Core 已识别新观察，effect 参数计数仍可能拒绝同参数的新外部观察。 | 无文件变化、7 个不同结果、同参数、不同调用 ID。对照：固定结果和 A/B 回访仍有界。 | STATIC；T002 固定正确行为。 |

复现失败时记录实际原因和源快照。删除或改写被推翻的修复任务，保留调查记录。不得让错误的原判断成为产品要求。

## 已有测试必须保留的含义

- `agent_api/goal_facade/tests/execution/refusal.rs`：阈值 3 时稳定只读工具实际执行 3 次，重复拒绝后模型调用有界，无成功 End。
- `refusal/productive.rs`：拒绝后采取不同方法并获得新观察，仍能继续；系统已给模型改变做法的机会。
- `refusal/mutation.rs`：A/B 往返会停止，持续新状态和有界必要变更仍允许执行。
- `agent_api/tests/fact_log/revalidation.rs`：真实字节变化允许第 4 次相同验证，同字节重写不允许。
- `execution/novelty.rs`：不同 Run 的相同观察不算新进展，重开会话后停滞保持。
- `execution/novelty/productive.rs`：相同工具观察伴随真正被接受的步骤时，Goal 仍支持推进。
- `execution/automatic.rs`：停滞后已接受的新计划只获得一次预算内机会。
- `fact_control/receipts/tests/novel_information.rs`：宿主提示、调用身份和工具自报摘要不能制造新信息。

## 接线约束

1. 不把 Core 私有结果封装解析下沉到 effect。
2. 不让 effect 反向依赖 Core。共享部分只能使用通用类型和宿主规范化后的观察身份。
3. 保持 `ToolRunner::prepare`、`ToolPreparationRequest` 和 `PreparedToolExecution` 的现有调用/字面量构造兼容。
4. 若现有接口不足，只允许必要的、有默认实现的 trait 接线扩展。正常路径仍委托现有准备方法；拒绝路径不得先运行未知钩子再声称没有效果。
5. 具体接口由 T002 的复现及适配器清单确定。必须用旧调用形式的编译/运行测试验明兼容，不能仅以“增加方法”推断兼容。
6. 观察缺失或不完整时保持现有保守处理，不以 `None` 冒充新信息，不重放不确定操作。

## 取消或后置的方案

| 方案 | 处理 | 原因 |
| --- | --- | --- |
| 新 LoopGuard / RecoveryController / 独立恢复额度 | 取消 | 与既有调度、拒绝终止及 Goal continuation 重叠。 |
| 独立持久化动作账本 | 取消 | 原始事实已存在；只保留可重建派生状态。 |
| 每步完整扫描事实日志的第二个检测器 | 取消 | 会重复现有 fold；在原投影维护统计。 |
| 根据命令关键词识别测试、等待或进展 | 取消 | 违反语义判定边界，并会误判合法工具。 |
| 普遍复用缓存结果、跳过重复工具执行 | 取消 | 前次相同结果不能证明本次仍相同。 |
| 常驻恢复提示、额外裁判模型 | 后置 | 当前无收益证据；先修统计和准入。 |
| RL 样本导出和 DPO 训练 | 后置 | 主事实路径输入证据主要是摘要/哈希；旧循环记录器不能直接证明完整训练上下文。 |

## 测量限制

轨迹中的重复字节、模型输入里的重复内容和实际被计费 token 是三种指标。分别使用 `tool_result_weights`、`ToolResultContextUsageV1` 和 provider usage；不得将其中一种替代另一种。真实模型训练、模型价格或账单费用均不属于本轮已验证事实。


## 2026-11-02 复核：研究方法和运行时修复各自的边界

[DeLoop 官方仓库](https://github.com/cskyl/deloop) 当前仍为项目页，挖掘/评测代码、数据和训练适配器尚未发布。其主要方法用同一轨迹中的重复动作和后续破环动作构建偏好对，再做DPO；它没有提供可直接嵌入Aster的运行时控制器。相同调用得到新观察不算循环。仓库也报告显式动作记录仍可能循环，因此“能找回历史”不能直接推出“会高效使用历史”。此处是研究含义的概述，Aster收益只以自己的冻结测试为准。

Aster已修复的事实序号分组、跨压缩统计和新观察准入继续是唯一对应owner，不复制论文检测器或新增LLM裁判。R5真实归档恢复发生了19次往返，且两项候选长数据任务均失败；恢复入口接通只证明可访问，不能充当长程完成/效率证据。R6先处理实际已复现的归档search契约缺陷，不增加恢复预算。

R6静态测量真实SDK基础注册表：16工具，描述和参数值合计28935字节；无真实模型调用的隔离SDK Session：19工具，合计36293字节。均不是原ACP的23工具完整面，也不是native token计费。实际原ACP的43761字节工具面和18970字节system仍以relay为准。测量代码和输入身份在 `docs/evidence/code/2026-11-02-artifact-search-contract/`；未读取用户ACL来做静态测量。

后续候选先考虑收敛可恢复数据的呈现：当前归档只含被丢弃窗口，旧观察需要追逐旧摘要中的链接。若R6不能排除这一开销，可在已有压缩时的原日志读取中，复用现有checkpoint公开投影，生成一个有界、可直接搜索的原始公开会话视图。只改变cache投影，原事实/调度/权限/完成判据不变；不新建索引、账本、控制器或永久保留根。必须先用跨两次有损压缩的独立值做red，证明最新视图不能直接恢复早期原文，再实施并沿原任务测试。当前这是待证候选，不能标为已优化。


后续优先级更新：已有 `ToolPresentationProfileV1::Code` 正是固定工具面开销的既有owner。隔离SDK同源19定义Adaptive/Direct各36293字节，Code6784字节。先验证这个已有模式的真实正确性与native计量，再决定是否仍有必要扩展原文cache投影；不在此基础上叠加另一个selector，也不从两文件控制推出长程收益。R6两项原长数据首次结果已保留且均失败。
