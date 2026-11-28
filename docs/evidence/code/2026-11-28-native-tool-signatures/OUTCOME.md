# 原生工具签名的 token 与能力边界

先从同一冻结两轮任务捕获实际 ACP 请求。用同一个合成用户输入，只保留双方各自的工具定义时，Aster 为 5,629 输入 token，独立 DSH 为 5,059。Aster 的完整首请求为 7,202，DSH 为 5,860。工具定义解释了 570 个 token 的差距。单独计量时保留了服务商的真实序列化和模型用量。 [分项回执](first-request-usage.json)

在中继上保留原生函数名、工具数、调用身份和宿主原校验。把可完整表达的字段类型、必填、默认值、模式及说明转换成紧凑对象签名。遇到不支持的 Schema 节点或严格声明时保留原定义。实验没有修改产品、原始工具 Schema、工具执行或授权。短任务的 26 个函数中有 25 个可转换。

| 固定首请求的试验 | 输入 token | 相对原表示节省 |
| --- | ---: | ---: |
| Aster 原表示 | 7,202 | — |
| 完整签名，原生参数为无字段对象 | 6,322 | 880 |
| 完整签名，原生参数为空 | 6,197 | 1,005 |
| 签名加原生必填枚举 | 6,401 | 801 |
| 签名加全部原生必填字段 | 6,882 | 320 |
| 签名加全部原生字段名 | 7,160 | 42 |
| 仅转换无嵌套对象的定义 | 6,747 | 455 |
| 独立 DSH | 5,860 | — |

完整签名的 `program` 两次首次都漏传判别字段 `type`。原宿主校验拒绝了两次调用，修正后均交付。保留全部原生必填字段、以及只保留原生必填枚举的两种试验，都恢复了程序一次调用成功。必填枚举的保留按 Schema 结构实施，覆盖脚本、搜索、Git 和后台操作等定义。 [调用字段与原始拒绝](tool-attempts.json) · [完全签名判分](measured/signature/program_approved/trials.json) · [原生必填枚举判分](measured/discriminator/program_approved/trials.json)

## 原生必填枚举的真实任务

两组 Aster 使用同一个 ACP 二进制，配套 ID 为 `aster-cli-pair-v1:40758b5399df33c8e2eadce2e72de6ff52798b83:aarch64-apple-darwin`，ACP SHA-256 为 `3e1852bdff970b47acdcdd268b7cacef4aa0df054aa6a6b31babad20446b50cb`。两次调换组间顺序。独立 DSH 为 `0.2.2-rc.5`。两组都使用完整目录及同一 DeepSeek Flash，关闭思考，temperature 为 0，短任务和独立能力上限为 1,024 输出 token，六轮为 4,096。原冻结输入、权限、超时和判分不变。共保存 144 次任务试验、1,066 条完整 HTTP 200 厂商回执，以及 20 条单请求计量回执。

| 总 token 中位数，三方各两次 | 原表示 Aster | 紧凑签名 Aster | 独立 DSH |
| --- | ---: | ---: | ---: |
| 冻结两轮 | 31,042 | 27,774 | 25,265.5 |
| 冻结六轮 | 221,235.5 | 193,763.5 | 255,102 |
| 两轮通过 | 2/2 | 2/2 | 2/2 |
| 六轮严格通过 | 0/2 | 0/2 | 1/2 |
| 六轮正确产物与输入保全 | 12/12 | 12/12 | 12/12 |
| 六轮绑定核验 | 12/12 | 12/12 | — |

两轮候选比原表示低 10.53%，比 DSH 高 9.93%。六轮候选比原表示低 12.42%，比 DSH 低 24.04%。六轮未命中输入的中位数分别为 18,286.5、17,572 和 8,087.5。Aster 原组每次多两次 `ls`，候选每次多一次，DSH 有一次多 `glob`，原严格判分均保留。 [两轮](measured/discriminator/short/trials.json) · [六轮正序](measured/discriminator/long/trials.json) · [六轮反序](measured/discriminator/long-reverse/trials.json)

精确读窗口、重复批读、MCP、目录发现、搜索、脚本及带绑定核验的精确编辑，两版各通过 2/2。真实混合批次原表示为 1/2，候选为 2/2。绑定核验以原运行器的文件字节、工具回执、结束状态和效果摘要断言为准。通用 `task_pass` 要求输入文件不变，`qualified_pass` 另含按需能力实验的目录判据。 [精确批读](measured/discriminator/read_window/trials.json) · [混合批次](measured/discriminator/batch_mixed/trials.json) · [MCP](measured/discriminator/mcp_schema/trials.json) · [绑定核验](measured/discriminator/verified_edit_exact_count/trials.json)

## 表单反例

复杂表单原表示通过 1/2，必填枚举方案通过 0/2。两次候选都提交了缺少有效 `root/elements/state` 的 JSON Render 文档，后续重复同一无效调用。仅保留该函数的原生定义时，候选通过 1/2，同批原表示为 0/2。这一隔离只用于诊断。另一轮按原日志中身份匹配的失败回执为同一工具恢复完整原生声明，确认两个候选均触发恢复，最终仍为 0/2，该轮原表示 1/2。工具名和真实验证相同不足以证明模型生成的结构化负载能力等价。 [全量试验](measured/discriminator/form/trials.json) · [原定义隔离](measured/form-native-isolation/form/trials.json) · [失败后恢复](measured/failure-fallback/form/trials.json) · [恢复的逐请求证据](measured/failure-fallback/form/provider-usage.jsonl)

在同一合成首请求上另合并了常驻的重复规则段，保留原环境、会话能力和语言段。仅替换这份规则段为 6,867 输入 token，与必填枚举方案合用为 6,066。这个组合比 DSH 首请求仍多 206，常驻规则试稿只做了计量。 [规则试稿](standing-probe.md) · [组合计量](standing-request-usage.json)

## 复跑

macOS 27.2.1 arm64。通过 `DEEPSEEK_API_KEY` 提供已获准的凭据。把 `ACP`、`DSH` 和 `EMBEDDING` 指向各自二进制和 embedding 资产目录，输出路径应尚未存在。

```sh
python3 matrix.py --scenario short --projection native --capture-first \
  --acp "$ACP" --dsh "$DSH" --embedding "$EMBEDDING" --out "$CAPTURE"
python3 measure_first.py "$CAPTURE" "$METER"
python3 measure_first.py "$CAPTURE" "$STANDING_METER" --standing standing-probe.md
python3 matrix.py --scenario long --projection discriminator \
  --acp "$ACP" --dsh "$DSH" --embedding "$EMBEDDING" --out "$LONG"
```

六轮增加 `--reverse` 复跑反序。独立能力用相同运行器和 `--scenario read_window`、`batch_mixed`、`program_approved`、`form` 等场景。两个表单隔离分别追加 `--native-function ask_user_form` 和 `--native-on-failure`。源码只新增评测和证据，运行了 Python 语法、原生声明保留、失败回执身份匹配、逐笔用量等式、证据字节和 `git diff --check` 核对。本机现有构建沿用上一轮已经验收的 CLI 和 ACP。 [前一轮构建记录](../2026-11-28-grep-display-projection/OUTCOME.md)

## Explicitly not claimed

紧凑签名试验未进入 Aster 默认工具表示。真实表单和六轮严格判分尚不足以证明能力保持。短任务总量仍高于 DSH。组合常驻试稿只有首请求计量证据。原有的产品、权限策略、配置和已运行会话未被实验替换。用量含缓存命中，未测账单金额或其他模型与平台。
