# 最新消息按钮布局与文案

日期：2026-11-06

## 结果

- 保留原“回到底部”的滚动行为，将悬浮按钮从消息流右侧改为输入框顶部外侧居中。
- 按钮高度从 36px 降为 28px，图标从 20px 降为 14px，并在图标右侧显示“最新消息”。
- 继续复用输入框实际占位计算的底部偏移，不改变滚动定位或输入框布局。
- 新增中英文文案：`最新消息` / `Latest messages`。

## 验证

```bash
cd /Users/roylin/code/aster/apps/studio
pnpm test:renderer \
  src/renderer/components/chat/messages/list/__tests__/MessageVirtualList.test.tsx \
  src/renderer/components/chat/messages/__tests__/MessageList.test.tsx
pnpm test:renderer src/renderer/components/chat/messages/list/__tests__/MessageVirtualList.test.tsx
```

- 按钮与消息列表联合测试：2 个文件，61／61 通过。
- 格式化后的按钮定向复测：31／31 通过。
- 回归断言覆盖居中悬浮层、28px 高度、图标与“最新消息”文字、输入框偏移及点击滚动行为。
- 对本次 TypeScript 文件运行 Oxlint：0 warnings、0 errors；ESLint：0 errors；Oxfmt check：通过。
- `git diff --check`：通过。

## Explicitly not claimed

- 不宣称完整 `pnpm i18n:check` 通过；它仍被 8 条本次之外的既有 `Aster` 文案规则错误阻断。新增的中英文 key 已同步，其他排序与缩进附带改动已撤回。
- 不宣称已进行 Studio GUI、完整构建、全量类型检查或发布验收；本次验收为渲染器组件测试和定向静态检查。
