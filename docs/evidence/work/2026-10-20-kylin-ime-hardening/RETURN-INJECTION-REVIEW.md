# Return 注入的独立键态核验

未发现 fixture 将普通 Return 改成 Shift+Return 的证据。`target/evaluations/kylin-ime-gui/driver.py:16` 构造 `['xdotool', 'key', '--clearmodifiers', '--delay', '120', *args[1:]]`，不添加 Shift 或其他修饰键，也不改写传入的 `Return`。

在自建 Xvfb `:180` 和独立 GTK 按键记录窗口中执行同一命令，结果如下：

| 注入条件 | GDK keysym / keyval | 硬件 keycode | 按下时 state |
| --- | --- | --- | --- |
| `Return` | `Return` / 65293 | 36 | 0 |
| `Shift+Return` 正向控制 | `Return` / 65293 | 36 | 1（Shift） |
| `ctrl+Return` 正向控制 | `Return` / 65293 | 36 | 4（Control） |
| `alt+Return` 正向控制 | `Return` / 65293 | 36 | 8（Mod1） |
| `super+Return` 正向控制 | `Return` / 65293 | 36 | 67108928（Super + Mod4） |
| 先同时按住 Shift/Control/Alt/Super，再执行原始普通 Return 命令 | `Return` / 65293 | 36 | 0 |

两个普通 Return 条件中，按下与释放的 state 均为 0，Shift、Control、Alt/Mod1、Meta、Super/Mod4 全部为 false。正向控制验证记录器能观察到对应修饰键；`--clearmodifiers` 在已有修饰键按住时也成功清除了普通 Return 的键态。

[原始有界日志](native-gui-return-injection.log) 与 [源码身份及机器断言](native-gui-return-injection.json) 已保存。记录器和 runner 留在 `target/evaluations/kylin-ime-hardening/return-injection-review/`。运行退出 0，未修改产品源码或现有 driver，未操作其他 DISPLAY、proof 应用进程或停止容器。

本核验只证明所列 Xvfb/GDK 事件的键态。它不证明真实 WebKit 页面的 DOM `key`、`shiftKey`、`isComposing` 或编辑器快捷键状态；普通 Return 在应用内换行的原因仍需用实际 DOM 观察结果定位。
