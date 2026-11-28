# 工作区变动与输入缓存

在先前六轮评测里，工作区记忆从第三轮起进入用户消息。到下一轮，事实日志重建原始消息，被修改的历史用户消息还原，后续工具历史也失去这一次的缓存命中。实际请求中，从第三轮到第四轮时有一份 871 字节的消息还原为 308 字节，从第四轮到第五轮时有一份 1,427 字节的消息还原为 308 字节。对应的 ACP 事实日志记录了 `memory` 上下文摘要。两次额外目录调用分别发生在读取缺失文件之后和助手发出首个工具批次时。现有结果不足以将它们归因于同一个读文件故障。[原始请求指纹](../2026-11-27-verification-evidence-reuse/final/long-a/provider-usage.jsonl)

## 当轮记忆放在工具结果之后

候选在首个请求保留原投影，工具回执到达后把同一份 XML 记忆移到当前回执末尾。完整回执和图像、工具权限、召回输入、用户消息和持久化格式沿用原路径。以 DeepSeek Flash、关闭思考、temperature 0、输出上限 4,096 固定执行两次相反顺序的六轮比较，保持原输入和独立判分。两组 Aster 都显式启用按需目录。独立 DSH 为 `0.2.2-rc.5`。

| 六轮对照 | 总 token 中位数 | 未命中缓存输入中位数 | 第 3–6 轮主请求未命中输入中位数 | 正确文件与保护 | 严格工具通过 |
| --- | ---: | ---: | ---: | ---: | ---: |
| Aster 原投影 | 172,472 | 14,889.5 | 6,671.5 | 12/12 | 1/2 |
| Aster 尾部候选 | 168,991 | 19,482 | 10,056 | 12/12 | 1/2 |
| 独立 DSH | 268,785.5 | 7,394 | 4,659 | 12/12 | 1/2 |

两组 Aster 的 24 轮都取得宿主绑定核验。工具限制失分来自 Aster 反向比较各一次 `ls` 和 DSH 正向比较一次 `glob`。两轮所有 179 个厂商请求都返回 HTTP 200 和完整用量。候选两次总 token 分别为 152,916 和 185,066，基线分别为 171,665 和 173,279。候选尾部的历史改写范围缩短，不过原先在当轮工具链中保持稳定的记忆，需要在每次工具结果更新时重新处理。只看第 3–6 轮，候选未命中输入两次分别为 9,752 和 10,360，基线分别为 7,424 和 5,919。两场试验后撤回候选源码，评测运行器保留记忆所在消息角色，用于区分跨轮失效与轮内失效。[正向原始判分](long-a/trials.json) · [反向原始判分](long-b/trials.json) · [用量汇总](summary.json)

## 工作区信息保持 system 角色

直接使用此前隔离任务捕获的 DeepSeek 请求及全部 26 项原工具定义。基线将常驻提示与工作区信息放在一条 system 消息中。候选在同一位置发送两条 system 消息，内容与顺序一样。每组先发目录 A，再换到目录 B，再重复目录 B。保持原目录 B 时，两种方案分别只需处理 244 和 245 个未命中输入 token。换到目录 B 时，单段为 6,516，双段为 373。每个请求都收到真实厂商用量。[原始前缀计量](environment-prefix.json)

另用一条要求复述宿主目录的独立提示进行反序复验，目录分别含空格和中文，工具保持可用。两边四个响应都逐字复述各自环境目录，也没有产生工具调用。更换目录时，单段为 6,501 个未命中输入，双段为 358。总输入约为 7,910，双段比单段多 1 token。将工具选择设为 `none` 的对照实际只处理约 1,823 输入 token，这时两种写法的跨工作区命中几乎一致。测量表明失效主要涉及常驻工具目录，不能用字节长度或消息数量来估算这部分缓存成本。[工具可用时](environment-grounding-with-tools.json) · [关闭工具对照](environment-grounding.json)

## 实际 ACP 请求

Core 原型使用已准入的常驻提示字节作为边界。第一条 system 消息保留该前缀，第二条保留其余 system 内容。两条相连可逐字还原原系统文本。工作区内容里的标记无法决定边界，工具选择和权限沿用原调用路径。两套原型均在本轮末撤回。

| 六轮对照 | 总 token 中位数 | 未命中缓存输入中位数 | 正确文件与保护 | 严格工具通过 |
| --- | ---: | ---: | ---: | ---: |
| Aster 现有投影 | 182,361.5 | 17,260.5 | 12/12 | 0/2 |
| Aster 双 system 原型 | 181,359 | 15,125 | 12/12 | 0/2 |
| 独立 DSH | 261,362 | 7,399.5 | 12/12 | 1/2 |

184 个厂商请求全部取得 HTTP 200 和完整用量。第一处工作区里，Aster 原版和原型首个工具请求分别消耗 4,127 和 4,128 个未命中输入 token。第二处工作区里，原版仍为 4,127，原型降至 416。实际请求的第一条 system 指纹一致，第二条随目录变化。两组 Aster 均获得 12 轮绑定核验。额外目录或搜索调用造成两组仍有严格判分失败。[正向](system-a/trials.json) · [反向](system-b/trials.json) · [指纹与汇总](system-split-summary.json)

在现有精确命令任务中，原投影 4/4，双 system 2/4。原型失分的两次都先发出当前批准范围之外的 Bash 调用，宿主拒绝保护了工作区，交付仍缺少要求的验证。原型通过了混合批次 2/2 和 MCP 2/2。精确命令的复现使这一投影的默认和按需目录版本都退出本轮构建。[精确命令一](system-verified_edit_exact_count/trials.json) · [精确命令二](system-verification-repeat/trials.json) · [混合批次](system-batch_mixed/trials.json) · [MCP](system-mcp_schema/trials.json)

排查候选范围时发现 `--native-catalog` 的包装器已向两组写入 `tool_presentation { mode = "deferred" }`。额外手动包装重复声明了这一 ACL 段，触发两组共四次初始化拒绝，未取得模型用量。新增 `--default-catalog` 使用普通转接器，不附加工具呈现配置或工具定义投影。四次默认目录试验的首条 system 和用户消息的 SHA-256 一致，均只有一条 system 消息且未发布延迟目录。原版 1/2，按需限定原型 0/2。相同首请求也出现过不同的模型工具决定，这些默认目录样本不足以比较全面能力。按需限定原型自己的六轮产物 6/6，有一次额外 `ls`。[重复 ACL 记录](guarded-deferred-exact/trials.json) · [默认目录对照](actual-default-exact/trials.json) · [限定原型六轮](guarded-full-revision/trials.json)

## 本地验证

- 使用原 [六轮驱动](../2026-11-27-six-turn-dsh-comparison/revisions.py)、原冻结夹具、`--reference-deferred` 和 `--embedding`。运行顺序为 `candidate release dsh` 与 `dsh release candidate`。基线 ACP SHA-256 为 `bb14c0113330c13a6c7eb69107739a5ee12d10ba0dcb744715506cf9ed5c6e8c`，候选为 `87dcc363524e0455d51493852cb6d51fee2dc602e1f4e98d365ac50b09754fb0`。
- 候选执行 `cargo test --locked -p aster-core --lib -- recalled_context_follows prepared_context_does_not_replace prepare_turn_context verification_preview tool_history_tests --test-threads=4`，34 项通过。`cargo check --locked --workspace --all-targets` 和 `cargo clippy --locked -p aster-core --all-targets --no-deps --features ci-all,deep-live` 通过，留有已有警告。全工作区格式检查仍报告已有的 38 个文件。
- 撤回后从根工作区执行 `cargo build --locked -p aster -p aster-acp --bin aster --bin aster-acp`，配套构建通过，恢复到实验前的源码 ID `eabcfad3d630729b5e8e9d2d539caef9d14bd2fb`。以该配套运行 `boundary_e2e --skip e_bash --nocapture`，9/9 通过。排除项见[原生边界记录](../2026-11-27-host-denial-and-pending-verification/OUTCOME.md)。
- 对恢复后的源码运行 `cargo test --locked -p aster-core --lib -- llm::anthropic llm::openai turn_context prepared_context_does_not_replace --test-threads=4`，112 项通过。
- `cargo test --locked -p aster-acp --test long_horizon_real_llm --no-run` 通过。运行既有 `real_native_file_workflow_multiple_user_turns --ignored --exact --nocapture` 时，`glm-01/glm-5.3` 的 11 次上游请求均返回 HTTP 429，0 轮完成。进程和中继完成清理。[GLM 原始结果](glm-result.json)
- `kimi-01/k3.1-pro` 使用同一恢复后配套和现有六轮验证。6/6 轮产物通过独立判分，6/6 保持输入字节，6/6 获得宿主绑定核验。35 次上游 HTTP 200 均收到用量，合计 229,719 token。[Kimi 原始结果](kimi-result.json)
- 双 system 原型配套源码 ID 为 `059e13e84102b81b77ac25cfa4e5d66b2ff04087`，ACP SHA-256 为 `58c64eea14111085977a10bf44be4ad0330d1498a72e610eb9a3aa13b562f3d7`。原型执行模型、记忆和工具历史定向测试 144 项通过，真实终端边界 9 项通过，工作区检查和 Core `ci-all,deep-live` 特性 Clippy 通过。最终评测器的 Python 语法验证通过。

## Explicitly not claimed

候选缓存收益来自协议探针和试验二进制。实际配套保留经过先前验证的投影。GLM 厂商限流和长任务的严格工具失分仍待独立解决。未估算计费，也未用所列样本推断其他任务和模型的全面等价。
