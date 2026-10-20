# AsterDream v0.1.2：macOS 更新、目标状态及续跑修复

macOS arm64 候选包已完成 Developer ID 签名、Apple 公证及原生升级验收。使用 v0.1.0 原安装包的隔离副本，从签名下载、整包替换到公证票据和 Gatekeeper 检查均通过。没有发布 GitHub release，也没有替换线上更新清单。

## 已证实的更新失败原因

公开清单见 [live-feed-before.json](live-feed-before.json)，release 资产见 [releases-before.json](releases-before.json)。

| 环节 | 实际情况 | 影响 |
| --- | --- | --- |
| 清单 | 宣告 v0.1.1，macOS 地址指向 `aster-v0.1.1/AsterDream.app.tar.gz` | 版本比较可以成功，用户能看到新版本 |
| 公开下载 | 该地址实际返回 HTTP 404 | 下载阶段失败，无法进入验签和安装 |
| 版本资产 | 实际发布标签为 `v0.1.1`，没有 `aster-v0.1.1`；资产只有 DMG、校验和及其他平台安装包，没有所指的 `.app.tar.gz` 更新包 | 只改版本号或重试客户端不能修复缺失资产 |
| 本地打包 | 缺少 `bundle.createUpdaterArtifacts`；打包脚本仅确认 DMG，未强制检查签名更新包 | 发布流程不能保证清单所指更新包已经生成和上传 |
| 错误显示 | 下载、验签、安装错误合并为笼统失败，安装命令失败还会返回成功 | 用户无法判断失败阶段，重试状态可能不正确 |

旧客户端只接受本仓库的 `aster-v<版本>` 下载路径。直接改成 `v0.1.1`、把 DMG 当作更新归档或更换更新公钥，都不能建立兼容的 v0.1.0 升级路径。新清单应在 `aster-v0.1.2` 的签名更新资产公开可下载后生成，再替换 `latest/latest.json`。已核对[原版 DMG 的校验和及旧二进制内嵌公钥](original-release.json)。格式依据 [Tauri 官方更新文档](https://v2.tauri.app/plugin/updater/)。

另修复了源码中的状态顺序问题：原生先保存可安装包，再发出下载完成事件；取出待安装包时原子进入 Busy，安装失败保留重试包。没有把这些次要源码缺陷当作已证实的用户报错根因。

## 本轮修改

- Work 配置、Cargo 包及前端版本统一为 v0.1.2，生成完整更新归档与 `.sig`；宿主、ACP、Office 一起打包。
- 打包缺少更新签名输入或最终更新资产时失败。新增 `prepare-update.py`，检查兼容标签、已上传资产的大小和 SHA-256、真实公开下载、完整 App 结构、版本及旧包升级。通过后才产生可发布的 `latest.json`，CI 增加对应边界回归。发布验收要求 macOS 宿主，并读取原生测试在全部断言通过后写出的版本凭据；退出码为零但测试被跳过，或凭据版本不符时均拒绝生成清单。
- 下载、签名及安装错误使用不同代码和本地化文案。新下载重置旧 ready 状态，安装失败恢复按钮以便重试。
- 中文宿主的目标系统状态改为简短中文；原始目标保留原文，输入区只显示紧凑目标按钮，全文点击展开、Escape 收起。没有翻译或改写历史用户内容。
- 自动续跑只将有已验证工作区 mutation 的已接受步骤计为进展。默认 8 个 slice、连续 2 个无进展 slice、ACL 优先级及完成验证标准保持不变。详见 [issue #1 修复报告](../../code/2026-10-20-goal-continuation-stall/OUTCOME.md)。

### 本机签名障碍的更正

最初根据工具的「Wrong password」提示判断缺少密码，这个判断不准确。现有文件带 `Sc` 标记，但私钥材料未加密、校验字段为零。由私钥导出的 Ed25519 公钥和挑战签名均与 v0.1.0 内置公钥匹配。

在内存中补全标准校验及空口令封装后，Tauri 2.11.5 接受该签名输入；最终更新归档也通过原生更新库的生产公钥验签。原密钥文件及产品公钥保持不变，没有猜测密码，没有把私钥写入证据目录。见 [安全元数据](signing-key-recovery.json)。这是准备新版本时发现的独立本机问题，不能据此断言原线上 v0.1.1 的签名有错；原线上归档不可下载，未完成其验签。

## 验证结果

| 检查 | 结果 | 证据 |
| --- | --- | --- |
| 完整 release 打包 | 通过；前端、ACP、Office、原生宿主及签名更新归档生成成功 | [release-build-final.log](release-build-final.log) |
| 原生更新状态、版本及 URL 边界 | 18 项通过 | [native-updater-tests-final.log](native-updater-tests-final.log) |
| 原生下载和安装集成 | 3 项通过，含真实旧包副本、生产公钥、最终归档、公证票据及 Gatekeeper | [production-upgrade-tests.log](production-upgrade-tests.log) |
| 清单发布边界 | 6 项通过；拒绝缺失标签、资产、错误 digest、不同公钥、下载字节或大小不符、非 macOS 宿主及未实际验收的版本 | [feed-boundary-tests-final.log](feed-boundary-tests-final.log) |
| 更新交互及紧凑目标展示 | 44 项通过 | [renderer-tests-final.log](renderer-tests-final.log) |
| 任务计划与输入区回归 | 264 项通过 | [planning-regression.log](planning-regression.log) |
| CUA 原生界面 | 看到中文目标状态；目标详情点击展开、Escape 收起；规划首页无额外描述行；v0.1.2 手动检查更新返回「你的软件已是最新版本」 | [CUA 记录和边界](CUA.md) |
| Apple 公证 | Accepted；服务日志无 issues | [服务日志](notary-service-log.json) |
| 发行文件 | App 与 DMG 的 stapler、代码签名、Gatekeeper 和 DMG 完整性检查通过 | [notary-verification.log](notary-verification.log) |
| DMG 实际内容 | 只读挂载，版本、三枚 arm64 二进制、UUID 与冻结 App 一致；内部 App 被 Gatekeeper 接受 | [mounted-dmg-verification.json](mounted-dmg-verification.json) |
| 应用身份 | v0.1.0 与 v0.1.2 均为 `dev.a3s.studio`、`aster-dream`，只变更版本 | [bundle-identities.json](bundle-identities.json) |
| 格式及静态检查 | 最终 `cargo fmt --all -- --check`、修改文件 oxfmt 和 oxlint 通过；ESLint 无错误、有一条现有依赖边界警告；i18n 校验通过 | 对应 fmt、oxlint、eslint、i18n 日志 |

原生集成测试实际使用 tauri-plugin-updater 及 loopback HTTP，MockRuntime 只提供无窗口宿主。失败场景确认 404、被改动归档及签名版本不符均不会改写旧 App。正常场景确认宿主、ACP、Office 整包更新，App 外的合成会话 sentinel 不变。生产公钥场景在包含空格的隔离路径复制原 v0.1.0 App，再安装最终 v0.1.2 归档，实际通过 `codesign --verify --deep --strict`、`stapler validate` 和 `spctl`。没有关闭发行配置中的 HTTPS 或签名保护。

中间一次打包与原生测试并行，导致共享资源目录被调试版覆盖；该候选已作废，日志保留为 `release-build-r2-mixed-companions.log`。最终串行重打包，并在冻结发行文件前核对 ACP、Office 的 release/bundle UUID 一致，见 [伴随程序身份](release-companions.json)。仅下列冻结文件用于交付。

## 候选文件与来源

- [macOS arm64 DMG](../../../../target/packages/asterdream-0.1.2-20261020/aster_0.1.2_aarch64.dmg)：75,408,832 bytes，SHA-256 `5e80def2d39dbe6da656f893d75e76a980e129140b2677a889dadfb5dee6bc14`。
- [签名更新归档](../../../../target/packages/asterdream-0.1.2-20261020/AsterDream.app.tar.gz)：74,840,124 bytes，SHA-256 `2fc48d0a0fc901474ee7913a0265ea332689e85e97f7f94ce80132193d9ccacb`，附同目录 `.sig` 和 `SHA256SUMS`。
- Apple 公证单号：`dec8d88d-b2e2-48ed-8fbf-aea823e0ad97`。签名团队：`5L9P7TS6DM`。
- 构建包含当前工作区未提交改动，并非干净 tag 构建；根仓库及 Work 的基线见 [source-provenance.json](source-provenance.json)。没有自动提交、合并、推送或发布这些混合改动。

本地 `upgrade-test-input.json` 仅提供集成验收所需版本及签名，没有下载 URL，不能用作线上清单。真正的 `latest.json` 仍须在版本资产发布后由验收脚本生成。

## Explicitly not claimed

- 未替换公开更新清单，未在真实旧客户端中通过公开源点击安装。当前线上 v0.1.0 更新失败仍需发布兼容资产及清单后才能恢复。本次验证是当前原生更新库对真实旧包隔离副本的完整安装过程。
- 未验证 Intel Mac、Windows、Linux 或另一台 Mac。CUA 使用 ad-hoc 测试副本；正式发行 App 的签名和 Gatekeeper 验证独立完成。
- 全量 Web 类型检查仍失败：当前工作区 278 条诊断、94 个文件，见 [typecheck-web.log](typecheck-web.log)。工作区依赖检查 4 项通过、2 项失败：已有 `apps/work/.git` 和 `a3s-gateway` 的 publish 元数据，见 [workspace-dependencies.log](workspace-dependencies.log)。未宣称完整 CI 通过，未运行会批量改写无关文件的全仓 `pnpm lint`。
- CUA 的旧模型测试地址不可用，没有把该次界面检查算成真实模型任务成功。目标按钮的状态来源及即时刷新仍有既有问题，快速命令回执也可能空白，详见 [CUA.md](CUA.md)。本轮不宣称修复了所有目标状态同步缺陷。
- T01 的交付通过独立验收，但原生目标终态停在 `needs_input`，未误报完成；完整记录见 [T01 报告](../../studio/2026-10-20-goal-t01-cua/OUTCOME.md)。没有重写该交付程序或改变原输入。
