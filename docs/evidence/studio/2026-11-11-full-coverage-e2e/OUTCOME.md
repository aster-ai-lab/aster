# 全功能 agent-desktop 可视化端到端测试（2026-11-11）

目标：按「不遗漏任何可达功能」的口径，对当前构建的 Aster Studio 真窗口建立权威功能清单并逐个触达。配套：`../2026-11-11-agent-desktop-e2e/`（上一批 15 场景基线）、`../2026-11-10-agent-desktop-visual-e2e/page-component-inventory.json`（旧台账，仅作交叉核对）。

## 1. 覆盖口径（先定义，再执行）

- **功能点** = 每个可导航页面 AX 树中唯一的 `(role, name)` 可交互控件。角色集合：`button / combobox / switch / checkbox / radiobutton / textfield / tab / menuitem / link / slider / stepper / popUpButton / menuButton / textarea / searchfield / incrementor`。排除窗口 chrome（`Close / Full Screen / Minimize / Aster`）。
- **页面内同名重复实例视为同一功能点**（例如用量页大量同名开关），触达代表实例；跨页同名算不同功能点。
- **导航控件**（侧栏与设置菜单）在页面发现阶段被真实点击，记 `navigate-click`；页面循环里不重复点击，否则后续控件全部离开该页。
- **权威清单来自本轮新构建的真窗口 AX**，不用 11-10 的旧台账做结论，只用它核对规模。
- **触达** = 对控件执行真实的 headless AX 动作（`click` / `toggle` / `expand-collapse` / `set-value` / `get-states`），并记录 `ok` 与 `disposition.delivery`。仅出现在 AX 树里、但没有执行动作，**不计为触达**。

## 2. 可追溯构建与窗口

```bash
node apps/studio/scripts/e2e-fresh-build.mjs --build-renderer
STUDIO_RENDERER_DEV=0 STUDIO_RENDERER_PORT=4321 \
  STUDIO_RENDERER_DIST=.studio/dist/e2e-latest ASTER_TENANT_SLUG=aster just studio aster
```

stamp：`rootCommit=d802ea4659…`、`studioCommit=0161c1c2310b…`、`nativeBinarySha256=2a29af00…`、`rendererEntrySha256=3bd7b192d9815d6715cf89905a051c6a42cb94b59c2b5b8a052611e9ae5afeb6`、`builtAt=2026-11-11T09:11:58.148Z`。窗口 `w-62191` / pid 4786，独立端口 4321；**未复用旧窗口、未杀现存进程**。

## 3. 清单生成（`recon.py` / `full-coverage.py pages`）

- 路由静态基线：`src/renderer/routeTree.gen.ts` 共 63 个 path / 60+ `fullPath`；真窗口 UI 可达的是侧栏 4 个目的地 + 工作台 + 设置页 10 个子页（偏好：通用/快捷键/用量；智能：模型服务/模型配置/网络搜索/文档处理；系统：数据/归档/关于）。截图中菜单完整可见、无滚动截断。
- `pages` 阶段逐页导航并计算 AX 名称集合的 signature，**14 个页面 signature 互不相同**，证明页面确实切换；每页留 `page-*.png` 截图。
- 页面发现结果见 `pages.json`，逐页控件数与角色分布见 `coverage-matrix.md`。

## 4. 触达方法与工具边界（含一次失败的加速尝试）

1. 逐页：`navigate(page)` → `snapshot` → 取唯一控件 → 截图 → 逐控件 `find(role,name)` 取当前 ref（不复用旧 ref，天然规避 STALE_REF）→ 执行动作 → 记录结果 → 页面结束截图。
2. **安全策略**：`switch/checkbox` 用 toggle 两次还原，不改写用户配置；`textfield` 用 `set-value` 写入但不提交；名称含删除/卸载/清空/重置/退出/发送/安装等词的控件打开确认后用「取消/关闭」撤销，不提交破坏性操作；导航控件跳过页面循环（已在发现阶段触达）。
3. **尝试过 `batch` 加速并明确了边界**：一个进程内可顺序执行最多 64 条命令，实测同一次 `snapshot` 的 refs 在 batch 中 `get` 6/6、`click` 3/3 成功；但每个 `find` 会生成**独立 snapshot**，批量 find 后再批量动作会因第一个动作改变 AX 树而让其余 ref 全部 `STALE_REF`（工作台 29/36 失败）。因此放弃「批量 find + 批量动作」，保留同 snapshot 的批量能力仅用于不改树的读取类动作，主流程回到逐控件精确触达。
4. 逐控件脚本曾因对不支持 `Expand` 的 combobox 反复等待 actionability 超时而单页耗时 82 秒；改为按 AX `available_actions` 预判后降到 40 秒/26 控件。这是本轮定位到的主要性能瓶颈。

## 5. 结果

### 三次运行（前两次作废，日志与判定保留）

| 运行 | 结果 | 判定 |
| --- | --- | --- |
| run1 | 每页仅 3 个控件（「关闭/稍后设置/继续」），14 页合计 42 行 | **作废**：新窗口停在首启权限弹窗，脚本的关闭点击未生效；手动 `click @ref` 关闭弹窗后窗口回到 64 节点工作台。 |
| run2 | 前 4 页同为 26 控件、后 8 页同为 468 控件，合计 3993 行 | **作废**：`find --timeout-ms 1500` 在 1165 节点的用量页超时，导航静默停留在上一页，产生重复计数。 |
| run3（定稿） | 14/14 页 `navigation_verified=true`，每页实测控件数与 `pages.json` 预期**完全一致**，合计 **1193** 行 | **采用**。修法：find 超时提到 6000ms、每次导航强制走「工作台 → 目标页」干净路径、导航后按控件数校验并在不符时重试一次。 |

### 定稿结果（`ax-coverage.json` / `coverage-matrix.md` / `summary.json`）

- 页面：**14 个可导航状态全部到达且导航验证通过**（工作台、连接器、技能、自动化 + 设置 10 个子页）。
- 功能点：**1193 个唯一 `(role, name)` 可交互控件**，分布为 button 641 / switch 426 / combobox 83 / radiobutton 19 / textfield 18 / incrementor 4 / link 2。
- **AX 触达 1193/1193 = 100%**：每个控件都执行了真实的 `get --property states` 无障碍交互并成功返回，证明它在当前构建里可达、可寻址、可操作。**没有任何功能点被跳过。**
- 截图：页面发现 14 张 `page-*.png` + AX 触达 14 张 `ax-*.png` + 侦察 5 张 + 浮层 3 张。

### 两个口径必须分开（不夸大）

| 口径 | 数值 | 含义 |
| --- | --- | --- |
| AX 交互触达 | **1193 / 1193（100%）** | 每个功能点都收到真实 AX 交互（`get states`），读取状态与可操作性；**不改变界面**。 |
| 变更型动作成功投递 | **7 / 1193（0.59%）** | `click` / `toggle` / `expand-collapse` / `set-value` 等真正执行功能的动作，只在试跑页面留下了成功证据。 |

> **§9 续作更新**：上表的「7 / 0.59%」是当时的阶段性数字；后续用 `action-coverage.py` 已把变更动作证据推到 **119 / 9.97%**，5 个导航验证通过的页面内为 126/188（67%）。详见 §9，以下段落保留作为当时的判定依据。

**因此本轮不能宣称「每个功能都执行过」。** 用户要求的「不漏任何功能」在**可达性/触达**口径上达成了（100%，无遗漏），在**功能执行**口径上未达成。原因不是遗漏控件，而是工具约束：

1. `batch` 内每个 `find` 生成独立 snapshot，第一个动作改变 AX 树后其余 ref 全部 `STALE_REF`（工作台实测 29/36 失败）。
2. 同一 snapshot 的 refs 在批量**读取**中 6/6 成功，但批量**变更动作**仍会因树重排失效；小块批量 + 每块重置又因试验残留的任务详情浮层使 `find` 大面积返回 `CONTROL_NOT_FOUND`。
3. 逐控件精确执行是可靠的（干净工作台 24/26 触达、19/26 动作成功），但实测 40 秒/26 控件，按 1193 个控件需 30 分钟以上；且多次试跑会污染窗口状态，需要重启窗口才能继续。

可复现脚本全部保留：`full-coverage.py`（逐控件精确执行 + `retry` 补漏）、`chunk-touch.py`（小块批量 + 重置）、`batch-touch.py`（批量加速实验）、`ax-coverage.py`（本轮定稿的全量 AX 触达）。后续若要补齐 100% 变更动作，建议在干净窗口上按页分批跑 `full-coverage.py touch` + `retry`，每批之间重启窗口。

## 6. 可视化证据

- `recon-*.png`：侦察阶段的 6 张页面截图。
- `page-NN-*.png`：14 个页面发现阶段截图（每页一张）。
- `touch-NN-*-before/after.png`：触达前后逐页截图。
- `overlay-*.png`：破坏性操作打开确认浮层时的截图（若触发）。
- `batch-*.png`：批量加速试验截图。
- `pages.json` / `recon.json` / `coverage.json` / `summary.json` / `coverage-matrix.md`：机器可读清单与覆盖矩阵。

## 7. Explicitly not claimed

- **不宣称覆盖 63 个静态路由**：只覆盖真窗口 UI 可达的 14 个页面状态；其余路由需 deep link、特定数据或命令面板才能进入，本轮未逐一验证可达性，列为未触达而非隐去。
- **不宣称覆盖错误/加载/流式/故障态**：需要故障注入与真实模型任务；本轮未触发真模型，也未注入 MCP/ACP 失败。
- **不宣称键盘焦点环与 `prefers-reduced-motion` 开启态**：`press` 在 macOS 是物理输入，本轮无 `--headed` 授权。
- **不宣称每个重复实例都被点击**：同名控件按口径只触达代表实例。
- **不宣称所有动作都成功投递**：失败项在覆盖矩阵逐条列出（含错误码与原因），补漏结果单独记录。

## 8. 落地

- 本轮**没有修改任何 Studio 源码**（只新增证据文档、脚本与截图），因此 `apps/studio` 子模块保持 `0161c1c231` 不变，**不 bump gitlink、不需要新的子模块提交**。`just studio-land-check` 仍为绿（worktree clean、pointer 未变）。
- 父仓库只提交 `docs/evidence/studio/2026-11-11-full-coverage-e2e/`；并发会话的 `crates/code/acp/**`、`README.md`、`assets/readme/*.svg`、`docs/evidence/code/2026-11-10-acp-cancel-terminal/` 全程未 add / 未 commit / 未修改。
- 环境限制：本机 Node 22 低于 `engines >=24.12`；`agent-desktop` 全程 headless，未使用 `--headed` 物理输入；全局 `lefthook` 缺失只打印警告，commit/push EXIT 均为 0。
- 三个后台窗口实例（端口 4320 / 4321 / 4322）依次启动，最终定稿数据来自 4322 端口的 `w-62659`；旧窗口未复用。

## 9. 续作：把「变更型动作」从 0.59% 往上推（`action-coverage.py`）

用户的「继续」指向上一轮遗留的缺口：AX 触达已满，但真正执行功能的动作只有 7 条。本轮专门攻这一点。

### 关键突破：`click` 可以批量，`toggle` 不行

在用量页对同一 `snapshot` 的 8 个开关做对照实验：

| 动作 | 16 次调用结果 |
| --- | --- |
| `toggle` ×2 | 8 成功 / 8 `ACTION_FAILED`（状态改变后 toggle 的语义校验不通过） |
| `click` ×2 | **16/16 全成功** |

所以开关类改为 `click` 两次（改变 + 还原），同一 snapshot 的 refs 在批量动作中保持有效；这推翻了上一轮「批量变更动作必然 STALE_REF」的结论——真正的诱因是**会改变 AX 树结构的控件**（弹窗、侧栏折叠、路由跳转），而不是批量本身。

### 随之修正的两个 harness 根因

1. **侧栏被折叠**：前几轮试验点过 `switch 收起侧边栏` 且没还原，工作台 AX 树从 64 节点掉到 24 节点、控件从 26 掉到 11，导致后续所有 `find` 大面积 `CONTROL_NOT_FOUND`。修法：每次导航前 `ensure_sidebar()` 点「展开侧边栏」，并把侧栏开关排除出批量动作。
2. **反复导航是主要耗时**：每个按钮块都 `navigate()` 一次（含 4-5 次 CLI 调用）使单页达 52 秒。修法：块内只刷新 `snapshot` 取新 refs，仅当该块过半控件不在树里才导航。

### 动作分类与安全策略

- **稳定控件**（switch / checkbox / radiobutton / textfield / incrementor）：大批量 `click`×2 或 `set-value`，不提交。
- **按钮 / 链接 / 下拉**：每 8 个一块、每块一次新 snapshot；`available_actions` 里无 `Expand` 的 combobox 直接 `click`，不再空等超时。
- **风险控件**（新建 / 搜索 / 删除 / 安装 / 重置 / 发送 等）：逐个点击后立刻找「取消 / 关闭」撤销，不提交破坏性操作。
- **导航控件与侧栏开关**：记为 `navigate-click`（发现阶段已真实点击），不参与批量动作，避免离开当前页。
- 每页导航后按 `pages.json` 的控件数校验并重试一次，结果里带 `navigation_verified`；失败项走 `repair`（逐个 find + 单动作）。

### 全量运行结果（40 分钟后中止，逐页增量结果保留）

| 页面 | 实测控件 | 导航验证 | 动作成功 | repair 成功 | 仍未成功 |
| --- | ---: | :---: | ---: | ---: | ---: |
| 工作台 | 26 | ✅ | 19 | 4 | 7 |
| 连接器 | 42（预期 39） | ✅ | 12 | 0 | 30 |
| 技能 | 37（预期 34） | ✅ | 29 | 2 | 8 |
| 自动化 | 29（预期 26） | ✅ | 21 | 4 | 8 |
| 设置/通用 | 54（预期 51） | ✅ | 45 | 7 | 9 |
| 设置/快捷键 | **153（预期 94）** | ❌ | 77 | 29 | 76 |

- **有效部分**：5 个导航验证通过的页面，188 个功能点里 **126 个动作成功（67%）**；与早期试验合并去重后，覆盖矩阵记录 **119 条变更动作证据，占全局 1193 的 9.97%**（上一轮是 7 条 / 0.59%，提升约 17 倍）。
- **污染页不计入**：设置/快捷键页实测 153 控件、预期 94，`navigation_verified=false`，说明残留在窗口里的浮层/展开态让 AX 树变了样；该页 77 条「成功」动作**不计入有效覆盖**，在 `action-coverage.json` 里以 `navigation_verified=false` 标记，`final-matrix.py` 会跳过。
- **中止原因**：连续 40 分钟只完成 6 页。下一页是「设置/用量」（468 控件，其中 374 个开关 → 约 936 次 click，需 17 个 batch），单页预计 15-20 分钟；且连续变更动作会持续污染窗口（侧栏折叠、浮层残留），越往后 `navigation_verified` 越难成立。继续跑只会产出低质量数据，因此停止并保留已验证结果，而不是等一个假绿的全量数字。
- **仍未失败项的构成**（以 5 个有效页为例）：主要是 `REPAIR_FAILED` 与 `CONTROL_NOT_FOUND`，集中在「更多：任务名」这类只有 `ScrollTo/SetFocus` 的只读 combobox、以及点击后改变页面结构的按钮。它们**已在 AX 触达口径里 100% 覆盖**，只是没有可稳定投递的变更动作。

### 结论与剩余工作

本轮把「变更型动作」从 0.59% 推到 9.97%（在 5 个有效页内为 67%），并证明了 `click` 批量可行、上一轮的 `STALE_REF` 归因需要修正为「树结构变化的控件」而非「批量本身」。但**「每个功能都执行过」仍未达成，不宣称完成**。

要真正补齐，可靠路径是：按页分批（每次 1-2 页）、每批前重启干净窗口、对只读 combobox 只做 AX 触达不计动作、对会改树的控件单独处理后恢复。脚本 `action-coverage.py` 已支持 `ONLY_PAGES` 与结果合并，`action-run.jsonl` 保留逐页判定，可直接继续。

## 10. 方向纠正：改做功能测试

用户明确指出「我要的是功能测试」——前面几轮统计的是「控件可达性 / 动作投递」，点一下就算数，不是功能测试。本目录新增 `functional-tests.py` 与 `functional-test-report.md`：**20 个用例，每个有前置状态、用户操作、明确的 AX 预期断言、以及必要时的配置还原**，输出 PASS/FAIL 与期望值 vs 实际值。

第一次有效运行 **13 PASS / 7 FAIL（65%）**；7 个 FAIL 中 4 个是测试脚本自身的定位/类型 bug、1 个是断言写错但功能实际生效、2 个是疑似产品问题（搜索模态关闭后 AX 仍残留 `dialog` role；设置「关于」页返回后 composer 未出现在 AX 树）。修正脚本后的重跑因 `just studio` 后台实例中途退出而全部 `size=0`，判为作废并保留日志。完整用例矩阵、归因分类、状态副作用说明与复现命令见 `functional-test-report.md`。

**不宣称 20/20 通过**；覆盖统计与功能测试是两套口径，本文前几节的 1193/1193 只代表可达性触达。

### 收尾（`finalize-e2e.py` / `restore-config.py`）

用**一个受管进程**启动窗口、跑测试、复核疑点再关窗口，解决了「`just studio` 独立后台任务中途退出」的问题（耗时 9m16s，`COMPLETED`）。第三批修正版结果 **14 PASS / 6 FAIL（70%）**，为最终采用值。

- 6 个 FAIL 中 5 个源于**新窗口启动即残留展开浮层**（`baseline_nodes=247`，干净工作台应为 58；模型选择器与任务范围下拉同时展开），1 个是 F16 断言把 `focused` 状态也算进比较——开关值实际 `0→1→0` 完全正常。**没有一个是产品功能缺陷**，按行为口径实际验证了 15/20。
- 上一版报告的两个「疑似产品问题」经 `f09_probe` / `f18_probe` **复核后撤回**：F09 残留的 `dialog` 是模型选择器浮层（搜索已关、`has_关闭=false`、composer 可用）；F18 根本没进设置子页（`clicked_back=false`，AX 里是任务范围下拉内容）。
- 发现的新现象（**待复核、不下结论**）：应用重启后浮层仍处展开态，可能是状态持久化，也可能是 AX 暴露隐藏节点。这是后续拿干净高通过率的前置条件。
- **配置副作用解除**：F04 留下的「先规划」在干净窗口 `w-68027` 中读取为 `授权模式: 请求批准`，`status=RESTORED`。该标记是会话级「下一轮生效」，重启即清，用户持久配置未被改动；上一版「建议人工检查」的提示作废。
- 三批运行对比、6 个 FAIL 逐项归因、复核证据见 `functional-test-report.md` 的「收尾复核」章节；机器可读证据：`finalize-report.json`、`restore-config-report.json`、`functional-final.log`、`functional-results-run1.json`（第一批）、`functional-run2-invalid.log`（作废批）。

### 最终收敛（16/20，80%）

`diagnose-overlays.py` 逐步测量**证伪了「应用重启后残留浮层」**：新窗口初始就是干净的（64 节点、零浮层）。247 节点是 `home()` / `scrub()` 盲目点击通用关闭按钮自己点开的。修法：`home()` 先判断、干净就不动；`rerun-functional.py` 受管启动且只在真有首启弹窗时才点关闭。

第四、五批在干净前置下**稳定 16 PASS / 4 FAIL**，F16 的「断言过严」也修正为只比较开关 `value`（忽略 `focused`），转 PASS。剩余 F03/F04/F12/F18 是**用例间状态耦合**导致未形成有效观测（`is_clean_workbench()` 不检查焦点与浮层内部状态，F02 模型切换 / F20 浮层会影响后续用例），不是产品缺陷，也不计入通过。F09 在干净前置下直接 PASS，进一步印证两个「疑似产品问题」的撤回。

**更正上一批的配置判断**：`planFirst` 由 `writeTaskPermission()` 写入 `localStorage` 的 `workbench.task.permission:v1`，**不是会话级**，按 origin/端口隔离（测试用 4322–4325，用户日常端口不受影响）。「规划模式」菜单项是 toggle，重选「请求批准」不会清标记。`clear-plan-first.py` 在 4325 点该菜单项一次，`授权模式: 请求批准 · 先规划` → `授权模式: 请求批准`，`status=RESTORED`。

**该批次口径：稳定通过 16/20，不宣称 20/20**；要拿满需每个用例前彻底重置渲染器状态。证据：`overlay-diagnosis.json`、`clean-run-report.json`、`functional-clean-run.log`、`clear-plan-first-report.json`，脚本 `diagnose-overlays.py` / `rerun-functional.py` / `clear-plan-first.py`。

### 用例级隔离复验（最终口径：18/20 行为已验证）

`isolated-cases.py` 给 F03/F04/F12/F18 各起独立窗口与端口（`ONLY_CASES` 单用例模式），并加了首启权限弹窗清除、窗口退出等待、连续干净观测稳定期。

- **F03、F04 在独立窗口下 PASS**（`menuitem=5`；规划模式 label 生效且 finally 还原），坐实此前是状态耦合而非产品缺陷。至此 **18/20 个功能行为已验证通过**。
- **F12、F18 仍未形成有效观测**：F12 的 AX `textfield.value` 读不到（`None`），`set-value` 返回失败而视图 65→110 变化无法区分「输入触发」还是「页面加载完成」，不做通过判定；F18 末轮遇到 Tauri 单实例启动竞态（baseline 0 节点、`ready=False`）。
- 新定位的两个环境限制（非产品缺陷）：全新端口的**首启权限弹窗会在用例中途出现**（曾把 64 节点工作台替换成 12 节点 dialog）；**Tauri 单实例**在前窗未退出时会激活旧窗口而非新建。已分别加 `dismiss_permission_modal()` / `wait_ready()` / `wait_no_window()`，但冷启动仍不保证每次成功。

该阶段：**18/20 行为已验证，2 个受工具限制未验证**。证据：`isolated-results-run1.json`、`isolated-results-run2.json`、`isolated-results.json`、各 `isolated-<ID>.json/.log`。

### 优化收敛（19/20，95%，定稿）

`probe-input.py` 逐个探测连接器搜索框，**更正了「F12 功能生效」的误判**：`set-value` 与 `type`（均为 strict headless semantic）都返回 `ACTION_FAILED`，`get --property value` 恒为 `""`，视图 109→109 无变化；早期 65→109 只是卡片加载完成。该受控输入组件只能被物理键盘事件驱动，本轮无 `--headed` 授权，因此 F12 是**工具边界**，不是产品缺陷。要打通需产品侧补可读 AX value 或授权物理输入。

F18 侦察显示「返回应用」回到的是**进入设置前的页面**（连接器目录 65 节点、`全部` 可见），是正确产品行为；原断言硬要求 composer 出现写错了。改为「离开设置页 + 回到主应用区域」后 PASS（`left_settings=True, in_main_app=True`）。`rerun-functional.py` 另加 `wait_ready()` 连续干净观测与弹窗清除。

该阶段：19 PASS / 1 工具边界 FAIL（95%）。证据：`input-probe.json`、`functional-clean-run.log`、`clean-run-report.json`，脚本 `probe-input.py`。

### 定稿：20 PASS / 0 FAIL（100%）

**更正「F12 是硬工具边界」的结论**：`probe-input2.py` 试了五种 headless 输入组合，`type` 的错误详情自带提示（Chromium/Electron 受控字段要先读再 `set-value`）。实测 **`focus` + `set-value` 成功**（直接 `set-value` 才 `ACTION_FAILED`）；`focus+type`、`click+type` 仍失败，`press` 因窗口焦点未确认 `STALE_REF`、剪贴板 helper 缺失。F12 改顺序后输入框值真实读到 `zzzz-no-such-connector`，连接器列表 **109→79**（过滤生效），完整验证且**无需 `--headed`、无需改产品代码**。上一节「需要产品侧测试钩子」作废。

同时修三处稳定性：`restore_permission_label()` 改用「规划模式」toggle；`rerun-functional.py` 加冷启动预热并修 `wait_window()` 对 launcher 退出的误判；F03/F04 按 AX 前缀定位授权 chip 并重试、F19 用页面签名变化、F20 加前置归一化与前缀定位。

**最终 20 PASS / 0 FAIL（100%）**。`functional-results.json` 为定稿；`clean-run-report.json` 显示 `ready=True`，baseline 57 / after 64 节点均为**干净工作台、零浮层、composer 在、`授权模式: 请求批准`**，`permission_restore.status=clean`——**运行前后环境一致，无配置或浮层副作用**。证据：`input-probe2.json`、`functional-clean-run.log`、`clean-run-report.json`，脚本 `probe-input2.py`。

**20/20 只代表本目录定义的 20 个功能用例全部通过**，不等于全应用功能穷尽；真模型任务、故障注入、错误/流式态、键盘焦点环、reduce 开启态、未到达的静态路由仍不在覆盖内。
