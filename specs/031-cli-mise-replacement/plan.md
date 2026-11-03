# Implementation Plan: Aster CLI replaces mise

**Branch**: current checkout | **Spec**: [spec.md](spec.md)

**Gate context**: existing CLI composition root and tool-acquisition owner.

**Updated**: 2026-11-03. 收敛实施计划；完全替代尚未完成。

## Summary

Extend the existing implementation in place. Repair actual installation first,
then implement project selection, locks, environments, tasks, backend coverage,
and host operations. Keep the full baseline inventory open until every feature
has behavioral evidence. Do not introduce a mise executable dependency.
The official extension-source follow-up uses `aster-ai-lab/plugins`, explicit
domain directories, native AIX validation and associated GHCR OCI artifacts;
see [trusted-plugins.md](../../docs/trusted-plugins.md). Integrate its signed
catalog/immutable digest contract in the existing extension owner.
Authoring uses the native SDK's reproducible AIX builder, then native package
inspection and domain/OCI preparation. Test byte reproducibility, unchanged
source/archive identity, executable modes, source bounds, rejected links and
preservation of existing outputs before producing official plugin artifacts.

完成有两个独立门槛：冻结的 **24 类 mise 能力全部获得行为验收**，以及
原 **35 应用 + 14 依赖/运行组件共 49 项全部完成插件迁移**。新增插件单列，
不能替换原项或凑数。实现、验证、制品发布分别记录状态，不能相互代替。

## Constitution Check

| Principle | Status | Evidence in the plan |
| --- | --- | --- |
| I. Fail closed | PASS | Typed boundary errors; explicit unsupported operations |
| II. Evidence retained | PASS | T013 writes the dated CLI outcome |
| III. Hermetic tests | PASS | Every behavior task includes real fixture execution |
| IV. Closed sets | PASS | Parse/as_str for admitted modes, shells, backends |
| V. One workspace / published APIs | PASS | Extend existing owners; preserve public APIs |
| VI. Local schema changes | PASS | Locks and runtime state stay with their owner |
| VII. Shrink-only ratchets | PASS | No allowlist expansion |
| VIII. Replaceable transports | PASS | Backend implementation stays behind tool contracts |
| IX. One configuration language | PASS | ACL-only product configuration |
| XIII. One repository | PASS | Current checkout; no extra repository or worktree |
| XIV. Names are not guessed | PASS | Exact declared identifiers and upstream registry data |
| XV. Meaning is not matched | PASS | Typed commands and mechanical version syntax |
| XVI. Pickers are drawn | PASS | Studio reuses shared Picker/Menu/SelectionHighlight; no native select |
| XVII. Dialogs stay in place | PASS | Installation, retry and confirmation follow DESIGN.md, including dark/reduced-motion behavior |

Principles XI/XII: retain counterexamples, verify the actual implementation, and
keep failures visible instead of redefining full replacement.

## Project Structure

Documentation: `spec.md`, `plan.md`, `research.md`, `tasks.md`,
`contracts/capability-matrix.md`, and frozen upstream inventory data.

| Layer | Path | Planned changes |
| --- | --- | --- |
| CLI | `apps/aster/src/main.rs` | Existing dispatch and utility behavior |
| CLI commands | `crates/terminal/crates/codegen/aster-pager/src/env_manager_cmd.rs` | Existing tool/environment commands and sibling tests |
| Tool library | `crates/use/tool-acquire/src/` | Declaration loading, selected installations, versions, locks, sources |
| Tasks and host adapters | existing owning crates under `crates/` | Reuse process, filesystem, HTTP, and scheduling adapters; extend only when needed |
| Deterministic tasks | `crates/code/core/src/environment_tasks/` (planned module) | Reuse bounded process/cancellation and existing `aster-lane`; provide one Rust API to CLI/ACP/Studio, without requiring a model or durable TS workflow |
| Host operations | `crates/code/core/src/environment_host/` (planned module) | Typed provisioning/dotfile/daemon plan, state, apply and platform adapters; add files only when their slice starts |
| Plugin lifecycle | `crates/use/src/plugin_manager/`, `extension_cli/`, `plugin_lifecycle/`, `crates/use/extension/src/` | Reuse native AIX, TUF/GHCR, immutable cache, reviewed plans, grants and receipts |
| Studio host | `apps/studio/src-tauri/src/{capabilities_registry,runtime_components,capabilities_registry_jobs}.rs` | Shared SDK, tenant-owned operations/recovery, required-component protection and window-scoped Tauri permissions |
| Studio UI | `apps/studio/src/renderer/pages/plugins/`, `components/layout/`, `src/shared/ipc/schemas/` | Registry data, component management, actual stages/bytes, retry/reopen/restart, theme and accessibility |
| Official sources | `plugins/plugins/<domain>/<publisher>/<name>/`, `domains.acl`, `registry/`, `recipes/` | Existing authorized submodule; native packages in associated GHCR, bound by signed metadata |
| Integration tests | `apps/aster/tests/` | Isolated CLI fixture execution |
| Evidence | `docs/evidence/cli/2026-11-01-mise-replacement/` | Commands, actual outputs, coverage limits |

`crates/terminal/crates/codegen` is the retained imported source layout described
in `docs/migration.md`; handwritten command files are not generated outputs.
Actual generated protocol files remain untouched.

## Research

- `env_manager_cmd.rs::acquire_all` already uses `spawn_blocking`; create and
  destroy its blocking HTTP client inside that closure.
- `env_acl.rs` already owns ACL parsing/rendering and merging. Extend it rather
  than adding another configuration language or loader in the CLI.
- `store.rs` already stages verified payloads and preserves the prior version
  on checksum failures. Preserve these contracts when selecting installations.
- `acquire.rs` already verifies resolved versions and exposes local storage.
- `crates/code/core/src/tools/process*` and the existing terminal process
  adapters own cancellation primitives; task execution must reuse them.
- `crates/code/core/src/orchestration/workflow.rs` is agent orchestration and
  does not stand in for deterministic command-task execution.
- `mcp_cmd.rs` owns existing client configuration. New server capability must
  preserve its commands rather than reassigning the namespace.

## Verification Plan

- Run focused `aster-use-tool-acquire` tests before and after changes.
- Run pager command tests and isolated CLI integration checks for actual
  selection, environment restoration, installation errors, and task effects.
- Run `cargo clippy --locked -p <owner> --all-targets --no-deps` and supported
  feature checks for each materially changed package.
- Run `cargo fmt --all -- --check` and `tests/workspace_dependencies.py`.
- Build `aster` and `aster-acp` together before binary acceptance.
- Retain failures from the starting checkout separately from regressions.
- Full completion requires affected Windows, Linux, and macOS native checks;
  a local macOS pass does not certify another platform.

## 收敛基线与当前状态

- 基线保持 mise `v2026.10.25`、提交 `3c78bbb2d58c0b668fe8d5bebf4a8cb66194e1aa`；
  [690 条 source_paths](contracts/mise-baseline.json) 是来源索引，不是功能通过数。
  保留 C01–C24 的所有必需子能力及其原有实验状态。后续上游增量另列，不覆盖冻结输入。
- T001–T003 有已完成增量，T004–T014 尚未关闭。已实现的部分版本、锁、离线、shims、
  卸载能力不能折算为整类通过。prune 已实现并通过 12 个冻结原生场景及运行中脚本的
  依赖读取场景；profiles、stubs 与其他完整后端语义仍需收口。
- 35 个原应用 manifest 仍在 `apps/use-registry/packages`；14 项依赖身份/顺序见
  [原始迁移清单](../../docs/trusted-plugins.md#desktop-dependency-migration)。源码存在、
  能打包、已发布、实际可用必须分别验收。
- Sandbox、Document、Office、OCR 的目录记录不代表原 49 项完成。当前 Office 固定 AIX
  已隔离安装并通过 8 条实际命令及 DOCX/XLSX/PPTX 验证，见
  [本轮证据](../../docs/evidence/desktop/2026-11-03-studio-plugin-chrome-followup/OUTCOME.md)。
  此证据可复用，不需在源码/制品不变时重新下载，但不替代原生 Studio GUI 验收。
- 旧组合 renderer 检查保留 304 通过、2 项原有 TabRouter 失败；修改前组件复现同名失败。
  新增插件恢复/重试及既有安装接口检查为 77 通过。最新全局 web 类型检查仍有
  286 项其他路径错误；7 项依赖检查通过。初批根格式通过；最新根格式在并行修改的
  `apps/aster/tests/tui_913_real_llm.rs` 失败，本批所属文件格式检查通过。Pager 原有 42 条 Clippy 阻塞
  已在其 owner 修复，tool-acquire/Pager/CLI 全目标 Clippy 已通过。原生 GUI 连接
  仍不可用。所有必需发布 gate 继续保留，不以局部成功豁免。
- 原 49 项台账与 24 类状态文件已建立；35 原应用 AIX 重新构建通过，690 个冻结
  源文件的 Git blob 身份均校验一致。M0 的叶能力/领域/目标/性能对账仍未完成。
  安装作业已加入租户持久化、宿主重启核对和精确版本重试；18 项原生回归通过。
  结果见 [本轮收敛证据](../../docs/evidence/cli/2026-11-03-convergence/OUTCOME.md)。

宪法表中的 PASS 仅表示方案遵守约束，不表示上述实现或验收已通过。当前检出及既有、
已授权的 Studio/plugins 子模块继续使用，不新增仓库、worktree、workspace 或 lockfile。

## 里程碑与出口条件

建议顺序 **M0 → M1 → M2 → M3 → M4 → M5 → M6 → M7 → M8**。
49 项台账从 M0 建立，随各阶段逐项关闭；M7 是总对账，不重新开始迁移。
每次只推进一个可验收实现切片，失败先按原因隔离复现，再进入下一切片。

| 阶段 | 能力 / 原任务 | 交付范围 | 必须满足的出口条件 |
| --- | --- | --- | --- |
| **M0 基线和阻塞收口** | 全量；T012/T013 | 展开必需子能力、49 项身份映射、平台/命令兼容表；归属所有 gate、GUI、runner、签名与发布前提 | 每个缺口都有源码 owner、固定输入、预期效果/退出、执行命令、证据位置和阻塞项；无未归属项，不改冻结基线 |
| **M1 安装管理闭环** | C07 部分，FR-013/014；T008 | 可信源→AIX→计划/授权→安装→使用→更新/移除；外部制品；作业持久化及宿主重启 reconciliation | 内置组件和一个原始普通插件完成真实链路；断网/重启/摘要漂移/仅有缓存元数据可恢复且不误报 ready；真实 GUI 的明暗色、空态、计数/字节、关闭重开、键盘通过 |
| **M2 工具生命周期** | C02、C03、C04、C05；T004 | 引用/别名剩余语义、多版本、upgrade/rollback/prune、配置跟踪、锁迁移、Windows dispatch、stubs/sync | 冻结用例逐项匹配；坏包不破坏原版本；prune 保留全局/多项目所选及运行中版本；preview 零删除、offline 零网络 |
| **M3 Runtime 与后端** | C01、C06，C07 工具/后端插件；T008 | 基线语言 runtime、发布/语言/自定义后端与注册表、布局/来源/签名/选项、legacy ABI | 每个支持组合实际获取并执行功能；标准库、包管理器、多入口、模式和 companions 完整；不能以名称已解析、空适配器或版本输出算覆盖 |
| **M4 环境与 shell** | C08、C09、C10、C11，C07 环境插件；T005 | profiles/层叠、unset/default/required、dotenv/source/PATH、模板、secret、alias/hook、virtualenv、动态进退目录 | 支持 shell 实际进入/离开嵌套与兄弟项目，准确恢复；空格/非 ASCII 路径通过；不累积 PATH、不泄露 secret，不执行不受信脚本 |
| **M5 确定性任务** | C12、C13、C14、C15、C16；T006/T007 | named/file task、参数/发现、DAG/共享依赖/并发/退出/取消；增量、本地/远程缓存、watch、monorepo 图 | 文件效果和调用日志证明顺序、去重、并发上限、取消和缓存恢复；cache hit 不执行任务；循环/坏缓存先拒绝；模型回答不作执行证据 |
| **M6 主机与长生命周期** | C17、C18、C19、C20、C21，C07 package-manager 插件；T009/T010 | bootstrap/modules、包/账号/文件/服务/防火墙/Compose/Git/SSH；dotfiles/history/sync/encryption；daemon/readiness/端口/worktree cleanup | 适用 OS 的隔离 plan/apply/status/第二次 no-op/rollback/cleanup；远程用隔离目标；实际效果、拒绝和退出匹配，不只验收计划文本 |
| **M7 原 49 项迁移与发布** | C07、FR-013/014；T008 | 35 应用 + 14 依赖的领域、身份、目标变体、来源、依赖、宿主版本、GHCR/TUF、旧安装接管/升级 | 原始项精确 49/49、不改名删项；适用 target 都有制品和消费者证据；内置项隐藏但可管理、required 受保护；旧依赖链接转插件页；配置/租户数据不丢失 |
| **M8 全面验收与三产品交付** | C22、C23、C24 及全量复核；T011–T014 | diagnostics/completion/docs/skills/policy、环境 MCP server、release 性能、原生平台、最终归档 | 24 类及全部必需子项通过、49 项通过、必需 gates 全绿、发布阻塞清零；按现有发布规范交付 |

### 每个实现切片的固定闭环

1. 固定一个真实场景和失败/拒绝反例，写清文件效果、退出码、网络/进程次数。
2. 只改所属库及必要调用者、测试、配置；CLI/ACP/Studio 共享 API，不复制业务逻辑。
3. 跑 owner 的 hermetic tests、支持 features 的 Clippy、格式及相关 workspace checks。
4. 做所需的真实已安装程序、宿主或最终制品验收；版本/ack/安装成功仅是辅助事实。
5. 留存源码/制品身份、命令输出、平台、工具链、features、build profile 和限制，再更新台账。
   已通过后不无理由重复扩展、重跑或追加无关 UI 润色。

### M2 首个切片：prune

先匹配现有 12 个冻结场景：无使用者的 current、全局 pin、多项目、prefix/多版本、配置
修改/删除、preview/exit-code、无人确认、configs-only、真实运行中的可执行文件。

- 用 `env_context.rs` 的来源信息建立显式跟踪写入；use/install/exec 在准入成功后调用。
  只读 loader 保持只读，preview/prune 自身不伪造一次项目使用。
- 复用 `all_uninstall_candidates`、`uninstall_candidate_for`、`remove_version_sync` 和现有
  版本解析，不另写匹配器，不用 current 指针决定保留，不因 payload 缺失跳过有效记录。
- 保留配置实际解析到的版本；真实进程与安装路径映射用于运行保护，不能只看 Aster PID 文件。
- `--dry-run` 不改声明/payload/锁/指针，`--dry-run-code` 退出匹配，无人确认不执行删除。
- 这一个切片通过后再补 upgrade/rollback 和剩余选择模式；新增 help 不关闭 T004。

第二个切片已接入 `tool upgrade`：现有安装器负责校验制品，配置所有者负责更新实际
声明文件，现有 prune 计划负责保护仍在使用的旧版本。十二个新增冻结对照场景与坏包
保留原版本、父目录配置和跨项目保留场景已有执行证据。默认宽限期清理、自动更新锁、
其余策略及完整原生平台验收仍未关闭。最终结果以本批证据中的命令和源码身份为准。

最新工具库 150 项、环境命令 Pager 23 项、全目标 Clippy 和 CLI/ACP 成对构建通过。
CLI 全套尝试为 32 通过、6 超时、1 忽略；六项在同一制品上按原条件单独复核通过。
再次全套验收先遭遇编译器无进展，随后直接启动测试程序也未进入用例。均保留为基础
执行阻塞，不能由单项复核替代全套通过；固定输入、超时和断言保持不变，尚不发布。

### 49 项台账与兼容边界

M0 生成 `contracts/legacy-plugin-inventory.json`，并建立 `contracts/convergence-status.json`。
两者来自源清单与原生 manifest 检查，不由名称/目录正则猜身份或领域。

每个原始项记录：来源/ID/名称/顺序、源路径/摘要、AIX ID、显式 domain/owners、target、
surface、builtin/required 权威、版本/宿主兼容、制品 URL/大小/摘要/布局/上游验证、
OCI/AIX/manifest 摘要、安装/更新/回滚/移除证据和阻塞项。实现、验证、发布为独立状态。
平台变体不增加 49 的分母；共享依赖仍保留原始身份及显式映射，不通过去重删行。

CUA 按明确的 CLI producer manifest/发布契约选制品，不用仓库 Latest 猜文件名；外部
工具不经未验证安装脚本执行。旧安装仅在来源、版本和完整制品可验证时接管，否则明确
待处理，不能凭二进制存在伪造受信回执。迁移不复制凭证/用户会话。

产品继续采用 Aster 命名空间和 ACL。现有 agent/use/mcp/plugin 命令含义保留；任务使用
显式 task 命令，环境 MCP 使用独立服务入口，不把现有 MCP 客户端管理冒充为 server。
外部 registry 数据经受限适配器读取，不进入配置层叠或授权策略。

## 验收、性能与阻塞规则

- 工具：owner suites、pager sibling tests、`apps/aster/tests/env_manager.rs` 真 CLI 差分。
  固定 reference 二进制、输入、超时和判分器；不硬编码输出或删反例。
- 插件：既有 SDK/CLI suites + Studio 原生 registry/runtime tests；合法/篡改 AIX、
  TUF/GHCR、外部制品、resume/offline、计划摘要、授权、租户、required、宿主版本兼容。
- 任务/主机：真实进程/文件效果；账号、服务、网络和远程操作只在隔离 native 目标验收。
- UI：既有 renderer/E2E 和真实 Studio 原生窗口；browser mock 不替代原生包/IPC 验收。
  不恢复已删除的依赖设置页、性能调试面板、百科选择页或重复终端 header。
- 目标平台由 M0 按既有 release/qualification 矩阵冻结：CLI/ACP 覆盖现有 macOS/Linux/
  Windows 目标；桌面先覆盖现有 darwin-aarch64 通道，其余声明支持目标逐项原生验收。
  不把交叉编译、WSL 运行 Windows 程序或单台 Mac 通过当成其他目标通过。
- 无 mise 验收时从 PATH 去掉 mise，隔离状态并检查执行链不启动/下载 mise。产品实现
  不捆绑或委托 mise；基准二进制只用于验收。
- 沿用既有固定性能工作负载，在 release profile 记录冷/热 p50/p95、资源、请求、子进程。
  M0 先固定基线与允差，不在失败后改输入。不可变安装重复选择不下载、offline 请求数
  为 0、task cache hit 的任务效果执行数为 0；明显回退先 profile 再修，debug 时长不算交付。
- 子能力具备实现、固定行为见证和适用平台证据才通过；任一必需子项失败，C 行不能关闭。
  基线支持而 Aster 尚不支持的条目仍是缺口，不能改名 unsupported 后算覆盖。
- GUI、native runner、签名/公证、发布凭证等前提缺失，登记为对应 gate 的阻塞，不能跳过。
  当前测试/类型/格式失败归属 owner 修共因，不扩 allowlist 或降检查强度。
- 每个候选绑定父仓库提交、Studio/plugins gitlink、组件锁及制品 SHA。变更只复验受影响
  切片，旧失败和旧证据保留。台账汇报必需子项状态，不用测试数或目录数当完成率。

## 发布门槛与顺序

严格复用 [release-governance.md](../../docs/release-governance.md)。开发进程退出或 debug
build 成功不构成发版资格。本计划不执行 commit/tag/upload。

1. M8 门槛全过，形成干净且可定位源码的候选；在既有授权和流程内处理提交与发布。
2. Core/ACP：版本、平台归档和 `crates/code/manual/RELEASE_X.Y.Z.md` 对齐，发布
   `aster-acp-vX.Y.Z`，`--latest=false`。
3. CLI：同提交构建 `aster`/`aster-acp`，归档携共享 BUILD_ID、SHA256SUMS 和完整 sandbox
   companions；最终归档安装到隔离 prefix，通过 `tests/cli_sandbox_smoke.py` 的真实
   Linux 自动模式及 Darwin YOLO 命令执行后才可创建草稿/发布 `aster-cli-vX.Y.Z`。
4. 桌面：消费已发布组件锁；轻量包先匿名安装固定 Sandbox/Document/Office。最终归档
   按源契约核对 native libraries、OCR/models、字体、Office/ACP、capture helpers、renderer
   入口，完成签名、公证和原生验收，再生成/发布 channel manifest 与 `vX.Y.Z`。
5. 只有版本化桌面 Release 可标记仓库 Latest。CLI/ACP/滚动通道均 `--latest=false`；
   核对 desktop-stable、aster-cli-stable 和桌面 Latest 上旧 CLI 兼容清单，指向版本化制品，
   不创建字面 latest tag。
6. 回读公开下载和摘要，保留当前/上一版可回滚制品；保留仍被底座/锁引用的 OCI digest，
   不覆盖版本化二进制或移动已发布 tag。

## 执行口径与偏差

首批为 M0 对账/修发布阻塞 → M1 补宿主恢复与原生 GUI → M2 prune/生命周期。
M0 完成子项拆分及真实基线后再给逐项工作量与关键路径，不承诺未经估算的发版日期。

共享 procedure 中的退役桌面路径示例由当前 `apps/studio` 替代；在原 plan/template 结构上
完善，不覆盖原 spec、冻结 contracts、T001–T014 或历史证据。没有新增应用、仓库或工作区。
本次仅更新计划，不声明 prune、任务系统、49 项迁移或跨平台/发布已完成。
