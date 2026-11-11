# 反馈与错误文案加固验收（2026-11-11 第四批）

配套：上一批 `../2026-11-11-agent-desktop-e2e/OUTCOME.md`、总计划 `../2026-11-11-ui-ux-page-audit/REMEDIATION-PLAN.md`。本文只记录**实际改动、实际命令、实际计数、实测结果**。

## 1. 基准

- Studio 基线 `8c4df145e7`；父仓库 `56d814c72`（本轮期间并发会话又新增 `README.md` / `assets/readme/*.svg` 等未提交改动，全程未触碰）。
- 改动规模：`6 files changed, 56 insertions(+), 4 deletions(-)`（Studio 子模块）。

## 2. 实际改动

| 项 | 文件 | 改动 |
| --- | --- | --- |
| **根因修复（最重要）** | `utils/userFacingText.ts` | `isInternalProcessErrorPayload` 在 `typeof error === 'object'` 分支显式检查 `error instanceof Error` 的 `message` 与 `cause`，再走 `Object.values` 递归。 |
| 静默失败 | `settings/McpSettings/McpSettings.tsx` | 外层 catch（覆盖非活跃服务器保存与意外表单异常）补 `toast.error(t('settings.mcp.updateError'))`。活跃服务器分支原有 `popup.error` 会吞掉异常，不会重复弹。 |
| 内部诊断外露 | 同上 | 保存失败 popup 与删除失败 toast 不再直出 `error.message`：`isInternalProcessErrorPayload(error)` 为真时回退 `t('error.unknown')`，否则经 `redactInternalPaths` 去掉绝对路径；空串也回退通用文案。 |
| 文案用户向 | `i18n/locales/{zh-cn,en-us}.json` | `capabilities.modelscope.unsupported`：「该条目没有可识别的本地接入配置」→「当前环境暂不支持安装」；英文同理收短。该 key 同时用于不可用卡片的按钮位文案与详情弹窗的 `role="alert"` 提示，短句在两处都成立。 |
| 回归断言 | `utils/__tests__/userFacingText.test.ts`、`settings/McpSettings/__tests__/McpSettings.test.tsx` | 新增「原生 Error 的不可枚举 message 也能识别内部诊断」与「删除失败 toast 不含 `completion gate` / `workspace mutation` / `/Users/`」两条。 |
| 证据更正 | `../2026-11-11-agent-desktop-e2e/ux-analysis.md`、`OUTCOME.md` | 撤回上一批把 `WorkbenchSessionCard` 任务统计浮层判为「I4 内部记账平铺缺陷」的结论；改为「误判更正」。 |

## 3. 根因发现：原生 `Error` 的 `message` 不可枚举

新增的回归断言第一次**失败**，收到的 toast 是 `settings.mcp.deleteError: completion gate: workspace mutation at …/aster failed`：

- `redactInternalPaths` 生效了（`/Users/roylin/private/aster` → `…/aster`），但 `completion gate` / `workspace mutation` 仍外露 → 说明 `isInternalProcessErrorPayload(error)` 返回了 **false**。
- 原因：`Error#message`（以及 `name` / `stack`）是**不可枚举**属性，`Object.values(new Error('…'))` 返回 `[]`。上一批把检测器从「单字段黑名单」升级为「整个错误对象递归扫描」时，测试只覆盖了可枚举的 plain object / JSON 字符串，**恰好漏掉了最常见的原生 Error 载体**。
- 影响面不止 McpSettings：所有把原生 `Error` 直接交给 `isInternalProcessErrorPayload` 的出口（含上一批改过的任务页 / 通用错误卡）都可能有同类漏洞。修复放在工具层而非某个调用点，并补了专门针对 `new Error(gate)` 与 `error.cause` 的断言。
- 修复后 `ErrorBlock.test.tsx` 与 `userFacingText.test.ts` 均通过，说明没有破坏既有净化语义。

## 4. 主动排除的「伪缺陷」（避免过度修改）

复核后**没有改动**以下代码，理由记录在此：

| 曾疑似缺陷 | 复核结论 |
| --- | --- |
| `WorkbenchSessionCard` 任务统计浮层 | 有意产品功能：`WorkbenchSessionCard.tsx` 渲染 `role="region"`「任务统计」，数据来自 `/agent-sessions/:sessionId/statistics`，每个指标有 `stat_hint.*` tooltip 解释口径，`WorkbenchSessionCard.test.tsx:88-130` 明确断言 12 个指标全在。与「错误卡被动暴露内部诊断」性质不同。已更正上一批证据，未改代码。 |
| `RuntimeComponentsSection` 安装失败只写 log | `ComponentCard` 本身按 `snapshot.operation.status === 'failed'` / `snapshot.status === 'error'` 渲染错误态，`finally { await refresh() }` 会刷新快照。已有可见反馈，加 toast 会重复。 |
| `EnvironmentDependencies.installTool` 失败只写 log | 源码注释明确「Card-level failures surface through the operation state (BinaryInstallFailureRow), not a dialog」，同样是状态驱动反馈。 |
| `ModelSettings.onSelectDefault` 无 success toast / 无 pending | Select 的选中态本身即成功反馈，失败已有 `toast.error`。补 success toast 属于噪声，不符合 I1/I2。 |
| 设置页其余静默 catch | 27 处中不乏「部分能力降级但主流程可用」的合理静默；本批只修确认会造成「点了保存却零反馈」的一处，其余仍需逐处判断，不以批量加 toast 充数。 |

## 5. 验证命令与结果

```bash
git diff --check
node scripts/check-motion-contract.mjs
node_modules/.bin/tsx scripts/i18n-check.ts
node_modules/.bin/vitest run --project renderer \
  src/renderer/utils/__tests__/userFacingText.test.ts \
  src/renderer/pages/settings/McpSettings/__tests__/McpSettings.test.tsx \
  src/renderer/components/chat/messages/blocks/__tests__/ErrorBlock.test.tsx
```

| 检查 | 结果 |
| --- | --- |
| `git diff --check` | EXIT=0 |
| `check-motion-contract.mjs` | `scanned 56 CSS files` / `OK`（本批未改 CSS） |
| `i18n-check.ts` | `已校验 25308 条翻译` / EXIT=0（只改既有 key 的值，字典序不变） |
| 3 个套件 | `Test Files 3 passed (3)`，`Tests 46 passed (46)`，EXIT=0 |
| 新增断言首次运行 | **故意记录失败**：净化断言收到 `completion gate`，据此定位 §3 根因；修复后转绿。 |
| `CapabilitiesPage.test.tsx` | 6 个失败。用 `git stash push -- .` 在干净 HEAD 复跑，**同样 6 个失败**（首个为 `aria-label` 期望 `capabilities.install` 实得 `capabilities.installing`）→ 既有，与本批文案改动无关。 |
| 保存失败 toast 的单元验证 | **未获得**。两次尝试构造非活跃服务器保存失败（完整 server / trusted stdio server），请求均未到达 `updateMcpServer` 的 catch（表单或信任链路提前返回），`toastError` 调用数为 0。按「不假绿」纪律**移除该断言**，不保留一个靠放宽条件才能通过的用例；代码修复保留，但该路径无单元实证。 |

## 6. 真窗口边界

本批未重跑 agent-desktop E2E。理由：改动集中在**错误路径**（需要故障注入才能触发 MCP 保存/删除失败）与工具层净化，`fresh-build` stamp 在上批提交后已 stale，重建并重启窗口只能复核到连接器卡片的新文案这一处。上一批 `../2026-11-11-agent-desktop-e2e/` 已有 15 场景 / 14 截图的完整基线；本批**不宣称**新文案或错误反馈在真窗口被验证过。若需验收，应按 `AGENTS.md:83-93` 重新 `--build` + 重启窗口，并通过故障注入触发 MCP 保存失败。

## 7. Explicitly not claimed

- 未做 Stage 1 三条结构性改动、Stage 2 的 ESLint catch 规则与 `state-freshness` 检查、设置页其余静默 catch、`ModelSettings` 三态、重复 keyframes、600ms 浮层、布局动画、消息搜索 `aria-busy`。
- 未做 Stage 4 量化（反馈时延打点、reduce 两态对比、零 CLS、CI 实跑、真模型 222 项消息机制覆盖）。
- 未修 `CapabilitiesPage` 的 6 个既有失败、上一批的 6 个 vitest 失败、309 条 typecheck 错误、8 个文件格式漂移、`KnowledgeNotesPage` 的 51 个环境问题失败。
- 未验证键盘焦点环与 `prefers-reduced-motion` 开启态；未触发真实模型任务。
- 「非活跃服务器保存失败」的 toast 修复**只有代码与类型正确性的依据，无单元或真窗口行为实证**（见 §5）。
- 连接器新文案只在 i18n 检查与代码层确认，无真窗口截图。

## 8. 落地

- **Studio 子模块**：`0161c1c231 fix(ux): stop silent MCP failures and gate native Error diagnostics`，已 `git push origin HEAD:main`（`8c4df145e7..0161c1c231`）。
- **父仓库**：`just studio-land-check` 在 gitlink bump 前后各一次 EXIT=0；本次提交只暂存 `apps/studio` gitlink、`docs/evidence/studio/2026-11-11-feedback-hardening/`，以及上一批两处证据更正（`2026-11-11-agent-desktop-e2e/ux-analysis.md` 与 `OUTCOME.md`）。`git diff --cached --name-status` 核对无并发会话文件。
- **并发隔离**：`crates/code/acp/**`、`README.md`、`assets/readme/*.svg`、`docs/evidence/code/2026-11-10-acp-cancel-terminal/` 全程未 add / 未 commit / 未修改；父仓库推送为 fast-forward。
- **环境限制**：本机 Node 22 低于 `engines >=24.12`，门禁走等价入口（`node_modules/.bin/vitest` / `tsx` / `oxfmt`）；全局 `lefthook` 缺失只打印警告，commit/push EXIT 均为 0。
