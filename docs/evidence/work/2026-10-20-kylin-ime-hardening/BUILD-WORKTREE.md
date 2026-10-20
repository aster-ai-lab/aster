# 隔离构建准备

为避免其他线程修改共享 Rust 源码影响本次构建，建立同仓库 worktree：

- 根目录：`target/worktrees/kylin-ime-hardening-20261020`，detached `d4a8393f12a6d526a4d08693ac76e73e87414b14`。
- Work：上述目录内 `apps/work`，detached `9aad91f1e934c731e540cf99be25733e25c3a411`。
- 仅应用当前 Work 的 21 个已跟踪文件 diff，并显式复制新增 `ComposerSurface.ime.test.tsx`；根目录仅复制 `tests/kylin_input_method.py` 和 `tests/kylin_onefile.py`。没有复制其他线程的 Rust、cloud 或 API 修改。
- 补丁保存在 `target/evals/kylin-ime-hardening-20261020/work.patch`，SHA256 为 `d7c31f63840170728f9ef6a88046df31f06bf7f18ded71d9dd15769f59c6d99b`。

源文件逐项哈希、Git 状态、依赖和缓存记录在 [准备身份 JSON](build-worktree-preparation.json)。隔离 Work 的完整已跟踪 diff 与补丁逐字节相同；22 个 Work 源文件和两个根测试均与快照来源匹配。没有暂存或提交修改。

## 构建依赖

10 个 `node_modules` 目录使用薄包装目录，保留原有包与 workspace 的相对符号链接，只共享现有 `.pnpm` 依赖 store。没有复制工具缓存、包管理器会话状态或 `.env`。`@a3s-lab/office`、UI 和 intelligent-ui 均解析到隔离 Work 内；新 Office 构建输出不会落到原工作区。

只复制了必需的离线依赖：`dependencies/offline-flatpak`、fcitx 源归档、`python`、`python-wheels`、`node`、`immodules`、`native-libs` 和 `native-licenses`。未复制旧 Flatpak 暂存目录、仓库、冻结包或旧主程序/ACP/Office 二进制到 `dist`。

确认本次旧构建容器已经停止，且仍运行的专属验证容器仅有 `tail -f /dev/null` 后，以 `cp -cR` 将 `target/kylin-x86_64` APFS 克隆到隔离根目录的 `target`。样本文件哈希相同、inode 不同；构建产物缓存不与原目录共享可变文件。

## 后续构建命令

准备阶段未启动构建。主代理复核后可运行：

```bash
cd /Users/roylin/code/aster/target/worktrees/kylin-ime-hardening-20261020/apps/work
CARGO_TARGET_DIR=/Users/roylin/code/aster/target/worktrees/kylin-ime-hardening-20261020/target \
STUDIO_RENDERER_DIST=.studio-package \
KYLIN_BUILD_IMAGE=aster-kylin-build:24.04 \
bash scripts/kylin-flatpak/build-host.sh
```

主程序、ACP 和 Office 由该脚本同批构建。宿主 Office wasm 目标目录和 Linux 目标目录均留在隔离根目录；Docker 镜像和锁定的下载依赖缓存沿用现有环境。

## Explicitly not claimed

- 本文件仅证明构建准备和快照身份；尚未运行新构建、安装或 GUI 验收。
- worktree 的 `.git` 指针使用宿主绝对路径。当前 `build-host.sh` 的容器挂载不包含这些 Git 元数据，`emit_commit()` 会省略可选的 `ASTER_DREAM_COMMIT`；源码身份由上述快照与哈希单独保存。这不会把另一线程的源码加入构建。
- 共享 `.pnpm` store 是现有锁定依赖；不应在准备或构建期间运行包安装或修改 store。
