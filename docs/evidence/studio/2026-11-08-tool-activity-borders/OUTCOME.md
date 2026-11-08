# 任务消息流：工具调用与聚合去边框

用户截图反馈：「任务消息流中的工具调用和聚合能不能不要这么多边框」。截图里一个「探索工作区 7 个步骤」的聚合块被多层框套住：外层大圆角卡片、每条工具行之间的细线、展开输出区又嵌一个独立圆角框、左侧还有虚线 timeline 引导线。

## 边框来源（`src/renderer/pages/agents/taskConversation/task-conversation.css`）

| 视觉 | 规则 |
| --- | --- |
| 外层大圆角卡片 | `.task-tool-activity { border: 1px solid var(--task-line); border-radius: 12px; background: var(--task-paper) }` |
| 聚合头与列表之间的横线 | `.task-tool-activity > [data-task-disclosure] { border-top: 1px solid var(--task-line) }` |
| 展开输出区的分隔线与底色 | `.task-tool-row [data-agent-tool-details] { border-block-start: 1px solid var(--task-line); background: var(--task-canvas) }` |
| 目录结果的独立圆角框 | `.task-dir { border: 1px solid #ececec; border-radius: 8px; background: #fff }` |
| 左侧虚线引导线 | `.task-tool-activity [data-task-disclosure]::before` / `.task-tool-group__item::before` 的 `border-inline-start: 1px dashed` |
| 每行之间的「细线」 | `.task-tool-row` / `__summary` 的白色 `background` 与外层 `--task-paper` 在 `margin: 2px 0` 间隙里形成的色差 |

## 改动

把原来分散在 1304–1319 的 activity 规则替换为一组**仅在 `.task-tool-activity` 作用域内生效**的扁平化覆盖，不动其它场景（独立工具行、`.task-file-activity` 文件卡片、wiki 卡片等）：

- 外层 `border: 0` / `border-radius: 0` / `background: transparent`；
- disclosure 顶边 `border-top: 0`，并 `display: none` 掉所有后代的虚线 timeline 伪元素；
- `.task-tool-row` / `__summary` 背景透明，消除行间色差（hover 的 `#f7f7f7` 反馈保留）；
- `[data-agent-tool-details]` 去掉 `border-block-start`、背景透明；
- `.task-dir` / `.task-tool-result` 去掉边框与圆角（背景保留，代码块/结果仍可辨）。

层级改由**缩进 + 留白 + hover + 图标**表达，不再靠层层嵌套的卡片。没有改 JSX、没有删任何 DOM、没有改可访问名或交互。

## 验证

| 检查 | 结果 |
| --- | --- |
| `pnpm exec rsbuild build`（`STUDIO_RENDERER_DIST=/tmp/aster-e2e-dist`） | **exit 0，19.0s** |
| `node scripts/run-perf-layout-contracts.mjs` | **9 文件 / 185 用例全过** |
| `git diff --check` | 通过 |
| 编译后 CSS 的 DOM 级 computed-style 验证（Playwright `setContent` 注入真实 `index.*.css`） | `.task-tool-activity`、disclosure、`.task-tool-row`、`__summary`、details、`.task-dir` 的 **四边 border 全为 0px**，外层/details 背景 `rgba(0,0,0,0)`；`.task-dir` 半径 0px |

DOM 验证脚本用 `@playwright/test` 的 `chromium` 渲染最小 activity 结构，避免只改源码、实际被更高特异性规则覆盖而没生效。

## 查看效果

静态预览仍在 `http://127.0.0.1:4318`，dist 已用最新 CSS 重新构建，刷新即可看到。真实窗口走 `just dev`，renderer CSS 会热更新（native 构建的资源路径问题见 `docs/evidence/studio/2026-11-08-native-build-fix/OUTCOME.md`）。

未提交、未 push。
