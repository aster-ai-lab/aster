# 工具拒绝后的恢复开发计划

**规格** [spec.md](spec.md) · **任务** [tasks.md](tasks.md) · **归属** Core、ACP、CLI · **端到端验证** [E01–E14](#端到端验收矩阵) · [扩展计划 E15–E29](#扩展端到端测试计划) · [系统性验收 E30–E62](e2e-plan.md)

## 恢复以原目标和执行证据为准

一次工具拒绝给出了当前权限和执行条件下的事实。Core 应当把这个事实作为下一次决策的约束。对于 Aster，目标完成依赖原验收证据，权限依赖可信宿主，进展依赖可归属的原始工具观察和已验证产物。拒绝本身只说明某一步受阻。可恢复的路径应当保留这些事实，收敛到允许的步骤、明确的等待条件，或停止原因。

实施前的基线有四个交接缺口。

| 层 | 已有实现 | 缺口 |
| --- | --- | --- |
| 工具准备 | [Core 权限分类](../../crates/code/core/src/fact_control/tool_results.rs#L423) 和 [工具凭据](../../crates/code/core/src/fact_control/receipts.rs#L297) 区分原参数、有效参数、规则拒绝与需要确认 | 最终拒绝细节主要以文本传播，且通用的命令形状建议可能与明确禁止的操作混在一起 |
| 循环控制 | [拒绝与工作区状态检测](../../crates/code/core/src/fact_control/receipts/refusal.rs#L106) 按原始运行和已准备的工具参数关联。重复拒绝会在 [下一次请求前](../../crates/code/core/src/fact_control.rs#L754) 终止 | 第一次拒绝仍依赖模型自行理解错误文本并改变计划 |
| 目标续跑 | [Goal 等待](../../crates/code/core/src/agent_api/goal_facade.rs#L358) 和 [续跑决策](../../crates/code/core/src/goal_control/continuation.rs#L159) 已有预算、等待条件与核验 | 缺少从具体工具阻塞到可等待、可恢复目标步骤的连接 |
| 交互 | [ACP 工具通知](../../crates/code/acp/src/agent/turn.rs#L396) 带有 `permission.denied`。CLI 会对终止情况套用 [通用失败提示](../../crates/terminal/crates/codegen/aster-pager/src/app/error_display.rs#L359) | 用户需要从第一条拒绝追查发生了什么，重发提示也没有指出恢复条件 |

## 决策和执行分别由 Core 与模型承担

1. **在产生结果的地方保留类型。** 复用 `PermissionEvaluation`、确认答复、已有工具拒绝和执行观察。恢复分类按下表。Core 的 `FactToolResultEnvelope` 已有可选 `metadata`，优先用它保存显式解码的原因。恢复字段由 Core 从权威边界写入并绑定到已准备的调用。工具提供的任意元数据只当作输出。保持外层 `ToolExecutionResultV1` 字段、原日志序列、锁和归属。旧字符串结果按既有规则回放，来源保留未知。
2. **利用首次拒绝后的正常模型调用。** 从当前 Run 的可信拒绝中投影指令，带上可公开的来源、原因和下一步。模型可以改计划、完成独立待办或提出具体阻塞。用现有的 Run、授权绑定、工具与有效参数身份关联已拒操作。`decision_from_response` 的确认分流和实际执行前都要执行对应校验，使盲重试走既有拒绝结算。遇到 hook 重写时依据最终参数校验，保留 hook 已产生的观察义务。异名、改写或不同参数的调用依然逐次经过原权限检查。
3. **为无进展规定硬上限。** 首次拒绝后的下一次正常模型调用沿用原预算。若替代工具首次返回结构化拒绝，且本次未重复已被拒绝的工具，允许模型再作一次决定。其余无进展路径在下一次模型请求前停止；这也约束未设总工具轮次的会话。使用原 Run 的工具结果回放这个上限，保留已有 Goal 的切片上限、无进展上限和绝对期限。新的可归属观察可以推动计划，只有与操作匹配的真实审批或授权变化能允许原被拒操作。换参数、压缩或重启无权刷新额度。
4. **把等待交回原生命周期。** 候选操作确实允许申请确认时，用 `ConfirmationRequired` 和原确认 ID。真正需要用户输入的 Goal 采用现有 `QuestionAnswer` 或 `ConfirmationAnswer` 等待条件。重启后先验证原条件及现存结果，已过期的确认答复保持无效。旧运行未释放、写入结果不确定或日志无法可信回放时，先走既有 `Reconcile` 分支。外部副作用依照原执行观察核验。
5. **ACP 投影状态，CLI 负责呈现。** 沿用已有 typed error data、工具元数据与终止投影的模式，兼容域内老消息和不识别新信息的客户端。已有字段不足时在 ACP 自己拥有的扩展中加可选结构化字段。经典用户可见状态为「原操作被拒，正在继续其他步骤」「等待确认或输入」「恢复预算用完」。完成文案来自原验收，权限理由来自受限字段，敏感参数和完整命令不新增到终止摘要中。

这条路径把恢复放在第一次拒绝之后，并保留现有测试约束：重复拒绝已成立时不能再继续计费生成。现有重复读取在得到新观察时仍可继续。方案复用仓库里的 [有界 Goal 重规划](../../crates/code/core/src/goal_control/facts/run/continuation.rs#L80) 与 [重复拒绝基线](../../crates/code/core/src/agent_api/goal_facade/tests/execution/refusal.rs#L125)。

| 原始结果 | Core 决策 | 允许继续的条件 |
| --- | --- | --- |
| 策略禁止、用户明确拒绝 | 拒绝原操作，重规划允许的步骤 | 替代步骤通过原权限检查；原操作须有与它匹配的可信授权变化 |
| 可申请确认 | 复用现有确认交互和精确调用身份 | 绑定授权通过，目标从该步骤继续 |
| 确切的临时故障 | 已知未启动或确认幂等时使用原有重试机制 | 仍有预算，条件允许重试 |
| 重复拒绝或停滞 | 消费一次原预算内的机会，达到原熔断条件或无进展上限即结算 | 可信输入或真正的新进展，且原预算允许 |
| 工具可能已生效或旧原因未知 | 保留结果及证据缺口，按既有核验和恢复契约处理 | 能证明现态或取得适用的可信决定 |
| 已取消、硬预算耗尽 | 按原终止原因结算和保留证据 | 后续启动须按宿主既有授权和预算入口处理 |

## 宪法检查

此表检查设计，实施证明在对应验收阶段填写。

| 原则 | 状态 | 方案依据 |
| --- | --- | --- |
| I. Fail closed | PASS | 原策略、确认 ID、执行观察、取消和核验门槛仍是执行依据 |
| II. Evidence retained | PASS | 实施后记录命令、数量、平台和覆盖限制 |
| III. Hermetic tests | PASS | 每个行为任务绑定 owning package 的测试 |
| IV. Closed sets | PASS | 拒绝与恢复按已声明类型，新增闭集提供 `parse()` / `as_str()` |
| V. Workspace / published APIs | PASS | 根工作区和公开 Rust API 延续 |
| VI. Schema ownership | PASS | 原事实和宿主扩展在各自 owner 内演进，覆盖旧记录回放 |
| VII. Shrink-only ratchets | N/A | 无 allowlist 变更 |
| VIII. Replaceable transports | PASS | Core 不依赖 ACP 或 TUI；投影通过现有接缝 |
| IX. ACL | N/A | 使用现有策略及预算 |
| X. Commands and tests | PASS | 现有宿主命令和测试路径，Rust 测试置于 sibling 文件 |
| XI–XII. Judgment and adversarial review | PASS | 以现存硬停为基线，测试僵化模型、参数变体与进程恢复 |
| XIII. One repository | PASS | 当前 checkout 及其根工作区 |
| XIV–XV. Names and meaning | PASS | 工具身份、拒绝和审批来自原结构化记录，模型选择替代步骤 |
| XVI–XVII. Pickers and dialogs | N/A | 使用现有终端行和确认交互 |

## 文件落点

| 职责 | 预计修改的现有路径 | 既有模式 |
| --- | --- | --- |
| 拒绝类型与副作用 | `crates/code/core/src/fact_control/receipts.rs`、`fact_control/tool_results.rs`、`fact_control/executor_tools.rs` | 原始与有效参数绑定、执行拒绝、`confirmation.answered` |
| 下一次模型调用与次数限制 | `crates/code/core/src/fact_control.rs`、`fact_control/receipts/refusal.rs`、`fact_control/model_call.rs` | 原始事实传播、回合控制、循环熔断 |
| 目标等待与恢复 | `crates/code/core/src/agent_api/goal_facade.rs`、`goal_control/continuation.rs`、`goal_control/facts/run/continuation.rs` | Goal wait、原 Run 结算、预算持久化 |
| ACP | `crates/code/acp/src/agent/turn.rs`、`crates/code/acp/tests/` | `permission_denial` 元数据、`reconciliation_error`、stdio 接线 |
| CLI | `crates/terminal/crates/codegen/aster-pager/src/app/error_display.rs`、`app/turn_completion.rs` 及现有测试文件 | 终止展示统一入口和已声明的回合结果 |

## 交付与验证

按 [tasks.md](tasks.md) 的依赖顺序落地。先用模拟响应对旧实现建立可复现的缺口，再验证真实工具与事实日志。端到端冻结目标、权限、预期产物、时间上限和判定标准。真实 LLM 记录成功路径和阻塞路径的实际表现。自动测试断言执行计数、最终状态和产物，屏幕截图用来核对用户看得到的交互。

实施通过的标准包括 0 次越权执行、已完成写入总计 1 次、只有匹配输入能解除阻塞、循环达到现有上限后没有新模型请求。能力已存在或已有证据充分的路径照常完成。分别记录原生平台的结果，未跑平台列入覆盖限制。

### 端到端验收矩阵

本轮扩展覆盖 Core 原始事实、ACP stdio、CLI 真实 PTY、无人值守、真实模型和 macOS 原生 Terminal。P0 是合入门槛，P1 是发布候选资格验证。跨进程新增能力对应 T004，先按目标行为建立用例，若现有实现无法满足就保留失败及其证据。现有 108 项相关测试、117 项有重叠的 Goal 回归与真实 DeepSeek 替代路径是已验证基线。四项 Pager 既有失败和未取得的窗口截图在原记录中逐项可查。 [已执行证据](../../docs/evidence/code/2026-11-26-denied-tool-recovery/OUTCOME.md)

每个用例冻结提示、ACL、模型响应序列或真实模型标识、工作区初始字节、注入点、时间上限、预期原始事实和产物。密闭用例使用真实构建的 `aster`、`aster-acp`、Core、工具执行器和临时文件系统，以仓库内已有的本地 HTTP fixture 控制模型与故障。审批用真实 ACP 请求，由测试端发送绑定答复。用带空格和中文的临时路径验证便携场景。Unix Bash 拒绝保留为原事故回归，Windows 的同类断言通过原生可用的工具执行。

| 用例 | 优先级和入口 | 操作与通过条件 |
| --- | --- | --- |
| E01 · 拒绝后完成原任务 | P0 · ACP / PTY | 拒绝 Bash，再用已授权工具读取工作区内标记。受拒命令启动 0 次，允许读取 1 次，拒绝与成功凭据归属于同一 Run，终态成功且回答引用实测标记。 |
| E02 · 无进展有界 | P0 · ACP / 无人值守 | 在下一次重规划中分别发同参数、不同参数、不同调用 ID 的受拒请求，再用现有 compaction 或上下文切换路径复验。同一工具再次被拒绝即停止。另一工具的首次结构化拒绝可以触发一次额外的决定，之后无进展即停止。停止码为 `run.tool_refusal_cycle`，停止事实之后推理请求增量为 0，CLI 在同一会话显示一次原因，还能接受下一条用户输入。 |
| E03 · 有进展可以继续 | P0 · ACP / 无人值守 | 在两次拒绝中间加入经验证的新文件观察、原已允许的检查或满足原验收的产物。相同读结果和回访已见状态不能续预算，真正新证据能够继续。以请求计数、观察摘要及现有 `run.tool_state_cycle` 断言判定。 |
| E04 · 输入后审批 | P0 · ACP / PTY | `ask_user` 与写操作同批出现时，先问问题。回答后显示待审批调用。审批前文件与启动计数为 0，匹配审批后写入计数为 1，文件字节匹配，原 Run 只有一个终态。 |
| E05 · 错误和过期答复 | P0 · ACP / Core | 在待审批、取消后和重开后送入错误 ID、其他会话答复、重复答复，以及有效参数被改写前的旧答复。原操作执行次数保持 0。有效答复只能消费一次，明确拒绝、撤销、超时及无审批客户端仍按各自原因结算。 |
| E06 · 等待中的冷重启 | P0 · ACP / 无人值守 | 在收到审批或问题事件后、用户回答前关闭进程，用同一持久目录和 Session ID 启动新进程。已完成产物及原预算保留，状态指向原步骤，旧进程的审批无效。仅重新加载会话时推理请求增量为 0。需要新确认时由可信宿主签发与实际调用匹配的新审批，后续写入总计 1 次。 |
| E07 · 写入与回执之间崩溃 | P0 · Core / ACP | 用测试同步点停在「已准备但未启动」「已发生写入但无终结凭据」「终结凭据已落盘但 Goal 未结算」三处，再终止测试进程。第一处依据原准备记录与可信启动证据判断能否继续。第二处进入 `Reconcile`，暂时禁止重放和完成。第三处只补结算，已确认写入跨进程仍为 1 次。文件存在本身不解除债务。 |
| E08 · Goal 完成门槛 | P0 · Core / 无人值守 | 一个可独立步骤已完成，另一必做步骤待输入或受拒，模型声明「已完成」。终态保持等待输入或需要核验，下一依赖步骤不得启动，产物及消耗留存。再分别以匹配输入、已取消和已耗尽预算三条路径复验。 |
| E09 · 传输断开与重连 | P0 · ACP / PTY | 在空闲会话、提示处理中和权限卡可见时关闭 ACP 管道或终止对应夹具进程。区分主动退出和意外断连，终端提供原会话恢复入口，手动恢复使用同一 Session ID。不存在幽灵审批、重复终态或无主子进程。 |
| E10 · 供应商与取消 | P0 · ACP / 无人值守 | 在第一次拒绝之后注入 429/503、流中断或停在模型响应之前，验证已有重试次数与截止时间、耗尽后的原因和已保留凭据。分别在模型、等待输入和启动工具的已知边界取消，主动取消后不得发新请求或开始工具。取消过程中的未结算执行保留核验状态。 |
| E11 · 真实模型替代路径 | P1 · Kimi / ACP | 本轮固定 `../config.acl` 中的 `kimi-01/k3.1-pro`，沿用现有工作区外读取夹具及 120 秒上限。原拒绝、许可读取、产物标记和正常终态全部出现才通过。模型跳过指定步骤或供应商不可用时分别记录。 |
| E12 · 真实模型审批和重开 | P1 · Kimi / ACP | 以同一 Kimi 模型和原权限入口验证单次写入需审批。要求模型先询问两个固定标记值中选哪一个，再把选定的标记写入 `result.txt`。测试客户端提交固定选项和匹配审批，核对写入一次且文件字节精确匹配。保存后冷重开，通过新提示读出原产物，确认原写入未重放。审批等待中的冷重启仍由 E06 验证。 |
| E13 · 看得见的恢复 | P1 · macOS / agent-desktop | 用真实 Kimi 演示自动改道后完成，以 E04、E02、E09 的密闭响应分别演示等待与审批后继续、有界停止后输入下一轮、断连后恢复同一会话。每段保留操作前后实际画面、Terminal 文本和对应事实序号。 |
| E14 · 从归档启动 | P1 · 已有发布矩阵 | 对候选归档执行现有成员和校验和验证。在临时安装目录启动归档的配对二进制，跑一次确定性的 E01 和重开已保存会话的冒烟用例。记录目标平台、原生执行方式和实际二进制身份。 |

### 把失败归到发生的边界

- **审批和副作用。** 启动计数来自 fixture 工具独立记录，审批与执行关联 `session_id`、`run_id`、调用 ID 和有效参数，完成以 Goal 事实和实际产物为准。冷重启夹具先确认旧进程和其子进程退出，再由新进程读取同一目录。依原始事实检查已消耗的回合、切片、请求和预算。未知执行的用例需要同时覆盖「有可信结果能结算」和「缺少可信结果继续等待核验」。原有 Goal wait 只重新开启步骤，工具执行仍要通过原审批边界。
- **时限和故障定位。** 优先以 ACP 事件、fixture 同步点或可见事实定位注入时刻，保留独立外层超时。密闭用例沿用各 harness 时限，供应商故障用例现有外层上限为 60 秒。E12 每轮外层上限设为 120 秒，确定性客户端收到每个输入请求后在 10 秒内提交答复，分别记录两种时间。以实际推理计数证明停机效果，模型目录请求另计。分开记录产品回归、模型行为、供应商错误、测试夹具错误和视觉环境阻塞。
- **重复性。** P0 对每个新增状态转换先跑可单独复现的用例，再跑受影响套件。P1 的 E11、E12 各计划三个预先登记的首轮样本，逐个报告和保存模型、请求数、耗时及判定。失败复跑保留首轮记录。真实模型样本全部出现预期结果才可标为通过。安全和副作用断言在每个可执行样本里都须满足。
- **视觉判定。** 先用 `agent-desktop status` 检查权限，再用 `list-windows` 将新窗口绑定到本次构建的进程及测试工作区，截取一次校准画面确认窗口运作。按 [visual TUI harness](../../apps/aster/tests/visual_tui_harness/README.md) 用 `agent-desktop` 操作窗口。若单窗截图失败，按同一 harness 用 `screencapture -R` 尝试有可信边界的窗口截图。失败时记录如 `global_window_inventory_unstable` 的实际原因，并保全 PTY 与事实证据。完成视觉验收须取得真实画面。先例同时保留了断连、恢复和请求数的证明。 [已有视觉证据](../../docs/evidence/code/2026-11-15-agent-connection-recovery-agent-desktop/OUTCOME.md)

### 执行顺序与门槛

| 顺序 | 落点 | 退出条件 |
| --- | --- | --- |
| 1 · 固定基线 | [现有证据](../../docs/evidence/code/2026-11-26-denied-tool-recovery/OUTCOME.md)、[现有测试命令](tasks.md) | 记录 checkout、Rust、OS/架构、Cargo features、CLI/ACP `--build-id` 和各自 SHA-256。两者从当前 checkout 同时构建，固定绝对路径，记录每项既有通过和失败。 |
| 2 · 密闭合入检查 | [构建 CI](../../.github/workflows/ci.yml)、[代码资格验证](../../.github/workflows/qualify-code.yml) | E01–E10 在适用入口全过。现有分层为 macOS 完整 PTY、Linux 原生 stdio/无人值守、Windows Core/ACP 便携覆盖。新 PTY 和进程故障用例按已有原生 harness 扩展。尚无原生用例的组合明确记录缺口。 |
| 3 · 发布候选资格 | [CLI 资格验证](../../.github/workflows/qualify-aster-cli.yml)、[发布归档工作流](../../.github/workflows/release-aster-cli.yml) | 真实模型、可视化、归档验证各自产出结果。候选归档来自同一干净提交和已有发布 profile。按实际归档平台记录运行，交叉编译只计编译验证。 |
| 4 · 证据归档 | `docs/evidence/code/<执行日期>-denied-tool-recovery-e2e/OUTCOME.md` | 每个 E 编号记录命令、夹具、注入序号、运行配置、首轮判定和复跑、审批与执行计数、请求数、耗时、产物摘要和证据路径。原始材料存放在现有 `.runtime/tui-full-e2e/` 下，写入带 `Explicitly not claimed` 的结果。凭据和真实用户数据排除在记录之外。 |

从仓库根目录按 [tasks.md](tasks.md) 初始化 `apps/studio` 并构建 CLI/ACP。以下入口可用于第一批回归。`pair` 为该次构建留存的临时配对目录，使用绝对路径。`shipped_tui_e2e` 读配对路径覆盖项。`unattended_e2e` 默认取 Cargo 的 `CARGO_BIN_EXE_aster-acp`，指定候选归档时用既有 `ASTER_LH_CANDIDATE_MANIFEST` 绑定路径、摘要和构建身份。真实模型套件使用 Cargo 构建的 ACP。按每个 harness 实际启动的路径记录身份。生产进程边界测试新增后，加入相应既有 test target 的过滤范围。 [候选二进制选择逻辑](../../crates/code/acp/tests/long_horizon/candidate.rs)

```sh
cargo build --locked -p aster -p aster-acp --bin aster --bin aster-acp
cargo test --locked -p aster-core --lib -- agent_api::goal_facade::tests::execution receipt_restore_tests convergence
cargo test --locked -p aster-acp --test acp_conformance_stdio --test approval_rules_stdio -- --test-threads=1
ASTER_TUI_BIN="$pair/aster" ASTER_ACP_BIN="$pair/aster-acp" \
  cargo test --locked -p aster-acp --test shipped_tui_e2e -- --test-threads=1
cargo test --locked -p aster-acp --test unattended_e2e -- --test-threads=1
ASTER_CONFIG_FILE="$aster_test_acl" ASTER_LIVE_MODEL=kimi-01/k3.1-pro \
  cargo test --locked -p aster-acp --test live_interactive_dialogue_real_llm -- \
  real_agent_recovers_from_an_outside_workspace_refusal_in_the_same_turn --ignored --test-threads=1
```

| 场景负责人 | 扩展现有文件与范例 |
| --- | --- |
| T004 · 跨进程状态与结算 | [Goal 等待](../../crates/code/core/src/agent_api/goal_facade/tests/execution/waits.rs)、[重开预算](../../crates/code/core/src/agent_api/goal_facade/tests/execution/recovery_limits.rs)、[原事实结算](../../crates/code/core/src/agent_api/goal_facade/tests/execution/settlement.rs)。先锁定 E06–E08 的契约，再以实际进程补证。 |
| T006 · ACP 与 CLI | [真实 ACP 与 PTY](../../crates/code/acp/tests/shipped_tui_e2e.rs)、[运行边界](../../crates/code/acp/tests/shipped_tui_e2e/operation_liveness.rs)、[供应商故障](../../crates/code/acp/tests/unattended_e2e/provider_failures.rs)。复用 suite 的本地 provider、已有取消和计数设施。 |
| T006 · 真实模型与视觉 | [实际 provider 对话](../../crates/code/acp/tests/live_interactive_dialogue_real_llm.rs)、[visual TUI harness](../../apps/aster/tests/visual_tui_harness/README.md)。保留原事故的固定用例，追加审批和复验用例。 |

以上是后续执行计划。当前保留 T004、T006 的待办状态，沿用原有权限、预算、Goal 验收和核验证据边界。

### 扩展端到端测试计划

把同一项工作从首次请求跟到最终交付。每次故障都检查谁有权让工作继续、哪些步骤已经真实发生、剩余预算是否保留，以及恢复后能否满足原验收。范围是 Core、ACP、CLI 和安装产物的验收。E01–E14 的冻结输入、首轮判定和时限继续有效。新增用例复用现有 ACL、权限模式、Goal、ACP 交互和核验入口。 [Kimi 首轮执行记录](../../docs/evidence/code/2026-11-26-kimi-tool-recovery-e2e/OUTCOME.md)

#### 优先执行的故障场景

| 用例 | 入口与故障注入 | 独立验收证据 |
| --- | --- | --- |
| E15 · 冷启动后恢复等待 · P0 | Core + ACP stdio。分别在问题和审批已对客户端可见、答复尚未提交时终止 ACP。新进程载入原 Session，再输入一次过期答复和一次满足恢复条件的新答复。无人值守入口验证已有的缺少输入结算。 | 原 Goal、必做步骤、已交付文件与已消耗预算可追溯。仅 `session/load` 时新增推理及执行均为 0。过期答复带来的执行为 0，合法答复最多消费一次。等待有可定位的后续动作，批准的目标写入总计一次。 |
| E16 · 从启动前到回执后的中断 · P0 | Core + 安装版 ACP。分别停在准备凭据已持久化、工具尚未启动，写入后、终结凭据前，以及成功凭据持久化后、Goal 结算前。第二处重开后分别提交同参数调用和另一写操作，含新 Run 和新的一次性审批。 | 启动前保留 fixture 的独立启动计数。结果未知时，同参数和其他写入的启动增量均为 0，允许产品契约中的独立读取与核验。重新审批仅说明新授权，原效果仍须结算。提供可归属的核验证据后才解除阻塞。第三处补结算后原文件摘要、写入计数和成功凭据各保持一份。 |
| E17 · 核验与 Goal 完成 · P0 | ACP + 无人值守。对同一份变更走匹配核验通过、缺失核验、核验失败、旧 Run 或不同产物核验四条路径。模型在每条路径都声明完成。 | 独立检查产物、当前变更的绑定核验、Goal 终态及依赖步骤启动数。只有匹配且通过的核验能关闭对应变更的义务。原目标还有必做步骤时继续保留该步骤。含取消、预算耗尽的反例。 |
| E18 · 并发与迟到答复 · P0 | ACP stdio。两会话同时等待确认，交叉提交答复，在一侧取消后发送其迟到答复和工具终态；另一侧独立批准。模拟 provider 复用调用 ID。 | 记录每侧的 Session、Run、ACP 请求、确认和有效参数摘要。取消侧终态一次且权限未转移，正确一侧只执行一次。其他会话的等待、预算和文件摘要不受影响。 |
| E19 · 长任务的预算连续性 · P0 | Core Goal + CLI。沿用长任务夹具，先完成独立子步骤，再触发拒绝，组合一次 compaction、暂停／继续和冷重开。分别安排一个新原始观察与一个重复观察。 | 原目标、独立产物和剩余预算连续。真实新观察允许继续。重复观察、压缩和进程更换在现有上限之外新增重规划请求为 0。记录请求数、切片数、恢复耗时和清理后的子进程数。触及既有硬期限后停止原因可读，之后的独立用户回合可以正常运行。 |
| E20 · 拒绝原因与工具执行对照 · P0 | ACP 交互、无人值守及原生工具边界。对照宿主策略拒绝、明确拒绝、等候超时、无审批渠道、运行时工作区边界、工具正常启动后非零退出。用 Unix Bash 复现原事故，Windows 用原生支持的工具。 | 每种情况按自己的稳定类型和已有恢复路径判定，保留 ACP 与结果原始状态。前置拒绝启动为 0，正常非零退出保留退出码及真实执行记录。逐条核对是否读取了可替代输入、是否提示等待，以及退出因果。 |
| E21 · 推理与工具取消边界 · P0 | ACP + 无人值守。在原始拒绝之后，按独立变体注入 429／503 后恢复、重试耗尽、响应首字节前挂起、部分 SSE 后断开，以及工具有副作用迹象后取消。 | 按当前策略和原绝对截止时间断言 HTTP 尝试数，区分一次模型决策与 HTTP 重试。记录取消发出、原连接关闭、子进程退出和终态。取消后新增请求和工具启动均为 0。效果不确定时保有核验义务，已得到的拒绝凭据仍可查。 |
| E22 · 回放、存储与工作区错误 · P0 | Core 持久化 + ACP。对照已支持的旧格式、按持久化边界切断的前缀、损坏记录和错误工作区。锁竞争或只读路径使用现有 store fixture 触发真实错误。 | 对照原始文件摘要和恢复投影。可读前缀内的已确认事实保留。无法证明的状态按原恢复契约报告，权限、完成事实和观察摘要保持可追溯。发生拒绝或读取错误时新工具启动和新推理请求增量均为 0。 |
| E23 · 截断、空结果与新进展 · P0 | Core 工具 + 交互和无人值守 ACP。用独立目录分别放入预算内的全未匹配文件、超预算的全未匹配文件、超预算的全匹配文件，得到完整零匹配、截断零匹配及截断部分匹配三个对照。目录遍历顺序不影响预期。 | 与原始 `truncated`、`matching_files`、扫描预算和恢复决策逐项比对。截断零匹配的用户可见结论体现结果不完整。换工具重新观察可以产生进展，重复的同一不完整观察消耗原上限。覆盖已在 Linux ARM64 复现的情况。 |
| E24 · 正常编码与恢复对照 · P0 | CLI PTY + ACP。复用小型多文件缺陷夹具，按读取定位、最小编辑、核验、退出、`--resume`、继续读取的顺序运行。脚本 provider 执行直通路径与一次工具拒绝后改道路径。 | 原测试由失败到通过。两条路径用同一独立验收比较精确变更、对照文件和核验摘要。重开匹配原 Session，历史可读，加载时新增推理为 0。每次已核验的变更在重开后都无重复执行。 |

E15 和 E16 先在同一份当前候选上取得红测，保留 E06、E07 的原日志和事实序号。E16 每个中断点采用独立 fixture 事件或可读取的持久前缀作为同步条件，并记录观察到的点。Unix 进程测试复用现有清理接口，Windows 以原生进程生命周期和文件写入证据验收。E16 的三个中断点属于进程终止验证，主机掉电另有持久化契约。扩展用例实现时，每条 P0 风险先留最小可独立执行的断言，再进入对应套件。 [恢复用例](../../crates/code/acp/tests/shipped_tui_e2e/recovery.rs) · [供应商故障 fixture](../../crates/code/acp/tests/unattended_e2e/provider_failures.rs) · [Core 回放测试](../../crates/code/core/src/agent_api/conversation_runtime/receipt_restore_tests/)

#### 发布候选、真实模型和可视化

| 用例 | 执行方式 | 通过条件 |
| --- | --- | --- |
| E25 · 原生 Terminal 完整用户路径 · P1 | macOS 当前构建 + `agent-desktop`。使用长文本和 UTF-8 工作区名，在问题、审批、执行、恢复之间调整窗口大小并滚动历史。用独立会话演示正常完成、明确受阻、主动取消和意外断连。断连场景重开原会话并续做。 | 窗口进程路径和配对身份与本次构建一致。每个交接点保留可见原始原因、可操作的下一步、回复后的状态及结束画面。对照 PTY 文本、事实序号和文件摘要，每张等待卡、每轮终态和每条已完成工具行各结算一次。 |
| E26 · Kimi 审批加绑定核验 · P1 | 从原 E12 分出新夹具，先冻结需要用户选择、写入和通过现有受信核验的全过程。用 `../config.acl`、`kimi-01/k3.1-pro` 和三个独立新 Session，每轮 120 秒，交互答复沿用 E12 的 10 秒上限。 | 三个首轮样本都需经历问题、精确审批、一次写入、匹配且通过的核验及 `EndTurn`。冷重开后读取同一文件并校验证据。固定的独立验收与 Aster 的核验同时通过，三次原 E12 判定照原记录保留。 |
| E27 · Kimi 多轮真实工作流 · P1 | 复用 E24 的正常编码夹具和独立验收，另冻结三个首轮样本。首轮读、修、验证，冷重开后按新提示回读产物和验证结果。两个样本走 ACP stdio，一个由真实 macOS PTY 启动 CLI 与 ACP。每轮预算 120 秒。 | 逐样本核对最终修改、验证结果、原 Session 与重开执行计数，保存分段耗时。固定夹具中允许模型自行选择有权的步骤。E11 的指定拒绝和替代路径继续用原挑战型夹具验证。 |
| E28 · 归档安装、覆盖安装与回滚 · P1 | 六个标准归档从同一干净提交用现有 release profile 产出。各自验证成员、摘要和 `BUILD_ID`，通过本地 `ASTER_RELEASE_BASE` 临时安装并启动。按既有安装器用例验证错误摘要、归档成员或第二个二进制晋升失败时的结果。 | 各原生目标都能运行两个已安装的二进制，已安装的 `aster` 启动同包 ACP，授权改道和保存会话重开通过。覆盖安装得到新配对，失败回滚留住原配对。Linux GNU 校验 glibc 2.31，musl 校验静态属性与原生启动；Windows 执行现有 PowerShell 安装及回滚测试。 |
| E29 · 跨版本续做 · P1 | 在原生平台用一份确切的上一稳定版本配对创建夹具会话，保存一个已核验的阶段和一个等待步骤。覆盖安装候选后从同一隔离持久目录恢复。 | 记录旧版和候选两套身份、事实前缀、原 Session、Goal、可支持的待办和预算。已核验写入增量为 0，旧审批带来的执行为 0。依据现有兼容范围，等待能以新授权继续或明确报告不可恢复原因。 |

六个标准发布目标是 `macos`（ARM64）、`windows`（x86-64）、`linux`／`linux-arm64`（GNU）和 `linux-musl`／`linux-musl-arm64`。若候选包含 `kylin-x86_64`，按现有契约额外校验它与 `linux` 的摘要相同，并验证通道选择。每行记录实际归档、OS、架构、libc 和运行环境。Windows 跑可移植 Core／ACP 以及原生安装器测试，现有 Unix PTY 套件的 Windows 图形交互覆盖单列。 [发布治理](../../docs/release-governance.md) · [归档矩阵](../../.github/workflows/release-aster-cli.yml) · [Windows 安装与回滚](../../tests/cli_pair_delivery.ps1) · [Linux musl 校验](../../tests/cli_musl.py)

#### 执行批次与归因

| 批次 | 顺序与退出条件 |
| --- | --- |
| 1 · 重现关键缺口 | 先单跑 E15、E16，然后跑 E17、E23。使用 E06、E07 原始事实做基线，固定相同工作区初态。E07 的 `#[ignore]` 严格用例显式运行。出现回归即记录首个违反的不变量和复现命令，修复后用同一夹具复验。 |
| 2 · 密闭回归 | E01–E10、E15–E24，再跑受影响的 Core、ACP、Pager 与便携 CI 套件。每个故障变体使用新进程和新工作区。macOS 跑完整 PTY，Linux 原生跑 stdio、无人值守及适用的 PTY 冒烟，Windows 跑便携 Core／ACP 与原生进程断言。 |
| 3 · 真实模型与视觉 | 密闭路径证实后执行 E11、E12 的原方案与 E26、E27 的新增方案，分别登记结果。先用本地 provider 校准视觉，通过后用当前配对做 E13 和 E25。每个真实样本都先确认实际模型、权限和二进制身份。 |
| 4 · 发布归档 | E14、E28、E29 按现有 release profile 和原生目标完成。每个平台从临时安装目录启动，记录二进制真实路径、构建 ID、平台证据以及失败回滚后的配对摘要。 |
| 5 · 归档和判定 | 每项记录通过、失败、环境阻塞或按契约不适用。P0 全部适用用例通过才完成合入验收。P1 按各自门槛标记，未取得的原生环境、真实样本或窗口截图影响对应资格。已知基线失败保留原记录及适用范围。 |

所有故障样本从单调时钟对齐事件。每个样本保存关联 ID、注入点、第一条 ACP 更新、请求发起与首字节、模型决定、审批等待、工具启动和结算、最终终态，以及进程退出和产物摘要。只有观察到的时间点进入归因，缺失时间点留空。已有测时设施能区分网络尝试、模型决定及本地工具。凭据、鉴权头、完整配置和真实用户数据从证据中移除。首轮结果与复跑分开，变更提示、时限或验收器形成新编号。 [ACP 与 PTY 观测](../../apps/aster/tests/tui_913_real_llm/acp_trace_proxy.py) · [请求用量观测](../../crates/code/acp/tests/long_horizon/timing.rs)

故障从第一处偏离预期的边界归因。相同的密闭响应重现等待丢失、重复执行或提前完成时，记录 Aster 的失败层。模型交付了可解码的决定，执行链路符合契约，但选择的下一步违反冻结任务时，记录模型决策。HTTP 错误、断流和取消分别按可见事件登记。若只观察到超时和最后一个拒绝事实，归因为未定，并报告最后已证实的阶段。夹具缺少审批、核验、原生 runner 或可用窗口时，记录实际缺项与现有证据。

从仓库根目录使用已初始化的 `apps/studio` 和已冻结配对执行。以下是现有可直接调用的入口，E15–E29 的新增变体落在对应入口再补测试函数名。`pair` 指本次构建或真实安装的绝对目录。

```sh
ASTER_TUI_BIN="$pair/aster" ASTER_ACP_BIN="$pair/aster-acp" \
  cargo test --locked -p aster-acp --test shipped_tui_e2e -- \
  recovery::shipped_agent_blocks_new_mutations_until_crashed_write_is_reconciled \
  --ignored --exact --nocapture --test-threads=1
cargo test --locked -p aster-acp --test unattended_e2e -- \
  provider_failures:: --nocapture --test-threads=1
cargo test --locked -p aster-acp --test unattended_e2e -- \
  incident_grep_is_bounded_through_default_interactive_and_unattended_acp --test-threads=1
ASTER_CONFIG_FILE="$PWD/../config.acl" ASTER_LIVE_MODEL=kimi-01/k3.1-pro \
  cargo test --locked -p aster-acp --test live_interactive_dialogue_real_llm -- \
  recovery::real_agent_answers_then_approves_a_single_write_and_reopens \
  --ignored --exact --nocapture --test-threads=1
ASTER_CONFIG_FILE="$PWD/../config.acl" ASTER_PINNED_MODEL=kimi-01/k3.1-pro \
  ASTER_TUI_BIN="$pair/aster" ASTER_ACP_BIN="$pair/aster-acp" \
  cargo test --locked -p aster --test tui_913_real_llm -- \
  real_llm_tui_turns_fix_the_diagnosed_bug --ignored --exact --nocapture --test-threads=1
```

最后一条是现有正常编码的真实终端对照用例。E27 单独冻结其冷重开和续做要求。E26 使用独立的新夹具，命令中的 E12 仍按原版本运行。每次执行在已有 `docs/evidence/code/<执行日期>-<场景>/OUTCOME.md` 路径记录命令、数量、各平台实际运行、首轮与复跑、证据及 `Explicitly not claimed`。视觉操作遵循现有 `agent-desktop` 驱动流程。 [视觉 harness](../../apps/aster/tests/visual_tui_harness/README.md)

### 规划基线

在 `d75f49664724` 的 macOS arm64、Rust `1.97.1` 上运行 `cargo test --locked -p aster-core --lib -- refusal_cycle a_single_run_cannot_keep_billing_after_its_identical_tool_calls_are_refused permission_diagnostics new_original_information_between_refusals`，30 项通过、0 项失败。本次结果覆盖现有拒绝诊断和熔断约束，恢复与重启场景在实施阶段验证。

Spec Kit 的 Codex 流程按 `e4914338e^:docs/spec-driven-development.md` 核对。规划采用当前检出的命令模板和 `AGENTS.md`。
