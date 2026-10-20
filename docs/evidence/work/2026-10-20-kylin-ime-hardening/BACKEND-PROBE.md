# GDK 实际后端与产品输入法总线修复

## 修复

`kylin_im_backend.c` 仅初始化 GDK 显示连接，不初始化 GTK 输入法模块。它读取真实的 `GdkDisplay` 类型，只输出 `x11` 或 `wayland`。启动器使用这个结果选择输入法，不再从桌面会话变量或 backend 列表猜测实际后端。探测最多占用五秒；失败或未知输出会明确诊断，并继续执行主程序自己的显示错误处理。

探针随既有 `build-immodule.sh` 使用现有 GDK 开发依赖编译。打包要求 Linux amd64 ELF、检查 GNOME runtime 链接，并安装可执行文件及对应 AGPL 源码到既有 AsterDream 许可证目录。无需添加依赖包或主机配置。

产品启动器同时把 `FCITX_DBUS_ADDRESS` 固定为当前非空 `DBUS_SESSION_BUS_ADDRESS`，防止继承的宿主覆盖或旧地址文件把输入法客户端导向另一条总线。当前 session 地址缺失或为空时只清除覆盖，不构造地址，不修改宿主配置。

## 验证

- 根 `python3 tests/kylin_input_method.py`：macOS 10 通过、2 跳过，见 [日志](backend-probe-after-macos.log)。跳过的是实际 Linux D-Bus 和 GDK/Xvfb 用例。
- 验证容器运行同一维护测试：12/12 通过，见 [日志](backend-probe-after-linux.log)。包含真实 GDK 探针、Xvfb 和 `x11`、`x11,wayland`、`wayland,x11`、`*` 四种选择；每种都携带失效 Wayland socket 和无效 GTK 输入模块配置，仍由真实显示类型选出 X11 模块。
- 实际 GNOME 50 runtime 的 bwrap/Xvfb 验证：四种 backend 均报告 `x11` 并选择 fcitx；只允许不可用 Wayland 时，探测失败有诊断而主程序目标仍执行，见 [日志](backend-probe-native-runtime.log)。
- `-Wall -Wextra -Werror` 编译和 `ldd` 成功；依赖 GDK、不依赖 GTK，见 [构建记录](backend-probe-native-build.log)。
- 原启动器的四个失败及后续产品总线覆盖失败已按不同用例单独复现，见 [初始失败](backend-probe-before.log)、[逐项失败](backend-probe-before-isolated.log)。产品 session 地址 fixture 全部为测试自建值，不记录真实用户总线地址。
- `bash -n` 与 scoped `git diff --check` 通过。

原树修改已同步到隔离构建 worktree，启动器、构建/打包钩子、C 源码、说明、根测试和编译后的探针共七个路径哈希一致，见 [同步身份](backend-probe-isolated-sync.json)。根主程序构建无需因这些独立打包文件中止。该记录是初始 worktree 准备后的后续同步，最终交付仍须保存最终完整源码身份。

## Explicitly not claimed

- 原生 Wayland 中文输入和完整麒麟硬件 WebKit 界面仍未验收；真实 GDK fallback 测试只证明所列环境实际选择 X11。
- 产品总线覆盖的原生中文提交由主代理的专用产品探针进一步验证；本记录中的新增子进程测试证明环境固定规则，不把它冒充完整中文端到端验收。
- 最终安装包尚需由主代理重新导出、安装和冻结；本阶段没有发布、提交或暂存代码。
