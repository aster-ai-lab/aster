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

## 第三轮：S1 收尾 —— 4 类 part + R9 checklist + live gate 约定

### 1. 补到 4 类真实 part（没有放宽判据）

第二轮 S1 缺口是「≥4 类 part」只拿到 3 类（`reasoning` 未出现）。本轮**不修改判据**，而是按模型非确定性重跑同一 gated 真 turn：第 1、2 次仍 3 类，**第 3 次拿到单次真实 turn 的 4 类 part**：

```
step-start / reasoning(text, state=done, thinkingMs=553) / text("LIVELLMREADY") / data-context-usage
```

fixture 仍是**一次真实 turn 的完整 parts**（没有拼接不同 turn 冒充）。同步更新子模块 fixture 与组件断言：4 类 part 集合、reasoning 的 `state=done` 与文本内容、markdown 叶子渲染真实回复。

**一次失败与修正**：首次断言直接找 `mock-thinking-block` 失败（1 failed | 93 passed）。读 DOM 后确认 `reasoning` 按设计收在**折叠的 process disclosure** 内（`MessagePartsRenderer.tsx` 开头注释即说明 reasoning 与 child tool groups 共用顶层 disclosure），`ThinkingBlock` 延迟挂载。按现有测试的既有做法加 `fireEvent.click(completed-process-trigger)` 展开再断言 `data-streaming="false"` 与真实 reasoning 文本 → **两次独立 `94/94 PASS`**（EXIT 0）。这是组件设计，不是产品缺陷；没有靠删断言换绿。

`oxlint --deny-warnings` 仍为 1 warning / 0 errors（与 HEAD 同，既有 `import React` 提示）；`git diff --check` OK。

### 2. R9 规则已固化

`REMEDIATION-PLAN.md` §2 R9 已把本会话的失误反推成**可勾选 checklist**（环境失败先读 `error.details`、禁止未换方向连重试 ≥3、`None` 永不当状态、判定覆盖全部诉求维度、两次独立 PASS 才计入、lint 警告与 HEAD 对比、真模型 gate、收尾还原）。开工前逐条确认。

### 3. 仓库级 live gate 约定（本节起生效）

真模型测试在本仓库的**唯一**准入方式：

| 约束 | 约定 |
| --- | --- |
| 开关 | 显式 `ASTER_E2E_LIVE_MODEL=1`；不设即 `SKIPPED`，CI 无任何 LLM secret，**永不默认触发** |
| 规模 | 一次运行**一个 turn**；`LIVE_TURN_TIMEOUT_MS` 默认 120000 上限；不跑长 campaign（C07 是 12 轮 / 7200s，需单独授权） |
| 凭证 | 只从 `~/.aster/config.acl` 在内存读取，不打印、不写回执、不进日志、不进环境变量 |
| 输出 | 回执脱敏（provider 名 / model / host / part 类型 / 耗时），失败保留原始结构化回执与断言结果 |
| 断言 | 不比对模型自由文本；只验结构事实、token 往返、part 类型与渲染形态 |
| 记账 | 组件层 vitest **不计入**用户 15:06 口径的 F 层；每次运行写 OUTCOME 轮次与 `Explicitly not claimed` |
| 费用 | 无预算门禁，靠「本地手动 + 单次 turn + 显式 gate」约束；异常重试需人工判断，不自动重试 |

### 4. S1 判定

**S1 达成**：4 类真实 part 有组件渲染断言（2× 94/94）、R9 checklist 固化、live gate 约定固化。仍未做（如实）：`coverage-matrix.md` 的 222 项统计未改，因为它的 R/B/L/S/F/C 编号描述真窗口截图场景、没有组件渲染维度，硬套会造假。

## 第四轮：真实 4 类 part 的覆盖补齐 —— 两个 D 类 part 的正确路径

S1 完成后继续推进「全部能力和组件渲染」。检查真实 fixture 的 4 类 part 各自在渲染层的去向，发现一个**不能硬造断言**的事实：

| part | `MessagePartsRenderer` 行为 | 可观察路径 |
| --- | --- | --- |
| `text` | markdown 叶子渲染 | ✅ 已有真实断言（`mock-markdown`） |
| `reasoning` | 折叠 process disclosure 内的 `ThinkingBlock` | ✅ 已有真实断言（展开后 `mock-thinking-block`） |
| `data-context-usage` | **`:672` 直接 `return null`** | 数据投影：`projectAgentContextChunk → restoreReportedAgentContext → context-usage 缓存 → ContextUsageSummary` |
| `step-start` | 无专门渲染分支 | 内部步边界，不可见 |

后两类正是 `coverage-matrix.md` 定义的 **D 类**（代码 `return null` / 内部状态，只能静态代码验收）。因此**没有给它们编造组件渲染断言**，而是补上真实数据链路的验证。

### 新增用例：`agentSessionContextProjection.test.ts`

复用该文件既有的 `projectAgentContextChunk` / `cacheService.getSharedSnapshot` 通道，把**真实 fixture 的 `data-context-usage.data`**（`totalTokens: 24798`、`maxTokens: 1000000`、`percentage: 2.4798`、`windowSource: "runtime"`、`measuredAt` 真实时间戳）喂进投影，断言这些**真实数值**进入缓存快照。

因为 `restoreReportedAgentContext` 用 `reportedAgentContextUsageSchema.safeParse`（zod 会 strip 未知字段），断言的是关键字段值而不是 `toEqual` 整个对象——避免把 schema 的正常规范化误判为失败。

### 验证（实测）

| 检查 | 结果 |
| --- | --- |
| `vitest run --project renderer agentSessionContextProjection.test.ts` | **7 passed (7)**，EXIT 0 |
| 两文件一起复跑（`MessagePartsRenderer` + `agentSessionContextProjection`） | **2 passed (2)**，EXIT 0 |
| `oxlint --deny-warnings` 新测试文件 | **0 warning / 0 error**，EXIT 0 |
| `git diff --check` | OK |

### 判定

- 真实 LLM turn 的 4 类 part 现在**各有正确归属**：2 类有组件渲染断言，2 类明确为 D 类并验证其中一类的数据投影链路。
- **不改 222 项统计**、`coverage-matrix.md` 不动（理由同第三轮）；**F 层仍 1/17 / 2/17**，组件/数据层 vitest 不计入用户口径。
- 未改产品源码。

## 第五轮：真实 token 用量闭合到可见 UI

第四轮把 `data-context-usage` 验到了「投影进缓存」，但还差最后一环：**那些真实数字有没有显示在界面上**。本轮补上，至此「真实 LLM turn → 消息 part → 数据链路 → 可见组件」闭合。

### 做法

`MessagePartsRenderer` 对该 part `return null`，所以可见 UI 由 `ContextUsageSummary` + `ContextUsageMeter` 消费缓存数据渲染。在现有 `contextUsage.test.tsx`（3 个用例）里追加 1 条，用**同一份真实 fixture** 的 `data-context-usage.data` 构造组件入参：

```
totalTokens 24798 → usedTokens
maxTokens 1000000
percentage 2.4798
model  "<uuid>::deepseek-flash" → modelName
```

断言真实数字出现在渲染文本里，且 `role="meter"` 的 `aria-valuenow` 与真实百分比一致。

**一次失败与修正**：首版用 `getByText(/24798/)` 找不到——组件做了**千分位格式化**，实际渲染 `24,798 / 1,000,000 (2.5%)`。改为从真实数字推导期望（`container.textContent` 去逗号后比对 `totalTokens`/`maxTokens`/`percentage.toFixed(1)`），断言针对**数值本身而非排版**，没有删断言或放宽匹配换绿。

### 验证（实测）

| 检查 | 结果 |
| --- | --- |
| `vitest run --project renderer contextUsage.test.tsx` | **1 passed (1)**，EXIT 0；**复跑同样 PASS** |
| `oxlint --deny-warnings` | **0 warning / 0 error**，EXIT 0 |
| `git diff --check` | OK |

### 判定

- 真实 LLM turn 的 4 类 part 现有完整归属：`text` / `reasoning` 有组件渲染断言，`step-start` 为不可见内部边界，`data-context-usage` 验到**投影 + 可见 UI 两环**。
- **不改 222 项统计**（理由同第三、四轮），**F 层仍 1/17 / 2/17**：组件层 vitest 按 15:06 口径不计入。
- 未改产品源码。fixture 在子模块测试目录各有一份副本，均同步自父仓 `fixtures/live-turn-parts.json`。

## Explicitly not claimed

- 不声称「任务对话流全部能力和组件渲染」已测完；只跑通了 1 个真 turn、验到 4 种 part 类型，222 项形态里绝大多数未执行。
- 不声称 renderer 组件渲染通过（`renderOk=false`），不声称首启对话框、偏好快照、404 已修复。
- 不声称把 L2 桩、合成 oracle 或 Gate A 结果计入任何覆盖；不声称 F 层数字变化。
- 不声称真模型输出可复现（本轮不比对模型文本，只验结构事实与 token 往返）。
- 不声称凭证被写入任何文件或日志；不声称 CI 能跑真模型（无 secret，且 gate 默认关闭）。
- 不声称 `reasoning_effort.rs` 修复的所有分支都已验证；只实证了「真请求到达模型并成功回复」这一条。
