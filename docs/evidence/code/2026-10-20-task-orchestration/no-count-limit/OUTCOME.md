# 默认交互任务不设次数上限

本轮请求已完成源码修改、配套构建和 Work → ACP → Core 联调。未提交代码，未替换或启动现有桌面应用，未新增付费模型调用。

## 生效范围

| 入口 | 当前行为 |
| --- | --- |
| Aster 终端 | 原本默认 `max_turns=None`，保持不变 |
| ACP / AsterDream 交互任务 | ACL 未配置时默认不设工具次数上限；effort 不再覆盖次数 |
| 显式 ACL `max_tool_rounds` | 正数限制继续生效；`0` 表示取消次数上限 |
| Core SDK | 兼容默认值 50；调用 `with_max_tool_rounds(0)` 才取消次数上限 |
| unattended | 继续保留 256 次上限 |

Core 的 `0` 在执行边界转换为真正的可选额度 `None`，同时取消由工具次数推导的转换次数上限。Effect 原有数值构造器中的 `0` 仍表示零工具额度；显式 `Some(0)` 组合额度也仍拒绝执行。没有用一个很大的整数模拟无上限。

超时、取消、审批、沙箱、重复调用和无进展保护保留。正数父级和可信角色额度继续约束子任务；模型输入 `task.max_steps=0` 被拒绝，避免借此扩大子任务能力。普通及检查点恢复不再固定使用 32 次转换，而是使用已准入的可选额度；原有 `resume_limit(0)` 仍只查看状态。

## 实测

- [Effect 最终回归](effect-final.json)：7 项通过，含 128 次工具执行、超过原派生转换阈值、取消、零及正数额度、显式转换上限和旧接口语义。此前 [Effect 库测试](effect-lib.json) 为 76 通过、1 项既有忽略。
- [Core 最终编译与恢复测试](core-final.json)、[关联回归](core-final-connected.json)：恢复、旧摘要兼容和 12 项无上限/显式额度测试通过。SDK 实际执行 64 次工具调用；stock 和 mixed harness 均覆盖。
- [Core 关联测试](core-connected.json)、[其余关联测试](core-connected-remainder.json)、[Task 生命周期](core-task-lifecycle.json)：子任务、输入校验、组合配置、显式额度和生命周期通过。`core-connected.json` 中初次摘要夹具失败由最终回归覆盖修正，原记录保留。
- [能力边界](core-capability.json)：15 项通过，包括无上限不能扩大有限父级额度、取消与资源回收。
- [ACP 实际会话测试](acp-final.json)、[关联回归](acp-final-connected.json)：12 项通过，覆盖打开/替换会话、各 effort、默认无上限、显式 ACL 和 unattended。
- [Work 真实 ACP 联调](work-r5-real-acp.json)：唯一指定的 ignored 测试实际运行并通过；64 次不同范围的文件读取、65 次受控模型请求、最终状态保存为 `success`。实际进程映射、cwd 和 ACP SHA 均匹配；子进程全部回收，无强制清理。耗时 24.54 秒，仅为本地功能验证，不代表 T01 性能。

联调使用真实 Work 原生服务、ACP、Core、Effect 和文件读取，只替换 provider 为本地受控响应。未配置 `max_tool_rounds`，未启动 VM，未调用 Bash，付费调用为 0。该测试不替代沙箱命令执行或 T01 的验证。

## 产物与构建

[配套构建](paired-build.json) 同时构建 `aster`、`a3s-code-acp`、`a3s-office-cli` 和 `aster-dream`，成功，耗时 84.88 秒。[Work 测试构建](work-test-build.json) 成功，耗时 11.40 秒。

开发包：`/Users/roylin/code/aster/target/development-stage/task-optimization-r2-20261020/AsterDream-TaskOptimization-R5.app`。

[签名记录](stage-r5.json) 为 `signed_strict_verified_not_launched`。真实联调使用该包内签名 ACP 的逐字节相同副本，SHA-256 为 `87e1ce16cc44e4efa8ea6b0fdeb2572079c6fc48a89f4e1e4ce077a1474b1780`；[冻结二进制及唯一测试列表](work-r5-binaries.json) 可核对。现有安装包及 R2/R3/R4 的公共文件保持原样。

## 验证边界

[最终源码冻结](final-source-freeze.json) 共 39 个文件；[最终检查](final-verification.json) 确认构建和联调后均未改变，两个仓库的 scoped diff 检查通过。38 个文件的[局部格式检查](final-scoped-format.json)通过；`agent_tests.rs` 保留既有格式差异，没有整体重排。

[全仓格式检查](workspace-format.json)仍有 12 个文件存在差异。[依赖检查](workspace-dependencies-py313.json)运行 6 项，其中 2 项失败：`apps/work/.git` 仍存在、`a3s-gateway` 的 publish 元数据不符合检查。保留并行迁移，不改动这些内容。系统 Python 缺少 `tomllib` 的首轮记录另存于 `workspace-dependencies.json`，随后使用 Python 3.13 完成检查。

保留最初 Core/ACP RED、并行 porcelain 编译未完成、取消夹具及摘要夹具失败的原始记录；最终结果以 `*-final*`、`core-capability` 和 `work-r5-real-acp` 为准。Core v3 的测试成功后，记录脚本因旧 Python 缺少 `hashlib.file_digest` 中断；已用流式 SHA 恢复记录并随后完成独立最终编译。Cargo JSON 日志省略 build-script 环境值。

此前付费 T01 的失败结论和产物未改写，本轮未重跑 T01，也不宣称已证明其耗时下降。Work 的旧助手编辑页 `maxToolCalls` 设置尚未接入本地 ACP；本轮所称显式额度入口指 ACL、SDK 和 harness，不对该旧 UI 作生效承诺。
