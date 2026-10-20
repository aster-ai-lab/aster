# Issues #2 / #3 / #4：ACP、headless 与 program 修复

本报告记录这三项的源码修复与维护回归。GitHub 完整正文及空评论列表在 [ISSUES.json](ISSUES.json)。没有改动 CLI 参数解析、内置模板内容、工作区权限或通用 PTC 的执行策略；没有修改本轮之外的 dirty 文件。

## 根因与最终行为

- [#2](https://github.com/aster-ai-lab/aster/issues/2)：headless 对空 `auth_methods` 无条件选择认证方法，提前返回 `Not signed in`；TUI 则允许非 Grok agent 使用本地 ACL 凭据。headless 现在复用同一空列表策略：非 Grok 跳过 ACP 登录；Grok 仍拒绝。列表非空时保留既有方法选择及真实认证 RPC，禁止启动需要浏览器的登录。
- [#3](https://github.com/aster-ai-lab/aster/issues/3)：pager 已发送 initialize `_meta.rules` / `systemPromptOverride`，ACP 没有消费。现在校验这两个字段必须是字符串，将非空 rules 保存在 ACP 连接拥有处，通过共同 `session_options_for` 附加至新建、恢复和替换会话的提示。追加保留原文、宿主上下文、运行合同及权限策略。当前 Core 只允许边缘提示定制，因此非空 `systemPromptOverride` 在握手返回明确的 `invalid_params`，并指向 `--rules` / `--append-system-prompt`；没有冒充完整 replace。空白字符串按未设置处理。
- [#4](https://github.com/aster-ai-lab/aster/issues/4)：两个 catalog 构造函数都丢弃目录；工具只有 script 路径。此外，code presentation 会再次覆盖 program 描述。现在保留目录，工具描述和 schema 公布准确模板名、必需参数及默认值；通过 `type="template"`、`name`、`inputs` 实例化，拒绝未知模板、缺少参数、非对象及额外输入。实例化数据作为 JSON 进入固定 runner，复用既有 QuickJS、`ToolInvoker` 与 `ctx.nested_tool_invocation`；不调用绕过会话治理的低层 `ProgramExecutor`。只读能力按实际步骤参数计算，显式 allowlist 也不能放行不可见工具或已过滤的 orchestrator。code presentation 保留原描述，再附加原有执行边界及 Tool signatures。

11 个修改文件及 SHA256 见 [source-ready 收据](issue-2-3-4-source-ready.json)。其中 `agent.rs` 另有 #5 代理要求的测试模块声明；`program_tool/worker_deadline_tests.rs` 仅把私有 struct literal 改为同一构造器，保留原工作线程拥有关系。未增加依赖或改变公开 Rust 类型/构造函数签名。

## 修复前证据

| 检查 | 结果 | 日志 |
| --- | --- | --- |
| 非 Grok 空认证列表 | 1 条预期行为回归失败：Not signed in | [headless before](issue-2-before.log) |
| initialize 非法提示字段 | 1 条失败：非法字段被默许 | [validation before](issue-3-before-validation.log) |
| 真实 loopback Anthropic 请求 | 1 条失败：system 中规则 marker 为 0 | [wire before](issue-3-before-wire-direct.log) |
| catalog 发现及真实模板执行 | 63 通过、2 条新增回归失败 | [catalog before](issue-4-before-regression.log) |
| 实际 code profile 投影 | 1 条失败：描述缺少 program_code_search | [projection before](issue-4-before-code-projection.log) |

wire before 使用前一条 Cargo 刚构建的维护测试二进制 `target/debug/deps/a3s_code_acp-180b7ad9f0e54c0c` 直接运行同名测试；仅终止了本代理尚在排队的重复 Cargo 命令。原队列日志 [保留](issue-3-before-wire.log)。四个主要生产文件的修复前身份见 [before source](issue-2-3-4-before-source.json)。

首轮 catalog 编译受另一个在途 #5 新测试的 `MemoryItem: PartialEq` 断言错误阻挡，原 [日志保留](issue-4-before.log)，不作行为失败证据。首轮 program after 的 87 通过 / 1 失败是新增 quote 回归误用了另一个 EchoTool fixture 的 `echo:` 前缀；已按该 fixture 返回原文的合同更正测试期望，未为此改生产。[首轮日志](issue-4-after.log) 与 [exact 更正后 1/1](issue-4-after-echo-correction.log) 均保留。

## 最终定向验证

所有命令都从仓库根运行，使用 pinned toolchain 和 `--locked`；每条进程组有 900 秒上限，均正常结束。完整命令、退出码及耗时见 [机器收据](issue-2-3-4-targeted-results.json)。每行是独立过滤运行；不叠加 before、exact 复跑或工作线程子测试的重复输出。

| Cargo 过滤范围 | 最终结果 | 日志 |
| --- | --- | --- |
| `-p a3s-code-core --lib program_` | 88 通过，0 失败 | [program](issue-4-after-final.log) |
| `-p a3s-code-core --lib tools::presentation::tests` | 5 通过，0 失败 | [presentation](issue-4-after-presentation.log) |
| `-p a3s-code-acp --lib agent::session_setup_tests` | 10 通过，0 失败 | [setup](issue-3-after-setup.log) |
| `-p a3s-code-acp --lib agent::reasoning_transport_tests` | 4 通过，0 失败 | [wire / replacement / resume](issue-3-after-wire.log) |
| `-p a3s-code-pager --lib headless::tests` | 35 通过，0 失败 | [headless](issue-2-after.log) |

维护回归覆盖：模板 schema 的实际 JSON Schema 校验和 code profile；两个内置模板在临时工作区的搜索/映射；自定义目录、含引号/中文数据、scoped nested 拒绝传播、只读与写入步骤、不可见/递归工具拒绝。既有真实 session 的 script 权限、预算、取消及 receipt 回归同在 program 过滤运行中通过。新增 template denial 单测使用受控 scoped invoker，不将它称作完整 HITL 端到端测试。

真实 Anthropic provider fixture 接收五次请求：新会话、三次 effort/session 替换、关闭后新 agent 初始化并恢复会话；每次 rules marker 恰好一次，且原运行合同与历史保留。普通及 unattended options 的规则追加和 source-review 权限仍安装也通过。

只对自有文件执行了分 edition 的 `rustfmt --check` 与 `git diff --check`，均通过；最终源码身份复核见 [static checks](issue-2-3-4-static-checks.json)。整个工作区检查及最终配对二进制 CLI 四场景由主代理另外报告。

## 明确不声明

- 没有实现完整 system prompt replacement；它现已明确拒绝，不再静默失效。
- #2 附带的 180 秒挂起未复现。主代理保存的 [真实配对 before](headless-before-result.json) 中，ACL-only 与 ACL + 合成 A3S_API_KEY 都在 0.2 秒内返回 Not signed in，均为 0 次 provider 调用。不能将该未定位挂起称为已找到根因。
- 没有使用真实模型密钥或外部模型来判定输出质量，也没有把确定性 provider fixture 称为真实 LLM 测试。
- 本报告不声明全部工作区测试、GitHub 关闭、提交推送或新版本发布完成。
