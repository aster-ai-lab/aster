# Aster 沙箱边界

Aster harness **不再**内嵌 MicroVM / OCI companions。进程级命令隔离由 **ksandbox** 提供；MicroVM 隔离仍属外部 **kbox**，二者正交。

| 组件 | 现状 |
| --- | --- |
| Core `BashSandbox` 默认 | `NativeBashSandbox` → 锁定的 `kense-labs/ksandbox` git 依赖（Seatbelt / namespaces / AppContainer） |
| `kbox` / 树内 `crates/box` | 已从 Aster 根工作区移除；MicroVM 隔离由外部 kbox 负责，**不是** Bash 默认路径 |
| `aster-confine` | 保留 API 兼容桩（no-op）；不再做内核强制隔离 |
| macOS OCI companions（`sandbox/` 进 CLI 归档） | 已从 `aster-cli` 发行物移除 |
| process-host / yolo | 显式 opt-in：ksandbox 无法初始化时的外层容器回退，或全权宿主执行 |

相关：`../kense/ksandbox`（进程沙箱）、`../kense/kbox`（MicroVM，可选）。
