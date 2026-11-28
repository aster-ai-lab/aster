# DeepSeek 工具定义按需展开试验

完整工具定义在每次主模型请求都会占据输入上下文。更短的输出、提示缓存和压缩旧工具结果分别影响输出、计费和历史输入，仍会留下这部分固定开销。本试验保留每个当前可调用工具的名称、类型签名和用途，让模型在需要约束、解释或示例时查阅当前目录中的完整定义。真实工具调用始终由同一份 Aster ACP 处理，沿用原参数校验、宿主权限和执行路径。中继仅应答实验用的定义查询。

## 冻结对照

DeepSeek Flash，关闭 thinking，temperature 0，最大输出 1,024。每组依次轮换三个对照的执行次序。两组 Aster 都使用同一个 ACP 文件，SHA-256 为 `e627e974b2286e66f7b32df88daf434f64d54acb9fee1374c64e371789dccc22`。DSH 为 `0.2.2-rc.5`。使用[上一轮的冻结输入和判定程序](../2026-11-27-deepseek-harness-token-comparison/compare.py)。统计一次两回合试验所有带用量的上游请求，包括辅助请求和任何定义查询，报告各项独立中位数。

| 对照 | 通过 | 总 token | 输入 token | 输出 token | 未命中缓存的输入 token | 主请求 | 带用量的全部请求 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Aster 完整定义 | 3/3 | 31,350 | 31,059 | 289 | 1,210 | 4 | 6 |
| Aster Chat 短签名 | 3/3 | 18,594 | 18,322 | 272 | 1,176 | 4 | 6 |
| 独立 DSH | 3/3 | 24,873 | 24,541 | 332 | 829 | 4 | 4 |

Chat 短签名较同一 Aster 完整定义减少 40.69%，较 DSH 减少 25.24%。三次成功的候选运行在 12 次主请求中改写了工具目录，每次链接到 26 个完整定义，实际查阅 0 次。带工具的请求中位数用量从 30,317 降至 17,586。其余请求中位数从 1,031 降至 1,014。候选另有一次连接被对端重置，中继记录了缺少用量的 502，重试后通过原判定。DSH 的未命中输入中位数仍更低，缓存和实际计费需单独解读。原始记录见[判定结果](frozen-chat-gated/trials.json)和[上游用量](frozen-chat-gated/provider-usage.jsonl)。

另一项两回合任务要求在两个槽位上分别重新执行同一条读取命令，再返回精确值。两组 Aster 共通过 6/6，主请求均为 4 次，完整定义 36,087 token，Chat 短签名 23,367 token，减少 35.25%。DSH 只参与冻结对照。[判定结果](repeated-chat-gated/trials.json)

备用的原生延迟工具定义使用 DeepSeek 的 Anthropic 兼容协议。在冻结对照中完整定义、原生延迟、DSH 分别为 31,286、16,662、25,046 token，均通过 3/3。在重复观察对照中为 36,047 和 21,365 token，均通过 3/3。独立的精确约束探针要求回传参数说明里的文字，两组均返回正确字面值。完整定义 1 请求、366 token，原生延迟 2 请求、962 token，含 1 次查阅。小目录的查阅开销可以超过可节省的输入。[原生冻结结果](frozen-signatures/trials.json) · [重复观察结果](repeated-signatures/trials.json) · [精确约束探针](activation.json)

## 能力回归与边界

从现有真实模型测试抽取了 search、glob、git、program、batch、ask_user、edit、patch、MCP 与 Skill，各运行两次。实验成本门槛为省下的完整定义字节多于该请求的系统提示字节，全部 20 个能力测试都保留完整定义。两组分别通过 9/10 和 8/10。两次 patch 均在验证时碰到 Bash 拒绝熔断，候选的 batch 则由模型为嵌套 read 选择了不符合当前完整定义的 `path` 字段。这组测试验证了门槛保留小目录的现有路径。大目录下的能力覆盖仍有待验证。[能力试验判定](capabilities-chat-gated/trials.json)

关闭成本门槛，沿用这十个现有能力用例。候选组在 27 次上游请求中启用了 Chat 短签名，通过 9/10。完整定义组用 26 次上游请求，通过 8/10。两组仍在 patch 的验证阶段碰到 Bash 拒绝熔断。完整定义组的 batch 这次失败。ask_user 在两组均通过，短签名多一次主请求，单次用量从 3,568 增到 5,094。另一个 Chat 探针正确查阅并使用完整定义中才出现的参数字面值，1 请求、366 token 增至 2 请求、987 token。小目录的成本门槛有直接测量依据。[启用短签名的能力试验](capabilities-chat-active/trials.json) · [Chat 精确约束探针](activation-chat/activation.json)

## 复跑

通过环境变量 `DEEPSEEK_API_KEY` 给脚本提供现有凭据。`ACP` 指向待测二进制；两组 Aster 应指向同一个文件。`DSH` 指向独立 DSH 程序。各 `--out` 指定尚未创建的目录。重复观察对照另加 `--repeated`，并把 `dsh` 从顺序中去掉。

```sh
python3 docs/evidence/code/2026-11-27-model-selected-tool-catalog/pilot.py \
  --chat-signatures --cost-gate --release "$ACP" --candidate "$ACP" \
  --dsh "$DSH" --embedding "$HOME/.local/bin/embedding" \
  --out "$OUT" --order release,candidate,dsh candidate,dsh,release dsh,release,candidate
```

能力探针使用根工作区的 `test_harness_capabilities_live_e2e` 测试二进制。测试程序由 `cargo test --locked -p aster-core --test test_harness_capabilities_live_e2e --no-run` 构建。`capabilities.py --binary <path> --out <path> --chat-signatures --cost-gate` 复用现有能力判定。`activation.py --out <path>` 运行原生精确约束探针。

## 完整目录复验

本轮两组 Aster 使用同一份本机 ACP，SHA-256 为 `005b23b8c34ddce458a7b701fe5151b2c1f3b5c520947db5864bb09089e0d688`。普通用例每次主请求有 26 个实际工具，精确 MCP 参数用例为 27 个。候选中继只替换厂商接收的工具投影，原来的执行、验证和权限由 ACP 处理。每个能力场景每组两次，冻结三方对照每组各三次。DeepSeek Flash、思考模式关闭、temperature 0、输出上限 1,024。用量按厂商回执累加，包括查阅定义和辅助请求。

| 同轮冻结任务，独立中位数 | Aster 完整定义 | Aster 候选 | DSH | 通过 |
| --- | ---: | ---: | ---: | ---: |
| 短签名保留原参数的短说明 | 31,468 | 22,764 | 25,141 | 9/9 |
| 加上原生必填字段的完整结构 | 31,410 | 26,134 | 25,150 | 9/9 |
| 加上原生必填字段的顶层类型和值 | 31,436 | 25,026 | 25,044 | 9/9 |
| **加上原生必填枚举或常量** | **31,402** | **23,530** | **25,180** | **9/9** |

最后一行比完整定义节省 25.07%，比独立 DSH 少 6.55%。各组主请求均为 4 次。输入缓存未命中的中位数分别为 1,335、1,115、937。候选保留必填枚举或常量的决策只依赖定义结构。短说明逐字保留符合长度条件的原字段说明。查阅仍可取出当前目录提供的完整原定义。[冻结判定](full-catalog/native-required-enum/frozen/trials.json) · [上游用量](full-catalog/native-required-enum/frozen/provider-usage.jsonl)

完整原生结构去掉注解及带注解短签名两种方案，使用原来的字节门槛时都没有触发压缩。其冻结对照用量保持约 31,000 token。去掉门槛后的各行结果进一步区分了表示质量和门槛本身的影响。[完整原生结构门槛](full-catalog/full-native-gated/frozen/trials.json) · [带说明短签名门槛](full-catalog/annotated-gated/frozen/trials.json)

分页用例中，裸短签名的两次运行最终都答对，但第一次实际发送 `offset=43`，比目标窗口晚一行，两次都多用一次主请求。加入原参数的 `0-based` 说明后，两次均从 `offset=42` 开始，全程恢复到四次主请求。仅在短签名中给出 Program 的 `type` 时，两个已授权用例都提交了缺少 `type` 的首个调用。将必填枚举放回原生 Schema 后，两次均第一次发出有效调用。[分页实际参数](full-catalog/read-range-audit.json) · [带说明但无原生枚举的 Program](full-catalog/annotated/program_approved/trials.json) · [保留原生枚举的 Program](full-catalog/native-required-enum/program_approved/trials.json)

| 完整目录独立场景 | 完整定义 | 必填枚举或常量 | 通过（完整 / 候选） | 主请求（完整 / 候选） |
| --- | ---: | ---: | ---: | --- |
| 精确分页和跨回合引用 | 30,969 | 23,114 | 2/2 · 2/2 | 4 · 4 |
| 工作区内容搜索 | 40,653.5 | 30,893.5 | 2/2 · 2/2 | 5 · 5 |
| Program 脚本及实际宿主批准 | 31,058 | 23,087 | 2/2 · 2/2 | 4 · 4 |
| 混合读取和搜索的原生 Batch | 31,220 | 23,377 | 2/2 · 2/2 | 4 · 4 |
| MCP 中只在完整参数说明出现的精确值 | 31,545.5 | 29,602.5 | 2/2 · 2/2 | 4 · 5 |

同质两文件 Batch 的早期用例里，两组各有两次返回正确值，但真实回执显示两组均使用多文件 `read`，严格的指定工具判分两组都是 0/2。首次 Program 用例的两组都由试验宿主拒绝脚本。后续单独用例只授权固定的纯计算源码，保留原试验记录。[完整目录结果](full-catalog/) · [混合 Batch](full-catalog/native-required-enum/batch_mixed/trials.json)

独立写入用例要求修改一个值、运行精确校验命令、报告该命令输出，并在第二回合重新读取。首轮夹具拒绝了候选发出的 `expected_replacements: 1`。第二批仅补齐同一项单次替换的合法参数形态，两组都通过文件字节及绑定核验摘要检查，实际 `edit` 和校验命令均成功。完整任务原样判分为完整定义 1/2、候选 0/2。两次候选和一次完整定义在已有验证结果之后又尝试了夹具未授权的额外命令，第一回合交付缺少用户要求的校验输出。保留此差异作为生产启用前的回归边界。[原夹具](full-catalog/verified-edit-enum/verified_edit/trials.json) · [允许精确替换次数的夹具](full-catalog/verified-edit-exact-count-enum/verified_edit_exact_count/trials.json)

### 账本证据

真实 MCP 工具的两次候选请求都先查完整定义，再提交精确参数并拿到工具回执。裸短签名的查阅在两次中分别花费 4,224 和 4,236 token，保留原生必填枚举的版本每次花费 5,425 token。厂商主请求和 ACP 回合总量的差值在六次候选试验中逐条等于查阅用量。相同场景完整定义组的厂商主请求总量与 ACP 回合总量相等。由 Core 接入时，每一次查阅后的新请求需要走原模型调用的预算、取消和用量生命周期。Chat Completions 需要客户端提供查阅通路。DeepSeek 的 Anthropic 和 Responses 协议另有原生延迟声明。[MCP 计账对账](full-catalog/mcp-accounting-audit.json) · [DeepSeek 工具调用文档](https://api-docs.deepseek.com/guides/tool_calls/#deferred-tool-loading)

### 复跑完整目录

对冻结命令追加 `--annotate-signatures --native-required enum`，省去 `--cost-gate`，即可复验保留必填枚举的当前试验。完整目录的独立对照可按下列方式运行。`--scenario` 支持 `read_window`、`search`、`program_approved`、`batch_mixed`、`mcp_schema` 与 `verified_edit_exact_count`。

```sh
python3 docs/evidence/code/2026-11-27-model-selected-tool-catalog/full_catalog.py \
  --scenario mcp_schema --acp "$ACP" --embedding "$HOME/.local/bin/embedding" \
  --annotate-signatures --native-required enum --ungated --out "$OUT"
```

新增 Python 脚本的语法检查及投影断言通过。断言覆盖必填枚举、参数名 `description`、实例数据中的同名键、完整必填结构和布尔 Schema 降级。所有真实调用结果和用量按 JSON 格式保留。

## Explicitly not claimed

这些是本机中继的实验数据。生产 Core、ACP 和 TUI 路由、真实流式延迟与中断、长会话累计计账、全目录能力覆盖及其他模型的表现需要单独验证。冻结对照给出了一种可以低于 DSH 的实际运行方式，能力保持仍需更全面的证据才能默认启用。
