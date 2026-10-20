# r2 安装包与原生验收证据独立闭合核对

在已声明的原生验收范围内，未发现新的交付缺口。冻结包、已安装输入、原生记录及新源码收据一致指向应用 commit `d588d6d5e186e0b960198af3dd75956ea4cd890367e47d4dcccf519b5a369bc8`。本次只读核对，不运行容器或 GUI。

## 身份和载荷

- 独立重算当前 stage 的 23 项关键文件，全部匹配 [r2 静态产物收据](artifact-r2-proof.json)。与前轮比较，只有包含 renderer 的 `bin/aster-dream` 改变；22 项原生输入保持相同，包含 launcher、GDK 探针、GTK 模块及 client、缓存和对应源码。
- 重算冻结目录主 `SHA256SUMS` 的 2 项，以及 ship `SHA256SUMS` 的 168 项，全部匹配。app bundle SHA-256 为 `9568953bc131317a482d1c3894b4be95d9ba82c5b7fb7d0cb67844a9a6011f9f`，与旧 Flatpak 运行收据一致；ship commit 文件、[包身份](package-r2-identity.json)、[安装后原生运行收据](native-r2-run-receipt.json) 及 [Flatpak 1.6 证明](flatpak16-r2-proof.json) 的 commit 相同。
- 阅读静态 verifier：它比较新 stage 和对应 commit 安装目录的内容及 metadata，校验 10 项 ELF、GTK cache/id、许可证与补丁源码，遍历 tar payload 比较完整 ship，并校验 `.run` 内嵌 gzip 与 tar.gz 完全相同、解包字节与容量头一致。该静态验证已通过；本次不重复冒充安装器执行。
- 构建后源码仍为 24 tracked + 5 untracked，31 项源身份不变；renderer 输出直接观察到模块级静态扩展数组，见 [构建后源码核对](source-freeze-r2-postbuild-review.json) 和 [renderer 输出核对](renderer-r2-static-extensions-review.json)。

## 原生正负向原因

| 条件 | 独立读取日志与 trace 得出的结果 |
| --- | --- |
| 新 commit 安装目录的维护 GTK 门禁 | 8 项协议通过；宿主 DISPLAY 与映射 `:99.0` 均完成中文、混合英文及 Escape 取消 |
| 实际 Flatpak 1.6.5，现代 daemon | host `:120` 的直接名称存在、client `:99` 的名称不存在；GTK 映射 `:99.0` 通过。现代 factory 1 次、旧 factory 0 次、按键调用 38 次 |
| 实际 Flatpak 1.6.5，官方旧源包 daemon | 先访问现代 `/org/freedesktop/portal/inputmethod`，再访问 `/inputmethod`，各 1 次；随后在旧 `/inputcontext/0` 上处理 38 次按键并完成中文提交 |
| 关闭 portal | portal owner 确认为 false，GTK 已进入映射显示，phase 0 超时且没有 `Fcitx control failed`；factory 和按键 trace 均为 0。失败来源是产品无法建立上下文，而非仅因退出码为 1 |
| 不绕过 seccomp 的 stock 执行 | `PR_SET_SECCOMP: Invalid argument`，尚未进入 GTK。这是当前 Rosetta 内核限制，不作为输入法功能负向通过 |

旧协议两 factory 的顺序与全部计数已从原始 `*-portal.log` 独立重算，匹配收据。此前 [缺模块与移除地址固定块负向](FINAL-NATIVE-NEGATIVES.md) 使用的相关源文件、metadata、模块、client、探针和 runtime 均与 r2 相同；其日志哈希也再次核对，保持“协议 8 项通过后 GTK phase 0 按预期失败”的明确原因，不将 r2 新 host 的 GUI 行为混入该结论。

## seccomp 与验收范围

独立读取实验 wrapper 和 13 条原始参数记录：它仅移除 `--seccomp FD` 对，其余展开后的语义参数逐项相等；X socket 保留由 `X120` 绑定到 `X99`。产品 metadata 没有因此改动。此收据不证明当前机器的 stock seccomp 可运行或安全属性。

被测为离线包中实际 Flatpak 1.6.5、bubblewrap 0.4.0 和 proxy 0.1.2 可执行程序，依赖库来自当前 Ubuntu 环境；官方旧 Fcitx 源包构建不代表所有麒麟发行版 fork。GTK 协议门禁不替代完整 WebKit 应用、真实麒麟硬件或原生 Wayland GUI 验收。

本次完整断言、当前 23 文件哈希、trace 重算及引用收据哈希保存于 [独立核对记录](artifact-r2-independent-review.json)。
