# 测试覆盖率与工程债修复计划

> 状态：**S1 已完成**（4 类真实 part 组件断言、R9 checklist、live gate 约定）。承接 `AGENT-DESKTOP-E2E-PLAN.md` v2 与 `docs/evidence/studio/2026-11-13-live-llm-e2e/OUTCOME.md`（第二、三轮）。**S2 起仍为规划**：S2 需 R1 裁决，S3 需 R3 系统侧动作，S4 需推送授权。
> 事实基线（2026-11-13 03:38 实测）：HEAD `ed054809b`，**本地领先远程 11 个 commit 未推送**；父仓 8 个 modified + 2 个未跟踪目录（并行 WIP）；子模块干净；4318/4325/4388/4598 全空闲；`agent-desktop list-apps` 仍 `TIMEOUT`。
> 目标不是「冲 100%」——`F06/F09/F12/F15` 结构不可达决定了上限。目标是**把可测面吃满、把阻塞归位、把工程债清零**。

## 0. 判定口径：什么算「修好」

| 问题 | 修好的判据 |
| --- | --- |
| GUI E2E 阻塞 | `agent-desktop list-apps` 在非受限上下文返回 `ok=true`，且一条既有合规用例（F11）重跑 PASS |
| 真模型渲染 | 真实 turn 的每种 part 在组件渲染层有通过断言，且 `apiOk && renderOk` 双绿 |
| 覆盖率 | MATRIX 的 F 行、结果表、口径注同步更新；每次计入都有两次独立 PASS 与 OUTCOME 轮次 |
| 工程债 | `git rev-list --count origin/main..HEAD == 0`、`git status --porcelain` 为空、版本号 > `0.2.2` |
| 流程 | 失败先取 `error.details` 再动手；任何「既有/环境」判定必须带根因证据 |

**共同纪律**：不放宽断言凑绿、不缩小分母、不把 L2 桩/Gate A/合成 oracle 计入用户口径、每批结果写 `Explicitly not claimed`。

## 1. 关键路径与优先级

```
R1 裁决 ─────┬──> R5 P1-a 五条计入 ──> F 层 7/17
             └──> R6 键盘类断言（需 R3）
R2 真实 parts → 组件渲染   ← 唯一不依赖任何外部决策的高 ROI 项，先做
R3 解除 /bin/ps ──> GUI E2E 全线恢复 ──> R6 / R7
R8 工程债 ──> 发布前置（独立线，随时可做，但需授权）
R4 真 host 渲染通道 ──> 产品/工程改造，或判定由 R2 替代、不重复投入
```

**排序理由**：R2 不依赖 P0 裁决、不依赖系统权限、不依赖产品改造，且直接对应用户「组件渲染」诉求 → 第一优先。R1 是零成本分叉点，一条回复即可解锁五条用例。R3/R8 需要 Lyon 侧动作。R4 投入大且与 R2 目标重叠，建议先不做。

## 2. 修复项逐条

### R1 · P0 口径裁决（Lyon，零成本，阻塞 R5/R6）

- **问题**：`MATRIX.md:36,68` 以「L2 桩不计入」排除 F01，但 `PLAN.md:245-257` §9.2 只要求「执行面 ≥ 最低执行面」，排除的是 Playwright/内存桩。`draft-navigation-visual-e2e.py` 是 `agent-desktop launch` 全程真窗口、两次 PASS 的合规用例。
- **动作**：确认「真窗口上的 L2 断言是否算达标」。若通过 → 改 MATRIX F01 行、结果表（2/17）、口径注；若否定 → P1-a 全部降级为抽样，数字不动。
- **验收**：MATRIX 与 OUTCOME 一致，无「计入以裁决通过为前提」的悬置表述。
- **责任**：Lyon 一句话；我执行改表。

### R2 · 真实 parts → 组件渲染（我，最高 ROI，先做）

- **问题**：4 次真实 turn 已产出 `step-start` / `text` / `reasoning` / `data-context-usage` 四种真实 part，但 222 项渲染形态里 B/C/D 类全部未执行；真 native host + 静态 preview 的 IPC 不兼容使 Playwright 渲染断言长期 FAIL。
- **动作**（绕开 host IPC，不碰真窗口）：
  1. 从 `live-conversation-e2e.mts` 导出一次真 turn 的**脱敏 parts 快照**（模型文本可保留结构、去掉任何凭证与个人信息），固化成 fixture 放 `docs/evidence/studio/2026-11-13-live-llm-e2e/fixtures/`。
  2. 在子模块新增一个 vitest 用例，把该 fixture 喂给 `MessagePartsRenderer`，断言四种 part 各自渲染出对应组件与 AX 形态（`data-ui`、`role`、`aria-busy` 终态）。
  3. 复用现有 `MainTextBlock` / `ThinkingBlock` / `MessageCostSummary` / `MessageTokenDetailsCard` 测试，补「真实 part 输入」这一条路径，不复制业务逻辑进测试。
  4. 在 222 项 `coverage-matrix.md` 里把由此实际触发的编号从「未执行」改为「已执行（真实模型 parts + 组件层）」，并**单独标注执行面是组件层、不是真窗口**。
- **验收**：vitest 绿、`git diff --check` 绿、至少 4 类 part 有断言；**不声称 F 层数字变化**（组件层不计入用户口径）。
- **✅ 已完成（2026-11-13）**：真实 turn 捕获 3 类 part 并固化成 fixture；在 `MessagePartsRenderer.test.tsx` 追加 1 条用例复用 `renderParts` helper；两次独立 `94/94 PASS`；oxlint 与 HEAD 同（既有 warning，未引入新问题）；子模块 `bbc7bb9c01` 已推送、land-check EXIT 0、父仓 gitlink 已 bump。**S1 的「≥4 类」未达成**（本轮模型未产出 `reasoning`），且 `coverage-matrix.md` 编号体系是真窗口截图场景、无对应维度，按不造假原则未改 222 项统计。详见 `2026-11-13-live-llm-e2e/OUTCOME.md`「第二轮」。
- **责任**：我（已完成）。

### R3 · 解除 `/bin/ps` 系统级阻塞（Lyon / 系统侧）

- **问题**：`agent-desktop` 硬编码 `/bin/ps`（SIP 保护），报 `ps: Operation not permitted (os error 1)`；`open -n -a` 从 Launch Services 上下文预热仍 TIMEOUT；libproc shim 因不走 PATH 无效。
- **动作**（按顺序试，每步记录结果）：
  1. 在**非受限登录终端**（Terminal.app / iTerm2）直接执行 `agent-desktop list-apps`，确认 `/bin/ps` 是否可执行。
  2. 可执行 → 把 GUI E2E 整体迁到该上下文执行（脚本不改）；不可执行 → 重启机器 / 重置辅助功能授权后复测。
  3. 若系统层始终拒绝 → 向 agent-desktop 上游提需求：进程枚举改 libproc / `sysctl`，去掉 setuid `/bin/ps` 依赖。
- **验收**：`list-apps ok=true` 且 F11 `AGENT_DESKTOP_LAUNCH_L3: PASS` 重跑成功。
- **责任**：Lyon 操作；我提供复测脚本与判据。**在此之前不再尝试任何预热/重试**（已在第三十四轮证明无效）。

### R4 · 真 native host 的 renderer 通道（产品/工程，建议暂缓）

- **问题**：`e2e-preview.mjs` 是 renderer-only L2 静态桩，IPC 与真 host 不兼容（首启对话框 / `Invalid native preference snapshot` / 404）。
- **选项**：① 让 `just studio` 起完整 dev renderer（4318 热更）+ 隔离实例，Playwright 连真窗口；② 给 native host 补 `STUDIO_RENDERER_DIST` 自 serve 的可用开关；③ 统一 preview 的 IPC stub 与真 host 契约。
- **建议**：**暂缓**。②/③ 是产品改造，① 仍受 R3 阻塞；R2 已能用更低成本覆盖「真实输出 × 组件渲染」。若后续必须验「真窗口里的消息气泡」，再在 R3 完成后回来做 ①。
- **责任**：暂不排期；记录为已知缺口。

### R5 · P1-a 五条计数（我，需 R1）

- **问题**：F05/F08/F10/F16(UI)/F17 已确认可写且不依赖键盘，但 P0 未裁决时计入无依据。
- **动作**：R1 通过后，按 `AGENT-DESKTOP-E2E-PLAN.md` §9 的逐功能规格表逐条写用例（单次 launch、纯语义、≤30s），每条两次独立 PASS 才计入；F08/F10 已有脚本，只需在裁决后按 §4 补全必须断言。
- **验收**：F 层 2/17 → 7/17（裁决通过前提）；每条有 OUTCOME 轮次与 `Explicitly not claimed`。

### R6 · 键盘类断言与 frontmost（我，需 R3）

- **问题**：窗口 `focused=false visible=true`、foreground 列表无 Aster，`type --headed` 全报 `focus_verification`（`retry: unsafe`），F02 中文输入/斜杠命令/队列、F01 补断言、F10 命令面板快捷键都卡在这里。
- **动作**：R3 解除后先跑一次 `focus-window --app Aster` 的最小诊断（之前因 host 退化未验证成功），确认能否解锁；再补 F02 完整用例与 F01 两条断言。
- **验收**：F02 完整断言两次 PASS，F 层 +1（裁决通过前提）。

### R7 · 4 项结构不可达（产品侧，只列缺口不写用例）

| ID | 解锁所需产品改造 |
| --- | --- |
| F06 | ACP fixture 发 `session/request_permission`，或提供可注入的待审批状态；终态需 UI 可判定 |
| F09 | 暴露产生工作区 tab 的操作；detach / 新窗口焦点需可断言入口 |
| F12 | 产出可校验的物理文件；明确「磁盘字节回读」在口径中的地位 |
| F15 | 非交互截图接口 + pooled 窗口的 AX 入口（当前 `recordingControl = null`、`screencapture -i` 需物理鼠标） |

**不在测试侧修**。写多少 UI 用例都无法达标，投入应放产品侧。

### R8 · 工程与发布债（我 + Lyon 授权）

| 项 | 动作 | 前置 |
| --- | --- | --- |
| 11 个未推送 commit | `git push origin main`（**单独授权**） | Lyon 确认 |
| 父仓 8 M + 2 ?? 并行 WIP | **不动**（非本线任务）；发布前由其主人处理或明确归档 | 对应任务负责人 |
| 版本仍 `0.2.2` | 子模块三处同步 bump 到 `0.2.3`+ → 提交推送 → 父仓 gitlink 再 bump | 发布决策 |
| Node 22.22.2 / `pnpm` bun 规范 | 装 Node 24.12 或确认 `just studio-contracts` 等价 vitest 入口 | 环境 |
| CI 无 LLM secret | **不要加**：真模型保持本地手动，靠 `ASTER_E2E_LIVE_MODEL=1` gate | — |
| 真模型治理 | 把脚本级 gate 提升为仓库约定：默认 ignore、单次 turn 上限、预算/超时上限、失败保留脱敏回执 | 我 |
| safe-delete guard | 已 best-effort 处理；记录为环境限制，不绕过 | — |
| 全量 renderer 套件 20m 超时 + 既有失败 | **第一步已完成（第三十五轮诊断）**：非单文件死锁，是规模×吞吐（172 文件 15 分钟只跑完 116；1572 文件外推 ≈3.4h）→ 应分片/调 CI 超时，而非修「超时文件」；chat 子目录另有 **7 文件 / 27 条既有失败**（与本会话改动无关）。`WorkspaceTaskDetails` 3 例根因已定位。按 AGENTS.md **记录不修**（不阻塞当前任务）。后续：分片方案 + 独立排期逐批修过时断言 | 我，独立任务 |

### R9 · 流程改进（我，防止判断失误复发）

本会话已出现的失误与对应规则：

| 失误 | 规则 |
| --- | --- |
| 把两处可修的测试失败判成「既有/环境」放过 | 失败必须单跑拿根因，才允许归类 |
| host 反复预热多轮才读 `error.details` | 环境类失败第一步取完整 `error.details`，禁止连续重试 ≥3 次不改诊断方向 |
| F08 把 `None` 当合法状态、live 首版只凭 `apiOk` 报 PASS | 判定必须覆盖用户诉求的全部维度；`None`/缺失永不等于「变化」或「通过」 |
| 同一规划写了两版 | 新请求先读现有文档，增量更新而非重写 |
| 擅自改数字/放宽断言 | 任何计入都要两次独立 PASS + OUTCOME 轮次 + 免责段 |

**R9 已固化为以下可勾选 checklist（每轮 E2E 开工前逐条确认）**：

- [ ] 环境类失败**第一步读取完整 `error.details`**，禁止在未换诊断方向前连续重试 ≥3 次
- [ ] 失败先单跑拿根因，才允许归类为「既有 / 环境」；证据写进 OUTCOME
- [ ] `None` / 缺失 / 空值**永不当作合法状态或通过**；控件随状态改名时按名字判定
- [ ] 判定条件覆盖用户诉求的**全部维度**（例：真模型用例必须 `apiOk && renderOk`，不能只凭 API 成功）
- [ ] 新请求先读现有文档，增量更新而非重写
- [ ] 任何计入覆盖的用例：两次独立 PASS + 更新台账 + OUTCOME 轮次 + `Explicitly not claimed`
- [ ] lint / 测试警告必须与 HEAD 同目录对比后再判「既有」，不顺手改无关代码、不放宽 gate
- [ ] 真实模型调用：`ASTER_E2E_LIVE_MODEL=1`、单次 turn、超时上限、凭证仅内存、回执脱敏
- [ ] 收尾：ACL 还原、端口空闲、无残留进程、构建基线重新对齐

## 3. 阶段与量化 Checkpoint

| 阶段 | 内容 | 出口判据 |
| --- | --- | --- |
| **S1（可立即做）** | R2 + R9 + R8 的治理项 | **✅ 已达成**：R2 补到 **4 类真实 part**（`step-start`/`reasoning`/`text`/`data-context-usage`，2× 94/94 PASS），R9 checklist 已固化于 §2 R9，live gate 约定已固化于 live-llm OUTCOME §7。仍如实保留：`coverage-matrix.md` 编号体系不匹配，222 项统计未改 |
| **S2（需一条裁决）** | R1 → R5 | F 层 **2/17 → 7/17**（裁决通过）；五条用例各两次 PASS |
| **S3（需系统侧动作）** | R3 → R6 | `list-apps ok=true`；F02 完整断言 PASS；F 层 → **8/17** |
| **S4（需授权）** | R8 推送/版本/工作树 + R7 产品清单移交 | 0 未推送 commit、工作树干净、版本号 > 0.2.2、四项缺口有产品侧 owner |
| 不做 | R4（暂缓）、F06/F09/F12/F15 测试侧投入、发布执行 | 明确记录为不修项 |

**上限诚实**：不改产品约 7–9/17（P2 fixture 顺利可到 10–11/17），4 项结构不可达决定 100% 不可达。

## 4. 责任矩阵

| 方 | 负责 |
| --- | --- |
| 我 | R2、R5、R6、R8 治理项、R9、全部落证与台账同步 |
| Lyon | R1 裁决、R3 终端操作、推送授权、发布决策 |
| 系统/上游 | `/bin/ps` 权限、agent-desktop 进程枚举实现 |
| 产品侧 | R4（若做）、R7 四项入口/接口、F13/F14/F16(L3) 改造 |
| 并行任务负责人 | 父仓 8 M + 2 ?? WIP |

## 5. 风险与不修项

- **R1 若被否定**，S2 的五条用例全部降级为抽样，F 层维持 1/17，计划需重排（已在 v2 §10 给了合理假设）。
- **R3 若长期无解**，GUI E2E 永久停在现有 6 条合规用例，键盘类断言不做；不因此放宽口径。
- **R2 的组件层结果不计入用户口径**（15:06 要求 agent-desktop 可视化 E2E），只作为 F03/F07 的真实模型补充验收。
- **发布执行不在本计划**：版本 bump、双语 notes、workflow_dispatch、tag、网站刷新需单独授权与决策。
- **不动别人的 WIP**：父仓 8 个 modified 文件与 2 个未跟踪目录一律保留。

## Explicitly not claimed

- 本文件是规划，**未执行任何修复、未改任何覆盖数字、未推送任何 commit**。
- 不声称 R3 可在本会话内解决，不声称 R4 已定方案，不声称 100% 可达。
- 不声称 S2/S3 的数字是既成事实——它们以 R1 裁决通过、R3 解除为前提。
- 不声称全量 renderer 套件、Gate A/B、4 项结构不可达在本计划内被修复（R8 只列了独立排期方向）。
