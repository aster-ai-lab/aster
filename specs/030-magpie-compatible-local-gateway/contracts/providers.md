# Frozen provider inventory

**Source**: Magpie v0.1.2523, source commit `fb3942ee67a719a4e4733cdb632873defd42f28b`, `internal/provider/presets.go`.

源文件 SHA-256：`43ecb50c602c17ba328908f57178d9185d4423a79a0cb7739c29d1d65ec5733b`。60 个顶层静态预设，以下顺序为源声明顺序；不是测试通过数。33 个直接声明的 region/plan ID 列在各行；另有 33 个由函数生成的 region 条目，下表保持调用参数顺序。不能把破折号理解为没有区域。

共同 UI/数据 owner：Studio 现有 ProviderSettingsPage、ConnectionSettings/ProviderApiKeyListDrawer、ModelSyncPreviewPanel、useProvider 与 native ProviderStore/providers.rs。每行沿用这些入口；特殊认证扩现有 adapter，不建立第二套 provider/key 管理。

Owning tasks：T016（协议/能力/特殊认证）、T026/T030/T038（keys/账户/headers/proxy）、T040/T046（目录/价格）、T130（导入）；remote-magpie 还需 T120，decision 来源还需 T136。每行 fixture 目标为现有 provider-registry/ProviderSettings 与 native providers 测试扩展，以及 Gateway loopback HTTP/SSE 实测；真实账户与适用平台证据由 T037/T071/T134 逐行保留。

端点字段列仅忠实列出源 literal（含区域里的字段），不是对运行时所有协议的推断。Vertex/Azure/remote 的特殊 transport 与认证需要读对应源并实测。全表当前均为未验收。

| ID | Exact preset ID | Source name | Kind | 源端点字段 | 直接声明 region/plan ID | Source line |
| --- | --- | --- | --- | --- | --- | --- |
| PV01 | `anthropic` | Anthropic | KindVendor | Anthropic | — | 163 |
| PV02 | `openai` | OpenAI | KindVendor | Chat, Responses | — | 168 |
| PV03 | `google` | Google Gemini | KindVendor | Chat | — | 171 |
| PV04 | `google-vertex` | Google Vertex AI | KindVendor | 特殊 transport | — | 187 |
| PV05 | `deepseek` | DeepSeek | KindVendor | Chat, Responses, Anthropic | — | 197 |
| PV06 | `xai` | xAI | KindVendor | Chat, Responses, Anthropic | — | 200 |
| PV07 | `moonshot` | Kimi | KindVendor | Chat, Anthropic | — | 205 |
| PV08 | `moonshot-cn` | Kimi (China) | KindVendor | Chat, Anthropic | — | 208 |
| PV09 | `kimi-code` | Kimi Code | KindVendor | Chat, Anthropic | — | 213 |
| PV10 | `kimi-code-cn` | Kimi Code (China) | KindVendor | Chat, Anthropic | — | 217 |
| PV11 | `zhipu` | Zhipu GLM | KindVendor | Chat, Responses, Anthropic | `api`, `coding` | 221 |
| PV12 | `zai` | Z.ai | KindVendor | Chat, Responses, Anthropic | `api`, `coding` | 232 |
| PV13 | `minimax` | MiniMax | KindVendor | Chat, Anthropic | — | 239 |
| PV14 | `minimax-cn` | MiniMax (China) | KindVendor | Chat, Anthropic | — | 242 |
| PV15 | `stepfun` | StepFun | KindVendor | Chat, Anthropic | `plan`, `api` | 247 |
| PV16 | `stepfun-cn` | StepFun (China) | KindVendor | Chat, Anthropic | `plan`, `api` | 254 |
| PV17 | `xiaomi` | Xiaomi MiMo | KindVendor | Chat, Responses, Anthropic | `plan-cn`, `plan-sgp`, `plan-ams`, `api` | 269 |
| PV18 | `baidu-qianfan` | Baidu Qianfan | KindVendor | Chat, Responses, Anthropic | `personal`, `team`, `api` | 286 |
| PV19 | `tencent-cloud` | Tencent Cloud | KindVendor | Chat, Responses, Anthropic | `plan`, `cn`, `intl`, `enterprise`, `enterprise-intl` | 322 |
| PV20 | `huaweicloud` | Huawei Cloud MaaS | KindVendor | Chat, Anthropic | `plan`, `api` | 350 |
| PV21 | `volcengine` | Volcengine Ark | KindVendor | Chat, Responses, Anthropic | `coding`, `agent`, `api` | 368 |
| PV22 | `qwen` | Qwen | KindVendor | Chat, Anthropic | — | 384 |
| PV23 | `qwen-cn` | Qwen (China) | KindVendor | Chat, Anthropic | — | 388 |
| PV24 | `qwen-token-plan` | Qwen Token Plan | KindVendor | Chat, Anthropic | — | 400 |
| PV25 | `bailian-token-plan` | Bailian Token Plan | KindVendor | Chat, Anthropic, Decide | — | 409 |
| PV26 | `qwencloud-token-plan` | Qwen Cloud Token Plan | KindVendor | Chat, Anthropic | — | 423 |
| PV27 | `qwencloud` | Qwen Cloud | KindVendor | Chat, Anthropic | — | 430 |
| PV28 | `mistral` | Mistral | KindVendor | Chat | — | 435 |
| PV29 | `groq` | Groq | KindVendor | Chat, Responses | — | 438 |
| PV30 | `bedrock` | Amazon Bedrock | KindVendor | Chat, Responses, Anthropic | — | 451 |
| PV31 | `bedrock-mantle` | Amazon Bedrock (Mantle) | KindVendor | Chat, Responses, Anthropic | — | 474 |
| PV32 | `azure` | Azure OpenAI | KindVendor | 特殊 transport | — | 486 |
| PV33 | `ollama-cloud` | Ollama Cloud | KindVendor | Chat, Anthropic, Decide | — | 494 |
| PV34 | `nous` | Nous Portal | KindVendor | Chat | — | 506 |
| PV35 | `openrouter` | OpenRouter | KindRelay | Chat, Anthropic, Decide | — | 511 |
| PV36 | `opencode-go` | OpenCode Go | KindRelay | Chat, Responses, Anthropic | — | 520 |
| PV37 | `clinepass` | ClinePass | KindRelay | Chat | — | 528 |
| PV38 | `opencode-zen` | OpenCode Zen | KindRelay | Chat, Responses, Anthropic, Decide | — | 536 |
| PV39 | `kilo` | Kilo Gateway | KindRelay | Chat | — | 550 |
| PV40 | `commandcode` | Command Code | KindRelay | Chat, Responses, Anthropic | — | 557 |
| PV41 | `together` | Together AI | KindRelay | Chat | — | 560 |
| PV42 | `fireworks` | Fireworks | KindRelay | Chat | — | 563 |
| PV43 | `siliconflow` | SiliconFlow | KindRelay | Chat | `cn`, `intl` | 566 |
| PV44 | `nvidia` | NVIDIA NIM | KindRelay | Chat | — | 577 |
| PV45 | `modelscope` | ModelScope | KindRelay | Chat, Responses | — | 582 |
| PV46 | `aihubmix` | AiHubMix | KindRelay | Chat, Anthropic | — | 586 |
| PV47 | `pipellm` | PipeLLM | KindRelay | Chat, Responses, Anthropic | — | 593 |
| PV48 | `302ai` | 302.AI | KindRelay | Chat, Anthropic | — | 596 |
| PV49 | `cherryin` | CherryIN | KindRelay | Chat, Responses, Anthropic | — | 599 |
| PV50 | `yylx` | 鱼鱼连线 | KindRelay | Chat, Anthropic | `auto`, `global`, `cn` | 602 |
| PV51 | `typesafe` | TypeSafe Jev | KindVendor | Decide | — | 613 |
| PV52 | `vercel-jev` | Jev · Vercel AI Gateway | KindRelay | Decide | — | 618 |
| PV53 | `cloudflare-jev` | Cloudflare Workers AI · Jev, Clef | KindRelay | Decide | — | 624 |
| PV54 | `bailian-decision` | Bailian Decision Model | KindVendor | Decide | `cn-beijing`, `ap-southeast-1`, `token-plan` | 631 |
| PV55 | `qwen-decision` | Qwen Decision Model | KindVendor | Decide | — | 644 |
| PV56 | `ollama` | Ollama | KindLocal | Chat, Anthropic, Decide | — | 651 |
| PV57 | `lmstudio` | LM Studio | KindLocal | Chat | — | 655 |
| PV58 | `omlx` | oMLX | KindLocal | Chat, Responses, Anthropic, Decide | — | 660 |
| PV59 | `mlx-serve` | MLX-Serve | KindLocal | Chat, Responses, Anthropic, Decide | — | 666 |
| PV60 | `remote-magpie` | Remote magpie | KindRemote | 特殊 transport | — | 674 |

## 生成式区域（源参数顺序）

| Preset | 函数 / 来源行 | Exact region IDs | 声明端点 |
| --- | --- | --- | --- |
| `qwencloud` | `qwenCloudRegions` / 434 | `ap-southeast-1`, `us-east-1`, `eu-central-1`, `ap-northeast-1`, `cn-hongkong`, `cn-beijing` | Chat, Anthropic |
| `bedrock` | `bedrockRegions` / 455–456 | `us-east-1`, `us-east-2`, `us-west-2`, `eu-central-1`, `eu-west-1`, `eu-west-3`, `ap-northeast-1`, `ap-southeast-1`, `ap-southeast-2`, `ap-south-1` | Chat, Responses, Anthropic |
| `bedrock-mantle` | `mantleRegions` / 478–480 | `us-east-1`, `us-east-2`, `us-west-2`, `eu-central-1`, `eu-central-2`, `eu-north-1`, `eu-south-1`, `eu-west-1`, `eu-west-2`, `ap-northeast-1`, `ap-northeast-2`, `ap-south-1`, `ap-southeast-1`, `ap-southeast-2`, `ap-southeast-3`, `ap-southeast-4`, `sa-east-1` | Chat, Responses, Anthropic |

这些是源中显式 ID 的机械展开，不是可用性或 native/live 通过记录。生成函数在同文件 693/711/732 行；具体 URL 由这些函数与 Bedrock transport owner 提供。

## 每行仍须展开

- FromPreset/region 的具体 URL/path、可选 headers、认证方式、模型输入类型/effort、目录刷新和厂商扩展字段。未知能力必须明确拒绝，不能从模型或 provider 名称推断。
- 以上 33 个生成式区域与每个 plan 的 native transport 都需实际 fixture；不能只测默认区域。
- Vertex 的 project/location/gcloud token 与原生/partner/Claude transport；Azure 的 resource/deployment/API version；Bedrock 的 bearer/SigV4 与 region；local 服务的真实能力发现。
- 旧 presetAliases：`qianfan-token-plan`→`baidu-qianfan`，`tencent-token-plan`→`tencent-cloud/plan`，`tencent-tokenhub-cn`→`tencent-cloud/cn`，`tencent-tokenhub`→`tencent-cloud/intl`。这些是精确源映射，不按名称猜归属。
- 自定义 provider、插件动态 providers 与用户显式第三方订阅另按 CAP03/CAP06/CAP17；商业追踪/广告不进入 Aster，但可调用的 provider 能力不被删除。
- 每行保存 test ID、实际执行数量、OS/架构、fixture 与 native/live 结果和 evidence。T007/T134 未逐项核对完前，不将 60 个预设计为已覆盖。
