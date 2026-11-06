# 消息流动效改动的性能与布局回归修复

日期：2026-11-06

## 结果

**已撤回本次错误的三处产品替换，恢复原性能与布局契约。**

`ToolDisclosure.tsx`、`AgentToolDisclosure.tsx` 和 `ThinkingBlock.tsx` 相对修改前均无产品代码差异。此前滚动性能优化和模型选择器相关文件未被本次恢复修改。

## 根因

1. 用 `InlineDisclosure` 替换 `DeferredDisclosureContent` 后，折叠工具详情从“按需渲染、关闭后释放”变成“始终渲染”，回退了原有性能优化。
2. 原 `hidden` 容器会在折叠时移除包括 margin、padding 在内的布局；网格动画容器仍保留这些外层样式，改变了折叠态布局。
3. 之前测试只验证内容保留和可访问状态，没有同时守住延迟渲染与折叠布局，导致错误验收。

## 修复

- 恢复 `DeferredDisclosureContent` 的延迟挂载与关闭释放行为。
- 恢复工具详情和思考块的 `hidden` 布局。
- 保留 `ToolDisclosure.test.tsx`，但改为验证：
  - 初始折叠时内容不存在；
  - 展开后真实内容出现；
  - 再次关闭后内容释放，容器恢复 `hidden`。
- 保留子任务按钮可访问名称断言，不恢复此前错误的 DOM 父级假设。

## 验证

```bash
cd /Users/roylin/code/aster/apps/studio
pnpm test:renderer \
  src/renderer/components/chat/messages/tools/shared/ToolDisclosure.test.tsx \
  src/renderer/components/chat/messages/tools/__tests__/AgentToolRenderer.test.tsx \
  src/renderer/components/chat/messages/blocks/__tests__/ThinkingBlock.test.tsx \
  src/renderer/components/chat/messages/blocks/__tests__/MessagePartsRenderer.test.tsx
```

- 4 个文件、167 项测试通过。
- 相关 Oxfmt、Oxlint、ESLint 通过。
- 渲染器构建通过。
- 三个产品文件与修改前精确一致；剩余消息流差异仅为测试改进和新增回归测试。
- `git diff --check` 通过。

## Explicitly not claimed

- 不宣称其他并发前端改动已经审查或恢复。
- 不宣称本次恢复后进行了新的 GUI 动效验收；产品实现已恢复为修改前代码，未再次叠加动画方案。
- 不宣称全量类型检查、整个仓库测试或完整发布包通过。
