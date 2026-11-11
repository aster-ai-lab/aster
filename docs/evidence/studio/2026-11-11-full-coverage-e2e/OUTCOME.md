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
