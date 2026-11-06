# 任务消息流组件连续性与 ArcUI 动效

> 后续更正：本次 `InlineDisclosure` 替换撤回了原有的延迟挂载优化，并改变了折叠态间距布局。三处产品实现已全部恢复，本文不再代表当前代码状态。根因与恢复验证见 `2026-11-06-message-motion-regression/OUTCOME.md`。

日期：2026-11-06

## 结果

**实现及已执行的验收完成。** 本次没有给所有消息组件机械增加入场动画，而是统一关键内联详情的状态连续性：触发器保持原有职责，内容按需展开，反向操作不卸载内容，折叠内容立即退出可访问与交互树。

保留了现有消息虚拟化、滚动位置记忆、展开状态存储、滚动锚点和业务状态机制。未修改模型选择器、思考强度控件或其待办文件。

## 修改

- `ToolDisclosure.tsx`
  - 删除仅用于延迟挂载内容的 `DeferredDisclosureContent`。
  - 使用现有 `InlineDisclosure`，不再通过即时 `hidden` 切换内容。
- `AgentToolDisclosure.tsx`
  - 工具详情使用同一 `InlineDisclosure`。
  - 保留 `contentId`、展开状态和滚动锚点。
- `ThinkingBlock.tsx`
  - 思考详情使用同一 `InlineDisclosure`。
- 测试
  - 新增 `ToolDisclosure.test.tsx`，验证折叠内容保留挂载，同时具有 `inert` 与 `aria-hidden`。
  - 更新工具和思考块测试，区分“保留在 DOM”与“可见、可交互”。
  - 子任务完成状态改为验证按钮的真实可访问名称，不再依赖错误的 DOM 父级假设。

共享 `InlineDisclosure` 使用 `grid-rows-[0fr]` 到 `grid-rows-[1fr]`、透明度和 400ms 无回弹过渡；折叠时立即设置 `inert` 和 `aria-hidden`，并包含 `motion-reduce:transition-none` 与 `motion-reduce:delay-0`。

## 自动验证

最终源码结果：

- 消息流四套测试：**4 个文件，167 项通过**。
  - `ToolDisclosure.test.tsx`：1 项。
  - `AgentToolRenderer.test.tsx`：55 项。
  - `ThinkingBlock.test.tsx`：19 项。
  - `MessagePartsRenderer.test.tsx`：92 项。
- 真实 UI 包原生测试入口：**1 项通过**，覆盖折叠时立即退出交互树、反向展开保留输入状态。
- 相关文件 Oxfmt、Oxlint、ESLint：通过，无错误或警告。
- `git diff --check`：通过。
- 渲染器构建：通过。
- `ASTER_DEFER_RUNTIME_COMPONENTS=1 just build-studio`：通过，Rust `dev` profile 完成。
- 验收环境：macOS arm64，Node.js 24.22.1；未执行 Windows 或 Linux 原生验证。

主要命令：

```bash
cd /Users/roylin/code/aster/apps/studio
pnpm test:renderer \
  src/renderer/components/chat/messages/tools/shared/ToolDisclosure.test.tsx \
  src/renderer/components/chat/messages/tools/__tests__/AgentToolRenderer.test.tsx \
  src/renderer/components/chat/messages/blocks/__tests__/ThinkingBlock.test.tsx \
  src/renderer/components/chat/messages/blocks/__tests__/MessagePartsRenderer.test.tsx

cd /Users/roylin/code/aster/packages/ui
pnpm test src/components/primitives/__tests__/inline-disclosure.test.tsx

cd /Users/roylin/code/aster/apps/studio
ASTER_TENANT_SLUG=aster STUDIO_RENDERER_DIST=.studio/dist/message-motion-arc \
  node node_modules/@rsbuild/core/bin/rsbuild.js build

cd /Users/roylin/code/aster
ASTER_DEFER_RUNTIME_COMPONENTS=1 just build-studio
```

## 真实窗口验收

使用当前源码构建的独立 `fork7` Studio 实例、隔离 HOME 和确定性本地模拟器。未使用用户真实会话。

### 思考块

1. 模型返回真实 Responses reasoning summary 事件，窗口生成思考块。
2. 折叠时标题显示摘要；展开后标题保持原位，同一思考内容在内联区域出现。
3. 在固定触发位置间隔 120ms 执行收起、重新展开；最终 `aria-expanded` 对应状态为展开，完整思考内容仍保留。
4. 截图显示内容区域连续展开，没有整块替换或新增另一套组件。

证据：

- [思考块折叠](artifacts/thinking-closed.png)
- [思考块展开](artifacts/thinking-open.png)
- [思考块快速反向展开](artifacts/thinking-rapid-reopen.png)
- [展开状态记录](artifacts/thinking-open-snapshot.json)
- [快速反向状态记录](artifacts/thinking-rapid-reopen-snapshot.json)

### 工具详情

1. 模型发起写入工具调用，窗口生成工具卡片。
2. 展开后显示同一份 JSON 输入和错误详情。
3. 收起后通过最新触发器引用重新展开，JSON 输入、错误详情及展开状态均保留。
4. 宿主拒绝未授权写入，错误始终可读；模拟器的后续文本不被当作实际写入成功证据。
5. 收起时底部跟随会重新定位卡片，因此一次基于旧坐标的第二次点击未命中；该次操作未被记为快速反向通过，最终验收使用最新语义引用完成。

证据：

- [工具详情折叠](artifacts/tool-collapsed.png)
- [工具详情展开](artifacts/tool-open.png)
- [工具详情重新展开](artifacts/tool-reopened.png)
- [工具展开状态记录](artifacts/tool-open-snapshot.json)
- [工具重新展开状态记录](artifacts/tool-reopened-snapshot.json)

真实窗口启动使用：

```bash
STUDIO_RENDERER_PORT=44363
STUDIO_RENDERER_DEV=0
STUDIO_RENDERER_DIST=.studio/dist/message-motion-arc
STUDIO_COMPANIONS_STAGED=1
ASTER_DEFER_RUNTIME_COMPONENTS=1
ASTER_STUDIO_INSTANCE=fork7
```

验收结束后，本任务启动的 Studio、渲染预览和模拟器均已关闭；44363 与 58516 端口无残留监听。

## 减少动效边界

最终构建 CSS 明确包含：

```css
@media (prefers-reduced-motion:reduce) {
  .motion-reduce\:transition-none { transition-property: none; }
  .motion-reduce\:delay-0 { transition-delay: 0s; }
}
```

当前机器没有 Playwright Chromium 或可复用 Chrome。为避免下载额外运行时或改变用户的 macOS 全局减少动效设置，本次未执行浏览器媒体仿真或桌面级减少动效实测。自动测试覆盖状态连续性，构建产物确认减少动效规则存在；不把这两者表述为桌面减少动效实测通过。

## Explicitly not claimed

- 不宣称所有消息组件都需要或已经加入新的入场动画；本次统一的是关键内联展开行为。
- 不宣称基于帧率或耗时基准的性能百分比提升。
- 不宣称 macOS 系统级减少动效、Windows 或 Linux 原生行为已经实测。
- 不宣称完整发布包或可选沙箱组件通过发布验收；原生构建使用延迟可选组件模式。
- 不宣称全量 TypeScript 检查、整个仓库测试或所有 GUI 流程通过。
- 不宣称工具写入获得授权或实际落盘；本场景验证的是消息流展开、状态保留和错误可读性。
- 未修改另一进程负责的模型选择器相关文件。
