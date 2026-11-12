# 业务功能验证覆盖率 100% — 执行结果（2026-11-12）

## 结论先行

**未达成，且不是「再跑一次」能达成的。** 按仓库权威口径（`PLAN.md` §4/§9），F 层 17 条业务功能中 **8 条要求 L3 真宿主用例，其中 7 条在本仓库里根本没有 L3 用例**；唯一 1 条 L3 用例本轮也未通过。因此 F 层结构性上限约 10/17（58.8%），当前严格达标数为 **0/17**。任何「100%」的说法都只能靠换分母得到。

## 口径（不自选分母）

- 主口径：**F 层 F01–F17 共 17 条业务功能**，验收标准见 `docs/evidence/studio/2026-11-10-page-feature-component-e2e/PLAN.md` §9：每条至少一条通过用例、执行面 ≥ 最低执行面、**L3 功能用 L2 桩通过不计覆盖**。
- 辅助 Gate A：`apps/studio/tests/e2e/COVERAGE_UPLIFT_PLAN.md` 定义；本轮实测 7 个 config 声明 **490** 例（旧文档 483 已过时：settings 20→27）。
- 辅助 Gate B：页面 × 组件，基线 1522，需 inventory 的 `components[].coveredBy`。
- 逐条台账见同目录 `MATRIX.md`。

## 实际执行的命令与结果

```bash
# 按 AGENTS.md 从当前 checkout 重建可追溯产物
cd apps/studio && node scripts/e2e-fresh-build.mjs --build     # EXIT 0, 1m40s
node scripts/e2e-fresh-build.mjs --verify                       # EXIT 0
# verified f87d4657f/0161c1c23 native=79776bccd8ae renderer=3bd7b192d981

# 7 config 声明数（playwright test --list）
# settings 27 / task-conversation 36 / knowledge 16 / composer-audit 204
# workspace-motion 8 / workbench-office 194 / task-turn-state 5 = 490

node scripts/e2e-runner.mjs --output /tmp/aster-e2e-matrix      # 全量矩阵
node scripts/e2e-coverage-gate.mjs --logs=<3 个已完成日志> --declared=490 --target=1.0

# 唯一 L3 真宿主用例
./node_modules/.bin/playwright test --config playwright.connectors.config.ts --reporter=list
```

### Gate A（官方门禁脚本，已完成 3/7 config 时取值）

| 项 | 数值 |
| --- | --- |
| 声明分母 | 490 |
| 通过 / 失败 / 跳过 | **29 / 43 / 7** |
| 已执行 / 未执行 | 72 / 411 |
| **Gate A 通过率** | **5.9% — FAIL**（目标 100%） |
| 执行内通过率 | 40.3% — FAIL |
| 结构上限（已执行/声明） | 14.7% |
| 原计划 95% 目标 | FAIL |
| Gate B | **UNMEASURABLE**（无合规 inventory） |

各 config：`settings` 23/2/2、`task-conversation` **0/36/0**、`knowledge` 6/5/5；其余 4 个 config 未结束，未计入上述分子。

**矩阵中止说明**：`e2e-runner.mjs` 在第 4 个 config（`composer-audit`，204 例、chromium+webkit 双引擎）上运行超过 35 分钟仍未完成，本轮主动中止（后台任务 killed，35m17s）。因此 `workspace-motion`(8)、`workbench-office`(194)、`task-turn-state`(5) 与 `composer-audit`(204) 共 411 例**未执行**。这与 gate 输出的 `unexecuted=411` 一致。中止不影响结论：F 层 7 条 L3 功能无用例这一结构性缺口与矩阵结果无关。

### 失败归因（抽样，均非「未跑」）

- `settings/update-release-notes.spec.ts` 2 例：真实断言失败。期望「2 个版本 · v0.1.2109 – v0.1.2111」，实际渲染「自 v0.2.2 以来的更新内容」——release manifest 与断言不同步。
- `task-conversation` 36 例全失败：首启「运行时组件」对话框的 `[data-slot="dialog-close"]` 定位超时（35 处 `Timeout 20000ms exceeded`）。`--stub-runtime` 可绕过，但那是注入桩、对 L3 业务验证无意义，故未用于凑数。
- `knowledge` 5 失败 + 5 跳过：`knowledge-hub.spec.ts` 的 `test.skip`/`describe.skip`。

### F 层结论

- 9 条最低执行面为 L2 的功能：依赖 Gate A 场景通过；当前 29 个通过场景里尚未逐条完成「每条功能至少一例通过且执行面达标」的映射核定，**不计为已达标**。
- 8 条 L3 功能：**F06/F09/F11/F12/F13/F14/F15 共 7 条无任何 L3 用例**；F07 有 1 条，本轮结果见下。
- **F 层严格达标 0/17**。理论上限 10/17 ≈ 58.8%（9 条 L2 全达标 + F07 通过）。

## 本轮对 L3 基建的实质修复

`connector-native.spec.ts` 是唯一符合 L3 定义的用例（隔离 `ASTER_STUDIO_INSTANCE`、synthetic model server、真实 MCP SDK + native host 4598，**不需要真实模型 API**）。首次运行报 `Synthetic model did not start`，根因是 `model_server.py` 依赖的 `tests/e2e/capability_baseline.py` **在当前工作树缺失**。

从历史 commit `a2fdfd625beb0813a023fa12b9fb9d7ccf8ea1e8` 恢复了该文件（234 行，含 `ModelHandler` 与 `decide`，且其依赖 `json_render_model.py` 仍在工作树）。恢复后 `model_server.py` 可正常启动并输出 port/executable，L3 用例从「结构性无法启动」推进到**实际运行到功能断言阶段**。

第二次运行仍 FAIL，两处原因：
1. 主失败：`dismissBrowserBootstrap()` 中「正在准备组件」对话框的「关闭」按钮定位超时（`connector-native.spec.ts:19`/`:68`）——spec 与当前首启对话框行为不同步，属测试基建缺陷，非业务功能缺陷。
2. `afterAll` 清理：WorkBuddy 的 safe-delete 保护拦截删除含 89 个文件的隔离 profile（`SAFE_DELETE_BULK_CONFIRM_REQUIRED`，阈值 50）。**未绕过该保护**，隔离目录保留在 `~/Library/Application Support/dev.aster.studio/studio/aster/instances/`。

## 要达到 100% 缺什么（精确清单）

1. **补 7 条 L3 真宿主测试**：F06 工具审批（live native ACP）、F09 多标签/窗口 detach、F11 退出进程重启后的偏好持久化、F12 DOCX/XLSX/PPTX/PDF/PNG 真实字节产物、F13 真实安装 3 个 runtime 组件与系统权限、F14 真实更新通道/DB 迁移/单实例、F15 截图录屏真实文件与脱敏。这些需要**写产品级测试代码**，不是增加运行次数。
2. **修 1 条 L3**：`connector-native.spec.ts` 的首启对话框处理。
3. **修复/重建 Gate A 环境**：43 个失败（尤其 36 个首启对话框连锁失败）、7 个 skip、411 个未执行。
4. **Gate B 合规 inventory**：补齐 `components[].coveredBy`，否则永远 UNMEASURABLE。
5. **外部授权**：真实安装、更新通道、DB 迁移、退出重启、系统权限、真模型链路（`2026-11-11-live-model-e2e/OUTCOME.md` 记真模型 0 请求到达）。这些会改动运行环境，需要用户明确授权；本轮未执行。
6. **契约门禁**：F03/F04/F05 还要求 `just studio-contracts` 9 个 pinned vitest 全绿，本轮未跑。

## 授权后的实质推进（第二轮）

用户明确授权「写 L3 测试并执行破坏性操作」后，按「隔离实例、不碰日常 profile」前提执行，优先修杠杆最大的阻塞。

### 根因链与修复（逐条有验证）

| # | 阻塞 | 根因 | 处理 | 验证结果 |
| --- | --- | --- | --- | --- |
| 1 | L3 用例 `Synthetic model did not start` | `tests/e2e/capability_baseline.py` 在当前工作树缺失 | 从历史 commit `a2fdfd625` 恢复 234 行 | `model_server.py` 正常输出 port/executable |
| 2 | 首启对话框关闭按钮定位超时 | spec 用 `getByRole('dialog', {name:'正在准备组件'})`；当前产品标题已变，且 `canDismiss=requiredPrepared` 为 false 时**不渲染关闭按钮** | 改用稳定 `data-ui` 定位、容忍无关闭按钮；`afterAll` 清理失败不再污染判定（**未绕过 safe-delete**） | 不再卡对话框，推进到下一步 |
| 3 | 页面**全白屏** | native host 把 webview 指向 `http://127.0.0.1:4388`，但**该端口无任何服务监听**（harness 漏起静态 preview） | 补起 `STUDIO_RENDERER_PORT=4388 e2e-preview.mjs` | `HTTP 200`，renderer 正常渲染 |
| 4 | 36 个 task-conversation 用例连锁失败 | 同一首启对话框：`runtime.components.required_missing` 无桩 → `requiredPrepared=false` | 用 runner 自带的 `--stub-runtime`（L2 渲染器 fixture，不改变其 L2 性质） | **0 通过 → 13 通过** |
| 5 | fresh-build stamp 反复 stale | 父仓提交推进 rootCommit | 重新 `e2e-fresh-build.mjs --build`（1m19s，stamp `137b868cd`） | `--verify` 通过 |

### 数字变化（Gate A，3 个 config）

| | 修复前 | 修复后（带 `--stub-runtime`） |
| --- | ---: | ---: |
| 通过 | 29 | **41** |
| 失败 | 43 | **31** |
| 跳过 | 7 | 7 |
| task-conversation 通过 | 0 | **13** |

分 config（修复后）：`settings` 22/3/2、`task-conversation` 13/23/0、`knowledge` 6/5/5。官方 gate 未重算，按同口径 Gate A 约 **41/490 = 8.4%**，仍远未达 100%。

### L3 用例当前卡点（未通过）

补上 4388 服务后页面正常渲染，但 `dialog-overlay` 遮罩仍拦截点击：首启对话框因 `required_missing` 非空而 `canDismiss=false`，既无关闭按钮也不消失。要让 native host 侧放行需注入 Tauri IPC 代理（拦截 `runtime_components_ipc` 并透传其余命令），属于测试基建改造；且即便修通这一条，F06/F09/F11/F12/F13/F14/F15 **7 条功能仍完全没有 L3 用例**，不会改变 100% 不可达的结论。

`--stub-runtime` 只作用于静态 preview 的 L2 套件；对走真 native host 的 L3 用例不适用，未用它凑 L3 通过数。

### 工作树改动（未提交，待评审）

- `apps/studio/tests/e2e/connectors/connector-native.spec.ts`：修 `dismissBrowserBootstrap()` 与 `afterAll` 清理容错（子模块内修改）。
- `tests/e2e/capability_baseline.py`：从历史恢复的 234 行，未跟踪。
- 以上均**未提交**：按 `AGENTS.md` 的 Studio land discipline，测试源码改动应在子模块落地后再 bump 父仓 gitlink；当前父仓有并发会话改动，未擅自 bump。是否入库请用户决定。
- 隔离 profile 因 safe-delete 保护未清理，留在 `~/Library/Application Support/dev.aster.studio/studio/aster/instances/`；未绕过该保护。

## 第三轮：F07 通过，F 层 0/17 → 1/17

继续推进后，**唯一那条 L3 真宿主用例真正跑通了**：

```
1 passed (37.7s)
✓ tests/e2e/connectors/connector-native.spec.ts:37:5
  real authority persists installations and real ACP invokes stdio, SSE and HTTP connectors
```

它在真 native host 上验证了：连接器安装经 reload 持久化、stdio/SSE/HTTP 三种 MCP 传输的真实 ACP turn 与工具回执、`host.restart()` 后 `capabilities/mcp.json` 仍在、删除连接器后不再被调用。**这是 F 层第一条严格达标的业务功能（1/17 ≈ 5.9%）**，按 §9 不是桩、不是凑数。

### 又定位并修掉的两个根因

| 阻塞 | 根因 | 处理 |
| --- | --- | --- |
| 首启对话框遮罩拦截点击 | 真 native host 上 `requiredPrepared=false` | 给 4388 静态服务加仓库现成的 `STUDIO_E2E_STUB_RUNTIME=1`（拦截 `runtime_components_ipc` 并透传其余 IPC），**未自写代理、未改 spec 逻辑** |
| `dialog.locator('textarea')` strict mode violation | 弹窗里「参数」与「JSON」两个 textarea | 改用稳定的 `[data-ui="capabilities.connector.custom.json-input"]` |
| ACP turn 返回 `CAPABILITY-MISSING:mcp`（工具未注入） | **`tests/e2e/capability_mcp.py` 也缺失**，stdio 连接器指向不存在的脚本，MCP 进程从未启动 | 从历史 commit `e9a4acb36` 恢复 31 行；恢复后 `mcp-process.jsonl` 出现 `started`/`called` |

即：L3 fixture 集一共缺了 **2 个文件**（`capability_baseline.py` 234 行、`capability_mcp.py` 31 行），这是它此前从未跑通的结构性原因，与产品功能无关。

### 真宿主证据（constitution 要求）

- 构建：沿用 00:34 的 `e2e-fresh-build --build` 产物（root `137b868cd` / studio `0161c1c231`，native `79776bccd8ae`、renderer `3bd7b192d981`）；此后仅文档提交，产物未变。
- 启动命令：`STUDIO_RENDERER_PORT=4388 STUDIO_E2E_STUB_RUNTIME=1 node scripts/e2e-preview.mjs`，`./node_modules/.bin/playwright test --config playwright.connectors.config.ts --reporter=list`。
- 产物目录 `work/connector-e2e-c566e257-f52/`（同批最新）：`mcp-process.jsonl`（`started`/`called`）、`stdio-turn.json`（1294 B 真实 turn 收据）、`model.jsonl`、`native.log`、`proxy.log`。
- 清理：`afterAll` 因 sandbox safe-delete 保护（144 文件 > 阈值 50）跳过删除，**未绕过**；隔离 profile 留在 `~/Library/Application Support/dev.aster.studio/studio/aster/instances/`。

### F11 尝试与中止理由（含实测）

本轮实际写了 `preferences-native.spec.ts` 并跑了一次（不是纸上推断），确认了两条**结构性约束**：

1. **Playwright 浏览器里写不了偏好**。`GeneralSettings.tsx` 的 5 个开关全部走 `usePreference()` → Tauri IPC（`preference_read` / `preference_commit`），而 `playwright.connectors.config.ts` 的 `page` 是普通 Chromium（经 4598 proxy 访问静态 renderer），**没有 `__TAURI_INTERNALS__`**。实测：进入设置页成功、`#setting-general-launch-onboot` 的 switch 可见、`aria-checked=false`，点击后 15s 内仍是 `false`（`toHaveAttribute` 超时）。F07 之所以能过，是因为它的写操作走 `host.api()` 的 HTTP 路由，UI 只承担展示与安装触发。
2. **即便换成真 Tauri webview，也过不了 F11 的验收标准**。`PLAN.md` §4 对 F11 的要求是「退出进程后重启从磁盘恢复（**非 localStorage 假象**）」。进程重启后 `localStorage` 依然保留，headless 又无法执行 JS 清除它，因此「重启后值还在」无法区分真磁盘持久化与 localStorage 桩。要严格达标，需要能在重启前清空 webview 存储并对 SQLite 落盘做独立校验。

另外 `hostToken` 在 renderer 侧无可定位的注入全局，也没有 `/api/preferences` HTTP 路由（已查 `src-tauri/src` 路由表），所以没有捷径。

按 `AGENTS.md` 的 fail-closed 约束，**已删除该未通过的 spec 与其 `test-results` 产物**，不留半成品；上述结论作为后续实现 F11 的前置条件记录在此。F 层仍为 **1/17**。

### 测试源码仍未入库

- `apps/studio/tests/e2e/connectors/connector-native.spec.ts`（helper + textarea 选择器修复，子模块内 M）
- `tests/e2e/capability_baseline.py`、`tests/e2e/capability_mcp.py`（父仓未跟踪，从历史恢复）
- 子模块 `apps/studio` **无 upstream**，按 land discipline 无法「push 子模块提交后再 bump 父仓 gitlink」，因此本轮不硬提交、不 bump，全部留在工作树待用户决定。

## 第四轮：把 Gate A 从 5.9% 推到 36.9%（6/7 config）

补齐此前**从未执行过**的 3 个 config，并修好一个流程认知错误。

```bash
# 刷新可追溯产物（每次父仓提交后 rootCommit 都会漂移）
node scripts/e2e-fresh-build.mjs --build       # EXIT 0, 1m26s
node scripts/e2e-fresh-build.mjs --verify      # verified 69d4933fa/0161c1c23

# 补跑 3 个从未执行的 config（带 --stub-runtime）
node scripts/e2e-runner.mjs --stub-runtime \
  --configs playwright.workbench-office.config.ts,playwright.workspace-motion.config.ts,playwright.task-turn-state.config.ts \
  --output /tmp/aster-e2e-rest

# 官方门禁：合并 6 个 config 的 list 日志
node scripts/e2e-coverage-gate.mjs --logs=<6 logs> --declared=490 --target=1.0
```

| config | 通过 | 失败 | 跳过 |
| --- | ---: | ---: | ---: |
| settings | 22 | 3 | 2 |
| task-conversation | 13 | 23 | 0 |
| knowledge | 6 | 5 | 5 |
| **workbench-office** | **137** | 2 | 55 |
| workspace-motion | 5 | 3 | 0 |
| task-turn-state | 1 | 4 | 0 |

官方门禁输出：`Gate A 36.9% FAIL (need 100%)`，`pass=181 fail=40 skip=62 executed=221 unexecuted=207`，`pass rate among executed=81.9%`，`structural ceiling = 45.1% (221/490)`，`deduped=3`，`Gate B UNMEASURABLE`。相比最初的 5.9%（29/490）是实质提升，但仍远未达 100%。

**第 7 个 config 仍卡住**：`composer-audit`（204 例、chromium+webkit 双引擎）带 `--stub-runtime` 跑了 **16m50s 仍未完成**，按预设上限主动中止。此前不带桩跑了 35m17s 也未完成。它是 `unexecuted=207` 的主体，也是当前环境的硬瓶颈；本轮未计入分子。

**更正上一轮的一个判断**：`apps/studio` 子模块**有** remote（`git@github.com:aster-ai-lab/studio.git`），只是 HEAD 处于 detached、没有 upstream tracking，我上一轮说「无 upstream 无法 push」不准确。真实约束是按项目规则「提交与推送分开、推送需单独授权」——用户尚未授权向子模块远程推送，因此测试源码修复仍留工作树，父仓 gitlink 未 bump。

**本轮把两个 L3 fixture 正式入库**（父仓 `tests/e2e/` 文件，不涉及子模块 gitlink）：`capability_baseline.py`（234 行，来自 `a2fdfd625`）与 `capability_mcp.py`（31 行，来自 `e9a4acb36`）。它们是 F07 能跑通的必要条件；不入库则下次 clone 后 L3 用例又会结构性失败。

F 层仍 **1/17**；Gate A 提升不等于 F 层提升，二者口径不同。

## 第五轮：查清 composer-audit 为什么跑不完

第 7 个 config 一直「跑十几分钟没结果」，本轮定位到**两个独立原因**，都不是产品问题。

### 1. 启动即被 safe-delete 拦截

直接单引擎运行时 1 秒就退出：

```
Error: [safe-delete][SAFE_DELETE_BULK_CONFIRM_REQUIRED]
{"count":117,"threshold":50,"targets":[".../test-results/composer-audit"]}
```

Playwright 启动要清空 `test-results/composer-audit`（累积 117 个文件），超过沙箱阈值 50。**未绕过保护**，改为给每次运行指定全新的 `--output=/tmp/...` 空目录，问题消失。

### 2. 真正的耗时瓶颈：失败用例各等满 20–23 秒

`playwright.composer-audit.config.ts` 继承 `workbench-office`：`timeout 90s`、`expect.timeout 20s`、`use.actionTimeout 20s`、`workers 1`、`fullyParallel false`。单引擎 102 例实测：

- 通过的用例 **1–3 秒**（最慢 3.2s）
- 失败的用例 **20–23 秒**，集中在 `message-actions.spec.ts`（MA-02/03/05/06/08/09/15–18，导出/保存/复制为图片）、`composer-audit.spec.ts`（UI-04 中文输入法等）、`permission-posture.spec.ts`、`popup-motion.spec.ts`
- 正式配置下跑到第 66/102 例即撞 12 分钟上限（`exit=124`）；按 28/66 的失败率外推，单引擎约 18 分钟、**双引擎约 36 分钟**，与之前 16m50s / 35m17s 两次超时吻合

这些失败用例需要 native 文件保存、真实 IME、真实动画帧等能力，静态 renderer preview 提供不了，因此在当前环境下**注定失败**，只是每个都要等满超时。

### 3. 受控实验：压缩超时，跑完全量

临时诊断 config 把 `expect.timeout` 和 `use.actionTimeout` **同时**设为 8s（只压 `expect` 无效，第一版实验仍有大量 20–22s 失败，说明等待发生在 action 层）。8s 远大于通过用例最慢的 3.2s、远小于失败用例的 20s，**不改变任何通过/失败判定**：

```
47 passed / 55 failed，9.7 分钟跑完 chromium 全量 102 例
失败耗时降为 8.4–9.9s
```

诊断 config 跑完已删除，未留在工作树。

### 4. 合并后的门禁数字

把这份 chromium 日志与之前 6 个 config 一起喂给官方门禁（`--logs=<7 logs> --declared=490`）：

```
Gate A 46.5% FAIL  pass=228 fail=95 skip=62 executed=323 unexecuted=105
pass rate among executed=70.6%  structural ceiling=65.9%  deduped=3
Gate B UNMEASURABLE
```

相比 6 config 的 36.9% 又进了一步，但**这份日志来自诊断配置，不是正式门禁运行**，口径已在 `MATRIX.md` 标注。剩余 105 例未执行几乎全是 `composer-audit` 的 **webkit 引擎**（102 例）。

**F 层仍 1/17**：Gate A 的场景通过率提升不增加任何业务功能覆盖，两者分母与判定规则都不同。

## 第六轮：新增第二条 L3 真宿主用例（F14 部分覆盖）

不再刷 Gate A，改为复用已跑通的 `nativeHarness` 写一条**新的 L3 用例**，验证 F14 的一个真实业务断言。

```bash
# 真 native host 的静态 renderer（带仓库现成的 runtime 桩）
STUDIO_RENDERER_PORT=4388 STUDIO_RENDERER_DIST=.studio/dist/e2e-latest \
  STUDIO_E2E_STUB_RUNTIME=1 node scripts/e2e-preview.mjs

./node_modules/.bin/playwright test --config playwright.connectors.config.ts \
  --grep 'agent-plane entities' --output=/tmp/aster-restart-e2e --reporter=list
# ✓ restart-persistence.spec.ts:27:5 (6.3s)   1 passed (28.3s)
```

新增 `apps/studio/tests/e2e/connectors/restart-persistence.spec.ts`，断言链：

1. `GET /api/agent-sessions` 返回 harness 初始化时创建的 `host.session.id`；
2. `host.profile/agent-plane/sessions.json` 落盘文件包含该 id、`model-control/usage.sqlite` 非空；
3. `host.restart()` —— **停掉 proxy 与真实 native host 二进制再重新拉起**，不是页面 reload、不是内存 fixture；
4. 重启后再次 `GET /api/agent-sessions` 仍返回同一 session，`sessions.json` / `agents.json` 仍可读、`usage.sqlite` 仍非空。

`cleanup` 因 sandbox safe-delete 保护（94 文件 > 阈值 50）跳过，**未绕过**。

### 为什么 F 层仍是 1/17，不是 2/17

`PLAN.md` §4 给 F14 的「必须断言」是四项：真实更新通道与清单、**DB 迁移后旧数据可读**、二次启动激活已运行窗口后退出、退出守卫保留进行中任务。本用例只覆盖第二项（且未真正触发 schema 迁移）。按 §9「每条功能至少一条通过用例且执行面达标」的完整语义，**记为 F14 部分覆盖，不预支为严格达标**。

补齐还需要的三项里，「单实例」有可行路径：`src-tauri/src/single_instance.rs` 用 `flock` 做租户锁，锁失败时 activate 已有窗口再退出，可在 host 运行时再 spawn 一次 `stage-launcher` 输出的 Studio 二进制（同 `ASTER_STUDIO_INSTANCE`）并断言第二个进程很快退出。但那要额外起一个真窗口进程，且「更新通道/清单」「退出守卫」仍无法覆盖，本轮未做。

当前 `playwright.connectors.config.ts --list`：**2 tests in 2 files**（F07 的 `connector-native.spec.ts` + 本轮 `restart-persistence.spec.ts`），两条都 PASS。**L3 真宿主用例数 1 → 2，F 层严格达标仍 1/17**。

两个 spec 都在 `apps/studio` 子模块工作树（`connector-native.spec.ts` 为 M、`restart-persistence.spec.ts` 未跟踪），父仓 gitlink 未 bump——向子模块远程推送需单独授权。

## 第七轮：证伪单实例捷径，定位 F06 审批契约

### 单实例验证实测失败（更正第六轮的判断）

第六轮末尾我写「单实例有可行路径：`single_instance.rs` 用 `flock`，再 spawn 一次 Studio 断言第二进程退出」。本轮**实际写了这个 test 并跑了**，结论是**错的**：

```
1 failed / 1 passed (41.1s)
expect(received).not.toBeNull()  Received: null
输出里出现 studio-main-window focused=false visible=true
```

第二个实例没有自行退出，反而启动了新窗口，20 秒后被测试主动 kill。`src-tauri/src/lib.rs:428-440` 显示锁用的是 `tenant::storage_slug(&tenant_slug)`，而 launcher 的 `storage_slug` / fork 选择并不按我假设的 `ASTER_STUDIO_INSTANCE` 简单共享；`claim_quiet` 的注释也写明「Used while picking a free fork」。要正确触发 `Claim::Busy` 需先弄清 launcher 如何选 fork，本轮未继续深挖。

**已删除该失败 test 与其 import**，套件恢复 `2 tests in 2 files`（`--list` 确认），并复验第一条仍通过：`✓ ... (6.3s) / 1 passed (20.7s)`。没有留下跑不通的半成品。残留 Studio 进程已确认清空（4388 只剩 preview，随后也已停止）。

### F06 的契约已定位，可作为下一轮起点

查到工具审批在真 native host 上的完整 HTTP 面：

- `apps/studio/src-tauri/src/agent_plane/controllers.rs:937`：`#[controller("/api/agent-approvals")]`，`POST /api/agent-approvals/{id}`，body 为 `{approved, sessionId, executionId, anchorMessageId, messageId, toolCallId}`（`ApprovalAnswer`，缺字段或多余字段都拒绝），返回 `{ok}`。
- 触发链：ACP `session/request_permission` → SSE `awaiting-approval` → 消息 part `state="approval-requested"`，其 `approval.id` 与绑定字段见 `agent_plane/native_approval_alignment_tests.rs`（含「错误身份一律拒绝且单次有效」的契约）。
- 要用它复现 F06，需要新建 `permission_mode: 'default'` 的 agent/session（harness 现有的是 `bypassPermissions`，直接改会破坏 F07），再从消息流提取绑定，分别批准/拒绝并断言两次终态可区分。本轮**未实现**，故 F06 仍无用例。

**F 层仍 1/17**；本轮净结果是证伪了一条错误捷径 + 记录 F06 的可执行契约，未新增通过用例。

## 第八轮：F06 审批链路探测（打通工具注册，但审批未触发）

按第七轮留下的契约实测 F06。写了一个临时 `approval-probe.spec.ts`（跑完已删除，套件恢复 `2 tests in 2 files`），逐步打印真实 HTTP 形状。

### 打通的部分（可复用的新能力）

1. `GET /api/providers` → `{providers:[...]}`；必须挑 **`isCustom` / `baseUrl` 指向 `127.0.0.1` 的 synthetic provider**（第一次误选预设 `silicon`，turn 直接报 `model ... absent from the complete launch generation`）。
2. `POST /api/agents` 带 `configuration: { permission_mode: 'default' }` 成功创建 agent，`POST /api/agent-sessions` 用 `workspace: {type:'system'}` 成功创建 session。
3. **`PUT /api/capabilities/custom`** 可直接用 HTTP 装连接器（`{ok:true}`，`GET` 回读确认），无需 UI 流程；它同时写出 `capabilities/custom-connectors.json` 与 `capabilities/mcp.json`。
4. **关键坑**：PUT 会丢掉 `env`，而 `capability_mcp.py` 启动即读 `os.environ['ASTER_TEST_MCP_LOG']`，缺了就 KeyError 退出 → MCP 进程起不来 → 工具不进 turn（`offered:false`、模型回 `CAPABILITY-MISSING:mcp`）。按 F07 成功时的格式直接补写 `mcp.json` 的 `env` 后 **`host.restart()`**，实测 `offered: true`、`mcp-process.jsonl` 出现 `started`、turn 进入 `attached` 并产生 `tool-input-start`。

至此，「不经过 UI、纯 HTTP 自包含地装 MCP 连接器并发起真实 ACP turn」这条链路是通的，可作为后续用例的基座。

### 没打通的部分（F06 的真实阻塞）

`GET /api/agent-sessions/{id}/turn`（即 `ai.stream.attach`）的 `streamStatus.pendingApprovals` **始终为 `[]`**，`bufferedChunks` 里只有 `start` / `start-step` / `message-metadata` / `tool-input-start`，**没有 `tool-approval-request`**，消息 parts 也没有 `state="approval-requested"`。turn 长时间停在 `attached`，MCP 进程只 `started`、未被 `called`。

也就是说：agent 的 `permission_mode: 'default'` 被接收并落盘了，但**这个 synthetic ACP harness 不会对 MCP 工具调用发出 `session/request_permission`**，因此 `POST /api/agent-approvals/{id}` 根本没有可响应的审批。这属于 ACP runtime 行为，不是 HTTP 契约问题；要触发审批可能需要换成需要授权的内置工具（如 `write`/`edit`）、或让 ACP node fixture 主动发 `session/request_permission`，本轮未做。

**F06 仍无用例，F 层仍 1/17**。本轮没有新增通过用例，净收益是打通了可复用的工具注册链路并精确定位了审批触发的阻塞点。

## 第九轮：F11 再探（HTTP 直写 desktop-settings，仍失败）

之前 F11 卡在「Playwright 页面没有 Tauri IPC」和「localStorage 无法排除」。这轮换了思路：找 native host 自己的 HTTP 设置入口，用磁盘文件作为「非 localStorage 假象」的证据。

**找到的契约**：`authority.rs:1203` 的 `POST /api/desktop-settings`，body 为 5 个必填 bool（`launchOnBoot`/`tray`/`trayOnClose`/`trayOnLaunch`/`preventSleepWhenBusy`）+ `language`；`desktop_settings::update()` 会 `save()` 到 `support_dir(slug)/desktop-settings.json`。这个文件正是 GeneralSettings 那批开关的落盘位置，由 **Rust 进程**写入，浏览器 localStorage 不可能写到 native host 的 support 目录。

写了临时 `desktop-settings-persistence.spec.ts`（跑完已删），两次运行：

1. 第一次 `ENOENT`：全新隔离 profile 里 `desktop-settings.json` 本来就不存在（host 只在首次 update 时写）。改为先显式 POST 一个 baseline 建立文件。
2. 第二次：写入成功 `{ok:true}`、磁盘核对 `preventSleepWhenBusy: true` **通过**；但 `host.restart()` 之后磁盘值变回 `false`，`expect(afterRestart).toEqual(onDisk)` FAIL。

**原因未定**：`desktop_settings::boot()` 只 `load()` 不写；renderer 源码里也找不到直接 POST `/api/desktop-settings` 的调用点。可能是 storage_slug 差异、或重启后 renderer 首次 preference 同步把默认值写回。在没有确凿证据前，**不把它报成产品缺陷**，也不改期望值去凑绿。

可以确定的是：**HTTP 直写绕过了产品真实的 preference 链路**（`usePreference` → Tauri IPC），所以即便它没被重置也不构成 F11 覆盖。F11 的三项必须断言（IPC 回读、跨页即时生效、重启后从磁盘恢复且非 localStorage）仍未满足。

**F 层仍 1/17**。连续三轮（单实例证伪、F06 探测、F11 HTTP 直写）都没有新增通过用例——瓶颈始终在「先造测试能力」，不在执行次数。当前 `playwright.connectors.config.ts --list` 保持 `2 tests in 2 files`（F07 + F14 部分），无半成品。

## 第十轮：路线 2 不成立，并把 L3 成果固化到父仓

### 「Tauri IPC 桥」可行性证伪

上一轮把「给 Playwright 页面架一座 Tauri IPC 桥」列为可立项的基础工程。本轮查证后**它不成立**：

- `preferences.rs:784/805` 的 `preference_read` / `preference_commit` 是 `#[tauri::command]`，签名强依赖 `tauri::Webview` 与 `tauri::State<PreferenceHost>`；`check_window()` 校验 webview label、URL 与 `host_token`。
- `lib.rs:245-246` 只把它们注册进 Tauri 的 `invoke_handler`；`controllers.rs`、`authority.rs` 里**没有任何 preference HTTP 路由**。
- 因此在普通 Chromium 里转发 HTTP 替代不了它；要「架桥」必须改产品 Rust 代码新增路由并绕过 webview 校验，那已经不是测试基建，且会触碰并发会话正在改的 crate 范围。本轮**未改产品代码**。

顺带查清偏好真实落盘：`preferences.rs:155` 的 `PreferenceStore` 用 `std::fs` 把 ACL 快照写到 `<root>/<scope>.acl`（不是 localStorage）。这给了后续用**真 Tauri webview**（agent-desktop）做 F11 的一条理论路径：改设置后核对 `.acl` 文件、退出进程重启、再回读，即可排除 localStorage 假象。本轮未实施。

### 不需要推送也能保住成果

之前一直说「两个 spec 在子模块工作树、不 push 就会丢」。其实不必 push：已把完整内容固化进父仓证据目录 `specs/`：

| 文件 | 内容 |
| --- | --- |
| `specs/connector-native.spec.ts` | F07 L3 用例的完整当前内容（含修好的 helper / textarea 选择器 / afterAll 容错） |
| `specs/restart-persistence.spec.ts` | F14 部分覆盖 L3 用例的完整内容 |

恢复方式：把两个 `.ts` 复制回 `apps/studio/tests/e2e/connectors/` 覆盖同名文件即可（子模块 HEAD 为 `0161c1c231`）。配合本轮已入库父仓的 `tests/e2e/capability_baseline.py` 与 `capability_mcp.py`、以及 `STUDIO_RENDERER_PORT=4388 STUDIO_E2E_STUB_RUNTIME=1 node scripts/e2e-preview.mjs`，即可复现 `1 passed / 37.7s`（F07）与 `1 passed / 20.7s`（F14 部分）。

> 原先还存了一份 `connector-native.patch`（相对子模块 HEAD 的 diff），但它作为纯文本被 `git diff --check` 判为尾随空格（上下文空行在 diff 格式里就是一个空格），且完整 `.ts` 副本已能恢复，故移除。

**这解决了「成果随时可能丢」的风险，且不依赖向子模块远程推送的授权。**

### 当前状态

`playwright.connectors.config.ts --list` = `2 tests in 2 files`，两条都曾通过；**F 层仍 1/17**，本轮无新增通过用例。连续第四轮试探未提升 F 层，但排除了一个被高估的方案并固化了已有成果。

## Explicitly not claimed

- **不声称业务功能验证覆盖率 100%**。F 层仅 **F07 一条**严格达标（1/17 ≈ 5.9%），其余 16 条未达标或未核定。
- 不声称 Gate A 达到 70%/95%/100%（正式配置 6/7 config 为 **36.9% FAIL**；合并诊断日志后 **46.5% FAIL**，执行内 70.6%）。
- 不声称 `composer-audit` 的 **webkit 引擎**有任何结果（102 例未执行）；不声称 chromium 全量那次是正式门禁运行（它用了 8s 超时的诊断配置，仅判定不变）。
- 不声称把 `expect.timeout` / `actionTimeout` 改成 8s 是正式配置变更；临时诊断 config 已删除。
- 不声称 Gate B 可测量（UNMEASURABLE）。
- 不声称用渲染器桩证明 native 行为。（F07 与 F14 部分两条 L3 用例确实通过，见下条；此处仅不认可以桩充数。）
- 两个恢复的 fixture 已于第四轮提交到父仓 `tests/e2e/`；但**不声称**它们是当前版本重新设计的产物（内容为历史文件原样恢复），也不声称已通过独立代码评审。
- 不声称零覆盖已消除、不声称发布就绪、不声称未跑过的检查通过。
- 不声称绕过了 safe-delete 保护；隔离 profile 未被清理。
- **声称两条 L3 真宿主用例通过**（F07 `1 passed / 37.7s` 有 receipts；F14 部分 `1 passed / 28.3s`，清理后复验 `1 passed / 20.7s`），但**不声称 F14 严格达标**（只覆盖其 4 项必须断言中的 1 项）、不声称单实例验证通过、不声称 F06 或 F11 有任何用例或覆盖（两轮探测均失败并已删除临时 spec）、不声称 `desktop-settings` 重启后被重置是产品缺陷（原因未定）、不声称其余 6 条 L3 功能有用例、不声称 F 层达到 100%（仍 1/17）、不声称 Gate A / Gate B 达标。
- 不声称 `--stub-runtime` 可用于或已用于 L3 真宿主验证；它只作用于静态 preview 的 L2 套件。
- 不声称 7 条无用例的 L3 功能已被补齐，也不声称已执行真实安装/更新/DB 迁移/退出重启/系统权限/真模型等破坏性操作——用户虽已授权，本轮在定位到结构性缺口后未继续扩大改动范围。
- 不声称 `apps/studio/tests/e2e/connectors/connector-native.spec.ts` 的修复已在子模块正式入库：它仍在工作树，父仓 gitlink 未 bump（向子模块远程推送需单独授权）。`specs/` 里的固化副本只是内容备份，不等同于正式提交。
- 不声称 Tauri IPC 桥已实现、不声称改过任何产品 Rust 代码（本轮只做可行性查证，结论是「不成立」）；不声称 F11 已有覆盖。
