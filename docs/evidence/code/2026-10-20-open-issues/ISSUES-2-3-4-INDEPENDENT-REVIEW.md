# Issues 2、3、4 独立调用链审查

本审查只读产品源码和已有回归；没有重复 Cargo、运行应用、访问真实凭据或修改生产文件。已读源文件身份见 [收据](issue-2-3-4-independent-review.json)。

## 结论与后续

当前主体实现未发现新的权限绕过或 rules 丢失路径。另确认了一项实际呈现缺口：`tools/presentation.rs` 的 `code_projection` 原来完全覆盖 program description，因此 code profile 下目录解释文字消失、仅参数 schema 保留模板名。已独立核对最终修复：原 description 作为固定 code-profile 文案前缀保留，现有治理/signatures、schema 和工具集合均未改变。新增真实 code-profile 回归在修复前准确失败，日志为 [issue-4-before-code-projection.log](issue-4-before-code-projection.log)。最终 11 文件静态审查无未决源码发现；运行测试以实现代理及根代理后续收据为准。

## 已核对的真实路径

- **#2**：headless 读取实际 initialize response 的 `grokShell`，空 auth_methods 复用 TUI 的 `empty_auth_methods_needs_login` 策略。仅非 Grok 空列表跳过 ACP 登录；Grok 空列表仍拒绝，非空方法仍走既有选择、interactive 拒绝和 authenticate 请求。返回值只保持与 TUI 一样的本地配置语义，没有提供或提升 provider 凭据。
- **#3**：接收端 initialize 对 rules 和 systemPromptOverride 的类型严格校验，非空全文替换明确返回 invalid_params。rules 保存在 agent 独立字段，`session_options_for` 在原 host/unattended options 上追加；new/load 共用 `open_core_session`，模型及 effort 替换共用 `replace_core_session`，两者都重建同一份 options。没有从旧 prompt 再累加，因此恢复/替换不会重复追加。source_review 的硬 permission policy 在追加后仍安装，workspace admission 路径不变。
- **#4**：生产注册器仍用同一 `ProgramCatalog::with_builtin_programs`，description/schema 与按名执行读取同一 catalog。名字及参数名准确匹配，未知项拒绝；用户输入作为 Program JSON 数据进入固定 TEMPLATE_RUNNER，没有拼接到 JavaScript source。执行复用 `execute_script_program`，context scoped invoker 优先于低层 fallback，子调用使用 Nested origin。真实 ScopedToolInvoker 继续执行 schema、verifier read-only、permission/confirmation、cancellation 和审计。`ProgramExecutor` 的裸 registry 路径没有被采用。
- 模板 preflight 同时检查 available 和 allowed 工具，保留 program/dynamic_workflow/parallel_task 过滤；read_only 根据实际实例化步骤及其参数推导，空/未知/不可实例化模板不能声明只读。没有使用字符串猜测模板或工具语义。

## 测试范围核对

- #2 新测试精确验证空列表两种 agent、非交互 api-key/cached-token 请求，以及 browser-login 拒绝。真实成对 CLI/provider stub E2E 由根代理负责；本审查没有替代该验收。
- #3 option 回归覆盖 host 与 unattended 的追加和硬限制保留；wire 回归使用真实 Agent session 与本地 HTTP provider，检查 new、三次实际 `set_session_model` effort replacement、关闭后 reload 的五个模型请求，暗号恰好一次且 core boundary 保留。这里的替换使用同一模型 id，不宣称测试了另一外部 provider。
- #4 新测试覆盖发布 schema、未知/缺参/错形参数、被过滤 orchestrator、按具体 Git 参数判断只读、两个真实工作区内置模板及含引号/中文的 JSON 数据原样传递。新增 denial 测试使用显式 scoped test invoker；不是完整 HITL 或真实会话测试。既有真实 fact-run script denial/cancellation 测试覆盖同一共享执行链；模板分支没有独立实现另一套 VM 或 cancellation。
- 首轮 #4 after 的唯一失败是新增 echo 断言用了另一个 fixture 的 `echo:` 前缀；精确纠正后的测试日志为 [issue-4-after-echo-correction.log](issue-4-after-echo-correction.log)，已核到 1/1 通过。其余批次及 projection 回归以实现代理/根代理最终收据为准，本审查不提前声明通过。

不重复运行已通过测试，不声称完整外部 provider、真实 HITL 或整仓库验证完成。
