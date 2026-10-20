# 麒麟输入法深度修复与交付验收（R4）

**当前结论：R1–R13 与 V1–V2 在本轮限定范围内完成。源码、定向回归、最终 R4 构建/安装、产物、原生协议和真实 WebKit GUI 均通过对应验收，证据已归档，专用测试容器已清理。尚未取得物理麒麟实机验收；全量检查的既有失败保留如下。**

本轮修复覆盖打包后的输入法连接、中文组合会话、富文本稳定性和发送后的正文收敛。使用同仓库隔离 worktree，根基线 `d4a8393f12a6d526a4d08693ac76e73e87414b14`、Work 基线 `9aad91f1e934c731e540cf99be25733e25c3a411`，未带入其他线程的 Rust 修改、用户配置、密钥或会话。R4 已冻结 33 个源码/测试文件；原始与隔离树一致，R3 的 31 个文件未变，仅新增 R13 两个路径。[源码收据](source-freeze-r4.json)、[构建后独立复核](SOURCE-R4-REVIEW.md)

可安装产物：[v0.1.2 麒麟 x86_64 onefile](../../../../target/packages/asterdream-0.1.2-kylin-x86_64-20261020-hardening-r4/aster_0.1.2_kylin-v10_x86_64.run)，[完整包校验清单](../../../../target/packages/asterdream-0.1.2-kylin-x86_64-20261020-hardening-r4/SHA256SUMS)。最终 GUI 共 3 次运行、18 张选定截图、8 次实际流式发送；root 独立目检其中 5 张控制及终态截图。

## 已确认修复

| 范围 | 根因修复与可复核结果 |
| --- | --- |
| R1、R4：输入入口与全局快捷键 | 权限备注、追问、变量编辑和 QuickPanel 同时识别组合会话、原生组合标志和 229；选字或取消不误触发批准、发送、退出或关闭。15 个新增用例在旧代码失败，相关五文件修复后 168 项通过。[前端证据](frontend-a-validation.json) |
| R2、R3：变量组合生命周期 | 显式空值保留取消语义；自定义 compositionend 继续交给 ProseMirror 清理。真实 Tiptap/ProseMirror 五项回归覆盖提交、取消、缺省结束数据与长度拒绝，不手动改写 composing 状态。[日志](composer-ime-final.log) |
| R5、R9：实际图形后端与产品总线 | 小型 GDK 探针读取实际 display 类型；产品入口固定到当前会话过滤总线，拒绝继承地址把客户端引向另一条总线。Linux launcher 12/12，macOS 10 通过、2 项环境跳过；实际 runtime 验证有序 backend 列表回退到 X11。[后端证据](BACKEND-PROBE.md) |
| R6：离线安装磁盘检查 | 分别检查解包盘和实际 Flatpak 用户数据盘；支持既有 FLATPAK_USER_DIR、XDG_DATA_HOME、未创建目录与分盘，无容量信息时拒绝安装。七项隔离测试及安装器自检通过。[安装器证据](INSTALLER.md) |
| R7：原生门禁隔离 | 被测 GTK 连接过滤总线，控制辅助程序单独连接私有控制总线；清理并主动注入地址污染。已纠正“旧精确权限必定输入失败”的错误解释：Flatpak 隐式 portal 规则可使该路径成功，控制通道失败不能冒充 GTK 失败。[原生复核](FINAL-NATIVE-REVIEW.md) |
| R8：旧 portal 协议 | 新路径仅遇明确 UnknownObject/UnknownMethod 才重试一次旧路径；拒绝、取消、NoReply 等不回退。真实客户端八项协议回归通过，并验证 FocusIn 和中文 CommitString；cancelled 用例是服务端编码错误，不是 GCancellable.cancel 的真实生命周期。[协议收据](fcitx-protocol-validation.json) |
| R10：桌面图标闭包 | 缺 ImageMagick 时复用现有 GTK 缩略图工具；无图标拒绝打包。安装后的 PNG 为 128×128，入口、图标与 stage 的内容身份通过 R4 正式核验。[产物核验](artifact-r4-proof.json) |
| R11：富文本首页渲染循环 | 稳定无模型分支的静态扩展名数组，关闭附件反复注册导致的 React #185。真实 provider/附件运行时/Tiptap 生命周期回归在旧源码复现同栈错误，修复后通过；R2 实际 rich 首页也已保持稳定。[根因与回归](COMPOSER-LIFECYCLE.md) |
| R12：首次普通键被误标 repeat | Fcitx 4 配方恢复上游异步前置 snooper。旧 OFF 模块在真实 GTK 回归中 a/b/c 各投递两次并失败，新 ON 模块在宿主/映射显示均各一次。保留前端长按防重复发送，不启用同步 DBus 作为产品默认。[原生修复](SNOOPER-R12.md) |
| R13：快速完成后正文留空 | 乐观 ack 写入会使在途终态 SWR 刷新作废；写入后启动非阻塞重验证，重复 ack 同样收敛。增加已提交会话归属保护，防旧回调把 A 行写入 B，又不拒绝悬挂切换时仍可见的 A。八项真实 SWR 回归通过，后台读失败不反转已接受发送。[修复证据](FAST-COMPLETION-HANDOFF.md)、[独立复核](R13-INDEPENDENT-REVIEW.md) |

## 回归与既有门禁边界

下列前端结果分别描述各自范围，**384、33、58 不相加为“独立测试总数”**：

- R1–R4：八个相关文件 **384/384** 通过。[日志](frontend-regression-final.log)
- R11：五文件 **33/33** 通过，包含前述 IME 用例。[日志](composer-lifecycle-regression.log)
- R13：新八项 **8/8**；相邻五文件 **58/58**。最终相同测试对保留旧 hook 为 4 失败、4 通过，旧源码和失败日志保留。[说明及命令](FAST-COMPLETION-HANDOFF.md)
- 相应 scoped Oxlint、ESLint、Oxfmt、diff 检查通过；已记录的隔离 Rust 格式检查通过。没有运行会写全树的 lint/format 去改其他线程文件。
- **完整 Web TypeScript 仍失败于既有 278 条诊断**；按文件/错误码与基线一致，R13 两文件零诊断，新增/减少均为 0。此前本轮新增测试的类型诊断已修复，不称全量类型检查通过。[对照](typecheck-web-r13-comparison.json)
- **workspace_dependencies.py 为 4 通过、2 既有失败**：嵌套 Work Git 布局与“.git 不存在”断言冲突；基线 a3s-gateway 的 publish 为 None。未放宽断言或修改这些基线条件。[完整日志](workspace-dependencies-isolated.log)

## 已执行的 R3 原生交付证据

R3 是包含 R12 的**原生候选**，不包含 R13，不是最终 R4：应用 commit 为 `9976324975943ee39ae5727d6c6f935b2d50e98bc1954992d7e9deda09124776`，app bundle SHA256 为 `5c80ccfed10fa6f3ec5fbaa140d61f2693daebee8cb754d1cc2776e67eb8d031`。已核对 stage/实际安装的 23 个关键文件、metadata 和 168 个 ship 校验项目；R3 没有最终 onefile 或完整 GUI 验收结论。[身份收据](native-r3-identity-proof.json)

- 实际安装路径通过八协议及宿主/映射两条 GTK 路径，a/b/c 均只投递一次。
- 真正调用随包 **Flatpak 1.6.5、bubblewrap 0.4.0、xdg-dbus-proxy 0.1.2**，宿主 `:120` 映射到客户端 `:99.0`。现代 daemon 走新工厂一次/按键 38 次；旧协议走新工厂失败一次、旧工厂成功一次/按键 38 次。中文、混输和 Esc 均通过。[运行身份、命令和边界](flatpak16-r3-proof.json)
- 禁用 portal 的对照明确停在 GTK phase 0；另两个独立 stage 副本分别只删除模块或移除产品总线固定块，均先通过八协议，再按预期在 GTK phase 0 退出 1。全 manifest、模式/链接与独立 inode 证明每个副本只有一项破坏；不是把控制失败、外围超时或任意失败当作正确负向。[负向审查](FINAL-R3-NATIVE-NEGATIVES.md)

R4 已重新执行正向原生测试及 portal 禁用对照。R3 的两个单点破坏负向未在 R4 重跑；复用范围仅为完全相同的 22 个非 host 关键文件、metadata、固定 runtime 和四项维护探针。主程序因 R13 重建，另行记录身份。[复用绑定](native-r4-r3-negative-reuse-identity.json)、[独立审查](NATIVE-R4-REUSE-REVIEW.md)

## 最终 R4 交付验收

| 交付项 | 当前状态 |
| --- | --- |
| 同批构建 renderer、Office、ACP、Office CLI 和主程序的最终退出码及源码后验核对 | **通过**：同批构建退出 0，33 文件构建后复核一致，生成 capability env 为零对；主程序 BuildID `5734771caf4692dd74c1d52675531dbb67dce009`。[构建日志](build-host-r4.log)、[环境收据](source-env-r4-postbuild-proof.json) |
| 冻结目录、实际安装 app commit、bundle/run/tar 大小与 SHA256、全量 SHA256SUMS | **通过**：目录 `target/packages/asterdream-0.1.2-kylin-x86_64-20261020-hardening-r4/`；实际 app commit `9ccb0ea27ee2a19dd63506f3d5ed79be7a88856869b74a7fe74231d2378410af`，bundle SHA256 `62840dfddea793ae675627be7247307cf46b2fdcb92e3c55705cbdb16918f4e1`。run 572006174 字节，SHA256 `f5afb90acbea7d624a46fefd02a92e8831d8516b7ae12612b7a8a96f5fcdbc06`；tar 572005288 字节，SHA256 `6b9445098f3fe219e7c471f9eec243a16d17af81de895f4025f2a5ced2f9c28b`。[身份](package-r4-identity.json) |
| stage/installed ELF、GTK 缓存绝对路径及实际 module ID、入口/精确权限/图标、GPL 源码/补丁、tar/gzip 与 onefile 载荷独立核验 | **通过**：23 项 stage/installed 内容一致，10 项 ELF 为 x86_64，128×128 图标、准确缓存 ID/绝对路径、许可证/补丁/配方、168 项 ship manifest、169 项归档内容、gzip CRC 和 onefile 载荷已核对。data 文件安装后可由 0666 收紧为 0644，不声称模式全部相等。[核验](artifact-r4-proof.json)、[独立交付审查](ARTIFACT-R4-INDEPENDENT-REVIEW.md) |
| R4 与已验证 R3 原生文件/metadata/探针身份绑定，或不等时重新执行 | **通过（明确范围）**：安装路径八协议、两显示 GTK；实际随包 Flatpak 1.6 新/旧协议、中文/混输/Esc 和单次键投递通过。禁用 portal 在 GTK phase 0 预期失败；原始 seccomp 被 Rosetta 拒绝发生在 GTK 前，未算 IME 失败。两个单点破坏沿用已绑定的 R3 证据。[R4 原生收据](flatpak16-r4-proof.json) |
| 实际 rich WebKit：两协议的默认异步、普通 Enter、中文 Space、Esc 恢复、候选 Return 与后续发送；现代 GUI 的英文混输和真实长按；旧 GUI 的快速二次 Enter | **通过（按实际矩阵）**：现代 `run-7fnFml` 与旧协议 `run-V6V5Mp` 均保持真实 rich、无 React #185；Space 提交、Esc 取消/恢复、候选 Return 不误发及保持 IME 开启后发送均通过。现代另测混输 abc 和 1000ms 长按，仅一次发送；旧协议另测 370ms 快速二次 Enter，仅第二键后产生一次请求。旧 GUI 不声称验证了英文混输，旧协议的 mixed 验证在原生门禁。[最终 GUI 报告](GUI-R4-REVIEW.md)、[结构化结果](gui-r4-observations/result.json)、[GUI 身份绑定](gui-r4-observations/identity.json) |
| 保持瞬时 SSE 的本地 synthetic fixture：首次创建会话后用户消息、清空草稿和完整助手正文均可见；同时保留无 observer 注入的确认 | **通过**：瞬时 SSE 未加延迟，用户消息、清空草稿和完整 `OFFLINE_IME_FIXTURE_OK` 正文可见；现代 `run-Wfa1CS` 不加载观察器，中文、候选 Return 不误发/保持开启后发送、两份完整回复均通过。[最终 GUI 报告](GUI-R4-REVIEW.md)、[结构化结果](gui-r4-observations/result.json)、[GUI 身份绑定](gui-r4-observations/identity.json) |
| 本轮拥有的应用/daemon/Xvfb/namespace/容器及临时输出的最终处理与清理收据；保留证据和历史候选 | **完成**：三个 GUI 用例正常停止，进程及各自 scope 命令均已消失；根代理核对完整 ID 后仅停止并删除本轮专用容器。保留冻结包、历史记录、同仓隔离 worktree 及 synthetic 复现资料。[GUI 清理](gui-r4-observations/cleanup.json)、[最终容器清理](cleanup-r4.json) |
| 最终提交/推送/发布的实际操作记录 | 本轮未提交、合并、推送或发布；保留可审查源码、安装包及证据。 |

## 证据不能覆盖的范围

- **未取得物理麒麟发行版实机验收**：候选窗定位、真实键盘硬件、跨窗口焦点、系统原装 daemon、原生 Wayland、ARM 和更新后首次启动均不能从容器结果推定。旧协议最低测试使用官方 4.2.9.6 源归档（内部仍报告 4.2.9.5），按归档 SHA256 标识；不是麒麟发行版原装 binary，也不代表所有旧版本或搜狗专有组件。
- GUI 实验是隔离 Xvfb/XTEST ASCII 按键与人工截图复核，未使用剪贴板注入中文。A3S Test 的 Linux GUI profile 当前 unsupported，不能称 CUA/A3S 认证通过；AT-SPI 未提供可用 WebKit 子树。既有 R2 的 env A/B 支持 R12 因果，但不替代最终 R4 默认包验收。[GUI 方法与已知失败边界](GUI-R2-REVIEW.md)
- Rosetta 拒绝原始 `PR_SET_SECCOMP`。仅测试 wrapper 删除精确 `--seccomp FD` 参数对，并断言其余语义参数一致；完整 WebKit 实验还需测试专用 sandbox-disable 环境。wrapper/覆盖不随产品交付；旧执行文件配合现代 Ubuntu 依赖的结果，**不证明麒麟原机或沙箱安全**。
- 本地模型 fixture 只使用无效 synthetic 身份/密钥和 loopback，不验证真实登录、外部模型或完整业务任务。R13 复现与旧截图一致，但旧 run 未记录内存 mutation 顺序，不能称唯一解释所有空白截图。

最终归纳前的 `OUTCOME.md` 和 `PLAN.md` 原文已分别完整保存在同目录的 `OUTCOME-PRE-R4-HISTORY.md`、`PLAN-PRE-R4-HISTORY.md`；精确 SHA256 见 `outcome-r4-history-preservation.json`。历史失败及修正记录均保留。
