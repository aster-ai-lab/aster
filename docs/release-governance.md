# 构建与发布规范

本仓库在一个 Git 历史中维护多个产品。Git tag 只标识源码提交，不自动标识产品；因此每个发布流程必须使用自己的标签命名空间和更新通道，不得依赖仓库级 GitHub Latest 发现产品更新。

## 标签与 Latest

| 产品或用途 | 标签 | 发布内容 |
| --- | --- | --- |
| Aster 桌面版 | `vX.Y.Z` | 桌面安装包；通用 `v*` 命名空间只属于 Aster 应用 |
| Aster CLI | `aster-cli-vX.Y.Z` | 同源码构建的 `aster` 与 `a3s-code-acp` 成对归档及 `latest.json` |
| Core／ACP | `a3s-code-vX.Y.Z` | Core／ACP 源码版本与 ACP 平台归档 |
| 桌面滚动通道 | `desktop-stable` | `desktop-latest.json` 与桌面更新归档 |
| CLI 滚动通道 | `aster-cli-stable` | 指向当前版本化 CLI Release 的 `latest.json` |

只有 Aster 桌面版的版本化 Release 可以标记为仓库 Latest。Core／ACP、CLI 和滚动通道发布必须显式使用 `--latest=false`，不得改变 Latest。

GitHub Latest 上的 `latest.json` 是旧版 CLI 安装器和更新器的兼容入口。新版客户端不得读取该地址。每次发布新的 CLI stable 版本时，必须同时更新 `aster-cli-stable/latest.json` 和当前 Aster Latest Release 上的兼容 `latest.json`；清单始终指向版本化 `aster-cli-vX.Y.Z` Release，不指向滚动通道中的二进制。

## 构建边界

- 从仓库根目录使用固定 Rust 工具链和锁文件构建。
- CLI 的 `aster` 与 `a3s-code-acp` 必须从同一干净提交构建、验证并成对发布；归档同时携带共享 `BUILD_ID` 和 `SHA256SUMS`。
- 桌面版通过 `release-aster-desktop.yml` 构建并更新 `desktop-stable`；安装包版本与 Core／ACP 版本相互独立。
- Core／ACP 通过 `release-code.yml` 构建。只有 `a3s-code-vX.Y.Z` tag 可以触发该流程，tag 版本必须与两个 Cargo 包版本一致，并存在对应的 `crates/code/manual/RELEASE_X.Y.Z.md`。
- 不得从有未提交修改或未完成指定平台验证的工作树发布。发布记录必须能定位到准确源码提交。

## 发布顺序

### Core／ACP

1. 在干净提交上完成 Core／ACP 验证并更新版本与发布说明。
2. 推送 `a3s-code-vX.Y.Z`，不要推送裸 `vX.Y.Z`。
3. 由 `release-code.yml` 完成平台构建和 Release 创建；该流程固定 `--latest=false`。

### Aster CLI

1. 用 `release-aster-cli.yml` 构建三个平台归档并生成版本化草稿 Release。
2. 检查草稿的提交、归档成员、版本和 SHA-256 后，发布 `aster-cli-vX.Y.Z`。
3. 运行 `release-aster-cli-channel.yml`，输入相同的 `X.Y.Z`。该流程拒绝草稿或预发布版本，验证清单后更新 `aster-cli-stable` 和 Aster Latest 上的旧客户端兼容清单。

### Aster 桌面版

1. 用桌面发布流程完成签名、公证和平台验证。
2. 发布版本化 `vX.Y.Z` 安装包，并将其标记为仓库 Latest。
3. 确认 `desktop-stable/desktop-latest.json` 指向新版本。
4. 保留或刷新 Latest Release 上的 CLI 兼容 `latest.json`，验证旧入口仍能返回有效的 CLI 清单。

## 已发布内容的修正

- 不得把已发布 tag 移到另一个提交，也不得替换已发布二进制来冒充原版本。
- 标签命名空间错误可以在保持同一提交、同一二进制和同一 SHA-256 的前提下迁移 Release；验证新地址后再删除旧 tag。
- 滚动通道和明确约定的兼容清单可以更新；版本化归档的历史校验值不得因此改变。
- 修正后必须验证公开下载地址、Latest 归属和两个更新通道，而不能只检查 Release 页面标题。

`tests/release_governance.py` 固化上述命名空间、通道地址和 CLI 通道发布行为；主 CI 与 CLI 发布预检都必须运行该测试。
