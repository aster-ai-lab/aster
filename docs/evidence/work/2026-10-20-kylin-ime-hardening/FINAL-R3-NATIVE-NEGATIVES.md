# r3 最终 stage 的两项负向控制

已重新复制 r3 stage 并运行当前维护门禁，没有继承 r2 的负向结果。两个副本均先通过全部 8 个协议场景，再由 GTK phase 0 超时明确拒绝缺失模块或错误产品总线连接。两项都没有 controller failure、daemon readiness failure、中文提交或 GTK PASS。

| 单项破坏 | 协议 | GTK 失败原因 | 退出码 | 整体耗时 |
| --- | --- | --- | --- | --- |
| 仅删除 `files/lib/im/fcitx4/im-fcitx.so` | 精确 8/8 通过 | 模块路径明确报缺失，phase 0 超时 | 1 | 19.53 秒 |
| 仅移除 launcher 的完整 bus 清理与 pin 块 | 精确 8/8 通过 | phase 0 超时，未提交中文 | 1 | 18.89 秒 |

完整、有限输出见 [missing-module 日志](native-final-r3-missing-module.log) 和 [unpinned-bus 日志](native-final-r3-unpinned-bus.log)。每次 harness 外围有 60 秒上限；不是把外围超时或任意退出 1 当作正确负向。日志保持协议名称与顺序、GTK 启动、指定 phase 0 超时及无控制失败的逐项断言。[完整结果与命令](native-final-r3-negative-results.json)

## r3 身份和唯一差异

源为主代理明确确认已定稿的 `target/worktrees/kylin-ime-hardening-20261020/apps/work/dist/kylin/flatpak-build`。在获准后才用宿主 `cp -cRp` 复制整个 stage 至 `target/evaluations/kylin-ime-hardening/final-r3-negatives/fixture/`，随后各注入一项破坏。核对全部 3,598 个路径的内容 SHA256、模式、目录模式和链接目标，每个副本均只有表中一个路径差异。

每个完整副本的 2,972 个 regular inode 均与源独立；删除模块后，两副本之间的 2,971 个共同 regular inode 也独立。测试结束再次核对完整 manifest：源 stage 与各 fixture 均未变化。[身份、唯一差异、原生文件与维护探针哈希](native-final-r3-negative-fixtures.json)

完整源 manifest：`target/evaluations/kylin-ime-hardening/final-r3-negatives/source-stage-manifest.json`，SHA256 为 `7d5578673f7d25ecb436e9a5b517d1125f017779942a174e7772d91dd3529ed8`。其中三个 Fcitx 4 原生文件分别绑定本轮新产物：

- `im-fcitx.so`：`73503354d8594dd03feb4cb27ad7a5e7f17a455eda58e2ea2e3138e8ed2d0734`
- `libfcitx-gclient.so.1`：`7db5cc21ed9c62933e4785b221773027b64d091e40b2d92a9a7d4b268fe091d3`
- `libfcitx-utils.so.0`：`10718ef3263b1b2b386e1fda073578526bbad3466acee01c9ef5ff69b2519cf0`

metadata 和随包 ON 构建配方身份也在收据中。最终绑定至已冻结 canonical ship 与实际安装的 app commit `9976324975943ee39ae5727d6c6f935b2d50e98bc1954992d7e9deda09124776`。绑定时重新核对完整源 stage manifest 未变，23 个关键文件及 metadata 均与 [r3 安装身份核验](native-r3-identity-proof.json) 匹配；canonical app bundle SHA256 为 `5c80ccfed10fa6f3ec5fbaa140d61f2693daebee8cb754d1cc2776e67eb8d031`，与 ship 的 checksum 和身份核验一致。[实际安装运行收据](native-r3-run-receipt.json)

原始负向执行发生在 export 完成前，因此日志头的 `supplied_app_commit: null` 保持原样，日志 SHA256 未变化。两份 JSON 的 `app_commit` 与 `commit_binding` 保存上述后续绑定及核验收据哈希；不使用旧 r2 ship 身份。

编排脚本为 `target/evaluations/kylin-ime-hardening/run-final-r3-negatives.py`。其小型 selftest 校验 manifest 的模式与链接、只移除恰好一个完整 pin 块，以及 controller failure/退出 124 无法误通过。脚本在没有显式 `--go` 时不读取 stage 或容器；运行版本哈希保存在身份收据中。

## 证明范围

两项保留当前维护 harness 注入的存活空总线 B，控制辅助程序仍显式使用私有控制总线 A。没有修改产品源码、最终 stage、安装 bundle，没有运行 `flatpak install/run` 或操作 GUI。

负向在首种宿主 DISPLAY 的 phase 0 退出，未继续映射 display；本轮不能据此宣称负向映射显示、完整 WebKit GUI 或真实麒麟硬件验收。r3 正常 stage 的 8 协议及双 display 每键一次正向由主代理的 `package-r3.log` 单独记录。

r3 保留为原生能力候选，没有生成最终 onefile，也没有完整 GUI 验收结论。R13 的 SWR 完成态刷新竞态正在独立修复；后续 r4 只有在重新核对原生文件、launcher、metadata 与维护探针身份相等后，才可复用这批原生负向证据。
