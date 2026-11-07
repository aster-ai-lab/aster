# 模型选择器重复滚动条修复

日期：2026-11-06

## 结果

- 模型列表外层固定高度容器不再单独滚动；保留虚拟列表这一条纵向滚动。
- 修正外层 padding、列表行间距与虚拟列表高度的合计，消除原有的 8px 高度差。
- 模型详情卡仅保留必要的内容纵向滚动，禁止横向滚动，不再显示底部横向滚动条。
- 模型选择、键盘导航与思考强度滑块行为保持不变。

## 验证

```bash
cd /Users/roylin/code/aster/apps/studio
pnpm test:renderer \
  src/renderer/components/ModelSelector/__tests__/ModelSelector.test.tsx \
  src/renderer/components/ModelSelector/__tests__/ModelSelectorDetailCard.test.tsx \
  src/renderer/components/__tests__/ModelSpeedControl.ui.test.tsx
pnpm test:renderer src/renderer/components/ModelSelector/__tests__/ModelSelectorDetailCard.test.tsx
```

- 模型选择器、详情卡与滑块联合测试：3 个文件，47／47 通过。
- 格式化后的详情卡复测：8／8 通过。
- 回归断言覆盖单一列表滚动容器、修正后的虚拟列表高度，以及详情卡 `overflow-y-auto` / `overflow-x-hidden`。
- 对本次 TypeScript 文件运行 Oxlint：0 warnings、0 errors；ESLint：0 errors；Oxfmt check：通过。
- `git diff --check`：通过。

## Explicitly not claimed

- 不宣称已进行 Studio GUI、完整构建、全量类型检查或发布验收；本次验收为渲染器组件测试和定向静态检查。
