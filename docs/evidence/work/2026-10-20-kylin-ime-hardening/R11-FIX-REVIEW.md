# R11 最小修复独立复核

未发现新的确定缺陷或兼容性阻断。把 Agent fallback 扩展名列表保存在模块静态数组中，能够消除已经确认的注册版本闭环；顺序、内容和现有模型能力分支均保持原语义。复核未修改产品源码，未运行容器或 GUI。

## 所有共享数组调用点

生产环境只有 `AgentComposer.tsx:1087` 与 `:2723` 两处调用 `agentFileExtensions()`。已沿两处传值继续追到：

| 下游 | 对扩展名数组的操作 |
| --- | --- |
| `ComposerToolDerivedStateProvider`、attachment tool 定义 | 透传并作为 memo 依赖；不修改数组 |
| `AttachmentToolRuntime` | 读取 `length`；文件对话框用 `map` 创建新过滤数组；大集合传给文件过滤函数 |
| `useQueuedWorkbenchFiles` | ref 保存当前参数；传给文件过滤函数 |
| `ComposerSurfaceRuntime`、`composerPaste` | `includes` 及传给粘贴/拖放处理 |
| `pasteHandling`、`useFileDragDrop`、`filterSupportedFiles` | `includes`、`new Set`；仅修改新建的结果集合和附件数组 |

全仓搜索并追踪这些路径后，没有发现对该输入数组的原地排序、增删、填充、索引赋值或 `Object.assign`。搜索到的 table 扩展包 `extensions.push(...)` 操作的是另一个局部 Tiptap 扩展集合，与文件扩展名无数据流关系。

`useComposerFileCapabilities` 对存在模型的 Agent 仍在 `useMemo` 中返回 `[...ALL_FILE_EXTS]`；Chat 的多模态门控也未改变。此次只有无模型、Workbench 可先暂存附件的 fallback 改为共享静态列表，不会把 Chat 的能力结果改成 Agent 全集合。

## 维护回归确实经过闭环

新增 `ComposerSurface.lifecycle.test.tsx` 使用真实 `QuickPanelProvider`、`ComposerToolRuntimeProvider`、`AttachmentToolRuntime`、`ComposerSurfaceRuntime` 和 launcher-version context 订阅。组件没有用 mock setter、固定版本或截断的注册函数来停止环路；测试文件只固定 translation 函数，现有 renderer setup 也未 mock 这些生命周期组件。

测试在无模型时走真实 fallback 扩展名函数，实际注册附件 launcher，并等待真实 Tiptap editor。随后通过真实编辑器 transaction 插入文本，再用普通 Enter 验证提交。`quickPanelEnabled={false}` 关闭结果面板，但不移除 runtime 内实际 `ComposerToolFooterActionsSync`，因此本次出错的发布链仍存在。

[原代码收据](composer-lifecycle-before.log) 明确在 `QuickPanelProvider.tsx:107`、`ComposerToolRuntime.tsx:218` 报同一个 Maximum update depth 错误；[修复后收据](composer-lifecycle-after.log) 的该项通过。[相关回归收据](composer-lifecycle-regression.log) 为 5 个文件、33 项通过。本次独立复核读取这些收据，未重复执行测试；scoped `git diff --check` 通过。

## 模型切换边界

无模型条件已经由新增回归直接覆盖。`undefined → 稳定模型对象 → undefined` 会在模型改变时使既有 capability `useMemo` 有界重算，再回到同一个静态 fallback 数组；已核对的下游没有依赖每次取得独立可变副本。没有证据表明还需额外生产修复。

当前维护用例未执行模型切换；同一真实 Provider harness 补这条切换可以增加边界覆盖，但不是本次已发现、必须修复的欠缺。共享列表的 `string[]` 类型仍允许将来的调用者修改它；当前已追踪调用者都不这样使用，本次未扩大到全链路 readonly 类型改造。

修复源码、维护用例、被追踪关键组件及前后日志的本次 SHA-256 保存于 [复核身份](r11-independent-fix-review.json)。真实最终包 GUI、DOM 键态和最终安装载荷验证由主任务另行记录。
