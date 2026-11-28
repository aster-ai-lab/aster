# 在原生工具列表展开完整定义

按需目录接受精确名称的查阅后，下一次请求把该工具的原定义放回原生工具列表中的同一位置。查阅回执只确认定义已在列表中。未请求的定义继续使用短形态。查阅仍限于当前模型决策的已准入定义。当前目录重新检查字节收益，收益不足、混批、无效名称或查阅额度用尽时走普通完整目录。运行能力证据重新生成投影，核对位置、描述和 Schema。全部实际工具仍通过原执行和授权路径，查阅回执只存在于当次发送上下文。

## DeepSeek Flash 流式对照

macOS arm64，Rust 1.97.1。两组 Aster 都显式开启按需目录。原 ACP SHA-256 为 `6b5609d42ad1f29e8e3899b2a6f649be18c12dd7878e04cb465d38156ab23c62`，候选 ACP 为 `d2d32eedf6b52f5aec9560861e0ef28d1ec5788cb296f947f82ddecc3667e987`。同一官方 DeepSeek Flash，思考关闭、temperature 0、最多输出 1,024。沿用原脚本任务、请求顺序、权限夹具和判分，新增的记录只保留查阅回执的字节数。共 33/33 通过，所有厂商用量齐全。

| MCP 精确输入，每组 2 次 | 原格式 | 原生展开 |
| --- | ---: | ---: |
| 完成查阅后工具回执字节数 | 396 | 61 |
| 随后模型请求字节数 | 21,478 | 21,353 |
| 随后模型请求输入 token | 5,440 | 5,414 |
| 主请求总 token 中位数 | 27,393.5 | 27,348.5 |
| 全流程总 token 中位数 | 28,637.5 | 28,686.5 |
| 精确调用通过 | 2/2 | 2/2 |

查阅之后的请求在两次重复中均减少 26 输入 token。两组每次均有 5 次主请求，厂商与 ACP 的主请求用量逐次相同。主请求中位数降 45，辅助请求用量中位数增加 94，全流程中位数增加 49。[原始回执](short/mcp_schema/provider-usage.jsonl) · [逐次用量对账](mcp-accounting.json)

| 两轮冻结比较 | 通过 | 总 token 中位数 | 未命中缓存输入 | 主请求 |
| --- | ---: | ---: | ---: | ---: |
| 上一版 Aster 按需目录 | 3/3 | 23,442 | 1,281 | 4 |
| 当前 Aster 按需目录 | 3/3 | 23,516 | 1,256 | 4 |
| 独立 DSH `0.2.2-rc.5` | 3/3 | 24,981 | 799 | 4 |

冻结比较没有触发查阅。候选比同轮 DSH 低 5.86%，比上一版中位数高 74。其余独立场景覆盖精确分页、混合 Batch、搜索、Program 和写入后绑定核验，两组各通过 10/10。每次验证写入均有绑定摘要，最终文件与原冻结字节一致。[冻结任务](short/frozen/trials.json) · [独立场景及残留指标](short/)

复跑短场景时，[原生冻结驱动](../2026-11-27-model-selected-tool-catalog/native.py) 使用 `--reference-deferred --release "$BASELINE_ACP" --acp "$CURRENT_ACP"`，[独立场景驱动](../2026-11-27-model-selected-tool-catalog/full_catalog.py) 使用 `--native-catalog --reference-acp "$BASELINE_ACP" --acp "$CURRENT_ACP"`。两者从 `DEEPSEEK_API_KEY` 读取现有凭据。

## 六轮文件修订

沿用 `real_native_file_workflow_multiple_user_turns` 的六轮编辑和独立 JSON 判分。每轮替换订单和政策数据，测试模型重新读取当前输入、生成报告并保留来源和无关文件。宿主原有的字节核验绑定每轮改动摘要。两种工具呈现使用同一对本机构建二进制，`ASTER_LH_NATIVE_TOOL_PRESENTATION` 显式选择 `adaptive` 或 `deferred`，两者均关闭思考，输出上限均为 4,096。

第一组试验的两种呈现都通过六轮，每侧最后一条已发出的后台请求在立即关闭进程时没有完整回执。保留[完整目录](long-before-usage-drain/adaptive/result.json)与[按需目录](long-before-usage-drain/deferred/result.json)的记录。复跑使用同一冻结判分，评测在已发出请求取得用量且请求数稳定后关闭进程，最多等待 20 秒。

| 完整计账复跑，每种呈现一组 | 完整目录 | 按需目录 |
| --- | ---: | ---: |
| 文件判分、来源保全及绑定核验 | 6/6 | 6/6 |
| 全流程总 token | 289,321 | 218,293 |
| 输入 token | 284,669 | 212,884 |
| 输出 token | 4,652 | 5,409 |
| 未命中缓存的输入 token | 31,241 | 19,744 |
| 主模型请求 | 26 | 24 |
| 辅助模型请求 | 12 | 12 |
| 主请求内的工具定义字节数 | 19,334 | 12,810 |

按需目录全流程少 71,028 token（24.55%）。两组每一轮的七字段报告一致，权限确认各 6 次。共 74 个上游响应，用量与缓存计数均可逐项核对。按需组的 24 个主请求都携带了按需目录。配套构建 ID 为 `aster-cli-pair-v1:6d776cbe7cb39b152efa8d5ba95f98d7261be164:aarch64-apple-darwin`，两组 ACP SHA-256 均为 `0fc992f694d5b6be2052288ea5a5627e6ceef4a13a203c4cbbfa9891edbd3436`。[完整目录和厂商用量](long/adaptive/result.json) · [按需目录和厂商用量](long/deferred/result.json)

复跑使用 `cargo test --locked -p aster-acp --test long_horizon_real_llm --no-run` 后重新构建 CLI 配套，从生成的测试文件运行 `real_native_file_workflow_multiple_user_turns --ignored --exact --nocapture`。两侧的 `ASTER_LIVE_MODEL=deepseek/deepseek-flash`、`ASTER_CONFIG_FILE` 指定同一份显式声明 `none` 的受信 ACL，`ASTER_LH_EVIDENCE_DIR` 指向不同的证据目录。

## 本地验证

| 命令或用例 | 结果 |
| --- | --- |
| `cargo test --locked -p aster-core --lib deferred_catalog` | 4 项通过。验证精确原生 Schema、查询边界和投影成本，覆盖混批、未知工具、提交内容被替换以及两次真实模型调用的账本。 |
| Core 单元测试二进制过滤 `presentation` | 18 项通过。 |
| `cargo build --locked -p aster -p aster-acp --bin aster --bin aster-acp` | 通过。重新成对构建。 |
| `boundary_e2e --skip e_bash --nocapture` | 对最终配对二进制 9 项通过，保留已知 `e_bash` 的既有失败记录。 |
| `cargo check --locked --workspace --all-targets` 和 `cargo clippy --locked -p aster-core -p aster-acp --all-targets --no-deps` | 通过，保留已有告警。计账等待调整后的 `cargo clippy --locked -p aster-acp --test long_horizon_real_llm --no-deps` 通过。 |
| `cargo fmt --all -- --check` 与 `git diff --check` | 全工作区格式检查仍有 38 个已有差异文件，diff 检查通过。 |

## Explicitly not claimed

按需目录仍为显式选择。覆盖 DeepSeek Flash 关闭思考时的现有冻结任务、六轮文件场景和所列独立能力。没有证明跨模型、其他平台和任意长期任务的能力等价，也没有测量实际账单。长任务没有独立 DSH 样本。
