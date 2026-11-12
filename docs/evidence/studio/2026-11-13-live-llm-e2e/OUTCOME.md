# 真实 LLM × 任务对话流 × 组件渲染 E2E（第一轮）

> 状态：**部分打通**。真实 LLM 请求 → 真 ACP turn → assistant 消息全链路已实证成功 4 次；**renderer 组件渲染断言尚未通过**，不声称完成。
> 本轮所有真实模型调用均为本地手动、单次 turn、`max` 上限 120s，凭证仅内存持有、未打印、未落盘。

## 0. 目标与仓库基线

用户要求「用真实 LLM 测试任务对话流的全部能力和组件渲染」。只读调研确认：

- 仓库此前**没有**「真模型 × 真宿主 × 全组件渲染」的打通证据。`apps/studio/tests/e2e/interactive-coding/` 是唯一标为 real model 的入口，但断言的是**代码产物**而非 UI（`README.md:3-5,46-48`）。
- `docs/evidence/studio/2026-11-11-live-model-e2e/OUTCOME.md:27` 记录最近一次真窗口真模型尝试：**「请求从未到达模型（nothing was sent）」**，0 个真实回复；根因是 `reasoning_effort.rs` 把 `default` 哨兵当 wire 值。该修复现已在代码里（`:113-116`），但 `OUTCOME.md:140,171` 承认**行为未获实证**。
- `tests/e2e/connectors/nativeHarness.ts` 是全仓唯一验证过的「真 native host + 真 ACP turn + Playwright」编排（L3），但模型是合成 oracle `model_server.py`。
- `docs/evidence/studio/2026-11-11-message-flow-ui-inventory/INVENTORY.md` 有 **222 项**渲染形态清单；`.../2026-11-11-message-flow-visual-e2e/coverage-matrix.md:66` 判定 B/C/D 类（需真模型或构造 part 的）**全部未执行**。
- `tests/e2e/JSON_RENDER.md:22` / `KNOWLEDGE.md:31,53` 定下口径：确定性用例日常回归，**外部真实模型是独立补充验收**，且「不能以夹具代替真实模型」。

## 1. 前置实证

| 检查 | 结果 |
| --- | --- |
| `~/.aster/config.acl` 结构 | 5 个 provider（gateway / kimi / deepseek / glm-01 / glm-02），各带 `base_url` / `api_key` / `models`；**内容未读取输出** |
| 相关环境变量 | `OPENAI_API_KEY` / `ANTHROPIC_API_KEY` / `ASTER_API_KEY` / `ASTER_GATEWAY_KEY` 等**全部 UNSET**；凭证只可能来自本机 ACL |
| 极小真实请求（deepseek / `deepseek-flash`，`max_tokens=8`） | **`http=200 has_choices=True`，0.9s** → 凭证有效、网络可达 |
| 工具链 | `bun 1.4.2`、`tsx`、`playwright` 均可用；`Studio` debug 产物存在；子模块工作树干净 |
| `node --version` | `v22.22.2`（仓库要求 ≥24.12，历史记录会 `ERR_PNPM_UNSUPPORTED_ENGINE`；本轮用 `tsx` 直跑 TS，未触发 pnpm 门禁） |

## 2. 实现：`live-conversation-e2e.mts`

**不改子模块源码**：用 `tsx` + `pathToFileURL` 直接 import `apps/studio/tests/e2e/connectors/nativeHarness.ts`，在 harness 起来后用其 `api()` 追加注册**真实 provider**，创建独立 workspace / agent / session，发一次真 turn，再用 Playwright 连 4598 做渲染断言。

- **显式 gate**：`ASTER_E2E_LIVE_MODEL=1`，否则 `SKIPPED`；CI 无 LLM secret，默认永不触发真实调用。
- **凭证**：只在内存里从 `~/.aster/config.acl` 解析，不写入 receipt、不打印、不进日志。
- **一次 turn**：固定 prompt（`Reply with exactly the token LIVELLMREADY...`），`LIVE_TURN_TIMEOUT_MS=120000` 上限。
- **判定收紧**：`apiOk && renderOk` 才 PASS。首版曾只凭 `apiOk` 就报 PASS，已修正——用户要的是组件渲染，不能拿 API 成功凑绿。
- **清理**：`host.close()` 会被 WorkBuddy 的 `SAFE_DELETE_BULK_CONFIRM_REQUIRED`（隔离 profile >50 文件）拦下；改为 best-effort catch，**不让清理错误掩盖原始异常**，也不因此放宽任何断言。

## 3. 实际结果（4 次真 turn，全部模型链路成功）

| 运行 | 真 turn | parts | 渲染断言 |
| --- | --- | --- | --- |
| run3 | `success`，4047ms | `step-start`, `text`, `data-context-usage` | 未验（首版判定缺陷） |
| run4 | `success`，2020ms | `step-start`, `text`, `data-context-usage` | `http=404`，页面空 |
| run5 | `success`，2019ms | `step-start`, `text`, `data-context-usage` | `http=200` 但首启对话框挡住 |
| run6/r7 | `success`，2021/2022ms | `step-start`, `reasoning`, `text`, `data-context-usage` | `Invalid native preference snapshot` / `404` |

`last-turn.json` 回执（脱敏）：`status=success`、`tokenPresent=true`、`searchableTextLength=12`、`partTypes` 含 `text` 与 `data-context-usage`。

**这是本轮最重要的结论**：真实请求确实到达了 `api.deepseek.com`，真模型回复经真 ACP turn 落成 assistant 消息，且产生了 `text` / `reasoning` / `step-start` / `data-context-usage` 四种真实 part。**11-11 的「0 请求到达模型」不再成立**，`reasoning_effort.rs` 的 F3 修复首次获得行为实证。

## 4. 组件渲染为何还没打通（逐层排除）

| 尝试 | 结果 | 诊断 |
| --- | --- | --- |
| 直接连 4598，无 4388 服务 | `http=404`, `htmlLength=39` | `nativeHarness.ts:90` 只设 `STUDIO_RENDERER_PORT=4388`，**不启动 renderer**；`playwright.connectors.config.ts` 也没有 webServer |
| 外部 `e2e-preview.mjs` 起在 4388 | `http=200 title=Aster htmlLength=191692`，但 `bodyLength=245 messageNodes=0` | 首启 `FirstRunRuntimeComponentsDialog` 报 `runtime component host missing`，主界面没挂载 |
| 加 `STUDIO_E2E_STUB_RUNTIME=1` | 变成 `PreferenceService: Invalid native preference snapshot` | 静态 preview 的 IPC stub 与**真 native host** 的偏好快照不兼容 |
| 停外部 preview，给 host 设 `STUDIO_RENDERER_DIST` + `STUDIO_RENDERER_DEV=0` | `http=404` | 该开关不触发 host 自行 serve renderer |

**根因**：`e2e-preview.mjs` 注释自述是 "Static preview for **renderer-only** E2E"，服务于 L2 内存桩；它和真 native host 的 IPC 契约不是一套。要验真 host 会话的组件渲染，必须用 host 自己的 renderer 通道，或换一种渲染验证方式。

## 5. 下一步（三条可行路径）

1. **真窗口渲染**：用 `just studio aster`（`STUDIO_RENDERER_DEV=1` 热更 4318）+ `ASTER_STUDIO_INSTANCE` 隔离实例，让 Playwright 连真窗口；需先解决 agent-desktop/TCC 的系统级 `ps` 阻塞（见 `2026-11-12-business-function-coverage/OUTCOME.md` 第三十四轮）。
2. **真实 parts → 组件渲染**（推荐，绕开 host IPC）：把本轮拿到的真实 `parts`（`step-start`/`reasoning`/`text`/`data-context-usage`）固化成 fixture，喂给已有的 `MessagePartsRenderer` / `MainTextBlock` / `ThinkingBlock` / `MessageCostSummary` 等 **172 个 chat vitest** 组件测试，断言每种 part 的渲染形态。这是「真实 LLM 输出 + 组件渲染」的组合，且不受 222 项里 C 类「真窗口无 `addInitScript` 通道」限制。
3. **扩展 222 项 B 类**：按 `coverage-matrix.md` 用受控工具/故障注入触发 32 个工具渲染器、审批、表单、压缩、产物卡片；需「真模型 + fixture 混合」通道，且 F06 在 `permission_mode:'default'` 下不发 `session/request_permission`（需产品侧解决）。

## 6. 判定与状态

- **真实 LLM 对话流最小链路：PASS（4 次一致）**；**组件渲染：FAIL / 未验**。整体用例判定 `LIVE_LLM_E2E: FAIL`（`apiOk=true renderOk=false`），**不声称用户要求已达成**。
- 业务功能覆盖台账的 F 层数字**不变**（新口径 1/17、原口径 2/17）：本轮用的是真宿主 + 真模型 Playwright 通道，按用户 15:06 口径（必须 agent-desktop 可视化 E2E）**不计入 F 层**；但它是 F03/F07 的真实模型补充验收证据。
- 收尾：无残留 Aster/Studio 进程，4318/4325/4388/4598 全空闲，未改子模块源码、未改产品源码。

## 第二轮：R2 完成 —— 真实 parts 进入组件渲染层

按 `docs/evidence/studio/2026-11-12-business-function-coverage/REMEDIATION-PLAN.md` 的 S1 优先项执行。

### 1. 脚本补上完整 parts 导出

`live-conversation-e2e.mts` 现在把 assistant 的**完整 `data.parts`**（不是仅类型名）写到 `fixtures/live-turn-parts.json`。固定合成 prompt（`Reply with exactly the token LIVELLMREADY...`），所以载荷不含凭证或个人信息。

再跑一次真 turn（`ASTER_E2E_LIVE_MODEL=1`，2.0s，`status=success`），捕获到的真实 parts：

| # | type | 真实内容 |
| --- | --- | --- |
| 1 | `step-start` | 无负载 |
| 2 | `text` | `text: "LIVELLMREADY"`, `state: "done"` |
| 3 | `data-context-usage` | `totalTokens: 24797`, `maxTokens: 1000000`, `percentage: 2.4797`, `outputTokens: 7`, `turnTokens: 24804`, `windowSource: "runtime"` |

（本轮模型未产出 `reasoning` part；上一轮部分运行里出现过，属模型非确定性，不作为断言前提。）

### 2. 复用现有组件测试，不新建 harness

在子模块 `MessagePartsRenderer.test.tsx`（原有 2602 行、完整 `MessageListProvider` / `PartsProvider` setup）里**追加 1 条 `it()`**，复用模块级 `renderParts()` helper，把真实 fixture 直接喂给 `MessagePartsRenderer`：

- 断言 fixture 元信息（`source=live-llm-turn`、`status=success`）与三类 part 集合
- 断言真实 text 内容为 `LIVELLMREADY`
- `renderParts(parts)` 后断言容器存在、**无 `mock-error-block`**、markdown 叶子渲染出真实回复文本

没有把业务分发逻辑复制进测试，也没有新建一套 Provider/mock。

### 3. 验证（两次独立运行，实测）

| 检查 | 结果 |
| --- | --- |
| `vitest run --project renderer MessagePartsRenderer.test.tsx` | **94 passed (94)**，EXIT 0；**复跑同样 94/94 EXIT 0** |
| `git diff --check` | OK |
| `oxlint --deny-warnings` | 1 warning / 0 errors —— **与 HEAD 版本在同目录跑完全一致**（既有 `import React from 'react'` default-export 提示），本次 +30 行未引入新 lint 发现 |
| 子模块落地 | `bbc7bb9c01` 已推送 `studio.git` main；`just studio-land-check` **EXIT 0**（clean / pointer advances / delivery commits clean） |

### 4. 关于 222 项清单，如实说明偏差

R2 原计划第 4 步要「把触发的编号在 `coverage-matrix.md` 里标为已执行」。实际打开后发现那份清单的编号是**真窗口截图场景**（R/B/L/S/F/C 系列，如 `R06`、`C06`、`F04`），没有「text part 组件渲染」这一维度。硬套编号会制造假覆盖，因此**不改 222 项统计**，也不声称其中任何一项因本工作变为「已执行」。本轮的覆盖是**组件层 vitest**，单独计账。

### 5. 判定

- **R2 完成**：真实 LLM 输出的 3 类 part 已进入渲染层组件测试并通过断言。S1 的「≥4 类 part」**未完全达成**（本轮只捕获到 3 类，`reasoning` 未出现），如实标注。
- **F 层数字不变**：1/17（用户口径）/ 2/17（PLAN §9）。组件层 vitest 按用户 15:06 口径（必须 agent-desktop 可视化 E2E）**不计入 F 层**；这是 F03/F07 的真实模型补充验收证据。
- 未改产品源码；父仓 gitlink 已按 land discipline 推进到 `bbc7bb9c01`。

## Explicitly not claimed

- 不声称「任务对话流全部能力和组件渲染」已测完；只跑通了 1 个真 turn、验到 4 种 part 类型，222 项形态里绝大多数未执行。
- 不声称 renderer 组件渲染通过（`renderOk=false`），不声称首启对话框、偏好快照、404 已修复。
- 不声称把 L2 桩、合成 oracle 或 Gate A 结果计入任何覆盖；不声称 F 层数字变化。
- 不声称真模型输出可复现（本轮不比对模型文本，只验结构事实与 token 往返）。
- 不声称凭证被写入任何文件或日志；不声称 CI 能跑真模型（无 secret，且 gate 默认关闭）。
- 不声称 `reasoning_effort.rs` 修复的所有分支都已验证；只实证了「真请求到达模型并成功回复」这一条。
