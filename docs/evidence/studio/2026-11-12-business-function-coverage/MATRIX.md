# 业务功能验证覆盖率台账（2026-11-12）

## 口径（不再自选分母）

沿用仓库已有的权威清单，不新造分母：

- **主口径 F 层**：`docs/evidence/studio/2026-11-10-page-feature-component-e2e/PLAN.md` §4「逐功能矩阵」的 **F01–F17 共 17 条业务功能**。
- **验收标准**（同文档 §9）：每条功能至少一条**通过**的用例，且**执行面 ≥ 该功能的最低执行面**；**标 L3 的功能用 L2 桩通过不计覆盖**。
- **辅助口径 Gate A**：`apps/studio/tests/e2e/COVERAGE_UPLIFT_PLAN.md` 定义的场景通过率。本轮实测 7 个 config 声明 **490** 个用例（旧文档写 483，settings 已从 20 增至 27）。
- **辅助口径 Gate B**：页面 × 组件台账，基线 1522 组件，要求 inventory 的 `components[].coveredBy`。
- C 层 20 个组件族、P 层 33 个页面，见同一计划 §3/§5。

> 「业务功能验证覆盖率 100%」在本台账中只指 **F 层 17/17**。Gate A 场景全绿不等于 F 层达标，因为绝大多数场景是 L2 渲染器桩。

## 执行面与现有用例事实（静态盘点）

`apps/studio/tests/e2e/` 实测：

| 事实 | 数值 |
| --- | --- |
| `*.spec.ts` 文件 | 83 |
| 使用 `installWorkbench`（内存桩 / L2） | 55 |
| 使用 `addInitScript`（注入桩 / L2） | 24 |
| **真宿主（L3）spec** | **1**（`connectors/connector-native.spec.ts`） |
| 无 spec 的功能域 | `interactive-coding/`、`document-tools/`、`office/`、`messages/`、`followup/` |
| 全仓 `matrix.json` | 从未生成（全量矩阵此前未跑过） |

> 上表是**本日开始时**的静态盘点，保留作为基线。后续新增的真宿主用例：`restart-persistence.spec.ts`（F14 部分）、`specs/f11-desktop-e2e.py`（F11 达标）、`specs/f13-first-run-e2e.py`（F13 部分）。当前严格达标 F07 + F11 = **2/17**。

`connector-native.spec.ts` 用隔离 `ASTER_STUDIO_INSTANCE` profile、synthetic `model_server.py`、真实 MCP SDK 与 native host（端口 4598），**不需要真实模型 API**。另两条真宿主用例是 `restart-persistence.spec.ts`（native host 重启）与 `specs/f11-desktop-e2e.py`（真 Tauri webview + agent-desktop）。

## F 层 17 条逐条台账

| ID | 业务功能 | 最低执行面 | 现有 L3 用例 | 本轮状态 | 达标缺口 |
| --- | --- | --- | --- | --- | --- |
| F01 | 会话/草稿生命周期与导航保持 | L2 + L3 抽样 | 仅 L2 桩 | 见 Gate A 结果 | 需 L3 抽样（真实跨页草稿保持） |
| F02 | Composer 输入/斜杠命令/附件/队列 | L2 | L2 桩 | 见 Gate A 结果 | 达 L2 即可 |
| F03 | 流式消息/工具披露/引用 | L2 + 契约门禁 | L2 桩 | 见 Gate A 结果 | 需契约门禁绿 |
| F04 | 消息虚拟列表与滚动记忆 | L2 + 契约门禁 | L2 桩 | 见 Gate A 结果 | 需契约门禁绿 |
| F05 | 模型选择整链 | L2 + 契约门禁 | L2 桩 | 见 Gate A 结果 | 需契约门禁绿 |
| F06 | 工具审批批准/拒绝 | **L3** | **无** | **未覆盖**（本轮已探测） | 已打通 HTTP 装 MCP + 真实 ACP turn，但 `permission_mode:'default'` 在 synthetic harness 下不发 `session/request_permission`，`pendingApprovals` 恒为空；详见 OUTCOME 第八轮 |
| F07 | 真实 ACP turn 与连接器/MCP 调用 | **L3** | **有 1 条** | ✅ **本轮 L3 通过** | 已达标（见 OUTCOME 第三轮） |
| F08 | 项目/工作区/侧栏重排固定 | L2 + L3 抽样 | 仅 L2 桩 | 见 Gate A 结果 | 需 L3 抽样（重启后 reorder/pin 仍生效） |
| F09 | 多标签与窗口编排 | **L3** | **无** | **未覆盖** | 需真窗口 detach、Ctrl+Tab、关最后一个关窗 |
| F10 | 命令面板/全局搜索/深链 | L2 | L2 桩 | 见 Gate A 结果 | 达 L2 即可 |
| F11 | 偏好与跨进程持久化 | **L3** | **有 1 条**（agent-desktop 脚本） | ✅ **本轮 L3 通过，严格达标** | 真 Tauri webview + 真 IPC + ACL 磁盘快照 + 真进程退出重启；四项必须断言全覆盖。见 OUTCOME 第十一轮 |
| F12 | 知识库/文件/文档真实字节产物 | **L3** | **无** | **未覆盖** | 需 DOCX/XLSX/PPTX/PDF/PNG 真实字节校验；`office/host-journeys.acl` 自认内存文件边界 |
| F13 | 首次运行/运行时组件/系统权限 | **L3** | **有 1 条（部分）** | ⚠️ `PARTIAL_PASS`，未严格达标 | 真实首启全链路 + 磁盘字节证据已验证；但产品只装 1 个 required 组件，PLAN 要求 3 个（另 2 个 optional 且依赖页隐藏）。见 OUTCOME 第十二轮 |
| F14 | 更新/退出守卫/单实例 | **L3** | **有 1 条（部分）** | ⚠️ 新 L3 用例通过，未严格达标 | 已验证「重启后旧数据可读 + 落盘」；仍缺更新通道/清单、单实例二次启动、退出守卫 |
| F15 | 截图/录屏/视觉标注 | **L3** | **无** | **未覆盖**（本轮已探测） | 真窗口无任何截图/录屏/标注入口；`screencapture -i` 需物理鼠标，与 PLAN S4 冲突。见 OUTCOME 第十四轮 |
| F16 | 错误态/离线/崩溃恢复/无障碍 | L2 + L3 抽样 | 仅 L2 桩 | 见 Gate A 结果 | 需 L3 抽样（杀进程重启草稿保留） |
| F17 | 外观矩阵 | L2 | L2 桩 | 见 Gate A 结果 | 达 L2 即可 |

**结构性缺口（本日开始时）**：8 条 L3 功能里 7 条完全没有 L3 用例。经多轮推进，现状是 **2 条严格达标**（F07、F11）、**2 条部分覆盖**（F13、F14，按 §9 不计）、**4 条仍完全无用例**（F06/F09/F12/F15）。按 §9「L3 用桩通过不计覆盖」，后 6 条无论 Gate A 跑出多少通过数都计 0。

因此 F 层当前理论上限 = 9 条 L2 功能 + F07 + F11 = **11/17 ≈ 64.7%**，前提是 9 条 L2 功能全部有通过用例且契约门禁绿。（F11 达标前该上限为 10/17；若再补完 F13/F14 的部分覆盖用例可达 13/17，但那需要新增断言。）

## 本轮实测结果

| 口径 | 分母 | 通过 | 失败 | 跳过 | 未执行 | 通过率 | 判定 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| **F 层（业务功能验证）** | 17 | **2** | — | — | — | **11.8%** | **未达成 100%** |
| Gate A（**6/7 config**，正式配置） | 490 | 181 | 40 | 62 | 207 | **36.9%** | FAIL |
| Gate A（**7 份日志合并**，含 composer-audit chromium 诊断运行） | 490 | **228** | 95 | 62 | 105 | **46.5%** | FAIL（目标 100%） |
| Gate B（页面 × 组件） | 1522 | — | — | — | — | **UNMEASURABLE** | 无合规 inventory |
| 真宿主验证用例 | **4** | **4** | 0 | 0 | 0 | **100%** | ✅ F07、F11 达标；F14、F13 部分覆盖 |

> **新口径（用户 15:06 追加）**：测试必须是 agent-desktop 启动应用的可视化端到端测试。按此口径，上表 4 条里只有 **F11、F13 两条**符合（agent-desktop 在真 Tauri 窗口上驱动），F07/F14 与全部 Gate A 套件是 Playwright + runtime 桩、**不计入**；F 层因此为 **1/17**（仅 F11 严格达标，F13 部分符合）。第十六轮又新增第 3 条符合形态的 `specs/draft-restart-visual-e2e.py`（F16 抽样），但结果为 `NOT_PERSISTED` 负面发现、**不计覆盖**。启动环节仍由 `just studio` 完成——`agent-desktop launch` 四次实测均无法启动实例 bundle，详见 `OUTCOME.md`「口径补充」。本轮 Gate A 补跑的 `142 passed / 10 failed / 55 skipped` 同样不计入。

- 6 config 官方门禁：`pass=181 fail=40 skip=62 executed=221 unexecuted=207`，执行内 81.9%，`structural ceiling=45.1%`，`deduped=3`。
- 7 份日志合并：`pass=228 fail=95 skip=62 executed=323 unexecuted=105`，执行内 **70.6%**，`structural ceiling=65.9%`，`deduped=3`。**第 7 份日志来自诊断配置（`expect.timeout` 与 `use.actionTimeout` 均 8s），不是正式门禁运行**；其通过/失败判定未改变（正式配置下通过用例最慢 3.2s、失败用例 20–23s），但口径须标注。剩余未执行的 105 例几乎全是 `composer-audit` 的 **webkit 引擎**（102 例）。
- 两次 `--stub-runtime` 全量尝试（16m50s、35m17s）均未完成；根因见 `OUTCOME.md` 第五轮：不是卡住，是失败用例各耗 20–23s 超时累积。

**F 层「2」= F07 + F11**，两条都在真宿主上严格达标：

- F07：真 native host 上 stdio/SSE/HTTP 三种 MCP 的真实 ACP turn、安装持久化、restart 后仍在、删除后不再调用。
- F11（第十一轮新增）：**真 Tauri webview** 改偏好 → `PreferenceStore` 写 ACL 快照 → 真进程退出 → 重启同实例 → UI 与磁盘双重回读；四项必须断言（IPC 回读、跨页即时生效、重启后从磁盘恢复、非 localStorage 假象）全覆盖，用例可复跑（`specs/f11-desktop-e2e.py`，`F11_L3: PASS`）。

另有**两条部分覆盖用例，均不预支为达标**：

- F14（`restart-persistence.spec.ts`）：真进程重启后 `GET /api/agent-sessions` 仍返回同一 session、`sessions.json`/`agents.json` 落盘可读、`usage.sqlite` 非空；只覆盖其 4 项必须断言中的 1 项。
- F13（`specs/f13-first-run-e2e.py`，`F13_L3: PARTIAL_PASS`）：真实例首启完整链路（检查→连接插件源→下载→安装→就绪）、`registry.json` + 25 个非空 payload 文件 + sha256、UI 摘要版本与磁盘一致、三项权限对话框、主页可用、退出并重启后不再弹首启；但**产品实际只安装 1 个 required 组件**，PLAN.md 要求 3 个（另 2 个 optional 且 `/settings/dependencies` 在 Aster 租户隐藏）。

9 条 L2 功能虽有通过场景，但尚未逐条核定「每条至少一例通过且执行面达标」，不预支为达标；**完全无任何用例的 L3 功能剩 4 条**（F06/F09/F12/F15）。

**结构性上限**：即便补齐所有 L2 映射、契约门禁全绿，按现有 L3 用例最多 **11/17 ≈ 64.7%**。剩下 4 条需新写产品级真宿主测试，另有 F13/F14 需补完必须断言（补完可到 13/17），都属于开发工作量而非执行次数。

### 授权后修复带来的变化（Gate A，同 3 个 config）

| | 修复前 | 修复后（`--stub-runtime`） |
| --- | ---: | ---: |
| 通过 | 29 | **41** |
| 失败 | 43 | **31** |
| task-conversation 通过 | 0 | **13** |

这一阶段同 3 个 config 的 Gate A 约 8.4%；**后续第三轮 F07 已通过、第四轮扩展到 6/7 config 后官方门禁为 36.9%**，见上表与 `OUTCOME.md` 的第三、四轮章节。本表保留阶段性数字以说明演进，最终数字以上表为准。

命令、hash、逐项失败归因、100% 所需清单与 `Explicitly not claimed` 见同目录 `OUTCOME.md`。
