# 麒麟输入法修复的独立复审（2026-10-20）

审查对象为 Work 提交 `9aad91f` 的输入法改动、相关调用链及本地冻结的 v0.1.2 安装包。负载按单个桌面用户、普通中文输入、权限确认和离线升级评估。本轮没有修改产品源码或安装包。

结论：现有 fcitx4 模块闭包、独立缓存、RPATH 与产物身份核验成立；仍有 7 项具体问题。应先处理第 1 项误授权问题，并修复输入事件和安装路径问题后重新打包。暂不将现有包认定为完整输入法修复版本。

## 必须先修复

### 1. P1：输入法确认键可提交工具批准

[PermissionRequestComposer.tsx:329](../../../../apps/work/src/renderer/components/composer/variants/PermissionRequestComposer.tsx#L329) 仅检查 `nativeEvent.isComposing`。在打开但为空的拒绝备注框中，发送 `Enter / keyCode=229 / isComposing=false`，实际调用 `onRespond({approved:true})`。用户确认中文候选，可能变成批准待执行工具操作。

相同漏检存在于 [AskUserQuestionComposer.tsx:339](../../../../apps/work/src/renderer/components/composer/variants/AskUserQuestionComposer.tsx#L339)，会提前提交回答；[PromptVariableToken.tsx:99](../../../../apps/work/src/renderer/components/composer/PromptVariableToken.tsx#L99) 会提前退出变量编辑，并在该探针中触发 enter 和 blur 两次提交。

最小修复是让这些输入入口同时保护组合会话状态和 229，保留浏览器默认选词行为，加入真实 DOM 事件回归。主 Composer 的保护无法覆盖这些替代输入框。独立重现日志为 `target/evaluations/kylin-composer-review/permission-isolated.log`、`question-isolated.log`、`token-isolated.log`。

## 应修复

### 2. P2：变量选字结束后，编辑器仍处于组合状态

[ComposerSurfaceRuntime.tsx:1747](../../../../apps/work/src/renderer/components/composer/ComposerSurfaceRuntime.tsx#L1747) 的自定义 `compositionend` 返回 true，阻止 ProseMirror 自身的组合状态清理。真实 Composer/Tiptap/ProseMirror 探针中，选中的变量已变成“上海”，100 ms 后 `view.composing` 仍为 true；新加的 keydown 保护会继续跳过快捷键处理。

最小修复是完成变量业务更新后让 ProseMirror 接收结束事件，正常退出组合状态，包括空值和输入长度限制分支。不能靠测试手动将模拟 editor 的 composing 状态改成 false。独立重现：`composition-isolated.log`。

### 3. P2：取消选字会把拼音中间值写入变量

[ComposerSurfaceRuntime.tsx:1744](../../../../apps/work/src/renderer/components/composer/ComposerSurfaceRuntime.tsx#L1744) 和后续 `nextValue` 计算使用 `data || 旧值`，丢失空字符串的含义。对选中变量发送 `update("shanghai") → update("") → end("")` 后，原值 `${city}` 被改为 `shanghai`。

取消组合允许产生空数据，见 [W3C UI Events](https://w3c.github.io/uievents/#event-type-compositionend)。最小修复是保留明确的空字符串，取消时不写回旧 preedit，同时完成组合生命周期清理。独立重现：`cancel-isolated.log`。

### 4. P2：全局 Esc 捕获早于主编辑器保护

[QuickPanelView.tsx:947](../../../../apps/work/src/renderer/components/QuickPanel/QuickPanelView.tsx#L947) 在 window capture 阶段只检查 `isComposing`。`Escape / 229 / false` 实际关闭菜单并阻止默认行为，编辑器自己的事件保护尚未执行。

最小修复是让捕获处理器和面板键盘入口使用一致的输入法保护；回归必须挂载真实 QuickPanel，而非仅模拟 dispatchKeyDown。否则取消选词会关闭应用菜单。独立重现：`quickpanel-isolated.log`。

### 5. P2：把 Wayland 会话变量误当成应用实际后端

[kylin-launch.sh:50](../../../../apps/work/scripts/kylin-flatpak/kylin-launch.sh#L50) 仅凭 `WAYLAND_DISPLAY` 就进入不加载模块的分支。已用实际暂存模块重现：设置 `WAYLAND_DISPLAY=wayland-0`、`GDK_BACKEND=x11`、`DISPLAY=:0`、fcitx4 名称已拥有，`--print` 仍输出空 GTK 模块及缓存、空 NOTE。

[GTK 文档](https://docs.gtk.org/gtk3/running.html)确认 `GDK_BACKEND=x11` 选择 X11 后端；[Fcitx 文档](https://fcitx-im.org/wiki/Using_Fcitx_5_on_Wayland)区分原生 Wayland 与 XWayland 的模块要求。最小修复是至少正确处理显式 X11 后端，并给其他原生 Wayland 路径清楚的支持边界；不能只由会话变量决定应用输入链路。该情形会让 XWayland 窗口再次失去包内 fcitx 模块。

### 6. P2：离线安装器检查错误的文件系统

[install-on-kylin.sh:121](../../../../apps/work/scripts/kylin-flatpak/install-on-kylin.sh#L121) 检查 `df -Pk .`；单文件入口通过 `mktemp` 在临时目录解包，然后进入该目录。`flatpak --user` 实际安装位置在用户数据目录。

隔离 preflight fixture 中，临时文件系统为 2 GiB、HOME 空闲 20 GiB，仍退出“磁盘不足 3GB”。反过来，HOME 空间不足也未被该检查覆盖。最小修复是分别检查解包空间和 `${XDG_DATA_HOME:-$HOME/.local/share}/flatpak` 所在文件系统，新增临时目录与安装目录分盘的回归。

### 7. P2：原生测试仍继承可覆盖总线的环境变量

[test-input-method.sh:30](../../../../apps/work/scripts/kylin-flatpak/test-input-method.sh#L30) 未清理 `FCITX_DBUS_ADDRESS`。已打包的 fcitx4 客户端优先使用此变量，再使用测试的会话总线或配置文件；共享 `/tmp` 及网络使可达的外部地址能够绕过测试的 filtered proxy。

两条私有 D-Bus 的实证中，指定 session bus A 和 override B，真实产品客户端连接 B；清除 override 后连接 A。最小修复是测试初始化时清除此变量，并在环境污染回归中显式设置它。无需据此修改产品启动器。探针保留于 `target/evaluations/kylin-ime-review/address_probe.c` 和 `address_probe.sh`。这不证明之前干净构建容器的通过记录无效，但测试不完全独立。

## 尚缺的目标环境验证

- 冻结安装包包含 `flatpak_1.6.5-0ubuntu0.5_amd64.deb`，现有输入法协议验证使用 1.14.6/bwrap。Flatpak 1.6.5 会将宿主显示编号映射为 `:99.0`，见 [官方实现](https://github.com/flatpak/flatpak/blob/1.6.5/common/flatpak-run.c#L175-L190)。标准 fcitx4 的 generic alias 和 portal fallback 可以兼容，不能据此断言产品必坏；但当前保留宿主 DISPLAY 的探针没有覆盖该实际路径，应补目标 Flatpak 的显示映射和 portal 集成验证。
- 尚无麒麟实机完整 AsterDream/WebKit 验收。需要覆盖候选窗、Space/Enter 确认、Esc 取消、变量编辑、权限备注、追问、跨窗口焦点及更新后首次启动。GTK 协议与 JS 事件探针分别通过，不代表二者在实机上的组合链路通过。
- 最终打包日志保留应用图标未导出的既有警告，属于较低优先级交付缺口，不影响上述输入法问题的判断。

## 本轮核对通过的事实

冻结安装包 SHA256、572,041,057 字节大小、13 份源码哈希与原报告一致。212 个编辑器测试、5/5 原生正向和两个反向结果的日志均与报告相符。两次 Web 类型检查的 278 条错误逐行一致。

本轮重新运行 launcher 测试：macOS 7 通过、1 个真实 D-Bus 用例跳过；单文件安装测试 1 通过。上述 6 个新增前端探针均单独运行并重现问题，未将多项同时失败归因为同一个问题。原生地址覆盖探针使用独立临时总线，没有连接用户桌面会话。
