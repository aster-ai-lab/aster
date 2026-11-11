# 推理强度滑杆档位丢失修复（2026-11-10）

## 1. 问题与根因

用户反馈「模型选择弹窗里选择推理强度的滑杆又没了」。在本次 fresh build 的真窗口（`w-52438`）上复核：打开模型选择器后 167 个 AX 节点中 **0 个 `slider`、0 个「推理强度 / effort」相关节点**。

定位到的链路缺口，不是组件被删除：

- `src/renderer/pages/agents/WorkbenchModelPicker.tsx` 的 `acpPickerGroups()` 只从 ACP 运行时目录读取 `_meta.reasoningEfforts`（`acpModelEfforts()`），`reasoningEfforts: efforts` **没有回退到本地模型记录**；而同一个 option 的 `contextWindow` / `outputLimit` 都写了 `?? stored?.*` 兜底。
- `reasoningEffortLadder(_modelId, published)` 明确「Missing metadata never invokes model-family inference」，参数 `_modelId` 未被使用；空 vocabulary 直接得到空阶梯。
- `ModelSelectorDetailCard.tsx:154-160` 在 `runtimeReasoningEfforts === []` 时把 `effortOptions` 置为 `undefined`，**不会**回退到 `deriveThinkingOptions(model)`；随后 `hasCapabilityDetails` 为 false，整行「推理强度」不渲染。
- 两处叠加的结果：走 ACP 运行时目录时，只要运行时没发布 vocabulary，即使本地模型记录里显式保存了 `reasoning.selectableEfforts`，滑杆也会消失。

`src/renderer/data/modelProviders.ts:32` 的字段注释仍写着「Absent means infer from the model id」，与上述实现早已分叉（`git log -S 'Missing metadata never invokes'` 指向 `917d9bc4b1`，一次 worktree-preservation 落地）。

## 2. 改动

按「fail closed、按名字猜含义不可用」的原则，**只回退到显式存储的配置，不恢复模型族推断**。

| 文件（`apps/studio` 子模块内） | 改动 |
| --- | --- |
| `src/renderer/pages/agents/WorkbenchModelPicker.tsx` | `acpPickerGroups()` 中，ACP 未发布 vocabulary 时用 `deriveThinkingOptions(stored)` 兜底；发布 vocabulary 时仍以发布值为准 |
| `src/renderer/data/modelProviders.ts` | 修正 `reasoningEfforts` 字段的过时注释 |
| `src/renderer/pages/agents/__tests__/WorkbenchModelPicker.local.test.ts` | 新增 3 条回归断言：无发布但有显式存储 → 保留阶梯；两者皆无 → 仍为空（fail closed）；有发布 → 发布值权威 |

`git diff --stat`：**3 files changed, 59 insertions(+), 4 deletions(-)**；`git diff --check` EXIT=0。

## 3. 实际验证（命令与计数）

| 检查 | 命令 | 结果 |
| --- | --- | --- |
| 新增回归断言 | `cd apps/studio && node_modules/.bin/vitest run --project renderer src/renderer/pages/agents/__tests__/WorkbenchModelPicker.local.test.ts` | **4 tests passed** |
| 相关既有套件 | 同上，加 `WorkbenchModelPicker.popup.test.tsx`、`ModelSelectorDetailCard.test.tsx`、`ModelSpeedControl.ui.test.tsx` | **4 files / 49 tests passed**；其中 `offers only the ACP effort vocabulary and never an inferred model-family ladder` 保持通过 |
| 性能布局契约（等价入口） | `cd apps/studio && node_modules/.bin/vitest run --project renderer <scripts/perf-layout-contracts.json 的 9 个路径>` | **9 files / 185 tests passed** |
| 官方契约门禁 | `cd /Users/roylin/code/aster && just studio-contracts` | **EXIT=1**：`ERR_PNPM_UNSUPPORTED_ENGINE`，`wanted {"node":">=24.12.0"} (current {"node":"22.22.2"})`。环境限制，故用上一行等价入口实跑同一批 9 个文件 |
| renderer 类型检查 | `cd apps/studio && node --max-old-space-size=8192 node_modules/typescript/bin/tsc --noEmit -p tsconfig.web.json --composite false` | **EXIT=1，311 个 `error TS`**，分布在 `composerModeHome.e2e.test.tsx`(32)、`migrationV2/*`、`workInbox/*` 等既有文件；**本次 3 个改动文件 0 错误**。子模块改动前为干净 `42303bb224`，这些失败为预存在 |

四元组（本次改动相关）：通过 49 + 185（契约，含与相关套件的重复文件，分别计数）；失败 0；跳过 0；未执行：`just studio-contracts` 官方入口（环境阻断，已用等价入口覆盖）、`pnpm typecheck` 全量（pnpm engine 阻断，已用等价 tsc 命令覆盖）。

失败分类：本次改动引入 0；既有/环境 2（pnpm Node 引擎、renderer 全量类型检查的 311 个预存在错误）。

## 4. 边界与结论

- **当前选中的 `deepseek-flash` 仍不会显示滑杆，这是正确行为**：它不是推理模型，本地也没有 `reasoning.selectableEfforts`，按 `ModelSpeedControl.tsx:303` 的 `return null` 与 `canUseReasoningEffortSlider()` 的 >1 档要求应当隐藏。本次修的是「**本地已显式配置档位、却因 ACP 运行时未发布而被抹掉**」这一真实缺陷，不是把不支持的模型硬开出滑杆。
- 想在当前窗口看到滑杆：换用带 `reasoning.selectableEfforts`（≥2 个有效档位）的模型，或在模型配置中声明档位。
- `ModelSelectorDetailCard.tsx` 对 `runtimeReasoningEfforts === []` 不回退到 `deriveThinkingOptions(model)` 是另一处独立防御缺口，本次未改（扩大范围前需单独评估）。当所有调用方都经 `acpPickerGroups` 时，源头的兜底已覆盖该路径。
- 未做「模型不支持时给出明确说明」的 UI 提示（需新增 i18n key 并过 `i18n:hardcoded:strict` 门禁），如需要另起一轮。

## 5. Explicitly not claimed

- **未声称 `deepseek-flash` 现在会显示推理强度滑杆**；未改变该模型的档位配置。
- **未声称 `just studio-contracts` 门禁通过**：官方入口实际 EXIT=1，通过的是等价 vitest 入口实跑的 185/185。
- **未声称 renderer 类型检查通过**：实际 311 个预存在错误，仅能说明本次 3 个改动文件无新增类型错误。
- **未声称真窗口行为已重跑验证**：本次只做了代码与单测验证，没有重新构建并驱动窗口确认滑杆出现。
- **未声称修复了模型族推断**：推断仍被禁用，兜底只读取显式存储的配置。
- **未声称已落地交付**：`apps/studio` 子模块当前为**脏树**（3 个文件），未 commit、未 push、未 bump 父 gitlink；按 `AGENTS.md:94-98` 与 `studio-land-discipline.mdc` 需要推送子模块后由父仓库 bump，此步骤待用户授权。
- **未声称本文件之外的任何检查通过**。
