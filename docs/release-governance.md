# 构建与发布规范

本仓库在一个 Git 历史中维护多个产品。Git tag 只标识源码提交，不自动标识产品；因此每个发布流程必须使用自己的标签命名空间和更新通道，不得依赖仓库级 GitHub Latest 发现产品更新。

## 标签与 Latest

| 产品或用途 | 标签 | 发布内容 |
| --- | --- | --- |
| Aster 桌面版 | `vX.Y.Z` | 桌面安装包；通用 `v*` 命名空间只属于 Aster 应用 |
| Aster CLI | `aster-cli-vX.Y.Z` | 同源码构建的 `aster` 与 `aster-acp` 成对归档及 `latest.json` |
| Core／ACP | `aster-acp-vX.Y.Z` | Core／ACP 源码版本与 ACP 平台归档；标签与归档前缀一致 |
| 桌面滚动通道 | `desktop-stable` | `desktop-latest.json` 与桌面更新归档 |
| CLI 滚动通道 | `aster-cli-stable` | 指向当前版本化 CLI Release 的 `latest.json` |

历史标签 `a3s-code-v*` 与短暂使用的 `aster-v*`（Core／ACP）不再接收新发布；既有归档可按「已发布内容的修正」迁到 `aster-acp-v*`，且必须保持同一提交与同一 SHA-256。迁移完成后删除旧 Release 与旧 tag，不得并存两套同版本地址。

禁止创建名为 `latest` 的 Git tag 或 Release。GitHub 的「Latest」是 Release 标记，不是标签名；字面 tag `latest` 与 `/releases/latest` 语义冲突，属于历史错误入口，发现即删。

只有 Aster 桌面版的版本化 Release 可以标记为仓库 Latest。Core／ACP、CLI 和滚动通道发布必须显式使用 `--latest=false`，不得改变 Latest。

GitHub Latest 上的 `latest.json` 是旧版 CLI 安装器和更新器的兼容入口。新版客户端不得读取该地址。每次发布新的 CLI stable 版本时，必须同时更新 `aster-cli-stable/latest.json` 和当前 Aster Latest Release 上的兼容 `latest.json`；清单始终指向版本化 `aster-cli-vX.Y.Z` Release，不指向滚动通道中的二进制。

## 保留与清理

每个产品线的**版本化** Release 默认只保留当前版与上一版（共 2 个），便于回滚；更早的版本化 Release 与对应 tag 应删除。滚动通道 tag（`desktop-stable`、`aster-cli-stable`）永久保留，但通道上的历史二进制同样只保留当前与上一版所引用的归档。

| 保留 | 删除 |
| --- | --- |
| 当前与上一版 `vX.Y.Z`（桌面，含仓库 Latest） | 更早的桌面版本化 Release／tag |
| 当前与上一版 `aster-cli-vX.Y.Z` | 更早的 CLI 版本化 Release／tag |
| 当前与上一版 `aster-acp-vX.Y.Z` | 更早的 Core／ACP 版本化 Release／tag；全部 `a3s-code-v*`／`aster-v*`（Core／ACP） |
| `desktop-stable`、`aster-cli-stable` 及各自当前清单 | 字面 tag／Release `latest`；通道内超出保留窗口的旧归档 |

清理顺序：先确认通道清单与仓库 Latest 仍指向保留窗口内的版本与 SHA-256，再删 Release，最后删远程 tag。清理后必须复核公开下载地址、Latest 归属和两个更新通道。

## 构建边界

- 从仓库根目录使用固定 Rust 工具链和锁文件构建。
- CLI 的 `aster` 与 `aster-acp` 必须从同一干净提交构建、验证并成对发布；归档同时携带共享 `BUILD_ID` 和 `SHA256SUMS`。
- CLI Linux 归档按架构拆分：`linux` 为 x86_64，`linux-arm64` 为 aarch64；二者均以 glibc 2.31 为基线。
- 桌面版通过 `release-aster-desktop.yml` 构建并更新 `desktop-stable`；安装包版本与 Core／ACP 版本相互独立。
- Core／ACP 通过 `release-code.yml` 构建。只有 `aster-acp-vX.Y.Z` tag 可以触发该流程，tag 版本必须与两个 Cargo 包版本一致，并存在对应的 `crates/code/manual/RELEASE_X.Y.Z.md`。归档文件名使用同一前缀（`aster-acp-vX.Y.Z-<platform>.*`）。
- 不得从有未提交修改或未完成指定平台验证的工作树发布。发布记录必须能定位到准确源码提交。

## 发布顺序

### Core／ACP

1. 在干净提交上完成 Core／ACP 验证并更新版本与发布说明。
2. 推送 `aster-acp-vX.Y.Z`，不要推送裸 `vX.Y.Z`，也不要再使用已退役的 `aster-vX.Y.Z` / `a3s-code-vX.Y.Z`。
3. 由 `release-code.yml` 完成平台构建和 Release 创建；该流程固定 `--latest=false`。

### Aster CLI

1. 用 `release-aster-cli.yml` 构建平台归档。Aster CLI 包含 `aster`、`aster-acp`，以及离线 Model2Vec 资产目录 `embedding/`（`aster-acp` 发行构建关闭 `bundled-embedding`，运行时从该目录加载）。进程命令隔离由锁定的 ksandbox 依赖提供，不再打包 sandbox companions 或 kbox MicroVM。
   CLI 与 Studio 的 ACP feature 分叉：CLI 为 `--no-default-features --features svg-png,account-import`；Studio / 本地默认保留 `bundled-embedding`（及 Office 等桌面能力）。`aster-core` 在 ACP 上显式启用 `local-code`。
   在发布机上下载同一构建 run 的归档到 `dist/`，设置 `GH_REPO`、`BUILD_SHA`、`ARTIFACT_RUN_ID`，从干净的对应提交运行 `bash scripts/publish-aster-cli.sh`。
   发布脚本先核对归档来源提交的分支资格与归档成员，再生成草稿。任一失败均禁止创建或上传 Release。
2. 检查草稿的提交、归档成员、版本和 SHA-256 后，发布 `aster-cli-vX.Y.Z`。
3. 运行 `release-aster-cli-channel.yml`，输入相同的 `X.Y.Z`。该流程拒绝草稿或预发布版本，验证清单后更新 `aster-cli-stable` 和 Aster Latest 上的旧客户端兼容清单。

#### CLI 更新器兼容

- **通道清单**：`latest.json` 的 `assets` 可增加新平台键；已发布更新器必须忽略未知键，不能因未知平台拒绝整份清单。先发布带该行为的 CLI，再把新平台键写入 `aster-cli-stable`。
- **归档成员**：新增*必需*成员（如 `embedding/`）会使更旧的更新器无法解包。不要假设 `aster update` 能跨越这类布局变更；公告中写明用 `scripts/install.sh` / `scripts/install.ps1` 重装，或先发过渡版本。
- **禁止**为迁就旧更新器而跳过未知归档成员后仍报告成功：会装出缺文件的半套 pair。

### Aster 桌面版

桌面运行组件统一使用 `.aix` 插件，发布到 `aster-ai-lab/plugins` 仓库关联的 GitHub Packages（GHCR），由受信 TUF 目录绑定 OCI manifest、AIX blob 和原生包摘要。沙箱、文档引擎（包含 ONNX Runtime）、Office 分别使用 `aster/kbox`、`aster/aster-document`、`aster/aster-office`。平台是发布变体，不进入包 ID。底座构建只消费已发布的组件锁（`ASTER_RUNTIME_COMPONENTS_LOCK`），不重建运行时插件。底座内的 `runtime-components.json` 固定兼容版本、平台、包与归档摘要；已安装且版本、摘要一致的组件不再安装。Office 首次使用时安装。

`pnpm package:host` 构建现有 `aster-use` CLI，并通过 `registry package build/check/prepare` 生成 AIX、OCI manifest 和发布证据；这些构建产物不上传到桌面 Release。只有 `plugins` 的包发布流程与受信目录负责组件分发，桌面首次启动复用同一扩展 Registry 的安装记录和不可变版本目录。

首次迁移先发布完整过渡包（默认 `ASTER_DEFER_RUNTIME_COMPONENTS=0`），让客户端具备 AIX 组件与轻量更新包能力，并把随包资源通过原生 AIX 生命周期导入版本缓存。官方 GHCR 包、匿名读取、TUF 签名根与目录接入完成后，才能提供已发布组件锁和 `ASTER_PLUGIN_ROOT_SHA256`，启用 `ASTER_DEFER_RUNTIME_COMPONENTS=1` 生成并发布轻量底座；新安装必须能在没有用户预设源的机器上安装固定组件。未完成该验收时不能把候选轻量包放入稳定通道。仍被保留底座引用的 OCI digest 不得删除或替换。

轻量构建会使用临时配置，通过原生 SDK CLI 校验受信目录与锁文件，并真实匿名安装全部三个组件；私有包、摘要漂移或安装失败均会阻断发布。组件内容版本控制在冻结的 64 字符选择器上限内，归档、原生包和 manifest 仍使用完整 SHA-256 校验。

1. 用桌面发布流程完成签名、公证和平台验证。
2. 发布版本化 `vX.Y.Z` 安装包，并将其标记为仓库 Latest。
3. 确认 `desktop-stable/desktop-latest.json` 指向新版本。
4. 保留或刷新 Latest Release 上的 CLI 兼容 `latest.json`，验证旧入口仍能返回有效的 CLI 清单。

## 已发布内容的修正

- 不得把已发布 tag 移到另一个提交，也不得替换已发布二进制来冒充原版本。
- 标签命名空间错误可以在保持同一提交、同一二进制和同一 SHA-256 的前提下迁移 Release；验证新地址后再删除旧 tag。
- 滚动通道和明确约定的兼容清单可以更新；版本化归档的历史校验值不得因此改变。
- 修正后必须验证公开下载地址、Latest 归属和两个更新通道，而不能只检查 Release 页面标题。

## 分支卫生

默认分支只有 `main`。远程功能／修复／杂务分支在对应 Pull Request **合并或关闭后删除**；已完成发布的 `release/*` 分支在版本化 tag 与 Release 就绪后删除（提交由 tag 保留，勿依赖长期 release 分支）。

| 保留 | 删除 |
| --- | --- |
| `main` | 已合并 PR 的头部分支 |
| 仍打开的 PR 头部分支 | 已关闭且不打算再开的 PR 头部分支 |
| （无长期 `release/*`） | tip 已包含在对应产品 tag 中的 `release/*`；`ahead=0` 的陈旧实验分支 |

禁止把 `release/*` 当作第二个长期集成线。发布专用修复应先回到 `main`，或确认已打进版本化 tag 后再删分支。仓库应开启「PR 合并后自动删除头部分支」，避免再堆积已合并分支。

`tests/release_governance.py` 固化上述命名空间、通道地址、保留策略和 CLI 通道发布行为；主 CI 与 CLI 发布预检都必须运行该测试。

## 运行时组件锁与打包链路

`package_runtime_components.py` 的 `COMPONENTS` 与所有 runtime component lock 必须**同步修改**：组件从清单移除时，lock 中对应条目必须删除，不能只改名。0.2.1 发布事故即由此产生——`99aa11588f` 移除了 sandbox 组件，却只把 lock 条目从 `aster-sandbox` 改名为 `kbox`，`validate_pin` 因组件集合不符在打包最后一步拒绝所有 lock。三份 lock 需保持一致：`apps/studio/resources/runtime-components.lock.json`、`plugins/registry/locks/runtime/darwin-arm64.json`（本地 registry 镜像）与官方 `aster-ai-lab/plugins` 仓库的 `registry/locks/runtime/darwin-arm64.json`；保留组件的 SHA-256 身份不得改动。

`tests/runtime_component_locks.py` 校验 lock 与 `COMPONENTS` 的组件集合、结构合法性以及各 lock 间 SHA 身份一致。任何改动组件清单的提交都必须运行该测试，并至少用 `verify_runtime_registry.py` 对官方 registry 做一次真实匿名安装复验。

本机在智能体沙箱内打包时，SwiftPM 的嵌套 `sandbox-exec` 与批量删除保护属于**会话级环境限制**，不是产品缺陷，不得为此在产品构建脚本中加入沙箱绕过逻辑。可发布版本优先由 `release-aster-desktop.yml`（`macos-14` + Node 24.12）构建；本机应急只允许使用临时 fixture（构建进程 PATH 注入 `swift build --disable-sandbox`、把构建产物目录移走备份而不是删除），且 fixture 不得进入产品源码或已发布二进制。
