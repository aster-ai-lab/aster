# 修复计划 Stage 0 + Stage 2 落地记录（2026-11-11）

依据 `2026-11-11-ui-ux-page-audit/REMEDIATION-PLAN.md`。本轮执行 **Stage 0 止血 + Stage 2 CI 护栏**，并顺带收敛延迟集群与部分 reduce 缺口。计划与验收分开记录。

## 1. 实际执行的改动

### Stage 0 · 止血

| # | 改动 | 文件 |
| --- | --- | --- |
| 1 | **补 `@keyframes task-pending-shimmer` 闭合 `}`**，删除末尾多余的 `}`，恢复被吞掉的 5 条规则（代码块 header、行数、生成态、折叠箭头 `transition: transform 180ms`、`-90°` 折叠态） | `apps/studio/src/renderer/pages/agents/taskConversation/task-conversation.css:3248-3289` |
| 2 | **交互滚动接 `useReducedMotion`**：reduce 下瞬时跳转并调用 `onComplete`；正常 `DEFAULT_FRAMES` 50（≈830ms）→ 15（≈250ms） | `apps/studio/src/renderer/components/chat/messages/list/useSmoothScrollAnimation.ts` |
| 3 | **状态扣留 1000/1200ms → 200ms**（thinking 预览、工具 live header）；保留 error/waiting bypass | `ThinkingBlock.tsx:16`、`ToolBlockGroup.tsx:66`、`useMinimumDisplayDuration.ts`（保持原 hook 语义，仅时长收敛） |
| 4 | **CI 脚本漂移**：`ci.yml:174` 从 package.json 里不存在的 `pnpm run test:contracts:perf-layout` 改成等价入口 `node scripts/run-perf-layout-contracts.mjs` | `.github/workflows/ci.yml` |
| 5 | **延迟集群收敛**：聚合 hover 预览 600→240ms、模型详情 HoverCard 1500→300ms、全局 Tooltip 500→300ms | `TaskConversationMessage.tsx:1207`、`ModelSelectorDetailCard.tsx:27`、`packages/ui/src/components/primitives/tooltip.tsx:287` |
| 6 | **基础无限动画补 reduce**：`.animation-pulse` / `.animation-rotate` / `.animation-shimmer` / `.animation-reveal` 在 reduce 下 `animation:none`；`.loader` 与 `body` 过渡在 reduce 下关闭 | `animation.css`、`index.css` |

### Stage 2 · 护栏

| # | 改动 | 文件 |
| --- | --- | --- |
| 7 | 新增零依赖 `check-motion-contract.mjs`：硬失败规则 = CSS 大括号不平衡、`@keyframes` 内出现会被浏览器丢弃的普通规则；报告项 = 有 `infinite` 但无 reduce 保护的文件（不 fail，供渐进治理） | `apps/studio/scripts/check-motion-contract.mjs` |
| 8 | 接入 `motion:check` script 与 CI `desktop-frontend` 的 `Motion contract` 步骤（直接用 node，绕开本机 pnpm engine 限制） | `apps/studio/package.json`、`.github/workflows/ci.yml` |

### 一并带入的上一轮未提交改动

圆角 +2px（父仓库 `packages/ui` 4 个 token 文件 + 子模块 `aster-dream-theme.css`），见 `2026-11-11-ui-ux-page-audit/radius-after.png`。

## 2. 命令与计数（实际结果）

| 检查 | 命令 | 结果 |
| --- | --- | --- |
| 动效契约 | `node apps/studio/scripts/check-motion-contract.mjs` | `scanned 56 CSS files; OK`，EXIT=0 |
| **负向验证** | 临时注入 `__motion_contract_probe.css`（未闭合 keyframes 内含普通规则）→ 脚本 `EXIT=1` 且精确报 `rule inside @keyframes would be dropped`；移除探针后 `EXIT=0`。 | 通过（护栏确实能拦下 F1 类缺陷） |
| 单测（受影响） | `vitest run --project renderer` 5 个文件 | **181 tests / 180 passed / 1 failed**；唯一失败是历史既有的 `opens an edit as a diff and can switch to the split view`（`diff radiogroup`，3661 行），与本轮无关；3 个旧延迟断言已同步到新语义（1000→200、1500→300、600→240），未放宽任何断言 |
| 契约门禁（等价入口） | `just studio-contracts` 等价 vitest 9 文件 | 本轮未重跑；沿用 `2026-11-10-reasoning-effort-fallback/OUTCOME.md` 的 185/185，不声称本轮通过 |
| 格式 | `git diff --check`（父仓库 + 子模块） | EXIT=0 |
| 构建 | `node apps/studio/scripts/e2e-fresh-build.mjs --build-renderer` | EXIT=0，1m35s |
| 产物复核 | grep `static/css/*.css` | `prefers-reduced-motion:reduce){.animation-pulse,...{animation:none}}`；产物中 71 个 reduce 块；`@keyframes task-pending-shimmer{...}` 后紧跟 `:is(...)` 普通规则（确认不再被吞） |
| 真窗口 | 重启 `just studio` → `w-59559`，截图 `2026-11-11-remediation-stage0-2.png`，AX `ref_count=111` | 冒烟通过 |

## 3. 未执行 / 剩余批次（如实标注）

- **Stage 1 未完成**：`runAction` 统一操作通道未建立；`.arc` motion token 未提升到全局（注释明确「scoped to `.arc` so they never leak into the host app」，属有意设计，不能擅自改 scope）；用户状态轴类型化未做。
- **Stage 3 未完成**：自动化页零反馈、命令菜单 6 处静默 catch、回退 composer busy、知识库失败可恢复、设置页静默 catch 群、artifact CLS、`grid-template-rows` 布局动画、推理预览强制滚底、离屏暂停白名单、600ms 浮层吞点击、复制 timer 防抖、Spinner/BeatLoader reduce 包裹。按计划这些属于「由护栏输出违规清单后分批迁移」，需要 i18n 文案与较大组件改动，本轮不塞半成品。

## 4. 验收门槛对照（计划的 Stage 4 要求 vs 本轮实际）

| 计划门槛 | 本轮状态 |
| --- | --- |
| CI 门禁真跑、0 violations | 脚本已接入 CI，本地绿色；**未在 GitHub Actions 上实跑** |
| 负向验证「故意注入违规被拒绝」 | 已做并通过 |
| 真窗口 reduce 开/关两态截图对比 | 只做了正常态截图 + 产物 CSS 静态复核；**未切换系统 Reduce Motion 偏好做两态对比 |
| 反馈操作 → 首个可见反馈 ≤100ms 打点 | 未做（需要埋点与运行时测量） |
| 零 CLS / 帧率量化 | 未做 |
| AX 全文无内部诊断 | 上一轮已验证；本轮未重扫 |

## 5. Explicitly not claimed

- **未声称「全面优化完成」**：只完成了 Stage 0 + Stage 2，Stage 1/3 未动。
- **未声称 CI 门禁在 GitHub Actions 上通过**（只本地验证）。
- **未声称 reduced-motion 行为已做真窗口两态对比**；只确认了产物 CSS 含规则。
- **未声称 9 个性能布局契约文件在本轮通过**（沿用上一轮记录）。
- **未声称 181 个测试全绿**：1 个既有失败仍在，未削弱它。
- **未声称修复了 R4 状态轴**或所有 30+ 条审计症状。
- **未量化**帧率、CLS、反馈时延。
- 本轮全部为静态代码改动 + 本地验证；未做屏幕阅读器实测。
