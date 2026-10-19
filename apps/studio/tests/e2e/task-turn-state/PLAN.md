# 任务对话进行中状态（Studio）

确定性 Playwright 泳道。不接真实模型，不做桌面 CUA。
Vitest 已覆盖 `taskTurnPresentation` / `turnShowsWorkingStatus`；本泳道只锁壳层集成。

## 夹具

复用 `tests/e2e/workbench-office` 的 `installWorkbench`，并扩展 held stream：
触发句含「保持进行中」时，topic 保持 `streaming`，助手行带成功 tool + prose，直到 pause 或 release。

## 用例

见 [TEST-CASES.md](./TEST-CASES.md)。

## 门禁

```bash
pnpm exec playwright test -c playwright.task-turn-state.config.ts tests/e2e/task-turn-state/turn-state.spec.ts
```

断言只用 `task-pending`、`task-pending-verb`、稳定 aria-label「正在处理」、`data-task-turn-state`。
不断言某一个具体近义词。
