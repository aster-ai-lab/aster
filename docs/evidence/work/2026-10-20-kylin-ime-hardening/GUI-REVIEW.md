# 最终包的隔离 WebKit 输入观察

**整体 GUI 验收未通过。** 最终包能经真实 fcitx、Flatpak 和 WebKit 在应用中输入中文，但 rich composer 在首次聚焦后触发 React #185 并被 WorkbenchHome 的备用 textarea 替代。因此下列中文截图仅证明备用输入框的原生输入链。它们不能证明 Tiptap 的候选确认保护、任务会话提交或目标麒麟实机通过。

## 包、环境与边界

- 最终安装 commit：`d8477930b0cc4c2453e61425916fef9d13acadc26ed2e0f5ac9bde3f9757eb3c`；每次启动前以 `flatpak info --show-commit` 精确比较。二进制与 client 摘要在 [versions.json](gui-observations/versions.json)。
- 真正使用离线安装的 **Flatpak 1.6.5** 与 GNOME 50 runtime，启动已安装应用，而非 GTK probe。Xvfb 的 `:150` 被 Flatpak 映射到 `:99.0`。
- 每个 run 使用独立 mount/network namespace、DBus、XDG runtime/config/data/cache/state 与 fcitx profile。只启用 loopback；应用 netns 中只有 `lo`，路由为空。startup-proof 记录了应用实际环境及 namespace/mount 证据。
- Flatpak 1.6 会将 XDG data/config/cache 强制设为应用的 `.var/app` 路径。测试通过 private mount 把独立空目录 bind 到该路径，未修改 HOME，未读取、复制真实会话或账户配置。只生成明显无效的 synthetic session 字面量，以进入本地界面；这是输入测试，不验证登录。
- 容器的 root / Rosetta 运行约束要求**仅测试**移除 bwrap 的 seccomp fd，并设置 `WEBKIT_DISABLE_SANDBOX_THIS_IS_DANGEROUS=1`。wrapper 校验其他语义参数逐一相同，记录在 [bwrap-experiment.jsonl](gui-observations/bwrap-experiment.jsonl)。没有修改产品 metadata、源码或已安装二进制；这些实验不能作为沙箱安全验收。
- A3S Test 的当前 Linux GUI profile 为 unsupported，见 [gui-certification.json](gui-certification.json)。本次使用独立 XTEST/截图实验，不是 A3S CUA 认证，也不是麒麟发行版实机验收。AT-SPI 只暴露 GTK 框架/菜单，未暴露 WebKit 子树；截图文字由人工视觉复核，未写成自动 DOM 文本断言。

可重放的测试脚本快照在 [gui-fixture/run.sh](gui-fixture/run.sh)。它们只作用于已拥有的测试容器与仓库 target 路径。脚本不注册账户、访问外部聊天服务或复制用户数据。所有输入使用 ASCII XTEST 物理键序列；没有剪贴板或 Unicode 文本注入。

## 已观察到的输入行为

以下均发生在 **WorkbenchHome 备用 textarea**；不能计为 rich composer guard 通过。

| 输入链 | 可复核结果 | 截图 |
|---|---|---|
| fcitx 4.2.9.9：`n i h a o` → Space | 草稿“你好” | [modern-chinese.png](gui-observations/modern-chinese.png) |
| 关闭输入法 → `a b c` | 草稿“你好abc” | [modern-mixed.png](gui-observations/modern-mixed.png) |
| 开启输入法 → `n i` → Escape → `h a o` → Space | 草稿“你好abc好”，取消的 `ni` 未写入 | [modern-cancel-resume.png](gui-observations/modern-cancel-resume.png) |
| `n i h a o` → Return | 候选关闭，草稿为“nihao”；仍在首页 | [候选](gui-observations/modern-candidate.png)、[Return 后](gui-observations/modern-candidate-return.png) |
| 官方旧协议 daemon：`n i h a o` → Space | 草稿“你好” | [old-chinese.png](gui-observations/old-chinese.png) |
| 旧协议 daemon：在重复确认的“你好你好”后，`n i` → Escape → `h a o` → Space | 草稿“你好你好好”，取消后可以继续输入 | [old-cancel-resume.png](gui-observations/old-cancel-resume.png) |
| 旧协议 daemon：清空草稿，`n i h a o` → Return | 草稿“nihao”；仍在首页 | [old-candidate-return.png](gui-observations/old-candidate-return.png) |

本 fcitx 默认 Space 选择首个中文候选，Return 提交拼音原文；不把 Return 描述成选择了中文。旧 daemon 来自 [官方 4.2.9.6 archive](https://download.fcitx-im.org/fcitx/fcitx-4.2.9.6.tar.xz)，源码 SHA-256 为 `d55eee3f094037b5c32f0c5397d585fdf932cbc7d2091ed6f78fd7a9ae512c47`。该 archive 的 CMake project version 仍为 4.2.9.5，构建后的 `-v` 也如此；不是麒麟发行 binary。其隔离构建没有 X11 候选窗口 addon，所以只验证 DBus/portal 输入提交与取消恢复，不验候选窗外观。现代 host 候选窗也因容器缺少 CJK 字体而显示方框，应用内中文已正常显示。

## 为什么候选 Return 仍不能验收

增加普通 Return 正对照后发现：任何 IME 输入之前，纯 ASCII `abc` 后 Return 也只换行。重做新鲜 profile，分别使用默认 fcitx、`FCITX_ENABLE_SYNC_MODE=1` 和不启动任何 daemon，结果相同。对应 [fresh-ascii-return.png](gui-observations/fresh-ascii-return.png)、[sync-ascii-return.png](gui-observations/sync-ascii-return.png)、[no-daemon-ascii-return.png](gui-observations/no-daemon-ascii-return.png)。同步模式没有改变现象，不能据此改产品使用同步模式。

只读 observer 才确定实际目标。该独立诊断 run 向已有 WebKit user content manager 增加被动、有限的事件元数据监听；不改文本/偏好/editor 状态，不派发或阻止任何事件。源码、边界和编译方法在 [observer README](gui-fixture/readonly-observer/README.md)。它可能扰动时序，故保留上述无注入基准。

[dom-events.jsonl](gui-observations/dom-events.jsonl) 中：

- seq 8–9：最初聚焦过 `DIV.composer-tiptap`，`viewComposing=false`。
- 随后原生日志记录 React #185；完整 minified stack / componentStack 已提取到 [render-errors.json](gui-observations/render-errors.json)。同一错误也存在于现代、旧协议、sync、no-daemon **全部无注入基准**，不能归因观察库。
- seq 42–51：实际接收 Return 的已是 WorkbenchHome 备用 TEXTAREA。`key=Enter`、`code=Enter`、`keyCode=13`、`isComposing=false`、全部 modifier=false，capture 与 bubble 均 `defaultPrevented=false`。这个目标的 class 精确对应 `WorkbenchHome.tsx:1809`；该 textarea 只有 onChange，没有 onKeyDown。

因此普通换行没有经过 Tiptap 的 guard 或发送处理。蓝色发送按钮虽然清空了本地草稿，隔离环境没有 gateway model，未建立成功会话；这也不能代替发送正对照。修复或消除 rich composer 的 React render 错误后，必须在确认实际目标仍为 `DIV.composer-tiptap` 的条件下重做普通发送与候选确认对照。不能给备用 textarea 临时加 Enter handler 掩盖 rich composer 退出。

## 证据完整性与剩余范围

[validation.json](gui-observations/validation.json) 明确标记整体 GUI 未通过与 rich guard 未验证。[screenshots.json](gui-observations/screenshots.json) 将每张选用截图绑定原 run 路径与 SHA-256；对应 actions 和 startup-proof 亦保留。少数捕获帧为黑屏/过渡帧，未作为成功证据；取消恢复使用后续稳定文字帧。

最初 `run-LlQ2Vu` 未正确隔离 Flatpak 强制使用的 profile，停在登录页且未输入，完全排除。首次 observer run 已正常装载，但最初误读外层 app.log；实际 stderr 在独立 XDG_STATE_HOME/AsterDream/logs，没有把缺少外层输出当成产品失败。`run-s0m5xz` 停止期间脚本快照被更新，旧 shell 最终报语法错误；该 fixture 收尾错误不作为产品故障结论，其先前截图与独立 startup-proof 保留用于普通 Return 现象对照。后续 run 均正常退出；[cleanup.json](gui-observations/cleanup.json) 确认已记录的应用 PID 均不存在。

仍欠缺：rich composer 的真实候选/发送验收、实际麒麟发行版的 daemon/图形栈与输入法实机验收，以及原生 Wayland。当前证据不覆盖这些范围，也不覆盖认证、外部模型聊天或完整任务执行。
