# 全功能 agent-desktop 原生触达：第 1–2 批

日期：2026-11-11。目标是把 508 项 TUI 全功能验收矩阵推进到「零未触达」。本文件只记录实际执行、实际计数与未完成的边界。

## 口径与盘点

- 计划清单 `crates/code/manual/tui-e2e/20261110/catalog.json` 是 `PLANNED_ONLY`，508 个用例经参数展开后是 **1047 个执行行**，其中 161 项要求原生 agent-desktop 窗口（N lane）、88 项要求真模型或故障注入（L/F lane）。
- 源码 `inventory.json` 明确有 **38 个 unavailable local commands、18 个 unavailable settings、3 个 shell-gated commands**；`coverage-matrix.md` 的 D 类是代码返回 `null` 或无 UI 的形态；Studio 工作台缺「知识库」「小程序」路由。这些只能验证诚实拒绝（G lane），不能变成可用功能。
- 计划 README 明确：「Do not use a planning check, a screen keyword, a mock result, an ignored test, or a model's success narrative as a product pass.」因此本批的完成单位是 **入口触达 + AX/像素证据**，不是 oracle 通过。

## 第 1 批：命令入口

在独立隔离目录 `.runtime/tui-full-e2e/full-touch-20261111/` 中，用真实 Terminal 窗口逐个提交 slash 命令，每项留一份 AX 文本帧和一张 `screencapture -l` 截图。

| 指标 | 结果 |
| --- | --- |
| 唯一命令入口触达 | 69 |
| AX 中观察到明确拒绝 | 30（如 `TUI command /tutorial is unavailable with the Code backend. Use /help...`） |
| 其它观察响应 | 38 |
| 截图 / AX 帧 | 69 / 69 |

按 `catalog.coverage.commands` 映射（匹配 76/88 个 key、115 个 case id），把 **97 个用例从 `NOT_RUN` 改为 `PARTIAL`**，写入 `execution_rows: TOUCHED_NATIVE`。备份 `campaign.json.batch1-backup`。

## 第 2 批：状态变更命令与命令名收敛

第 1 批把会终止/替换会话的命令隔离了。第 2 批让每个这类命令在**同一个 Terminal 窗口内重启一个干净的 fullscreen 会话**后提交，断点续跑逐轮收敛：

`minimal / fullscreen / clear / compact / new / fork / resume / debug / delete / gboom / quit` — **11/11 全部触达**。

合计结果：

| 指标 | 结果 |
| --- | --- |
| `inventory.json` 唯一命令名 | 80 |
| 已触达 | **80 / 80，未触达 = 0** |
| 截图 / AX 帧 | **80 / 80** |
| campaign 状态 | `452/56/0` → `355/153/0` → **`345 NOT_RUN / 163 PARTIAL / 0 PASS`** |
| CMD 族 `NOT_RUN` | **清零** |

`PASS` 保持 0 是刻意的：只有入口触达证据，oracle 未验证。

## 工具根因（均为测试工具问题，不是产品失败）

1. composer 解析器把欢迎屏上半块 `▀` 装饰条当成草稿内容。
2. `run_records` 异步落盘，`Ctrl+O` 之后的 `records[-2]` 索引竞态；改为从 relay trace / session store 核验已发生的插入，不重放模型请求。
3. Terminal 窗口身份抖动：`activate-terminal` 加 5 次重试；复用窗口时 `cmd+v` 失效，改用 AX 键入。
4. **假启动**：`wait_for` 读到上一次退出残帧里的 `DeepSeek Flash · YOLO` 就判定就绪 → 改为要求 CLI 进程存在、frame 含 composer、且没有 `[进程已完成]`。
5. **`zuexec`**：AX 键入启动行被写成 `zuexec ...`，`zsh: command not found`，TUI 从未起来 → 精确解析 shell 命令行 + 最多 5 次清行重试。
6. **误发 `:q!`**：外部编辑器恢复条件里的裸 `~` 命中状态栏 home 缩写路径 → 收紧为必须匹配 vim 缓冲区特征。
7. `/edit-prompt` 拉起外部 vim、`/vim-mode` 让 `/` 变成搜索命令，均单独留证后隔离；私有 state 被状态命令污染后备份重建 `state/` / `workspace/`。

## 验证与默认链路

- 默认成对产物仍为 `aster` 899,194,232 B、`aster-acp` 308,714,456 B；BUILD_ID `58adbc450c…` 未变。
- 输入法恢复 `com.apple.inputmethod.SCIM.ITABC`；本批拥有的 CLI/ACP 进程已 TERM 清理。
- 私有 harness、80 张截图与 `campaign.json` 都在 `.runtime/tui-full-e2e/`（被 `.git/info/exclude` 排除），**未进入仓库**；本目录只有 2 个证据文件被提交。

## 剩余与下一批

`NOT_RUN` 345 项：`SETTING 86`、`KEY 76`、`SURFACE 27`、`TOOL 25`、`FIELD 16`、`COMPONENT 13`、`INPUT/CANCEL/READ/TOOLS 各 8`、`MODEL/ELICIT 各 7`、`BOOT/SLASH/CONFIG/FAULT 各 6`、`DISPLAY 5`，以及 WELCOME/PERM/FORM/SESSION/EXT/MEMORY/SOAK/PACKAGE/GOAL/QUEUE 等小族。163 个 `PARTIAL` 仍缺 oracle 判定。结构性不可达项按 G lane 单列，不计入可触达分母。

下一批优先 `SETTING` 与 `KEY`（需要逐项打开设置面板、发送绑定键，交互方式与命令入口不同），并为 `PARTIAL` 补 oracle。

## Explicitly not claimed

- 508 项矩阵已完成、任何用例已 `PASS`、或「全功能零未触达」已在整体意义上达成（目前只在**命令入口维度**达成 80/80）。
- 结构性不可达功能变得可用；`--headed` 物理输入授权、真模型/故障注入 L/F lane、跨平台验证已完成。
- 既有 3 个 `aster-pager` lib 失败、18 个历史 `cargo fmt` 文件被修复。
- 本批提交包含 `.runtime`、凭据或并行任务的 WIP；推送前已 `git fetch origin` 确认 `0/0`。
