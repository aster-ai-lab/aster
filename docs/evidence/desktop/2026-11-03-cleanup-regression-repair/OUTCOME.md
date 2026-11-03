# 清理回退核查与修复

用户指出清理导致功能失效后，暂停进一步删减，对照清理前像、验证指纹、Git reflog 和逐提交源码核查。本轮没有执行整体 checkout/restore，也没有删除用户配置。

## 确认的错误与接线缺口

| 项目 | 事实与影响 | 本轮处理 |
| --- | --- | --- |
| OpenClaw、DeepSeek Harness 入口 | 前一轮主动过滤 Aster 工具列表并重定向旧链接。这是把未接通的托管实现当成可移除功能，超出了完整替代目标 | 撤回过滤和重定向；两租户保留工具、快捷入口和原链接 |
| Code CLI 主进程桥接 | 当前前端已有版本化配置协议和 native writer，但桥接缺少五条 `code_cli` 请求，读取、保存、恢复、启动及终端查询返回 ROUTE_NOT_FOUND | 恢复精确命令表、宿主 token、版本字段与显式模型/路由身份，不使用通配转发 |
| 命名客户端/路由投影 | 连接类型缺失；页面没有按所选客户端投影模型，也未把完整目录和加载状态传给已有编辑控制器；配置识别使用默认 key | 补回共享类型及调用参数；复用既有目录过滤和配置识别，不建立新编辑器 |
| 路由保存后的目录 | Preference 写入后没有刷新 gateway snapshot，已挂载的 Code 页面可能继续使用旧目录 | 成功保存路由后查询同一 native status；查询失败清除旧快照 |
| CLI 权限 | 之前把 CLI 命令加入带 window 范围的通用 capability，实际授权测试证明浏览器子视图也继承了该许可 | 复用已有 runtime-components 的声明方式，CLI 仅授予 main webview；宿主认证保持 |
| 原生测试构建 | 插件 authority 测试调用了私有的兄弟测试 fixture，造成三个编译错误。这是测试构建缺口，不等于生产插件实现被删除 | 只调整 cfg(test) fixture 的 crate 可见性，生产权限和安装逻辑不变 |

恢复工具入口不代表 OpenClaw/DeepSeek 的旧托管 IPC 已经实现。该缺口仍须继续完成，不能再以隐藏入口代替功能交付。Hermes 使用共享 CLI 启动链；原 Dashboard 并无对应 native 处理器，没有恢复这一旧框架。

## 到底恢复了什么

以下来自当前 Studio 仓库的提交树和 reflog，不根据文件名猜测执行者或脚本。

| 时间 / 提交 | 实际内容 |
| --- | --- |
| 08:44，`91fc5b18b` | 保存合并后的本地 WIP。当时没有 Code CLI native bridge，也没有版本化的共享 CLI schema |
| 08:52，`ef50d5957` | 恢复 341 个路径，其中 56 个与 Code、IPC、语言等相关。包含 JSONC/TOML/YAML/.env 保留式编辑、配置版本和归属恢复、模型别名/路由投影及命名客户端编辑接线；这些是有效能力，本轮保留 |
| 08:56，`9bba90350` | 恢复两个语言文件中的 native menu 启动文案；保留 |
| 09:02，`42d3a42d4` | 重放 42 个 PR 优化路径，包含工作区浏览器、模型选择器、消息和弹层等；保留这些优化。但它把 `studioMainBridge.js` 的 Code CLI 桥接从“存在”变成“不存在”，版本化 schema 则保留，形成了不完整的混合状态 |

`ApiGatewayClientConnection` 和页面客户端投影在所核对的五个提交快照中均缺失，不能把其缺失全部归因于最后一次提交。检查期间未提交的修复也曾回到前像；没有证据确定每次写入者，因此不再把未确认进程当成原因。

前一轮 325 项测试只证明当时清理切片中的行为，不能证明整个 UI → IPC → native 链路正常。先前“清理完成”的结论需要按本记录纠正。

## 验证

原生从根仓、pinned Rust 1.97.1 执行；Darwin arm64，临时 ASTERX_HOME，既有 target/studio/toyou。前端使用已有工具和依赖，不启动或替换用户桌面窗口。

| 检查 | 结果 |
| --- | --- |
| Code、配置保留/恢复、命名客户端、共享 schema、真实 bridge 脚本、入口及侧栏：`vitest run --project renderer --project shared`，66 文件 | 930 通过，检查时修复源码指纹未变 |
| `cargo test --locked -p aster-studio --lib code_cli -- --nocapture` | 59 通过、1 ignored；包括真实 Tauri RPC、文件读写、拒绝错误宿主 token/非所属 target、版本冲突、原格式恢复及启动准备 |
| 原权限失败的独立复现 | 失败：同一 main window 的 browser-child 得到 code_cli 许可；没有修改断言 |
| `cargo clippy --locked -p aster-studio --all-targets --no-deps` | 通过，有范围外既有警告 |
| `cargo check --locked -p aster-studio --lib --no-default-features --features tenant-toyou` | 通过 |
| Aster / ToYou：`ASTER_TENANT_SLUG=<tenant> STUDIO_RENDERER_DIST=cleanup-regression-repair-<tenant> rsbuild build`，串行 | 均通过 |
| 定向 Oxfmt / ESLint / Oxlint；修复 fixture 的 rustfmt；定向 git diff --check | 均通过 |
| 全量 Web TypeScript | 294 条错误，本轮修复路径和 Code 相关路径中为零；未声明全仓通过 |
| `cargo fmt --all -- --check` | 范围外文件失败，修复 fixture 的定向检查通过 |

构建后，另一个变更向两个通用 capability 加入了 `allow-set-main-traffic-lights-rail-width`。本轮保留并单列它，没有将该权限归因为本轮修复。CLI 仍仅来自独立 main-webview capability；随后再执行 CLI 命令/授权的原生测试，实际结果见 verification.json，不将重跑重复计入 59。

私有前像、已验证源码、原始日志、差异、提交矩阵和最终指纹位于 `apps/studio/.studio/cache/cleanup-regression-repair` 与 `code-core-reconnect`。父仓只保存结果与指纹。保留 FirstRunPermissionsDialog、Workbench 等并行改动，未提交或重启应用。

## Explicitly not claimed

没有证明“所有功能已恢复”或 Magpie 完全替代。OpenClaw/DeepSeek 托管入口的原生实现、真实官方 CLI 完整矩阵、当前已运行桌面窗口的实际流程、Windows/Linux 原生运行、性能及全局类型门禁仍未验收。Ignored 不是通过。没有通过隐藏功能缩小原始目标。
