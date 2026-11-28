# GitHub 未关闭 issue 集中处置（2026-11-28）

## 范围与状态

通过 `gh issue list --state open --limit 1000` 取得 `aster-ai-lab/aster` 的 **36 个未关闭 issue**：#311、#324–#358。逐项读取正文和评论，区分已有修复、本轮修复、明确的能力边界和仍需外部证据的项目。未发表评论、关闭 issue、提交或推送。

本轮直接修改了 #341、#342、#343、#345、#346、#349、#351、#352、#354、#355、#357 的实现或回归测试。其他项目复用当前源码和已有证据，不重复改写。

## 逐项处置

| Issue | 当前处置与证据 |
| --- | --- |
| #311 | **仍缺 Windows 原生验收。** tenant 打包和跨平台查找修复保留；没有 Windows NSIS 安装包启动证据。见 [Windows 记录](../../desktop/2026-11-27-windows-tenant-installer/OUTCOME.md)。 |
| #324 | 已有新构建 GUI 证据：首次启动不再集中索取三项权限；功能触发的系统权限弹窗未宣称验证。 |
| #325 | 已有 GUI 证据：`xAI`、`魔搭社区` 和英文 `ModelScope` 文案正确。 |
| #326 | 已有 loopback 与重启证据：非法 URL 给出具体错误，合法自定义提供商可保存并重启保留。 |
| #327 | 已有模型元数据与 ACL 证据：工具能力、思考档位及 `reasoningEffort` 正确透传。 |
| #328 | 已有 Schema 与 loopback 上游证据：`office_file` 不再含数组形式的 `type`。 |
| #329 | 已有 TUI 回归：Code 后端提示 `/resume`，不再提示不可用的 `/dashboard`。 |
| #330 | 已有 ACP 协议证据：worktree list/gc/db-path 方法及对象参数可用。 |
| #331 | 按 issue 允许的替代契约处理：工具说明明确独立 `setsid/setpgid` 进程组可能存活；没有宣称会清理这些进程。 |
| #332 | 已有可操作的人工核验/新会话指引，不再指向不存在的 RPC；失败子任务只结清有完整证据的只读调用。 |
| #333 | 已有 TUI 回归：Code 后端不再假装支持 Ctrl+B 转后台，给出等待完成或 Ctrl+C 的指引。 |
| #334 | 已有渲染回归：设置宽度、终端上限、缓存失效及正文边距统一处理。 |
| #335 | 已有真实窗口拖放证据：PNG/HTML/Markdown 不再导航并覆盖主 UI。 |
| #336 | 当前 `AppMarkdown` 默认启用单美元数学公式，聊天设置继续透传。`remarkLatexMath.test.ts` **57 项通过**。 |
| #337 | 首页与各会话使用独立草稿存储；首页卸载/重挂、项目保留及首次发送的定向测试通过。 |
| #338 | 首次发送主动 adopt 新会话并关闭首页覆盖；定向 `moves a plain fallback send onto a new task` 通过。未测真实窗口白屏时长。 |
| #339 | 已核对 `bypassPermissions → yolo` 映射及下一轮生效路径。**原始会话、工作区和工具回执缺失，未复现，不能宣称用户现场已恢复。** |
| #340 | **仍需按 issue 所列模型与场景采样。** 当前只有其他 DeepSeek 评测的耗时，不能替代 `k3.1-pro` / `glm-5.3` 基线；未凭感觉做性能优化。 |
| #341 | `--plan` 映射到现有 plan 权限，显式 `--permission-mode` 优先；ACP 初始化不再清除启动 plan。两个 Pager 回归各 **1 项通过**；完整二进制模型首请求未复验。 |
| #342 | 项目 MCP 写入路径统一为 `.aster/config.toml`，与读取及帮助一致。`project_write_path_matches_project_discovery_contract` **1 项通过**。 |
| #343 | 当前 `rmcp` 不再提供可直接复用的旧 HTTP+SSE 服务类型。为避免误发 Streamable HTTP POST，旧 SSE 现在明确拒绝并提示改用 Streamable HTTP。**未实现旧 SSE 兼容，不据此关闭兼容性诉求。** `legacy_sse_is_refused_before_streamable_http_transport` **1 项通过**。 |
| #344 | 沿用当前效果核验与独立检查证据，不把成功的 Git worktree 变更误当成可跳过身份绑定结算。 |
| #345 | 所有 Shell 先在内存生成，再统一处理 `BrokenPipe`。真实二进制五种目标正常输出与修改前逐字节一致；关闭读端后均退出 0、无 panic、无 stderr。另有 **1 项单元回归**。 |
| #346 | 删除从进程 stderr 关键词推断沙箱拒绝并建议提权的逻辑；普通参数错误不再被标成沙箱拦截。`process_error_text_does_not_propose_escalation` **1 项通过**。 |
| #347 | DOCX 在分离 staging DOM 中一次渲染后替换页面，不因滚动重复渲染。`WordFilePreview.test.tsx` **12 项通过**；未测得原文件的 1 秒加载目标或真实滚动帧率。 |
| #348 | 项目归属继续使用会话原始 workspace，首页 pending project 只在首次发送时消费。相关项目/首页定向测试通过。 |
| #349 | `office_file` 支持完整新 revision 的原地替换；已存在普通文件在生成期间发生变化则拒绝，符号链接和非常规文件仍拒绝。中文 XLSX 新行/新公式及 DOCX 追加内容回归 **1 项通过**。 |
| #350 | “在项目中新建任务”传递指定 project id；`creates tasks in their project...` 等定向测试通过。 |
| #351 | ACP 尚无完整的 worktree 会话恢复实现。创建/恢复方法现在明确提示 `system Git + aster --cwd <path>`，不再只有无上下文的方法缺失。**未宣称实现创建/恢复。** 两个相关回归各 **1 项通过**。 |
| #352 | 分别保留 pull 与 publishDiagnostics 来源，去重合并，避免空 pull 覆盖 rustc 推送诊断。合并单测和真实 fake-LSP 进程回归各 **1 项通过**。 |
| #353 | 分类按钮的文字和图标均位于同一 button 点击区域；目录选择及筛选请求定向测试通过。 |
| #354 | Cron 可携带 IANA timezone，按该时区求值并输出 UTC instant；Agent 工具保留时区。Rust 测试已写，但被并行 Quark 改动的未覆盖 match 阻断，**尚未取得通过结果**。 |
| #355 | Agent 工具新增 RFC3339 `once`，映射现有原生一次性 trigger，不再以指定日期 Cron 冒充一次性；技能说明明确两者互斥。Rust 测试已写，验证阻断同 #354。 |
| #356 | 当前宿主确认/问答结算会把答案写回原运行并继续未完成回合；没有修改该共享路径。未重放 GLM 原始会话。 |
| #357 | 暂停任务的列表与详情不再把旧 nextRun 显示为即将运行；missed 状态保留诊断时间。`TasksSettings.test.tsx` **104 项通过**。 |
| #358 | 会话模型在 agent/model/provider/runtime 加载期间保持 loading，只有 invalidated 才显示不可用。`useAgentConversationBootstrap.test.ts` **11 项通过**。 |

#324–#335 的既有证据：[Core/TUI](../2026-11-27-open-issue-regressions/OUTCOME.md)、[Desktop](../../desktop/2026-11-27-open-issue-regressions/OUTCOME.md)。#332 的补充证据：[失败子任务结算](../2026-11-27-child-effect-closure-on-failure/OUTCOME.md)。

## 本轮实际执行的检查

以下命令均在本仓库根目录或其注明的 Studio 目录执行；每条 Rust/脚本检查以 `test -f .aster-tmp/cdp_probe.py &&` 绑定当前工作树。仅列实际结果，不把超时或编译失败写成通过。

- `cargo test --locked -p aster-pager --lib completions_cmd`：通过。随后 `cargo build --locked -p aster --bin aster` 成功。五种 Shell 的正常输出与基线逐字节一致，关闭管道均退出 0。
- `cargo test --locked -p aster-pager --lib launch_plan_mode_defaults_to_normal` 与 `apply_aster_slash_gate_hides_unsupported_builtins`：各 **1 项通过**。后续 `cargo check --locked -p aster --bin aster` 在 600 秒预算内未完成，**未宣称通过**。
- `cargo test --locked -p aster-shell --lib project_write_path_matches_project_discovery_contract`：**1 项通过**。
- `cargo test --locked -p aster-core --lib merge_keeps_pull_and_push_diagnostics_without_duplicates` 和 `empty_pull_diagnostics_preserve_published_compiler_results`：各 **1 项通过**。
- `cargo test --locked -p aster-core --lib process_error_text_does_not_propose_escalation`：**1 项通过**。
- `cargo test --locked -p aster-acp --lib existing_documents_are_replaced_with_the_complete_validated_revision`：**1 项通过**。
- `cargo test --locked -p aster-mcp --lib legacy_sse_is_refused_before_streamable_http_transport`：**1 项通过**。
- `cargo test --locked -p aster-acp --lib unsupported_session_creation_names_a_supported_fallback` 和 `every_host_method_parses_back_from_its_name`：各 **1 项通过**。
- `cargo test --locked -p aster-acp --lib worktree`：现有 `gc_duration_is_bounded_and_rejects_invalid_values` **1 项通过**。
- Studio `TasksSettings.test.tsx`：**104 项通过**。
- Studio `useAgentConversationBootstrap.test.ts` + `WordFilePreview.test.tsx`：合计 **23 项通过**。
- Studio `remarkLatexMath.test.ts`：**57 项通过**。
- 首页/项目/分类定向筛选：三个文件合计 **6 项通过，236 项跳过**。 broader run 为 **287 通过、16 失败**；失败位于既有样式、音频/PDF fixture 等非本轮路径，未宣称全套通过。
- 相关 Rust 文件定向 `rustfmt --check`、根仓库与 Studio `git diff --check`：通过。
- `cargo test --locked -p aster-studio --lib agent_plane::schedule`：两次均未得到通过结果。失败是并行 Quark 改动留下 `agent_browser_runtime.rs` 对 `BrowserBackend::Quark` 的非穷尽 match；另有并行修改期间的语法/类型错误。**这些外部失败没有被修改或掩盖，#354/#355 仍为未验证。**

## Explicitly not claimed

- **没有完成全部 36 项的关闭条件。** #311 缺 Windows 原生启动；#339 缺原始配置和拒绝回执；#340 缺指定模型分段测量；#343 只做到明确拒绝而非旧 SSE 兼容；#351 只提供明确降级路径而非 worktree 会话恢复；#354/#355 的 Rust 回归尚未运行通过。
- #338/#347 的真实窗口时延和滚动帧率未测量；#341 的真实模型首请求未重放；#356 的 GLM 原始确认会话未重放。
- 未进行 GitHub 评论、关闭、提交、推送、Studio 子模块 landing 或父 gitlink 更新。所有改动留在当前工作树，并保留大量无关并行修改。
- 未运行 Windows/Linux 原生检查、完整 workspace 构建、完整 Studio/Vitest 套件或新一轮 GUI E2E。广泛 Vitest 运行中的 16 个既有失败和并行 Quark 构建失败均不计为通过。
