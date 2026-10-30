# JSON Render 端到端测试

覆盖真实链路：本地模型发出 `ask_user_form` → a3s-code Core 执行工具 → ACP
请求输入 → 桌面渲染 `root/elements/state` → 宿主核对绑定 → 原工具调用恢复。
普通 `ask_user` 使用另一套 `question` 协议，不能代替 JSON Render 验收。

## 用例与通过条件

| 优先级 | 场景 | 必须核对的结果 |
| --- | --- | --- |
| P0 | 提交共享 `fixtures/form.json` | Name=Ada、Amount=4（number）、Ready=true（boolean）、Choice=alpha、Notes；同一 tool_call_id 收到 `a3s.code.user-form.v1`，`permission_grant=false` |
| P0 | 空值与范围校验 | 空 Name、未勾选必填 Ready、Amount=6 阻止提交；修正后仍可提交；等待期间没有工具答案或权限授权 |
| P0 | 取消等待 | 点击停止，活动执行结束；绑定不能提交；不能产生成功答案 |
| P0 | 错误绑定与重放 | 错 requestId、revision、owner 都返回 `ok=false` 且不消费合法请求；成功后重复原绑定也被拒绝 |
| P0 | 旧 OpenUI 输入 | `root = Input(...)` 返回 `invalid JSON Render document:`；桌面不出现可操作的 JSON 表单 |
| P0 | 历史与重启 | 已回答表单保留真实值且只读；进程退出时未回答的请求失效，重启不能恢复成可提交表单 |
| P1 | 流式重绘与切换任务 | 未结束 JSON 不执行；完整文档结束、重绘、切换回来不清空草稿；不同会话不能串答 |
| P1 | 原生边界拒绝 | 未知组件、动作、坏绑定、超限与循环节点拒绝；不退回 OpenUI 或执行表达式 |

每条保存模型的脱敏工具回执、宿主状态、AX 观察与窗口截图。只看到表单或
“已发送”不算通过。校验数据类型、工具身份和单次提交，不能只比较界面文案。
确定性用例是日常回归；外部真实模型是独立补充验收，失败时保留原始工具入参。

## 本地模型与已有终端回归

从根目录执行，不需要外部账号、API key 或新增依赖：

```sh
python3 -m unittest discover -s tests/e2e -p test_json_render_model.py -v
python3 tests/e2e/json_render_model.py --events target/json-render-model/events.jsonl
```

第二条输出 loopback `baseUrl`。在隔离的 Aster 测试实例中将 OpenAI 兼容提供商
指向这个地址，配置任意合成 key、模型 `json-render-cua`，显式选中该模型。
用 `JSON-RENDER-CUA-SUBMIT`、`JSON-RENDER-CUA-CANCEL` 或
`JSON-RENDER-CUA-INVALID` 开头的测试消息分别触发三个场景。模型只提供工具
调用和判定回执，不模拟 ACP、注入 DOM 或代替宿主提交答案。

终端已有 `ask-form.acl`，覆盖必填、范围、提交、取消和拒绝；由真实可执行文件夹具运行：

```sh
a3s-test check tests/e2e/ask-form.acl --json
cargo build -p aster -p a3s-code-acp
cargo test -p a3s-code-acp --test shipped_tui_e2e shipped_native_form_submit_cancel_and_reject -- --ignored --exact --nocapture
```

运行前从根目录构建匹配的 `aster` 和 `a3s-code-acp`。不要混用历史安装包。

## 桌面后台 CUA

使用 `a3s-test mcp` 的 GUI agent session；每次观察后执行一个动作，再重新观察。
由宿主绑定隔离的测试 bundle、PID、窗口和 profile，不操作用户的主实例。
安装版 A3S Test 与 CUA 的兼容、权限及后台投递必须先核对。

输入采用语义点击字段，再向该窗口已聚焦的编辑上下文输入。
WebKit 的 AXValue 赋值会被忽略，不能用 `fill` 的成功回执证明字段已修改。
当前 CUA 的带字段 token 文本操作可能经指针重新聚焦；后台代理应保留窗口绑定、
拒绝坐标/前台动作，并把文本与按键限制为不重新聚焦的后台投递。
每步检查实际界面，最终检查工具回执。富文本编辑器如果未接收文本，立即记录为
输入失败；不能重复发送后宣称 GUI 提示词已提交。补充用例允许通过宿主公开的
`POST /api/agent-sessions/{id}/turns` 准备真实请求，但必须注明 API 准备与 GUI 操作的边界。

已观察过的表单交互提取为 `json-render-desktop.acl`。它的前置条件是：中文界面、
默认值未修改的共享表单已由真实 `JSON-RENDER-CUA-SUBMIT` 工具请求打开，且桌面
后台代理已经配置；它不负责启动模型、配置提供商或在富文本编辑器中发送首条消息。
使用宿主固定的 GUI 配置运行，禁止为通过而放开前台/坐标动作：

```sh
a3s-test check tests/e2e/json-render-desktop.acl --json
```

`check` 只证明协议有效，不证明桌面执行通过。实际执行命令及后台代理必须随
工作机部署配置记录；每次结束 session，检查只清理自己拥有的桌面与 ACP 进程。

本次执行证据见 [OUTCOME](../../docs/evidence/desktop/2026-10-23-json-render-cua/OUTCOME.md)。
