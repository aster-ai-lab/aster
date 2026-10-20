# R11 重建后的交付核验

本页只记录构建、安装和原生链路；真实富文本 GUI 的结论单独记录，不从静态检查推定。

## 源码与构建

- 根基线 `d4a8393f12a6d526a4d08693ac76e73e87414b14`，Work 基线 `9aad91f1e934c731e540cf99be25733e25c3a411`。
- [源码冻结](source-freeze-r2.json)：31 个文件；隔离 Work 恰为 24 个 tracked 修改和 5 个 untracked 文件。R11 只增加能力列表修复和真实生命周期回归两个文件，原 29 个文件哈希未变。
- [构建后独立复核](source-freeze-r2-postbuild-review.json) 与 [新 renderer 实现](renderer-r2-static-extensions-review.json) 均通过。
- [同批构建日志](build-host-r2.log) 包含 Office kernel/JS、renderer、ACP、Office CLI 和主程序；退出 0。未带入其他线程的 Rust 修改。
- [构建环境核验](bundled-env-r2-proof.json)：三份生成的 capability env 均精确为空；`.env` 只是 0 字节挂载占位，没有复制用户配置或会话。

## 原生与随包运行时

- [打包门禁](package-r2.log) 和 [已安装路径门禁](native-installed-r2.log) 均通过：8 项真实客户端协议用例，以及宿主显示和映射 `:99.0` 的中文提交、混输、Esc 取消。
- [随包 Flatpak 1.6 收据](flatpak16-r2-proof.json) 绑定同一新应用 commit：现代 daemon 为新工厂 1 次、按键 38 次；官方 4.2.9.6 源包 daemon 为新工厂失败 1 次、旧工厂成功 1 次、按键 38 次。两者均由宿主 `:120` 映射至客户端 `:99.0`，中文、混输、Esc 通过。
- portal-disabled 对照的失败原因是 GTK phase 0 超时，不是编译或控制辅助程序失败；原始 seccomp 对照明确由 Rosetta 的 `PR_SET_SECCOMP` 拒绝而失败。见 [运行命令及退出码](native-r2-run-receipt.json)、[portal 负向日志](flatpak16-r2-portal-disabled.log) 和 [seccomp 日志](flatpak16-r2-stock.log)。
- 测试包装器只删 `--seccomp FD` 参数对，逐项断言其余语义参数不变。此包装器不随产品交付，结果不证明 seccomp 安全执行。
- [原生身份对照](artifact-r2-native-identity.json)：23 个关键文件中只有内嵌 renderer 的主程序变化；其余 22 个文件和 session bus policy 与此前独立负向测试一致。没有重复声称在新包上重新执行了“删模块”和“删地址固定块”实验。
- 最低验收基线按官方 4.2.9.6 源包 SHA256 标识；该归档内部仍报告 4.2.9.5。它不是麒麟发行版原装 binary，也不代表所有更旧版本、发行版分支或搜狗专有组件均通过。

## 冻结产物

目录：`target/packages/asterdream-0.1.2-kylin-x86_64-20261020-hardening-r2/`。

- 应用 commit：`d588d6d5e186e0b960198af3dd75956ea4cd890367e47d4dcccf519b5a369bc8`。
- 单文件 `.run`：571,991,949 字节，SHA256 `d375b5ad38d265bf48be4486095d4ce46dcead37a68410d31b354c32623ef8a6`。
- `.tar.gz`：571,991,063 字节，SHA256 `93b3ffa8a5936ec4bc5bd4fc636c660a0c3e79a1b57d5dddcd6014155f1b6f7f`。

[正式静态核验](artifact-r2-proof.json) 通过：23 个 stage/installed 关键文件、10 个 x86_64 ELF、GTK 缓存实际 ID/路径、入口/权限/图标、GPL 完整源码与补丁、168 个 ship 清单项、169 个 tar 文件、gzip CRC、单文件载荷与 tar 逐字节一致、全部校验和。身份另见 [安装包收据](package-r2-identity.json)。

此前 `hardening/` 的已知 GUI 失败包和更早目录均保留，没有覆盖；没有提交、推送或发布。
