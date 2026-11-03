# 剩余开发与验收路线图

更新：2026-11-03。范围继续使用 mise `v2026.10.25` 的冻结基线：**C01–C24
全部必需子能力，以及原 35 个应用 + 14 项依赖/运行组件，共 49 项迁移**。
新增插件不替代原始项，690 条上游来源记录不作为功能通过数。

## 已落地但尚未整体关闭

已实现租户隔离的插件安装作业持久化、重启核对、精确版本重试、共享安装状态；
工具声明来源跟踪、prune、升级范围/bump/pin/预览/离线与坏包保留原版本。
当前证据包括工具库 150 项、环境命令 Pager 23 项、插件原生 18 项和前端 77 项通过，
所属包全目标 Clippy 与 CLI/ACP 成对构建通过。原 35 个应用完成实际 AIX 构建。

这些结果不关闭完整能力族、49 项消费者迁移或发布验收。T001–T003 保留原状态，
T004–T014 继续未勾选。详见 [实施记录](../../docs/evidence/cli/2026-11-03-convergence/OUTCOME.md)。

## 下一批先处理

1. **恢复完整 CLI 验收。** 最近整套结果为 32 通过、6 超时、1 忽略；六项在相同
   测试程序和 CLI 上按原条件单独复核通过。后续重新编译与测试程序启动均遭遇
   无进展并已停止本任务的阻塞进程。必须获得完整运行结果，不能用单项复核替代。
   六项为：`cli_version_requests_match_frozen_mise_executions_and_lock_without_metadata`、
   `configured_path_and_system_requests_match_frozen_mise_context_results`、
   `exact_uninstall_keeps_other_versions_and_an_unrelated_project_pin`、
   `exec_tool_overrides_use_the_argument_root_and_preserve_configuration`、
   `legacy_v1_literal_requests_execute_the_pinned_tool_and_keep_the_current_pointer`、
   `multi_tool_uninstall_matches_the_five_frozen_native_mise_cases`。
2. **定位真实在线获取超时。** uv `0.13.6` 自动安装在原 120 秒限制内未完成。
   标签元数据与完整 release 列表读取的诊断不能证明完整安装成功，也未确定根因。
   固定版本、输入、超时、断言保持不变；修复后执行实际安装和工具命令。
3. **完成 M1 原生 Studio 链路。** 获取可用的原生 GUI 连接，验证普通插件及内置组件的
   检查、下载、安装、更新、移除、断网/重启恢复、关闭重开进度、明暗主题及键盘操作。
4. **继续 M2 生命周期。** 默认旧版本宽限期清理、升级后的自动锁更新、剩余回滚/
   selector/profile/stub/Windows dispatch 行为仍缺实现或完整验收。

## 按原顺序推进 M0–M8

| 阶段 / 原任务 | 剩余交付 | 代码或合同归属 | 关闭条件 |
| --- | --- | --- | --- |
| M0 / T012–T013 | 展开 24 类全部必需叶能力；49 项领域、许可证、目标、来源、消费者与发布状态逐项对账；归属全局 gate 与原生 runner 前提 | [能力矩阵](contracts/capability-matrix.md)、[状态台账](contracts/convergence-status.json)、[49 项清单](contracts/legacy-plugin-inventory.json) | 每个必需项有明确 owner、固定输入、预期效果/退出、执行命令、证据和阻塞；不改变冻结基线 |
| M1 / T008 | 普通插件更新及权限链路、内置组件中断恢复、原生 GUI 全流程 | `apps/studio` 私有子模块中的 capabilities registry/runtime components 与插件页面；`crates/use/extension` | 实际安装/使用/更新/移除；断网、重启、摘要漂移与缓存不足不误报 ready；租户/工作区数据保留 |
| M2 / T004 | 完整引用/别名、多版本与配置变体、宽限期清理、锁迁移/自动更新、回滚、Windows dispatch、stubs/sync | `crates/use/tool-acquire`、`aster-pager/src/env_manager_cmd` | 固定对照逐项匹配；坏包保留原安装；prune 保留所有实际使用者；preview 零删除；offline 零网络；完整 CLI 套件通过 |
| M3 / T008 | 所有基线语言 runtime、release/language/custom 后端与注册表；源构建、选项、布局、签名与 legacy ABI | `crates/use/tool-acquire`、`crates/use/extension` 与官方 `plugins` 子模块 | 每个适用目标实际获取并执行功能；标准库、包管理器、多入口、companions 完整；名称解析和版本输出不足以关闭 |
| M4 / T005 | profiles/层叠、环境值指令与文件、模板、secrets、alias/hook、virtualenv、动态目录切换 | 环境配置与 shell owner；[原任务](tasks.md) | 实际进入/离开嵌套与兄弟目录并准确恢复；路径含空格/非 ASCII；PATH 不累积、secret 不泄露、不执行不受信脚本 |
| M5 / T006–T007 | named/file tasks、参数/发现、DAG、共享依赖、并发/取消、增量、本地/远程 cache、watch 与 monorepo 图 | 后续任务 owner 及既有进程/缓存适配器 | 文件效果和执行日志证明顺序、去重、并发边界、取消与缓存恢复；循环/坏缓存拒绝；cache hit 不执行任务 |
| M6 / T009–T010 | bootstrap/modules、包/账号/文件/服务/防火墙/Compose/Git/SSH；dotfiles/history/sync/encryption；daemon/readiness/端口/worktree 清理 | 各原生平台/主机状态 owner；不新增第二个 workspace | 隔离目标上实际 plan/apply/status、第二次 no-op、rollback/cleanup；远程用隔离主机，按适用 OS 留证据 |
| M7 / T008 | 原 49 项逐项进入按领域组织的官方 AIX 源；制品、GHCR/TUF、宿主兼容、旧安装接管与消费者迁移 | [49 项清单](contracts/legacy-plugin-inventory.json)、`plugins` 子模块与原始应用/依赖 owner | 原身份与顺序完整保留，原始项精确 49/49；适用 target 有实际制品与消费者证据；内置项隐藏但可管理、required 受保护；不丢配置/租户数据 |
| M8 / T011–T014 | diagnostics/completion/docs/skills/policy、环境 MCP server、release 性能、原生平台、最终 CLI/ACP/桌面归档与发布 | [发布治理](../../docs/release-governance.md)、现有 CI 矩阵与发布流程 | 24 类及全部必需子项、49 项、所有必需 gates 通过；最终归档完成实际安装/执行、成员完整性、签名/公证与 channel 检查后发布 |

## 保留的验收阻塞

- 全局 web 类型检查保留 286 项本批插件页面之外的错误；不扩大 allowlist。
- 本批 Rust 格式/空白检查通过；最后全仓格式检查在并行修改的
  `apps/aster/tests/tui_913_real_llm.rs` 失败。按实际最终源树重跑并归属修复。
- 原生 GUI 连接不可用；Windows 交叉检查缺 Windows CRT/compiler 环境；
  Linux/Windows 及其他架构需要原生 runner。交叉编译或单台 Mac 不作为原生通过。
- release 模式冷/热分布、资源/网络/子进程数量尚未完成；debug 时间不关闭效率门槛。
- 最终 CLI/ACP 必须一起分发。macOS 最终归档需在隔离 prefix 安装后运行
  `tests/cli_sandbox_smoke.py`；桌面最终归档按源合同验证所有必需成员，之后才能生成/
  发布 channel manifest。签名、公证、制品回读、上一版回滚与发布命名规则继续保留。

## 后续交接规则

每次只推进一个可验收切片：真实场景与反例 → 所属库及必要调用者 → 固定测试和真实
执行 → 记录源/制品身份、命令、结果及限制 → 更新原任务和机器台账。已有证据在输入/
源码/制品不变时复用。未通过项保持未完成；本地提交不代表发布验收通过。

完整范围与出口条件继续以 [spec.md](spec.md)、[plan.md](plan.md)、
[tasks.md](tasks.md) 和上述合同为准。
