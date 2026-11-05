# Frozen Agent inventory

**Source**: [上游 reference Agents 表](https://github.com/yetone/magpie/blob/9162277b91c9327773b5a1e0413d59f24de6f3af/docs/reference.md#agents)。
**Inventory revision**: 2026-11-01；70 条。AG01–AG64 保持原源表名称/顺序与既有稳定 ID；AG65–AG70 补齐 README 与 frozen 源码中存在、主表未列出的六项。这里只保留索引，不复制上游完整说明。当前版本增量另由 T143 审查，不以旧清单作为完整替代的范围上限。

“已有 adapter”仅指 cliConfig/adapters.ts 已有条目，不表示全部字段、平台或真实运行通过。其余为待逐项核实，不能据此断言完全没有基础代码。所有条目由 T105 owning；T104 负责格式保留、T106 版本管理、T053 原生验收，WSL 另由 T127 验证。

| ID | 上游名称（原样） | 本轮代码盘点 | Owning task |
| --- | --- | --- | --- |
| AG01 | Claude Code | 已有 adapter，待验收 | T105 |
| AG02 | Claude Desktop | 待逐项核实 | T105 |
| AG03 | Codex | 已有 adapter，待验收 | T105 |
| AG04 | Gemini CLI | 已有 adapter，待验收 | T105 |
| AG05 | OpenCode | 已有 adapter，待验收 | T105 |
| AG06 | OpenChamber | 待逐项核实 | T105 |
| AG07 | MiMo Code | 待逐项核实 | T105 |
| AG08 | DevEco Code | 待逐项核实 | T105 |
| AG09 | Pi | 已有 adapter，待验收 | T105 |
| AG10 | StepCode | 待逐项核实 | T105 |
| AG11 | MothX | 待逐项核实 | T105 |
| AG12 | PenguinHarness | 待逐项核实 | T105 |
| AG13 | Aside | 待逐项核实 | T105 |
| AG14 | OmO (omo-ai) | 待逐项核实 | T105 |
| AG15 | Prime Agent | 待逐项核实 | T105 |
| AG16 | Goose | 待逐项核实 | T105 |
| AG17 | Cursor CLI | 待逐项核实 | T105 |
| AG18 | Zed | 待逐项核实 | T105 |
| AG19 | VS Code (Chat) | 待逐项核实 | T105 |
| AG20 | VS Code Insiders (Chat) | 待逐项核实 | T105 |
| AG21 | VSCodium (Chat) | 待逐项核实 | T105 |
| AG22 | Zoo Code (VS Code) | 待逐项核实 | T105 |
| AG23 | Copilot (JetBrains) | 待逐项核实 | T105 |
| AG24 | JetBrains Air | 待逐项核实 | T105 |
| AG25 | Copilot CLI | 待逐项核实 | T105 |
| AG26 | Crush | 待逐项核实 | T105 |
| AG27 | DeepSeek Harness (dsh) | 已核对 profile/.env/credentials 契约；现有 start IPC 缺原生映射，native adapter/完整验收待实现 | T105 |
| AG28 | Reasonix Studio (2.x, or a native Go `reasonix` 2.x / 1.39.x CLI) | 待逐项核实 | T105 |
| AG29 | Command Code | 待逐项核实 | T105 |
| AG30 | fx | 待逐项核实 | T105 |
| AG31 | omp (oh-my-pi) | 待逐项核实 | T105 |
| AG32 | Devin | 待逐项核实 | T105 |
| AG33 | Hermes Agent | 原生入口、命名连接与活动 profile 的读写/启动/独立恢复已验；provider catalog、多机器人同步及完整 Agent/平台/性能仍开放 | T105 |
| AG34 | Mister Morph | 待逐项核实 | T105 |
| AG35 | Kimi Code | 已有 adapter，待验收 | T105 |
| AG36 | Qwen Code | 已有 adapter，待验收 | T105 |
| AG37 | Muse Code | 待逐项核实 | T105 |
| AG38 | Empryo | 待逐项核实 | T105 |
| AG39 | Ante | 待逐项核实 | T105 |
| AG40 | AstrBot | 待逐项核实 | T105 |
| AG41 | QwenPaw | 已核对 frozen 源码；Code 缺专用 adapter，未实现/验收 | T105 |
| AG42 | OpenClaw | 已核对配置与 env/profile 路径契约；现有 renderer IPC 缺原生映射，native adapter/完整验收待实现 | T105 |
| AG43 | jcode | 待逐项核实 | T105 |
| AG44 | MiniMax Code (mcode) | 已有 adapter；YAML 保留切片已验，官方加载器/完整 Agent/性能待验收 | T105 |
| AG45 | Droid (Factory) | 待逐项核实 | T105 |
| AG46 | Cline (CLI) | 待逐项核实 | T105 |
| AG47 | Qoder (CLI) | 待逐项核实 | T105 |
| AG48 | Qoder CN (CLI) | 待逐项核实 | T105 |
| AG49 | Grok Build | 待逐项核实 | T105 |
| AG50 | Grok App | 待逐项核实 | T105 |
| AG51 | ZCode | 已核对新版 reasoning schema；Code 缺专用 adapter，未实现/验收 | T105 |
| AG52 | WorkBuddy | 待逐项核实 | T105 |
| AG53 | WorkBuddy AI | 待逐项核实 | T105 |
| AG54 | CodeBuddy (CodeBuddy Code CLI, CodeBuddy IDE) | 待逐项核实 | T105 |
| AG55 | Raycast | 待逐项核实 | T105 |
| AG56 | Kiro Crew | 待逐项核实 | T105 |
| AG57 | OpenPencil | 待逐项核实 | T105 |
| AG58 | NewMax | 待逐项核实 | T105 |
| AG59 | T3 Code | 待逐项核实 | T105 |
| AG60 | OpenHanako | 待逐项核实 | T105 |
| AG61 | AtomCode | 待逐项核实 | T105 |
| AG62 | Snow CLI | 待逐项核实 | T105 |
| AG63 | Alma | 待逐项核实 | T105 |
| AG64 | Lody | 待逐项核实 | T105 |
| AG65 | Claude Science | daemon/启动环境接管待实现与验收 | T105 |
| AG66 | Antigravity CLI | 官方 1.4.12 本机文字/工具、账本、额度及直接模型别名已验；完整行为、桌面与其他平台待验收 | T105 |
| AG67 | Cursor Private Inference | 原生启动环境/安装实例接管待实现与验收 | T105 |
| AG68 | Pencil | 独立 models.json adapter 待实现与验收 | T105 |
| AG69 | Cindy | 应用内确认的 provider import 链路待实现与验收 | T105 |
| AG70 | AionUi | 应用内 provider import/状态检测待实现与验收 | T105 |

## 非主表 Agent 的独立来源与入口

原 64 行只覆盖 reference 的主表，未覆盖同一 frozen README 的完整显式 Agent 名单。0.1.2141 README 列出 69 项；规划文档 revision 列出 70 项，增加 QwenPaw。以下六项在两个 README 中均存在，不能因主表未列出而丢弃。

所有源码链接相对上游 tag commit `eadeb89c48f2420818462227d7687cd7a989a1e5` 的 `internal/agent/`：

| ID | 来源 | 必须单独验证的行为 | Studio owner / test 目标 |
| --- | --- | --- | --- |
| AG65 | `claudescience.go`：claudeScience/scienceEnv/scienceRestart | 检测真实 daemon、保留已有端口与启动方式、接通 Anthropic 环境、重启/恢复；不是通用 TOML 写入 | 现有 CodeCli 管理/原生启动 owner；T105/T106 stub daemon 与 native fixture |
| AG66 | `agy.go`：agyIn/AgyStart/AgyLaunch | `~/.gemini/antigravity-cli/settings.json`、modelProvider/custom model、启动时 endpoint/key/model 环境，以及 WSL 独立实例；不写开发者 shell 配置作默认测试 | 已有 Antigravity CLI 入口/launch adapter；T105/T127 文件及 argv/env fixture |
| AG67 | `cursorlocal.go` 与 `cursorlocal_env_*` | 用真实 product.json 标识安装实例，独立启动环境、模型目录/effort、撤销环境；不能按 Cursor 文件名猜安装或把 Cursor CLI 当本项 | CodeCli 安装实例/原生启动 owner；T105/T106 各 OS 环境 fixture |
| AG68 | `pencil.go`：pencil/pencilProviderJSON | `~/.pencil/models.json` 的 provider/catalog，保留用户条目、运行中写入约束与模型列表；Pencil 与 OpenPencil 分开 | 现有 cliConfig adapters/草稿预览；T104/T105 真实 JSON 保留/撤销 fixture |
| AG69 | `cindy.go`：CindyLink/cindyHas | `cindy://provider/import` 的受控 provider payload、用户在目标应用确认、检测是否成功导入；不伪造文件写入已成功 | 现有 CodeCli preview/native open owner；T105/T053 URI payload/确认/失败 fixture |
| AG70 | `aionui.go`：AionUiLink/aionuiHas | 目标应用 provider import、检测现有 provider 与状态；不通过任意 URL 代替已生效配置 | 同上既有 CodeCli/native owner；T105/T053 导入/拒绝/撤销 fixture |

## 每行实施记录必须补齐

0.1.2523 的 source commit `fb3942ee67a719a4e4733cdb632873defd42f28b` 已固定；其 README 仍列 70 项。相对前一文档快照，AG41 QwenPaw 新增 `thinking_level` 与 provider 的 `thinking_param_style: "effort"`，T105 的该行 fixture/模型能力写入与撤销必须覆盖。

2026-11-02 源码复核：[完整差异证据](../../../docs/evidence/local-gateway/2026-11-02-reference-source-audit/OUTCOME.md) 的 D01/D03 记录 AG41/AG51 的具体行为、fixture/native 要求及现有 UI owner。AG41 的工作目录/secret 目录、动态 agent workspace、后端类型、共享 provider 文件及重启条件必须一起适配；`inherit` 清空、删除旧 `thinking_budget`、支持级别交集与 `config_overrides` 不能只靠写一个 effort 字段完成。接管前的 `active_model`（包括 null）及用户后续选择必须保留，其他 fallback/subagent 引用尚在时不能移除共享 provider。

AG51 的实际 `reasoning` 字段是 `levels`、`defaultLevel`、`providerOptionsByLevel`；旧 `defaultVariant` 迁到实际字段，用户每级选项优先保留，enabled thinking 带实际 budget，disabled 级发送 disabled。两项均接原 Code 配置侧栏、模型/effort picker、草稿预览与 native 文件 owner，不新增页面、编辑器或配置仓库。当前闭集 `CodeCli`/`CLI_CONFIG_ADAPTERS` 没有这两个专用适配；源码核对不是原生可用证明，T105/T053 保持未完成。

2026-11-02，AG66 已接入原 Code 草稿/原始编辑/恢复 owner：独立的 `~/.gemini/antigravity-cli/settings.json` 写入 `modelProvider`、选择及 Gemini custom model 目录，凭据仅经启动 IPC 传给子进程环境，`--model` 使用实际 custom model key。已有用户模型和未知字段保留；网关模型/路由经现有能力投影筛选，自己的登录不注入运行时凭据。配置与原生 fixture 切片见 [Antigravity evidence](../../../docs/evidence/local-gateway/2026-11-02-antigravity-cli/OUTCOME.md)。这不等于官方 CLI、外部 shell profile、WSL 独立实例、移除模型后的目录清理或 Agent 专用身份/计量已验收，T105/T053/T127 继续开放。

2026-11-02，AG66 增加固定官方 Antigravity CLI 1.4.12 的 macOS ARM64 原生验证：生产 adapter 草稿经原生 writer 落盘、ToolStore/既有 launch script 启动、真实 Gateway→ModelControl→账本。8,192 输出上限明确拒绝且零上游/零计量；65,536 上限下文字请求记 11 tokens，实际文件读取工具往返记两条记录共 22 tokens，工具结果和签名保留。11 项 Gemini 集成测试（含官方 CLI 与既有 Google SDK）通过，Clippy/根格式检查通过。见 [official CLI evidence](../../../docs/evidence/local-gateway/2026-11-02-antigravity-official/OUTCOME.md)。这不覆盖辅助轻量模型、输出额度感知的模型选择 UX、外部认证、其他平台/WSL、shell profile、目录清理或 Agent 专用身份；T105/T127 和完整替代继续开放。

2026-11-02，AG66/T105/T127 的额度校验复用现有模型选择、配置错误区与启动流程：以官方 1.4.12 的 65,536 输出要求过滤不足或未知的模型；原生目录按协议投影额度，路由只聚合相同协议的候选，IPC 保留路由和额度。保存/启动拒绝路径保留选择并阻止副作用，折叠高级项时也能看到原因，没有新增选择器或页面。879 项前端/共享测试、22 项原生测试及构建、局部 lint/i18n/格式检查通过。详见 [output limit evidence](../../../docs/evidence/local-gateway/2026-11-02-antigravity-output-limits/OUTCOME.md)。真实桌面、其他版本/平台、辅助模型及完整能力仍未验收；直接模型别名的草稿/启动一致性有明确后续项，完整替代继续开放。

2026-11-02，直接模型别名的后续缺口已在共享 resolver、配置归属识别、原始编辑与启动处修复：配置和启动使用同一 API ID，界面与偏好保留内部模型 ID。十种文件配置型 CLI 通过同一组别名回归；官方 Antigravity CLI 1.4.12 经实际 native writer、ToolStore 和启动脚本向合成本机上游请求正确模型。893 项前端/共享测试、53 项原生 Code CLI 测试通过，没有新增查询、页面或选择器。读取模型失败会阻止写入；旧文件需通过现有配置应用流程重新生成。真实桌面、外部认证、其他平台和完整替代仍开放。见 [direct alias evidence](../../../docs/evidence/local-gateway/2026-11-02-cli-model-alias/OUTCOME.md)。

- 参考二进制/文档 revision、Agent 版本、支持 OS/架构/安装方式，以及是否需要第三方登录/许可证。
- 原文的 exact config paths、环境目录覆盖、Windows package/用户 profile/WSL 实例和可写字段；不得从名字猜路径。
- model/provider/effort/small/角色模型和特殊 flags 的读取、预览、最小写入、运行时生效条件。
- 保留注释/顺序/缩进/用户字段的 fixture，接管撤销、多文件失败恢复和模型可见性。
- 安装检测、升级来源、session 读取与续开行为；不适用项引用源条件，缺环境记未测。
- test 路径、实际执行计数、native/live 结果与 evidence 链接。

G3 要求每行完成全部适用子项。一个通用 base URL 或一个成功配置写入不能覆盖整行。后续上游增加 Agent 时新增稳定 ID，不重排既有 ID。
