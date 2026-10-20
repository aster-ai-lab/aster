# 自动目标续跑的停滞边界

对应 [GitHub issue #1](https://github.com/aster-ai-lab/aster/issues/1)。修复前已用原生 Core API 重现：只读步骤被接受后，连续无进展计数被清零；同样的事实记录重新打开后仍允许继续执行。

## 修复

`evidence_for_goal_step` 从当前原始 Run 的有效工具收据折叠 mutation ledger。接受步骤时，只有该 ledger 包含工作区改动才重置连续无进展计数。只读验证仍可满足计划步骤和完成条件，但不再增加自动续跑额度。没有增加新的持久化事实字段。

默认 `max_slices = 8`、`max_no_progress_slices = 2`、ACL 配置优先级、手动目标控制及 completion gate 的验证规则均保留。没有恢复隐式全局工具数或 turn 数上限。

## 验证

| 层次 | 结果 | 证据 |
| --- | --- | --- |
| 缺陷回归先失败 | 修复前确实允许只读步骤重置停滞计数 | [red.log](red.log) |
| 同一回归修复后 | 通过；包含内存/文件事实存储及重新打开 | [green.log](green.log) |
| Core 目标执行 | 49 项通过；包含真实写入被接受后恢复自动续跑的正向用例 | [core-goal-tests.log](core-goal-tests.log) |
| Core 目标策略 | 42 项通过 | [core-policy-tests.log](core-policy-tests.log) |
| ACP 二进制端到端 | 7 项通过；启动真实 unattended ACP 进程，使用 loopback 模型服务及临时目录 | [unattended-e2e-full-r2.log](unattended-e2e-full-r2.log) |
| 中文目标状态 | 26 项通过、2 项忽略；按宿主显式 `zh-CN` 元数据选择文案，保留英文 CLI 行为 | [goal-language-tests.log](goal-language-tests.log) |

ACP 端到端新增场景确认：目标写入 `answer.txt`、完成一次只读核验后产生成功终态并写出结果文件；15 秒测试截止前返回 `EndTurn`，没有重跑同一核验，也没有请求权限。测试直接统计模型服务收到的核验工具请求，避免把协议事件数量误当成模型执行 turn 数。

完整 unattended 套件中移除了一个与已删除的旧隐式上限相矛盾的断言，保留原有成功结果及终态验证。没有为了让测试通过而恢复旧上限。上述测试组有重叠，不应将数量简单相加当成独立用例总数。

## Explicitly not claimed

没有重放 issue 原始数小时会话，没有使用付费真实模型复现耗时循环。确定性的二进制端到端使用模型替身，停滞计数缺陷通过真实 Core 事实与重新打开回归验证。ACP 协议进程在夹具清理时终止，未断言它在一次 prompt 后自行退出。未改变单个 slice 内部的执行截止机制，未宣称解决一切模型内部循环。完整工作区 CI 尚未通过，代码尚未提交、推送或在 GitHub 关闭 issue。
