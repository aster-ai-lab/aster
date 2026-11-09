# agent-desktop 逐页面逐组件可视化 E2E（不抢焦点）

## 目标与硬约束

用 agent-desktop 在 Aster Studio 真窗口上完成逐个页面、逐个组件的可视化端到端审查，**不抢鼠标、不抢窗口焦点**。全程只用 headless semantic：`list-windows` / `find` / `click <ref>`（AXPress）/ `snapshot` / `screenshot`。**未使用** `focus-window`、`--headed`、`mouse-*`、`--xy`、`press --headed` 或任何物理按键。

## 1. 构建与启动（符合 AGENTS.md 的 fresh-build 要求）

| 步骤 | 命令 / 结果 |
| --- | --- |
| native 主机 | `just build-studio` → **EXIT=0**，`Finished dev profile in 11.66s`（重跑 7.12s / 7.37s） |
| renderer | `ASTER_TENANT_SLUG=aster STUDIO_RENDERER_DIST=/tmp/aster-e2e-dist pnpm exec rsbuild build` → **EXIT=0**，23.1s |
| 预览 | `STUDIO_RENDERER_DIST=/tmp/aster-e2e-dist STUDIO_RENDERER_PORT=4318 node scripts/e2e-preview.mjs` → 200 |
| 窗口 | 手动 spawn fresh 二进制 `target/studio/aster/debug/studio-launcher/aster/Studio`，带 `ASTER_TENANT_SLUG=aster` / `STUDIO_RENDERER_PORT=4318` / `STUDIO_COMPANIONS_STAGED=1` / `CARGO_TARGET_DIR=target/studio/aster` / `TAURI_CONFIG`（由 `studio_product.py aster tauri-config` 生成）→ 窗口 `w-41685`（1440×920 @ 154,30） |

`just studio` 本身**不可用**：它内部的 cargo feature 组合报 `E0599: no method named handle_memory_flush / handle_memory_dream found for &agent::AsterCodeAgent`（`aster-acp` lib），而 `just build-studio` 的另一套 feature 组合能编译。因此按 `AGENTS.md` 允许的「explicit `STUDIO_RENDERER_DIST` rsbuild build/preview + fresh native binary」路线手动启动，未复用旧窗口、未用其它 worktree 的 renderer。

## 2. 解除首次运行门禁（测试环境隔离，不改产品源码）

上一轮已确认 `Runtime plugin pin identity mismatch` 会让 `FirstRunRuntimeComponentsDialog` 独占窗口、侧栏 0 matches。本轮排查：

- `COMPONENTS`（Rust `runtime_components.rs` 与 Python `package_runtime_components.py`）现在是 `aster-document` / `aster-office` / `aster-ocr`；已安装 app 的 pin 仍是旧的 `aster-sandbox`。
- 备份并修正 `/Applications/.a3s-update/Aster.app/unpacked/Aster.app/Contents/Resources/runtime-components.json`，并把正确 pin 放到 4 个可能的 `resource_dir()` 候选路径 —— **dev 窗口仍报同一错误**，说明它读的是另一处资源。继续追查代价过高。
- **最终方案**：在 `/tmp` 测试产物的 `windows/main/index.html` 注入一段最小 IPC 桩，只拦截 `runtime.components.list` 与 `runtime.components.required_missing`，返回 `{ok:true,data:[]}`；**其余所有 IPC 原样透传**。桩通过 `setInterval` 等 `window.api.ipcApi.request` 出现后包装，不修改任何仓库源码。效果：`pin identity mismatch` 日志 **0 次**，dialog 消失，侧栏「工作」「设置」及 69+ AX 引用可见。

这是测试 fixture（与 `installWorkbench` / `STUDIO_E2E_STUB_RUNTIME` 同类），**不是绕过被测功能**：runtime 组件安装本身不在本次页面×组件审查范围内，见 §6。

## 3. 审查方法

一次性脚本 `/tmp/aster-visual-audit.py`（不入库），逐页执行：

```
find --app Aster --window-id <id> --role button --name <step> --exact --first
  → data.match.ref_id → click <ref>          # 导航，headless AXPress
snapshot --app Aster --window-id <id> -i --compact --timeout-ms 15000
  → flatten 树，提取 role/name/states/ref
find --app Aster --window-id <id> --role heading --limit 3   # R3 判定，compact 树不含静态 heading
screenshot <full.png> --screen 0 → sips -c 920 1440 --cropOffset 30 154 → 窗口截图
```

- `screenshot --app/--window-id` 需要应用 inventory（调 `ps`），在工具沙箱下报 `application_inventory_unstable / ps: Operation not permitted`；改用 `--screen 0`（同样被动、不改焦点）再裁剪到窗口 bounds。
- 组件判据：`heading` 或 `INTERACTIVE` 集合（button / link / tab / checkbox / switch / combobox / textfield / disclosure / progressbar / slider / radiobutton / searchfield）。
- 规则：`R1` 无名控件、`R3` 无 heading、`R4` 同名控件 ≥4、`R5` offscreen >12、`R6` 多 dialog；沿用 `ax-page-audit.mjs` 的定义。

## 4. 结果（最终一轮）

| 指标 | 值 |
| --- | --- |
| 审查页面 | **14 / 14 全部导航成功**（工作、连接器、技能、自动化 + 10 个设置页；已去掉下线的「设置/API网关」） |
| 可交互组件 | **1424** |
| 零覆盖组件 | **0** |
| 截图 | **14 张**（`pages/01…14-*.png`） |
| issues | **3**，全部 `R5`：设置/快捷键 20、设置/用量 272、设置/模型服务 46 个 offscreen 交互件（长列表/虚拟化，按仓库规则只报告不判缺陷） |
| `R1` / `R3` / `R4` / `R6` | **全部 0**（14 页 `headingCount=3`） |

| 页面 | 组件 | refs | heading | issues |
| --- | ---: | ---: | ---: | ---: |
| 工作 | 35 | 36 | 3 | 0 |
| 连接器 | 62 | 81 | 3 | 0 |
| 技能 | 66 | 92 | 3 | 0 |
| 自动化 | 35 | 37 | 3 | 0 |
| 设置/通用 | 57 | 68 | 3 | 0 |
| 设置/快捷键 | 103 | 106 | 3 | 1 (R5) |
| 设置/用量 | 541 | 1316 | 3 | 1 (R5) |
| 设置/模型服务 | 137 | 149 | 3 | 1 (R5) |
| 设置/模型配置 | 53 | 56 | 3 | 0 |
| 设置/网络搜索 | 91 | 94 | 3 | 0 |
| 设置/文档处理 | 59 | 61 | 3 | 0 |
| 设置/数据 | 70 | 74 | 3 | 0 |
| 设置/归档 | 64 | 81 | 3 | 0 |
| 设置/关于 | 51 | 53 | 3 | 0 |

角色分布：`button 791`、`switch 463`、`combobox 130`、`radiobutton 21`、`textfield 18`、`link 1`。逐组件明细（page / role / name / states / selector / ref / coveredBy）在 `inventory.json` 的 `components[]`。

**截图校验**：目视检查 `06-设置-快捷键.png` —— 左侧导航高亮「快捷键」、主标题「快捷键」、开关与快捷键绑定控件齐全，裁剪无错位、无其它应用窗口干扰。

## 5. Gate B 口径（避免「假绿」）

- **AX 可观察覆盖 = 1424/1424 = 100%**：每个组件都有可访问名，被本次 `snapshot` 以 `[data-ax-role][data-ax-name]` 稳定选择器命中，并验证 role / states。`coveredBy` 记为 `ax-page-audit:<page>`。
- **交互命中覆盖未做**：本轮只对导航按钮执行了 `click`，**没有**对每个组件做点击/输入/状态切换断言。因此这个 100% **不代表功能覆盖率 100%**，也不代表 Gate B 的原定义（「被至少一条通过用例用稳定选择器命中」）已由 E2E spec 满足。
- `e2e-coverage-gate.mjs` 的脚本复核本轮未跑：它需要一个 Playwright list 日志，而之前几轮的 `/tmp/aster-*-e2e.log` 已被系统清理。上面的数字直接来自 `inventory.json`。
- 三次连续运行的组件数为 1450 / 1501 / **1424**：消息虚拟列表与设置页长列表随视口/滚动状态变化，属真实产品行为，不是断言放宽。以最终一轮（含截图）为准。

## 6. Explicitly not claimed

- **未宣称 runtime 组件安装/首启流程通过**：用测试 HTML 桩跳过了它，未修 `Runtime plugin pin identity mismatch` 的产品根因；该 pin 问题仍需产品/打包侧单独处理（见 `docs/evidence/studio/2026-11-09-agent-desktop-e2e/OUTCOME.md`）。
- **未宣称逐个组件做了交互验证**：只做了 AX 可观察性审查（名字、role、states）+ 页面导航点击；逐组件交互是 `TASK_CONVERSATION_E2E_PLAN.md` / `PAGE_COMPONENT_E2E_PLAN.md` 的 L1 后续工作。
- **未宣称 `just studio` 可用**：其 cargo feature 组合有编译错误；本轮走的是手动 fresh 启动路线。
- **未宣称 95% 目标或 Gate A 数字变化**：Gate A 仍按 `COVERAGE_UPLIFT_PLAN.md` 的 483 口径，本轮未重跑 Playwright 套件。
- **未宣称跨引擎 / 深浅色 / 窄屏 / reduce-motion 覆盖**：本轮是单一 chromium+Tauri 窗口、默认浅色、1440×920、无 reduce-motion。
- **未宣称修改了产品代码或提交**：仓库源码零改动；证据、台账、截图为本轮新增文件，未提交。

## 7. 环境恢复

- 停掉本轮启动的 Aster 窗口（`w-41685`）。
- 恢复 `/Applications/.a3s-update/Aster.app/unpacked/Aster.app/Contents/Resources/runtime-components.json` 到 `/tmp/aster-runtime-pin.backup.json` 的原始内容；删除 4 个候选目录里临时放置的 pin 文件。
- 恢复 `/tmp/aster-e2e-dist/windows/main/index.html`（去掉测试桩），保留静态预览 4318 供后续使用。
- 未提交、未 push。

## 8. 后续建议

1. 修 `runtime_components` 的 pin 根因，让真窗口无需测试桩即可进入主界面。
2. 把 `inventory.json` 的 `components[].coveredBy` 改为由**通过的 L1 E2E spec** 回填，得到符合 Gate B 原定义的真实百分比。
3. 对 `R5` 的 3 个页面（快捷键 / 用量 / 模型服务）补滚动与虚拟化断言，确认 offscreen 节点不是未渲染缺陷。
4. 按 `TASK_CONVERSATION_E2E_PLAN.md` 的 B5 把主题 / 390px / reduce-motion / 键盘 / en-US 维度套到这份 14 页台账上。
