# R12：Fcitx 4 异步按键前置过滤修复

配方恢复上游 Fcitx 4.2.9.9 的 `ENABLE_SNOOPER=ON`。旧 OFF 模块在真实 GTK 客户端中将每个英文按键投递两次；相同维护探针在新 ON 模块中每键只观察到一次。没有加入产品同步模式，也没有放宽前端的长按防重保护。

## 最小修改

- `build-fcitx4-immodule.sh`：R12 仅将 `-DENABLE_SNOOPER=OFF` 改为 `ON`。
- `gtk_input_probe.c`：phase 2 已关闭输入法后，被动统计 `a/b/c` 的 `key-press-event`；回调始终返回 `FALSE`，不修改事件。当实际文本达到 `你好abc` 时，断言三个计数各为 1，再进入取消候选测试。
- `test-input-method.sh`：清除私有测试进程继承的 `IBUS_ENABLE_SYNC_MODE`、`FCITX_ENABLE_SYNC_MODE`、`IBUS_DISABLE_SNOOPER`、`FCITX_DISABLE_SNOOPER`、`IBUS_NO_SNOOPER_APPS`、`FCITX_NO_SNOOPER_APPS`。这些覆盖可能使旧 OFF 配方也通过单次投递断言，因此不能带入本轮回归。

三个文件已在共享原树和隔离构建树中逐字节核对一致，并向主代理声明定稿。确切 SHA256 见 [stage 收据](snooper-r12-stage.json)。两个 shell 文件的 `bash -n` 与三个文件的限定 `git diff --check` 均通过。

## 负向与正向

| 检查 | r2 已安装 OFF 模块 | 仅重建 Fcitx 4 的 ON fixture |
| --- | --- | --- |
| 8 个协议场景 | 8/8 通过 | 8/8 通过 |
| 首次中文提交 | `你好` | `你好`，两种 display 均通过 |
| phase 2 英文投递 | `a=2 b=2 c=2` | `a=1 b=1 c=1`，两种 display 均通过 |
| 结果 | 退出 1，精确命中计数断言 | 退出 0，中文、混合英文、Escape 取消全部通过 |
| 耗时 | 3.89 秒 | 5.05 秒 |

负向不是超时或控制进程故障。它已经完成协议和首段中文，再由新增断言明确拒绝重复投递：[before 日志](snooper-r12-before.log)、[命令与结果](snooper-r12-before.json)。

正向运行同一维护脚本，依次测试宿主私有 `DISPLAY=:100` 和模拟 Flatpak 映射的客户端 `DISPLAY=:99.0`：[after 日志](snooper-r12-after.log)、[命令与结果](snooper-r12-after.json)。两条路径同时覆盖产品 launcher 的实际后端探测、当前会话总线 pin、GTK 模块加载、真实隔离 Fcitx daemon，以及协议回退。

构建仅运行现有 `build-fcitx4-immodule.sh`，34.19 秒成功：[构建日志](snooper-r12-build.log)、[构建命令](snooper-r12-build.json)。未运行主程序构建、打包、`flatpak install/run`，未替换已安装 r2，也未操作其他 GUI。

## 测试对象身份

负向读取已安装 r2 app commit `d588d6d5e186e0b960198af3dd75956ea4cd890367e47d4dcccf519b5a369bc8`。正向使用原 r2 stage 的独立 APFS clone：`target/evaluations/kylin-ime-hardening/r12-snooper/fixture-stage`。只覆盖重建的三个 Fcitx 4 原生二进制；其余 20 个关键文件、metadata、模块 id 和缓存保持原 r2 身份。[前后完整收据](snooper-r12-stage.json)

| 新原生文件 | SHA256 |
| --- | --- |
| `im-fcitx.so` | `73503354d8594dd03feb4cb27ad7a5e7f17a455eda58e2ea2e3138e8ed2d0734` |
| `libfcitx-gclient.so.1` | `7db5cc21ed9c62933e4785b221773027b64d091e40b2d92a9a7d4b268fe091d3` |
| `libfcitx-utils.so.0` | `10718ef3263b1b2b386e1fda073578526bbad3466acee01c9ef5ff69b2519cf0` |

脚本重建的三个二进制字节身份均发生变化。初版核验器曾假设两个辅助库应保持字节相同，此假设不成立；已将三个产物都作为新身份核对，并对它们共同运行上述协议与 GTK 正向测试。没有推断辅助库字节变化的原因。r3 发布收据须更新这三个原生身份，不能沿用 r2 的原生文件不变结论。

## 证明范围

本维护门禁证明 GTK widget 前的异步按键只投递一次，并保护中文提交、英文混输和候选取消。它使用排队 GDK 事件，不独立证明 WebKit 的 `DOM.repeat`、真实 XTEST 长按或完整麒麟桌面。真实 rich UI 的普通 Enter、长按和中文候选由主代理的独立 GUI 证据验收。机制与同步等待边界的实施前核对见 [原生调用链审查](R12-NATIVE-TRACE.md)。
