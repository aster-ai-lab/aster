# 显式目标规划合并请求与记忆权限投影

显式规划需要生成目标与计划，且当前 Run 没有现成结果时，Core 在一个模型请求中接收两份独立对象。目标条件、计划标题、步骤、依赖和各自验收条件分别解析。单项缺失或无效时，调用原有的对应路径。取消和不可重试的模型或预算控制错误仍由原有错误语义处理。带宿主逐调用预算钩子的会话保留原有请求形状。

完成回合仍进入记忆抽取。关系指令随本次实际提供的既有记忆 ID 出现，模型只可提交宿主能验证的替换和冲突关系。无 ID 时的抽取仍携带语言规则、置信阈值、长期记忆与进化字段以及完整输入证据。

## DeepSeek Flash 实测

macOS 27.2.1 arm64，Rust 1.97.1。厂商端点为 Chat Completions。关闭思考，temperature=0，输出上限 1024。每个场景各组交替执行 3 次，下表各列为各自的中位数。

| 场景 | 指标 | 优化前 | 优化后 | DSH 0.2.2-rc.5 |
| --- | --- | ---: | ---: | ---: |
| 原冻结任务 | 全程总 token | 33,442 | 33,424 | 24,965 |
| 原冻结任务 | 辅助记忆请求输入 token | 1,138 | 1,066 | — |
| 重复观测 | 全程总 token | 38,144 | 38,058 | — |
| 重复观测 | 辅助记忆请求输入 token | 1,083 | 999 | — |

原冻结任务 9/9 通过，48 份完整用量。重复观测 6/6 通过，36 份完整用量。记忆输入分别下降 6.33% 和 7.76%。默认任务中的规划模式没有进入合并分支，所以全程节省分别只有 18 和 86 个 token。用量记录中的 release 是上一轮本机 Aster。实测 ACP SHA-256 优化前为 `58495ed305a2787bf14e365d0e31208ca5017d435b5a97c97becccf95bb60d6f`，优化后为 `088944ae96f0fe12404d07c9a7117ddddfc21561b1c5ac748978d68370e9db2b`。

- [冻结参数](frozen/manifest.json)、[判分](frozen/trials.json) 和 [厂商用量](frozen/provider-usage.jsonl)。
- [重复观测参数](repeated/manifest.json)、[判分](repeated/trials.json) 和 [厂商用量](repeated/provider-usage.jsonl)。
- 单独提出中文长期记忆要求后，实际 DeepSeek 的两版 ACP 均在进程重开后查到同一类 1 条持久记录。见 [记录](memory-reopen.json)。

## 规划能力对照

SDK 的独立场景包含英文库存对账和中文翻译审计。对照组经透传的宿主预算钩子进入原有两条路径，候选组进入合并路径。比较脚本按厂商请求统计规划子流程的用量，判分读取实际 GoalExtracted 和 PlanningEnd 事件并核对验收条件、可核验步骤、指定交付物和工作区没有写入报告。[复现脚本](compare-planning.py)

带工具的探索组有 12 次任务和 55 份完整用量。旧双请求路径 5/6 通过，合并路径 4/6 通过。3 次失败都出现在中文任务的后续主回合，模型尝试调用被拒绝的 Bash，最终触发拒绝循环。英文任务的两组均 3/3 通过。保留这些结果，后续工具隔离组使用同一用户提示和判分，由两组宿主一起关闭工具入口。[探索组判分](planning-with-tools/trials.json) · [厂商用量](planning-with-tools/provider-usage.jsonl) · [失败原因](planning-with-tools/failures.json)

工具隔离首轮 10/12 通过，共 58 份用量。两组的中文主回合各出现一次相同的 Bash 拒绝。保留原有最终回合通过条件，复测额外记录目标和计划的分项断言。[初次隔离判分](planning-without-tools/trials.json) · [厂商用量](planning-without-tools/provider-usage.jsonl) · [失败原因](planning-without-tools/failures.json)

复测的 12 次规划均通过分项判分，完整回合 11/12 通过。61 份厂商用量齐全，所有请求的工具定义数量均为 0，主回合仍有一次相同拒绝。下表统计规划子流程中位数，覆盖两个语言场景，每组每个场景各 3 次。[分项和全程判分](planning-observed/trials.json) · [厂商用量](planning-observed/provider-usage.jsonl) · [失败原因](planning-observed/failures.json)

| 场景 | 指标 | 双请求 | 合并请求 |
| --- | --- | ---: | ---: |
| 英文库存对账 | 规划请求 | 2 | 1 |
| 英文库存对账 | 规划输入 token | 615 | 359 |
| 英文库存对账 | 规划总 token | 1,062 | 740 |
| 中文翻译审计 | 规划请求 | 2 | 1 |
| 中文翻译审计 | 规划输入 token | 637 | 370 |
| 中文翻译审计 | 规划总 token | 1,079 | 746 |

规划总 token 分别下降 30.32% 和 30.86%。复测中同场景各组的首个主请求体字节数相同。上述降幅只针对规划子流程。

## 本地验证

| 命令 | 实际结果 |
| --- | --- |
| `cargo build --locked -p aster-core -p aster-acp --lib --bin aster-acp` | 通过。 |
| `cargo build --locked -p aster -p aster-acp --bin aster --bin aster-acp` | 从最终源码构建通过。TUI 和 ACP 构建 ID 相同，为 `aster-cli-pair-v1:17c307bf20eff09301d750e1ce1b959d06e60842:aarch64-apple-darwin`。 |
| `cargo test --locked -p aster-core --lib -- agent_api::tests::stream::explicit_planning agent_api::tests::stream::auto_planning planning::llm_planner agent::memory_extraction_runtime memory_retention_tests --test-threads=4` | 95 项通过。覆盖单次合并、只补目标、只补计划、带预算守卫的原双调用、自动规划、记忆写入和拒绝。 |
| `cargo test --locked -p aster-acp --lib -- engineering prompt_boundary retrieval_seam count_limit session_setup stop_interrupts_unanswered_permissions_questions_and_forms --test-threads=4` | 40 项通过、3 项忽略。 |
| `cargo test --locked -p aster-core --test test_control_loop_deepseek_e2e --no-run` | 构建通过。实测测试二进制跑过真实核验写入与目标重开，2 项通过。 |
| `real_agent_recovers_from_an_outside_workspace_refusal_in_the_same_turn`，实际 DeepSeek 和实测 ACP | 1 项通过。一次拒绝后同轮完成已授权的读取。 |
| `cargo clippy --locked -p aster-core --all-targets --no-deps --features ci-all,deep-live` | 通过。复核新增路径，保留原有警告，包括旧目标测试中的 Copy 类型 clone。 |
| `cargo check --locked --workspace --all-targets` | 通过。保留原有告警和依赖未来兼容提示。 |
| `forms_catalog::kimi_catalog_native_viewport_and_typed_values_match_source` | 真实 Kimi 原生 PTY，13 种组件、8 个带类型值和工作区文件保真检查通过。后续追加的源码变化只有规划测试诊断。[原生断言](native-kimi.txt) |
| `shipped_native_form_submit_cancel_and_reject` | 用最终配对二进制重跑，1 项通过，覆盖表单校验、提交和取消。[原生断言](native-final-pair.txt) |
| `cargo fmt --all -- --check` 与 `git diff --check` | 通过。 |

## Explicitly not claimed

- 工具隔离组用于核对规划子流程。带工具的中文主回合样本不足以证明整回合可靠性持平，失败没有计入通过数。
- 厂商报告的是 token，缓存状态与生成字数会波动。本轮没有测账单金额。
- 本机构建和原生 TUI 验证跑在 macOS。Windows/Linux 原生运行、Studio GUI 和其他模型组合不在此次范围内。
