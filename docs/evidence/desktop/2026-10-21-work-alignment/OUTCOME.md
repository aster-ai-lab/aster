# Desktop / Work 对齐验收

状态：进行中。本文不是最终通过声明；原生协议审查、真实窗口和安装包验证仍在收尾。

范围见 [spec](../../../../specs/003-desktop-work-alignment/spec.md)、[plan](../../../../specs/003-desktop-work-alignment/plan.md) 和 [tasks](../../../../specs/003-desktop-work-alignment/tasks.md)。未提交、暂存、推送或发布本任务代码。

## 已有证据

| 项目 | 当前结果 | 证据 |
| --- | --- | --- |
| 基线与工作区边界 | Desktop 起点干净；保留原有 Work/Core/CLI/Gateway 等改动 | `BASELINE.json` |
| 输入法、富文本、快速回复 | 初版 13 suites / 429 pass；后续审批能力收口涉及两项重叠文件，不能与 UX 总数相加 | `IME-RENDERER.md`、`ime-source-receipt.json` |
| 规划、目标、进度、审批界面 | 最终 UX 27 files / 749 pass / 1 原有 skip；59 文件收据 | `ux-implementation.md`、`ux-source-ready.json` |
| 前端生产构建 | Office 与 renderer 成功；最终 renderer 8.60s，59 文件收据未改变 | `final-renderer-build-r2.json`、`final-renderer-build-r3.json` |
| 原生正向链 | 真实工作区 ACP 已执行规划拒写、审批错绑拒绝、批准、写入、读回；真实 goal 查询/创建/暂停/取消已联调 | `native-real-acp-protocol-after.log` |
| 原生更新器 | 22 项真实 Desktop crate 测试通过；随后增加闭集解析测试，待最终邻近聚合确认 | `native-updater-real.log` |
| macOS 整包封装工具 | 4 项测试通过，完整前端/租户/ACP/Office 资源封装；实际 release 包待验 | `mac-delivery-tests.log` |
| 工作区原生构建 | 根目录 pinned `just build-desktop` 成功；之后仅 Desktop 协议/身份补丁需要最终重编 | `final-host-build-r2.json`、`final-host-build-r3.json` |
| release ACP + Office | 根工作区独立输出构建成功，425.84s | `release-companions-build.json` |
| Linux 输入链 | 维护测试、真实 GTK/输入模块/旧新 portal 协议及缺资源拒绝已验；实际 Desktop host 正在隔离构建 | `IME-KYLIN-PACKAGING.md`、`IME-LINUX-PREPARATION.md` |
| 当前 Linux 原生 Office | x86_64 release、动态库解析与版本运行成功 | `ime-linux-office-artifact.json` |

本机 Office 一次构建失败来自两条依赖链接指向未打补丁的 Bun 包。按现有 pnpm 锁图原子重接，未更改源码或锁文件，随后完整构建通过。见 `ux-office-dependency-diagnosis.md`、`office-dependency-relink-applied.json`。

共享 `target/debug/deps` 约 300GB，且另一个构建占锁。未删除或终止他人的工作；将已有精确依赖独立 APFS 克隆到 `target/desktop-macos-alignment`，由 Cargo 自行验证/失效，再执行同一根工作区命令。见 `macos-isolated-target-seed.json`、`shared-build-retry.json`。

## 尚需关闭

- 完整 stdio 协议映射审查提出的请求 session 身份、响应 result/error 严格解包与最终回归。
- 最终原生源码构建、实际 Desktop 窗口中的中文快速回复、规划审阅/修改/执行/清理、审批/停止/重连、目标与模型持久化证据。
- macOS 完整包签名、资源身份与隔离旧版本 fixture 更新；Linux Desktop/ACP/Office 实际三二进制身份。
- 最终格式、相关契约与工作区检查、源码收据、逐项 FR-001–FR-012 审核和进程清理。

## 当前明确不声称

- 不声称全树 TypeScript 通过。受影响 UX 文件同配置比较为 19→18 个既有诊断、无新增诊断。
- 不声称 429 与 749 可相加，也不把 direct rustc 的真实 crate 测试等同于完整 Cargo/package 验收。
- 不把合成模型等同于外部模型质量评测，不复制用户登录状态或凭据。
- 不把临时 debug app 当作 release 包；旧版本更新 fixture 使用原始基线原生源码和合成较低版本，不能冒称已发布历史二进制。
- 不声称实体麒麟或严格 Flatpak export 已通过。当前容器 stock seccomp 返回 `PR_SET_SECCOMP EINVAL`，保留拒绝；没有生产降级绕过。
- 不声称 host 重启后恢复旧 goal。当前合同保证同 host 的重连与可用性信号；无当前 runtime 时返回 unavailable。
- 未改变 Desktop 更新服务渠道，未发布安装包或修改在线更新源。
