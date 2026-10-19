# 审批期限与迟到答复修复

本轮两个产品缺陷均先取得真实 RED，再完成最小修复。最终 **10 个不同的受控测试通过**；重复执行的新模块只计一次。没有使用付费模型，没有重跑 T01，也没有改变正在运行的 R2 评测。

## 修复范围

生产代码只有两处：

- Core `AgentSession::confirm_tool_use`：先沿用原有 manager 的有效答复与错误语义；manager 未找到接收器或未配置 manager，且会话存在 typed `config.work_policy` 时，返回 `Ok(false)`。不再尝试以 legacy owner 打开这个 bound Run。无 WorkPolicy 的真实 legacy fact wait 仍可通过原 fallback 回答；原始 owner 排他与拒绝检查未放宽。不一致的“配置无 policy、持久化有 policy”重开仍由原 guard 拒绝。
- ACP interactive session builder：内层 `ConfirmationPolicy` 使用既有 `permission_ask::ask_deadline()`，以饱和转换得到毫秒数，继续使用 `TimeoutAction::Reject`。默认期限由原来的 30 秒对齐至现有人类 ask 的 **30 分钟**。继续支持现有 `A3S_CODE_PERMISSION_ASK_TIMEOUT_SECS`；测试覆盖默认、1 秒、0、最大 u64 和非法值回退。没有增加配置项、自动批准或命令缓存。unattended 的既有授权分支和 Core SDK 默认策略未改。

两处生产文件在初次 GREEN 与最终 Core fixture 复验之间 SHA 不变：

| 文件 | SHA-256 |
| --- | --- |
| `crates/code/core/src/agent_api/governance_facade.rs` | `9554a8ab5cdfe4a55ab77e12e66ea9e2c3327d9c14193d819f22e94f871e8119` |
| `crates/code/acp/src/agent.rs` | `482d0b18add0f506feffdeea3a791d3d82ca4e32889636328f4a547db9e07b1c` |

完整 6 文件及最终测试夹具的冻结清单为 [source-freeze-owner-fixture.json](source-freeze-owner-fixture.json)。写本文时重新核对，6/6 与当前文件一致。`agent.rs` 保留了此前已存在的 `local_embedding` 与 `registration_cleanup_tests` 注册，不将其计为本修复。

## 实际 RED

| 用例 | 观察结果 | 原始证据 |
| --- | --- | --- |
| bound policy 的 missing / withdrawn confirmation | missing ID 得到 `Err(Internal(work_policy.invalid: bound policy requires the original session owner))`，而不是无执行的 `Ok(false)`；0/1，exit 101 | [core-red.log](core-red.log)、[命令与退出码](core-red.json) |
| ACP 内层与现有人类 ask 期限一致 | 既有环境设置为 1 秒时，内层仍为 30000 ms，预期 1000 ms；0/1，exit 101 | [acp-red.log](acp-red.log)、[命令与退出码](acp-red.json) |

RED 后增加有效 manager receiver 正例、无 policy 的 legacy public facade 正例，以及 deadline 参数边界；这些补充用例未冒称已执行 RED。

## 去重后的 GREEN

| 组 | 不同用例数 | 覆盖范围 | 原始证据 |
| --- | ---: | --- | --- |
| Core 新 confirmation 模块 | 2 | bound policy 的 live receiver 可回答，missing / withdrawn 不执行、不追加 facts、不调用 provider；真实 unbound parked fact 在有/无 manager 两种情况下均可精确拒绝 | [core-green-final.log](core-green-final.log)、[元数据](core-green-final.json) |
| Core owner 排他 | 1 | 第一个实际 provider 调用被持有期间拒绝第二 owner；legacy opener 仍拒绝 bound policy；真实退休后第二 owner 才可执行 | [core-owner-green.log](core-owner-green.log)、[元数据](core-owner-green.json) |
| Core 既有 session confirmation API | 2 | 有效 manager answer 与未配置 manager 的无 pending 行为 | [core-connected-1.log](core-connected-1.log)、[元数据](core-connected-1.json) |
| Core 既有 exact pending fact | 1 | 仅精确 pending ID 可答复，拒绝不执行工具 | [core-connected-3.log](core-connected-3.log)、[元数据](core-connected-3.json) |
| ACP 新 deadline 模块 | 2 | 实际 builder 与既有 ask helper 一致；默认、1、0、u64 最大值、非法值边界均保持 enabled + Reject | [acp-green.log](acp-green.log)、[元数据](acp-green.json) |
| ACP 既有 unanswered ask | 1 | 两类未答请求在既有配置期限上 fail closed | [acp-connected-1.log](acp-connected-1.log)、[元数据](acp-connected-1.json) |
| ACP 既有 unattended | 1 | 既有 AutoApprove 分支不被 interactive builder 改动影响 | [acp-connected-2.log](acp-connected-2.log)、[元数据](acp-connected-2.json) |
| **总计** | **10** | 全部通过，无重复累计 | |

## 既有 owner 夹具的诊断与修正

连接测试初跑在 5 秒等待 `entered` 时失败，见 [core-connected-2.log](core-connected-2.log)。在生产改动之前的 T005 冻结二进制 `92486c70…` 上运行同一测试，也在同一探针超时，见 [core-owner-prior-binary.log](core-owner-prior-binary.log) 与 [二进制身份/退出码](core-owner-prior-binary.json)。因此不能将此失败归因为新增 stale-answer guard。

先只修改测试以并行观察 semaphore、实际 worker 返回值与原 5 秒期限，并在失败前结算 worker / 两个 session / 两个 agent。[core-owner-diagnostic.log](core-owner-diagnostic.log) 显示：worker 在 0.09 秒已成功返回 `session-hello`，provider 完成 1 次，但 nonempty-tool entry 为 0。原夹具只在 tools 非空时发 entered；这个 owner 排他测试并不要求工具列表非空。

最终仅为此测试增加最小受控 `PolicyOwnerClient`，直接持有真实 complete / streaming provider 调用，不依赖工具列表。保留 owner 排他、legacy 拒绝、facts 不变与退休后成功的原断言，未延长 5 秒期限，也未改变生产工具呈现。最终 owner 用例 1/1 通过。

## 二进制与检查边界

- 最终 Core 二进制为 `target/qualification/approval-timeout-20261020/core-tests-final`，SHA `ca735de9078ddc14d63c889d15f3dcf52b243b21eca6447f7a707564e0112c60`，见 [core-final-binary.json](core-final-binary.json)。最终 owner 用例和新模块 2 用例来自这一轮构建/冻结产物。
- Core 既有 API 2 用例与 exact pending 1 用例来自较早的 `core-tests`，SHA `07b607dc…`，见 [core-green-binary.json](core-green-binary.json)。其生产 guard 与最终版本相同，但 owner 测试文件尚是旧夹具；不声称该旧产物包含最终整个测试文件。初次新模块 GREEN 也保留于 [core-green.log](core-green.log)，不再累计两次。
- ACP 冻结二进制 SHA `e3b3bf17…`，见 [acp-green-binary.json](acp-green-binary.json)，对应新模块 2 用例及 2 个连接回归。
- 定域 rustfmt 检查与 `git diff --check` 通过，初次记录见 [static-checks.json](static-checks.json)；owner 诊断与最终 fixture 分别完成同样定域检查。没有由此推导整个并行修改中的 workspace CI 全绿。

本修复没有合并 outer Ask 与内层 Host 两次审批，没有改变有效参数审批规则或 Stop / owner 取消语义。真实 R2 的超时和迟到答复仍按原样留证；尚无带本修复的新付费 CUA 重试，因此不能据这 10 个受控用例声称 T01 已加速、任务成功率提升或用户审批等待已实测改善。SDK canonical 修复、R3 guest smoke 与后续 R4 配套构建属于独立证据，不混入本轮 10 用例计数。
