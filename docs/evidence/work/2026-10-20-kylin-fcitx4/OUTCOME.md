# 麒麟 fcitx4 输入法修复（2026-10-20）

## 结论与范围

用户给出的进程名是 `fcitx` 和 `fcitx-dbus-watcher`，指向 fcitx4。本次修复针对既有麒麟 V10 SP1 x86_64 Flatpak 的 X11 路径；用户未提供可确认的应用版本或完整系统版本。

已修复原生输入法桥接和编辑器按键处理。完整 Linux v0.1.2 主程序、ACP 和 Office 已从根工作区重新构建。交付包及最终原生验证结果记录于下文。

## 原因与修复

1. 旧包只有 fcitx5 GTK 模块，不能代替 fcitx4 客户端。新增独立的 fcitx 4.2.9.9 GTK3 模块及其两个依赖库，使用 `$ORIGIN` RUNPATH；按桌面实际运行的输入法选择独立缓存，避免两个同名模块互相覆盖。不安装或替换用户的输入法守护进程。
2. fcitx4 GTK 客户端访问 `org.fcitx.Fcitx-N`，其中 N 来自 `DISPLAY`。仅允许固定 `org.fcitx.Fcitx` 名称不足以通信。Flatpak 改为 `org.fcitx.*` 命名空间权限，并按当前显示编号探测；继续保留 IBus 权限，不开放整个会话总线。
3. WebKit 的选词确认事件可能已经清除 `isComposing`，但仍带 `keyCode=229`。延迟加载输入框同时保护组合状态与 229；富文本输入框在 ProseMirror 的 `handleDOMEvents.keydown` 中截断整个快捷键和 suggestion 插件链，不调用 `preventDefault`，保留输入法默认处理。仅在局部 `handleKeyDown` 返回 false 会继续进入下游插件，因此不是完整修复。
4. 桌面入口和 Flatpak command 均使用 `kylin-launch`，原生启动 smoke 也改走相同入口。打包必须通过 GTK 模块实际加载与原生输入法协议测试，失败时不导出新包。

fcitx4 源包 SHA256：`307bddeabbec383eb27c4ea222d9f71d8d9011d7942f4010368e5bc3ca7440a1`。产品内附 GPL 许可证、对应源码归档和构建脚本。fcitx5-gtk 5.1.7 保留原有 LGPL 许可证。

上游依据：[应用与 Fcitx 的通信](https://fcitx-im.org/wiki/How_does_an_application_talk_to_Fcitx)、[fcitx4 显示专用连接实现](https://github.com/fcitx/fcitx/blob/4.2.9.9/src/lib/fcitx-gclient/fcitxconnection.c)、[Flatpak 总线权限](https://docs.flatpak.org/en/latest/sandbox-permissions.html)。

## 验证

| 检查 | 结果与证据 |
| --- | --- |
| 完整 v0.1.2 Linux 构建 | 主程序、ACP、Office 成功，Rust 1.97.1、x86-64 基线；[最终构建日志](build-host-v012-plugin-guard.log)。 |
| 编辑器回归 | 两份 composer 测试 212/212 通过；真实 Tiptap + suggestion 链的新事件用例修复前 4/4 失败，修复后通过。[前](composer-plugin-chain-before.log)、[后](composer-plugin-chain-after.log)。 |
| 启动选择和权限 | Linux 8/8 通过；macOS 7 通过、1 项真实 D-Bus 用例跳过。[Linux](launcher-linux-final.log)、[macOS](launcher-macos-final.log)。 |
| 安装器 | 单文件安装测试 1/1 通过；安装脚本 self-test 通过。[单文件](onefile-final.log)、[安装脚本](installer-final.log)。 |
| 原生协议正向 | 隔离控制辅助程序依赖后，当前策略连续 5/5 通过；最终 v0.1.2 暂存目录及污染环境分别通过。[最终暂存目录](native-v012-final.log)、[污染环境](native-positive-environment-isolation-final.log)。 |
| 原生协议反向 | 旧精确总线权限退出 1，显示专用连接被拒绝；缺少 fcitx4 退出 1，报告缺模块并在中文提交阶段超时，辅助程序无链接错误。[旧权限](native-negative-old-bus-policy.log)、[缺模块](native-negative-missing-fcitx4.log)。 |
| 最终包安装与校验 | 容器内 Flatpak 安装成功，16 个关键文件与暂存目录哈希一致，主程序、ACP、Office 与新构建一致；已安装包再次通过原生协议测试。[安装](bundle-v012-install.log)、[哈希与权限](artifact-v012-inspection.json)、[已安装包探针](native-v012-installed.log)。 |
| 冻结安装包 | gzip 及单文件载荷完整性通过，168 个载荷文件全部匹配 SHA256SUMS，内含应用包与已安装验证的应用包一致。[构建](onefile-v012-build.log)、[校验](onefile-v012-verification.json)。 |
| 格式和 lint | scoped oxlint、ESLint、oxfmt 通过；根 `cargo fmt --all -- --check` 通过。[编辑器 lint](composer-plugin-chain-lint.log)、[Rust fmt](cargo-fmt.log)。 |
| 全量 Web 类型检查 | 未通过：278 条诊断、94 个文件；之前更新修复已有相同数量，IME 修改处未新增诊断。包括原已存在的 ComposerSurfaceRuntime 可选布尔类型错误。[最终日志](typecheck-web-final.log)。 |
| 全量文档检查 | 未通过：184 个已有失效链接，首批为其他 evidence 日志和 ROADMAP 路径；未改写无关文档。[日志](docs-check.log)。 |

原生协议探针使用独立 Xvfb、D-Bus、fcitx4 配置目录，并按打包 metadata 的权限启动 `xdg-dbus-proxy`。GTK 客户端与守护进程的配置目录分离，避免从宿主 fcitx 地址文件绕过总线过滤。它使用实际打包的 launcher、GTK 模块及 GNOME 50 runtime，以 GDK 按键事件验证中文提交、中英文混输和 Esc 取消。

最初探针在中文提交后立即请求关闭输入法，再注入英文，曾因守护进程状态尚未改变而失败。保留 [打包时失败日志](package-v012-gated.log)，最终脚本在 GTK 事件循环中轮询实际状态，必要时重发幂等切换请求，达到所需状态后才注入后续按键，仍受 15 秒总限时约束。辅助控制程序的库只对子进程可见，GTK 不依赖测试注入的 `LD_LIBRARY_PATH`，因此缺模块反向测试不会被辅助程序的缺库错误冒充。

最终打包命令退出 0，记录于 [完整打包日志](package-v012-final.log)。其中保留 Flatpak 1.14.6 在导入离线 runtime bundle 时的 GLib/OSTree remote-name 诊断，以及既有未导出应用图标的警告；导入返回成功，应用 commit 与安装结果已另行核对。ComposerSurface 的 scoped ESLint 无错误、保留两条已有 hook 依赖警告，见 [日志](composer-lint.log)。

初期 `artifact-inspection.json` 和 `original-binary-identity.json` 仅用于验证旧 v0.1.1 二进制的打包夹具，不代表最终 v0.1.2。最终产物以 `artifact-v012-inspection.json` 和 `onefile-v012-verification.json` 为准。

## 本地交付

- 单文件：`target/packages/asterdream-0.1.2-kylin-x86_64-20261020/aster_0.1.2_kylin-v10_x86_64.run`。
- 大小：572,041,057 字节；SHA256：`761cc24f6cc305ab2cfbb39ba389113f10548bc2f0fb6d36c36d7e9f5a6b361c`。同目录保留归档和 `SHA256SUMS`。
- 应用 Flatpak SHA256：`af37c321dfa5983954785fb0d5de9a837fa4feda57e0a6935dac4621cabef436`。
- 应用 commit：`13573ab54e364f3935a8f9686e8a0b53052c78af227104b6b58f638614c639f0`。运行时为 GNOME 50，commit 和源码哈希见产物校验 JSON。

复制 `.run` 到麒麟电脑，以桌面登录用户运行 `bash aster_0.1.2_kylin-v10_x86_64.run`，不要加 sudo。更新后退出并重新打开 AsterDream。仅系统缺少 Flatpak 时，安装器才为系统依赖申请 sudo；应用安装仍属于桌面用户。

## 复现命令

在仓库根目录运行 `python3 tests/kylin_input_method.py`、`python3 tests/kylin_onefile.py` 和 `cargo fmt --all -- --check`。

在 `apps/work` 运行：

```bash
pnpm exec vitest run --project renderer src/renderer/components/composer/__tests__/ComposerSurface.test.tsx src/renderer/components/composer/__tests__/ComposerSurface.lazy.test.tsx
bash scripts/kylin-flatpak/install-on-kylin.sh --self-test
bash scripts/kylin-flatpak/build-host.sh
bash scripts/kylin-flatpak/build-immodule.sh
```

在具备依赖与 GNOME runtime 的 Linux amd64 打包容器中运行 `bash /src/scripts/kylin-flatpak/package-bundle.sh`。已有暂存目录也可直接运行 `bash /src/scripts/kylin-flatpak/test-input-method.sh APP_FILES RUNTIME_FILES`。依赖与完整容器命令见 Work 的 `scripts/kylin-flatpak/README.md`。本次使用专属容器 `aster-kylin-ime-native`，不读取桌面用户的输入法配置。

## Explicitly not claimed

- 尚未在用户的麒麟实机上完成整个 AsterDream/WebKit 界面的端到端验收。候选窗位置、真实硬件键盘、跨窗口焦点和搜狗本体仍需实机确认；GTK 协议探针及真实编辑器插件测试不能替代该验收。
- Wayland 保留原有未设置模块的行为，未经本次验证；缺少 GTK text-input 支持的合成器仍可能需要显式模块。未声称支持所有 Wayland、ARM 或兆芯硬件组合。
- Rosetta 内核拒绝 `PR_SET_SECCOMP`。模块检查使用同一已安装 runtime 的 bwrap 路径，协议探针使用受过滤的 D-Bus；产品权限和沙箱设置没有因此放宽。未声称在本机成功启动完整 Flatpak 沙箱 GUI。
- 截图中的模型 API HTTP 404 是接口路由问题，与输入法激活不同；本次没有猜测并修改用户接口协议。
- 本地验证和安装包不代表已发布版本；未声称完整 CI 全绿，也未改动其他任务的工作区修改。
