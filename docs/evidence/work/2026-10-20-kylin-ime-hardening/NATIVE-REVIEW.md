# 原生输入法二次审查

本审查不改产品源码。发现三条可复现的原生缺口；其中后端探测与地址处理已交主任务修复。下列失败证据绑定修复前 launcher（SHA256 `973422121c736208b2524f00cbca953f2fd5b04c09ddc73a1fb04ecf603eafed`）及原始 fcitx4 客户端库（SHA256 `986d45a8579bad6337c2528aa02a62154b0c1eaf7c11942683ffe839112dd1c6`），不代表随后修改的源码仍然失败。

## 已确认的问题

1. **旧 portal 对象路径不兼容。** 官方 4.2.9.6 源包的 daemon 注册 `/inputmethod`，返回 `/inputcontext/N`；产品 4.2.9.9 客户端只调用 `/org/freedesktop/portal/inputmethod`。同一个旧 daemon 在保留宿主 `DISPLAY=:100` 时，中文、混输和取消全部通过；仅把客户端显示映射为 Flatpak 的 `:99.0` 后，中文提交阶段超时、退出 1。[整链路日志](native-old4296-before.log)。因此，显示专用名称不可用而需要旧 portal 时，当前原始客户端不能连接。

   最小兼容修复：仅当新 factory 返回 `UnknownObject` 或 `UnknownMethod`，重试一次旧 factory；拒绝、断线、取消与其他错误不得触发回退。接口名称和方法签名兼容，IC 路径应使用返回值。旧 IC 信号在 `/inputcontext/N`，需要精确的 `org.freedesktop.portal.Fcitx` talk 权限；不开放完整会话总线，也不借通用 direct alias 改变多显示器语义。[旧协议源码](https://github.com/fcitx/fcitx/blob/4.2.9.6/src/frontend/ipcportal/ipcportal.c)、[旧路径定义](https://github.com/fcitx/fcitx/blob/4.2.9.6/src/frontend/ipcportal/ipcportal.h)、[新路径定义](https://github.com/fcitx/fcitx/blob/4.2.9.9/src/frontend/ipcportal/ipcportal.h)。

   测试源码来自官方 `fcitx-4.2.9.6.tar.xz`，SHA256 `d55eee3f094037b5c32f0c5397d585fdf932cbc7d2091ed6f78fd7a9ae512c47`，构建和安装均留在 `target/evaluations/kylin-ime-native-review/`。上游该源包及同名 tag 的 CMake 版本仍为 4.2.9.5，二进制也如此自报；这是旧协议兼容性实证，不是麒麟发行版原装二进制验收。[构建日志](native-old4296-build.log)、[上游版本定义](https://github.com/fcitx/fcitx/blob/4.2.9.6/CMakeLists.txt)。用户实际 daemon 的发行版版本和补丁仍未知。

2. **产品继承的 `FCITX_DBUS_ADDRESS` 可覆盖 launcher 的总线选择。** 相同产品夹具在干净环境中双显示模式通过；只给被测 GTK 进程设置另一条合法但没有输入法服务的隔离总线地址后，中文提交超时、退出 1。辅助控制程序明确使用测试 daemon 总线，不继承该地址，故失败不是辅助程序误连造成的。[干净对照](native-product-address-clean.log)、[覆盖失败](native-product-address-override.log)。调用链为 `fcitxconnection.c::_fcitx_get_address()` 优先读取环境变量，成功连接该地址后不会因那里没有服务而改回 launcher 检测的总线。这是路由正确性问题，不是由此证明了权限提升。[客户端源码](https://github.com/fcitx/fcitx/blob/4.2.9.9/src/lib/fcitx-gclient/fcitxconnection.c)。测试脚本清除变量只能修复测试隔离，产品自身也必须固定到实际允许的会话总线。当前源码已在修复此项，本报告未为修改后的版本签署验收。

3. **GDK 后端列表与自动回退不能用环境变量存在性判断。** 实际 GTK/Xvfb 中，`GDK_BACKEND=x11` 正确选择 X11 与 fcitx 模块；`x11,wayland` 及 `wayland,x11` 在 Wayland socket 不存在时均实际使用 X11，但旧 launcher 留空模块。[复现日志](native-gdk-backend-review.log)。探针使用 GTK 3.24.41；目标 GNOME 50 的 GTK 3.24.52 也按列表逐项尝试后端，调用链一致。[GTK 3.24.52 源码](https://github.com/GNOME/gtk/blob/3.24.52/gdk/gdkdisplaymanager.c)。该日志确认选择不一致，不单独证明完整应用中文失败。主任务正在改为真实 GDK 探测，修复后的结果应另行记录。

## 证据边界

- Flatpak 1.6.5 实验确实使用随包 Flatpak、bubblewrap 0.4.0 和 proxy 0.1.2 二进制，验证了宿主 `:120` 到沙箱 `:99.0`、portal 调用与中文提交。stock 运行被 Rosetta 的 seccomp 拒绝；通过记录来自只移除 `--seccomp` 参数的测试包装器，不能称为未经修改的完整沙箱通过。该实验绑定此前 v0.1.2 包，不是本轮尚未冻结的新产物。
- 原先“旧精确总线权限必须失败”的推论不成立：Flatpak 默认允许 portal 调用和指定路径的信号，新 daemon 可以经 portal 工作。当前测试已保留该正向对照；缺模块和 portal 不可用是不同的负向条件。
- 多输入法共存时，当前明确优先 fcitx5、其次 fcitx4、最后 IBus。拥有 D-Bus 名称不能证明哪个是用户正在使用的引擎；本次没有取得可确认的目标共存会话，故不把该政策直接判为新缺陷，也不声称所有共存配置已兼容。
- 尚无麒麟实机完整 WebKit 验收。上述协议、地址及后端实验均使用自有配置、Xvfb 和私有 D-Bus，不读取桌面用户配置；未安装或替换用户输入法。

本轮仍需对修复后的客户端运行旧 daemon 整链路、错误分类回归及最终包身份校验；不得仅凭 4.2.9.9 正向通过宣称旧 portal 已支持。

## 错误分类维护回归

新增 `scripts/kylin-flatpak/fcitx_client_protocol_test.c`，通过 GModule 加载真实客户端库，每例使用独立进程和 GTestDBus。成功例检查 factory 次数、返回 IC 路径上的 FocusIn，以及真实客户端收到中文 CommitString；失败例等待客户端异步处理结束后的 GObject finalization，不靠固定等待推断没有回退。

原始客户端 8 例中 3 例失败：UnknownObject、UnknownMethod 不能回退，双 UnknownMethod 未走到一次旧 factory；其余 5 例通过。打入主任务补丁后的真实库 8/8 通过，包括新路径不回退、旧路径仅一次、AccessDenied/Failed/typed Cancel/NoReply 不回退。[修复前](fcitx-protocol-before.log)、[修复后](fcitx-protocol-after.log)。编译使用 `-Wall -Wextra -Werror`；调用方式为 `fcitx-client-protocol-test LIBFCITX_GCLIENT`。此测试只验证客户端协议分支，不能替代旧 daemon 整链路及最终包复验。
