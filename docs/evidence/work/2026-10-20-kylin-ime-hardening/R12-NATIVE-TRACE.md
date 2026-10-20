# R12 富文本 repeat 的原生调用链只读核对

新 rich 场景的日志与“Fcitx 异步事件在 WebKit repeat 统计之后重放”的机制一致。当前还不能把它标为已修复；应以本轮 fresh rich 的 sync、前置 snooper 和正常长按控制验证。未使用旧 fallback 的 sync 结果排除本次问题。

## 实际 DOM 与注入记录

`run-wCZiI4` 只计 `window-capture`，共有 18 次 keydown，全部 `repeat=true`；17 次 keypress；36 次 keyup。后者按键名、code、timestamp 分组只有 18 组，每组恰有两次。因此双 keyup 不是同一事件 capture/bubble 的统计误差。

首次 Return 是序号 214：`Enter`、code `Enter`、keyCode 13、repeat true，组合状态与四个修饰键均 false。随后序号 216、218 是相同 timestamp 72068 的两次 capture keyup。actions 中只有一次普通 `xdotool key --clearmodifiers --delay 120 Return` 命令，没有 Shift 注入或手工长按。

只读 observer 仅被动读取事件属性，没有派发事件或改写 repeat。此前 [独立 GDK 注入核验](RETURN-INJECTION-REVIEW.md) 在自有 `:180` 中验证同一普通 Return 命令只有一对 press/release、修饰键为零。尚未在本次 proof 的 Xvfb 上新增原始 XEvent 采样，因此这两项是“未见 fixture 重复注入证据”，不是将 GDK 结果替代当前实际 DOM repeat。

## 随包 Fcitx 与 WebKit 的闭合候选

随包、带 SHA 固定的 4.2.9.9 原始 `gtk3/fcitximcontext.c`：

- 660–684：sync 分支在当前调用中返回；async 分支复制按键、异步发送，并吞掉原事件。
- 692–700：未消费/错误结果会给副本加 IgnoredMask，再用 `gdk_event_put` 重新排队。

WebKit GTK3 先记 repeat、再执行 IM filter；未收到 release 时，再来同一硬件码会判为重复。其 release 统计则在过滤之前清空。此顺序使异步重放 press 可能先后两次进入 tracker，只有第二次进入 DOM，从而首个 DOM keydown 已是 repeat true。[WebKit GTK3 入口](https://raw.githubusercontent.com/WebKit/WebKit/webkitgtk-2.52.6/Source/WebKit/UIProcess/API/gtk/WebKitWebViewBase.cpp)、[repeat tracker](https://raw.githubusercontent.com/WebKit/WebKit/webkitgtk-2.52.6/Source/WebKit/UIProcess/API/glib/KeyAutoRepeatHandler.cpp)

WebKit 的统一 IM filter 对 release 返回未处理结果，即使 GTK IM 吞了原 release；因此原 release 与随后 Fcitx replay 可分别进入 DOM，解释当前每键双 keyup 的形态。[InputMethodFilter](https://raw.githubusercontent.com/WebKit/WebKit/webkitgtk-2.52.6/Source/WebKit/UIProcess/API/glib/InputMethodFilter.cpp)

以上使用明确 WebKit tag 佐证机制，不把该 tag 冒充已识别的 GNOME runtime 源码版本；最终因果仍需当前包的 A/B。

## 优先验证上游默认的前置 snooper

上游 4.2.9.9 `CMakeLists.txt:36` 默认开启 snooper，当前构建配方显式关闭。class init 376–379 读取两个 DISABLE 变量，407 始终安装 GTK hook；没有命中 NO_SNOOPER_APPS 排除项时，同时设 `IBUS_DISABLE_SNOOPER=0` 与 `FCITX_DISABLE_SNOOPER=0` 可在现有二进制启用它，适合做不重建的对照。

`_key_snooper_cb` 在 WebKit widget 处理之前运行。它仅在有效且有焦点的 Fcitx context 下发送事件；async 分支 1629–1645 先吞原 press/release，IgnoredMask 重放则在 1600–1601 直接放行。这可使 WebKit 只看到一遍 replay press/release，理论上同时消除首键 repeat 与双 keyup，不必引入同步主线程等待。当前产品名称不在上游默认排除列表中；外部 NO_SNOOPER_APPS 覆盖仍须在实验环境中明确记录。

本次没有修改配方或加入环境覆盖。恢复上游行为是否充分，应由 fresh rich A/B、普通 tap、正常 hold-repeat、中文提交/取消和失去焦点后的恢复共同判断；前端已有 repeat 保护不应凭此放宽。

## 对现有 GTK 门禁补回归的设计评估

在当前 `gtk_input_probe.c` 的 phase 2（IME 已确认关闭、注入 `abc`）对 GtkEntry 的 `key-press-event` 只读计数，并在实际文本到达 `你好abc` 时检查 `a/b/c` 各一次，是合适的小型维护回归。GTK 文档明确普通 `g_signal_connect` handler 先于默认 handler，返回 false 继续传播，因此该计数器能观察 widget 默认 IM filter 之前的原事件与 replay；不应改键态或阻止传播。[GTK 按键 signal 顺序](https://docs.gtk.org/gtk3/signal.Widget.key-press-event.html)

在机制成立时，OFF+async 应各观察两次，ON+async 应各观察一次。逐键断言比总数三更容易识别混合异常；在真实文本状态断言能避免额外固定睡眠。这里 `key()` 注入的是 GDK 事件，不能把它称为硬件物理事件或完整 WebKit 回归；它与 rich XTEST A/B 分别保护前置边界和最终 DOM 行为。

该维护用例必须从私有环境清除两个 ENABLE_SYNC_MODE、两个 DISABLE_SNOOPER 和两个 NO_SNOOPER_APPS 覆盖。否则继承 sync=1 或显式 snooper=0 覆盖可使旧 OFF 配方也观察到各一次，造成错误通过。原 build OFF 的负向必须按该精确计数失败，才证明用例保护了本轮根因。

## sync 等候边界

`fcitxclient.c:587` 与 `:599` 的两条同步 ProcessKeyEvent 使用 `-1` 及空 cancellable。完整归档搜索未找到 proxy 默认 timeout setter。GIO 定义 `-1` 为继承 proxy 默认值；默认通常为 25 秒，`G_MAXINT` 才表示无 timeout。所以现有同步调用不是源码指定的无限等待，但活着却不回复的 daemon 可能让 GTK 主线程长时间停顿。[GIO call_sync](https://docs.gtk.org/gio/method.DBusProxy.call_sync.html)、[默认 timeout 属性](https://docs.gtk.org/gio/property.DBusProxy.g-default-timeout.html)

本次未实测 runtime 的具体等待时长或 daemon 断线延迟，也不猜新的超时阈值。若最终方案仍采用 sync，至少需要分别测“未连接 daemon 的本地 fallback”、“已连通后 daemon 暂停不回复”和“已连通后 daemon 退出”，记录等待时长、UI 主循环是否继续以及后续按键恢复。异步前置 snooper若通过，可避免为 repeat 修复额外启用这个同步 UI 等候路径。

本次事件计数、Enter 序列、原始输入与源归档身份保存于 [独立 trace](r12-native-independent-trace.json)。只读检查，没有产品修改、容器执行或 GUI 操作；A/B 结果由 proof 代理另行记录。
