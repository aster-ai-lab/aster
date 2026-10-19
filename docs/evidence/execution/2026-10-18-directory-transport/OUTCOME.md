# 目录传输源码同步

已同步上游签名提交 86e6cd2a21025875ba0903d7be74da58c7a66224 的相关执行库修改，
包括 Aster 已有独立 SDK 副本。已有 Cargo 包元数据和本地依赖路径保留。
迁移前逐文件与上游历史比较，未覆盖不属于该历史的自定义源码。
`migration.json` 记录每个同步文件的 SHA-256。

这是执行库源码同步；默认 Bash 仍使用原有路由，没有接入新的公共 Host Service。
凭据策略、资源分发、网络、工具链及产品端到端仍待完成，不能据此宣称用户问题已修复。
上游诊断证据位于 `crates/oci-runtime/docs/evidence/2026-10-18-directory-transport/`。

## 本地验证

上游签名源码 86e6cd2a2 的干净候选通过真实目录运行及 25/25 轮公共生命周期、
宿主强杀恢复、重放和清理验收。签名证据提交为 2af8f8147，报告已同步到上游证据目录。
359 项相关单元/文件系统检查通过，Linux 测试目标交叉编译通过，未执行 Linux 硬件验收。

本地首次 SDK 测试编译失败：另存于 crates/oci-sdk 的副本破坏了测试资源相对路径，
且 Aster 根工作区遗漏了上游现有的 oci-spec HugeTLB uint64 补丁。当前依赖已改为
原始布局中的 crates/oci-runtime/crates/sdk，并在根工作区应用相同补丁。
保留原有依赖版本；锁文件只将该 oci-spec 0.10.0 条目从 registry 切换到本地补丁。
SDK 和 core 标记为 publish=false。原始独立副本保留，未删除其他迁移中的文件。

独立构建目录中，Aster 根工作区 SDK 198 项测试通过。metadata --offline --locked
确认当前只有一个活动 SDK，路径正确且不可发布。默认 Python 3.9 缺少 tomllib，
依赖规则检查改用已安装的 Python 3.13；该检查仍被已有的 a3s-cloud-contracts
未设置 publish=false 挡住，原始输出见 dependency-gate.txt。未更改 Cloud 包。

Aster 工作区包含其他迁移改动，本批未做整体提交、发布或整库格式化。
没有执行 AsterDream 默认 Bash 和界面端到端验证；用户问题仍未完成修复。
