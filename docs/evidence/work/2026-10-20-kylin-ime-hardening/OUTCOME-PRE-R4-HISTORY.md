# 麒麟输入法缺陷优化与复审（2026-10-20）

状态：R1–R11 的根因修复、相关回归及 r2 安装包静态身份核验已完成；r2 的真实无模型富文本场景不再崩溃。完整 GUI 验收仍未通过：带模型的真实富文本发现 R12，首次按键被异步重放后误标为 repeat。已用同包对照确认恢复上游前置按键处理可消除此问题，正在形成维护回归和下一轮包。`hardening/` 与 `hardening-r2/` 均保留作历史复现，暂不作为完整修复交付。范围见 [PLAN.md](PLAN.md)，缺陷来源见 [前一轮审查](../2026-10-20-kylin-fcitx4/REVIEW.md)。

r2 的构建、23 项产物身份与现代/最低旧协议原生复测见 [R2-DELIVERY.md](R2-DELIVERY.md)；以下最初冻结包的记录保留为历史。新增真实生命周期测试的两处类型诊断已修复，完整 Web 类型检查回到相同的 278 条既有诊断，见 [对照收据](typecheck-web-r2-comparison.json)。

## 已完成的实现与回归

- R1、R4：权限备注、追问、变量 textarea 和 QuickPanel 的事件入口同时保护组合会话、原生组合标志和 229；浏览器继续处理选字/取消。普通快捷键恢复有回归。前端 A 的 15 个新增用例分别在原代码失败，修复后相关 168 项通过，见 `frontend-a-validation.json` 与 `frontend-a-final.log`。
- R2、R3：显式空组合数据保留取消语义；变量的自定义 compositionend 不再阻止 ProseMirror 结束组合。真实 Composer/Tiptap/ProseMirror 的 5 项覆盖提交、两种取消、缺省结束数据和长度拒绝；不手动改写 view.composing。见 `composer-ime-final.log`。
- 前端共 8 个相关测试文件 **384/384 通过**（`frontend-regression-final.log`）；随后仅修改新测试的编辑器初始化等待与 TypeScript 收窄，5 项复跑及格式检查通过。独立对抗复核未发现新的确定前端缺陷，见 [FRONTEND-REVIEW.md](FRONTEND-REVIEW.md)。
- R6：解包空间按 gzip 解码后的 tar 字节和 16 MiB 余量检查；安装空间按 Flatpak 用户数据目录及最近存在的父目录检查。支持 FLATPAK_USER_DIR、XDG_DATA_HOME 和分盘；无可用容量信息时拒绝安装。7 项安装器测试和自检通过，见 [INSTALLER.md](INSTALLER.md)。
- R7：测试清理 FCITX_DBUS_ADDRESS，默认包门禁主动注入另一条存活总线；GTK 保留过滤总线，控制辅助程序单独连接私有控制总线。宿主显示和映射 :99.0 两条路径通过，见 `native-isolation-final.log`。移除 fcitx4 模块仍按预期失败。
- R5：进一步复审实证，有序 GDK 后端列表仍能回退到 X11，不能从 WAYLAND_DISPLAY 或单个字符串判断。改为小型 GDK 探针查询实际 display 类型，不初始化 GTK 输入模块。Linux launcher **12/12 通过**，macOS **10 通过、2 项环境跳过**；实际 GNOME 50/Xvfb 的四种后端设置均正确选用 X11/fcitx。失败探针有诊断，仍将主程序启动交给应用处理。见 [BACKEND-PROBE.md](BACKEND-PROBE.md)。
- R8：最低验收基线为官方 4.2.9.6 源包的旧 portal 协议。4.2.9.9 客户端先试新路径，仅在明确 UnknownObject/UnknownMethod 时重试一次旧路径；拒绝、失败、编码取消与 NoReply 不回退。真实客户端的维护协议测试 **8/8 通过**，原库的三项失败各单跑确认。新旧输入上下文均验证了 FocusIn 和中文 CommitString。见 `fcitx-protocol-validation.json`。
- R9：产品入口固定 FCITX_DBUS_ADDRESS 到当前非空会话过滤总线；辅助控制显式单独设置两个地址。最终门禁直接给 GTK 产品入口注入存活空总线 B，现代 daemon 与旧协议 daemon 的宿主/映射显示共四条路径均通过。见 `native-modern-product-polluted.log`、`native-old4296-final-product-polluted.log`。
- R10：复用现有 gdk-pixbuf-thumbnailer，关闭缺 ImageMagick 时静默漏图标的分支；无生成结果则拒绝导出。最终导出无图标缺失警告，安装后 PNG 为 128×128，身份核对通过。

最终独立原生复审未发现新的确定缺陷，见 [FINAL-NATIVE-REVIEW.md](FINAL-NATIVE-REVIEW.md)。两个最终负向夹具逐路径核对，各只有一个差异：删除模块或移除产品地址固定块，都先通过 8 个协议用例，再可靠停在 GTK phase 0，退出 1；没有把控制辅助程序失败算作 GTK 失败。见 [FINAL-NATIVE-NEGATIVES.md](FINAL-NATIVE-NEGATIVES.md)。

## 必须更正的测试解释

原先“仅允许精确 org.fcitx.Fcitx 的旧权限必定使 GTK 输入失败”的推论不成立。Flatpak 隐式允许 portal 调用和相应广播；原探针还把 fcitx-remote 控制辅助程序放在过滤总线上，混入了控制通道失败。

当前探针补齐隐式 portal 规则，并将辅助控制与被测 GTK 分离。旧权限下 GTK 可以走 portal，通过宿主和映射显示两种模式，见 `native-old-policy-isolated-control.log`。保留此前失败日志，不能再将其归因为 GTK 必然无法输入。缺模块和不可达 GTK 通道才是可靠的负向控制。

## 实际随包 Flatpak 1.6 的证据

已执行离线包内 Flatpak 1.6.5、bubblewrap 0.4.0、xdg-dbus-proxy 0.1.2：宿主 :120 映射至客户端 :99.0，确认 -99 服务未被拥有，真实 portal 追踪为 1 次 CreateInputContext 和 38 次 ProcessKeyEvent，中文、混合英文、Esc 取消通过。只禁用隔离 daemon 的 portal 插件会失败。

Rosetta 拒绝 PR_SET_SECCOMP；实验包装器仅删除该参数对，并逐项断言其余语义参数一致。包装器不随产品交付，结果不证明 seccomp 执行。此次实验证明的是已冻结旧前端包的原生链路；需要和本轮新前端构建分开记录。完整身份、命令和限制见 [flatpak16-RESULT.md](flatpak16-RESULT.md)、`flatpak16-proof.json`。

最终新包已用同样的随包 Flatpak 1.6.5 重新执行现代和旧协议 daemon：两者均从宿主 :120 映射到 :99.0，中文、混输、Esc 取消通过。现代路径为 1 次工厂调用/38 次按键；旧协议为新路径一次失败后旧路径一次成功/38 次按键。只禁用 portal 的负向控制失败；原始 seccomp 路径依然被 Rosetta 拒绝。身份与原始参数一致性见 `flatpak16-final-proof.json`。该旧源包内部版本仍标为 4.2.9.5，报告按其官方 archive SHA256 标识，不能冒称已执行麒麟发行版原装 binary，也不能把通过扩展为所有更旧版本或发行版分支。

## 静态检查和隔离构建

- 相关前端 scoped Oxlint、ESLint、Oxfmt 通过；i18n 检查通过，校验 19,468 条翻译，未新增界面文案。
- Web 类型检查仍有 278 条既有诊断，与基线的文件/错误码计数一致；本轮新测试产生的 10 条诊断已全部修复。不能称全量类型检查通过。
- 原共享树 cargo fmt 因另一任务的 ACP 修改失败；未格式化该任务文件。隔离工作树 `cargo fmt --all -- --check` 通过。
- 隔离工作树的 workspace_dependencies.py 为 6 项中的 4 项通过、2 项失败：已有嵌套 Work Git 布局与测试的 `.git` 不存在断言冲突；基线 a3s-gateway 的 publish 字段为 None。未修改这些基线条件，也未放宽测试。
- 首次共享树主机构建因其他任务正在改 ACP 而主动停止，退出 137；随后使用同仓库的根/Work worktree，固定根 `d4a8393`、Work `9aad91f` 加本轮补丁重新构建。未复制在途 Rust 修改、用户 .env 或会话状态。准备身份见 [BUILD-WORKTREE.md](BUILD-WORKTREE.md)、`build-worktree-preparation.json`。

## 构建、交付与身份

**GUI 拒收记录**：聚焦 rich composer 触发 React #185，然后 ErrorBoundary 落入 WorkbenchHome 的普通 textarea。无注入的现代、同步、无 daemon、旧协议 run 均有相同错误栈。只读 DOM 观察确认普通 Return 是 Enter/13、无组合/修饰键，但实际目标为 fallback textarea；它只有 onChange，没有键盘发送处理。此前中文、混输和 Esc 截图证明 fallback 的真实 WebKit/输入法链路，不能证明 rich composer 或候选 Enter 保护通过。没有启用同步模式或给 fallback 加发送键来掩盖该错误，R11 修复与真正 rich surface 复测待完成。

隔离 `build-host.sh` 成功，同批构建 renderer、Office kernel/JS、ACP、Office CLI 和主程序；`package-bundle.sh` 的协议、GTK、运行库检查通过，导出后再安装最终 bundle，并对实际安装路径重跑门禁通过（`build-host-isolated.log`、`package-final.log`、`bundle-final-install.log`、`native-installed-final.log`）。可选 Git commit 构建字段受容器内 worktree 指针限制，完整源码身份另行冻结。

新冻结目录为 `target/packages/asterdream-0.1.2-kylin-x86_64-20261020-hardening/`，包含 ship、tar.gz、单文件 `.run` 和 SHA256SUMS。应用 commit 为 `d8477930b0cc4c2453e61425916fef9d13acadc26ed2e0f5ac9bde3f9757eb3c`。单文件为 **572,051,119 字节**，SHA256 为 `d8df9d1e0a783da3425bd2ca8edb6db37803337c235ec345462b85074767909a`。

独立正式核验通过：23 个 stage/installed 关键文件、10 个 x86_64 ELF、两份缓存的实际 ID/绝对路径、图标、入口与精确权限、完整 GPL 源码/补丁/构建调用均一致；168 个 ship 清单项目、169 个 tar 文件、gzip CRC、`.run` 载荷与 tar 逐字节一致、所有 SHA256SUMS 通过。见 `artifact-final-proof.json`。原始/隔离两份树的 **29/29 源文件**与冻结收据一致，见 `source-freeze-final.json`、`source-freeze-independent-proof.json`。未提交、推送或发布。

首轮产物核验器误把 fcitx5 的实际 ID 当成必须为 fcitx5，已根据真实缓存首条注册记录更正；两个模块注册 ID 可同为 fcitx，正是独立缓存的原因。保留 `artifact-first-verifier-assumption.*`，未改产品来满足错误断言。

交付过程的环境问题也保留：thin node_modules 的宿主绝对依赖链接在容器内补只读依赖桥后可读；Docker 对缺失挂载目标创建的 `.env` 是 0 字节占位，未复制真实 .env。一次导出重启发生重叠，停止两个自有进程并清理隔离中间目录后重新导出；最终 commit/载荷来自重新完成的单次导出。对已经安装的同一 commit 重复安装返回 already installed，后续实验改为先断言最终 commit 身份，没有将该重复安装当成升级失败。旧中断日志保留，最终包校验另行通过。

## Explicitly not claimed

- 未取得麒麟实机完整 AsterDream/WebKit、候选窗位置、物理按键、跨窗口焦点或更新后首次启动的验收证据。
- GTK 协议探针与真实框架 DOM 测试分别通过，不代表二者在目标实机上的完整端到端通过。
- 本机 A3S Test 报浏览器 0.3.2 不受支持（要求 >=0.4.0,<0.5.0），见 `a3s-test-capabilities.json`；没有据此声称 CUA/A3S GUI 验收通过。
- 不宣称旧 daemon、搜狗专有组件、原生 Wayland 或 ARM 已全面验收；不宣称 Rosetta 下的沙箱安全已验证。
- 不宣称全仓类型检查或全部 workspace 门禁通过；未处理其他任务的 ACP/core/cloud 或模型 HTTP 404 问题。
