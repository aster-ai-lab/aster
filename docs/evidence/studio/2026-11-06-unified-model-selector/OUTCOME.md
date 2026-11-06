# 统一模型选择弹窗

## 结果

- 工作台不再维护独立的模型列表、详情卡或第三级推理强度菜单。
- `ModelSelector` 是唯一的模型选择弹窗实现；`WorkbenchModelPicker` 仅负责 ACP 目录适配、模型选择保存和运行时恢复。
- 工作台运行时模型通过 `additionalCatalog` 进入共享选择器，并按 Studio 模型 ID 提供各模型的推理档位。
- 多档模型在共享详情卡中复用 `ReasoningEffortChoice` 滑杆；少量档位仍使用单选。ACP 发布能力保持权威，包括 `ultracode`。
- 加载、空目录、绑定失效、错误说明和重试复用共享弹窗的空状态区域。
- 已删除不再使用的 `modelPickerFloating.tsx` 及其测试。

## 修改范围

- `apps/studio/src/renderer/components/ModelSelector/ModelSelector.tsx`
- `apps/studio/src/renderer/components/ModelSelector/ModelSelectorDetailCard.tsx`
- `apps/studio/src/renderer/components/ModelSelector/types.ts`
- `apps/studio/src/renderer/components/ModelSelector/__tests__/ModelSelectorDetailCard.test.tsx`
- `apps/studio/src/renderer/components/ModelSpeedControl.tsx`
- `apps/studio/src/renderer/pages/agents/WorkbenchModelPicker.tsx`
- `apps/studio/src/renderer/pages/agents/__tests__/WorkbenchModelPicker.popup.test.tsx`
- 删除 `apps/studio/src/renderer/pages/agents/modelPickerFloating.tsx`
- 删除 `apps/studio/src/renderer/pages/agents/__tests__/modelPickerFloating.test.ts`

## 验证命令与结果

- 联合渲染测试：7 个测试文件、108 个用例全部通过。
  - 工作台弹窗 12 项；共享详情卡 8 项；共享选择器 18 项；目录数据 21 项；速度控件 21 项；目录辅助函数 9 项；工作台 Composer 19 项。
- Composer 推理强度与保存相关测试：16 项通过。
- 定向 `oxlint --deny-warnings`：0 个错误、0 个警告。
- 定向 `oxfmt --check`：全部通过。
- `pnpm typecheck:web`：仓库其他模块仍有大量既有错误；最终输出不包含本次修改文件。
- `STUDIO_RENDERER_DIST=unified-model-selector pnpm exec rsbuild build`：通过。
- `git diff --check`：通过。

## 真实窗口核验

- 启动隔离 Aster 窗口，加载 `.studio/dist/unified-model-selector`。
- 通过模型按钮打开共享模型列表，悬停 K3.1 Pro 后显示共享详情卡。
- 思考强度直接显示为滑杆；不存在第二套弹窗或第三级档位菜单。
- 截图：[verified.png](verified.png)。
- 测试实例已关闭。

## 外部重置与恢复

14:59 工作区被外部 `reset: moving to HEAD` 重置，未提交的实现一度被覆盖。已按同一验证差异恢复全部模型选择器修改，并重新完成上述 108 项测试、静态检查、构建和真实窗口核验。其他并行文件修改未纳入本任务，也未主动回退。

## 明确未声明

- 未声明全量类型检查、全量渲染测试或完整原生构建通过。
- 未修改模型保存协议、ACP 绑定规则或模型目录后端。
- 未提交代码。
