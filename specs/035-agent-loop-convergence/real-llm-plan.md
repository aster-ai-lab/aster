# DeLoop 机制真实 GLM 端到端测试计划

状态：EVALUATED_WITH_FAILURES / 已执行首轮评估，完整资格门未通过。结果见[GLM报告](../../docs/evidence/code/2026-11-01-agent-loop-convergence/glm-real/OUTCOME.md)。用户明确选择 `glm-01/glm-5.3`，配置来源为仓库根 `./.aster/config.acl`。本文扩展 035 的真实模型验收，不新增产品机制。

## 测试目标与固定条件

验证当前 Aster 在真实复杂任务中是否正确交付、保留副作用证据、允许必要复验和新观察，并在无进展时有界停止。只有真实进入对应分支，才记为该机制已覆盖。

- 模型固定为 ACL 中声明的 `glm-01/glm-5.3`，协议按 ACL 解析为 Anthropic。不改默认模型，不退回 Kimi、Flash 或其它 provider。
- 复用 `long_horizon_real_llm.rs`、`long_horizon/{live,driver,relay,fixture,rust_workflow,report}.rs`。不创建另一套 ACP、费用计量或完成判定器。
- 当前源码配对构建 `aster` 与 `aster-acp`，核对相同 `--build-id`；开始前冻结二进制、test binary、fixture、grader、模型能力和编译 VM 身份。构建/身份核对耗时与任务耗时分别记录。
- 上游凭据只由 `CodeConfig` 读取，留在宿主内存；子进程只获得临时本地 relay 密钥。不复制个人配置、凭据、历史会话或 reasoning 正文到证据。
- 每个首次尝试均保留；失败不删、不改 grader 后重算通过。诊断重试使用新目录和独立 attempt 编号。

## 首轮执行顺序

| 阶段 | 固定任务 / 输入 | 真实通过条件 | 上限 |
| --- | --- | --- | --- |
| P0 预检 | 当前 CLI/ACP pair；当前编译 VM；ACL 指定模型 | pair ID/字节一致；真实 guest Rust 1.97.1 编译并执行；ACL route 可用。失败即分类，不绕过沙箱。 | 不计作模型成功 |
| P1 校准 | 原 `real_native_file_workflow_multiple_user_turns`，seed 2401，六轮订单/税率/折扣/币种变化 | 六轮 report.json 逐字段满足独立算术 oracle；源文件完整；ACP bound outcome；全部工具和子进程结算；至少一次真实上游成功 | 原 40 requests；每次 output ≤4096；prompt 600s；workflow 1800s |
| P2 复杂工程 | 原 `real_rust_multifile_repair_binds_current_tests_and_independent_guest_oracle`，依次 seeds 2401–2405，每个首次尝试一次 | 真正运行原测试并先失败；修复多个 Rust 模块；当前原测试全过；独立 guest oracle 覆盖边界/取整/格式；保护文件不变；完成门通过；清理成功 | 每个保留原 40 requests / 4096 output / 600s prompt / 1800s workflow |
| P3 机制定向 | 下表的新 cohort；先冻结输入、上下文预算和 oracle，再运行 GLM | 对应原始事实必须证明压缩/重复/新观察/停滞实际发生；否则为 NOT_EXERCISED | 单独预算，不修改 P1/P2 的原配置 |

P1 是接线和计量校准，不能用它代替复杂工程结论。P2 每个 seed 独立计入分母；不因某次失败而更换模型、减少边界用例或延长原期限。

## P3 定向 cohort

| ID | 任务设计 | 必要证据及负对照 |
| --- | --- | --- |
| G1 跨压缩修复 | 在多模块修复任务中加入独立变更说明与长诊断，使用事先冻结的较小上下文预算 | `compaction.done` 实际 ≥2；保留旧失败/新测试证据；最终独立 oracle 通过。无压缩则不得宣称覆盖跨压缩。 |
| G2 新外部观察 | 单个 Run 中处理外部连续发布的版本化输入，累计完整审计产物；同路径同参数至少产生7个真实观察 | 原 `tool.prepared`/`tool.result` 绑定、观察版本、完整有序交付；不能只更换 call ID，不能用多次用户输入重置窗口代替。 |
| G3 稳定观察与错误恢复 | 相同任务含稳定不可变输入或一次明确拒绝，模型选择恢复方式 | 记录实际重试/替代动作/终态。成功须满足原目标；无进展须按原拒绝/状态循环停止；提前失败不算节约。 |
| G4 合法复验 | 已失败检查 → 真实源码变化 → 再检查，并保留同字节重写负对照 | 当前字节回执与原检查绑定；必要复验允许；同字节重写不能获取新额度。 |
| G5 Goal 长程恢复 | 跨 Run/重开后继续原目标，含新计划与重复旧观察 | 原累计预算、一次原有恢复机会、旧观察不重复计入信息增量；Unknown/Incomplete 不假完成。 |

P3 不以机械强迫模型做50/200/500次无用调用制造“长程”。这些档位已由受控真实 scheduler 测试验证；真实 GLM 用任务依赖、上下文压力和有效工作量体现长程。每个新 cohort 的独立验收器须先用正确产物、缺失产物、错误值、修改保护文件四类样本自证。

## 结果与效果评估

每个任务同时报告，不能只报成功样本：

1. 正确性：独立 oracle、当前原测试、保护文件、完成门、清理；分别为 PASS/FAIL/UNKNOWN。
2. 终态：完成、模型失败、预算、停滞、取消、provider、harness、runtime、身份不一致，保留原始原因。
3. 消耗：真实上游 requests、工具开始/成功/拒绝、压缩次数、provider input/output/cache usage、耗时；缺失 usage 为 Unknown。
4. 行为：同动作/同观察重试、同动作/新观察、必要复验、重复上下文；内部 digest 只用于评估，不能注入模型提示。
5. 机制覆盖：G1–G5 各自为 EXERCISED / NOT_EXERCISED / BLOCKED，不以工具调用多等同于机制被检验。

综合通过率的分母包括全部已开始的首次尝试。首轮6次样本只给逐任务结果、范围和描述统计，不宣称显著性能提升。小样本不报告无意义的 p95；列 min/median/max 和逐项原值。

真实费用只在上游 usage 完整且有该路由的明确计费价格时计算；不能用其它渠道公开价格推算私有代理账单。仅有 token 时只报 token。

当前保留的修改前 Core unit binary 不能代替修改前完整 ACP pair。因此首轮是候选绝对效果评估，不能声称相对旧版节约百分比。正式 A/B 需要可信 baseline pair，保持同模型、输入、seed、预算、oracle 和顺序平衡；否则 A/B 标记 NOT_RUN。

## 产物与复核

证据归 `docs/evidence/code/2026-11-01-agent-loop-convergence/glm-real/`：预注册清单、模型能力脱敏摘要、pair/runtime 身份、逐 attempt `result.json`、wire notifications、原检查输出、独立 oracle、usage/requests、产物、清理结果和总报告。

复开每个 result 与原产物交叉核对：seed、条数、字段、测试数量、当前文件 digest、工具终态和 token 完整性。报告区分：已经证明的机制、未触发的分支、明确失败、环境阻断，以及不能推断的收益。

## 当前已知限制

- 原 unattended 套件7个失败均已 SOLO 再现，涉及目标启动/绑定阶段；其中部分明确为 pre-analysis 结构化响应失败。未保留可运行同套件的修改前 ACP，因此不把它们伪称已由 baseline 证明，也不修改原模型脚本掩盖。
- 当前 Core 相关套件有两个已被修改前 binary 证明的既有失败；本规格新增 Core 7个用例通过。GLM 测试遇到这些路径时仍保留实际失败。
- 原生平台是 macOS arm64；VM 内 Linux/Rust 结果不等于原生 Linux/Windows Aster 资格。

## 已冻结的 G1/G2 实例

新增 `crates/code/acp/tests/deloop_real_llm.rs`：单个 ACP Run，7个外部版本，每版48条 JSONL 记录，共336条；GLM读取后宿主发布下一版。固定16K上下文、原40次上游调用/4096输出/1800秒总限；先不写中间文件，最后一次提交每版整数分金额与总计。保留原 P1/P2 不变。

合格条件同时满足：最终独立算术判定、原完成门、保护文件、清理；至少2次真实 compaction；至少7个规范化观察共享同一生效参数摘要；第7次有效读取前不得有工具工作区变更，避免借写文件刷新计数。任务正确性与机制实际触发分别记录。

新增本地 `aster-effect` dev-dependency 复用既有类型和摘要，不复制事实 schema 或引入外部包。原 P1/P2 用已经冻结的 pair 与 test binary 运行，防止新 cohort 的构建改变其执行字节。

首轮 native 尝试被 agent 操作中断时 ACP 已启动；记为已启动中断、费用 Unknown。诊断重试编号2独立保留。后续采用仓库已有的命令级 sha2 opt-level=3 设置加快身份核验，任务、模型、输入、oracle 和预算不变；构建/核验耗时仍单列。


## 实际执行状态

P0通过；P1首轮操作者中断、重试六轮通过；P2固定5个seeds全部运行，5/5产物正确、3/5完整流程合格。P3新G1/G2实例首轮在3次不同观察后response_truncated，0次压缩，任务失败、要求的机制覆盖NOT_EXERCISED。原G1多模块压缩修复未运行；G3/G5真实模型未运行，G4部分覆盖。没有延长预算、替换模型或放宽oracle重算通过。

截至本轮，7个首次尝试与1个诊断重试全部保留。完整消耗和时间摘要、缺失usage、旧版A/B未测及后续资格条件均以GLM报告为准。本文保留未执行cohort的设计，不能视为它们已经通过。

## 追加的独立容量诊断（原4K不改写）

最终4K候选775f6b的Rust五例全通过；单Run7次同参数新观察/7次压缩触发，但最终输出被max_tokens截断、无报告。保留原17requests/254444tokens/468782ms失败，不能把机制覆盖当任务通过。新增单次8192输出条件，只改变测试适配器额度，仍16K压缩预算、40请求、600s prompt/1800s workflow、原模型/请求/336条item记录（另有7条header）/算术oracle/源保护/新观察和压缩门。原4Kwrapper保持4096。新增case不进入原4K分母。

模型配置的声明输出上限必须允许8192，否则在调用模型前拒绝。新relay与子ACL使用同一额度；收集实际请求cap和原生usage。fidelity-audit.json逐字节核对除明确适配器参数/新wrapper/额度记录以外，原fixture和grader字节不变。重新构建并冻结pair+test，保持严格Candidate身份，无canonical回退。被审查的Core路径不变；共享checkout和模型无采样seed不构成隔离因果A/B。原4K失败、新条件结果及全部消耗并列，价格未知。
