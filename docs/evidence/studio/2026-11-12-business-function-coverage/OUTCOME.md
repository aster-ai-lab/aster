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

## Explicitly not claimed

- **不声称业务功能验证覆盖率 100%**，也不声称 F 层任何一条已严格达标。
- 不声称 Gate A 达到 70%/95%/100%（实测 5.9% FAIL，且仅 3/7 config 计入）；不声称剩余 4 个 config 的结果。
- 不声称 Gate B 可测量（UNMEASURABLE）。
- 不声称 L3 用例通过；不声称用渲染器桩证明 native 行为。
- 不声称 `capability_baseline.py` 的恢复已被项目正式接纳（仅恢复到工作树用于本轮验证，需评审后决定是否入库）。
- 不声称零覆盖已消除、不声称发布就绪、不声称未跑过的检查通过。
- 不声称绕过了 safe-delete 保护；隔离 profile 未被清理。
