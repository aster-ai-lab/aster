# R11 实际 bundle 调用链只读复核

与修复代理独立核对后，确认一个完整且可证伪的注册更新环：Workbench 主输入框在没有可用模型扩展名时，每次渲染生成新的 fallback 扩展名数组；附件 launcher 将数组身份变化转成重新注册，注册版本又直接驱动原组件渲染。普通焦点切换触发富文本 runtime 装载后，FooterActionsSync 的布局更新是实际 React 185 堆栈落点。

## 实际产物堆栈映射

从当前被测 `.studio-package` 直接读取压缩代码，无需另建或运行应用：

| 堆栈帧 | 对应实际代码 |
| --- | --- |
| `do.b300b640a9.js:1:64124` | `ComposerToolFooterActionsSync` 的 `useLayoutEffect` 调用 `updateFooterActions` |
| `do.b300b640a9.js:1:82355` | `QuickPanelProvider.updateFooterActions` 调用 `setFooterActions(actions)` |
| `s9z.955f177b8a.js:3:23626` | `MissingAgentHomeComposerInner`（压缩名 `s_`） |
| `s9z.955f177b8a.js:3:31275` | `MissingAgentHomeComposer`（压缩名 `sN`） |

实际 bundle 的 MissingAgent 内层保留 `H=c&&0===G.length?(0,nc.E)():G`，并在同一 host 条件下挂载 `AttachmentToolRuntime`、传入 `extensions:H`。因此本候选同时存在于对应源码和被测产物中。

## 最短环路

1. `AgentComposer.tsx:2703` 的 `useComposerToolLauncherVersion()` 直接订阅 launcher context。该 hook 在 `ComposerToolRuntime.tsx:370` 返回该 context 的 `version`。
2. `AgentComposer.tsx:2723` 在 host 存在、`modelSupportedExts.length === 0` 时调用 `agentFileExtensions()`；`useComposerFileCapabilities.ts:23` 使用 spread 构造新数组。无默认模型时，capability hook 明确返回空扩展名数组，满足此条件。
3. `AgentComposer.tsx:3090` 在 host 存在时确实挂载 `AttachmentToolRuntime`，并将上述数组作为 `extensions` 传入。
4. `AttachmentButton.tsx:63` 的 `openFileSelectDialog` 依赖 `extensions`；其身份改变会使 `AttachmentButton.tsx:87` 的 launcher 注册 effect 清理并重新注册。
5. `ComposerToolProvider.tsx:180` 和 `:184` 在注册、有效清理时分别递增 `launcherVersion`，返回步骤 1。

此闭环不需要 QuickPanel 全 context 的订阅才能成立。`useWorkbenchHomeComposer` 在 `WorkbenchHomeComposerExtras.tsx:153` 订阅 QuickPanel 是另一条返程路径；仅因 FooterSync 每次发布新的空数组，尚不足以单独证明无限循环。稳定 `updateFooterActions` 或空数组发布可能改变症状，不能替代对上述完整环路的验证。

主 AgentComposer 分支的 `AgentComposer.tsx:1087` 存在相同的 fallback 数组构造，应由修复代理一并检查。该分支的具体触发仍需独立用例；本次不把静态相似性当作已运行的接受结果。

## 证据范围

实际 bundle 和对应隔离源码的 SHA-256 保存在 [独立调用链身份](r11-independent-source-trace.json)。已将最短环路交给负责修复的 composer 代理，建议其使用真实 Provider、真实 AttachmentRuntime、真实 launcher-version 订阅复现并验证。

本次只读追踪；未修改前端、未运行或停止容器、未操作现有 GUI。真实 DOM 行为和修复后的回归结果由主任务与修复代理另行记录。
