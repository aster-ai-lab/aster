# Full Aster TUI acceptance campaign: Windows and native Linux

Status: in progress. Goal completion is not established by the historical ten
tests, by menu visibility, or by a passing scenario count. Each feature below
requires its actual effects and both platform results to be audited.

Latest fully executed same-source batch: Windows 35/35 (1492.90 s), Linux
35/35 (1760.44 s), pair `a891556b0cee8096b81768f051ce80918e460099`.
`RUNS-A891.json` binds both native target identities, executable hashes, 41
case reports per platform and every report digest. Both runs use the original
operator ACL and exact `kimi/k3.1-pro` model. The earlier `43ce1a06...` and
`32b8c01e...` counterexamples remain retained. This is a bounded live-suite
result, not completion of the 153-checkpoint campaign or the full pager
library. See `RUNS-A891.json`, `RUNS-43CE.json`, `FINAL-RUNS.json` and
`LIBRARY-FAILURES.json` for those distinct contracts.

The startup follow-up pair `18998267...` qualifies 9 native CLI contracts per
platform plus one native TUI draft-control case per platform. Configuration
tests pass 25/25 on Windows and Linux; the isolated cwd regression passes
1/1 on each. These focused runs do not replace the full 35-entry batch.
`STARTUP-RUNS.json` binds frozen graders, binary hashes and exact reports.
The strict checkpoint index now verifies 37/153 (24.18%); 62 remain partial,
53 pending, and one explicitly unavailable. Full acceptance remains open.

The F04.01 follow-up pair `4fe1c6c8c4de5d03acd36b95640201d44a8ce4d3`
passes `/new` history, permission and runtime isolation on Windows (20.43 s)
and Linux (26.96 s). Both sessions complete real Kimi turns, the new session
starts without inherited YOLO, and the original fact-log bytes remain unchanged.
Three related YOLO/Plan/explicit-deny scenarios also pass on Windows (169.49 s)
and Linux (187.03 s). Both platforms pass 35 focused `/new` hermetic tests and
ACP live-test-target Clippy; the pager's unchanged-path Clippy errors and
Windows ConHost-only toast expectation failures remain separately recorded.
`source/session-isolation-verification.json` binds the successful reports to
binary hashes and the verified source patch; the original failures remain in
`session-isolation-before-fix` on both platforms. This slice is not a new full
35-scenario batch or completion of the 153-checkpoint campaign.

The original operator-owned `./.a3s/config.acl` selects `kimi/k3.1-pro` for
live inference. It is never copied into fixtures or evidence. Temporary
workspaces/homes and redact-on-write evidence keep operator state isolated.

## 功能细化与入口复核（2026-10-28）

本节将现有验收范围细化为可独立判断的检查点，供后续测试与修复使用。
检查点是验收要求，不表示功能已实现或测试已通过；条件能力先核对实际注册和
admission，再决定测成功路径还是明确拒绝。后文保留各次运行及失败证据。

每项分别回答：从哪里进入、完成什么操作、产生什么真实结果、失败后如何恢复、
是否需要保存并重启验证、Windows 和 Linux 是否有各自证据。
同一个用例可以覆盖多个检查点，但必须指出各自断言；不把菜单数量、断言数量、
用例数量换算为功能完成率。

### 入口与能力的实际关系

| 层次 | 源码复核结果 | 验收影响 |
| --- | --- | --- |
| 普通命令 | effective 集合 31 项，其中 `/cd` 仅 Dashboard 使用；Code 隐藏 Dashboard，因此普通会话有 30 个命令入口 | 逐项核对本地效果与 ACP 效果，不能只检查补全列表 |
| 禁用命令 | unavailable 集合 42 项，另有 18 个禁用设置键 | 测隐藏、手输拒绝和不转发；不能宣称这些能力已经可用 |
| ACP 命令 | ACP 自身公布 `/goal`；其余很多命令由 pager 本地处理或调用扩展方法 | ACP 公布数量与 TUI 命令数量不能相互代替 |
| 非命令入口 | 快捷键、输入模式、面板、选择器、计划审阅、权限卡、表单、CLI 参数 | 隐藏某条 slash 命令不能证明其他入口已禁用或已可用 |
| Plan | `/plan`、`/view-plan` 被列为不可用，但 `--plan` 和模式切换仍需独立验收 | 区分进入只读模式、查看计划、审批计划和执行计划 |
| 会话恢复 | `/resume` 被隐藏，CLI `--resume`、`--continue` 和会话选择器是不同入口 | CLI 恢复成功不能替代选择器测试；恢复文字不能替代模型历史恢复 |
| 历史与队列 | `/history`、`/queue` 被隐藏；Up 历史与运行中输入/队列面板有独立本地代码 | 需要检查非命令入口、帮助文字和实际执行顺序 |
| 工具与扩展 | 工具按 workspace capability、runtime service 和配置注册 | `/plugins` 隐藏不等于模型侧插件工具不存在；SDK 注册也不等于本会话可使用 |
| 多模态与认证 | initialize 宣告文本能力、空 auth_methods；turn 入口拒绝 image/audio/resource/resource_link | 图片/语音粘贴和登录提示需要与后端能力一致 |

入口集合：pager `app/app_view.rs:525,578,614`；快捷键定义：
`actions/defaults.rs`；快捷键实际处理：`app/agent_view/input.rs`；
能力宣告：ACP `agent.rs::initialize` 与 `agent/commands.rs`；
输入拒绝：ACP `agent/turn.rs::prompt_text`。

30 个普通命令按主要入口归类如下；一个命令可能承担多个检查点。

| 入口组 | 命令 |
| --- | --- |
| 会话 | `/new`、`/home`、`/session-info`、`/delete`、`/quit` |
| 推理与上下文 | `/model`、`/effort`、`/context`、`/compact` |
| 配置 | `/settings`、`/config` |
| 持久目标 | `/goal` |
| 阅读导航 | `/jump`、`/expand`、`/find` |
| 输入编辑 | `/edit-prompt`、`/multiline`、`/vim-mode` |
| 权限 | `/yolo` |
| 结果交付 | `/export`、`/copy` |
| 外观与布局 | `/theme`、`/compact-mode`、`/timestamps`、`/toggle-mouse-reporting`、`/minimal`、`/fullscreen`、`/timeline` |
| 诊断帮助 | `/doctor`、`/help` |

### F01 启动、配套进程与退出

| 检查点 | 操作与成功判据 |
| --- | --- |
| F01.01 | 无模式参数启动：显示 Normal，实际请求使用原 ACL 的 `kimi/k3.1-pro` |
| F01.02 | 显式 `--plan` 启动：会话权限真实只读，不能由普通启动测试替代 |
| F01.03 | ACP 发现与握手：使用预期 companion，TUI/ACP source-pair 相同，平台目标正确 |
| F01.04 | companion 缺失、版本/源码不匹配、无法执行：给出原因并保持可退出，不回落到另一产品 |
| F01.05 | ACL 缺失、语法错误、模型不存在：区分首次配置路径和配置错误，不静默改用其他模型 |
| F01.06 | TTY 与非 TTY：交互/非交互路由明确，不能卡在不可操作的信任或权限界面 |
| F01.07 | 启动中取消、正常 `/quit`、异常子进程退出：终端模式恢复、子进程结束 |
| F01.08 | ACP 断开后重连：有明确状态；恢复的 session 和 draft 正确，已完成工具不重复执行 |

源码：`apps/aster/src/main.rs`、`apps/aster/src/startup.rs`、pager
`app/cli.rs`、`acp/mod.rs` 和 `agent_runtime.rs`。

### F02 工作区、信任与隔离

| 检查点 | 操作与成功判据 |
| --- | --- |
| F02.01 | 新工作区信任接受：仅批准指定 canonical workspace，随后才执行获准能力 |
| F02.02 | 信任拒绝与取消：不启动有副作用工作，不留下等价于接受的持久记录 |
| F02.03 | 重启与第二工作区：信任只对原工作区生效；会话和学习规则不串用 |
| F02.04 | 中文、空格、相对路径与平台路径：UI、ACP、工具指向同一真实目录 |
| F02.05 | workspace ACL/内容不能替换 host provider、凭据或提升 host 授权 |
| F02.06 | 越界路径、symlink/reparse point 与大文件读取：遵守原 sandbox 和 bounded-read 契约 |

源码：ACP `host_context.rs`、`agent/session_open.rs`，Core `workspace/`。

### F03 输入、编辑与快捷操作

| 检查点 | 操作与成功判据 |
| --- | --- |
| F03.01 | 单行、中文、符号、空白：实际 `user.message` 与提交内容逐字一致，仅提交一次 |
| F03.02 | 多行输入与提交键：换行保留，普通 Enter 与替代提交键符合当前模式 |
| F03.03 | 长文本/bracketed paste：不提前提交、不丢尾部、不把控制序列当正文 |
| F03.04 | 光标移动、词/行删除与选区：只改变目标 draft，不误操作 transcript 或其他面板 |
| F03.05 | undo/redo、清空与 stash/pop：恢复完整 draft，多行和 Unicode 不被破坏 |
| F03.06 | Up/Down 历史：真正回填 composer，能编辑再发送；屏幕旧文字不算回填证据 |
| F03.07 | 外部编辑器保存：等待编辑器退出，原样回填，不额外发送或丢失 draft |
| F03.08 | 编辑器取消、非零退出、启动失败：保留原 draft 并恢复终端控制 |
| F03.09 | 文件引用/补全：选择的真实文件及范围正确；不存在或越界引用明确拒绝 |
| F03.10 | `!` shell 模式：核对是否按直接命令语义执行，实际 exit/output/cancel 归属正确 |
| F03.11 | 运行中输入、发送与插话：区分排队和立即干预，正文不丢失、不重复发送 |
| F03.12 | 中文 IME 与原生粘贴：实际终端组合输入单独验证；PTY 注入 UTF-8 不代替 IME |

源码：pager `app/agent_view/prompt.rs`、`views/prompt_widget/`、
`actions/defaults.rs`、`app/effects/mod.rs::SendBashCommand`，ACP
`agent/turn.rs::prompt_text`。

### F04 会话的创建、切换与恢复

| 检查点 | 操作与成功判据 |
| --- | --- |
| F04.01 | `/new` 创建独立 session，前一会话的历史、权限、运行状态不污染新会话 |
| F04.02 | `/home` 返回对应界面，取消/返回不会误建或丢弃已有会话 |
| F04.03 | `/session-info` 的 id、cwd、model、mode、turn/context 与实际后端一致 |
| F04.04 | CLI `--resume` 恢复指定 id，恢复 transcript 后模型能利用先前历史而非重新读文件 |
| F04.05 | CLI `--continue` 选中正确工作区最近会话，不仅仅打开一个新会话 |
| F04.06 | 会话选择器筛选/选择/取消；重名标题仍按真实 id 选择，不猜测身份 |
| F04.07 | 切换运行中会话：回复、权限卡和工具结果回到原 owner，不写进当前可见会话 |
| F04.08 | `/delete` 确认与取消：只删指定会话，列表/磁盘一致，不影响其他会话 |
| F04.09 | 损坏记录、失踪文件、运行中退出后恢复：明确失败或恢复，不重放副作用 |

源码：ACP `agent/session_catalog.rs`、`session_open.rs`、`extensions.rs`，
pager `app/dispatch/session/` 和 `views/session_picker.rs`。

### F05 模型、配置与上下文

| 检查点 | 操作与成功判据 |
| --- | --- |
| F05.01 | `/model` 目录、默认值、当前选中值与 ACP 实际模型状态一致 |
| F05.02 | 切换后下一次真实推理使用所选模型，不仅更新 footer；不擅自换掉本次指定 Kimi |
| F05.03 | `/effort` 合法/非法值及实际 transport：区分 provider 原生参数与 host budget fallback |
| F05.04 | `/config` 新建/编辑/删除 provider，id 和连接字段真正保存且不覆盖其他 provider |
| F05.05 | 新建/编辑/删除 model，id、name、模型级 URL/key override 与所属 provider 正确 |
| F05.06 | 默认 provider/model、context/output limit、thinking budget、timeout 作用于后续请求 |
| F05.07 | 空 id、重复/非法配置、数值边界：保存前明确拒绝；旧有效 ACL 保持可用 |
| F05.08 | 字段编辑取消和整体放弃：不发生隐式保存；reload 恢复磁盘状态 |
| F05.09 | Save/reload/进程重启：ACL 可由 `a3s-acl` 重读，真实 Kimi 请求仍可工作 |
| F05.10 | 密钥默认掩码，环境变量引用保留；日志、导出和 evidence 不泄露原凭据 |
| F05.11 | `/context` 随真实轮次更新 used/total/free；未知 session 不伪造使用量 |
| F05.12 | `/compact` 和带 focus 压缩：产生真实后端结果；压缩后仍能回答先前关键信息，失败不丢会话 |

源码：pager `views/model_config/{state,document,render}.rs`、`slash/commands/`；
ACP `agent/models.rs`、`agent/extensions.rs`。

### F06 流式输出、排队与取消

| 检查点 | 操作与成功判据 |
| --- | --- |
| F06.01 | text/thinking delta 正确增量渲染，最终正文与真实完成结果一致 |
| F06.02 | ToolStart/更新/ToolEnd 对应同一调用卡，不重复建卡、不串接其他调用 |
| F06.03 | busy/idle 与真实 parent turn 一致，正在等待权限/表单不能显示为已完成 |
| F06.04 | 多条 queued prompt 按合同顺序各执行一次，不能拼接、遗失或重复执行 |
| F06.05 | 队列查看、编辑、删除、取消对目标项生效，未提交 draft 与已排队项分开 |
| F06.06 | 插话/干预产生对应后端操作，原 turn 与新消息的完成或取消有独立归属 |
| F06.07 | 取消模型流和正在运行工具：响应及时，进程/子任务停止，观察窗口内无晚到写入 |
| F06.08 | 失败后的输入框、重试与重连可操作，不重复已经完成的文件/进程副作用 |
| F06.09 | 切换/关闭会话后的迟到流事件不覆盖新会话，不解除另一会话的 busy 状态 |

源码：pager `app/dispatch/queue.rs`、ACP handler、ACP `agent/turn.rs` 和
`agent/extensions.rs::handle_interject`。

### F07 工具执行与编码工作流

以下工具族按实际会话注册验收。条件不满足时记录能力缺失和明确拒绝，不能把
未调用工具算作成功，也不能为了得到绿灯越过权限直接调用 host API。

| 检查点 | 操作与成功判据 |
| --- | --- |
| F07.01 | `read` 读取测试事先未知内容；范围、分页、EOF、缺失/越界错误正确 |
| F07.02 | `ls` 返回真实目录项，路径/排序/截断按工具合同，不暴露工作区外内容 |
| F07.03 | `search` 返回真实文件/位置，测试有匹配、无匹配、范围和截断；不把 UI `/find` 当代码搜索 |
| F07.04 | `write` 新建与覆盖：逐字节核对 Unicode、LF/CRLF、有/无尾换行，失败不留下半成品 |
| F07.05 | `edit` 精确替换：唯一/多重/未匹配分别有正确行为，只改变预期片段 |
| F07.06 | `patch` 单/多文件变更与冲突：真实结果、失败范围和 diff 一致 |
| F07.07 | `bash` 成功与非零退出：实际 exit code、stdout/stderr、cwd、引用/含空格参数正确 |
| F07.08 | shell timeout、输出截断和取消：实际停止，隐藏输出可检查，无虚构成功 |
| F07.09 | `download` 真实目标字节和目的路径正确，失败/拒绝无越权或半写入 |
| F07.10 | `background` start/poll/cancel：任务 id、进程输出与结束状态对应，无晚到副作用 |
| F07.11 | `git` 在隔离 fixture repo 中读状态/差异及执行获准操作；不触碰用户真实工作树 |
| F07.12 | `web_fetch` 实际受控端点内容、链接、错误/超时与来源呈现正确 |
| F07.13 | `web_search` 有配置时验证真实搜索结果与引用，无服务时验证明确不可用 |
| F07.14 | `update_plan` 更新 checklist/进度，UI 展示与实际 receipt 一致；不等于 Plan 模式验收 |
| F07.15 | `batch` 保留每项调用/结果，部分失败不会伪装全成功或重复执行成功项 |
| F07.16 | `program` 按实际 catalog 执行，参数/子调用/失败归属正确，不绕过权限 |
| F07.17 | 条件工具：代码智能、语义/混合检索、文档/Office、资产/wiki/MCP/plugin 逐族核对 admission 与结果 |
| F07.18 | 工具卡、diff、raw 与展开尾部对应真实参数/结果，内容不串卡、不漏关键错误 |
| F07.19 | 多轮修复实际 bug 并运行独立 checker；verified completion 绑定真实产物，助手宣称不算完成 |

源码：Core `tools/builtin/mod.rs::register_builtins`、`register_batch`、
`register_program`、task 注册函数及 ACP `agent/session_open.rs`。

### F08 权限与 sandbox

| 检查点 | 操作与成功判据 |
| --- | --- |
| F08.01 | Normal/Auto 的真实 read/write/exec 决策符合声明，不仅检查模式标签 |
| F08.02 | Allow once 只授权所选调用；相同工具的后续未授权调用仍按策略询问 |
| F08.03 | 可学习调用 Allow always 持久化正确工具/参数规则，重启仍生效 |
| F08.04 | 学习规则只作用于原 workspace，第二工作区仍须真实询问 |
| F08.05 | Reject 与 Cancel 产生不同的真实结果，均不修改目标内容 |
| F08.06 | YOLO 允许已获授权执行，同时 CLI/host 显式 deny 仍有效 |
| F08.07 | CLI `--allow`/`--deny` 经 TUI→ACP→Core 传递，重开/换模式后仍按同一规则执行 |
| F08.08 | Plan 与 Plan+YOLO 保持只读，学习规则/模式切换不能提升只读上限 |
| F08.09 | 迟到、重复、旧会话审批不授权新调用；取消后不能继续执行 |
| F08.10 | 平台 sandbox 真实拦截越界文件/进程能力，拒绝原因可见，不能仅用模型拒绝代替 |
| F08.11 | 学习 ACL 路径转义正确；权限卡参数和将要执行的真实调用一致 |

源码：ACP `agent/permission_state.rs`、`permission_ask.rs`、`approval_learning.rs`，
pager `acp/mod.rs`、`app/dispatch/modes.rs` 和 `app/effects/helpers.rs`。

### F09 计划与执行转换

| 检查点 | 操作与成功判据 |
| --- | --- |
| F09.01 | 显式 Plan 会话能真实 read/search；写入和有写副作用的 exec 按只读契约被拒绝 |
| F09.02 | 计划产生后能从实际可达入口查看对应计划，不依赖被隐藏命令 |
| F09.03 | 审阅的正文、文件来源和版本对应当前 plan，不显示上次会话残留 |
| F09.04 | Revise 把修改意见发给对应计划工作；放弃/取消只影响目标计划 |
| F09.05 | Execute 获得确认后才切换执行权限，并产生精确产物和独立验证结果 |
| F09.06 | Shift+Tab、YOLO toggle 与 ACP 确认顺序不造成 UI/后端模式不一致 |
| F09.07 | 执行启动后重复批准/修改/放弃，或旧 review 迟到，不能重复执行或篡改新计划 |
| F09.08 | 重启、重连、切换会话后计划状态与权限正确；复现失败须保留原 prompt/grader |

源码：pager `app/dispatch/modes.rs`、`app/acp_handler/session_notification.rs`、
`app/acp_handler/tests/plan_mode.rs`，ACP permission state 与 Core planning admission。

### F10 持久目标

| 检查点 | 操作与成功判据 |
| --- | --- |
| F10.01 | `/goal <objective>` 建立对应目标、session 绑定和 durable fact |
| F10.02 | status 显示真实目标与状态，目标为空或 id 不存在时明确说明 |
| F10.03 | 目标触发真实多轮工作和可观察进展，不仅写入目标文字 |
| F10.04 | pause 停止后续目标工作；resume 继续同一目标与累计预算 |
| F10.05 | clear 清除目标控制，不误删对话或把其他 session 的目标清除 |
| F10.06 | achieved 需要真实完成/验证，失败或仅有助手自述不能标为完成 |
| F10.07 | turn/time/token 限制分别在相应边界停止，失败/重试不能清零预算 |
| F10.08 | 退出重启后恢复目标身份、进展和限制，不重复已完成效果 |

源码：ACP `goal/`、`agent/goal_ext.rs`，Core goal control 与 fact store。

### F11 问答与结构化表单

| 检查点 | 操作与成功判据 |
| --- | --- |
| F11.01 | `ask_user` 真实问题、选项、自由输入、取消结果对应当前调用 |
| F11.02 | `ask_user_form` 按实际支持 schema/component 渲染，不能以单个表单代表所有组件 |
| F11.03 | required、type、range、选项约束拒绝非法值且保留可修改输入 |
| F11.04 | Tab/反向焦点、选择与文本输入命中目标字段，提交值类型与内容准确 |
| F11.05 | Cancel 使真实工具返回取消，模型/会话能继续，不把默认值当作确认 |
| F11.06 | 重复提交、迟到回复、切换 session 后回复不能回答另一请求 |
| F11.07 | 不支持组件/schema 明确拒绝，不显示一个无法完成的空表单 |
| F11.08 | 长表单与小窗口中所有必需字段和提交/取消控件可达 |

源码：ACP `form_request.rs`，Core `tools/builtin/ask_user*.rs`，pager
interaction/question/form handlers。

### F12 浏览、搜索与阅读

| 检查点 | 操作与成功判据 |
| --- | --- |
| F12.01 | `/find` 命中已知真实正文，视口/选中位置正确，不只打开搜索框 |
| F12.02 | next/previous 遍历预先知道的多个位置，零匹配与取消行为正确 |
| F12.03 | `/jump` 精确到指定 turn，选择器筛选/取消不改变错误位置 |
| F12.04 | 上/下轮、响应、顶部/底部、行/半页/整页导航作用于当前 pane |
| F12.05 | 单块折叠/展开、全部展开、thinking 展开各作用于正确块并可恢复 |
| F12.06 | Raw Markdown 与格式化显示切换后对应同一内容，不丢正文或改变导出内容 |
| F12.07 | block viewer 展示正确完整内容，关闭后恢复原选择与焦点 |
| F12.08 | 链接遍历/打开命中实际目标，路径/URL 可区分；失败后仍能回到原 pane |
| F12.09 | timeline 刻度、hover/选择和跳转对应真实 turn，窗口变化后位置仍正确 |

源码：pager `views/jump.rs`、`views/history_search.rs`、`app/dispatch/jump.rs`、
`actions/defaults.rs` 与 `scrollback/`。UI transcript search 与工具 search 分开验收。

### F13 设置、布局与终端交互

| 检查点 | 操作与成功判据 |
| --- | --- |
| F13.01 | `/settings` 筛选、描述、bool/enum/string/int/group 编辑分别可操作 |
| F13.02 | enum preview 的确认/取消、reset 默认值及校验错误符合界面承诺 |
| F13.03 | theme/compact/timestamp/timeline 真正改变渲染，不能仅核对保存值 |
| F13.04 | thinking 显示/宽度、工具分组、手动折叠等实际可见项产生对应效果 |
| F13.05 | 保存重启后设置真正应用；需重启项与即时项有明确区别 |
| F13.06 | Vim 模式执行真实移动、选择、折叠/搜索和编辑；启用偏好本身不算 Vim 功能通过 |
| F13.07 | minimal/fullscreen 切换与 relaunch 保留正确 session/draft，终端滚屏行为符合模式 |
| F13.08 | 窄/宽、低高度和连续 resize 后控件可达、文字不遮住输入或确认按钮 |
| F13.09 | 鼠标报告开关与实际点击、选择、滚动分别验收；协议事件与物理鼠标分别记证据 |
| F13.10 | 焦点进入/退出 pane、modal 和 help 后按键作用于预期位置；快捷键说明与后端能力一致 |

源码：pager `settings/defs.rs`、`views/settings_modal/`、`views/shortcuts_help.rs`、
`actions/defaults.rs` 和 agent view rendering/input。

### F14 复制、导出与剪贴板

| 检查点 | 操作与成功判据 |
| --- | --- |
| F14.01 | `/export` 的完整 transcript 按正确角色/轮次顺序导出，恢复会话后仍准确 |
| F14.02 | `/copy` 与 `/copy N` 选择正确实际 assistant message，不能误取 thinking/tool |
| F14.03 | `/copy [N] <file>` 逐字节保留内容、中文、空格路径与尾换行 |
| F14.04 | Copy block content 与 Copy command/path 复制各自指定内容，不混为整段回答 |
| F14.05 | 原生 Windows 系统剪贴板与 Linux 实际 clipboard service 分别读取验证目标内容 |
| F14.06 | clipboard 不可达时明确 fallback/失败，不因文件副本存在就报告系统剪贴板成功 |
| F14.07 | 路径不可写、目标是目录、无消息、非法 N：明确错误且不触发模型请求 |
| F14.08 | OSC52/终端转发仅在实际支持时验证；不能用一个平台的转发证据代替另一平台 |

源码：pager `slash/commands/{copy,export}.rs`、对应 dispatch/effects 与 clipboard 实现。

### F15 子任务、后台工作与结果归属

| 检查点 | 操作与成功判据 |
| --- | --- |
| F15.01 | 实际支持的 `task` delegation 创建可识别 child，与 parent/session/task id 对应 |
| F15.02 | 多个 child 的进度/文本/工具结果分别显示，不混入其他 child 或 parent 正文 |
| F15.03 | parent 等待并消费指定 child 的真实结果，不把任务已发起当任务已完成 |
| F15.04 | 子任务面板、选择/切换/接管等入口按实际 capability 验收成功或明确不可用 |
| F15.05 | 接管不提升 inherited capability，不修改另一个 session 的 draft/权限/状态 |
| F15.06 | child 失败、parent cancel、单任务 cancel 正确传播，无孤儿进程或晚到写入 |
| F15.07 | 后台 shell job 与 agent child 分开验收：身份、可取消对象、输出与完成归属不同 |

源码：Core task registration/child run、ACP child event handling、pager
child action filters 与 task/background 面板；不恢复禁用的上游 Dashboard 来代替这些验证。

### F16 错误、诊断和能力一致性

| 检查点 | 操作与成功判据 |
| --- | --- |
| F16.01 | 42 个禁用命令逐项拒绝，未知命令拒绝；均无新增 parent model/session work |
| F16.02 | 18 个禁用设置键及条件隐藏项不成为虚假可用入口 |
| F16.03 | 同一能力的 slash/快捷键/面板/帮助/CLI 入口行为一致，不留隐蔽死入口 |
| F16.04 | `/help` 和快捷键详情不承诺后端已明确拒绝的图片、命令或集成 |
| F16.05 | `/doctor` 返回实际检查结果与可定位问题；缺依赖不显示整体健康 |
| F16.06 | provider 鉴权/限流/超时/中断、ACP crash 分别显示原因并恢复可操作输入 |
| F16.07 | 错误重试/重连后已完成效果不重复，未完成工作不伪装完成 |
| F16.08 | 可选 MCP/plugin/Office/外部服务按实际配置与 capability 测激活或拒绝，不凭 SDK 名称推定可用 |
| F16.09 | 错误、日志、导出、session evidence 不泄露凭据或越权内容；退出后终端可继续使用 |

源码：pager command/settings gates、`views/shortcuts_help.rs`、`actions/defaults.rs`；
ACP initialize、`agent/turn.rs`、runtime admission 与错误处理。

### 历史证据映射与缺口（batch-2，后续修复见下文）

以下保留完整 21 场景批次 `batch-2` 当时的状态，source-pair 为
`1b5b3e3bd374c23f0325bd2e631c24becc0b9214`：
Windows 19 passed / 2 failed，Linux 19 passed / 2 failed。
两平台失败项不同。最新的双平台 23 场景完整通过结果及新增功能证据见后文；
本历史表中的缺陷状态不能代表当前候选源码。

| 范围 | 已保留的真实证据 | 尚不能据此宣称 |
| --- | --- | --- |
| F01–F03 | 配对身份、真实 Kimi read、多行 Unicode、含空格中文路径、实际 composer 历史两平台通过 | 全部启动异常、信任拒绝、长粘贴、外部编辑器、文件引用、直接 shell、原生 IME |
| F04 | resume/continue 恢复同一 id，Kimi 不调用工具回忆先前内容，两平台通过 | 选择器重名处理、删除、损坏记录、运行中切换和崩溃恢复 |
| F05 | provider/model CRUD、非法空 id、cancel/reload/save/restart 与后续 Kimi 请求两平台通过 | 所有字段边界、真实换模、effort wire、context 数值和压缩后信息保留 |
| F06–F07 | 真实读/写、修复并运行 checker、工具展开、取消长工具无晚到写入两平台通过 | 全流状态、队列/插话、全部工具族、所有编码/换行组合和恢复幂等性 |
| F08 | once/reject/cancel、Plan+YOLO 拒写两平台通过；学习规则重启/工作区隔离 Linux 通过 | 显式 deny 仍有真实失败，Windows 学习路径尚未完整通过，sandbox 全边界未覆盖 |
| F09 | 新 source-pair 的 Plan→执行 Windows 单场景通过；Linux 已不再权限拒绝 | Linux exact-byte 仍多一尾 LF；计划审阅/修改/放弃/重复审批整体尚未完成 |
| F10–F11 | 目标控制 durable facts、表单校验/提交/取消两平台通过 | 目标真实达成、所有预算/重启；所有表单组件和迟到/重复提交 |
| F12–F13 | 工具展开、部分偏好重启与实际反向切换两平台通过；命令入口巡检有证据 | 搜索真实位置、jump 精确 turn、Vim 真操作、全部布局/鼠标/终端输入 |
| F14 | 指定第 N 条回答精确文件字节、完整导出顺序、写入失败两平台通过 | 系统剪贴板、块复制、OSC52 及平台服务故障 |
| F15 | 源码入口与 owner 需要单独核对 | 真实子任务委派、进度、接管、取消和结果归属的完整 TUI E2E |
| F16 | 42 禁用命令和未知命令无模型转发、missing-read 真实失败 receipt 两平台通过 | 所有快捷键旁路/帮助一致性、provider/ACP 故障与无重复效果恢复 |

上述“通过”只指所列冻结用例的特定运行，不授予整域通过状态。
证据：`windows/batch-2/run.log`、`linux/batch-2/run.log` 及同目录各 case JSON；
后续 Plan 单场景见两平台 `plan-toggle-fixed/`。

### 修复与补测顺序

1. 先修已复现的权限与平台问题：CLI 显式 deny 未传入 sidecar；Windows
   governed `cat` 的只读操作被记录为 mutation；学习 ACL 已修生成器但需双端
   live 回归；Linux Plan 执行多一个 LF 的原因需区分模型、read 可观测性与工具契约。
   原 prompt、模型、超时和 exact-byte grader 保持不变。
2. 补关键闭环：信任拒绝/隔离、实际配置字段生效、压缩后记忆、队列/插话、计划
   审阅到执行、目标达成与限制、子任务结果归属。每项先确认真实可达入口和能力。
3. 补 UI 实际动作：find/jump 的位置、Vim、编辑器、原生 clipboard、resize、
   鼠标/IME。`shortcuts_help.rs` 仍有 `/history` 与图片粘贴提示，
   `actions/defaults.rs` 有 `/resume` 等共享帮助文字；这些与 Code gate 的
   一致性需要实际界面复核。直接 shell 的 block metadata 与 ACP text-only
   解析也需独立测试；当前仅是源码发现，不能冒充已复现 E2E 缺陷。
4. 所有适用检查点完成后，用同一最终源码重新成对构建 TUI/ACP，执行 Windows
   和原生 WSL Linux 全批次，核对换行/编码/脱敏/格式与 workspace checks，
   再按已授权要求提交、合并、推送 main。不以重复运行旧 21 场景替代缺失检查点。

跨平台矩阵必须包含：UTF-8、LF/CRLF、有/无尾换行；中文/空格路径；
ConPTY 与 Linux PTY 的 Enter/Backspace 编码；原生 Windows shell 与 Linux shell；
平台文件权限/ACL、symlink/reparse point；clipboard service 与终端协议差异。
原始 evidence/fixtures 按 `.gitattributes` 保留字节，不用批量换行归一化修复测试。

## Requirements and outstanding evidence

| ID | Feature domain | Required acceptance beyond opening an entry | Windows | Linux |
| --- | --- | --- | --- | --- |
| F01 | Startup and ACP | Default flags, host ACL/model, matched sibling, missing/bad companion/config, TTY admission and reconnect | Pending | Pending |
| F02 | Workspace and trust | Unicode/space paths, accept/reject/persist trust, host/workspace ACL boundary, cross-workspace isolation and path escapes | Pending | Pending |
| F03 | Composer | Exact multiline submission, UTF-8, long paste, actual history recall/edit, editor, file references, direct shell and native IME boundary | Pending | Pending |
| F04 | Session lifecycle | New/home, continue/resume, title ambiguity, delete/cancel, corrupt records and interrupted restart | Pending | Pending |
| F05 | Models/configuration | Catalog, model request, effort transport, provider edits, invalid/cancelled saves, restart application and secret protection | Pending | Pending |
| F06 | Streaming and turn state | Text/thinking/tool deltas, busy/idle, queued input, interrupted and failed streams, retry ownership | Pending | Pending |
| F07 | Real coding tools | Read/search/write/edit/shell, output/diff rendering, exact workspace mutations and verified completion | Pending | Pending |
| F08 | Permissions/sandbox | Once/always/reject/cancel, explicit denies under YOLO, Auto/Plan modes, workspace-scoped learning and sandbox boundaries | Pending | Pending |
| F09 | Planning | Explicit plan launch, read-only admission, review/revise/abandon/execute, transitions and stale/cancelled approvals | Pending | Pending |
| F10 | Durable goals | Control facts, actual multi-turn progress, achievement/limits and restart continuation | Pending | Pending |
| F11 | Human interaction | Typed forms/validation/cancel, supported components, repeat/late submissions and actual tool receipts | Pending | Pending |
| F12 | Navigation | Search match/next/previous positions, exact turn jump, folds/raw view, tool expansion and links | Pending | Pending |
| F13 | Presentation/input | Actual Vim actions, persisted appearance after restart, viewport/focus, physical mouse and terminal-specific input boundaries | Pending | Pending |
| F14 | Export/clipboard | Transcript and response file bytes, selected block and OS clipboard, encoding and failure fallback | Pending | Pending |
| F15 | Child work | Explicit delegation, child identity/progress, takeover isolation, cancellation and parent result ownership | Pending | Pending |
| F16 | Faults/capability boundaries | ACP/provider failures and recovery, no repeated effects, 42 disabled commands and unknown commands never sent to the model | Pending | Pending |

Normal mode is the default. Historical `--no-plan` tests do not establish the
separate `--plan` workflow. A disabled TUI entry is audited as an explicit
refusal, not counted as a working optional SDK integration.

## First additional batch

The new `functional` live tests cover exact multiline/Unicode input and actual
composer history, plan read followed by a verified execution-mode write,
failed-read receipts and backend/unknown-command refusal. These are partial
contracts within the domains above, not proof that the entire domains pass.
Inputs and outcome assertions are frozen before the first live execution;
subsequent fixes must distinguish implementation, driver and grader defects.

Historical evidence remains in the two earlier TUI qualification directories.
It is a baseline, not a current-main acceptance claim. Fresh evidence is
separated under `windows/` and `linux/` for this campaign.

## Atomic acceptance contracts

These contracts describe observable product behavior, not a scenario-count
target. A single test can prove several contracts; a contract can require
several tests. A domain remains pending until its applicable contracts have
separate Windows and native GNU Linux evidence.

| Domain | Operations and effects to verify |
| --- | --- |
| F01 | (1) Ordinary launch selects Normal and the host Kimi model. (2) Explicit Plan launch is distinct. (3) The discovered sibling ACP has the same source identity. (4) Missing/mismatched ACP and malformed/missing ACL report an actionable failure. (5) TTY/non-TTY, startup cancellation and reconnect preserve terminal/process state. |
| F02 | (1) Accept and reject a workspace trust request. (2) Trust persists only for the intended canonical workspace. (3) Unicode and space paths survive UI, ACP and tools. (4) Workspace content cannot replace host provider authority. (5) A second workspace cannot read another workspace's session or learned approval. (6) Escapes, links/reparse points and bounded reads preserve confinement. |
| F03 | (1) Single-line and multiline prompts arrive byte-for-byte, including LF and Unicode. (2) Editing, selection, clear and undo affect the intended draft. (3) Long/bracketed paste does not prematurely submit. (4) History recalls into the composer, can be edited, and is submitted once. (5) External-editor save/cancel preserves the draft. (6) File references and direct-shell input retain their actual typed semantics. (7) Native IME composition is separately qualified; injected UTF-8 is not IME evidence. |
| F04 | (1) New/home creates or selects the intended session. (2) Session info agrees with persisted identity. (3) CLI load/resume/continue selects the intended stored session. (4) Duplicate titles do not select a different identity. (5) Delete/cancel changes only the selected session. (6) Restart after interruption and corrupt/missing records fails or recovers explicitly without replaying effects. |
| F05 | (1) Catalog and current model agree with ACP. (2) Selecting a model affects the next actual request. (3) Reasoning effort reaches supported provider transport. (4) Add/edit/remove provider and model records, defaults, context/output limits and timeout. (5) Invalid edits are rejected; cancelling does not save. (6) Save/reload/restart uses the saved ACL and masks credentials. (7) Original operator ACL is never used as a destructive CRUD fixture. |
| F06 | (1) Text, thinking and tool deltas render in order. (2) Busy/idle follows the actual parent turn. (3) Queued input is submitted once with correct order/ownership. (4) Cancel terminates the intended work, with no late mutation. (5) Stream/provider/ACP failure restores a usable composer. (6) Retry/reconnect does not duplicate a completed tool effect. |
| F07 | (1) Read and search return unseen fixture content. (2) Write and edit produce exact intended bytes, including UTF-8, LF/CRLF and final-newline cases. (3) Shell success, nonzero exit, output streaming and timeout reflect actual process results. (4) Diff/tool cards identify the real input/output. (5) Expanded output exposes the hidden tail. (6) Verified completion is bound to actual output and a separate checker; an assistant claim alone is insufficient. |
| F08 | (1) Allow-once permits just the selected invocation. (2) Learnable allow-always persists a scoped rule. (3) Reject and cancel leave no mutation. (4) Normal/Auto/YOLO have their declared behavior. (5) Explicit deny remains binding under YOLO. (6) Plan remains read-only, including Plan plus YOLO. (7) Late/stale approvals and workspace changes cannot widen authority. (8) Sandbox refusal is visible and enforced. |
| F09 | (1) Explicit Plan admits real reads. (2) Actual non-plan writes are denied. (3) Plan/review content is visible. (4) Revise, abandon and cancel affect the intended plan. (5) Execute transitions before a verified mutation. (6) Shift+Tab, permission toggles, restart and stale review responses do not leave UI and ACP modes inconsistent. |
| F10 | (1) Goal create/status/pause/resume/clear emits corresponding durable facts. (2) Actual multi-turn work progresses toward the objective. (3) Pause prevents further goal work. (4) Achievement requires verified completion. (5) Turn/time/token limits stop correctly. (6) Restart resumes the same durable goal without repeating effects or losing limits. |
| F11 | (1) Supported typed question/form components render their actual schemas. (2) Required/range/type constraints reject invalid input. (3) Keyboard focus and submission carry exact typed values. (4) Cancellation produces an actual cancelled result. (5) Repeated/late submissions cannot answer another request. (6) Unsupported schemas/components refuse explicitly. |
| F12 | (1) Find selects the actual transcript match. (2) Next/previous advances between known positions. (3) Jump selects an exact known turn, not just a picker row. (4) Tool expand/collapse and raw views display the correct content. (5) Timeline/navigation changes the intended location. (6) Links and focus/escape return control to the intended pane. |
| F13 | (1) Theme, spacing, timestamps and timeline visibly change. (2) Saved settings survive restart and actually apply. (3) Vim navigation/edit actions operate on the intended pane. (4) Multiline, fullscreen/minimal and relaunch preserve drafts/session state as specified. (5) Narrow/wide viewport and resize keep controls reachable. (6) Mouse reporting, mouse actions and native terminal input have separate protocol/physical qualification. |
| F14 | (1) Transcript export contains the intended full conversation. (2) Response copy exports exact selected response bytes. (3) Selected block and OS clipboard contain the intended text. (4) Unicode, final newline and platform encoding are preserved. (5) Unwritable paths and missing clipboard service report failure/fallback without false success. |
| F15 | (1) A real supported delegation creates an identified child. (2) Child progress and result are shown under that child. (3) Parent completion waits for/consumes the intended result. (4) Selecting/taking over a child cannot mutate another session. (5) Cancel and failure propagate with correct ownership. (6) CLI/backend-disabled delegation refuses explicitly. |
| F16 | (1) Each of the 42 backend-disabled commands refuses without creating model/session work. (2) Unknown commands have the same no-forwarding boundary. (3) Disabled settings are absent or explicitly unavailable. (4) ACP/provider failures name their actual cause and recover without duplicate effects. (5) Optional services, media and authentication are qualified according to advertised capabilities, not inferred from SDK features. |

The effective-command constant has 31 entries. `/cd` is dashboard-only and the
Code-backed TUI hides Dashboard, leaving 30 ordinary reachable command entries.
Most are pager-local; ACP itself advertises `/goal`. Those are command-entry
counts, not complete feature or acceptance counts. The authoritative sets are
`A3S_CODE_EFFECTIVE_COMMANDS`, `A3S_CODE_UNAVAILABLE_COMMANDS` and
`A3S_CODE_UNAVAILABLE_SETTINGS` in the pager's `app/app_view.rs`.

### Tool and extension coverage within F07, F11 and F15

The baseline tool registry also includes `ls`, `patch`, `download`,
`background`, `git`, `web_fetch`, `web_search`, `update_plan`, `ask_user`,
`ask_user_form`, `batch` and `program`, beyond read/search/write/edit/bash.
Registration is conditional on the owning workspace capability or runtime
service. Code intelligence, semantic/hybrid retrieval, delegation, document
and Office tools, local assets, wiki and configured MCP/plugin tools need
their actual registration/admission checked before assigning a positive TUI
acceptance case. A hidden `/plugins` entry does not prove model-side plugin
tools are impossible; these are different boundaries.

For each admitted family, qualify its actual result and error path, card/raw
output rendering, ownership and cancellation. External network tools require
a controlled live endpoint plus refusal/error evidence. Download additionally
requires exact destination bytes and write authority; background work needs
start/poll/cancel/no-late-effect evidence; git must use an isolated fixture
repository; batch/program must preserve individual receipts and partial
failure semantics. None of these contracts is established by the original
read/write/edit/shell scenarios alone.

Useful owning sources for the audit:

- Startup/default flags: `apps/aster/src/main.rs` and pager `app/cli.rs`.
- Command/settings boundaries: pager `app/app_view.rs` and `slash/commands/`.
- Input/history/navigation: pager `app/agent_view/prompt.rs`,
  `views/prompt_widget/`, `views/history_search.rs`, `app/dispatch/jump.rs`.
- Provider CRUD: pager `views/model_config/document.rs`, `state.rs`, `render.rs`.
- Mode authority: pager `app/dispatch/modes.rs`, ACP
  `agent/permission_state.rs`, `agent/permission_ask.rs`, `host_context.rs`.
- Session identity/lifecycle: ACP `agent/session_catalog.rs`, `session_open.rs`
  and pager `app/dispatch/session/`.
- Tool admission: Core `tools/builtin/mod.rs` and ACP `agent/session_open.rs`.
- Goal/form/child receipt ownership: ACP `goal/`, `form_request.rs`, Core
  `tools/builtin/ask_user_form.rs` and pager child/interaction handlers.

## Initial expanded execution findings

- Windows and native Linux each ran the original four new scenarios with
  `kimi/k3.1-pro`: two passed and two failed. Original logs and case evidence
  are retained in the platform directories. No domain is promoted to passed.
- The 42 disabled commands plus an unknown command were refused with no new
  parent session/model facts on both platforms. A missing file produced one
  real failed read receipt, no retry and no created file on both platforms.
- Windows multiline submission lost LF with the original driver. A native
  ConPTY/crossterm event probe records bare LF as Ctrl+Enter, CR as Enter and
  CRLF as both events. The driver now sends native CR key encoding for logical
  LF while retaining the original exact `user.message` assertion. This is a
  transport correction, not a prompt or output-contract change.
- Linux submitted the exact multiline Unicode prompt and read the real
  Unicode/space filename. Its history check returned early on old scrollback
  text. The check now waits for the same actual-composer assertion; transcript
  keyword presence is still insufficient.
- Plan plus `/yolo` denied a real write on both platforms. The pager explicitly
  retains the Plan ceiling under that permission toggle. The execution test
  now retains that negative check and explicitly uses Shift+Tab before the
  unchanged write/verification prompt. Execution-mode success remains pending.
- A later Windows batch added real provider CRUD and preference-restart
  scenarios: three of six tests passed. Four settings were proven restored
  across process restart by actual inverse toggles and a subsequent real Kimi
  read in a distinct new session. Provider CRUD exposed fixture assumptions
  about generated ids/default names; the fixture now explicitly names its
  provider and edits the complete model name. The original failed run and its
  independent reproduction are retained.
- The terminal parser's logical `contents()` joins wrapped physical rows on
  Windows. The strict composer-row check therefore needs physical terminal
  rows, not substring matching in the flattened transcript. The new driver
  accessor uses the parser's real row API without relaxing the composer
  assertion.
- In Code mode, Shift+Tab from Plan+YOLO selected the UI default session mode
  and emitted ACP `default`, despite retaining the UI permission flag. Code
  uses a unified mode/posture catalog, so that wire value loses YOLO. A small
  real-dispatch regression failed before the correction, naming the actual
  `default` effect, and passed after the correction. Two existing legacy
  mode/policy-pin tests also passed. Both platform live execution acceptance
  remains pending. The mapping only changes the Code backend's wire posture;
  explicit permission denies still belong to Code's existing policy.
- One Linux expanded batch was refused before inference because sources were
  edited between the paired build and Cargo's test companion rebuild. The
  retained guard correctly detected different source identities. Its seven
  failures are harness/build admission failures, not seven model failures or
  product acceptance results. A matching fresh pair is required for rerun.
- The next matching Windows batch passed 6/7 expanded tests, including exact
  multiline/Unicode submission and actual composer history, Plan+YOLO denial,
  Plan exit followed by byte-verified execution, and preference restart. Its
  only failure was the provider-edit driver sending 0x08 (Ctrl+Backspace)
  instead of 0x7f (plain Backspace). A native event probe confirms the mapping;
  that physical-key correction and isolated rerun are in progress. The exact
  saved name and ACL assertions remain unchanged.
- The isolated corrected Windows provider-CRUD test passed, including its
  original exact saved-name assertions and real post-restart Kimi request.
  Linux also passed provider CRUD/restart and both exact multiline-input and
  actual history-recall assertions. Linux then failed while discarding the
  recalled multiline draft before the next command: the ordinary driver
  line-clear barrier is insufficient for that draft. The test now explicitly
  discards its idle unsubmitted draft with Ctrl+C before cleanup. This does
  not change its input or success assertions.

The final expanded rerun builds both binaries and the specific integration
test in one Cargo invocation (`--bins --test aster_tui_live_e2e -p aster
-p a3s-code-acp`) and runs the executable identified by Cargo's structured
compiler-artifact output. This keeps the feature union and source identity
stable instead of alternating separate build/test dependency graphs.
Both platform `combined/` runs are in progress; they cover seven additional
scenarios, not complete F01-F16 acceptance.

The matching Linux pre-combined run still denied the write and bash actions
after the Plan-exit footer condition, while Windows completed that scenario.
The denied receipts are retained in `linux/attempt-3/plan-read-write.json`.
The wire-id regression fix alone therefore does not establish reliable
cross-platform mode-transition acceptance. Confirmed mode/admission ordering
needs further investigation; no extra sleep or relaxed success condition is
used to promote this failure to a pass.

## Second expansion: confirmation ordering and lifecycle

The Linux write refusal reproduced alone on the same matching pre-fix pair.
A hermetic notification-order test then reproduced a second cause: a previous
Plan confirmation cleared a newer pending exit. The handler now retains that
pending exit until a matching confirmation arrives, mirroring the existing
protection for pending Plan entry. The new test failed before and passed
after the correction. The relevant plan-handler packet passed 26/26.

That packet initially had one independently reproduced Windows failure: its
kept-plan fixture used a fixed Unix file URI and expected a native Windows
path conversion. The fixture now builds file URIs from real temporary native
paths and retains the exact path/content assertions; no production URI
conversion was changed to accept an invalid platform path.

`lifecycle` adds four frozen real-provider contracts before their first run:

- Resume and continue must restore the same identity and let Kimi recall an
  unseen earlier token without any tool call, with per-session fact checkpoints.
- An explicit write deny must remain binding under YOLO, and a subsequent
  permitted read must still work.
- Allow-always must persist the exact command rule, survive restart without
  another card, and ask again in another workspace. The test checks actual
  card-answer counts and actual bash invocations, not just the rule file.
- `/copy 1` and `/copy 2` must write exact UTF-8 response bytes; export must
  preserve both turns in order; unwritable directory destinations and invalid
  response indices must refuse without model work.

Both source pairs and the full 21-scenario integration target were rebuilt in
one Cargo invocation per platform, then launched from Cargo's identified test
artifact. `windows/batch-2/` and `linux/batch-2/` retain this run. Completion of
these scenarios still does not establish all F01-F16 contracts.

`git fetch origin main` succeeded during this expansion. The remote remains
at `3185a2174`; no requested remote merge is outstanding at that observation.
Commit/merge/push is deferred until the full requested testing and fixes are
complete. Other dirty work in this shared workspace remains outside this
campaign's changes.

### Batch-2 results and remaining confirmed defects

Both matching source pairs (`1b5b3e3bd374c23f0325bd2e631c24becc0b9214`,
with their platform triples) completed all 21 ignored live tests. Windows:
19 passed, 2 failed, 1719.41 seconds. Native Linux: 19 passed, 2 failed,
1692.04 seconds. These are incomplete acceptance results, not a green suite.

New actual proofs on both platforms include model-history recall after resume
and continue without a tool call, exact Nth-response UTF-8 exports, transcript
order, and explicit destination/index errors without model work. Linux also
proved learned approval across restart and a fresh card in another workspace.
Per-session fact checkpoints are retained for the multi-session scenarios.

Confirmed failures and fixes still requiring full acceptance:

- Both platforms ignored the CLI `--deny Write(*)` in the tested YOLO path;
  the real write succeeded. This is an unresolved authority-propagation
  defect. Do not weaken the test or replace this with a Core-only deny proof.
  The legacy CLI parser accepts `Write` as an `Edit` alias, but sidecar
  initialize metadata does not transmit the parsed permission rules.
- Windows learned-approval execution hit the completion gate after `cat` was
  observed as a file mutation. The before/after byte digest and reported
  attributes are identical. A separate native PowerShell-read observation
  test passes, so ordinary file reading alone does not explain this. The
  complete governed tool/sandbox path still needs diagnosis. Mutation
  observation has not been disabled or relaxed.
- Independent Windows approval-store tests found 4/8 failures: native paths
  could not round-trip through the hand-built ACL. The writer now uses
  `a3s-acl`'s generator; the original tests plus a native/quoted-label case
  pass 9/9 on Windows. Both platform live learned-approval acceptance still
  needs rerunning on the final source pair.
- The earlier Plan-confirmation fix alone did not fix the Linux transition.
  Permission toggling under Plan redundantly issued another asynchronous
  `set_mode(plan)`, which could undo a newer `set_mode(yolo)`. The toggle now
  persists the preference without reissuing the unchanged Plan mode. A new
  wire-request test failed before and passed after this change.
- A fresh matching pair (`9bd49e5ccbb6b131444c4e3d49a67db8ee390f2c`)
  then passed the complete Plan/write scenario on Windows. Linux no longer
  refused the write, but failed the unchanged exact-byte assertion because
  Kimi wrote one extra final LF. The original prompt and byte assertion are
  retained. Read-tool EOF/newline observability and the write/verification
  contract need investigation before promoting this scenario to passed.

The live driver now reports an already-rendered `Turn failed` immediately,
using the same pre-existing failure assertion instead of waiting for a
success-only terminal fact until the unchanged 420-second limit. This changes
failure reporting latency, not model inputs, deadlines or success criteria.

All F01-F16 domains still require the outstanding atomic contracts above.
This campaign has not been committed, merged or pushed as complete.

The original planning positive scenario retains its original read and
write/verification prompts. Read-only refusal and permitted execution now
use separate sessions: inserting a prior denied write into the positive
scenario made the model carry a no-retry denial forward, which does not
isolate the mode-transition contract. Both original failed contexts remain
in evidence. No original model prompt or file-byte/verified-result success
assertion was relaxed.

## 2026-10-29: permission propagation and completion-ledger repair

The CLI `--allow`/`--deny` defect is now fixed in the candidate. Interactive
and headless initialize carry the original compact rules to the sidecar;
the legacy leader configuration remains separate. ACP validates a closed
object with string lists once, rejects unknown fields/empty rules, and trims
outer whitespace before storing the rules. Denies remain binding across
posture/model replacement and research admission. Allows cannot relax Plan,
research admission, or the host source-review ceiling.

The initial typed metadata implementation accepted an empty JSON sequence;
its negative test failed on both platforms. Object admission fixed that.
A later Linux regression proved that `"  Write(*)  "` allowed a real write
without boundary trimming. The unchanged denial assertion now passes after
trimming. These are implementation defects, not changed evaluation prompts.

Evidence before incorporating the subsequent remote-main updates:

- Five initial permission tests passed on each platform, two interactive /
  headless wire tests passed on Windows, and nine existing permission-state
  tests passed on each platform.
- The frozen explicit-deny Kimi scenario passed on both matched binaries,
  source-pair `d7afab3da312ef71565ee8b697d0edc51427a947`: Windows 42.89 s,
  Linux 58.45 s. A real write was denied under YOLO, original bytes remained,
  and a subsequent real Kimi read completed in the same session.
- Logs and exact case facts are under each platform's
  `cli-permissions-fixed/`, `cli-permissions-unit-final.log`,
  `cli-permissions-existing-state.log`, and Windows
  `cli-permissions-wire.log`.

### Windows read-only receipt investigation

An independent actual `BashTool` plus native AppContainer `cat` reproduced
the false completion gate. Content digest, node identity, attributes, length
and last-write time remained equal; NTFS ChangeTime advanced. Comparing the
bounded security descriptors also found inherited-ACE bookkeeping changes.
The diagnostic headers/offsets are retained in
`windows/windows-metadata-diagnosis.log`.

An experiment replacing ChangeTime with a security-metadata digest did not
establish the intended contract (31/33 tests passed; both distinct failures
also reproduced alone). It was withdrawn. Its patch and failed logs remain
under `windows/windows-metadata-experiment.patch` and `windows/windows-metadata-*`.
No Windows observer check, handle anchoring, timestamp consistency check or
sandbox policy was removed; no new Windows security dependency is shipped.

The actual completion defect was in `MutationLedger`: it introduced a new
write record even when a typed receipt proved identical before/after file
identities. The candidate now excludes such new records only for known
identities. Unknown kinds or unread file bytes remain unresolved. Existing
mutation records still reconcile with newer observations; an unchanged read
does not erase earlier writes or their verification debt. Failed invocations
and unknown invocation outcomes retain their respective refusal contracts.

- Two of the first five real-ledger regressions failed before the correction.
- The expanded seven-test ledger contract covers successful/failed no-change
  receipts, background/nested projection, prior writes, created-then-absent
  reconciliation, real byte/mode changes, unknown preimages, unread bytes and
  unknown invocation outcomes.
- Windows `harness_loop::` passed 81/81 after the correction and filename
  fixture repair. The real native sandbox read and five existing Windows
  observer checks passed 6/6. The final known-identity correction also passed
  an isolated native-read rerun.
- The older native-read unit oracle compared raw NTFS metadata equality.
  That oracle was disproved by the recorded ACL behavior. Its replacement
  verifies unchanged actual bytes/mode/node/mtime, a complete real receipt,
  and absence of new completion debt. The original frozen live Kimi prompt,
  result assertions and timeout remain unchanged.

The widened Windows ledger packet also independently reproduced a pre-existing
fixture error: Win32 rejects newline/tab filenames with error 123. A native
Rust probe confirmed this with ordinary and verbatim paths. Unix retains the
original real control/backslash filename cases; Windows explicitly tests
the native refusal while preserving the common real whitespace/Unicode and
typed unknown-path byte assertions. See `windows/native-path-rules.log` and
`windows/typed-paths-isolated.log`.

### Latest-main integration and current qualification

`git fetch origin main` found 20 newer commits, including ACP usage, tool
availability and completion changes. They are incorporated in the isolated
in-repository worktree `.tmp/tui-acceptance-20261029`, based on
`4faf25b927784f03f32777c05c3506228cdcb4cd`. Only the owned TUI changes and
sanitized campaign evidence were transferred; unrelated dirty work in the
original checkout remains outside the candidate. Studio and Power are at
their parent-pinned revisions (`fef6639ee36bd6ab4a24856debaebd22d5ee0587`
and `24b364c6935c1b62a689d91839ec394bfc74c059`).

The first WSL build in the native-created worktree failed during source-pair
enumeration, before inference: its Git pointer contained a Windows absolute
path. The task worktree/submodule forward pointers are now portable relative
paths; both native Git and WSL Git resolve the intended commits. The temporary
unsupported relative-worktree repository extension was removed, and unrelated
worktree pointers were restored. The failed admission log is retained as a
build-environment result, not a Kimi acceptance failure.

On this newer candidate, Linux permission tests pass 6/6, including the
whitespace rule; Windows completion-ledger tests pass 81/81. The first
latest-main pair (`1d66861bcae0e166f6bd7c472f384ec6b14071f4`) passed the
four lifecycle scenarios on Linux (216.80 s), but failed all four on
Windows (79.64 s). An isolated explicit-deny rerun also failed (9.68 s).
The Windows failures were fact-log writer contention, before the intended
workflow could complete. These results remain under `main-critical/` and
`windows/main-log-isolated/`.

ACP's new usage notification requested `durable_transcript()` during the
model worker's write lease. That reader opened a separate FileLog and
contended with the original writer. The correction reads through the
session's existing FileLog handle, retaining the independent-writer fence.
The new hermetic test holds the original writer, requires a foreign reader
to fail, and requires the session reader to return both committed messages.
It failed before the correction and passed on Windows and Linux afterward.

Fresh paired binaries (`f5f8c23749c90ae6d553d1a6ec4cf59c1c8b2f7e`)
then passed all four frozen lifecycle Kimi scenarios on Windows (192.14 s)
and Linux (195.65 s), using the original ACL and `kimi/k3.1-pro`.
See `windows/main-critical-owned-log/run.log` and
`linux/main-critical-owned-log/run.log`. They cover permanent approval and
restart/workspace scope, explicit deny followed by a successful read, exact
Nth-response copy/full export/error refusal, and resume/continue model
history. The exact owned source patch for that pair is retained at
`source/f5f8c237-owned.patch` (SHA-256
`ed5a1311a48aec37cd3e50d737df2520c7bfca8d93d9dd15771cf7e0f67b00f2`).
It applies in reverse to the tested candidate based on the revision above.

These four scenarios do not complete F01–F16 or the 153-checkpoint matrix.
The Linux final-newline failure was subsequently corrected and qualified
below. Other unproved contracts remain open. No completion commit or push
has occurred.

### Broader durable-source and compaction checks

Both distinct Windows failures in the expanded core packet reproduced
alone. The cold-reopen fixture retained closed session/agent objects while
renaming their open log directory; Windows correctly refused with error 32.
Explicitly dropping those old objects before reopening fixed the fixture;
all three durable-source tests then passed, retaining missing-source refusal,
fork, checkpoint and all four policy/goal combinations. The automatic
compaction fixture declared 256 tokens, below the current 12,288-token
harness floor. A valid 16,384-token window and bounded larger history now
reach the actor, but the initial fixture provider's constant 1,024-token
usage did not trigger compaction. The unchanged compaction/source assertions
still failed that attempt. The fixture provider now reports usage estimated
from its actual received messages, instead of the constant 1,024. The
isolated automatic-compaction test then passed on Windows (0.28 s), including
the actual `compaction.done` fact and original source digest in the next
model input. This changes only the hermetic fixture; no live prompt, model,
timeout or output assertion was changed. The broader packet and Linux
confirmation remain pending.

### Exact EOF information and further qualification (2026-10-29)

The frozen Linux Plan-write failure retained one extra final LF. The source
reader used `lines()`, which erased terminators, while the numbered display
always added LF. A no-newline token and the same token ending in LF therefore
gave the model indistinguishable text. Four new real-reader tests reproduced
this loss before correction, both together and separately. They cover actual
LF/CRLF/unterminated files, a final page after pagination, authoritative unsaved
overlay text, and each file's own batch segment.

The candidate retains terminators during bounded local reads and reports the
final line ending explicitly in model-visible output and single-read metadata.
The existing public range shape and read method remain compatible; an additive
optional reader method defaults to unknown, without a whole-file fallback for
existing streaming-only backends. Overlay and manifest readers propagate the
observed source information. No separate stat/reopen guesses the ending, and
partial/unknown ranges do not claim a final newline. Read budgets, access
checks and the frozen live byte comparison remain in place.

- Windows read packet: 24/24 passed, including existing bounded batch
  continuation, long-line byte continuation, missing/escaping paths and images.
- Linux read packet: 25/25 passed (one additional Unix-only existing case).
- Windows compaction packet: 8/8 passed with the repaired hermetic usage fixture.
- Two new live scenarios are frozen before execution: actual `/compact` with
  and without focus must commit summaries and preserve an unseen key for the
  next Kimi request without tools; a bracketed Unicode paste above 16 KiB must
  reach exactly one byte-identical `user.message` and a settled response.
- Pair `fb465994a5277337843bd4580b6d153eccd61f8a` passed the unchanged
  Plan-read/exit/verified-write scenario on Windows (73.91 s) and Linux
  (54.63 s), including exact target bytes with no added final LF.
- The same pair passed actual compaction with and without focus on both
  platforms. Each summary retained the unseen key, and each next Kimi request
  recalled it without tools. Linux passed both new scenarios, including the
  exact long paste, in 89.28 s. See `read-endings-qualified/`.
- Windows long-paste acceptance remains failed. The first run stalled before
  a settled turn and was terminated with its recorded process identity and
  CPU time. An unchanged isolated run also exceeded a 180 s process deadline.
  A 1,024-byte chunked-write experiment did not fix it and was withdrawn.
  One diagnostic run exited the TUI after 12.99 s; other diagnostic runs
  stalled despite completing all 23,775 input bytes, the paste terminator and
  submit key. Their source-pair IDs, stderr, process records and watchdog
  outcomes are retained under `windows/long-paste-*`. These are failures,
  not passes or repeated sampling for a favorable model answer. The model
  did not receive the intended settled turn. Temporary input routing traces
  are being used to locate the actual blocker and will be removed.

The broader Linux Agent API packet initially returned 312 passed/11 failed.
All eleven failures reproduced individually under `linux/expanded-isolated/`.
Six long-horizon/checker cases subsequently passed alone with the installed
native Node directory and `NVM_DIR` explicitly available. A symlink-only PATH
had left its NVM target outside the sandbox mount set; this was an environment
issue, not authorization to weaken the sandbox. One follow-up isolation loop
was interrupted by Cargo replacing its test executable; the remaining cases
still require results from the stable current binary. Inline WSL shell status
wrapping also proved unreliable; current campaigns invoke retained LF scripts
with `wsl --exec bash` and preserve the actual test exit status.

Windows long-horizon checks passed 17/18; the remaining plan-heavy case also
failed alone. Its scripted reader supplied the obsolete `path` field rather
than schema-admitted `file_path`, so six invalid reads stopped the intended
choreography before its edit. Only that fixture argument was corrected; actual
edit, suite execution, receipts and verified completion assertions remain.

Both stream failures also reproduced on Windows. The retained diagnostic
shows ToolStart and ToolEnd use the same host-generated invocation ID and
foreground child endings precede the parent ToolEnd. Host confirmation
intentionally replaces reusable provider IDs, so the fixtures' literal
provider-ID filters never observed the real endings. The fixtures now bind
the actual announced ID, require its matching successful ending, and retain
the ordering/count checks. The checkpoint assertion was strengthened to
require that exact host ID in the stored messages, as well as the file path;
the file path alone was already present in the user prompt. The diagnostic
print statements were removed, and their patch/log retained. The full current
Windows Agent API packet is running; Linux confirmation is pending. No frozen
live grader, timeout, prompt or model was changed for these fixture repairs.

### Current full Agent API packets and latest remote integration

The repaired stream packet passes 15/15 on Windows, including the stronger
checkpoint assertion. A second cold-source fixture held its original FileLog
and session while renaming the source directory; its missing-source proof
now closes/drops those objects before removal and reopens the same log after
restoration. The isolated Windows test passed, retaining busy, missing,
changed, omitted and unsettled-source refusal checks.

The plan-heavy diagnostic then identified the actual refusal:
`run.tool_refusal_cycle` after nine provider calls. Its script repeatedly
submitted the identical plan and read the same file six times. The duplicate
guard remains intact. The fixture now makes real progress through six
distinct source files and updates those six plan steps accordingly. It also
requires each plan/read receipt to succeed and each read to contain its own
file's actual content. The actual edit, unchanged checker, complete effect
receipts and verified completion remain required. The isolated Windows test
passed (8.93 s).

- Windows current Agent API packet: **311 passed / 0 failed**, 209.73 s;
  `windows/agent-api-fixture-final.log`.
- Linux current Agent API packet: **323 passed / 0 failed**, 124.13 s;
  `linux/agent-api-fixture-final.log`. This run uses the native Linux `/tmp`
  for temporary fixtures and the installed native Node/NVM toolchain.
- The preceding Linux run in the NTFS-backed temporary directory passed
  321/323 in 545.82 s. Besides the then-unrepaired plan fixture, one goal
  test refused an automatic host-clock rollback. That clock failure did not
  reproduce alone; the full current packet also passed. No clock guard was
  relaxed. The earlier failure and isolation logs remain retained.
- An earlier Windows verified-check case failed while Cargo was replacing a
  PATH executable whose sandbox ACL lease was being restored. It passed
  alone afterward and in the final full packet. Builds and sandbox test
  execution are now sequenced per platform.
- Linux line-ending guards pass 6/6 after the EOF work. The most recent
  whole-workspace fmt attempt found only the new durable-reader module's
  ordering; that ordering is corrected. A final whole-workspace fmt check
  after diagnostics are removed remains required.

`git fetch origin main` at 06:58 found eight newer web/documentation commits.
Their diff contains no TUI/Core/ACP source, Rust workspace or lockfile changes.
They were fast-forwarded into the isolated candidate, now based on
`d0a1ecbd5900ee2275ccf6809499292707a794ce`. The original dirty checkout
remains preserved. No completion commit or push has occurred. Neither these
unit packets nor the 23 currently defined live scenarios complete the
153-checkpoint campaign.

### Native Windows paste: event batching, submission and Unicode (2026-10-29)

The unchanged 23,775-byte long-paste case exposed three distinct native
Windows failures. Their failed runs remain in the evidence tree.

1. Native Console Space key releases were treated as voice-chord boundaries
   without an owned voice hold. This split a long paste into thousands of
   individual edits. The real coalescer regression produced 26,111 events
   before the fix. It now preserves voice releases only for an owned hold or
   an actual incoming voice press. Eight focused Windows cases pass, along
   with 105 existing event-loop cases; Linux passes its 107-case packet.
2. After batching was fixed, the driver's final Enter could join the last
   native input batch. A Windows End-key fence now separates the completed
   non-command paste from submission. The payload and frozen byte assertion
   are unchanged. Linux passed the exact case in 15.30 s on pair
   `503af41b9d9f58b0f61096c38251e757a10bdcc6`; Windows then reached the real
   model but failed the byte grade in 31.38 s.
3. That byte failure was precise: expected 23,775 bytes, actual 22,751. All
   512 U+0301 combining accents were absent. An independent native Console
   probe, through the same PTY implementation, observed each committed mark
   as a nonzero UTF-16 character on `VK_MENU` key-up. Crossterm 0.28.1 marked
   this committed text as `Release`, which the application correctly ignores
   for ordinary released keys.

The local Crossterm patch changes only the committed Alt-code scalar branch
to emit a text `Press`. A real Windows INPUT_RECORD regression fails before
the fix (3 passed / 1 failed) and passes after it (4/4). The packet also
preserves ordinary Alt releases, ordinary character releases and committed
surrogate-pair behavior. See `windows/crossterm-alt-code-before.log`,
`windows/crossterm-windows-parse-after.log`,
`windows/native-console-key-records-cpr.log`, and both platform
`native-paste-fence-qualified/` directories. The initial probe missed the
terminal cursor-position response and timed out; that counterexample is
retained separately.

The published 0.28.1 package is internalized under
`crates/third-party/crossterm`, retaining its MIT license, package identity,
API, examples and notices. Root workspace membership and a local patch use
the one root lockfile. No existing registry package versions were upgraded;
four added registry packages are upstream test dependencies. The paired CLI
stamp covers this local parser. Fresh paired Windows/Linux qualification is
in progress; a passing parser regression alone does not establish exact
end-to-end paste fidelity or native IME support.

### Complete 23-scenario regression after native-input repairs

Pair `bd25365589634ad2ceecc80e0aea4ef1d8bf3a99` passes the complete,
unchanged 23-scenario live packet on both native platforms:

| Platform | Result | Elapsed | Evidence |
| --- | --- | --- | --- |
| Native Windows / ConPTY | 23 passed / 0 failed | 958.03 s | `windows/full-23-native-unicode/` |
| Native GNU/Linux / WSL PTY | 23 passed / 0 failed | 1170.91 s | `linux/full-23-native-unicode/` |

Both use the original operator ACL and exact `kimi/k3.1-pro` model.
Each pair is copied to a unique immutable binary directory, with checked
build identities and SHA-256 values retained in `pair.json`. This prevents
another build in the shared Cargo target from replacing a running test's
companion. The individual long-paste reruns also pass on this pair: Windows
21.99 s, Linux 20.71 s. All 23,775 bytes, including 512 decomposed U+0301
marks, reach one actual user message unchanged.

This packet closes the previously retained failures in the defined scenarios:
exact Plan-to-execution bytes, explicit CLI denial under YOLO, learned
permission restart/workspace scope, durable replay and native long paste.
It does not complete the 153-checkpoint feature inventory.

### Draft, editor, help and reading controls: further expansion

Six more live entry points bring the defined suite to 29 scenarios. The
actual contracts cover:

- Word and line deletion, cursor movement, undo/redo, stash/pop, clear and
  undo of clear. No request is submitted while editing; the final Unicode
  draft must match one actual `user.message` byte-for-byte.
- Real external-editor save, nonzero exit, invalid UTF-8 and missing
  executable. Each returns control; the explicitly submitted saved/original
  draft must be exact and Kimi must complete the subsequent request.
- Actual shortcut detail pages for paste, history and sessions, rather than
  only filtered row visibility. Three isolated Windows failures confirmed
  that Code advertised image input, `/history`, `/resume` and Dashboard
  capabilities it does not offer. Code-specific help now describes text
  input and reachable history/session controls. The public help-building
  API and other backends' existing descriptions are retained. Four new
  hermetic cases plus the existing help packet pass 73/73 on each platform.
- Three independently positioned matches in real tool output, next,
  previous, arrows, wraparound and no matches; selection of the first real
  turn; previewing a second turn and cancelling back to the first; actual
  Vim top/bottom navigation and content reachability at 58x16, 90x26 and
  120x40. Local navigation must not append backend facts or change source
  fixture bytes.

Pair `25f6f0f867358b34bec5ced1cc9b7f3153b07c88` passes Windows
`input-controls-qualified/` 5/5 (60.41 s) and
`reading-controls-qualified/` 1/1 (36.10 s). Linux qualification of these
six entry points is now recorded: reading 1/1 (41.47 s), three help details
and draft controls pass, and the editor readiness correction passes all four
editor branches on the same `25f6f0f...` production pair (39.81 s). The initial
Linux editor failure and its isolated reproduction are retained; the new
session had not yet bound when `/session-info` first ran. The driver waits
within the original readiness deadline for an actual UUID and retains all
exact draft and file-byte assertions. This was not a complete 29-scenario
batch on one final source revision.

Two harness counterexamples are retained. The first draft readiness check
searched a rendered line for the full decomposed Unicode string; the screen
representation did not preserve the following space in that comparison.
It failed again alone. Readiness now waits for an ASCII prefix, while the
original final byte assertion remains unchanged and passes. The first help
probe selected a category header instead of entering a detail page; its
apparent green results are discarded as capability evidence. The corrected
probe requires the real detail body, fails all three cases, and each failure
was reproduced alone before the product fix. See `input-controls-before/`,
`input-controls-isolated/`, `input-controls-readiness-fixed/`, and the three
`*-help-isolated/` directories.

The source patch for `25f6f0f867358b34bec5ced1cc9b7f3153b07c88` is
`source/25f6f0f867358b34bec5ced1cc9b7f3153b07c88-owned.patch` (666,083 bytes,
SHA-256 `c8ba34a5986fc401fc09953fdb46a214d29616056b6c05105803ad4f1c70e14c`).
It includes the local vendor and new sibling tests, and passes a reverse
apply check against base `d0a1ecbd5900ee2275ccf6809499292707a794ce`.

The imported archive contained CRLF in 63 text files. These new local source
and documentation files are normalized to UTF-8/LF; the two PNG assets retain
their original bytes. See `source/crossterm-line-endings.json`. Line-ending
guards pass 6/6 with Linux Git (9.853 s), and 6/6 with native Windows Git
through WSL Python (27.385 s). The workspace dependency packet passes 6/6
after local Crossterm membership (128.158 s). The whole-workspace fmt check
also passed before the subsequent help/reading additions; a final check of
those additions remains required.

At 08:48, another remote fetch found `2505350ce` and `a0e64b702`: product
release-channel isolation, shared UI updates and a newer Studio submodule.
The CLI stable-channel URL changes in Rust. After that paired build finished,
the candidate fast-forwarded to `a0e64b702bac3095fece03b5be723b432632eb3d`
and initialized its pinned Studio commit
`26edd31e3ccc3d045dd4711c7c9c92788848eb85`. The parent pins that commit;
the private Studio source is not copied into the parent. Latest-main checks
pass: workspace dependencies 6/6 (132.163 s), release governance 8/8
(0.261 s), and CLI-pair delivery 10/10 (0.696 s).

## 2026-10-29: queue, direct-command contract and ACP audit

The next retained source pair is
`d0bfd7fb9c9c76dc440fae2a9a688e96199a9d14`, built separately for native
Windows and Linux. Its owned patch is 703,258 bytes, SHA-256
`ad44893880c48d68e858c14fd2cc44275d4f6a9500463e3fd7d616c83c182b19`,
based on `a0e64b702...`; the source snapshot passes reverse application.
`stream-controls-qualified/` passes 2/2 on Windows (63.90 s) and Linux
(92.18 s). The suite now has 31 entry points. A complete 31-entry run of this
same immutable pair is in progress on each platform; the older 23-entry
batch is not represented as a final 31-entry result.

That complete run has now finished: `full-31-current-pair/` passes Windows
31/31 (1,288.90 s) and Linux 31/31 (1,529.26 s). Each platform has 36 case
reports because several entry points exercise separate fixture branches.
Both complete runs use the same `d0bfd7fb...` source identity with their
respective native target. The later send-now and exit-code fixes still
require their own final qualification; this result is not transferred to
those newer source bytes. The checkpoint index now references this complete
31-entry phase rather than mixing earlier partial batches.

The queue scenario starts a real delayed native command, enqueues three
follow-ups, deletes the last and edits the second. Exact durable messages,
three independently completed replies, one actual shell call and unchanged
source bytes prove ordered, once-only execution. The initial probe expected
the word “queued”; the actual numbered row was the correct readiness signal.
That failure was reproduced alone before correcting readiness. The frozen
prompts, order and final business assertions were preserved.

The `!` direct-command surface had a real contract bug: ACP flattened
`bash_command` metadata into a model prompt. Both platforms reproduced it,
including an independent Windows run. Code now explicitly refuses this
unsupported direct-command surface before inference or effects and keeps
the draft. ACP rejects the recognized metadata key even when its value is
null or malformed. Code help omits that shortcut; other backends keep their
existing behavior. This is an explicitly unavailable capability, not usable
direct Shell qualification. The normal model-authorized `bash` tool remains
tested separately. Hermetic checks pass on both platforms: refusal dispatch
5/5, help 74/74, and the two other-backend dispatch cases 1/1 each.

The full ACP library audit initially reported Windows 341 passed / 13 failed /
8 ignored and Linux 382 passed / 7 failed / 8 ignored. Each distinct failing
test was re-run alone, and every failure reproduced. Retained logs distinguish
these issues and corrections:

- Tool-history fixtures expected bare JSON after Core had adopted the exact
  `Tool exit code: 0` envelope. Test-only decoders now require that successful
  prefix and preserve all document, plugin, ownership and Office field
  assertions. Pre-execution schema refusals instead require the actual error
  flag, no success header and no accepted JSON. The first over-broad decoder
  failed again alone and that failure is retained.
- The pinned Studio delivery contract is 4,092 characters; with 8,000 user
  characters and two separators it needs 12,094. The old 12,000-character
  host bound clipped 94 characters. The fixed bound is 12 Ki characters
  (12,288), still enforced for Unicode, NUL trimming and oversized input.
  The contract contains no CR characters; this defect was not an EOL issue.
- Windows executable fixtures now compile actual PE sentinels with pinned
  Rust instead of attempting to execute Unix scripts. Read-only argument and
  no-side-effect assertions remain binding.
- A real Windows path defect appended `/file` to a verbatim `\\?\C:\...`
  root. Typed local workspace services now use `Path::join`; virtual workspace
  display labels retain their declared contract. Two sibling tests verify
  actual Unicode/space-path file access and virtual labels. Windows Office
  proof, nested PNG and real verified-completion cases now pass without
  relaxing proof admission.
- Windows refuses renaming a directory with open fact-log handles. The
  platform fixture now verifies the actual kernel refusal, unchanged disk
  identity and a successful legitimate observation. Unix still exercises a
  successful replacement followed by stale-observation refusal. This does
  not claim the Unix replacement operation succeeded on Windows.
- The run-observation fixture accepts the known `x.ai/turn/outcome`
  notification while continuing to enforce the RunObserve schema. The
  intentionally panicking worker test remains intentional.

After correction the whole ACP library passes Windows **355/355**, with
8 ignored (85.82 s), and Linux **390/390**, with 8 ignored (118.33 s).
Ignored cases are not counted as passes. The native path packet passes 2/2
on each platform, and read regressions pass Windows 24/24 and Linux 25/25.
`cargo fmt --all -- --check` passes under WSL after these source changes.

`CHECKPOINTS.json` retains exactly 16 domains and 153 original checkpoints.
Its generator validates successful evidence, immutable file hashes, model,
source-pair identity and unchanged operator ACL. Status remains
`complete: false`; 72 successful evidence files are linked at this stage.
Partial contracts and unavailable capabilities are kept distinct. The index
does not turn a scenario count into a feature-completion percentage.

## Checks completed during this expansion

- Native Windows regression: 1 new real-dispatch test reproduced the defect
  before the fix and passed after it; 2 existing mode/policy tests passed.
- `python3 tests/line_endings.py`: 6/6 using Linux Git, and 6/6 using native
  Windows Git through WSL (`ASTER_TEST_GIT`). Windows Python is not installed;
  this does not claim a native Windows Python run.
- `python3 tests/workspace_dependencies.py`: 6/6 under WSL.
- Focused formatting for `aster`, `a3s-code-acp` and `a3s-code-core` passed;
  the edited pager files also passed direct rustfmt checks. The latest-main
  isolated candidate passed `cargo fmt --all -- --check` under WSL, before
  the subsequent durable-reader and fixture edits. Native Windows whole-
  workspace formatting hit OS error 206 (command line too long); focused
  formatting passed there. Earlier checks in the unrelated dirty original
  checkout failed on existing Gateway formatting. Final candidate formatting
  after all source changes is still required.
- The first 34 retained evidence files passed an exact scan against resolved
  provider API keys, URLs and header values loaded by `CodeConfig`. New
  evidence still needs the same final scan.

## Explicitly not claimed

- Completion of this campaign or of any whole feature domain based only on
  the first additional batch.
- Native IME/physical mouse qualification from injected PTY bytes, or a
  functioning optional service based only on a hidden/refused entry.
- Clean workspace/CI, release qualification, macOS acceptance, or changed
  operator credentials/configuration.

## 2026-10-29: native cancellation and terminal ownership

The candidate fast-forwarded again to
`78b78a695769b09a27ce3791fdcdef8c06a408a1`. Its eight new upstream commits
change backend CI fixtures, mount handling, a retrieval benchmark and the
pinned Studio source. Studio is initialized at the new parent pin
`4b4c5d3a025c0c63d89121f965c6626b3d22a6fd`. The unrelated dirty original
checkout and its stash remain untouched.

The first three tool-contract probes used pair
`8f6a422022891e74e19e139f5508fec5768a98f1`. Windows 0/3 and Linux 0/3 failed;
every failing case was rerun alone on each platform. Two initial probe
contracts needed correction, while the results also exposed product bugs:

- The search tool explicitly permits non-default pagination only for
  `files_with_matches` and `count`. The original content/context-plus-limit
  request is retained and now separately requires that exact refusal. New
  legal requests test content/context, bounded file pagination and no matches.
  The tests retain one-call identities, unknown fixture tokens, scope and
  unchanged bytes. The invalid request is not claimed as successful search.
- Windows native sandbox cwd is a temporary DOS drive root mapped to the
  workspace. The relative command executes the unique workspace script and
  returns its unknown stdout/stderr tokens. The corrected cwd oracle uses
  typed Windows path components for the mapped root; Linux still compares
  canonical paths. The command, script, original model prompt, nonzero status
  and exact script-byte assertions are preserved.
- A real shell exit 7 survived in metadata but both `ToolOutput` conversion
  paths changed it to 1. A shared conversion now preserves a bounded native
  exit code consistent with success/failure. Invalid or contradictory metadata
  retains the existing 0/1 fallback. Two regression tests, including a real
  native child process, failed before correction and were each rerun alone.
- ACP ignored `sendNow` and queued it behind the running turn. Its recognized
  boolean metadata now calls the existing cancellation path before waiting
  for the original turn permit. Malformed values are refused before work.

Pair `0a5338d573aa5e6c6d6ff84db276ccd3ae95117b` passes two tool probes but
fails cancellation recovery: Windows 2 passed / 1 failed (700.83 s), Linux
2 passed / 1 failed (650.86 s). The original 420-second live turn deadline
is unchanged. `tool-contracts-qualified/` is a historical phase name, not a
claim that its failed scenario qualified. Both successful scenarios retain
real `ls` ordering and limits, legal search context/pagination/no matches,
actual shell exit 7, both streams, native cwd and same-session recovery.
The ACP library at this intermediate stage passes Windows 357/357, 8 ignored
(101.36 s), Linux 392/392, 8 ignored (120.21 s).

The native send-now counterexample contains two separate defects. First,
ACP cancellation forcibly aborted the original fact worker after a short
drain. Diagnostics prove that the native command stopped but the original
scope had no returned close proof; Core correctly retained its admission.
Increasing cooperative grace to five seconds still failed and was reverted.
The correction removes that forced abort from this host path. The original
worker performs its existing capability cleanup; its original public join
continues to hold the serial turn permit. No owner or release check is waived.

Second, after natural cleanup, a structured `ToolEffectUnresolved` event was
recorded while the RunStore still reported Executing. That event is an error
terminal just like ordinary Error, but was missing from the snapshot reducer.
A new sibling regression reproduced Created instead of Failed; the late-event
case already passed. The correction marks this exact typed event Failed,
retains its obligation IDs and respects the existing terminal-state guard.
This allows the original error publication and release proof to complete;
it does not clear workspace or invocation debt.

The initial send-now fixture wrote `started.txt` before cancellation, then
required a no-tool follow-up to complete successfully. That expectation
conflicts with the existing verified-completion gate. The original prompts,
failed reports and frozen source additions are retained under
`source/8f6a422022891e74e19e139f5508fec5768a98f1-live-additions/`. The revised
contract preserves exact message bytes, one original invocation, actual
cancellation, no late write and the real `started.txt` bytes, and requires an
explicit `tool_effect_unresolved` refusal without a false completed turn.
This is a refusal contract, not successful artifact completion. A separate
new live scenario cancels an actual partial Kimi model response with no file
side effects and requires the next exact message to complete once in the
same session. Neither failure is hidden by substituting that success case.

The native hermetic fixture uses a real sandboxed process and a controlled
streaming client. It verifies both cancelled and failed terminal states,
the typed refusal, actual idle admission, unchanged obligations and no late
write. Its initial store directory was inside its workspace, which added
store files to mutation observation; it now uses the same sibling
home/workspace separation as the live driver. Initial compile errors and
subsequent red experiments are retained. An early TurnQueue fixture compile
log was overwritten by its compile-corrected red run; the original error was
`Arc<TurnQueue>` has no `lock_owned`, corrected to its real `acquire_owned`
API. No zero-test or compile-failure invocation is counted as a passing test.
All temporary owner/cancellation diagnostics were removed from source.

Code shortcut detail now describes send-now cancellation and resource release,
with the existing permission and verification rules. Other backends keep
existing help. The sessions-help live entry also opens the actual send-now
detail. Case reports and the 35 entry points are not equated with coverage.

The current paired build and full ACP/native regression pipelines are in
progress. Final results, source identities, source snapshot, secret scan and
landing remain required. The latest completed source-only UTF-8/LF audit
covered 146 owned files without CR or BOM; later additions need its final
rerun. No byte-identified evidence or fixture is normalized.

The additional CI guard packet passes line endings 6/6 (10.556 s), workspace
dependencies 6/6 (171.197 s), Studio checkout ownership, CLI pair delivery,
release governance, Linux installer, Kylin input-method launcher and progress
benchmark checks. Musl qualification initially failed because the pinned
AArch64 target was not installed, failed again alone, and passes 2/2 after
installing the explicit pinned musl targets. Native Windows local Crossterm
passes 69 tests, with 10 ignored. These do not claim an entire workspace CI
or physical IME/mouse run.

The terminal-state regression first expected an obligation ID in the display
string, then expected only the bare error code. Both assertions were incorrect:
the existing `Display` contract contains the code, obligation count and
reconciliation guidance; IDs remain in the structured event. Those red logs
are retained. The current test requires Failed, the defined code/count and
guidance, no result text, one event with the exact original IDs, and monotonic
Cancelled state for a late typed error. Windows passes 2/2 on this contract.

Pair `07cc0ea17bde1d8ce19f6b562362ad721ce9cdd3` preserves the raw next user
message after cancellation. Previously, owed-work guidance was appended to
that user message. The guidance now belongs to the captured per-turn system
prompt, retaining existing host guidance, sorted JSON-escaped workspace paths
and the original completion obligations. A native hermetic fixture captures
the next actual model input and verifies this separation. Another fixture
executes the same real failed script as the live probe and captures the exact
exit-7 header and independent unknown stdout/stderr tokens in its ToolResult.
These controlled clients diagnose the model interface; they are not counted
as Kimi end-to-end observations.

On this pair, Windows `owed-guidance-extended-tools/` passes 3 scenarios and
fails the model-stream cancellation scenario (348.24 s). Real scoped ls/search,
native failed shell with both streams/cwd and next-request recovery, and native
tool cancellation with exact next user bytes and explicit unresolved-effect
refusal pass. The failed model scenario is rerun alone on both platforms:
Windows 0/1 (19.10 s), Linux 0/1 (17.22 s), under
`owed-model-cancel-isolated/`. Linux's earlier two-case cancellation batch
passes the native tool case and fails the model case (45.75 s).

The model cancellation failure is not fixed by preloading the composer. The
original model is still running when the shortcut is sent, but Core appends
one narrative `turn.complete` for its synthetic interrupted response and
another for the actual next response. The existing sealed publication
correctly records the original Run Cancelled and suppresses its successful
End. Durable completion bookkeeping must follow that same cancellation
decision. A new focused model-stream regression is being established before
the correction; the original prompts, deadline and exactly-one-completion
live assertion remain unchanged.

At this stage, Windows full ACP passes 359 tests with 8 ignored (113.68 s),
Windows Core `agent_api::tests::` passes 311 (295.57 s), and Linux the same
Core filter passes 323 (162.40 s). The latest source-only UTF-8/LF audit covers
153 owned files with no CR or BOM. The phase's retained owned patch has
749674 bytes, SHA-256
`6c5f9371cb4fbf0671d3d270b793ecaccacdcf50ebd12b5bf5a192ad374c69ea`,
based on `78b78a695769b09a27ce3791fdcdef8c06a408a1`, with a successful reverse
check. These are phase results; later source changes require new identities.

The candidate subsequently fast-forwarded to
`5629b3009ba757cb74a4b4463168ed480eca5553`, initializing Studio at
`7beaeed73b592c9d3700c5f90594561b9543cc68`. The two upstream commits change
shared-source CI, submodule pins and release packaging, without overlap with
the owned fixes. The original dirty checkout remains untouched.

The new partial-model regression fails on both platforms before correction:
Windows 0/1 (0.18 s), Linux 0/1 (0.69 s). The owned completion gate now leaves
an already cancelled result for its original sealed publisher, without
appending an accountable completion fact. The original publisher still
records Cancelled and suppresses End; interrupted history remains retained.
Existing execution debt is not cleared. The complete typed-interruption
group passes 13/13 on Windows (3.82 s) and Linux (3.30 s), including late token
cancellation after sealed success and preservation of real cleanup errors.

Pair `5862dded8faeab96d4be04ae9ae535e3c474973e` passes both original native
cancellation probes on Windows (42.34 s) and Linux (48.14 s), under
`completion-facts-cancel/`. Its owned patch is 755858 bytes, SHA-256
`46aa066850095b016b39335a08a50b86d2341d132249dd0a2419c4c0c18352dc`,
with a successful reverse check against the new base. Full 35-entry live
batches on this pair remain in progress; an added send-now help subcase has
already failed on both platforms. Independent Windows repetition reproduces
it (11.59 s). The case searched lowercase `send now`, then required that exact
case in a correctly displayed title `Send now`. Its query now uses the
literal title case; the detail-body and forbidden-help assertions remain.
This is a fixture correction and requires a new paired source identity.

Broader, previously unrun library suites expose additional failures and must
not be reported as clean. At this stage, full ACP passes Windows 359/359,
8 ignored (120.86 s), Linux 394/394, 8 ignored (141.64 s). The earlier Linux
ACP batch had one native-plugin cutover timeout (393 passed, 1 failed,
8 ignored, 158.14 s); independent repetition passes 1/1 (2.02 s), and the
subsequent full batch passes without changing its ten-second deadline.

Core conversation-runtime passes Linux 86/86 (17.78 s); Windows passes 79
and fails 7 (23.17 s). Each of the seven is independently repeated under
`conversation-isolated-5862/` and fails at the same fixture comparison:
the API's normalized `C:\...` workspace versus the standard library's
verbatim `\\?\C:\...` canonical spelling. The fixture now canonicalizes
both actual paths before comparison. It still requires the same real
filesystem identity; no production workspace trust check changes.

Full pager library results are Windows 10017 passed / 165 failed (140.18 s)
and Linux 10210 passed / 19 failed / 4 ignored (258.76 s). Every failed test
was rerun alone: all 165 and all 19 reproduce. The Windows explicit modern
terminal diagnostic (`GROK_FORCE_LEGACY_CONSOLE=0`) repeats the same 165
oracles: 53 pass, 112 fail. This diagnostic is not substituted for the
retained default legacy-console failures. An untouched same-repository
worktree at the exact upstream base is being compiled for attribution.

One owned pager failure is an outdated permission-ack fixture: under Plan,
the corrected production path intentionally sends no redundant Plan toggle.
The fixture now requests an actual YOLO transition, refuses its existing
`session/set_mode` acknowledgment and still requires PermissionModeFailed.
The separate Plan no-repeat regression remains. Other broad pager failures
are not yet resolved or waived.

The final static guard packet passes source line endings, internal workspace
dependency boundaries, Studio checkout ownership, paired CLI delivery,
release governance, Linux installer, Kylin launcher, progress benchmark and
musl linker hardening. Workspace formatting also passes. No whole-workspace
CI or whole-feature completion is inferred from those checks.

The completed pair-5862 live batches are Windows 34 passed / 1 failed
(1417.04 s), Linux 33 passed / 2 failed (1605.81 s). Windows fails only the
case-sensitive help fixture described above. Linux additionally fails the
unchanged shell final-answer quote oracle. That shell case passes independent
repetition (36.25 s), without changing its prompt, script, model or grader.
This is retained model-answer variability, not a stable-success claim.

The corrected native-path conversation suite passes Windows 86/86 (33.60 s)
and Linux 86/86 (19.38 s). The actual YOLO acknowledgment-refusal test passes
1/1 on each platform. The resulting pair
`32b8c01e8224e33564c3f38d03c84dffcd465d95` passes the actual sessions/send-now
help entry on Windows (12.03 s) and Linux (15.40 s). Its new full 35-entry
batches are in progress under `full-35-qualification-final/`.

The untouched upstream Windows worktree at `5629b3009` passes 9997 tests and
fails 165 (150.41 s); all 165 fail again independently. Its working tree is
clean. Exact comparison with the candidate finds 164 shared failures, one
upstream-only Plan-kept failure, and one candidate-only permission-ack fixture
failure that is now corrected. `LIBRARY-FAILURES.json` binds all case names,
independent logs and their SHA-256 digests. It does not infer Linux baseline
execution from matching names and does not claim the pager suite is clean.

The final source-only LF audit covers 159 files with no CR or BOM. The staged
line-ending guard passes 6/6 (9.984 s). Four retained upstream Crossterm
Markdown files keep their original spacing, including hard line breaks;
path-specific whitespace attributes preserve that layout while the global
source LF rule remains. Raw terminal logs and unified-patch context have
equally bounded evidence exceptions. No Rust source whitespace rule is waived.
The supplementary landing snapshot, including these attributes, contains
759507 bytes with SHA-256
`e61d27227d7f3c72ef5abb854847719a94f5ae71caf8f2d00c4af17fd4edcb10`
and passes reverse checking. The CLI source identity remains `32b8c01e...`.

Related CI filters pass memory-retention 14/14 on Windows (12.61 s) and Linux
(6.73 s). The combined command also names
`sandbox::native::lifetime_tests`, but that module is macOS-only and contributes
zero tests here; it is not counted as Windows/Linux lifecycle qualification.
Native process cancellation is evidenced by the real native/live tests above.
The v912 seam passes 7 with 1 ignored on Windows (12.80 s) and Linux (16.60 s).
ACP conformance stdio is Unix-only: its zero-test Windows invocation is
excluded; native Linux passes 10/10 (9.18 s). These protocol/SDK fixtures use a scripted model, not Kimi, and do
not establish TUI support for SDK-only attachment surfaces.

The final pair-32b8 batch completes Windows 35/35 (1417.58 s), Linux 34 passed
and 1 failed (1645.04 s). Both native cancellation cases, failed-shell streams
and recovery, and corrected help cases pass on each platform. Linux fails
`kimi_draft_edits_undo_redo_stash_and_clear_preserve_exact_text` at its first
negative screen assertion: `WRONG` remains visible 200 ms after one Ctrl+W.
The same exact case passes independently (13.43 s) with one exact Kimi request,
all edit/undo/stash checks and no tools. This does not replace the failed
batch result. A follow-up will replace fixed-delay negative checks with a
bounded observable-state wait, retaining the original input, expected text,
single-key operations and turn deadline. No such harness correction is
included in the pair-32b8 product source or its reported full batch.

The prelanding exact scan passes 1534 evidence files against the resolved
original provider API keys, base URLs and header values. The scanner accepts
explicit original-ACL and evidence paths, so operator configuration is never
copied into a fixture or the original dirty checkout. The final exact scan
after reconciliation passes 1549 files. Source and evidence are staged only
from the owned manifest and this campaign directory; the original dirty
checkout, credentials, prior stash and private submodule source stay separate.

The final precommit snapshot includes the single historical experiment-patch
spacing exception: 759661 bytes, SHA-256
`9775085742f3a1cd05da5af15a1b17fc67f2262c56b34c4127c0a67f02167d6a`,
reverse check passed. Its 159-file UTF-8/LF audit passes again. The CLI source
digest remains `32b8c01e...`; no product source changes follow the live run.

## Native startup and observable input follow-up (2026-10-29)

The draft-control driver now waits at most eight seconds for the observable
negative screen condition after one original key operation. It does not send
an extra key or change the expected raw user message, model or turn deadline.
The first full-paragraph Unicode display matcher was invalid on Windows:
physical-screen reconstruction did not retain the space next to a combining
character. That pilot failure is retained. The corrected matcher uses the
original prefix and an explicitly supplied visible tail marker; final raw
UTF-8 message equality remains unchanged. The resulting pair-43ce full batch
passes draft editing on both platforms, and the final pair passes its focused
native draft case on Windows (21.02 s) and Linux (14.24 s).

Pair-43ce's Windows search case fails a count assertion in the full batch and
passes independently (166.83 s). Linux explicit denial does not reach the
required write preparation in its batch and passes independently (95.66 s).
Linux shell preserves the real exit 7 and both stream tokens in its tool
receipt, but the final assistant answer omits those outputs. It fails again
independently (39.76 s). All three failures and their repetitions are retained;
no prompt, model, timeout or final-answer assertion is changed to pass them.

Native CLI probing finds that relative --cwd changes the process directory
but remains relative in PagerArgs. Headless startup then canonicalizes the
same relative path under the new directory and fails with OS error 2 on both
platforms. apply_cwd now resolves and retains the absolute path at the original
launch boundary. The child-process hermetic test exercises Unicode and spaces,
launch-relative debug/socket paths, repeated application, missing paths and a
file used as a directory, without changing another parallel test's cwd.

A malformed ACP companion originally reported only an OS executable-format
error. The paired build check now identifies the ACP companion and directs
users to rebuild or reinstall both executables together. Missing, mismatched
and malformed native companions are tested separately; each exits before a
model or tool call. Temporary bundle executables have native PE/ELF headers
where appropriate, and no external-agent override or other-product state is
used.

Malformed ACL syntax originally reported a line and column without the actual
source path. The error now includes the participating ACL source paths. A
first context-chain implementation accidentally hid the original reason from
error.to_string(): the unchanged goal_control refusal regression fails in
both complete config groups and again independently. The final implementation
retains that reason together with the new paths. Both config groups pass
25/25, including the original failure and two new actual-loader diagnostics
cases. These diagnostic tests also retain file bytes and do not include the
fixture credential in the reported error.

The final native CLI packet qualifies eight startup cases per platform:
non-TTY JSON output, Unicode/space relative cwd, missing/mismatched/malformed
ACP companion, missing ACL, malformed ACL, and an undeclared model. The two
positive requests use the original ACL and kimi/k3.1-pro; fault cases use an
explicit missing/synthetic ACL or a deliberately undeclared model. The
original ACL remains byte-identical. Source IDs and binary hashes are bound
in STARTUP-RUNS.json, and all original failed probes remain available.

A separate non-TTY permission probe initially expected tool.prepared and a
denied tool receipt. Actual scheduler source ends before preparation when
confirmation.answered is false (crates/effect/src/coding.rs); this assumption
was wrong. Its failed reports and exact frozen grader are retained and excluded
from qualification. A separate scheduler-confirmation contract verifies the
original user message, a real model-requested write, the matching call ID in
the rejected confirmation, zero prepared/executed tools, no file, the scheduler's
denied terminal, one completed turn and normal non-TTY process exit. It passes
on Windows and Linux with the same input and frozen grader. This is an oracle
correction justified by the actual protocol, not a product permission bypass.

The follow-up integrates upstream main through 8e521909e; Studio is initialized
at ee723a124699525f528cc413d5c7bf7eba916cc0. Those upstream changes do not alter
any paired CLI build input. The CLI source digest stays
189982670423c6c055335d5e701b491d295583e5. The final source-only patch against that
landing base is 12436 bytes, SHA-256
3df36b92d1208ba52089d5e56a546adbb62ce0acb36c26d077c07b5f18c9f20b,
and passes reverse checking. Its six owned Rust files pass UTF-8/LF with no CR
or BOM. The original dirty checkout and its index are not used for landing.

Final follow-up checks pass the nine Linux workspace guards, native Windows
staged line endings (6/6), and cargo fmt --all -- --check in WSL after the latest
Studio pin. Windows cargo fmt --all reaches OS error 206 (command-line length),
which is retained as a tooling failure and is not reported as a formatting pass.
The mutable checkpoint index generator explicitly writes UTF-8/LF bytes to
avoid Windows-only whole-file CRLF churn; immutable probe reports, raw logs and
source patches retain their original bytes. The exact provider-secret scan
passes; its retained startup-provider-secret-scan-final.log records the final count.

The actual Windows and Linux checkpoint generators produce identical UTF-8/LF
bytes (SHA-256 890f49de2404428f33b7b00ae9778ce2b00b58501841444a625e2465daacc851).
The independent byte comparison is retained in source/checkpoint-cross-platform-repro.json.

## 2026-10-29: shell-output continuation and green 35-entry pair

The current candidate records digest-bound stdout/stderr boundaries for native
sandbox bash receipts. Folded model history labels the two streams only when
the retained text, byte lengths and SHA-256 values all match; changed, redacted
or malformed text falls back to the original envelope without acquiring a
stream identity. The runtime contract retains literal diagnostic arguments and
requires actual output even for failed commands. Hermetic cases cover Unicode,
empty and unterminated text, altered metadata and redaction, real exit codes,
and original tool-call arguments.

The earlier Windows `full-35-batchfix-native` run stopped at a sandbox-backend
availability failure before `tool.prepared`. The unchanged interruption case
passes the new `continuation-preflight/` on both platforms, and the complete
continuation then passes Windows 35/35 in 1492.90 seconds and Linux 35/35 in
1760.44 seconds. Each platform has 41 case reports because some entry points
exercise multiple fixture branches. Pair
`a891556b0cee8096b81768f051ce80918e460099` and every executable/report digest
are bound in `RUNS-A891.json`. The shell scenario reports actual exit 7, both
independent output streams and native cwd in the final Kimi answer on both
platforms, then completes the same-session recovery request. No prompt, model,
deadline or live grader was changed for this continuation.

After that live batch, a test-only lock-scope correction removed the new
Clippy warning without changing product code, prompts or live assertions. The
focused real-scheduler test passes 1/1 on each platform, and Core/ACP `--lib`
Clippy passes on each platform. Full `--all-targets` Clippy remains red on
unchanged existing tests: Windows soak integration tests lack `c_void`
imports, and both platforms retain two `unused_io_amount` errors in
`llm/http/redirect_tests.rs`. The reruns no longer report the owned
`tool_call_fidelity` warning. These unrelated existing errors are not hidden or
waived by the green live batch.

The continuation also retains these successful checks:

- Whole-workspace `cargo fmt --all -- --check` in WSL, followed by focused Core
  formatting after the test-only scope correction.
- Line-ending guards 6/6, including a final rerun after evidence generation;
  internal workspace dependencies 6/6.
- CLI pair delivery 10/10, release governance 9/9, Linux installer 13/13,
  Kylin launcher 11 tests with one conditional skip, progress benchmark 8/8,
  musl linker guards 2/2, and Studio checkout ownership.
- Current Linux Core library build, plus the exact provider-secret scan over
  2137 evidence files. The initial wrong-target rlib selection did not execute
  the scanner and is retained separately from the final passing log.

`source/generate_checkpoints.py` regenerated the index from 104 successful
evidence files. It retains 153 checkpoints and the existing 18 verified, 75
partial, 59 pending and one explicitly unavailable distribution, with
`complete: false`. Windows and Linux reads of the regenerated UTF-8/LF index
have the same SHA-256
`a8b3adbc02e9a7fb5dd04f49ae633fc84be4270a90ac2e37134b3bb7eed5c43f`.
The 35-entry green pair closes the retained shell-output inconsistency for its
frozen contracts; it does not complete the remaining feature matrix, full
pager library, physical IME/mouse qualification or release acceptance. No
completion commit or push is claimed by this continuation.

## 2026-10-29: real TUI workspace-trust acceptance

Local/dev Aster builds intentionally auto-trust workspace content. The two new
ignored `workspace_trust` scenarios therefore use the existing
`GROK_TEST_VERSION` test hook to exercise the release-stamped gate without
adding a product bypass. Each fixture is an isolated Git workspace with a
`.mcp.json` declaration and a native PowerShell or shell process that writes a
marker and exits successfully. The short-lived process isolates trust admission
from MCP handshake behavior; the tests do not claim that the declared server
implements MCP.

Before acceptance, both platforms show the actual workspace path, no marker,
no session/model facts and no `trusted_folders.toml`. Acceptance persists one
grant, starts the declared process and permits a real Kimi read. A real restart
completes another read without another trust prompt. A second workspace in the
same isolated home asks again; declining it leaves the original grant
byte-identical. Separate `n` and Esc branches exit before the marker, model
work or an acceptance record. Final `trust-acceptance-short-fixture/` batches
pass Windows 2/2 in 46.68 seconds and Linux 2/2 in 60.53 seconds.

Early retained failures distinguish harness synchronization from product
behavior. Session-info popups and model footers were not valid readiness
barriers. The shared live observer now requires the exact current
`user.message` before accepting `turn.complete`, preventing historical replay
from settling a new prompt. As a regression check, the existing
resume/continue model-history scenario also passes on Windows (143.90 seconds)
and Linux (62.44 seconds). No production source was changed for trust.

The checkpoint generator links the new reports and now records 21 verified, 74
partial, 57 pending and one explicitly unavailable checkpoint from 110
successful evidence files. Its Windows and Linux SHA-256 is
`663a6b5e4c7f5c1a562ae53af1cb730c5f84172486a942d6e49e41e1be0f2df7`.
F02.01, F02.02 and F02.03 are verified only for their listed contracts;
provider/workspace ACL authority, path escapes, physical terminal behavior and
the other 132 non-verified checkpoints remain open.

Both ACP live-test-target Clippy runs pass. Final line-ending guards pass 6/6,
and the exact provider-secret scan passes over 2269 evidence files.

## 2026-10-29: workspace provider and host-authority boundary

The `workspace-acl-authority` scenario adds a same-name `kimi` provider with a
synthetic key and an intentionally unusable loopback URL to the workspace ACL.
The original host ACL remains byte-identical. After trust acceptance, a real
Kimi read succeeds, proving that the workspace provider did not reroute the
request or replace host credentials. The same session then requires a real
write under `--yolo --deny Write(*)`; the write receipt is denied and the
original file bytes remain unchanged. Thus workspace content cannot relax the
explicit host deny either. The final focused run passes Windows 1/1 in 53.68
seconds and Linux 1/1 in 42.59 seconds.

F02.05 is now verified for this listed contract. The index contains 22
verified, 74 partial, 56 pending and one explicitly unavailable checkpoint
from 112 successful evidence files, with SHA-256
`5d3bef1a37ea86ab8d9827866c0b7d7c3be1114f10c7e65d79084ccaf26d4441`.
Both ACP live-test-target Clippy runs pass, line-ending guards pass 6/6, and
the exact provider-secret scan passes over 2285 evidence files. Path escapes,
link/reparse-point behavior and bounded reads remain open under F02.06.

## 2026-10-29: path confinement and bounded reads

Four independent live cases per platform exercise actual read receipts rather
than model-only refusals. Absolute and `../` escapes fail closed without
returning the outside secret. A Windows junction and a Linux symlink likewise
cannot expose the outside file. The large-file read returns exactly twenty
lines, with `requested_limit=20`, `applied_limit=20`, `returned_lines=20`,
`next_offset=20` and `eof=false`; line 21 and the later secret are absent. The
final `path-boundaries-diagnostic/` run passes Windows 1/1 in 62.16 seconds
and Linux 1/1 in 99.29 seconds.

Independent sessions are required so a previous permission refusal cannot make
the model skip the next diagnostic. The assertions accept the real plain-text
`denied` receipt as well as JSON `failed` envelopes, while the bounded success
continues to require structured range metadata. These harness corrections do
not change the sandbox or the frozen read outcomes.

F02.06 is now verified for its listed contract. The index has 23 verified, 74
partial, 55 pending and one explicitly unavailable checkpoint from 120
successful evidence files, with SHA-256
`6bb820e0d9267e2f920d18ae5ed7213e6f35f5e45c67e105629e585ebab7827a`.
Both ACP live-test-target Clippy runs pass, line-ending guards pass 6/6, and
the exact provider-secret scan passes over 2332 evidence files.

## 2026-10-29: interactive relative cwd with Unicode and spaces

The interactive `relative-cwd` scenario launches the real TUI with a relative
`--cwd` of `工作 空间`, then reads a token that exists only in that directory.
The original directory has been renamed, so a successful read proves that UI,
ACP and the tool all use the relocated workspace rather than the launch parent
or the previous directory. Windows passes 1/1 in 17.12 seconds and Linux 1/1 in
21.55 seconds. The original host ACL is unchanged.

The first attempt exposed a test-driver duplicate `--cwd`; a later Linux run
also retained a stale test executable. These harness results stay separate
from the final binary-qualified evidence. Source anchors are workspace-relative
by contract, so the assertions use the actual read result and old/new directory
state instead of assuming an absolute anchor. No product source changed.

F02.04 is verified for its listed contract, completing all six F02 checkpoint
contracts. The index has 24 verified, 73 partial, 55 pending and one explicitly
unavailable checkpoint from 122 successful evidence files, with SHA-256
`f6a71ca65b84551ac4fe7d1106c216ae600b2b5818199639e9ac11e0bb06f0df`.
Both ACP live-test-target Clippy runs pass, line-ending guards pass 6/6, and
the exact provider-secret scan passes over 2380 evidence files.

## 2026-10-29: file references, line ranges and workspace confinement

The file-reference acceptance uses actual fuzzy completion and a real file chip.
It opens the viewer, selects lines 1-2 and requires the composer to contain
`@ref-target.txt:1-2`. Missing-file and outside-workspace cases first select a
real file, then remove it or redirect its parent through a Windows junction or
Linux symlink. This avoids treating plain text as a file-reference chip.

Before the fix, both platforms silently cancelled a missing-file preview. Both
also displayed the synthetic outside secret after the parent-directory link was
redirected. These counterexamples remain under `file-references-before-fix/`
and `file-references-boundary-before-fix/`; earlier harness-input failures are
kept separately and are not presented as product defects.

The preview entry now canonicalizes the target, rejects a destination outside
the session workspace and reports missing or unreadable files. It preserves the
original relative path for display. The final pair shares source digest
`be1f1bc23acbd2ecb5c01205657b84d961fa0a16`; its source patch passes reverse
application checking. Windows passes 3/3 in 14.19 seconds and Linux passes 3/3
in 24.97 seconds. No case starts model work or changes the original ACL.

Each platform also passes the three new hermetic viewer tests and the two
existing focus-owner regressions. Both ACP live-test-target Clippy runs pass.
The full pager Clippy attempt remains red on 30 pre-existing errors in
unmodified paths, with no diagnostics in the owned paths; it is not reported
as a passing workspace-wide lint result.

F03.09 is verified for its listed contract. The index has 25 verified, 73
partial, 54 pending and one explicitly unavailable checkpoint from 128
successful evidence files, with SHA-256
`0e55fb764beb7c2921ef83514602f78106e0b3eb335935ccbfa890c8773a828d`.
Whole-workspace formatting passes under WSL, and both line-ending guards pass
6/6. Exact provider-secret scans are recorded in
`source/file-ref-secret-scan-*.log`. Full acceptance remains open.

## 2026-10-29: literal multiline keystrokes

Two new live scenarios use direct key input rather than bracketed paste. In
multiline mode, bare Enter inserts newlines and Alt+Enter submits. In normal
mode, Alt+Enter and backslash followed by Enter insert newlines without starting
model work, then bare Enter submits. Unicode text and every newline must match
the exact `user.message`, with one settled response and no tool calls.

The existing `be1f1bc2...` product pair passes Windows 2/2 in 21.38 seconds and
Linux 2/2 in 24.51 seconds. Both ACP live-test-target Clippy runs pass. No
product source changed for this checkpoint.

F03.02 is verified for its listed contract. The index has 26 verified, 72
partial, 54 pending and one explicitly unavailable checkpoint from 132
successful evidence files, with SHA-256
`65ad7744c6b1f0e15800402ff763d0b8b198de69c0cd6c36e0d1dad9521b2207`.
Exact provider-secret scans are recorded in
`source/multiline-secret-scan-*.log`. Full acceptance remains open.

## 2026-10-29: selection and multiline edit controls

The selection scenario uses Shift+Left to select an exact suffix, replaces it,
then selects and deletes it with Backspace. It does not use a word-deletion
shortcut as a substitute. Unicode before the selection remains intact, no
transcript is submitted during editing, and the final request is byte-exact.

The multiline scenario creates three direct-key lines, then exercises undo,
redo, Ctrl+S stash/pop and clear followed by undo. The final three-line Unicode
`user.message` is checked byte for byte. Both scenarios use the existing
`be1f1bc2...` product pair and change no product source.

Selection editing passes Windows 1/1 in 12.01 seconds and Linux 1/1 in 15.71
seconds. Multiline recovery passes Windows 1/1 in 10.79 seconds and Linux 1/1
in 24.05 seconds. Both ACP live-test-target Clippy runs pass. Historical draft
counterexamples remain linked.

F03.04 and F03.05 are verified for their listed contracts. The index has 28
verified, 70 partial, 54 pending and one explicitly unavailable checkpoint from
136 successful evidence files, with SHA-256
`99f60f4649efbf94197caaa3ebf5ee3736ced9c51d53d29370980f494b0ea68d`.
Exact provider-secret scans are recorded in `source/edit-secret-scan-*.log`.
Full acceptance remains open.

## 2026-10-30: history recall, edit and resubmission

The new scenario first submits `ORIGINAL-HISTORY`, recalls the actual prompt
through Up and Enter, deletes the old token, types `EDITED-HISTORY` and submits
again. The two distinct `user.message` texts must occur once each in order, on
the same session, with one settled response for each and no tool calls.

An initial harness wait matched `history` in the old transcript and submitted
before the panel had opened. A subsequent row check used single-line `contents()`
on Windows instead of the driver's row-preserving `physical_screen()`. Both were
harness synchronization/parsing errors, not missing product completion events;
the failed reports already contained the real second `turn.complete`. The final
scenario waits for the actual panel border and for the original prompt to appear
in the composer itself. Compile and format failures are retained separately.

The corrected scenario passes Windows 1/1 in 17.23 seconds and Linux 1/1 in
21.19 seconds. Both ACP live-test-target Clippy runs pass. The original ACL is
unchanged and no product source changed.

F03.06 is verified for its listed contract. The index has 29 verified, 69
partial, 54 pending and one explicitly unavailable checkpoint from 138
successful evidence files, with SHA-256
`b30fe8990e3fe786b7a6588861e6503cfc461376971143a1811e3e892ae11d06`.
Exact provider-secret scans are recorded in `source/history-secret-scan-*.log`.
Full acceptance remains open.

## 2026-10-30: external editor cancel without save

A native PowerShell or shell editor writes only an exit marker and exits with
status zero; it never writes the temporary prompt file. The marker proves the
real editor ran and exited. The original draft must remain unchanged and no
model work may occur before an explicit submission.

The first fixture incorrectly expected a second editor argument. The actual
editor passes only the prompt path, so the marker was absent; this was a fixture
bug, not an editor-hang defect. Embedding the marker path in the script fixes
it. Those failures remain under `editor-cancel-marker-args/`.

The corrected scenario passes Windows 1/1 in 18.21 seconds and Linux 1/1 in
10.99 seconds. Explicitly submitting the preserved draft then returns a real
response, proving terminal control was restored. Both ACP live-test-target
Clippy runs pass. The original ACL and product source are unchanged.

F03.08 is verified for its listed contract. The index has 30 verified, 68
partial, 54 pending and one explicitly unavailable checkpoint from 140
successful evidence files, with SHA-256
`13841dee51faed090b3bd46fc631981ab92e249c079e30bdcd5b4e9727a017fd`.
Exact provider-secret scans are recorded in `source/editor-secret-scan-*.log`.
Full acceptance remains open.

## 2026-10-30: current-pair queue and send-now qualification

The existing queue scenario already checks three queued rows: delete the last,
edit the second, then drain the exact messages and responses once in order. The
model-stream send-now scenario also covers text typed while a turn is running
but not yet submitted. Thus the remaining draft and queue-source combinations
did not need a new scenario; they needed current-pair evidence.

All three scenarios were rerun on the `be1f1bc2...` pair. Queue editing and
draining passes Windows 1/1 in 61.05 seconds and Linux 1/1 in 61.04 seconds.
The two send-now scenarios pass Windows 2/2 in 37.37 seconds and Linux 2/2 in
52.65 seconds. They cover running-tool cancellation without late writes and
model-stream cancellation followed by one exact settled request. Both ACP
live-test-target Clippy runs pass; no product source changed.

F03.11 and F06.05 are verified for their listed contracts. The index has 32
verified, 66 partial, 54 pending and one explicitly unavailable checkpoint from
146 successful evidence files, with SHA-256
`5cb6243f80d2050eb6a510ca63f634541812a36b2064f36e70bbbe67e2eb5100`.
Historical counterexamples remain linked. Exact provider-secret scans are
recorded in `source/queue-secret-scan-*.log`. Full acceptance remains open.

## 2026-10-30: home picker cancellation and original-session return

The new live scenario creates a real session, opens `/home`, selects `Resume
session`, cancels with `Esc:back`, then returns to the original session. The
original ID, transcript and fact log must remain intact, with no extra session
or model call. A subsequent exact prompt completes a second real Kimi turn in
that restored session. Windows passes 1/1 in 18.09 seconds and Linux passes 1/1
in 23.54 seconds on source pair
`a2188f93f97c2608c63bc3e38e9bf08e32eaedef`.

Two defects were corrected before this result. TUI-created sessions now carry an
explicit persisted `sessionKind=interactive`; ACP validates this closed field
and the default catalog excludes unknown or headless records without guessing
from titles or IDs. The pager also accepts Code's `id` field in addition to its
existing session-ID aliases, so a returned interactive session is not discarded
by the picker. New marker/type-conflict, catalog-filter, ID-alias and
home-return regressions pass on both platforms, alongside focused Core, ACP and
live-target Clippy checks. The final pair and reverse-checked owned patch are
bound in `source/home-return-verification.json`. This is not a new complete
35-scenario batch or completion of the 153-checkpoint campaign.

F04.02 is verified for its listed contract. The index has 34 verified, 64
partial, 54 pending and one explicitly unavailable checkpoint from 154
successful evidence files, with SHA-256
`c326d2fde2e3c55b4c78f9845463a3ebc9cb49d00335b108b274f31ab9903852`.
Whole-workspace formatting passes under WSL; both line-ending guards pass 6/6.
Exact provider-secret scans are recorded in `source/home-return-secret-scan-*.log`.
The unchanged-path pager Clippy errors and recorded native-console assertion
failures remain separate, unresolved pre-existing issues. Full acceptance
remains open.

## 2026-10-30: session info agrees with the live backend

The new scenario reads an empty session, completes two real Kimi turns, switches
to YOLO between them, and checks every `/session-info` field against the actual
ACP response. A transparent stdio trace proxy forwards `--build-id` and all
protocol traffic unchanged to the frozen companion. It does not replace the
model or tools and its raw trace stays in the temporary fixture, not in evidence.
Both platforms verify ID, canonical cwd, configured model, `permissionPosture`,
turn counts, context usage, total window and percentage. Windows passes in 31.59
seconds with 26758/128000 tokens (21%) after turn two; Linux passes in 38.35
seconds with 26524/128000 tokens (21%). Both exit normally with the original
ACL unchanged.

The initial frozen runs failed only the mode assertion after ID, cwd, model,
zero turn and zero context had matched. ACP already returned `permissionPosture`,
but the shared typed response discarded it and the pager never displayed it.
`SessionInfoResponse` now accepts that optional canonical value and the
session-info fields show it as `Mode`; older responses without it remain
unchanged. Twelve focused session-info regressions pass on each platform.
Independent verification checks both reports, increasing real usage, frozen
binary hashes, test sources and the reverse-checked owned patch. Source pair
`a878a396eca1ee4dac118fe65ecc40d4487ed669` is bound in
`source/session-info-verification.json`; its patch is 113107 bytes with SHA-256
`d57a37d5e709b04e794cbf2bd391e6bf927796bf84a890e3f950da99bb1bed84`.

F04.03 is verified for its listed contract. The index has 35 verified, 63
partial, 54 pending and one explicitly unavailable checkpoint from 152
successful evidence files, with SHA-256
`406fdf004ac4fc63a045e6f01bb48137cece8edab6c14d5d8ea3f130d9a4c026`.
Whole-workspace formatting passes under WSL. Clippy remains red on existing
unmodified Shell and dependency paths; its diagnostics are retained in the
platform `session-info-final-pair/shell-clippy*.log` files and are not reported
as clean. Exact provider-secret scans are recorded in
`source/session-info-secret-scan-*.log`. This slice is not a new complete
35-scenario batch or completion of the 153-checkpoint campaign.

## 2026-10-30: continue selects the latest session per workspace

A new live scenario creates A1 in the original workspace, B1 in a second
workspace, then A2 in the original workspace. All three are interactive sessions
under one home with real Kimi turns and distinct remembered tokens. `--continue`
must select A2 for the original workspace and B1 for the second, not simply the
globally newest session or the older A1. Each restored session recalls its own
token without any tool call. The older A1 fact log must remain byte-identical.
Windows passes in 71.19 seconds and Linux in 57.20 seconds; each completes five
real Kimi turns and five normal exits with the original ACL unchanged.

This needs no product correction: the already qualified
`a878a396eca1ee4dac118fe65ecc40d4487ed669` pair is reused. The new lifecycle
source and independent checks are bound in `source/continue-verification.json`.
The complete current owned patch is retained as `source/continue-owned.patch`
(116787 bytes, SHA-256
`759a078df940019d5f138390afa481fb3183bab9928e80c9651a4baaf7e60f07`) and passes
reverse application checking. The first test-only ownership compile failure is
retained and is not a product result.

F04.05 is verified for its listed contract. The index has 36 verified, 62
partial, 54 pending and one explicitly unavailable checkpoint from 154
successful evidence files, with SHA-256
`a1ba8f3331bc23735b552366862aca89e61bc4889a51c351f1e83d8269b547ff`.
Exact provider-secret scans are recorded in `source/continue-secret-scan-*.log`.
Full acceptance remains open.

## 2026-10-30: session picker preserves command keys and real identity

A new live scenario creates two interactive sessions in one workspace with the
same title, different UUIDs and different remembered tokens. After resuming the
newer session, `/home` opens the picker and `/` activates search before pasting
the complete older UUID. The first Esc leaves search, the second clears the
query and restores both duplicate-title rows, and the third closes the picker.
Cancellation creates no third fact log and leaves the newer history byte-identical.
Reopening the picker and selecting the filtered row restores the older ID; its
next real Kimi turn recalls the older token without tools, again leaving the
newer history unchanged. Windows passes in 31.38 seconds and Linux in 41.06
seconds, each with three real model turns and three normal exits. The original
ACL remains unchanged.

Both platforms initially failed to cancel the filtered picker. The input event
merge dropped non-pasteable `Event::Key` values, including Esc, while combining
paste fragments with native text events. Those keys are now preserved in order.
The failing input regression produced one event instead of paste plus Esc; after
the fix, both platforms pass all 9 native-paste regressions and the Welcome
three-stage Esc state-machine regression. The new pair is
`dc0b1f6c30f7ae609186c1c699a913e1d67bfb76`. Reports, binary hashes, focused
checks and the reverse-checked source patch are bound in
`source/picker-verification.json` and `source/picker-owned.patch`. The original
failures remain in both `picker-before` directories.

The driver was also corrected without changing the product contract:
`/session-info` is only queried after the picker closes, and cancellation checks
history and fact-log counts rather than mistaking Welcome's unused optimistic
session for the session just left. The final case deliberately activates search
before pasting. The retained Windows `picker-final-before-search-focus` failure
shows an unfocused native paste beginning with `e` triggering the expand shortcut;
that separate input path is not claimed as verified by this result. Intermediate
harness failures are retained and are not counted as product passes.

F04.06 is verified for its listed contract. The index has 37 verified, 62
partial, 53 pending and one explicitly unavailable checkpoint from 156
successful evidence files, with SHA-256
`b4d13948603b93b44cde3634eab9e535ba6e0fd61cd155f0fb52538638f91bee`.
Whole-workspace formatting passes under WSL; both line-ending guards pass 6/6.
Exact provider-secret scans are recorded in `source/picker-secret-scan-*.log`.
This is not a new complete 35-scenario batch or completion of the 153-checkpoint
campaign. Full acceptance remains open.

## 2026-10-30: native paste preserves picker query text

The previously unverified unfocused-paste path is now exercised directly. A cold
launch opens the session picker and pastes a UUID-shaped value beginning with
`e` four times without first activating search. Every iteration must preserve
the complete query and must not expand the selected session card. The original
same-title UUID selection, cancellation and history-recall scenario is rerun in
the same batch. Windows passes both cases in 45.43 seconds and Linux in 56.57
seconds, with four explicit query observations and normal exits on each platform.
The original ACL remains unchanged.

The event trace showed two cooperating causes: a single-line native paste could
remain ordinary picker command keys, and the initial detector returned as soon as
it received the first key release, before later text arrived. The detector now
keeps collecting until text arrives or the fixed deadline expires, while initial
and continuation quiet windows share the existing 10 ms bound. Single-line paste
coalescing is enabled only for the Welcome, AgentModal and Dashboard session
pickers; ordinary editors and a standalone `e` command keep their existing
behavior. Both platforms pass all 11 native-paste regressions, including an
immediate-release-plus-delayed-text counterexample. Temporary diagnostic logging
was removed before the final pair was built.

The final pair is `f44293dfe21600e0f6002752e69f7a90f534be87`. Reports, frozen
binary hashes and the reverse-checked owned patch are bound in
`source/picker-release-final-verification.json` and
`source/picker-release-final-owned.patch`. The original Windows failure and the
failing Release regression remain retained; intermediate candidate pairs are not
reported as final passes. Focused Clippy remains red on an existing unused import
in unmodified `a3s-tty-utils`, recorded in
`windows/picker-release-final-pair/clippy.log`.

F04.06 remains verified for its listed contract, now including the unfocused
native-paste path. The index has 37 verified, 62 partial, 53 pending and one
explicitly unavailable checkpoint from 160 successful evidence files, with
SHA-256 `703c6e1d14f830e4048550e44913578eed3e48c3a9e3beb62916ac346641ee11`.
Whole-workspace formatting passes under WSL and both line-ending guards pass
6/6. Exact provider-secret scans are recorded in
`source/picker-release-final-secret-scan-*.log`. Full acceptance remains open.

## 2026-10-30 continuation: native terminal gates and background ownership

The continuation retains the 153 original checkpoints, `kimi/k3.1-pro`,
existing prompts, timeouts and grading assertions. Both native product pairs
were rebuilt and frozen with source digest
`e1a37d311f896ceebb6a7a228a0f6a889f15313b`. The source capture is
`source/continuation-20261030-source.json`, with a reverse-checked 142194-byte
owned patch and SHA-256
`3a169a531e4853bd0543dd174513abd3802533eaee5d6c7c40587900c45b881e`.

Removed the unused Windows `AsRawHandle` import and restricted the Linux-only
runtime-test imports and OOM test mutex import to Linux. The actual
`a3s-tty-utils` implementation passes strict all-target Clippy on both native
platforms. Its Windows tests pass 39/39 plus two doctests; Linux tests pass
75/75 plus two doctests. Logs are retained under each platform's
`continuation-20261030-gates/`. Whole-workspace formatting passes under WSL
(`source/continuation-20261030-fmt-before.log`) and both native line-ending
guards pass 6/6. Full-product strict Clippy still fails on two existing
`a3s-common` findings (`derivable_impls` and `unnecessary_sort_by`); these
failures are retained in `continuation-20261030-gates/product-clippy.log`.

The previous background-owner evaluation's Windows failure is retained at
`windows/background-owner-before/background-session-owner.json`. Its tool
creates `owner-result.txt`, while the fixed prompt permits only that one tool
call and the final reply. The completion gate correctly refuses unverified
workspace mutation. This is not counted as a passing evaluation, and neither
the prompt nor the completion gate was weakened.

Added a separate read-only background-owner evaluation. It reads an unseen
fixture token through a real, permission-gated bash call while switching
between two live sessions. Windows passes: session B's log remains
byte-identical while A owns the pending permission, single tool result and
final reply; the fixture bytes remain unchanged. Evidence is
`windows/continuation-20261030-owner/background-session-read-only-owner.json`.
An exact resolved-provider-value scan of that report passes. Linux's matching
evaluation and both platforms' complete 57-entrypoint live regression batches
are in progress; their retained logs are under
`<platform>/continuation-20261030-pair/`. The 57-entrypoint test inventory is
recorded separately and does not replace the 153-checkpoint contract.

Explicitly not claimed: full 153-checkpoint completion, a passing product
Clippy gate, or whole-pager library qualification. The baseline test failures,
new failed evaluations and original grading contracts remain available.

### Continuation results: ownership, Plan admission and harness attribution

Both native read-only background-owner evaluations pass: Windows 54.90s and
Linux 54.56s. `source/continuation-20261030-verification.json` binds their
reports, paired binaries and terminal gates to the original frozen source.
F04.07 is verified for its listed contract. The mutation-producing evaluation
and its completion-gate failure remain separate, retained evaluations.

The original Windows 57-entrypoint batch finishes with **52 passed and 5
failed**, in 1941.15s. Its unmodified log is
`windows/continuation-20261030-pair/all-57-run.log`. It is not reported as a
passing batch. Four failing entrypoints now have independent passing native
follow-ups on both platforms:

- `session-info-contract`: the batch launcher omitted the required
  `A3S_CODE_ACP_TRACE_PROXY`. Restored the transparent native relay, retaining
  its source at `source/acp_trace_proxy.rs`. With the relay configured, the
  unchanged scenario passes in 28.69s on Windows and 37.20s on Linux, including
  canonical identity, cwd, mode, turns and actual provider usage.
- `backend-refusal`: the unchanged independent Windows reproduction also
  fails at `/workflow`. Its previous `/workflows` refusal contains the next
  command as a substring, so the driver accepted old screen text as readiness
  and submitted before the new draft arrived. The driver now waits for the
  exact draft in the actual boxed or minimal composer, within the existing
  eight-second timeout. All 42 disabled commands and the unknown command pass
  on Windows (24.23s) and Linux (35.73s). No prompt or grading assertion changed.
- `expand`: the original timeout occurs with `/fullscreen` still in the
  minimal composer and the same command already visible in its footer hint.
  The corrected input barrier passes the unchanged scenario on Windows
  (22.08s) and Linux (18.40s).
- `surface`: the original mouse-restoration assertion fails with
  `/toggle-mouse-reporting` still in the composer. The corrected driver passes
  the whole unchanged surface scenario on Windows (139.68s) and Linux
  (174.75s), including the later minimal/fullscreen transitions.

The fifth original failure is the mutation-producing background-owner
evaluation described above: the completion gate rejects a mutation with no
verification. Its original prompt and assertions remain unchanged and the
failure is not converted to a pass. Supplemental runs are not substituted
into the original 57-entrypoint totals.

Added a separate Plan search/exec admission scenario. Both platforms really
search an unseen fixture, then attempt a side-effect-producing bash call.
The recorded admission is `denied`, the output file remains absent, and the
fixture bytes remain exact. Windows passes in 79.91s; Linux passes in 83.42s.
Together with the completed current-pair Plan read/write scenarios, this
verifies F01.02 and F09.01. Reports and provenance are bound by
`source/continuation-20261030-plan-verification.json` and
`source/continuation-20261030-harness-plan.json`.

Only test-driver/scenario sources changed after freezing the product pair;
the supplemental runs explicitly reuse the frozen `e1a37d3` product. Separate
test-binary hashes and archived driver versions are retained in
`source/continuation-20261030-harness-followup.json`,
`source/continuation-20261030-harness-plan.json` and
`source/continuation-20261030-controls-verification.json`. A Windows attempt
to relink the still-running original test executable fails with LNK1104 and
is retained; the supplemental harnesses are linked to distinct paths instead.

The regenerated 153-checkpoint index now has **40 verified, 60 partial, 52
pending and one explicitly unavailable**, with SHA-256
`2a52407518e4263c23d7fab5d2df6e80fd7de315fec25ff0c727fd4c17a2d89b`.
`source/continuation-20261030-index-audit.json` independently checks all 206
referenced file hashes and both-platform evidence for every verified contract.
Whole-workspace formatting after the Plan addition passes under WSL; the
final driver and all edited Rust files also pass focused rustfmt checks.
Both native exact resolved-provider-value scans pass, including the campaign
scan retained in `source/continuation-20261030-campaign-secret-scan-*-final.log`.

The original Linux 57-entrypoint batch is still running. Once it ends, run
`source/reconcile_continuation_batch.py` to bind both original batch summaries
and every passing/failing report into `RUNS-E1A57.json`. Full acceptance,
product strict Clippy and whole-pager qualification remain open.

### 2026-10-30 completed 57-entrypoint batches and native question/form follow-up

The original `e1a37d311f896ceebb6a7a228a0f6a889f15313b` batches have now
finished on both platforms: **Windows 52 passed / 5 failed in 1941.15s;
Linux/WSL 54 passed / 3 failed in 2348.20s**. `RUNS-E1A57.json` binds the
original inventory, logs and passing/failing case reports. Supplemental
passes do not replace those totals. Four Windows failures were independently
resolved as input-readiness/proxy harness defects on the same frozen product;
the Linux write-and-verify startup repro also passed independently. The
mutation-producing background-owner evaluation still fails on both platforms:
its fixed prompt permits the write but excludes the verification required by
the completion gate. That evaluation is retained without changing its prompt
or weakening the gate.

New real-provider question tests first reproduced a product failure on both
native platforms: dismissing `ask_user` closed its dialog but left the turn
waiting for an answer beyond 90 seconds. The ACP question handler now uses
the existing owned-session cancellation when the permission dialog is
dismissed. A real in-memory ACP transport regression verifies the cancelled
stop reason and that another session remains uncancelled. The full regular
ACP library suites pass on source `70f49387956f5bba7ade5552b9ca82291464886f`:
Windows **363 passed, 8 ignored**; Linux **398 passed, 8 ignored**. The eight
existing ignored cases require other live providers, Cursor authentication,
or Studio and are not counted as passes.

The initial cancellation grader also had a separate defect: it demanded a
successful `turn.complete` fact, although cancellation intentionally has no
success completion fact. That grader and its failures remain unchanged.
A separate test retains the same prompt and checks the actual ACP
`cancelled` response, idle UI, no synthetic default answer, no success
completion, and a fresh successful real-model request in the same session.
Both native platforms pass. Choices, disabled free input and an unseen
Unicode free-text answer are verified against the actual question ids and
canonical question-answer payloads. The initial Linux model response that
asked its second question as plain text instead of calling `ask_user` is
retained as a model failure.

The complete shared JSON Render component catalog is now exercised: **13
distinct component types**, including all eight editable fields and native
text/metric/chart mappings. The actual tool receipts contain exactly eight
edited typed values, including Unicode text, numeric values, enum and boolean,
on both platforms. A historical grader expecting integer-only metric display
and every field to be simultaneously visible is retained as an excluded
evaluation; a separate native-profile case uses the same source document and
prompt and checks the shared parser's numeric display and scrollable viewport.
Unknown component, extra schema version, cyclic document and a document with
no editable field all produce real tool errors without blank input cards;
each platform then completes a fresh real-model request.

The 24-required-field test found two small-window product defects at 14x64.
Five reserved scrollback rows initially hid form fields and actions. A
form-specific layout can now use those rows and hides competing background
panes while the form needs the space. After that fix, an invalid submit could
still scroll the first field error outside the viewport while actions had
focus. The body scroll target now includes that first error. Both failures
remain as counterexamples. Focused real-buffer tests pass **25/25 on both
platforms**, including the visible error and reachable actions; the agent
layout suite had also passed **54/54 on both** after the first layout fix.
These focused suites do not qualify the whole pager library.

The unchanged live long-form case now passes on both platforms: each of the
24 fields and both actions is reachable, invalid submission exposes the
required error, reverse Tab reaches the preceding field, all 24 exact values
reach the actual receipt, and cancellation returns no submitted defaults
before the same session completes a new model request. Final native times
are **Windows 147.20s / Linux 194.84s**. Required, invalid numeric type,
minimum, maximum, constrained enum and required boolean tests also pass on
both platforms (**36.67s / 27.62s**). The earlier Linux constraint-test
failure was a screen-parser defect: the exact error was visible inside its
heavy border. The follow-up accepts both native decorative borders while
retaining the precise error-text and submission assertions.

`RUNS-QUESTIONS-FORMS-20261030.json` independently binds seven selected
successful live cases per platform across their actual source versions.
Catalog/question/cancel cases use
`2484faae030dbb8ee7bd1c765db3ca28ad1988bb`; final constraint and long-form
cases use `d027629c78e5b0923727fadaae5ba3d8bd41c742`. Each phase retains
native Windows MZ and Linux ELF Aster/ACP pairs, a separately hashed native
test executable and an identical 67-entrypoint inventory. That inventory
includes historical failed graders; no claim is made that all 67 ran
together or passed. The original model `kimi/k3.1-pro`, operator ACL and
fixed prompts are preserved.

The final source snapshot records 55 changed source files and a reverse-
checked 193708-byte patch with SHA-256
`50ff7b6f73572a7734c6819c3d0f205d061ccc51174e7106aa07772b4ef57738`.
The earlier 2484 source is explicitly marked as reconstructed from the
recorded two later layout edits; all 55 archived file hashes and the exact
shared source digest are independently verified. The new form-core test
dependency resolves locally; Cargo regenerated the single added lockfile
dependency edge. Workspace dependency guards pass six tests on each native
platform. Whole-workspace WSL formatting, both native line-ending guards,
and both exact resolved-provider-value evidence scans pass. The Windows
whole-workspace format invocation hits OS error 206 (command line too long);
its unchanged log is retained. The same formatter passes when explicitly
checking the six affected product packages (`aster`, ACP, core, pager,
tty-utils and shell) in one bounded invocation. This does not turn the
failed Windows whole-workspace invocation into a pass.

The regenerated index is now **46 verified, 57 partial, 49 pending and one
explicitly unavailable**, still exactly **153 checkpoints / 16 domains**.
`source/forms-20261030-index-audit.json` checks all 230 referenced file hashes
and both-platform observations for every verified contract. The index SHA-256
is `d0708ea53bda49459e65c8ccc6a2e1cfebcf83a5516bc56c389d5e53d209e8d6`.
F11.01, F11.02, F11.04, F11.05 and F11.08 now have evidence for their listed
contracts; F11.03 remains partial because step behavior and other property
combinations are not qualified. The shared parser currently validates a
`step` parameter but drops it from the field schema; no claim of step
enforcement is made. Duplicate/late/cross-session form replies, real OS IME,
remaining Plan flows, reconnect/recovery and the other pending domains remain
open. Full 153-checkpoint acceptance, product strict Clippy and whole-pager
qualification are **not complete**.

### 2026-10-30 form reply ownership and step counterexample

The next native source pair is
`b44752d77fa649bf767498820c1fdb123aaa13ac`, with **56 archived changed source
files** and matching Windows/Linux **69-entrypoint** harness inventories.
Only tests and an existing test-helper visibility changed in this increment;
the prior product fixes are unchanged. The two selected live cases and the
exact Windows step reprobe are reconciled in
`RUNS-FORMS-OWNER-STEP-20261030.json`. This is not a claim that all 69 live
entrypoints ran together.

F11.06 now has both native and handler evidence. The real-provider case
starts an A form while B is active, preserves B's local draft, completes a
fresh B model request, returns to A, and presses Enter twice. There is exactly
one accepted A tool receipt with the unseen Unicode host answer. B has no
form tool call or A answer and completes another real model turn. Windows
passes in **44.68s** and Linux in **56.37s**. The actual session identities
and independently retained A/B facts are checked by the reconciler.

A separate real-handler regression keeps the old A response sender alive
while new A and B forms both wait. The delayed old answer reaches only the
old receiver; the new receivers stay empty. Two old resolved notifications
leave both new cards open. Real application key dispatch submits distinct
A/B answers to their own receivers, each response channel closes after its
single delivery, and both displaced drafts are restored exactly. The new
test passes on Windows (**0.03s**) and Linux (**0.56s**), without a provider
or copied business logic.

The step test confirms a product defect on both native platforms:
**a Slider with min=0, max=10 and step=2 accepts level=3 and returns it in a
successful `ask_user_form` receipt**. The receipt also contains the untouched
count=2. The intended rejection and editable-draft assertions fail. Windows
first had an independent model transcription failure: its Button node was
copied into malformed JSON and the tool correctly rejected that document
before opening a card (**129.76s**). The original failed evaluation is
retained. An exact reprobe with the same frozen binaries, test executable,
prompt and assertions then reaches the actual step defect (**21.01s**).
Linux reaches the same product defect in its initial run (**30.00s**).
The later fractional Input-step branch is not reached and is not qualified.
F11.03 remains partial; the step defect is open and the failing grader has
not been changed into a passing one.

The index now contains **47 verified, 57 partial, 48 pending and one
explicitly unavailable**, preserving all **153 checkpoints / 16 domains**.
`source/forms-20261030-owner-step-index-audit.json` verifies all **237**
referenced file hashes and both-platform observations for every verified
contract. Its CHECKPOINTS SHA-256 is
`b571d982ab362c51a4fefe1a73d17515102782417c9f3d81828c17ab96572258`.
Full acceptance, the step repair, remaining Plan flows, navigation/recovery,
child-agent and physical-IME work are still open.

Both exact resolved-provider-value scans pass for the 3744 files present at
the end of this native increment. The first whole-workspace format check
finds two ACP test files formatted with edition 2024 style although that
package declares edition 2021. They are corrected using edition 2021;
Windows focused formatting now passes. Archived b447 copies formatted by
the same pinned formatter match the corrected files byte-for-byte, recorded
in `source/forms-20261030-owner-step-format-only.json`. The current source
digest after this formatting-only correction is
`295070d681078ee1833dfe33e733cbb8a1054d7e`; no new runtime pair is claimed
for it. The completed native results continue to bind the preserved b447
source and binaries. A final whole-workspace WSL format check is in progress;
the initial failed format log is retained.

That final whole-workspace WSL format invocation has completed successfully.

### 2026-10-30 batched tool, checklist and real-child acceptance

The next batch adds eight real-provider cases covering patch, Git, batch,
program, Plan program admission, checklist updates, background shell tasks
and two delegated child runs. Its native runtime digest is
`c860e66c1e42149f47a9bfadff219fd6fae2f8fa`; Windows MZ and Linux ELF Aster/ACP
pairs and matching **77-entrypoint** harness inventories are frozen.
`source/tool-wave-20261030-source.json` archives 57 changed source files.
The original result is **Windows 2 passed / 6 failed; Linux 2 passed / 6
failed**. `RUNS-TOOL-WAVE-20261030.json` preserves all 16 case reports,
individual exit codes and logs. It does not claim that all 77 entries ran.

Both native batch cases pass (**23.05s / 26.34s**): three read members retain
their caller ids and input order, two exact unseen successes and one real
missing-file failure, with `partial_failure`, failed index 1 and separate
success/failure counts. The program cases also pass (**43.00s / 46.73s**):
the embedded QuickJS VM really reads an unseen Unicode fixture once and
returns complete nested receipts, then refuses a write outside its
`allowed_tools` before dispatch. Registered templates and other limits are
still open, so F07.16 remains partial.

Several initial failures are harness/evaluation defects, kept separately:
the original patch prompt excludes bash, although a read does not satisfy
the unchanged command-verification completion gate; the first Git grader
expects per-file names from a summary-only status response; the Plan
program grader parses a real plaintext `denied` receipt as JSON; and the
checklist/child cases expect an ACP trace that they have not enabled.
Windows checklist input also reaches the focused todo pane instead of the
composer. These original evaluations are not changed into passes.

Five distinct follow-up entrypoints retain the applicable inputs and
business assertions, correct those observer/setup assumptions, and add
eligible command verification for patch. Their **82-entrypoint** harnesses
are separately frozen, with source digest
`b19065d586f0e577498a8a2acb082deef990d26a`. Product sources outside the test
directory are byte-identical to the c860 snapshot, so both platforms reuse
their original frozen product pair. Each product binary and build id is
verified before reuse. Product and test-source digests are recorded
separately in `harness-source.json`; no product rebuild is claimed.

The follow-up totals are **Windows 2 passed / 3 failed; Linux 4 passed / 1
failed**. Both platforms pass the actual Plan program policy denial
(**42.50s / 49.55s**): complete empty effects, no nested write dispatch and
no output file. The denial is read as its real typed receipt, without
manufacturing a JSON tool result.

Both real dual-child cases pass (**40.75s / 53.27s**). One parent `task`
call creates two distinct task ids and child session ids. Each child really
reads its own unseen fixture and returns an exclusive output and source
anchor. The actual ACP stream has exactly one start and finish for each
child, matching parent/child identities and intervening tool progress. Both
finishes precede the parent's terminal response, and the parent consumes
both ordered outputs. F15.01 and F15.03 are verified. Child panel selection,
takeover and the remaining cancellation combinations are not inferred from
these protocol passes.

The checklist case passes completely on Linux in **51.52s**. Windows passes
all the same business assertions but initially fails teardown: `/quit` goes
into the still-focused checklist filter. A new cleanup entrypoint uses the
documented overlay Space binding after those assertions, then exits cleanly
in **53.36s**. That separately frozen Windows **83-entrypoint** harness has
source digest `76fb76a8fb44ff42da5fc6adb5e47643acb0e36e` and still reuses
the c860 product. The actual native rows show completed `CHECKLIST-SECOND`
before cancelled `CHECKLIST-FIRST`, with the old row absent; the real ACP
metadata retains the cancellation state. F07.14 is verified.

Remaining genuine failures and blocked branches in this batch are explicit:

- Windows Git rejects the actual CLI-initialized fixture as not a repository
  when its path carries the extended canonical prefix. The fixture lives
  under this worktree's `.tmp`; no user's real Git tree is used for mutation.
- WSL Git status and exact diff pass. Creating the requested branch also
  changes the current branch, violating the explicit no-checkout assertion.
  The later bad-ref branch is not reached. This is retained rather than
  changing the expected current branch.
- WSL patch passes single-file, multi-file and conflicting-second-file
  all-or-nothing checks, independent command verification and exact host
  byte assertions in **101.80s**. Windows executes the first patch, but the
  specified PowerShell verification receives
  `tool_effect_effective_arguments_unapproved`; its later multi-file and
  conflict branches remain untested. No host waiver or policy weakening is
  used to obtain a pass.
- Both background cases execute start/tail/stop/tail and a shell check. The
  final tail correctly reports that the stopped Run-owned task no longer
  exists. Parent completion then fails on the same effect digest
  `37f85931d2dadd52f8095b896f85bcfd85f20beb15f9231ea012df554e5f1790`,
  whose 23 entries project paths such as `.agents` and `.bashrc` from present
  to missing. This is a recorded effect projection, not proof that those
  user paths were actually deleted. Attribution remains open. The planned
  61-second post-completion late-write observation is not reached and is
  not counted as passed.

`RUNS-TOOL-WAVE-QUALIFIED-20261030.json` independently verifies the frozen
harnesses, archived source hashes, reused binaries, all follow-up exit codes,
native checklist status glyphs and each child's recorded wire lifecycle.
It links the original failing runs and does not replace their totals.

The complete source matrix remains **153 checkpoints / 16 domains** and now
contains **51 verified, 63 partial, 38 pending and one explicitly
unavailable**. `source/tool-wave-20261030-index-audit.json` checks **272**
referenced file hashes and requires an actual passing case on each platform
for every verified contract; provenance manifests alone cannot qualify a
contract. CHECKPOINTS SHA-256 is
`13b81879019c4302ca2ee0535c55110de6a8337e6f8a0dc921d17ce6deb70ac4`.
Full acceptance, the remaining registered tools, navigation/clipboard,
Plan review/recovery and physical IME remain open. Product strict Clippy and
whole-pager qualification also remain open.

Final verification for this batch passes whole-workspace formatting under
WSL and the native Windows edition-2021 formatter check for the integration
test root and its modules. Both exact resolved-provider-value scans pass
(4023 files observed on Windows, 4024 on Linux; the additional scan log was
created between the two starts). Reopened artifacts retain the original
153 source rows, 16 domains, all failing runs and the 51/63/38/1 counts.


## 2026-10-30 bounded coverage batch: templates, limits, navigation and settings

The 13 frozen entrypoints ran independently on both native platforms using
unchanged c860 product pairs and harness source 93f2d3eee01aaf64850b87402628cd2a0813c4e0.
Windows passed 9/13 in 361.08 seconds; WSL/Linux passed 10/13 in 389.92 seconds.
The native compiled inventory was 96, not a claim that all 96 entrypoints ran.
`RUNS-COVERAGE-20261030.json` retains every original outcome. The first Windows
launcher attempt failed before starting a case because it used Rust's `::`
namespace in a log filename; the matching Linux compilation was stopped before
case execution. The portable retry encodes only log filenames as `__`, without
changing test IDs, prompts, model, budgets or graders. The pre-execution error
is retained in `source/coverage-20261030-pre-execution-failure.json`.

Actual positive runs cover the seven-step registered repo map, numeric template
parameter error ownership, a one-call script budget, the 100ms embedded-VM timeout
and fresh-turn recovery, 256-byte Unicode output truncation, successful staged
batch bindings and isolated missing-reference failure, and anonymous wiki search
with actual titles and source URLs. The code-search template passes on WSL.
On Windows it reports the frozen 10000ms program timeout while its one real search
receipt succeeds with duration_ms=30048; this original observation remains a
counterexample rather than being relabeled by raising the budget.

Both read-viewer runs expose a product counterexample: the actual read receipt
contains all 120 fixture lines including VIEW-ROW-119, but End in the full block
viewer only reaches approximately line 76 and its final line is cut mid-content.
`agent/tool_render.rs` limits ACP text previews to 4000 characters. No product
fix was inserted into this batch, and the later Home/close/reopen assertions were
not reached.

Three independent profiles correct explicit evaluation mistakes while preserving
the original 13 tests byte-for-byte. Voice mode defaults on, so the conditional
hidden-row profile now sets the real public GROK_VOICE_MODE=0 input before startup.
It checks all 18 backend-inapplicable keys and all three voice keys individually,
plus two supported local controls and unchanged preferences/facts. The original
Windows theme grader compares whole-screen foreground color membership, which
varies with text/chrome; the independent profile observes actual background cell
(0,0), preview without persistence, Escape restoration, canonical Day commit and
reset No/Yes. The original WSL Markdown grader assumes two matches, while real
Thinking adds a third. The independent profile selects the last actual assistant
match, toggles formatted/raw twice, and verifies exact canonical UTF-8 export.
All three independent cases pass on both OSes: Windows 3/3 in 74.78 seconds and
WSL 3/3 in 87.32 seconds. Their native inventory is 99; only these three new cases
ran in this phase. Harness source is a1208f00e45e87991b37d6eb21d7496cfadb4125.
`RUNS-COVERAGE-UI-QUALIFIED-20261030.json` binds the original failures and the
independent profile evidence to the unchanged products.

The unchanged 153-row / 16-domain matrix now has 53 verified, 66 partial,
33 pending and one explicitly unavailable checkpoint. F12.06 and F16.02 gain
full listed-contract qualification. Theme preview/reset, public wiki search and
the failed read viewer gain explicit partial coverage; remaining combinations
are not marked passed. The matrix audit checks 312 referenced hashes and actual
passing cases on both platforms for every verified checkpoint. Windows and WSL
regeneration produce identical UTF-8/LF bytes, SHA-256
9246c3b5e301eecc7013621ead200d3c4a4eac2bdb06a96885209a4d77cb0d60.
This is continuing progress, not completion of comprehensive acceptance. Physical
IME, unresolved partial combinations, product strict Clippy and whole-pager
qualification remain open.

Final checks pass for the current frozen source: Windows edition-2021 formatting
and WSL whole-workspace cargo fmt --all -- --check. Both exact resolved-provider
secret scans pass over 4251 observed files. Native test/build processes have all
exited. The 59 frozen source files still match their archived hashes; reopened
run reports, matrix and outcome retain the 53/66/33/1 status counts. Full
acceptance remains open. See source/coverage-20261030-final-checks.json.


## 2026-10-30 native controlled-HTTP coverage and merge handoff

The unchanged c860 native product pairs ran 14 additional independent network
cases per platform. Initial HTTP_PROXY setup also captured the model's HTTPS
transport, so both initial 14-case phases fail before any tool dispatch. These
28 setup failures remain in their original reports. The separate provider-direct
profile sets only the test process NO_PROXY=* input: the model transport honors
it, while the tools' explicit safe_http proxy still uses the controlled origin.
No original ACL, model, prompt, timeout, or grader was changed. The 14-case test
source remains byte-identical across those two harness snapshots. Provider-direct
results are Windows 12/14 (842.71s) and WSL 13/14 (520.34s), with inventory 113.

WSL passes exact 65536-byte binary download to a Unicode/space destination,
SHA-256, and independent shell verification. Both platforms pass existing-file
refusal, failed hash with overwrite preserving the original bytes, declared size
limits, HTTP 404, private literal refusal before network, and truncated HTTP body
cleanup without promoted partial files. Windows itself returns successful
65536-byte download metadata and an independent host observation confirms exact
fixture bytes. Its subsequent actual bash Get-Item verification is prepared but
produces no result before the unchanged 420s observation deadline. The bash tool's
default command timeout is 120000ms; the precise blocking stage remains open.
A process initially observed with the same parent PID was later proven older than
the ACP and therefore a PID-reuse coincidence; it was not changed or killed.
The original observation and explicit correction are both retained.

Both platforms pass semantic-main extraction, actual Markdown link, source URLs
visible in native screens, redirect original/final anchors, HTTP 404 and 429,
real one-second timeout, private URL refusal, and a fresh model turn after each
fetch error. The original pagination test wrongly counts its JSON-document read
and artifact read as a single read and is retained as an excluded evaluation.
An independent inline-read case passes on WSL. On Windows its real artifact read
receives outcome=denied with tool_effect_effective_arguments_unapproved for the
URI web_fetch itself returned; the grader subsequently fails to parse that raw
policy denial as JSON. This underlying Windows policy counterexample is retained.
A final independent direct-pagination case passes on both platforms, verifying
exact Unicode slices 0..17 and 17..EOF, source, total length and artifact identity.
Its source appends to both previous frozen test sources without editing them;
compiled inventory is 115, and this phase runs only that one new case per OS.
`RUNS-NETWORK-20261030.json` reconciles all eight phases and retains every failure.

The matrix remains 153 source rows in 16 domains: 53 verified, 68 partial,
31 pending, one explicitly unavailable. Download and web_fetch now have actual
positive/negative coverage and precisely identified remaining failures. The new
index audit checks 388 referenced hashes and actual dual-platform passing cases
for each verified row. Windows/WSL regeneration is byte-identical UTF-8/LF,
SHA-256 09e4dd66ef5d95e96159fa565d5d40d94d1c7b804811a097694bc1cd5cc22687.
Native Windows focused formatting and WSL root cargo fmt --all -- --check pass.
Comprehensive testing is still incomplete; these historical c860 observations do
not qualify a different integration product automatically. The user's merge/push
request is handled on a separate checkout of the latest remote main, preserving
the original acceptance worktree, evidence bytes and the dirty main workspace.

Pre-commit exact resolved-provider-value scans pass on Windows and WSL over
4679 observed files each. Byte-identified evidence remains under -text attributes;
no product binary or original ACL is added to the commit.
