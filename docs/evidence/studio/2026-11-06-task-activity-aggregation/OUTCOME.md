# 任务消息流执行活动聚合

日期：2026-11-06

## 结果

- 同一连续执行段中的思考、任务计划与工具调用按到达顺序进入同一个聚合卡片；正文和其他消息块仍分段展示。
- 聚合卡片与任务计划默认折叠，运行中的工具不再强制展开，用户仍可手动查看明细。
- 聚合卡片右侧显示各工具调用实际耗时之和；没有可测时间的历史调用不虚构时长。
- 纯思考、纯计划和普通两条工具调用保留原有展示，不显示为“0 次工具调用”。

## 验证

```bash
cd /Users/roylin/code/aster/apps/studio
pnpm test:renderer \
  src/renderer/pages/agents/taskConversation/__tests__/TaskConversationMessage.test.tsx \
  src/renderer/pages/agents/taskConversation/__tests__/TaskPlanSnapshot.test.tsx
```

- 共 2 个文件、152 项测试，151 项通过。
- 新增的聚合顺序、单调用加思考、运行态默认折叠、耗时求和及计划默认折叠用例均通过。
- `TaskPlanSnapshot.test.tsx`：9／9 通过。
- 唯一失败为 `opens an edit as a diff and can switch to the split view`：单次编辑工具不进入本次聚合分支，失败点是既有差异分栏断言未取得 `radiogroup`，本次未修改差异组件或该断言。
- 对本次生产 TypeScript 文件及计划测试运行 Oxlint：0 warnings、0 errors。
- 对本次 TypeScript 文件运行 ESLint：0 errors；保留 `TaskConversationMessage.tsx` 原有的 2 条跨页面导入 warning。
- 对本次 TypeScript 文件运行 Oxfmt check：通过。
- `git diff --check`：通过。

## Explicitly not claimed

- 不宣称完整 `pnpm lint`、全量 TypeScript 检查或全仓测试通过；这些命令暴露了仓库中大量既有错误。
- CSS 全文件 Oxfmt check 未通过：`task-conversation.css:3228` 存在本次改动之外的既有 `keyframe block is expected` 解析错误。
- 不宣称已进行 Studio GUI、Windows/Linux、完整构建或发布验收；本次验收为渲染器组件测试和定向静态检查。
