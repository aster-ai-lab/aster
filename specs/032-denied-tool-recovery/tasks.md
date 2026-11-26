# 工具拒绝后的恢复实施任务

**输入** [spec.md](spec.md) · [plan.md](plan.md)

每个行为任务随实现提交密闭测试。按编号顺序推进，完成该任务的可观察结果后进入依赖任务。

## 阶段一 · Core 原因与恢复

- [x] **T001 · US1、US3 · 固定回归场景。** 在 `crates/code/core/src/agent_api/tests/fact_log/convergence.rs` 加入真实执行入口的脚本模型场景。固定临时工作区、Bash 受阻权限和允许的替代工具，记录原实现对首次拒绝、重复请求与相同目标的行为。保留既有同参数有效观察、重复拒绝、状态环和 compaction 测试。
- [x] **T002 · US1、US3 · 在执行边界保留结构化结果。** 在 `crates/code/core/src/fact_control/receipts.rs` 为能够确认来源的执行前拒绝保留类型，沿用原工具结果文本。`permission_diagnostics` 验证硬拒绝启动数为 0、结果不确定时保留核验义务及原拒绝原因的展示。
- [x] **T003 · US1、US3 · 挂接首次拒绝后的有界重规划。** 在 `crates/code/core/src/fact_control.rs` 的正常模型调用与 `fact_control/receipts/refusal.rs` 的观察窗口中接入 T002 的信息。测试由首次拒绝引导到 `read` 等允许的工具并完成目标。重复同参数、每次换成不同被拒参数和压缩重放均在预算内停止。实际产生新事实时允许继续。保留 `a_single_run_cannot_keep_billing_after_its_identical_tool_calls_are_refused` 和已存在的工作区状态循环断言。

## 阶段二 · 目标等待、ACP 与 CLI

- [ ] **T004 · US2、US3 · 复用 Goal 等待及结算。** 在 `crates/code/core/src/agent_api/goal_facade.rs`、`goal_control/continuation.rs` 接入阻塞原因与原步骤。用 `agent_api/goal_facade/tests/execution/waits.rs` 和 `recovery_limits.rs` 验证确认待办、错误决定 ID、重复答复、取消及进程重启。已验证产物复用且写入计数为 1，未结算运行先核验，重启保留已消耗预算。增加仍有受阻必做步骤时模型声称「已完成」的反例。无人值守返回明确的缺少输入状态，普通会话保留原会话的恢复入口。
- [x] **T005 · US1、US2 · 连通 ACP 与 CLI。** ACP 区分明确拒绝、超时、取消和客户端不可用，并把原因带到原工具。CLI 识别稳定的循环停止代码。真实 PTY 用例验证停止提示及原会话的下一条输入，协议与审批回归通过。

T004 已验证同一 Run 先等待用户问题，再等待精确的工具确认，最后只写一次。跨进程恢复新 Goal 阻塞与副作用核验仍待单独验证。

## 阶段三 · 产品端到端验证

- [ ] **T006 · US1–US3 · 用同 checkout 的二进制验证。** 更新 `crates/code/acp/tests/shipped_tui_e2e.rs` 和对应的 `unattended_e2e` 场景。冻结输入与判定条件，覆盖同回合完成、有明确确认后续跑、无授权路径暂停、僵化模型有界终止、已完成步骤重启后不重演。使用 `live_interactive_dialogue_real_llm.rs` 的现有设施补真实 LLM 的冻结用例。macOS 用 [Visual TUI harness](../../apps/aster/tests/visual_tui_harness/README.md) 和 `agent-desktop` 打开新构建的 Aster，留存一次恢复与一次阻塞后继续的画面，另核对对应事实和产物。
- [x] **T007 · US1–US3 · 留存实现证据。** [验证记录](../../docs/evidence/code/2026-11-26-denied-tool-recovery/OUTCOME.md) 包含配对二进制、工具链、测试计数、原生 Terminal 的事实序列和截图限制。

T006 的成对产品、真实模型、PTY 和无人值守回归已通过。原生 Terminal 的恢复事实已记录，`agent-desktop` 未能导出画面。

后续 Kimi 全量执行见 [端到端验证记录](../../docs/evidence/code/2026-11-26-kimi-tool-recovery-e2e/OUTCOME.md)。这轮已取得窗口截图，冷启动等待和未核验效果的写入前门槛仍有缺口。冻结的真实模型样本保留独立判定。

- [ ] **T008 · US1–US3 · 执行扩展端到端验收。** 按 [E15–E29 执行批次](plan.md#扩展端到端测试计划)，先复现冷启动等待、未知副作用和截断结果，再通过密闭回归、真实 Kimi、可视化和原生安装归档各自的门槛。T004 对应状态恢复和核验证据，T006 对应 CLI、ACP、模型和归档入口。新增样本与原 E01–E14 逐项记录。

扩展实测已记录。E15、E16、E23 和 E29 保留严格失败证据，E26 三个冻结样本通过，E27 的一个首轮样本由夹具拦截。E14 和 E28 已验证 macOS ARM64、Linux ARM64 GNU 及 musl 配对归档，Windows 和 Linux x86-64 仍待原生验收。E29 在上述三个已测归档上均复现跨版本待答状态丢失。 [扩展执行记录](../../docs/evidence/code/2026-11-26-kimi-tool-recovery-e2e/OUTCOME.md#expanded-matrix-observations)

- [ ] **T009 · US1–US3 · 执行系统性端到端验收。** 按 [E30–E44](e2e-plan.md) 先执行已证实失败和正对照，再做跨层一致性、真实 Kimi、可视化和原生归档验收。现有严格判定和历史首轮样本继续保留，逐份候选记录可执行入口、覆盖矩阵及证据。T004、T006 和 T008 继续按各自完成条件验收。

首批执行新增 E35 可复现失败。只读事实日志允许冷加载，新提示却先花一次模型预分析才因无法落盘而拒绝。三个已测归档均复现，恢复权限后同一会话完成一次写入与核验。E30 的显式等待对照、E32 的收敛对照和 E36 的硬截止对照在同三平台有证据。E39 的三个首轮保留各自判定。 [系统性执行记录](../../docs/evidence/code/2026-11-26-kimi-tool-recovery-e2e/OUTCOME.md#systematic-matrix-observations)

E38 已完成五组原生 PTY 十轮对照。E41-03 的真实 Kimi 无人值守两阶段结果已用当前回执、对应 Run 和独立产物复核，保留冻结夹具的首轮失败。现有 Kimi 十轮用例在首轮触发写入核验门槛。E44 在三个原生归档验证切换失败后可用原版本重开并读回已核验产物。 [两阶段 Kimi 与十轮记录](../../docs/evidence/code/2026-11-26-kimi-tool-recovery-e2e/OUTCOME.md#e41-staged-unattended-kimi-and-existing-ten-turn-control)

E41 的 ACP 和 PTY 补测冷重开同一会话后都完成了依赖阶段，保留首轮判定。E42 用本次成对构建完成 Mermaid 原生界面、PNG 操作和 ACP 断连后的同会话续聊。 [ACP 和 PTY 记录](../../docs/evidence/code/2026-11-26-kimi-tool-recovery-e2e/OUTCOME.md#e41-staged-acp-and-native-pty-kimi) · [Mermaid 与断连恢复截图](../../docs/evidence/code/2026-11-26-kimi-tool-recovery-e2e/OUTCOME.md#e42-mermaid-in-the-current-checkout)

E39-D3 真实 Kimi 拒绝后改道、完成核验和冷读通过。E43 在三个原生安装版验证了默认发现配套 ACP、拒绝后安全读回和冷启动。E44 的未结算 Goal 升级后仍要求核对且不再执行。E31 同时记录了候选版 ACP 被强制终止后的子进程存活，以及正常取消的成功清理。 [运行证据](../../docs/evidence/code/2026-11-26-kimi-tool-recovery-e2e/OUTCOME.md#systematic-matrix-observations)

- [ ] **T010 · US1–US3 · 补齐跨入口和组合故障验收。** 先按 [候选门槛](e2e-plan.md#最先复验的候选门槛) 验证四组已知问题，再执行 [E45–E50](e2e-plan.md#e45e50--恢复状态与复合故障) 的排队、持久故障、预算与十轮连续交互。登记 [E51–E53](e2e-plan.md#e51e53--真实模型可视化与安装版补验) 的六个 Kimi 首轮、两个密闭视觉轨迹与一个真实模型窗口轨迹、原生安装和独立 ACP 归档。各项按表中入口与依赖执行，并将结果写入证据文件。

已记录 E34 的三个现有回归和 E37 的十二个安装版依赖门控对照。E47 在三个已测原生归档通过断连后的草稿恢复和逐条发送。E48 在同三个归档通过十五个普通提示与冷加载对照，锁争用拦在模型调用之前。E49 的十八个搜索样本取得原始观察和终态，十二个截断样本仍未通过完整性投影。E51-05 的真实 Kimi 无人值守首轮通过。E51-01 已改道、写入与核验，回合终态超时，保留首轮失败。 [新增实测](../../docs/evidence/code/2026-11-26-kimi-tool-recovery-e2e/OUTCOME.md#additional-e45e53-observations)

E45 在三个原生候选复现自动问题答复前后的续跑失败，已落盘答复和审批的原始前缀保留，旧回复和已派发连接器均无重放。E51-02 的真实 Kimi PTY 正常编码首轮通过。E52 的工具回执后断连轨迹使用同 checkout 新构建，保存四张原生界面截图。真实 Kimi 原生窗口已拍到审批和验证按键投递，核验审批到达时超过首轮时限。E53 用真实旧版二进制验证三个 ARM64 原生目标的内置更新、错误摘要拒绝及升级后配对启动。 [执行记录](../../docs/evidence/code/2026-11-26-kimi-tool-recovery-e2e/OUTCOME.md#additional-e45e53-observations) · [原生画面](../../docs/evidence/code/2026-11-26-kimi-tool-recovery-e2e/OUTCOME.md#e52-interrupted-after-a-durable-tool-receipt)

E48 新增 18 个显式 Goal 续跑对照，覆盖同三平台的只读日志、锁冲突、尾部损坏和工作区错配。E50 的 12 个传输控制均遵守原截止，过期后 `_aster/goal/state` 无法返回原 Goal 定义。原生 PTY 的六组连续交互共匹配 60 个回合终态，覆盖工具回执晚于原回合终态送达。E30 的三个无人值守显式等待与 E51-06 的三个自动提问密闭对照均停止在待输入。真实 Kimi 自动首轮在 360 秒触发未结算状态，保留前置修复，未到达提问。宿主登记等待的真实 Kimi 补测在 120.442 秒返回 `needs_input`，保留原等待和已核验修复。 [执行记录](../../docs/evidence/code/2026-11-26-kimi-tool-recovery-e2e/OUTCOME.md#additional-e45e53-observations)

E46 在三个原生安装版验证了 27 次真实子进程切断。显式续做和允许的读取各有六个正对照，十二个普通提示在新审批后先执行了写入。三个系统明确拒绝启动的样本仍生成未知效果；另有三个已通过绑定检查的切点在冷开后未结算。两条 Core SDK 结算测试通过。 [E46 切点与计数](../../docs/evidence/code/2026-11-26-kimi-tool-recovery-e2e/OUTCOME.md#e46-native-execution-new-prompts-and-settlement)

E53 的独立 macOS ARM64 ACP 构建成功。用工作流命令打包时，严格成员检查发现额外的 `._aster-acp`；禁用 AppleDouble 打包的本地对照只有一个可执行成员，两个协议冒烟通过。 [独立 ACP 打包记录](../../docs/evidence/code/2026-11-26-kimi-tool-recovery-e2e/OUTCOME.md#e53-standalone-acp-on-macos-arm64)

- [ ] **T011 · US1–US3 · 在实际入口锁定关键状态转换。** 跑 [E54–E58](e2e-plan.md#e54e62--跨边界验收)。先固定 E46 写入前门槛及结算、E49 投影和 E35 接纳的复现，再执行并发所有者、取消与截止、外部连接器等独立对照。每个失败先定位所属包的密闭入口，再查安装版 ACP 和相应的 PTY 或无人值守结果。
- [ ] **T012 · US1–US3 · 验证长时间使用、真实模型和用户操作。** 跑 [E59–E61](e2e-plan.md#p1--长时间运行真实模型原生界面和用户安装)。登记二十轮短测试、两组原生 120 分钟会话和六个真实 Kimi 首轮。将实际窗口截图、TTY 和同一 Session 的事实对应，标注每次截止后的请求和工具计数。
- [ ] **T013 · US1–US3 · 从实际分发入口验收。** 跑 [E62](e2e-plan.md#p1--长时间运行真实模型原生界面和用户安装)。分别记录 Core SDK、五个独立 ACP 和六个成对 CLI 发行件的资格。原生新 Shell 能找到预期命令，归档能升级及回滚，更新后的相同 Session 满足对应恢复条件。按实际发布工作流的候选身份签收证据。

## 实施验证命令

从仓库根目录执行，确认 `apps/studio` 已初始化。保持仓库的 Rust `1.97.1` 与现有 feature 组合。新增过滤项以实施时的测试函数名为准。

```sh
cargo fmt --all -- --check
cargo test --locked -p aster-core --lib -- refusal_cycle permission_diagnostics convergence recovery_limits execution::refusal execution::waits
cargo test --locked -p aster-acp --test acp_conformance_stdio --test approval_rules_stdio -- --test-threads=1
cargo test --locked -p aster-pager --lib -- error_display turn_completion
cargo clippy --locked -p aster-core --all-targets --no-deps
cargo clippy --locked -p aster-acp --all-targets --no-deps
cargo clippy --locked -p aster-pager --all-targets --no-deps
cargo check --locked --workspace --all-targets
cargo check --locked -p aster-core --features ci-all,deep-live
cargo build --locked -p aster -p aster-acp --bin aster --bin aster-acp
cargo test --locked -p aster-acp --test shipped_tui_e2e --test unattended_e2e -- --test-threads=1
```

真实模型用例按既有 `ASTER_CONFIG_FILE` 配置，固定模型和验收器后运行 `cargo test --locked -p aster-acp --test live_interactive_dialogue_real_llm -- <新增过滤项> --ignored --test-threads=1`。缺少 provider 条件时，单独记录该验证阻塞。额外发布 profile 和原生平台检查按仓库 [CI](../../.github/workflows/ci.yml) 与 [代码资格矩阵](../../.github/workflows/qualify-code.yml) 执行。

## 最终检查

- [x] 受拒操作 0 次启动，允许的替代路径满足原验收。
- [ ] 输入或审批匹配前暂停，匹配后例行恢复。已完成写入在断连与重启后仍只有 1 次。
- [x] 重复与状态环保护、新观察与正常工具退出通过已有密闭检查。
- [ ] 跨进程恢复仍保留未知副作用的写入门槛。E07 严格用例保持显式执行，E16 补同参数重放和结算断点。
- [x] 序列化兼容、格式、lint、CI 工作区检查和实际二进制验证均有结果。
