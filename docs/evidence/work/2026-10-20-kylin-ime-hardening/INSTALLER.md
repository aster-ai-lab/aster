# 麒麟离线安装器磁盘检查修复

## 结果

`install-on-kylin.sh` 现在检查 Flatpak 实际用户安装目录所在的文件系统，依次采用既有 `FLATPAK_USER_DIR`、`XDG_DATA_HOME/flatpak` 和默认用户数据目录。目录尚未创建时，检查最近的已存在父目录。无法读取容量时明确拒绝安装，不跳过检查。

单文件包装器在构建时流式测量解压后的 tar 大小，把所需 KiB 和 16 MiB 余量写入固定宽度安装头。数字范围受安装头容量约束。运行时先检查 `TMPDIR` 所在的解压文件系统，空间不足时不调用 tar 或安装器，并提示清理或设置 `TMPDIR`。用户数据盘和临时盘分别检查。

保留原来的非 root 应用安装、单独 sudo 系统组件引导、SHA256、精确 `.commit` 更新和离线安装流程。未添加运行时依赖或新的配置选项。

## 验证

- `python3 tests/kylin_onefile.py`：7/7 通过，见 [最终日志](installer-after.log)。覆盖临时盘不足 3GB 而安装盘充足、安装盘不足但临时盘充足、未创建的数据目录、`FLATPAK_USER_DIR` 优先级、无法读取容量，以及单文件解压前拒绝和原有损坏载荷拒绝。
- 原有 Work `9aad91f` 的五个不同合同失败均单独复现，见 [逐项修复前日志](installer-before-isolated.log)；初始 4 项失败见 [初始日志](installer-before.log)。测试使用临时 XDG 目录、真实校验文件和替代系统命令，不安装软件，不修改真实用户的 HOME 或输入法状态。
- `bash apps/work/scripts/kylin-flatpak/install-on-kylin.sh --self-test`：通过，见 [日志](installer-selftest.log)。
- `bash -n apps/work/scripts/kylin-flatpak/install-on-kylin.sh apps/work/scripts/kylin-flatpak/pack-onefile.sh`、`python3 -m py_compile tests/kylin_onefile.py` 以及 scoped `git diff --check`：通过。

## Explicitly not claimed

- 没有执行麒麟实机的 sudo/apt/Flatpak 安装事务；磁盘布局通过隔离系统命令 fixture 验证。
- 未重新构建或替换之前冻结的 v0.1.2 `.run`。新的空间检查需要重新打包后才会进入安装包。
- 容量预检不保证消除并发写入、文件系统配额或安装期间空间变化造成的错误。
