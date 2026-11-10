# Aster 桌面版 0.2.2：签名公证 DMG 与发布

用户指令：「按 aster 的发布规范打包新的 dmg 签名安装包，确保用户安装不需要去设置页面授权」。

用户决策：`完整重构建最新源码`（含刚拉取的 studio PR #9 usage 性能改动）＋ `bump 版本并发布 Release`；工作树里已验证的功能改动 `一并提交并推送`。

## 1. 源码与提交

| 项 | 值 |
| --- | --- |
| 发布构建对应父仓提交 | `4e9f22cb634e43607d601995055edc6c19d421d0`（`release(desktop): bump to 0.2.2 and advance the studio gitlink`），tag `v0.2.2` 指向此提交 |
| studio gitlink | `42303bb224`（`release(studio): bump desktop to 0.2.2`） |
| 已推送的功能提交 | 父仓 `b542eaffa`（178 文件，TUI 控制 / reasoning 发现 / 验证器修复 / README 资产 / 11-10 证据）；studio `8475affe9e`（ACP driver unattended 支持） |
| 发布后网站提交 | `aa076c8d9` |

远程 `main` 在推送期间收到并发提交 `a79b3a575`，用 `git rebase origin/main` 合入后推送，无冲突、无覆盖。`just studio-land-check` → **EXIT 0**（worktree clean / pointer advances / delivery commits clean）。`python3 -m unittest discover -s tests -p release_governance.py -v` → **10/10 OK**。

## 2. 构建命令与实际结果

本机会话级限制（SwiftPM 嵌套 `sandbox-exec`、批量删除保护、Node 22 与 `engines.node >=24.12`）沿用 `docs/release-governance.md` 允许的 `/tmp` fixture：编排脚本 `/tmp/aster-022-front.sh`、`/tmp/aster-022-back.sh` 只复刻 `package-host.sh` 的两半并直接调用 node/cargo/tauri，`/tmp/aster-swift-bin/swift` 仅给 `swift build` 注入 `--disable-sandbox`；renderer 产物目录与 `.app` 的三个资源目录先 `mv` 备份而不是删除。**产品源码与构建脚本未改动**。

| 命令 | 实际结果 |
| --- | --- |
| `bash /tmp/aster-022-front.sh` | `FRONT_EXIT=0`，14m29s；`FRONT: renderer built` / `companions built` / `tauri app built` / `FRONT_OK`；renderer bundle `index.ee1f4a60c6.js`，`.app` `CFBundleShortVersionString=0.2.2` |
| `bash /tmp/aster-022-back.sh` | `BACK_EXIT=0`，3m48s；`codesign --verify --deep --strict` OK；`verified fresh anonymous runtime plugin installation`；notarytool `Accepted`；`stapler` OK；`spctl --assess --type execute` = `Notarized Developer ID`；`desktop archive verified` |
| `ASTER_NOTARY_PROFILE=aster-notary bash apps/studio/scripts/package-dmg.sh` | `DMG_EXIT=0`，1m30s；notarytool `{"status":"Accepted"}`；`stapler staple/validate` OK；`spctl --assess --type open` = `Notarized Developer ID`；`notarized=yes` |
| `shasum -a 256` 重命名 | `dist/aster_0.2.2_aarch64.dmg` + 同名 `.sha256` |

公证提交：`.app` `2d9bc803-7051-425c-ab14-98fc0c3cbaaa`、DMG `07bb3c5a-415b-4a01-b41e-d644ad5d11af`，均 `Accepted`。签名身份 `Developer ID Application: Lijin Zhu (5L9P7TS6DM)`，Team `5L9P7TS6DM`，hardened runtime + secure timestamp，DMG 与内部 `.app` 均 `Notarization Ticket=stapled`。

## 3. 「安装不需要去设置页面授权」验收

| 检查 | 结果 |
| --- | --- |
| `codesign --verify --strict --verbose=2` / `-dv` DMG | OK；`Notarization Ticket=stapled` |
| `xcrun stapler validate` DMG | `The validate action worked!` |
| `spctl --assess --type open --context context:primary-signature` DMG | `accepted` / `source=Notarized Developer ID` |
| 模拟下载：`xattr -w com.apple.quarantine "0083;…;Safari;…"` 后同一 `spctl` | `accepted` / `Notarized Developer ID` |
| 挂载 DMG 后内部 `.app`：`codesign -dv` / `stapler validate` / `spctl --assess --type execute` | ticket stapled / validate OK / `accepted` |
| 交付清单 `studio-delivery.json` 逐项 sha256+size | `delivery entries=100 problems=0`，`version=0.2.2 tenant=aster` |
| 隔离 HOME 首次启动（挂载卷内主二进制，12s） | `RUNNING_OK: stayed alive for 12s with an isolated HOME` |

结论：DMG 与内部 app 均为已公证并装订票据的 Developer ID 构建，带 quarantine 属性时 Gatekeeper 直接放行，不需要「系统设置 → 隐私与安全性 → 仍要打开」。同时复验了 0.2.1 事故的两个坑（staging 后清单重算、首次启动不闪退）。

## 4. 发布结果

| 项 | 值 |
| --- | --- |
| Release | `v0.2.2`「Aster v0.2.2 · macOS Apple Silicon」，非 draft / 非 prerelease，**仓库 Latest** |
| 资产 | `aster_0.2.2_aarch64.dmg`（138M）、`.sha256`、`latest.json`（CLI 0.3.3 兼容清单，取自 `aster-cli-stable`） |
| DMG SHA-256 | `65cd294986d402c828db77c54541476c199acef7baef692345795ccfe76ff6b0` |
| desktop-stable | `Aster-darwin-aarch64-v0.2.2.zip`（140,273,750 B，SHA `8d9c9ccedc99560df500941c65a8cbd251a9535ab0835576ffff1b9797c315f0`）+ `desktop-latest.json`（version 0.2.2，history 10，`generate`/`check` 均通过） |
| 网站 | `node apps/web/scripts/build_releases.mjs --refresh` / `--check` EXIT 0，desktop 0.2.1 → 0.2.2；`PATH=/opt/homebrew/bin:$PATH npm run build` → 223 页；`node --test tests/releases.test.mjs tests/download.test.cjs tests/home.test.cjs` → **37/37** |
| 线上复核 | 重新下载线上 DMG 与本地 `cmp` 字节一致、SHA 一致、sidecar 一致；线上 `desktop-latest.json` `check` = version 0.2.2；Latest 上 `latest.json` = CLI 0.3.3；远程 `refs/tags/v0.2.2` = `4e9f22cb…` |

## 5. 已知偏差与并发情况（不假绿）

- **构建期间工作树被并发写入**：`crates/code/acp/src/agent.rs`、`agent/turn.rs`、`agent/turn_terminal_tests.rs`、`agent_tests.rs`、`turn_cancel.rs` 在 17:44:05 被另一并发工作修改，未提交、未纳入本次提交，也未擅自提交/丢弃。`.app` 内 companion 于 17:43:29 stage，早于该修改；Studio 主二进制由干净的 studio 源码树构建。`package-dmg.sh` 因此打印 `dirty worktree — local artifact only` 警告。本次发布的可复现基准是 tag `v0.2.2` / 上述两个提交。
- **形态为 slim base release**（`ASTER_DEFER_RUNTIME_COMPONENTS=1`）：与 CI `release-aster-desktop.yml` 默认 `defer_runtime_components=false`（full bootstrap）不同；本机缺 `ASTER_OCR_MODELS` / `ASTER_OCR_NOTICES` 与 OCR 模型目录，无法产出 full bootstrap。已用官方 registry 做真实匿名安装复验；全新机器首次启动会从官方 registry 安装固定组件。
- **平台**：arm64 only，不支持 Intel Mac / macOS 11 以下。
- **保留窗口未清理**：`v0.2.0`、`v0.2.1` 的版本化 Release/tag 仍在（规范要求只留当前与上一版）；删除属破坏性外部动作，本次未获授权，未执行。
- 本机 Node 22.22.2 不满足 studio `engines.node >=24.12`；未混用 Node 26 构建 renderer（沿用上次已验证的 Node 22 renderer 链路），网站构建使用 Homebrew Node 26.8.1。

## 6. Explicitly not claimed

- 未运行 Studio / desktop GUI 真窗口端到端（agent-desktop / CUA / headed scenario）；只做了挂载卷内主二进制的隔离 HOME 启动 smoke 与签名/Gatekeeper 检查。
- 未声称该构建等价于 CI `macos-14` full-bootstrap 产物；未声称支持 Intel Mac。
- 未替换或修改 `v0.2.1` 的任何线上资产；未创建字面 `latest` tag；未改动 CLI/Core 更新通道的版本化归档。
- 未修复产品代码中的任何问题；本次仅按现有发布链路出包、签名、公证与发布。
- 未清理并发产生的 5 个未提交文件，也未把它们编入发布记录。

## 7. 日志与辅助文件

`/tmp/aster-022-front.log`、`/tmp/aster-022-back.log`、`/tmp/aster-022-package-dmg.log`、`/tmp/aster-022-web-build.log`；编排与验证脚本 `/tmp/aster-022-front.sh`、`/tmp/aster-022-back.sh`、`/tmp/aster-022-verify-delivery.py`、`/tmp/aster-022-smoke.py`；备份 `/tmp/aster-022-renderer-backup-20261110-173441`、`/tmp/aster-022-app-resources-20261110-175034`。全部留在 `/tmp`，未进入产品源码或二进制。
