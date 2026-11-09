# 使用 agent-desktop 做真窗口端到端测试（含阻塞说明）

## 目标

「继续提高测试覆盖率，使用 agent-desktop 完成端到端测试」。按 `AGENTS.md`：真窗口 E2E 必须先从**本 checkout** 构建 native 主机与 companion，且不得复用过期窗口；GUI 审查用 headless semantic 操作，不抢焦点。

## 1. 已完成的前置（此前一直被阻塞，本次打通）

| 步骤 | 命令 / 结果 |
| --- | --- |
| fresh native build | `cd /Users/roylin/code/aster && just build-studio` → **EXIT=0**，`Finished dev profile in 1m 18s`。此前多轮被 `screencapturekit` SwiftPM `sandbox_apply: Operation not permitted` 挡住，本次未复现 |
| 启动窗口 | `ASTER_TENANT_SLUG=aster just studio`（后台）→ `agent-desktop list-windows` 出现 `Aster` 窗口 `w-38965`（1440×920），renderer dev server 在 4318（200） |
| 重启尝试 | 关掉后用 `ASTER_DEFER_RUNTIME_COMPONENTS=1 just studio` 重启得到 `w-39015`，用于排除 defer 开关影响 |
| agent-desktop 能力 | `snapshot -i --compact`、`find --role button --name ... --exact`、`click <ref>` 均 **ok**；`click` 机制为 `semantic_api` / `AXPress`，默认 headless，不 focus、不动光标、不用 `--headed` |

一个完整的 observe → act → verify 循环已在本 checkout 的真窗口上跑通：

```
agent-desktop find --app Aster --window-id w-39015 --role button --name '重试安装' --exact
  → ref @ssbek7ghnq1dc:e1
agent-desktop click @ssbek7ghnq1dc:e1
  → ok=true, mechanism=semantic_api, outcome=succeeded
```

## 2. 阻塞：首次运行组件门禁挡住全部页面

窗口 AX 树里只有一个模态：

```
dialog 正在准备组件
  button 重试安装
button Close / Full Screen / Minimize   (窗口按钮)
```

侧栏导航按钮（`工作` / `设置` / `技能` / `连接器`）在 `find` 下全部 **0 matches**，因此 `ax-page-audit.mjs` 的逐页导航与 `tests/e2e/agent-desktop/*.py` 场景都无从执行。

根因（native 日志）：

```
Failed to read runtime components ... FirstRunRuntimeComponentsDialog
  IpcError: Runtime plugin pin identity mismatch
Failed to install required runtime components ... 同一错误
```

来自 `apps/studio/src-tauri/src/runtime_components.rs`：
- `initialise_pin()` 用 `resource_root().join(PIN_NAME)` 读 `runtime-components.json`，`load_pin(path, env!("CARGO_PKG_VERSION"))`；
- `pin.schema != "aster.studio.runtime-components.v1" || pin.version != version || pin.components.len() != COMPONENTS.len()` 时返回 `Runtime plugin pin identity mismatch`；
- `pin_error` 会让 `runtime.components.list` / `required_missing` 等 IPC 直接报错，`FirstRunRuntimeComponentsDialog` 的 `canDismiss = requiredPrepared` 为 false，连关闭按钮都不渲染。

已排除的绕过方式：
- `agent-desktop press --app Aster --window-id ... escape`：工具沙箱下 `ps: Operation not permitted` → `application_inventory_unstable`；
- `agent-desktop press escape`（不带 app）：成功投递（`CGEventPost`），但 Radix 模态在 `canDismiss=false` 时不响应 Escape；
- headless 点击「重试安装」：pin mismatch 是确定性的，重试无效；
- `ASTER_DEFER_RUNTIME_COMPONENTS=1` 重启：该变量只影响 Python 打包 / `tauri-config` 的 resources 声明，Rust 运行时不读，日志仍报同一错误；
- `DOCS_RS=1` 跳过 Swift bridge：会产出缺符号的坏二进制，不采用。

## 3. 已落地的能力改进（可复用）

`apps/studio/scripts/ax-page-audit.mjs`：

- `PAGES` 去掉已下线的「设置/API网关」页，现为 14 页（工作 / 连接器 / 技能 / 自动化 + 10 个设置页，与 `tests/e2e/settings/support.ts` 的 `ASTER_SETTINGS_PAGES` 一致）；
- `audit()` 为每个组件产出 `{ page, role, name, states, selector, coveredBy: [] }`，`selector` 形如 `[data-ax-role="button"][data-ax-name="…"]`；
- report 顶层新增扁平 `components[]` 台账，供 `scripts/e2e-coverage-gate.mjs` 的 Gate B 直接消费；`results[]` 保留原汇总字段（不含逐组件明细，避免输出膨胀）；
- `pressEscape()` 改为 3s 超时并容错：沙箱下 `press --app` 失败不会让每页卡 25s。

`node --check scripts/ax-page-audit.mjs` 通过，`git diff --check` 通过。

## 4. 实际跑出的数字

| 检查 | 结果 |
| --- | --- |
| `node scripts/ax-page-audit.mjs --app Aster --window-id w-39015 --pages '工作' --out /tmp/aster-ax-inventory.json` | 产出 1 页 / 4 组件 / 0 issues；`components[]` 全部带 `coveredBy` 数组 ✓ |
| `node scripts/e2e-coverage-gate.mjs /tmp/aster-final-e2e.log /tmp/aster-ax-inventory.json --target=0.7` | **Gate B 从 `UNMEASURABLE` 变为 `0.0% — FAIL`（measurable=true, covered=0, total=4, zeroCoverage=4）** |
| 对照（无 inventory） | `Gate B: UNMEASURABLE — no inventory supplied` |
| Gate A（同一命令） | `28.8% (139/483) — FAIL`，与上一轮一致，**没有新增通过用例** |

Inventory 只有模态与窗口按钮（4 个），不是真实 14 页组件，因此 0% 只代表口径打通，**不代表产品覆盖率**。

## 5. Explicitly not claimed

- **未宣称完成 14 页逐页面审查**：被 first-run 模态阻挡，未产出真实组件台账，未生成 `coveredBy` 命中。
- **未宣称 Gate A / Gate B 覆盖率数字提高**：Gate A 仍 139/483 = 28.8%；Gate B 仅从不可测变为可测且为 0%。
- **未宣称 `tests/e2e/agent-desktop/*.py` 场景通过**：这些脚本要求「already-running, isolated Aster window」且能访问 composer「描述任务」，当前窗口不满足，未执行。
- **未宣称修复 runtime pin**：这是 native 侧产品状态（`runtime_components.rs` 的 pin 版本/组件集合校验），没有改产品代码、没有伪造或删除 pin 文件来骗过门禁。
- **未宣称 `press --app` 可用**：受工具沙箱 `ps` 权限限制；所有已记录的交互都是 headless semantic `click <ref>` / `snapshot` / `find`。
- **未宣称主功能 E2E 重跑**：沿用上一轮 `/tmp/aster-final-e2e.log`（139 passed / 0 failed / 55 skipped），本轮未重跑 renderer 套件。

## 6. 环境恢复

测试后已停掉 `just studio`（窗口与 dev server 一并关闭），`agent-desktop list-windows` 中无 Aster 窗口；随后用 `STUDIO_RENDERER_DIST=/tmp/aster-e2e-dist STUDIO_RENDERER_PORT=4318 node scripts/e2e-preview.mjs` 恢复静态预览（200）。改动文件仅 `apps/studio/scripts/ax-page-audit.mjs`。

## 7. 下一步（解除阻塞后即可提高覆盖率）

1. 修 `Runtime plugin pin identity mismatch`：让 `resource_root()/runtime-components.json` 的 `schema` / `version`（须等于 `aster-studio` 的 `CARGO_PKG_VERSION`）/ `components` 集合与 `COMPONENTS` 一致，或用 `scripts/package_runtime_components.py` / `studio_delivery.py` 重新生成 pin；这是产品/打包侧改动，需要单独授权。
2. 窗口能进入主界面后：`node scripts/ax-page-audit.mjs --app Aster --window-id <id> --out <inventory.json> --strict` 产出 14 页真实台账。
3. 用通过的 E2E spec 的可访问名 / `data-ui` 选择器回填 `components[].coveredBy`，再由 `e2e-coverage-gate.mjs` 给出 Gate B 真实百分比。
4. 并行推进 `tests/e2e/COVERAGE_UPLIFT_PLAN.md` 的 S1（其余 config 纳管），把 483 分母下 289 个未执行用例纳入 Gate A。
