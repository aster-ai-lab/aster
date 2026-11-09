# 集成 aster-acp 9.2.1 / aster-cli 0.3.1 并重测

## 目标与结论

在 Aster Studio 集成 Core/ACP 9.2.1 与 CLI 0.3.1，重新测试功能。**版本链路已全部对齐，native 与 renderer 均可从当前 checkout 构建，主功能/设置页 E2E 全绿，且与上一版基线逐项一致——未引入新回归。** 未验证与既有失败见 §5。

发布证据（外部提交）：`1a34fc4a5 docs(cli): record aster-cli 0.3.1 and Core/ACP 9.2.1 publish evidence`、`docs/evidence/code/2026-11-08-aster-cli-0.3.1/OUTCOME.md`，tag `aster-acp-v9.2.1` / `aster-cli-v0.3.1`。

## 1. 集成前状态

| 位置 | 集成前 | 集成后 |
| --- | --- | --- |
| `crates/code/acp/Cargo.toml` / `crates/code/core/Cargo.toml` | 9.2.1 ✓ | 9.2.1 |
| `apps/aster/Cargo.toml`（CLI） | 0.3.1 ✓ | 0.3.1 |
| `Cargo.lock` `aster-acp` / `aster` | 9.2.1 / 0.3.1 ✓ | 9.2.1 / 0.3.1 |
| `target/studio/aster/debug/aster-acp` | 9.2.1 ✓ | 9.2.1 |
| `target/studio/aster/debug/studio-resources/agent/aster-acp` | 9.2.1 ✓ | 9.2.1 |
| `apps/studio/src-tauri/resources/agent/aster-acp`（非 staged 回退 companion，未被 git 跟踪） | **9.2.0** | **9.2.1** |
| 中英 locale `library.config.agent.field.runtime.option_description.aster` | **aster 9.2.0** | **aster 9.2.1** |
| `target/debug/aster --version` | 0.3.1 ✓ | `aster 0.3.1 (1a34fc4a5be3)` |
| `target/release-dist/aster` / `aster-acp`（上一版发布草稿） | 0.3.0 / 9.2.0 | 未改动，见 §2 |

## 2. 本轮改动

| 动作 | 细节 |
| --- | --- |
| 同步 companion | `chflags nouchg` → `cp target/studio/aster/debug/aster-acp apps/studio/src-tauri/resources/agent/aster-acp` → `chmod 755`；`--version` 从 9.2.0 变为 9.2.1。该文件由 `build.rs` 的 `copy_companion` 在非 staged 构建时填充；staged 构建读 `target/.../studio-resources/agent/` |
| 版本文案 | 两个 locale 的 `9.2.0` → `9.2.1`，`pnpm i18n:check` 通过（25294 条） |
| 未改 | `>=0.3.0, <0.4.0` 是 ACL 的 `requires_use` 版本范围（use-registry），0.3.1 落在范围内；`feature.model_routing.*` preference schema / `modelRouting.ts` / Rust `local_model_budget_status` 仍被 Code CLI、设备连接、apiGateway 引用，保持原样；`target/release-dist/` 是上一版 0.3.0 + ACP 9.2.0 的本地发布草稿，Studio 不打包它，覆盖会混淆历史发布证据，因此不动 |

CLI 0.3.1 是独立发布物，Studio 通过 workspace 的 Core/ACP 9.2.1 与之对齐；`just build-studio` 重新编译后的 `target/debug/aster` 已是 0.3.1。

## 3. 验证命令与结果

| 检查 | 命令 | 结果 |
| --- | --- | --- |
| native 编译 | `cd /Users/roylin/code/aster && just build-studio` | **EXIT=0**，`Finished dev profile in 11.07s`（9.2.1 / 0.3.1 无 API 破坏；`build.rs` 的 staged companion 一致性 assert 通过） |
| renderer 构建 | `ASTER_TENANT_SLUG=aster STUDIO_RENDERER_DIST=/tmp/aster-e2e-dist pnpm exec rsbuild build` | **EXIT=0**，17.1s |
| i18n | `pnpm i18n:check` | 通过，25294 条 |
| 布局契约 | `node scripts/run-perf-layout-contracts.mjs` | **EXIT=0，185 用例** |
| 关键 renderer 单测 | `vitest run --project renderer`（ModelSettings / focus / WorkbenchScopePicker / workspaceProductAvailability / typeIconContract） | **5 文件 / 49 用例全过** |
| 构建脚本单测 | `python3 -m unittest discover -s tests -p studio_product_test.py` | **14/14** |
| **主功能 E2E** | `pnpm playwright test --config playwright.workbench-office.config.ts --output=/tmp/aster-921-e2e-results --reporter=list` | **`E2E_EXIT=0`，139 passed / 0 failed / 55 skipped** |
| 设置页 E2E | `pnpm playwright test --config playwright.settings.config.ts --reporter=list` | **18 passed / 0 failed / 2 skipped** |
| 其余短 config 回归 | `task-turn-state` / `workspace-motion` / `knowledge` | 1/5、5/8、6/16，与上一版基线**逐项一致**（见 §4） |
| scripts 全套单测 | `CODEBUDDY_SAFE_DELETE_BULK_THRESHOLD=1000000 pnpm exec vitest run --project scripts` | 481 passed / 34 failed（515）；失败文件集合与上一版基线**完全相同** |
| 门禁 | `node scripts/e2e-coverage-gate.mjs /tmp/aster-921-e2e.log --target=0.7` | Gate A **28.8%（139/483）FAIL**、`unexecuted=289`；已执行口径 139/139 = 100%；Gate B `UNMEASURABLE` |
| `git diff --check` | 根目录 + `apps/studio` | 通过 |

预览：`STUDIO_RENDERER_DIST=/tmp/aster-e2e-dist STUDIO_RENDERER_PORT=4318 node scripts/e2e-preview.mjs`（**不启用** `STUDIO_E2E_STUB_RUNTIME`，该桩会覆盖 fixture IPC 使 writing 两例失败），E2E 产物全部写 `/tmp/aster-921-*`。

## 4. 既有失败（与 9.2.1 / 0.3.1 集成无因果关系）

- **非默认 config**：`task-turn-state` 1 passed / 4 failed、`workspace-motion` 5 passed / 3 failed（3 个失败全是 webkit 动效，chromium 5 个全过）、`knowledge` 6 passed / 5 failed / 5 skipped。根因是既有 spec 漂移（发送按钮 `aria-disabled`、`.task-reasoning` 边框期望、已下线的「插件」入口）与跨引擎差异，上一版基线数字完全相同。未修、未放宽断言、未改分母。
- **scripts 单测 34 failed**：`electron-builder` 依赖缺失、`packages/ui` 源文件缺失（UI contract 扫描）、Cherry→Aster 品牌重命名后未更新的 fixture、被外部提交移走的 `.github/workflows/*.yml`、CI release baseline、`/tmp/cherry-phase-*/app.json` fixture 缺失等。失败文件集合与上一版一致（14 个），总失败数 34 vs 33 为参数化用例计数波动。
- **版本链路残留**：`target/release-dist/`（上一版 0.3.0 + ACP 9.2.0 发布草稿）未同步，Studio 构建不读它。

## 5. Explicitly not claimed

- **未宣称 native 真窗口端到端通过**：上一轮已确认真窗口会被 `FirstRunRuntimeComponentsDialog` 的 `Runtime plugin pin identity mismatch` 模态遮挡，侧栏不可达；本轮只做了 `just build-studio` 编译验证，未启动窗口、未跑 `agent-desktop` 逐页面审查。修复 pin 是产品/打包侧改动，仍需单独授权（见 `docs/evidence/studio/2026-11-09-agent-desktop-e2e/OUTCOME.md`）。
- **未宣称 Gate A 覆盖率提高**：主口径 483 下仍 139 通过（28.8%），289 个未执行用例未纳管。
- **未宣称 Gate B 可测**：本轮未生成 agent-desktop inventory，仍 `UNMEASURABLE`。
- **未宣称 7 个 config 全部通过** / **未宣称 scripts 全套单测通过** / **未宣称 renderer 1698 个单测文件全套执行**：按功能级验证选关键子集 + 完整 E2E 矩阵。
- **未宣称 CLI 二进制被打包进 Studio**：CLI 0.3.1 是独立发布物。
- **未宣称修改 Cargo.lock / crate 版本**：本轮开始时它们已是 9.2.1 / 0.3.1，改动只涉及 companion 同步与 locale 文案。

## 6. 状态

工作树：根目录有 58 项改动，主要为 `apps/use-registry/` 的 untracked 资产（此前为修复 rsbuild 资产缺失从历史提交恢复）与 `apps/studio` submodule 内的 `scripts/ax-page-audit.mjs`（上一轮 Gate B 台账改造）；本轮新增改动仅为两个 locale 与未跟踪的 companion 二进制。静态预览 4318 仍在运行。未提交、未 push。
