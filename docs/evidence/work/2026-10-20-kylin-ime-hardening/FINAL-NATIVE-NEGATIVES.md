# 最终 stage 的两项负向控制

已在当前最终 stage 的独立 APFS 克隆上分别单跑维护中的 `test-input-method.sh`。两项均按预期失败，证明协议测试通过不能掩盖 GTK 模块缺失或产品错误总线连接。未修改产品源码、隔离构建 stage 或正在导出的安装包。

| 单项注入 | 协议 fixture | GTK 结果 | 整体退出码 | 运行时间 |
| --- | --- | --- | --- | --- |
| 仅删除 `files/lib/im/fcitx4/im-fcitx.so` | 8/8 通过 | phase 0 超时，另明确报告模块文件缺失 | 1 | 18.05 秒 |
| 仅删除 launcher 的完整 `FCITX_DBUS_ADDRESS` 清理与 pin 块 | 8/8 通过 | phase 0 超时，未提交中文 | 1 | 17.85 秒 |

完整、有限输出分别保存在 [缺失模块日志](native-final-missing-module.log) 和 [未固定产品总线日志](native-final-unpinned-bus.log)；[结果断言](native-final-negative-results.json) 记录退出码、协议数量、无协议失败、指定 GTK 失败阶段及无 GTK 通过。每次维护 harness 外围另设 60 秒上限。

源 stage 为 `target/worktrees/kylin-ime-hardening-20261020/apps/work/dist/kylin/flatpak-build`。fixture 保存在 `target/evaluations/kylin-ime-hardening/final-negatives/fixture/{missing-module,unpinned-bus}`。全量比较 3,582 个源路径的文件内容、模式、目录模式和链接目标，每个 fixture 均只有表中一个路径不同；前者缺少一个文件，后者仅移除明确的五行块。运行前后原 stage 与两个 fixture 的完整 manifest 一致，详见 [源身份及唯一差异](native-final-negative-fixtures.json)。完整源 manifest 留在 `target/evaluations/kylin-ime-hardening/final-negatives/source-stage-manifest.json`。

未改变的真实 client、GTK/C 协议探针源码、启动器后端探针、metadata 及维护 harness 的 SHA-256 也保存在身份记录中。未删除模块时的原始模块 SHA-256 为 `0a9356681e4a42b611f5a3027451b4ae8e95d5c36fe5a7906e23db60b83d7c10`；未删除 pin 块时的原始 launcher SHA-256 为 `6a6f459d0b7561faf7de2b400012e10f5e4d34342861f25ac896f052f37a9f84`，仅移除块后的值为 `8c923b352d7c20d02d49b0836ab9dafbfe705f00819bb2015ede4d66742adcf5`。

两项均保留维护 harness 注入的存活空总线 B，以及控制辅助程序同时使用私有控制总线 A 的两个地址覆盖。日志没有 `Fcitx control failed`；失败来自 GTK 未形成正常输入上下文，未把控制程序连错总线当作产品负向证据。

此为维护 GTK 门禁的单项破坏验证。harness 在第一种宿主 DISPLAY 的 phase 0 失败后即退出，因此本次负向运行未继续映射 `:99.0`；不据此宣称负向映射显示、完整 WebKit GUI 或真实麒麟硬件验收。正常最终 stage 的两种 DISPLAY 正向结果见 [包门禁记录](package-final.log)。
