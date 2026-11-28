# Core 原生按需工具目录

`tool_presentation { mode = "deferred" }` 在现有宿主 ACL 中显式启用。本轮支持拥有原会话调用端口的主请求，传输为接收已关闭思考模式的 DeepSeek Flash Chat Completions。现有权限可见性、模型工具筛选、工具实际参数验证和确认均走已准入的路径。

Core 投影同一批工具的参数签名、简短原字段说明和原生必填枚举或常量。短目录的参数说明与模式标记来自 Schema，每个真实工具名称仍可直接调用。有效 `load_tool_definitions` 回复从该次模型请求已准入的完整定义里按精确工具名取值。查阅是模型侧消息，后续每次模型请求都在原调用端口获取许可、运行真实流式传输并逐笔记录用量。自动展开限于该次模型决策过程，后续模型决策按当前目录重新评估。Core 对未知工具、同一次响应夹带实际执行，以及缺少新信息的重复查阅改用完整目录。保留原工具名称发生冲突时的原始调用路径。大小门槛以现有 DeepSeek 传输实际保留的定义字节为基准。

## 最终配套构建的复验

对最后的大小门槛调整重新构建并复跑。Aster 两组使用同一份 ACP 文件，SHA-256 `70480ce0f6007392e533fd98b40bab73f364cb342967f03f1efbd7d5d446ac9e`。冻结任务的模型、顺序、请求设置及判分沿用下面的对照。

| 冻结任务中位数 | 通过 | 总 token | 未命中缓存的输入 token | 主请求 |
| --- | ---: | ---: | ---: | ---: |
| Aster 完整工具定义 | 3/3 | 31,350 | 1,200 | 4 |
| Aster 原生按需目录 | 3/3 | 23,344 | 1,347 | 4 |
| 独立 DSH | 3/3 | 24,968 | 969 | 4 |

候选总 token 比同构建的完整目录低 25.5%，比 DSH 低 6.5%。MCP 用例每组通过 2/2，总 token 中位数由 31,653 降至 28,661.5。两次候选均完成一次目录查阅，厂商和 ACP 各计 5 次主请求，主请求 token 逐次一致。写入核验交付每组通过 2/2，总 token 中位数由 47,155 降至 35,051，四次的最终文件字节和绑定验证摘要一致。共通过 17/17。[冻结任务](delivered/frozen/trials.json) · [MCP](delivered/mcp_schema/trials.json) · [写入核验交付](delivered/verified_edit_exact_count/trials.json) · [MCP 对账](mcp-accounting.json)

## 原生流式全矩阵

macOS 27.2.1、arm64、Rust 1.97.1。两组 Aster 使用同一份 ACP 文件，SHA-256 `5dfd14e88e325fd9585bdd4719453ac68140d4597bc2de084e39374c8e8ab1a3`。该试验在最终核对小目录字节门槛前执行。中继转发实际流式响应并记录用量，候选启动器仅配置仓库中的 Core 模式。两组 Aster 的本地环回端点都显式声明 `none` 推理能力，厂商控制保持关闭思考、temperature 0、最大输出 1,024。独立 DSH 为 `0.2.2-rc.5`。沿用[冻结任务和原判分](../2026-11-27-deepseek-harness-token-comparison/compare.py)。

| 冻结任务，每项各自中位数 | 通过 | 总 token | 未命中缓存的输入 token | 主请求 |
| --- | ---: | ---: | ---: | ---: |
| Aster 完整工具定义 | 3/3 | 31,310 | 1,210 | 4 |
| Aster 原生按需目录 | 3/3 | 23,438 | 1,289 | 4 |
| 独立 DSH | 3/3 | 24,880 | 894 | 4 |

原生候选比同一 Aster 少 25.14% 的总 token，比 DSH 低 5.80%。总量为厂商回执的主请求及辅助请求之和。[判定](full-catalog/frozen/trials.json) · [完整用量](full-catalog/frozen/provider-usage.jsonl)

| 独立完整目录用例 | 完整定义总 token | 原生按需总 token | 通过（完整 / 候选） | 主请求（完整 / 候选） |
| --- | ---: | ---: | ---: | --- |
| 精确分页 | 30,949 | 22,844 | 2/2 · 2/2 | 4 · 4 |
| 工作区内容搜索 | 40,509 | 30,466.5 | 2/2 · 2/2 | 5 · 5 |
| Program 脚本 | 30,885 | 29,063 | 2/2 · 2/2 | 4 · 5 |
| 混合 Batch | 31,317 | 23,048 | 2/2 · 2/2 | 4 · 4 |
| MCP 精确参数 | 31,467 | 28,690 | 2/2 · 2/2 | 4 · 5 |
| 写入、校验回执及最终交付 | 47,146 | 35,243.5 | 2/2 · 2/2 | 6 · 6 |

Program 的两次候选均第一次提交了有效脚本参数。相较完整版本，第二回合各额外调用一次 `ls`。MCP 的每次候选运行都查阅一次当前完整定义并执行真实工具。厂商和 ACP 报告的 5 次主请求数量及总 token 逐条相同，完整工具组的 4 次请求也相符。独立完整目录场景共运行 24 次，全部通过任务原样判分。[所有场景](full-catalog/) · [逐次 MCP 账本对账](mcp-accounting.json)

首次试验里的环回网关缺少能力声明，候选两次均回退到普通完整目录。[回退记录](fallback-before-capability-declaration/mcp_schema/trials.json)

在提供具体交付指引之前，写入用例的两组都有实际核验成功后再次提出未授权命令的记录，四次完整任务判定均失败。Core 现在在已有绑定验证的观察旁提示模型使用现有证据报告所需结果，并保留用户要求的剩余工作。相同授权和判分下，两组各通过 2/2。四次的目标文件逐字节相同、核验摘要相同，且第一回合交付真实校验输出。[原失败](before-verification-guidance/verified_edit_exact_count/trials.json) · [修复后的判定](full-catalog/verified_edit_exact_count/trials.json)

## 验证与复跑

- `cargo test --locked -p aster-core --lib deferred_catalog`，4 项通过。覆盖原生必填值、短说明、原定义不变、大小门槛基于原传输投影、无权的查阅、同批实际工具不执行、查阅前后同一会话用量累加。
- `cargo test --locked -p aster-core --lib presentation`，18 项通过。`cargo test --locked -p aster-core --lib final_delivery`，3 项通过。
- `cargo clippy --locked -p aster-core --all-targets --no-deps` 与 `cargo check --locked --workspace --all-targets` 通过，保留已有警告。全工作区格式检查报告已有的 38 个文件差异。
- `cargo build --locked -p aster -p aster-acp --bin aster --bin aster-acp`。产物位于当前工作区的 `target/debug`。

冻测用同一份 ACP 文件打开两种 ACL 模式。通过环境变量 `DEEPSEEK_API_KEY` 提供现有凭据。`$OUT` 应为尚未创建的路径。

```sh
python3 docs/evidence/code/2026-11-27-model-selected-tool-catalog/native.py \
  --release "$ACP" --acp "$ACP" --dsh "$DSH" --embedding "$HOME/.local/bin/embedding" \
  --out "$OUT" --order release,candidate,dsh candidate,dsh,release dsh,release,candidate
```

跨能力独立场景使用 [full_catalog.py](../2026-11-27-model-selected-tool-catalog/full_catalog.py) 的 `--native-catalog` 模式，以 `--scenario` 选择用例。

## Explicitly not claimed

目前产品模式显式启用。现有证据覆盖所列宿主、模型、开关和用例。默认采用仍需要更广的交互、恢复、长会话和其他平台能力证据。厂商未命中缓存输入的用量单列，实际账单金额没有参与测试。
