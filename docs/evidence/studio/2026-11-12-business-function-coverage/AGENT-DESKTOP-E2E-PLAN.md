# 基于 agent-desktop 的 F 层端到端覆盖率提升规划

> 状态：**规划稿**，未执行任何用例。事实基线来自只读调研（PLAN §4/§9、MATRIX、OUTCOME、specs、renderer 源码、agent-desktop help）。
> 目标：在用户口径下把 F 层从当前 **1/17** 尽可能往上推，并明确哪些项目不可能靠写用例达标。

## 0. 口径、现状与一个待裁决点

**用户口径（15:06 起多次强调）**：测试必须由 `agent-desktop launch` 启动应用，全程在真实 Tauri 窗口上做可视化端到端断言；Playwright + runtime 桩、Gate A 套件不计入。

**权威判定**：`docs/evidence/studio/2026-11-10-page-feature-component-e2e/PLAN.md:245-257` §9.2/§9.5 —— 每条 `Pxx.Fyy` 至少 1 条通过用例且**执行面 ≥ 最低执行面**；标 L3 的功能用 L2 桩通过不计覆盖。真宿主证据须含构建 hash、启动命令、窗口来源、input receipts、artifacts、cleanup 六要素。

**现状**：F 层 1/17（仅 F11）。完全合规且 PASS 的可视化 E2E 有 3 条：F11 偏好持久化（两次复跑）、F01 跨页草稿（修复产品 bug 后两次 PASS）、F14 单实例抽样。真宿主用例 7/7 通过（这是用例通过率，不是功能覆盖率）。

**⚠️ 待 Lyon 裁决（预计 +1，零代码成本）**：`MATRIX.md:36,68` 以「L2 桩按新口径不计入」为由把 F01 排除在覆盖外。但 `draft-navigation-visual-e2e.py` 是 `agent-desktop launch` 启动、全程真窗口断言的**完全合规用例**，其执行面是 L3（真窗口）≥ F01 的最低执行面「L2 + L3 抽样」中的 L2 部分；被 §9.2 排除的是 Playwright/内存桩。若确认「真窗口上的断言即满足口径」，F01 应从「抽样 PASS 未计入」改为达标，**F 层立即变为 2/17**。本规划不擅自改数字，等裁决。

## 1. 事实基线

### 1.1 可复用骨架（`specs/`）

| 脚本 | 提供什么 |
| --- | --- |
| `agent-desktop-launch-e2e.py` | **主骨架**：`ax()` JSON 封装、`install_wrapper()` 造 `AsterVisualE2E.app`、`launch/wait_for_window/dismiss_onboarding`、`ref/open_settings/switch_value`、`acl_path()`、`close-app --force` + relaunch |
| `draft-navigation-visual-e2e.py` | `type_draft()` = `focus → 重新 find → type --headed`（composer 唯一可用输入序列）、`composer_value`、`clear_draft`、`visible_text_count` |
| `single-instance-visual-e2e.py` | `visible_aster_windows()` / `aster_main_apps()`（`list-windows`/`list-apps` 计数与 PID 比对） |
| `visual-e2e-launcher.sh` | 在 Launch Services 上下文重启 agent-desktop host，解决 `ps: Operation not permitted` |
| `f13-first-run-e2e.py` | 真首启组件安装链路（当前 `PARTIAL_PASS`） |

### 1.2 agent-desktop 能力边界

- **可用**：`launch/close-app`、`snapshot`（`--app/--window-id/--surface/--include-bounds`）、`find`（`--role/--name/--exact/--count`）、`get`（`text/value/states/bounds/role`）、`is`（`visible/enabled/checked/focused/...`）、`click/toggle/check/uncheck/set-value/clear/focus/select/expand/collapse/scroll/scroll-to`、`type`、`press`、`resize-window`、`wait`、`screenshot`。
- **菜单不是子命令**：用 `snapshot --surface menu|menubar` + `find --surface menu`。
- **已知限制**：
  1. composer 是 contenteditable，`set-value` / headless `type` 均 `ACTION_FAILED`；必须 `focus → 重取 ref → type --headed`。设置搜索 combobox 相反：`set-value` 成功、`type` 失败。
  2. `focus` 使旧 ref 失效 → `STALE_REF`；动作可能已落地，`disposition.retry` 仅 `not_delivered` 才 safe。
  3. 只见 AX 树，**磁盘字节 / HTTP 后端状态 / 进程 lock 身份 / OS 级跨窗口焦点不可读**。F11 先例是把 Python 读 `.acl` 降级为 *evidence, never a test action*。
  4. 二次 `launch` 返回 `APP_UNRESPONSIVE`，NSWorkspace 未真起第二进程 → 未触发产品 `Claim::Busy`。
  5. `ps` setuid 被沙箱拒时全部窗口命令超时；**每轮跑用例前先 `agent-desktop list-apps` 确认 ok，否则用 `visual-e2e-launcher.sh` 预热 host**。
  6. `--headed` 口径有张力：`COVERAGE_UPLIFT_PLAN.md:139` 禁 `--headed`，但 `OUTCOME.md:643` 把 `type --headed` 判为「物理键盘、不涉坐标或鼠标，符合 S4」。**每条用例须在 OUTCOME 标注所用形态**。

### 1.3 每条用例的强制前置（AGENTS.md）

```
just build-studio                          # 根仓，native host
node scripts/e2e-fresh-build.mjs --build # 当前 tree 的 renderer
node scripts/e2e-fresh-build.mjs --verify
agent-desktop list-apps                    # host 健康确认，必要时预热
```
父仓提交会改 `rootCommit` 使 stamp stale，**最后一个动作必须是 build/verify**；OUTCOME 记录构建 hash 与启动命令，否则绿色结果无效。

## 2. 可测性分类（决定上限）

| 类别 | 功能 | 说明 |
| --- | --- | --- |
| **A 直接可做** | F01、F02、F05、F08、F10、F11、F16(UI 部分)、F17 | 有稳定 AX 入口，真窗口断言即可 |
| **B 需 fixture** | F03、F04、F07 | 需先在 UI 里造出会话/流式 turn/长列表 |
| **C 需产品改造** | F13、F14、F16(L3 抽样) | 实现与 §4 验收不符或分支不可触发 |
| **D 结构不可达** | F06、F09、F12、F15 | 见 §5，写多少 UI 用例都无法达标 |

**上限判断**：不改产品，靠 A + B 最多推到约 **7–9/17**；D 类 4 项必须由产品侧提供入口/接口/可注入前置状态。

## 3. 统一用例模板

所有新脚本放 `specs/`，用 `importlib` 复用主骨架（不复制基础设施），统一输出 `<ID>_L3: PASS|FAIL` 与 `EXIT 0|1`，`finally` 里 `close-app --force`，并还原所有被改的偏好/草稿。每条必须包含：

1. 前置：构建 + verify + host 预热 + 清理残留 Aster/Studio 进程与端口
2. `agent-desktop launch` 启动，`wait_for_window` + `dismiss_onboarding`
3. 数据准备（**只做前置，不作断言**）
4. 可视化断言：`snapshot/find/get/is`，至少一条 `find` 命中 + 一条状态回读
5. 如涉跨进程：`close-app --force` → 再次 `launch` → 回读
6. 磁盘/HTTP 读仅作 evidence，主断言留在 AX 层
7. 还原 + `close-app` + 端口/窗口清洁核对

## 4. 路线图

### P0 — 口径复核（目标 1/17 → 2/17）

| 动作 | 说明 |
| --- | --- |
| 裁决 F01 | 确认「真窗口 L3 断言满足 L2 最低执行面」后，改 `MATRIX.md` F01 行与结果表，并在 OUTCOME 记录裁决依据。零代码。 |
| 复核 F01 断言完整度 | `draft-navigation-visual-e2e.py` 现在只覆盖「跨页往返草稿保留」。按 §4 补「空草稿不落盘」「切 Tab 后状态保持」两条真窗口断言，避免只凭一条抽样就计入。 |

### P1 — 真窗口 L2 断言（目标再 +4~5，全部 A 类）

| ID | 用例 | 关键命令序列 | 判定 |
| --- | --- | --- | --- |
| **F02** | composer 输入/斜杠命令/附件/队列 | `focus → re-find → type --headed` 输入中文与 `/clear`；`find --name '斜杠命令'`、`get --property value`、`find` 队列 dock 文本 | 命令菜单项与 `promptText` 一致、附件加入后草稿不变、空白输入不落盘 |
| **F05** | 模型选择整链 | `find --role tab` → `find --role tablist` → `set-value` 搜索框过滤 → `press` 上下键 → `get --property value` | 唯一 ModelSelector 实例、选中态/思考强度变更与还原一致 |
| **F08** | 项目/侧栏重排固定 + 重启抽样 | 侧栏 `setPinned/reorder` 对应按钮 → `close-app --force` → `launch` → `find --name` 顺序/固定态 | 重启后顺序与 pin 仍生效（复用 F11 重启骨架） |
| **F10** | 命令面板/全局搜索 | `press cmd+k` 类拉起 → `set-value` 写「搜索设置...」→ `find --count` 结果项 / 空结果态 | 结果可清除、空态文案、恢复开会话 |
| **F16(UI)** | 错误态/无障碍 | `find --name` 空/加载/重试/离线文案、`is --property focused`、`snapshot --surface alert` | 文案固定 locale、焦点返回、name/role/value 完整 |
| **F17** | 外观矩阵（受限） | `resize-window` 窄窗口 + `snapshot --include-bounds` | 无重叠/溢出；注意 `/settings/appearance` 隐藏，主题切换入口受限，只能做缩放与溢出 |

> P1 的每一条都按 §9.2 先确认「最低执行面」，L2 功能用真窗口断言即满足；若裁决不通过，P1 全部只算抽样、不涨数字——**所以 P0 裁决是 P1 的前置**。

### P2 — 需 fixture（目标再 +2~3）

| ID | 前置准备 | 用例与判定 |
| --- | --- | --- |
| **F03** | 用 `apps/studio/tests/e2e/connectors/model_server.py` 起合成模型，在 UI 里发起一次真实 turn | `is --property` 读 `aria-busy` → `find` Stop 按钮 → 终态不重放；工具披露折叠/聚合行、引用可跳转 |
| **F04** | 造 >100 条消息的会话 | `scroll --direction down --amount N` 后切走切回，`get --property bounds` + `find --count` 断言位置恢复与节点回收 |
| **F07** | `agent-desktop launch` 起带 synthetic MCP 的实例（`tests/e2e/capability_mcp.py`） | UI 内发起任务并读回执；**磁盘/HTTP 回读只作 evidence**，照 F11 先例；不复制现有 Playwright `nativeHarness` 形态 |

**共同风险**：fixture 由外部进程/HTTP 注入，与「全程可视化」口径有张力。每条须在 OUTCOME 明确「fixture 仅作前置准备，所有断言在 AX 层」。

### P3 — 需产品改造（不承诺数字，先列清缺口）

| ID | 产品侧需要提供 | 之后可做的用例 |
| --- | --- | --- |
| **F13** | 真实匿名安装 3 个 runtime 组件，或修订 §4 的「3 个」验收；`/settings/dependencies` 当前隐藏 | 复用 `f13-first-run-e2e.py` 首启链路，升级为 `agent-desktop launch` 形态 |
| **F14** | 可注入的更新通道/清单、`Claim::Busy` 可触发路径、退出守卫的进行中任务前置 | 补 `MainExitGuard` Dialog 断言 + 重启后 DB 旧数据可读；单实例抽样已 PASS |
| **F16(L3)** | 修复「杀进程重启草稿保留」（实测 `NOT_PERSISTED`），或把抽样点改为「进行中 turn 有明确终态」 | 用 `close-app --force` + `launch` 做崩溃恢复回读 |

## 5. 明确不投入：4 项结构不可达

| ID | 依据 | 要解锁必须 |
| --- | --- | --- |
| **F15** | `AgentComposer.tsx:1069-1070,1757` 硬编码 `recordingControl = null`；`/settings/screenshot` 隐藏；`screenshot.rs:146-147` 用 `screencapture -i` 需物理鼠标，与 S4 禁 `--headed` 冲突 | 产品提供非交互截图接口 + pooled 窗口的 AX 可驱动入口 |
| **F09** | 真窗口实测 `find --role tab` = 0，无任何工作区标签控件（`OUTCOME.md:544-554`）；detach/新窗口焦点需 `focus-window`（PLAN 禁） | 产品暴露产生工作区 tab 的操作 + 可断言的窗口焦点/标题 |
| **F06** | 批准/拒绝按钮存在（`PermissionRequestComposer.tsx:509-528`），但 synthetic ACP 不发 `session/request_permission`，`pendingApprovals` 恒空；「真实回写/两次终态可区分」须读后端 part 状态 | ACP fixture 发审批请求 + 产品提供 UI 可判定的终态 |
| **F12** | 「保存到文档/下载」按钮存在（`OutputActions.tsx:57-77`），但 §4 要求校验**真实字节**，office harness 自认内存边界（`office/README.md:47-49`），主断言必须落磁盘 | 产品产出可校验的物理文件 + 明确「磁盘回读」在口径中的地位 |

`OUTCOME.md:577-588` 已给出同向结论：要从个位数走到 11+，投入应放在**产品侧测试接口与前置状态**，而不是继续在 UI 上找入口。

## 6. 验收与落证规范

- 每条用例**至少两次独立复跑 PASS** 才计入；FAIL 按 fail-closed 记为发现、不计覆盖、不放宽断言凑绿。
- 记录：构建命令与 hash、启动命令、窗口来源、`snapshot_id`/ref receipts、UI 文本对照（`find --name <token>` 命中数）、cleanup 结果。
- 更新 `MATRIX.md` 的 F 行 / 结果表 / 口径注，在 `OUTCOME.md` 追加轮次（含「Explicitly not claimed」）。
- 不把 Gate A 补跑、Playwright 桩、L2 注入 fixture 计入用户口径的 F 层。
- 提交后按既有流程重跑 `e2e-fresh-build --build/--verify` 对齐 stamp。

## 7. 风险与待决

1. **P0 裁决未定**：这是整个规划的分叉点，影响 P1 全部用例是否算数。
2. **host 会退化**：`agent-desktop` 常驻 host 被 Bash 上下文 respawn 后 `ps` 又被禁；每轮先预热，必要时把预热固化进用例前置。
3. **`--headed` 口径**：`type --headed` 已被本台账接受，但 `COVERAGE_UPLIFT_PLAN.md:139` 文字上禁止；建议统一成「允许物理键盘、禁止坐标/鼠标拖拽」并写进口径注。
4. **工作树状态**：父仓仍有并行 WIP，发布前需干净工作树；本规划文档自身不要变成发布阻塞项。
5. **上限诚实**：D 类 4 项决定了 100% 不可能靠测试侧达成，目标应表述为「把可测面吃满」而非「全达标」。
