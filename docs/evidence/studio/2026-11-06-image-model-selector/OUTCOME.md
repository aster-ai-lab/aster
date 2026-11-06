# 图片布局的唯一模型选择弹窗与思考强度滑杆

## 结果

- 模型选择弹窗统一使用图片中的 384px 布局：标题与当前模型、关闭按钮、搜索、提供商 tabs、分组模型列表和左侧详情卡。
- 不再使用另一套 SelectorShell 外观承载模型选择；共享数据、选择、键盘导航和运行时恢复逻辑保留。
- 详情卡直接包含 `ReasoningEffortChoice` 滑杆，无第三级档位菜单。
- ACP 模型继续通过 `additionalCatalog` 进入同一弹窗，运行时发布的推理档位保持权威，包括 `ultracode`。
- 多选、空选项、运行时错误、空目录和重试均留在同一弹窗结构中。

## 修改范围

- `apps/studio/src/renderer/components/ModelSelector/ModelSelector.tsx`
- `apps/studio/src/renderer/components/ModelSelector/ModelSelectorDetailCard.tsx`
- `apps/studio/src/renderer/components/ModelSelector/types.ts`
- `apps/studio/src/renderer/components/ModelSelector/__tests__/ModelSelector.test.tsx`
- `apps/studio/src/renderer/components/ModelSelector/__tests__/ModelSelectorDetailCard.test.tsx`
- `apps/studio/src/renderer/components/ModelSpeedControl.tsx`
- `apps/studio/src/renderer/pages/agents/WorkbenchModelPicker.tsx`
- `apps/studio/src/renderer/pages/agents/__tests__/WorkbenchModelPicker.popup.test.tsx`
- 删除不再使用的 `modelPickerFloating.tsx` 及其测试。

## 验证命令与结果

- 联合渲染回归：7 个测试文件、105 个用例全部通过。
- 最终共享弹窗与工作台复验：27 个用例全部通过。
- 定向 `oxlint --deny-warnings`：0 个错误、0 个警告。
- 定向 `oxfmt --check`：全部通过。
- `pnpm typecheck:web`：仓库其他模块仍有既有错误；最终输出不包含本次修改文件。
- `STUDIO_RENDERER_DIST=image-model-selector pnpm exec rsbuild build`：通过。
- `git diff --check`：通过。

## 真实窗口核验

- 启动隔离 Aster 窗口，加载 `.studio/dist/image-model-selector`。
- 模型列表与详情卡均采用图片中的布局和位置。
- 思考强度滑杆直接显示在详情卡内，没有第三级菜单。
- 截图：[verified.png](verified.png)。
- 测试实例已关闭。

## 明确未声明

- 未声明全量类型检查、全量渲染测试或完整原生构建通过。
- 未修改 ACP 绑定、模型保存协议或模型目录后端。
- 未提交代码。
