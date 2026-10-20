# R4 原生身份及 R3 负向复用核对

R4 canonical ship 与已安装 commit 为 `9ccb0ea27ee2a19dd63506f3d5ed79be7a88856869b74a7fe74231d2378410af`。本轮只读检查已安装文件和定稿 stage，满足复用 R3 两项 GTK 原生负向的身份条件，没有重新执行这两项测试。

- 除主程序外的 22 个关键文件 SHA256、大小和 stage 模式与 R3 相同；R4 stage 和已安装 23 项的内容、大小一致。
- 四个数据文件（icon、两个 cache、Fcitx 5 module-id）安装时由 `0666` 收紧为 `0644`，仅移除 group/other 写权限，读取和执行位不变。其他 19 项模式相同；不将 stage 与 installed 的全部模式称为相等。
- 完整 metadata 字节、SHA256 和 Session Bus Policy 与 R3 相同，包含 Fcitx portal 的精确授权。
- GTK 探针、C 协议 fixture、维护 harness 和 GDK backend 探针源码与 R3 身份相同；GNOME runtime 的实际解析路径也与 R3 负向使用的固定 runtime 相同。
- 两份 R3 负向原始日志哈希仍匹配。可复用其“缺失模块”和“去掉产品 bus pin”各只有一处差异、协议先通过后 GTK phase 0 失败、无 controller failure 的结果。

[已安装身份逐项核验](native-r4-r3-negative-reuse-identity.json) 保存全部文件哈希、模式、metadata、policy、探针身份和 runtime 解析路径。[独立复用结论](native-r4-r3-negative-reuse-review.json) 关联 canonical commit 文件、R3 负向收据和原始日志的哈希。

主程序由 R13 前端变化重新构建，单独记录 SHA256 `5ee63ca6bb54fa1d347e45e063b78884adf1a965d65cc20c5d7d9cffe52023d1`；不能据原生身份复用替代新主程序的 GUI 验收。其 renderer 的实际提交保护和后台刷新代码见 [构建输出核验](renderer-r4-history-handoff-review.json)。

同时只读核对既有 R4 六项运行日志：已安装包的 8 个协议及两种 display 均通过，每个英文键各一次；随包 Flatpak 1.6 的现代和旧协议条件均在映射 `:99.0` 下通过；禁用 portal 条件在 phase 0 超时且无控制程序失败；stock 条件在 GTK 启动前因 `PR_SET_SECCOMP: Invalid argument` 失败。现代/旧协议正向采用既有移除 seccomp 参数的测试 wrapper，不能据此宣称当前容器的 stock seccomp 已兼容。

未启动应用、未执行新的测试、未安装、构建、打包或停止进程；仅用只读 `docker exec ... python3` 对已安装文件求哈希。外层 tar/onefile/checksum 已由 [最终静态审查](ARTIFACT-R4-INDEPENDENT-REVIEW.md) 单独闭合；完整 GUI 由独立证据验收。
