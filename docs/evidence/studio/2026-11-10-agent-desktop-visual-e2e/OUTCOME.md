# 使用 agent-desktop 完成可视化端到端测试（2026-11-10）

按 `docs/evidence/studio/2026-11-10-page-feature-component-e2e/PLAN.md` 执行 L0 台账与 P0 可视化场景。本文件只记录**实际执行结果**；计划与验收分开，不回写计划文档。

## 1. Fresh-build 前置（`AGENTS.md:83-93`）

| 项 | 命令 / 结果 |
| --- | --- |
| 构建 | `cd /Users/roylin/code/aster && node apps/studio/scripts/e2e-fresh-build.mjs --build` → **EXIT=0，2m40s**。内部依次执行 `<repo> just build-studio aster`（`Finished dev profile in 38.04s`）与 `<studio> node_modules/.bin/rsbuild build`（`ready built in 20.4s`） |
| Stamp | `apps/studio/.studio/e2e-build-stamp.json`：`rootCommit=4ced2b135225802e3c633b1b8d81cc7a947184ed`、`studioCommit=42303bb224ad250329723234081fbc5fc67b4525`、`nativeBinarySha256=576652e33f170e04433fc166504484b0f22e4ef98171fd4f2afc2c0f6f8e51a7`、`rendererEntrySha256=5ec1360e59cc838c78749a723a69bab03a971a42a7794be1eb93131578cf110a`、`rendererDist=.studio/dist/e2e-latest`、`builtAt=2026-11-10T13:16:58.351Z` |
| 校验 | `node apps/studio/scripts/e2e-fresh-build.mjs --verify` → `verified 4ced2b135/42303bb22 native=576652e33f17 renderer=5ec1360e59cc` |
| 启动尝试 1（失败） | `STUDIO_RENDERER_DIST=.studio/dist/e2e-latest ASTER_TENANT_SLUG=aster just studio aster` → `just studio: timed out waiting for the Cherry main window`，EXIT=1。本机 `node v22.22.2`，rsbuild hot-reload dev server 未在 4318 就绪 |
| 启动尝试 2（成功） | `STUDIO_RENDERER_DEV=0 STUDIO_RENDERER_DIST=.studio/dist/e2e-latest ASTER_TENANT_SLUG=aster just studio aster`。重新 `rsbuild build` + `rsbuild preview`（静态 renderer 来自同一 checkout）；`curl --noproxy '*' http://127.0.0.1:4318/windows/main/index.html` = **200** |
| 窗口 | `agent-desktop list-windows --app Aster` → `w-52438`，pid `95376`，1440×920，`visible=true`、`accessible=true`、**`is_focused=false`**（全程未抢焦点） |

`agent-desktop status`：platform=`macos`，accessibility=`granted`，screen_recording=`granted`，automation=`unknown`。
`agent-desktop list-apps` 因沙箱 `ps: Operation not permitted` 返回 `application_inventory_unstable`；改用无 `--app` 的 `list-windows` 与显式 `--window-id` 绑定，`list-windows --app Aster` 随后正常工作。

**全程 headless semantic 投递**：`snapshot` / `find` / `click <ref>` / `screenshot`。未使用 `--headed`、`focus-window`、坐标、鼠标，也未使用 `press`。

**历史阻塞未复现**：`docs/evidence/studio/2026-11-09-agent-desktop-e2e/OUTCOME.md` 记录的 `FirstRunRuntimeComponentsDialog`（Runtime plugin pin identity mismatch）本次没有挡住窗口，AX 树直接是工作页主界面。本轮**没有修复**该 native 侧问题，只是未复现。

## 2. L0 逐页面 · 组件台账

命令：
```
node apps/studio/scripts/ax-page-audit.mjs --app Aster --window-id w-52438 \
  --out docs/evidence/studio/2026-11-10-agent-desktop-visual-e2e/page-component-inventory.json
```

结果：14 页全部导航成功、0 页失败；`totals = {pages: 14, components: 1299, issues: 3}`；`components[]` **1299/1299** 均带 `coveredBy` 数组与稳定 selector。

| 页面 | 组件 | 页面 | 组件 |
| --- | ---: | --- | ---: |
| 工作 | 35 | 设置/快捷键 | 103 |
| 连接器 | 35 | 设置/用量 | 475 |
| 技能 | 35 | 设置/模型服务 | 137 |
| 自动化 | 35 | 设置/模型配置 | 53 |
| 设置/通用 | 57 | 设置/网络搜索 | 91 |
| 设置/文档处理 | 58 | 设置/数据 | 70 |
| 设置/归档 | 64 | 设置/关于 | 51 |

角色分布：button 705、switch 424、combobox 130、radiobutton 21、textfield 18、link 1。

3 个 `R5` 观测项（offscreen 交互件）：设置/快捷键 20、设置/用量 205、设置/模型服务 46，合计 271。按计划 §9，`offscreen` 只作性能观测，**不作为失败断言**。

**基线校准**：旧稿 `PAGE_COMPONENT_E2E_PLAN.md` 的基线是 15 页 / 1522 组件（含已下线的「设置/API 网关 63」），`scripts/e2e-coverage-gate.mjs:29` 仍硬编码 `BASELINE_COMPONENTS=1522`。本轮真窗口重测为 **14 页 / 1299 组件**。两者页面集合不同，**差额 223 不可直接相减归因**（含下线页面与 UI 结构变化）。新基线写入 `page-component-inventory.json`，待 PLAN.md S0-④ 更新门禁常量。

## 3. 可视化交互场景

命令：`python3 docs/evidence/studio/2026-11-10-agent-desktop-visual-e2e/visual-scenarios.py w-52438`

最终轮 **7 pass / 0 fail / 0 skip / 0 未执行**，7 个场景 `restored=true`（每次都回到工作台初始态）。

| ID | 场景 | 关键断言 | 结果 |
| --- | --- | --- | --- |
| S01 | 工作台基线 | AX 树完整、36 refs、主界面与 composer 可访问、无首启模态 | PASS |
| S02 | 设置导航与返回 | 语义点击「设置」→ 67 refs、出现模型配置/用量/快捷键、无 API 网关残留 →「返回应用」回工作台 | PASS |
| S03 | 搜索任务对话框 | 打开后主界面隔离（7 refs）、「关闭」可点、关闭后恢复 | PASS |
| S04 | 任务会话页 | 打开历史任务、会话页 refs > 20、消息流与工具行可见 | PASS |
| S05 | 模型选择器 | 触发可定位（显示名或 model id）、弹出后有可选模型 | PASS |
| S06 | 侧栏收放 | switch 切换后「新建任务」消失、再次切换后恢复 | PASS |
| S07 | 错误态 | 无 `Runtime plugin` / `pin identity` / `正在准备组件` / `重试安装` | PASS |

截图（均 1508×988 PNG）：`work-home.png`、`settings-general.png`、`search-dialog.png`、`task-conversation.png`、`model-selector.png`、`sidebar-collapsed.png`。

**迭代留痕**：`scenarios-run1.json` 3/7、`scenarios-run2.json` 4/7、`scenarios-run3.json` 1/7、`scenarios-run4.json` 6/7、最终 `scenarios.json` 7/7。前四轮的失败全部来自**本轮新建取证脚本的 harness 缺陷**，非产品回归：

1. `-i` 快照只含交互节点，却断言静态空态文案 → 改断言为「对话框可关闭且隔离主界面」，空态由截图验证；
2. 断言失败后未恢复对话框，导致后续场景连锁找不到工作台控件 → 加入 `restore_to_workbench()` 与每场景 `finally` 清理；
3. 会话页 143 个 AX 节点超出默认 3s 观察期限 → `--timeout-ms 15000` + 一次重试；
4. 侧栏收起后工作台控件隐藏、switch 可访问名变化 → 恢复逻辑按「关闭 → 返回应用 → 展开侧栏 → 工作」顺序兜底；
5. 模型触发器的可访问名在不同工作台状态下为 `DeepSeek Flash` 或 `deepseek-flash` → 按稳定 token 匹配。

## 4. 覆盖门禁（实际结果）

命令：
```
node apps/studio/scripts/e2e-coverage-gate.mjs \
  docs/evidence/studio/2026-11-10-agent-desktop-visual-e2e/playwright-not-run.log \
  docs/evidence/studio/2026-11-10-agent-desktop-visual-e2e/page-component-inventory.json \
  --target=0.7
```
→ **EXIT=1**（完整输出见 `coverage-gate.txt`）。`playwright-not-run.log` 是本轮显式标注「未执行 Playwright」的空日志，避免复用旧结果充数。

| 门禁 | 结果 |
| --- | --- |
| Gate A | `0.0% — FAIL`；pass=0 fail=0 skip=0 executed=0 **unexecuted=483**；need 339 passes |
| Gate B | `measurable=true`、`covered=0`、`total=1299`、`coverage=0.0% — FAIL`、`zeroCoverage=1299` |

零覆盖清单导出为 `zero-coverage.json`（逐条 `page / role / name / selector` + 按页按角色汇总）。

**结论**：本轮把 Gate B 从历史证据里的「1 页 / 4 组件、不可代表产品」推进到 **14 页 / 1299 组件可测台账**，但「通过的用例 → `coveredBy`」回填（PLAN.md S0-③）尚未实现，因此 Gate B 仍是 0%。**这不是产品覆盖率提升**，不得据此声称覆盖达标。

## 5. 四元组与失败分类

| 类别 | 数量 | 说明 |
| --- | ---: | --- |
| 通过 | 14 页审计 + 7 个原生 AX 场景 | 最终轮 |
| 失败 | 0 | 最终轮；前四轮 harness 失败已留痕并修复 |
| 跳过 | 0 | — |
| 未执行 | Playwright 483 例；PLAN.md 的 L3 P0 场景（真实 ACP / 工具审批 / 跨进程持久化 / 连接器 native / 更新 / 崩溃恢复 / 多窗口编排） | 需真模型、隔离实例或产品侧前置 |

失败分类：本轮最终无失败；历史四轮全部归为 **harness 缺陷（本次取证脚本引入）**，无「既有产品失败」，无「环境失败」计入最终结论（沙箱 `ps` 权限与 Node 22 的 dev-server 超时作为环境限制单列于 §1）。

## 6. 环境状态与下一步

- Studio 窗口 `w-52438` 目前**仍在运行**（来自本次 fresh build），状态已恢复到工作台；`just studio` 的后台任务 ID 为 `UFZeic`。如需关闭可停掉该后台任务，之后 4318 端口一并释放。
- 本轮未恢复 `e2e-preview.mjs` 静态预览（没有后续 renderer 套件要跑），也未修改 `apps/studio` 子模块内任何文件；子模块仍为干净 gitlink `42303bb224`。
- 下一步按 PLAN.md S0：③ 用通过的用例回填 `coveredBy`；④ 把 `BASELINE_COMPONENTS` / `DECLARED_TESTS` 按 1299 与单租户口径重算并在 OUTCOME 留前后数字；B1 修 `ci.yml:178` 的 `test:contracts:perf-layout` 漂移。之后进入 S1 的 L3 真宿主场景。

## 7. Explicitly not claimed

- **未声称 Gate A / Gate B 达标**：两者实际均为 FAIL（0.0%）。
- **未声称产品覆盖率为 0% 之外的任何值**，也未声称 1522→1299 是覆盖率变化；这只是台账基线重测。
- **未声称 `coveredBy` 已回填**、未声称零覆盖清单已消除。
- **未声称 Playwright renderer 套件、CI 契约门禁、`just studio-contracts` / `just studio-land-check` 在本轮通过或已运行**。
- **未声称 L3 场景已执行**：真实 ACP turn、工具审批批准/拒绝、跨进程重启持久化、连接器 stdio/SSE/HTTP、真实更新与 DB 迁移、崩溃/断网恢复、多窗口焦点与剪贴板、真实文件字节产物均未执行。
- **未声称修复 runtime pin**：本轮只是未复现该阻塞。
- **未声称 `just studio` 的 hot-reload 模式可用**：它确实超时失败，成功的是 preview 模式。
- **未声称跨引擎 / 跨 OS 覆盖**：仅 macOS arm64、单 chromium webview、单 `zh-CN` locale。
- **未声称截图构成视觉回归基线**：它们只是状态取证，没有像素对比。
- **未声称本轮任何结果已提交**：新增证据文件均在工作区，未 commit、未 push。
