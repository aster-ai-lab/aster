# R8/R9 最终原生独立审查

本次只读复核未发现新的确定产品缺陷或交付阻断。复核未修改产品源码，未操作正在导出安装包的容器，也未提交或暂存代码。以下结论限定于所列源码和既有运行记录。

## 异步回退与对象生命周期

- `fcitx4_legacy_portal.patch:34`：异步 `call_finish` 通过 `GError` 读取明确错误，并在失败分支清理错误对象。仅当前 proxy 路径恰为 `/org/freedesktop/portal/inputmethod` 且错误为 `UnknownObject` 或 `UnknownMethod` 时，重试旧 `/inputmethod`。旧路径再次失败不能满足路径条件，因此每次输入上下文创建最多回退一次。权限拒绝、通用失败、取消类错误和传输失败均不进入此分支。
- 补丁基于随包保存的 4.2.9.9 原始源码追踪：`_fcitx_client_create_ic` 持有一个 `self` 引用；原有 direct → portal → context 三段异步链传递该引用。新增重试沿用同一个引用，未提前释放或额外增加引用；旧 factory 创建失败、旧 factory 调用失败及最终 context proxy 回调分别由原有终止路径释放它。重试前旧请求的 cancellable 已释放，下一段创建自己的 cancellable；原有 `_fcitx_client_clean_up` 取消并清理仍在途的请求。
- `fcitx_client_protocol_test.c:98` 在错误请求尚未回复时释放外部 client 引用，依靠真实 client 的最终化回调证明异步终止已完成；断言包含新旧 factory 的精确调用次数。旧路径成功还必须通过真实 client 的 `connected`、返回路径上的 `FocusIn` 和中文 `CommitString`，不能仅靠返回非空结果通过。
- [修复前记录](fcitx-protocol-before.log) 中 3 个回退用例失败；[修复后记录](fcitx-protocol-after.log) 的 8 个用例全部通过，包括现代成功不回退、两种明确未知错误各回退一次、旧路径再次未知时停止及 4 种拒绝路径。[验证身份](fcitx-protocol-validation.json) 保存所测 client 和 fixture 的哈希。

## 产品总线与控制辅助程序隔离

- `test-input-method.sh:60` 创建另一条存活但没有 Fcitx 名称的私有总线 B；`test-input-method.sh:99` 给 GTK 产品进程设置过滤后的会话总线 A，同时故意把 `FCITX_DBUS_ADDRESS` 指向 B。`kylin-launch.sh:136` 在执行 GTK 目标前将该变量固定到非空当前会话地址，因此此正向测试需要产品本身的地址修复才能通过。
- `gtk_input_probe.c:14` 只为 `fcitx-remote` 子进程建立单独的环境。该环境同时把 `DBUS_SESSION_BUS_ADDRESS` 和 `FCITX_DBUS_ADDRESS` 指向私有控制总线 A，并恢复 daemon 所在的宿主 DISPLAY；控制程序所需旧库目录也仅传给这个子进程。GTK 主进程继续使用产品启动器设置的过滤总线和真实 runtime 库，不继承控制程序的地址覆盖或库路径。
- `test-input-method.sh:79` 同时覆盖宿主 DISPLAY 与映射到 `:99.0` 的 X socket/cookie；GTK 主进程的显示映射不会被控制程序恢复宿主 DISPLAY 的行为掩盖。
- 现代 daemon 的 [污染产品环境记录](native-modern-product-polluted.log) 和官方 4.2.9.6 源包 daemon 的 [污染产品环境记录](native-old4296-final-product-polluted.log) 均在这两种 DISPLAY 下完成中文、混合英文及 Escape 取消。最终 [包门禁记录](package-final.log) 同时包含 8 项协议通过及两种 DISPLAY 的 GTK 通过。
- `package-bundle.sh:330` 精确授予 `org.freedesktop.portal.Fcitx`，覆盖旧 portal 返回路径上的信号；未为此增加完整 session bus 暴露。`build-fcitx4-immodule.sh:21` 用 `--fuzz=0` 应用补丁；`package-bundle.sh:200` 将许可证、精确原始源码、补丁及构建脚本一起保存。

## 覆盖边界

- `fcitx_client_protocol_test.c:102` 的 `cancelled` 用例由服务器返回编码的 `G_IO_ERROR_CANCELLED`。它证明该错误不会触发旧路径回退，并证明对应异步错误路径释放 client；它未主动调用客户端的 `g_cancellable_cancel`，不能据此宣称已测真实取消与断线重连之间的时序。
- 协议 fixture 加载实际待打包 client，但本身运行在 Linux 构建环境的 GLib/GIO 上；GTK 门禁使用 GNOME 50 runtime 和真实隔离 Fcitx daemon。二者互补，均不等同于完整 WebKit 应用、真实麒麟硬件桌面、候选框位置或原生 Wayland 输入法验收。
- 本复核只核验已有运行收据，并在宿主执行 shell 语法检查及日志断言；未重复启动容器测试。新包导出、安装及最终载荷身份由主任务另行记录。

## 本次核验身份

| 路径（`apps/work/scripts/kylin-flatpak/`） | SHA-256 |
| --- | --- |
| `fcitx4_legacy_portal.patch` | `a5330e62b2fa668920261857593b6efc38016cbcb72b4535e96091ac9e7d9755` |
| `fcitx_client_protocol_test.c` | `ced6920541e3e14b90431a9fe1042e71ea6b5a7dfedc84401e4d4e19ca69e69a` |
| `test-input-method.sh` | `0eeedbc8210418988cb0717e35a1d8972cfb1466aec96f08124118b641b54766` |
| `gtk_input_probe.c` | `1d20306dfe10f1de9d41a8fc2af41b1125f8005ad7d1ff93b72ea623c2769eeb` |
| `kylin-launch.sh` | `6a6f459d0b7561faf7de2b400012e10f5e4d34342861f25ac896f052f37a9f84` |

本机复核：`bash -n` 通过；协议日志 8 项通过断言成立；现代与旧 daemon 的日志各有两种 DISPLAY 的完整通过记录。
