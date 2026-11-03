# Aster 无用功能清理收尾

后续复核纠正：隐藏 OpenClaw/DeepSeek 入口超出了完整替代目标，现已撤回；原清理切片的 325 项通过不代表整体功能正常。具体回退、提交恢复范围及修复见[清理回退核查](../2026-11-03-cleanup-regression-repair/OUTCOME.md)。下文保留当时执行记录。

本轮延续[已完成的清理](../2026-11-03-product-cleanup/OUTCOME.md)，关闭已确认失效的产品入口，删除孤立代码，没有扩展新的第三方管理功能。

- Hermes Dashboard 的控制器、IPC 声明、专用测试、缓存键、文案、状态锁和刷新入口已清除。Hermes 保留原 Code CLI 配置及 LaunchDialog。
- Aster 不再列出 OpenClaw、DeepSeek Harness 的专属托管入口。这两组操作在当前本机 bridge 中没有处理器；工具列表和侧栏快捷入口共用原列表，旧深链在加载页面前返回工具首页。共享宿主的其他租户条目保留，用户保存的配置和快捷方式数据不删除。没有声称其他租户的旧托管操作已经可用。
- Aster 设置导航和搜索不再提供旧设备配对，旧链接返回通用设置；ToYou 路由保留。
- 删除没有产品调用方的 `providerStoreRepair.ts` 及其测试，共 77 行。连同前一轮 Dashboard 文件，本批共删除五个文件。
- 网关继续链接原模型服务、路由、预算和用量页；原设置外壳保留唯一页面标题。核心接线的恢复单列于[接线记录](../../local-gateway/2026-11-03-core-reconnect/OUTCOME.md)，不把它计为清理了新的产品能力。

## 验证

当前检查使用仓库已有工具，未安装依赖、提交、安装或重启桌面应用。

| 检查 | 结果 |
| --- | --- |
| `vitest run --project renderer --project shared`，Code、启动/配置、设置、网关、产品边界等 17 文件 | 210 通过 |
| `vitest run --project renderer src/renderer/shell/studioMainBridge.test.ts` | 48 通过 |
| `vitest run --project renderer`，sidebarShortcuts 的 providers/runtime 两文件 | 67 通过 |
| 上述不同测试合计 | 325 通过，20 文件 |
| `ASTER_TENANT_SLUG=aster/toyou STUDIO_RENDERER_DIST=product-cleanup-final-<tenant> rsbuild build`，串行构建 | 两者通过 |
| 定向 Oxfmt、ESLint、Oxlint | 通过；ESLint 既有文案模板、跨页导入及测试 mock 按钮类型警告另保留 |
| i18n 同步/检查 | 25,538 条翻译通过 |
| `generate-preferences.js --check-native` | 通过 |
| 定向 `git diff --check`，Dashboard/孤立辅助代码引用检查 | 通过，无残留引用 |
| Web TypeScript 全量检查 | 未通过：355 条诊断，本批及核心接线路径中为零 |

新增入口回归的首次运行有三项失败：两个 Aster 列表用例复现旧入口仍存在；ToYou 允许用例因尚无 `beforeLoad` 函数而报测试调用错误，不能把它计为第三项产品缺陷。最终三项均通过。

扩大验证曾遇到客户端并发编辑测试的焦点竞态：单独执行通过，合并执行在输入前被侧栏的 `requestAnimationFrame` 自动聚焦打断。测试改为等待真实侧栏完成聚焦，再输入和提交；零值拒绝、scope 和持久化断言保留。最后 210 项重新全部通过。

私有前像、差异和日志保存在 `apps/studio/.studio/cache/product-cleanup-final`，原清理证据保留。父仓仅记录命令、结果与源码指纹。

## Explicitly not claimed

未宣称全部 Magpie 能力已替代或全仓检查全部通过。未启动实际桌面窗口、安装包或真实第三方运行时；Windows/Linux 原生行为未验证。其他租户仍使用的实现、Office/浏览器/知识能力和用户数据不在删除范围。
