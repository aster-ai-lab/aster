# R4 最终交付独立只读审查

静态交付和原生身份审查通过，未发现新的确定交付缺口。canonical ship 与已安装 commit 为 `9ccb0ea27ee2a19dd63506f3d5ed79be7a88856869b74a7fe74231d2378410af`。[完整独立断言收据](artifact-r4-independent-review.json)

- 两树 33 项源码哈希、隔离 Git 允许清单及 tracked patch `b62d9db8…` 仍匹配冻结收据，未混入共享树的其他 ACP/turns 改动。
- 外层 2 项与 ship 168 项 checksum 全部重新计算通过。tar 的全部 172 项（169 个 regular file、3 个目录）与 ship 路径、内容和模式一致，无链接、重复或越界成员。
- onefile 安装头精确匹配冻结配方；载荷偏移 886 字节，载荷与外层 tar 完全一致。实际解压 572,182,528 字节，容量字段按实际字节数与 16 MiB 余量计算。没有执行安装器。
- app bundle SHA256 为 `62840dfddea793ae675627be7247307cf46b2fdcb92e3c55705cbdb16918f4e1`；tar 为 `6b9445098f3fe219e7c471f9eec243a16d17af81de895f4025f2a5ced2f9c28b`；onefile 为 `f5afb90acbea7d624a46fefd02a92e8831d8516b7ae12612b7a8a96f5fcdbc06`。
- 23 项 stage/installed 内容身份匹配，10 个 ELF 均为 x86-64；主程序实际 GNU BuildID 为 `5734771caf4692dd74c1d52675531dbb67dce009`。R13 两个实际 renderer chunk 的后台刷新和提交保护身份仍匹配构建后证据。

除新主程序外的 22 项关键文件、完整 metadata/policy、维护 harness、GTK/C 协议及 backend 探针、固定 GNOME runtime 与 R3 一致，因此把 R3 的缺失模块和去除 bus pin 两项负向结果绑定复用于 R4。**没有重跑 R4 的这两项故障注入。** 四个数据文件的安装权限由 `0666` 收紧为 `0644`，内容不变；未声称全部 stage/installed 模式相同。[原生身份与负向复用审查](NATIVE-R4-REUSE-REVIEW.md)

只读核对的实际 Flatpak 1.6 trace 保留现代 factory 一次，以及旧协议依次请求现代/旧 factory 各一次；各有 38 次按键调用。13 条 wrapper 记录中 6 条映射 X socket，均只移除 seccomp FD 参数，其他语义参数不变。stock 条件仍在 GTK 启动前由 `PR_SET_SECCOMP: Invalid argument` 拒绝，不能据正向 wrapper 结果声称 stock seccomp 已兼容。[真实原生运行证据](flatpak16-r4-proof.json)

未决项是独立的最终完整 GUI 验收，不能由本静态/GTK 审查替代。此次没有运行、安装或停止应用，没有执行构建、打包或安装器；ELF 头也不能证明最低 CPU 指令集或真实麒麟硬件覆盖。
