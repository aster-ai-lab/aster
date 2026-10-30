# ACP 长程任务执行记录（架构目标未完成）

## Current published checkpoint

Aster CLI0.1.3 / Core+ACP9.1.5 is published at https://github.com/aster-ai-lab/aster/releases/tag/aster-cli-v0.1.3, from frozen source220562fe9912d72d7dedafb8b36b5655e1394d8f. The tag and public latest channel were independently checked. Exactly three platform archives and latest.json have matching uploaded server digests. Linux uses x86_64/glibc2.31; there is no special Kylin CLI archive or alias.

The selected macOS release archive passed actual ACP20 (13 product workflows+7 auxiliary), protocol10, controlled compiler-VM H1 with19 independent checks, five real installer/version/updater/reinstall checks, and a real public-network updater check. H1 took25,266ms for the controlled workflow, with8 tools/7 original approval requests; its18 synthetic tokens are not real-model billing. Whole-test times include binary identity verification. Binary/manifest/tester and5681-source-file fences stayed unchanged. Windows CI passed two native install/rollback controls; Linux binaries both pass the glibc ceiling and depend only on base runtime libraries. These are stated component scopes, not130 fully qualified cases or real-model performance results.

Mac CI was cancelled after over2h without an archive. The shipped Mac pair was built locally from the same pinned source and Rust1.97.1, using locked offline release/default features, in951.835s. Linux and Windows reuse successful run40110868267 archives. Local compiler features are retained; optional workflow features remain off. `release915three-qualification.json` binds this exact chosen subject. The first installer script assertion used the wrong JSON field spelling; its command log/red record is retained. Correct camelCase checks passed in a fresh isolated prefix without changing product code.

The broader architecture objective is unfinished (goal-tool status blocked). Arbitrary never-returning persistence/export callbacks, complete crash/recovery and changing-argument/model-derived-output loops, full cross-platform/GUI/soak and real-model speed/token results remain unproved. Studio baseline gaps still block whole-workspace CI; the chat-exposed read-only token still needs restriction and rotation.

## Historical checkpoints retained

## Earlier result

目标 active。规划已完成（120 个产品 case + 10 个测试系统自证、12 条主旅程、17 项任务）。T001–T003 基础设施部分已实现，尚未完成长程 ACP E2E 资格。未将基础测试或 provider 健康探针计为这些产品用例已通过。

**真实诊断新增：1 PASS，2 个默认配置失败记录保留。** [新样本](native-multiturn-diagnostic-6aa93ca4-e4d1-401f-88cd-bed4423a6c6b/result.json)通过实际 ACP/DeepSeek 完成六轮原生文件更新；它是 C01 的明确宿主 profile 诊断切片，不是完整 LH01/Rust/发布资格。

六轮各有当前数值 oracle、源文件保留和 `verified/passed` wire outcome。39 次真实上游请求 / HTTP 200 39 次，23 次工具调用、6 次一次性审批；无 relay 拒绝或传输失败。工作流 119.259 秒，含前置的整个测试 146.21 秒；不外推性能 SLA。总额依次 588、991、952、972、972、972 分。字节摘要验证使用已有显式 `disk_content` 宿主能力，语义由外部 checker 独立检查；二者不互相替代。

该原生样本配对身份 `aster-cli-pair-v1:4fad5193ef6d3547db4a75a9fac06a233e69b55a:aarch64-apple-darwin`，前后 source encoding 和三份二进制 SHA 均一致（`host_profile_before.json` / `host_profile_after.json`）。额外捕获的 1920 个 root dirty 文件在该样本中未变；根 HEAD `e9b6da57`。这个旁路补充不涵盖全部 ignored 构建输入，T001 的完整传递输入资格仍未完成。

默认 profile 的两次错误已 SOLO 复现并定位到正确的 completion gate 拒绝：未配置绑定验证，不能凭模型读回声明完成；不是上游 HTTP 成功就能完成任务。另确认原夹具遗漏 units 含义，v2 已明确为商品数量总和。两份 [诊断](native-multiturn-diagnostic-6b73d84a-f75e-45bf-b08c-377cea96ed29/DIAGNOSIS.md)和失败结果保留，新 profile 未覆盖它们。

## What landed

- `long_horizon_preflight.rs`：只用官方 ACL 解析发现模型和状态覆盖，无 Agent/账号导入/模型调用；两个防泄密测试通过。显式真实模型健康探针限制一次 generation、零重试、512 输出 token、120 秒。
- `long_horizon/report.rs` 与 `long_horizon_contracts.rs`：封闭状态/归因、当前证据检查、拒绝未知/重复/跨 campaign 身份、保留首轮失败、拒绝缺失前次 attempt、unknown usage 不计零、总量溢出拒绝。
- `long_horizon/config.rs`：构造 ACP 专用隔离 ACL，剥离原上游 key/headers/模型覆盖、账号导入与用户目录，只指向明确 loopback relay。通过 a3s-acl 生成正确转义的路径，不手拼用户配置。
- 自证过程保留了两个失败对照：遗漏首轮历史、路径用未加引号 Display 造成无效 ACL；均先留红、再修复。该早期阶段结果/配置自证 10 项通过，已由下述 30 项套件接续。

## Foundation validation update

新增真实 HTTP relay（复用公共 Core HttpClient）、ACP stdio 驱动、Rust 多文件 C0 夹具，以及原生文件六轮数据夹具。该阶段基础契约 **30 passed**，后续扩展至下述 39 项；共享模块在 real-LLM target 再跑 18 项，不能重复计作 48 个独立用例。ACP host attestor regression 1 passed；Core attestor regression 5 passed。真实模型用例默认 ignored，须显式选择可信 ACL、精确模型和证据目录。

修复并保留红/绿记录：relay 丢失 Retry-After；Darwin 已退出但未回收的子进程组被误判为清理失败。取消 relay 的有效终态允许 EOF 或明确 502，必须证明实际上游连接关闭，不能强迫只有 EOF。权限选择按 ACP PermissionOptionKind，未知选项取消；不复用名称/词表启发式。

真实数据诊断每轮独立验收当前 JSON，数字 golden 已预先固定；读取经 cap-std 限定 workspace，拒绝越界符号链接、超过 128 KiB 的产物和修改后的用户草稿。Rust C0 夹具证明已知修复可通过，旧产物和改 checker 不可冒充通过。该阶段尚未对真实模型生成的 Rust 运行外部编译/执行验收；现已由下述真实 Rust 样本与独立 VM 复核接续。

采集器现保留有界 RPC error 和精确 outcome 扩展，移除 fixture 凭证；thought 只留事件类型/字节数。早期两份 trace 中的 769 条 thought 正文已移除，前后 hash 记录在 `trace-sanitization.json`，新样本审计确认正文为 0。

## Checks and remaining work

依赖边界检查 6 passed，CLI pair delivery 10 passed。默认 Python 3.9 缺 `tomllib` 的失败保留；以已安装 Python 3.13 运行成功。新增文件 scoped rustfmt 通过；最终全仓 `cargo fmt --all -- --check` 仍检出私有 Studio `authority.rs` / `org_login.rs` 两个文件的格式差异，未覆盖其它会话修改。

T001–T003/T005/T009/T011/T015/T016 仅部分推进，任务未勾选完成。下一阶段：完整输入/身份与证据合同；H0/H1 协议、安全、故障屏障、取消/恢复；有资格的命令后端；更多真实模型/固定样本；实际 TUI/GUI；CI、资源/soak 与覆盖率。没有以一个诊断样本替代这些工作。

## Execution-backend preflight（早期记录，已由下节显式运行时接续）

当前 macOS/arm64：未设置 ASTER_SANDBOX_DIR，也无 target/debug/sandbox companions。历史 target/t013-assets-hardware 已不存在；历史证据不能替代当前文件。native backend 的 macOS 命令执行明确拒绝。Docker CLI 存在但 daemon socket 不存在。未设置 process-host fallback，未放宽权限或绕过拒绝。

该早期阶段需要命令的 Rust 资格受后端环境阻塞；后续显式运行时现已可用，不再作为当前阻塞理由。这是增加有明确边界的诊断 variant，不替换或降低原 Rust 旅程的验收标准。

## Compiler VM and Rust workflow update

当前已有独立拥有的 `target/qualification/acp-sandbox-bootstrap/assets`，只通过显式测试参数使用，没有全局安装或部署。root 固定 1.97.1 工具链已成功构建 ARM64 / x86_64 musl guest agent。修复了两项实际构建问题：libc 缺少 Linux mount UAPI 常量，以及 root musl flags 将 GCC 的 `-Wl` 参数传给 rust-lld。红/绿日志保留；实际两种架构 ELF 的 non-executable GNU_STACK、GNU_RELRO 和 static interpreter 检查通过。没有降低 sandbox 或链接硬化。

Alpine 系统镜像两次独立装配一致；官方 Rust 1.97.1 ARM64 镜像按 digest 匿名拉取，独立目录中生成 compiler rootfs。源、签名后的 host/shim、guest agents、镜像和 root 配置身份见 `sandbox-component-identities.json`。每个 Rust 工作流额外遍历实际 runtime 的 7269 个条目、约 1.037 GB 文件，前后比较内容、权限和链接目标；不将此称为全部传递 Rust 源构建证明。

真实 VM / Core SDK 预检通过：认证协议、工作区/凭证限制、Rust 编译并执行真实测试、setsid 后代的超时与取消清理（`sandbox-current-qualified-smokes.log` 的四项物理测试，加 SDK auth 日志）。原生 macOS 命令拒绝及 process-host fallback 禁用均保留。Box runtime 的 no-default-features helper 编译另有五项既有 feature-gate 错误，日志保留；实际 rootfs helper 使用 default features 成功，不宣称该无默认特性组合已支持。

Rust fixture 的独立隐藏验收只在 ACP 回收之后，于另外的 VM 中编译当前有界文件快照；模型生成的代码不在宿主运行。外部 checker、保护文件和交付前后字节均检查。真实 guest 反向控制通过：原始缺陷拒绝、完整参考修复通过、旧错内容拒绝、修改保护文件即使功能测试通过也拒绝（`rust-guest-oracle-controls.log`）。读取还拒绝符号链接、硬链接、非普通文件、越界及过大产物。

`rust-workflow-c0.log` 的三 target 汇总是 35 / 23 / 22 passed；共享测试去重后 **37 项**，不能相加当作 80 个独立用例。原 30 项结果留存。H1 主链第一样本已通过：7 次真实工具调用，5 个主模型完成步骤 / 8 个回环请求，四个模块/说明写入调用在一次响应中批量发出，初次失败和最终通过命令均为原声明命令，最终 `verified/passed`，独立 VM 的四项测试通过。ACP 工作阶段 20.691 秒、含 runtime 身份遍历与准备的全测试 96.66 秒；不把两种耗时混为产品延迟。证据：`rust-scripted-3d02cddb-f4d3-4312-9c5f-ef87748ef050/`。

新增配对产物冻结 helper 后再次编译，两份 bin 与当前来源编码匹配：`rust_candidate_before.json`。H1 对这份独立配对副本复验通过（`rust-scripted-frozen-pair.log`）：工作阶段 21.239 秒，全资格测试 207.80 秒。真实 DeepSeek 的 C01 Rust 单 seed 诊断已按 `rust-r1-registration.json` 预注册条件执行，详见下节。其结果不会回写旧的 native 六轮 cohort，也不替代全旅程/多 seed 资格。

新增 CI 默认/ headless-search 契约入口与两种静态 guest 构建；workflow YAML 解析通过，远端 CI 未运行。fresh 依赖边界 6 passed、CLI 配对交付 10 passed、musl linker 2 passed。scoped formatter 通过；完整 formatter 仍有并行任务私有 Studio 文件差异，见 `rust-workspace-fmt.log`，未覆盖它们。

## Real Rust R1 result and test-oracle fixes

[首轮 Rust R1](rust-multifile-real-diagnostic-0190ed21-934c-4689-917d-21ff955ebd23/DIAGNOSIS.md)整次 **FAIL**：11 个真实上游请求均为 HTTP 200，13 次工具调用，工作阶段 68.584 秒，含身份/独立 VM 的总测试 255.49 秒。三模块与 CHANGELOG 的当前交付通过独立验收，但模型未运行精确宿主合同命令：初次检查追加 echo 把失败状态 101 掩为 shell 0；最终检查另加清理前缀和 echo 后缀。因此完成门正确拒绝绑定；没有改弱合同或将功能正确冒称整个流程通过。

当前配对 source encoding 和三份候选 artifact SHA 在 `rust_real_before.json` / `rust_real_after.json` 一致；root HEAD 因其它任务改变，但相关源码字节未变。执行的 CLI 副本与完整 runtime 摘要一致。原 test executable 在之后 grader-only 修改前未独立归档，保留这个重放限制；不声称完成 T001 的全部输入或可迁移重放资格。

`usage-audit.json` 从 11 个唯一 callSequence 的 ACP ModelUsageBound 投影核对：输入 235,470、输出 6,108、缓存读取输入 102,656 token；扣除缓存读取的报告输入为 132,814。原始 provider usage presence 仍未独立采集，不计算金额，不把所有输入按未缓存计价。原轨迹还有一次读取 `.a3s` session 日志的额外命令；首个主请求输入 15,503 token，最终达到 30,124。提示组成、动态验证上下文与缓存前缀仍需受控比较，不能直接归因为 LoopX 或原 Terminal-Bench。

独立审查发现并 **先红后绿** 修复两项测试缺陷：有效 quantity 只判断 is_ok 而未核对返回值，导致始终返回 1 的错误实现被放行；guest/wire oracle 固定 4/3 测试数，导致合法新增测试被拒绝。现在要求原必需测试实际通过，允许额外测试，并核对具体解析值。`rust-oracle-quantity-red.log`、`rust-oracle-extra-tests-red.log` 保留 SOLO 失败；加强后的 VM 控制、额外测试和真实交付重验 **3 passed**（`rust-guest-oracle-strengthened-green.log`），零新增真实模型调用，原 R1 FAIL 不改写。

最终默认三个 target 为 37 / 25 / 24 passed，去重后 **39** 个基础契约；headless-search 另含 discovery 两项，去重后 **41**，这是另一 feature 组合，不相加当独立需求数。两种 feature 均零失败。三项 VM 测试另报，忽略项/未执行项不计通过；不是 130-case 覆盖率。

下一项 T016：让完成门优先明确已接纳的精确宿主命令，验证无需重做整项任务的有界核验恢复；保留状态掩盖/复合命令的拒绝。随后对系统/工具 schema、动态上下文及缓存前缀做无正文的被动观测和受控比较。模型行为、产品引导和测试规格分开归因；完整 R1/R2、故障、TUI/GUI、资源/soak/覆盖率仍未完成，目标继续 active。

## Exact-contract guidance and performance diagnosis

完成门原先只提供泛化核验建议，未优先告诉模型已接纳的精确宿主命令。Core 共享路径现按实际变更筛选有效合同并明确原样执行；无覆盖合同时拒绝猜测。两个实际 Engine 回归先红后绿；复合命令、无关合同、未知操作的拒绝保持。另修复恢复时任意 settlement key 可误删无关 unknown operation 的既有缺陷，只接受原有三个有归属的 namespace；76 项 harness 和 52 项恢复/来源回归通过，组间有重叠。

冻结候选 `7b53dcb436374e0accf16e97c05f09c38a604bc0` 的受控 H1 `rust-scripted-70cb393e-a00d-4976-bd89-042034195fc8` 通过：错误复合核验之后，下一模型请求实际包含精确合同引导，同一用户回合补跑裸命令，绑定完成并通过独立 VM 验收。工作阶段 22.248 秒，全资格测试 200.46 秒。不是重新发送整个任务，也没有 waiver。

[新真实 R1](rust-multifile-real-diagnostic-7283d1df-f1e0-4845-890b-0d674b26e5aa/adjudication.json)整体仍 **FAIL**，保留原始结果：当前产物、保护文件、独立 VM 语义检查和最终精确命令绑定均通过，wire outcome 为 verified/passed；但初始检查仍带 cd 前缀及 echo 后缀，stdout 显示三项失败及 EXIT=101，shell 却返回 0，未满足本变体的精确失败基线要求。12 次真实请求均 HTTP 200，14 次工具调用，工作阶段 70.734 秒，全资格测试 261.41 秒。配对 source encoding、三份候选 artifact SHA 及完整 runtime 树前后一致；实际测试可执行文件已独立归档。未宣布整条旅程通过或性能改善。

[用量审计](rust-multifile-real-diagnostic-7283d1df-f1e0-4845-890b-0d674b26e5aa/usage-audit.json)：12 个连续唯一 callSequence 的 ACP 投影共输入 217,377、输出 4,866、总量 222,243，缓存读取 133,248 token；原始 provider usage presence 尚未独立采集，不计算账单。与首个样本的动作不同，单 seed 前后值不能证明优化收益。

[无正文请求审计](rust-multifile-real-diagnostic-7283d1df-f1e0-4845-890b-0d674b26e5aa/request-audit.json)：11 个主请求每次均携带 24 个工具定义、42,761 字节 schema；首个主请求的 system 为 19,240 字节、报告输入 15,396 token，最终报告输入 22,780。主请求累计 852,768 字节，工具定义占 470,371 字节（字节不是 token）。工具摘要不变，system 有 5 个不同摘要；前 4/16 KiB 摘要均稳定。system 变化与部分请求缓存读取从 15,744 降至 3,840 token 同时发生，不能据此声称全部缓存失效或量化因果收益。

本轮初始源码存在进展判定组合风险：`harness_loop.rs` 的 information_delta 按调用身份收据计数，并非按相同输入快照下的新增信息去重；`goal_control/facts/settle.rs` 又把 `uncertainty_delta != 0` 也当进展，包含不确定性增加。后者现已由下节 V2 修正，信息去重仍待验证。plan_delta 字段允许 assistant output，但实际调用目前集中于已验证及取消路径，不能声称所有普通模型文本都会增加它。历史只读步骤清零停滞计数、跨回合 unknown 阻止写入的缺陷已有独立红/绿证据；尚未与原 Terminal-Bench 的任务、构建身份及轨迹对齐，不能将原耗时全部归因 LoopX 内化。

`git blame` 将上述四维进展计数定位到 2026-10-22 的 `5e855a3d26` / `56db267722`，晚于 2026-10-18 的源码内化提交 `d5398e219`。因此这个当前风险有可识别的后续规则修改来源，不能称作已经证明的初始 LoopX 导入回归；更早缺陷是否继承自上游仍需上游版本和原任务轨迹。

Terminal-Bench ACP 的默认 goal_continuation 路径与旧 host-admission 重试路径是替代分支；不能把两个上限相乘当作实际请求数。默认 max effort、全能力目录、规划/委派和恢复是额外成本来源，是否值得需同任务同模型受控比较。Harbor runner/LSP 安装属于 setup 耗时，不是模型 token 成本；本资格测试的 1 GB runtime 摘要与冻结/独立 VM 同样不属于 ACP 工作阶段。

## Versioned uncertainty-progress correction

新目标为“深度规划并调优和端到端测试 a3s code acp 的长程任务能力”，继续沿用全部 130 case / 12 旅程范围。[plan.md 第 6.1 节](../../../../specs/017-acp-long-horizon-e2e/plan.md)补充了收敛性、验证合同、缓存/提示、辅助调用和预算/重试的受控优化顺序；冷/热/构建/模型/工具/独立验收分开计时，真实 A/B 每臂至少 5 个预注册 fixture seed。不是性能收益声明。

`uncertainty-progress-red.log` 在原实现中通过实际 fact fold 证明：只有债务增加的 slice 仍清零停滞计数（实际 0，期望 1）。不能直接全局替换为 `> 0`，否则旧日志中已合法接纳的后续 Run 可能在重放时被判非法。新接纳源使用闭集 `turn_progress_delta_v2`，结算按其原始 admission 的版本解释进展：新记录只有债务减少可单独获得额度，V1/无 basis 的历史解释和原始负数计量保留；不改旧事实、权限、waiver、完成绑定或公共 Rust API。旧二进制不认识 V2 会拒绝新日志，不能将降级兼容视作已提供。

新增三项回归覆盖：负数及 i64::MIN 债务增加不能清零；债务减少、只读信息、有效变更和计划维度的正向对照；旧 V1 负向进展后的合法后续 admission 在文件日志重新打开后仍有效。12 项 continuation 回归通过（`uncertainty-progress-green.log`）；最终当前源码的 Goal/恢复/完成门及首次宿主合同提示 **186 passed**（`progress-v2-core-final.log`）。此前 184 项是重叠子集，不相加。

真实 ACP 回归先保留了 **1 passed / 7 failed**（`progress-v2-unattended-e2e.log`），7 项均逐项 SOLO 复现。原因是运行器将临时 fixture 放在本仓库 ignored target 下，却未阻止 Git 向上发现父仓库：声明的 non-git workspace 实际被观察成父 Git worktree，忽略文件没有变更收据。`progress-v2-fixture-git-isolation.json` 的 git rev-parse 正反控制证明该前置错误。只对隔离测试进程设置 `GIT_CEILING_DIRECTORIES`，不改产品或全局环境，验证目标单项先通过，再完整套件 **8 passed**（`progress-v2-unattended-isolated.log`）。原失败不覆盖，也不计产品/model 失败率；相同套件的环境变体独立报告。真实日志的 6 个 Goal session 均观察到 V2，见 `progress-v2-stdio-goal-basis.json`。停滞对照在 4 个 slice、14 次本地模型请求后如实结束为 needs_input，不冒称 Goal 成功；已完成目标只核验一次即结束的对照通过。

原始 stdio 协议 **10/10 logical cases，3 Rust tests passed**（`progress-v2-acp-conformance.log` / `.md`）。这些是脚本回环模型的真实 ACP 进程测试，unattended 使用其既有 benchmark host profile，不替代 OCI/R1 或全部取消后代清理资格。新测试父进程使用 env -i、专有 HOME/TMPDIR，未继承用户密钥或会话。

默认和 headless-search 四个基础 target 分别均为 39 / 2 / 27 / 27 passed，各自去重为 **44** 个测试名（含两项 discovery），零失败；分别的 ignored 数为 3 / 2 / 5 / 5，不计通过，两个 feature 的重复成绩不相加。日志为 `progress-v2-default-contracts.log` / `progress-v2-headless-contracts.log`，去重明细为对应 `*-unique.json`。未将其当作 130-case 运行覆盖率。

当前修改路径 scoped rustfmt 和 diff check 通过；依赖边界 6 passed，CLI 配对交付 10 passed。全仓 formatter 仍检出其它任务的私有 Studio authority/member_session/org_login 差异，未修改它们。两份 CLI 已恢复为一起构建的默认档，source encoding 为 `421964d3e43c0aee034a0eb9cae2ed335c19d05b`；实际受测 ACP SHA 前后及 feature 恢复后相同。Aster SHA 在 feature 矩阵后重建改变，且本轮没有运行 TUI，因此不声称 Aster 字节稳定或 U 层资格；source encoding 也不覆盖全部传递构建输入。真实 H0/Core tester 的当前路径/SHA 单列在 `progress-v2-validation.json`；另一个 long-horizon tester 是有意重新编译，不能拿它的变化冒称执行过新 R1。

本轮没有新增付费模型调用、提交、推送或部署。重复收据的信息新颖性、缓存布局优化、原始 provider usage、完整多 seed R1/R2、故障矩阵、TUI/GUI 和 soak 均仍未完成。

## Raw usage, model-visible exit status and Terminal-Bench attribution

新增测试侧的有界 JSON/SSE 原始用量观测器。每个 relay dispatch 记录 HTTP 状态及 Present/Missing/Invalid/Incomplete，严格核对必需整数、总数、缓存别名、范围与溢出；取消/缺失/损坏保持 unknown，原始传输不变，不保存响应/思考正文。只有全部调用的原始计数齐备且一致时才给出 token 总数。它支持当前 OpenAI-compatible 观测档，不等于所有 provider 或账单真实性证明；Reqwest 内部重定向仍可能让一次 dispatch 产生多跳，严格网络尝试预算尚未合格。

受控 H1 `rust-scripted-6f0a2122-4ec9-4f1c-96e8-0a79dbabadcf` 已通过同一 relay：9 次实际脚本生成、9 份 Present、9 个连续 ACP callSequence 逐一相同，总量 18（每次脚本固定 1 输入 + 1 输出），独立 VM/精确命令恢复及身份均通过。工作阶段 21.838 秒，全测试 196.82 秒；见其 `raw-usage-audit.json` 和 `raw-usage-h1.log`。这是 meter/协议自证，不是自然语言模型计费或优化收益；旧真实 R1 缺失的原始 usage 不能回填。

发现共享 `message_from_folded_tool` 从 typed fact envelope 投影模型消息时丢弃 exit_code，而 OpenAI-compatible tool 序列化也不发送独立 is_error；静默失败可能成为空工具消息。现仅在模型可见消息添加真实数字状态，保留原始输出/回执、图片、调用身份和无状态的旧/取消记录。`model-exit-status-red.log` 先失败，`model-exit-status-all-related.log` 的事实/恢复/Goal/完成门等相关 Core 共 **488 passed**（重叠子组不累加）。基线合同引导同样只明确用户已请求的检查，不新增步骤、不放宽完成门。首次 293 项回归中的 3 个失败均 SOLO；除状态前缀的真实预期更新外，两项既有 fixture 假设已最小修正并保留原失败。

新实际 H1 `rust-scripted-d5ef332d-0879-442e-83b6-0559ce020300` 没有通过（`exit-status-h1.log`，318.52 秒），不能用先前 H1 替它背书。测试替身硬编码模型必须收到 101，隔离会话实际 bash 回执为 1；替身 panic 又使整体结果文件未写出，因此不完整产物不计通过。小型 SOLO `scripted-exit-code-assumption-red-exact.log` 复现有效非零 1 被错误拒绝，现保存模型所见数字并在整体门与本次精确命令的 wire 回执核对；未知/0 不满足失败基线。`scripted-exit-code-assumption-green.log` **37 passed / 3 ignored / 2 filtered**，未计两项显式排除的 VM 门；新 H1 独立 cohort 已完成，详情见下段。先前误加 --exact 导致零测试的 `scripted-exit-code-assumption-red.log` 不计执行证据。

新 H1 `rust-scripted-66d114c8-85eb-4b75-a0f4-6b770e10b5cf` 的已声明工作流通过：模型在四次修复前实际收到失败码 **1**，与初次精确命令 wire 回执相同，之后经历复合核验拒绝、动态精确合同引导、裸命令成功、绑定 verified/passed 和独立 VM 验收。9 次已接纳脚本生成（6 主 + 3 辅助）的 raw/ACP 用量逐一一致，总量为人工固定 18；8 个工具、工作阶段 26.035 秒、全测试 216.74 秒。源码编码 `5ca2165646b94d51bec20ce972cac5d4244fb630`、配对二进制、实际 tester 归档 SHA 及 runtime 前后保持，见 `exit_status_bound_h1_before.json` / `exit_status_bound_h1_after.json` 和该 cohort 的 `exit-status-usage-audit.json`。另观察到 **1 次未归因的本地 relay admission rejection**，没有记录调用方/原因；零拒绝的独立审计断言失败已保留为 `exit-status-bound-h1-audit-initial-failure.log`。不将它补为零、不猜测 provider 故障或宣称全部网络尝试资格，原工作流 pass 仅按其既定门解释。真实模型与性能 A/B 尚未执行。

当前默认四基础 target 去重为 **54** 个测试名，原日志 46 / 2 / 35 / 37 passed，3 / 2 / 5 / 5 ignored，零失败（`exit-status-default-targets.json` / `exit-status-default-unique.json`）；上述 37 项受控修正另跑已绿。headless 的上一源码阶段也是 54，见 `exit-status-headless-unique.json`，未把这次 fixture 修改冒称已重跑该 feature。另有真实 stdio 3 个 Rust tests / 10 logical cases 及 unattended 8 passed；不是 130 原始 case 覆盖率。依赖边界 6 passed、配对交付 10 passed，当前修改 scoped fmt/diff 通过。完整 workspace formatter 仍有其它任务的 Studio 差异。

回应原 Terminal-Bench 原因分析：历史已证明只读步骤清零停滞、不确定性 latch 与完成门组成无出口、V1 债务增长当进展等代码缺陷。新调用身份收据是否真有新信息仍待处理；大 system/schema 与动态前缀又会放大重复请求成本。当前 interactive R1 没有自动 Goal，也出现高输入，因此不能把全部消耗归因自动续跑。已有上游 native 固定十任务记录（`../a3s/docs/evidence/code-harness/20261017-terminal-bench-subset/OUTCOME.md`）为全部超时、七项 empty-retry，它不是 ACP 或当前模型 cohort。旧 ACP 脚本不构建二进制、默认允许约 7.8 小时；原任务实际 launch 未取得，不能断言是否用了旧构建/该预算。出处和限制补入 spec `research.md`。

本仓库的 2026-10-18 导入不是 LoopX 引入点证据。只读追溯既有上游，2026-09-29 `411d19bb` 已有丢弃退出码的投影；2026-10-22 的进展规则则是后续本地修改。需要内化前版本、原始任务与实际构建轨迹，才能判定具体 LoopX 引入回归。此前“所有失败均属模型天花板、代码已无缺陷”的说法证据不足。没有新的付费模型调用、原 Terminal-Bench 重放、受控性能收益、发布或部署声明。

实际 ignored 子工作区仍有产品风险：仅给非 Git 测试 fixture 加 Git ceiling 修正了其前置条件，但不能证明合法的 ignored 嵌套 workspace 已能正确观察写入、生成变更收据和阻止错误 Narrative 完成。该问题作为独立 P0 待验证；不以 fixture 修正宣称产品已解决。

## Ignored-root correctness and single-hop transport

两个最小红例及各自 SOLO 已证明完整 ignored 子工作区是产品缺陷，不能只归为 fixture 问题：实际新增/修改/删除没有 effects，SDK 写入后错误返回 Narrative。原失败保留于 `ignored-workspace-red.log` / `ignored-workspace-red-solo-{1,2}.log`。现在由 Git check-ignore 的明确结果为该根启用已有的有界文件索引；可见 Git 与读取失败仍保留原边界，ignore 状态改变保持 Incomplete。活跃 Run 保留原始观察基线，整根由文件索引统一提供路径；后台子任务保留活跃文件基线，不能拿父 Git 的空状态替代已丢失的基线。没有新增产品配置、修改 ignore 文件、扩大工作区或绕过沙箱。

最终观察器/执行状态 **169 passed**（`ignored-workspace-core-regressions.log`），fact/Goal/完成门 **389 passed**（`ignored-workspace-fact-goal-regressions.log`），组有重叠，不能相加当独立需求。覆盖同 mtime 写入、创建/删除、force-added 文件、外部父目录修改、info/exclude、两种后台槽安装、ignore 边界变化及旧 Git-only 基线失效。`ignored-workspace-green.log` 是一次新字段漏补导致的编译失败，不计通过；`ignored-workspace-green-rerun.log` 的中间两例不替最终预算/读取失败回归。普通 Git 根内单独 ignored 后代的完整覆盖，以及文件索引跨进程持久恢复，仍未完成。

relay 的单次接纳曾因默认自动跳转实际到达两个服务，`relay-redirect-red.log` 有实际目的端计数 1（期望 0）。新增显式 `ReqwestHttpClient::without_redirects()`，复用原构造设置、返回可处理的构造拒绝；relay 单次构造并复用它。JSON/SSE 的目的端零请求、307 原样作为失败层返回。默认/shared MCP builder 的跳转行为保持，**66 passed**（`single-hop-http-mcp-regressions.log`）。闭集拒绝原因仅保存类型计数，不保留正文/密钥；`relay-rejection-reasons-red.log` 先证明原证据缺失。零外发、错误认证/路径/framing、空与不完整连接、预算与类型往返控制已进入最终基础测试。中间 `relay-single-hop-rejections-green.log` 是测试 sum 类型推断编译失败，不计通过。

默认与 headless-search 四 target 都是 **49 / 2 / 38 / 40 passed，3 / 2 / 5 / 5 ignored**，各自去重 **57**，不是两档相加、更不是 130-case 覆盖率。见 `ignored-relay-default-targets.json`、`ignored-relay-default-unique.json`、`ignored-relay-headless-targets.json`；feature 的 compiler artifacts 独立记录。默认 stdio **3 Rust tests / 10 logical cases** 和 unattended **8 passed**；未加 Git ceiling 的真实 ignored-root unattended 变体另 **8 passed / 21.34 秒**（`ignored-root-unattended.log`），不回写旧的错误 fixture 结果。依赖边界 **6 passed**、配对交付 **10 passed**；scoped fmt/diff 通过。全 workspace fmt 最终仍检出其它任务的 Gateway 三文件、Studio 四文件，见 `ignored-relay-workspace-fmt-final.log`，未修改这些文件。

最新 H1 `rust-scripted-c558ae9b-2bb8-422d-a244-674bbb318750` 的内部工作流通过：6 主 + 3 辅助生成，raw/ACP 人工计数逐一相同共 18，初次失败码 1 与模型所见一致，经过精确合同恢复，最终 bound verified/passed、当前产物与独立 VM 通过；本地拒绝 **0**。工作阶段 **31.707 秒**，Rust 测试 **227.01 秒**，不是提速比较。实际执行的冻结 pair/runtime/tester 均前后稳定。

该次的**外层资格没有通过**：外部观察器仅按父进程和可执行路径选择，误收了冻结 ACP 的 build-id 探针；其 cwd 为仓库根，之后没有继续采集会话 cwd。另一个独立审计发现，准备期间 canonical ACP 已被其它构建替换；实际冻结 ACP 匹配结束时 SHA，而不匹配启动前登记 SHA，虽然源码编码始终为 `626c36317b6a593ed2f152a46b53523bf198dee5`。不能拿 source ID 相同证明实际二进制/feature 相同。原内部 pass、外部断言失败、初次 canonical 稳定断言失败分别保留；详见该 cohort 的 `witness-adjudication.json` / `usage-audit.json`、`ignored-root-h1-launch.json` 与 `ignored-root-h1-audit-canonical-drift.log`。没有启动第二次 attempt，也没有把原 case 改成完成。

用户已将目标明确为“继续深度完成a3s-code架构优化，特别是长程任务不可死循环”。已有 130-case / 12-journey 资料保留，未执行状态不改写。当前优先收敛到统一运行内核的进展、累计预算/期限、取消与恢复边界；同时先绑定不可变候选输入，排除身份探针对外部会话观察的干扰，避免后续 E2E 再混入并发构建。完整 Flow 属于可选 dynamic-workflow，已核对默认与 headless-search 的 Core compiler features 均未启用它；常驻 Workflow facade 复用现有 AgentExecutor，不能只由模块存在推定它导致循环或 token 消耗。本轮没有真实模型付费调用、提交、推送或部署。

## Automatic Goal deadline during its original Run

本轮确认 Core SDK 的绝对 Goal 期限此前只在自动 slice 接纳时检查，不能停止已开始的主模型或完成判定调用。会话没有 max_execution_time 时，两种实际 pending 模型分别超过 3 秒期限仍存活；`goal-deadline-red-behavior.log` 为 **0 passed / 2 failed**，`goal-deadline-red-solo-{1,2}.log` 分别独立复现。最初误用 GoalTickDecision 变体，以及修复中 Box tuple 解构的编译失败分别保留于 `goal-deadline-red.log`、`goal-deadline-green.log`；不当作行为红例或通过证据。

共享 original owned worker 现在从自动 Goal tick 的原始受信 policy 取得绝对期限，复用已有宿主时钟，以同一个 Run 子取消 token 通知主模型、工具和 Goal 完成判定。它等待原执行的结算与清理后才返回，不用定时器伪造终止证明或释放 lease。通过原始 Created handle 向该 Run 保留一次 `goal.deadline_expired` 诊断；流式接口同时尝试有界发送。取消不会上升为 Session 取消；人工 send 不受自动 continuation 的过期 policy 限制。没有新增公共配置、依赖、工作流引擎、循环次数限制或后台定时任务。SDK tick 文档同步说明运行中的期限。

六项新增控制覆盖主模型 pending、Goal 判定 pending、流式无成功 End、工具 pending 及原 future 已释放后同会话再次人工调用、已过期自动 policy 不妨碍人工调用、合法 preview 后延迟 handoff 至已过期时不能触达模型。原始 Goal slice 中断结算、未知 usage、无伪造步骤接受、到期后不可自动再接纳均验证。`goal-deadline-final-focused.log` 为 **6 passed**；最终 Goal facade `goal-deadline-final-goal.log` 为 **66 passed**（包含这六项，不相加）。运行接纳/所有权 `goal-deadline-admission-regressions.log` 为 **65 passed / 1 ignored**；会话生命周期最终串行 `goal-deadline-owned-serial.log` 为 **85 passed**。

此前两线程会话组 `goal-deadline-owned-regressions.log` 为 **84 passed / 1 failed**：五秒 close drain 用例在 TraceCapture::last 找不到 memory_retention 事件。原失败保留，SOLO `goal-deadline-owned-solo.log` **1 passed**，随后整组串行 **85 passed**。尚未证明并行缺失事件的根因，不把它记成产品缺陷已修复或整组并行已通过。依赖边界 **6 passed**、静态 CLI 配对交付检查 **10 passed**；这不代表已经重新构建二进制。当前路径 scoped rustfmt / diff check 通过；全 workspace fmt 仍只有其它任务的 Gateway 三文件和 Studio 四文件差异（`goal-deadline-workspace-fmt.log`），未修改它们。

记录与命令见 `goal-deadline-validation.json`。当前 Core 生产源码已有变化，旧 CLI pair 不再视作最新构建：没有对本次修改重新构建 paired CLI、运行 ACP/H1、重放 Terminal-Bench 或发起付费模型调用，没有提交、推送或部署。整个架构目标保持 active。重复收据的新颖性、恢复的累计预算/期限、发布后辅助调用的存活期，以及不遵守取消的宿主回调仍需独立审查；当前取消后仍等待原始清理的设计不能被宣传为可强行抢占任意宿主 Rust 插件。

## Slice measurement and original observation novelty

上一目标回合是 progress：已修复运行中的自动 Goal 期限。本轮新确认进展计量的三个问题：末尾 turn 未进入 slice 汇总；每轮覆盖 slice 的不确定性起点，丢失净变化；重新投影收据时清空 applied 集合，重复增加进展计数。`slice-measurement-red.log` **6 passed / 2 failed**，两项各自 SOLO 仍失败；`receipt-progress-replay-red.log` 单项实际为 4（期望 1）。原失败保留。现在结算先吸收末尾 turn，不确定性保留 slice 起点；独立的 receipt 计量集合不随状态重放清空。

最初 SDK 红例 `information-novelty-red.log` / `information-novelty-red-two-turns.log` 在第三段前已经停滞，证明的是漏计导致过早停止，不能据此宣称原版本已经因重复信息无限续跑。仅修末尾计量后的中间实现 `information-novelty-after-measurement.log` 才观察到每段两个 receipt 被计为 9，并在三次重复后仍 Advance。`information-novelty-green.log` 虽名含 green 仍失败：系统追加的 repeat 提示使首段的模型可见消息出现两种，计数为 2 而非原始观察的 1。这些中间结果不覆盖、不归咎模型或 LoopX。

现在由原始受信 preparation、effective args digest、结果和原始观察摘要计算新信息；宿主在追加重复提示之前捕获摘要，输出正文、UI 提示和完成绑定保持原语义。元数据中的同名字符串不能提供这个宿主字段。完整原始 fact fold 验证准备/终止配对，Denied/Unknown/缺配对/损坏摘要/外部 session 不获正向信息额度；缺摘要的旧收据走既有模型投影。摘要只写入内部原始记录，并从主模型和 Goal 判定上下文排除；新增测试核对模型消息不变及实际 SDK 两类调用的全部输入没有该字段。不新增依赖、产品配置或工作流机制。

只调整新结算的计量，历史 fact fold 的 V1/V2 解释未改写。新的已接受步骤提供一次有事实绑定的计划进展，防止读取相同数据但完成真实步骤的任务被误停。实际 SDK 对照验证：同样的观察在不同 Run/receipt 身份下的额度为 **[1,0,0]**，两次无进展后 NeedsInput，Memory/File 重开后仍不可续跑；内容变化再回访为 **[1,1,0,0]**；重复信息但真实步骤被接受为 **[(1,0),(0,1),(0,1)]**（信息、计划），最终 Goal 完成。这里是受控 client 的真实 SDK/事实/存储链路，不是真实模型或完整 ACP 进程资格。

首次 Goal 回归为 **68 passed / 1 failed**，失败单项 SOLO 复现（`information-novelty-goal-regressions.log` / `information-novelty-legacy-solo.log`）。旧历史测试当场调用当前运行器构造旧记录，修正计量后已不再生成旧的全零结算。现在只在该私有 fixture 中明确冻结旧结算值并重新生成有效 digest，再验证旧接纳及 Memory/File 重开；生产日志没有截断或修改。原测试失败保留，未以放宽生产判断或篡改原数据修复。

最终 **Goal 69 passed**（`information-novelty-context-final-goal.log`）、**fact/receipt 188 passed**（`information-novelty-final-fact-rerun.log`）、**harness 78 passed**、**Goal control 51 passed**、**会话生命周期 85 passed**、**loop checkpoint 13 passed**、**session checkpoint 20 passed**；各组和前一轮重叠，不累加成需求覆盖率。新增 9 个测试名。`information-novelty-raw-observation.log` 的导入/错误转换，以及 `information-novelty-final-fact.log` 的测试 Message 未实现 PartialEq，均为编译失败，不计行为通过；后续修正日志独立保留。

依赖边界 **6 passed**、静态配对交付检查 **10 passed**，不是 paired CLI 已构建。scoped rustfmt/diff check 通过；全仓 formatter 仍只有其它任务的 Gateway 三文件与 Studio 四文件差异（`information-novelty-workspace-fmt.log`）。命令、范围和当前文件摘要在 `information-novelty-validation.json`。未提交、推送、部署、发起付费模型调用或重建 CLI；旧 pair 继续明确标记为不匹配当前 Core。

目标保持 active。当前计量只证明机械身份的新颖性，不能替代语义价值判断；原 mutation-progress fold 仍按出现过的变化计量，净零变更和工作区状态往返需版本化审查。恢复累计期限/usage、发布后辅助调用、宿主回调取消边界，以及 immutable candidate + actual ACP/H1 均未完成；不声称已经消除所有长程空转或取得性能改善。新私有检查点字段不构成旧二进制可无损重写新检查点的承诺。

## V3 net workspace progress and repeated-state control

真实 SDK 的净零写入测试先失败：同一 Run 写入临时内容后恢复原字节，旧计量仍给予自动续跑机会（`net-progress-sdk-red.log`）。新的 `turn_progress_delta_v3` 从原始、已验证且完整的工具/后台终止变更重建工作区观察切面，折叠同一路径的首次 before 与最终 after，并记录相对基线的稀疏状态摘要。单 slice 写后还原、创建后删除以及后续重复回到已观察状态，不再独自刷新 mutation 进展。缺失、不完整或不连贯的观察不能获得这项进展；这不是全工作区冻结或语义收益判断。

新接纳显式记录 V3；缺省、V1、V2 的历史接纳解释保留。原始 accepted step 可证明计划进展，裸 mutation/plan 数字不能替代原始证明。Memory/File 两种存储覆盖净零写入、A→B→A→B 和重开；合法 V2 历史仍可重放，再转入 V3 后按新规则停止。原日志没有改写，旧二进制读取 V3 的降级兼容性未获资格。

新增 10 个 Core 测试名。V3 阶段 Goal control **59 passed**、Goal facade **71 passed**、fact/receipt **188 passed**、harness **78 passed**、会话生命周期 **85 passed**。各组重叠，不累加；后面的当前 Goal **72 passed** 包含这组 Goal 控制。初次控制组导入不公开枚举导致的编译失败仍在 `net-progress-control-regressions.log`，不计行为通过；修正只在测试 fixture 中明确历史 V2。详情见 `net-and-timeout-validation.json`。

## Immutable paired candidate and fixture adjudication

测试侧新增闭合 candidate manifest，绑定两份 CLI 的绝对路径、字节数、SHA256 和共同 build-id。显式候选无效即拒绝，不回退到 canonical 路径；冻结副本重新校验内容。五项边界测试覆盖同 source-id 的不同字节、部分 pair、选择后修改、未知/过大 manifest、符号链接和非普通文件。它是测试输入记录，不是新增产品配置。实际 Driver 在 spawn 后记录 PID，外层观察器不再按文件名猜测 build-id 探针。

首份冻结 pair `0c714f14ab5005264fc1a6887a59c12a802b5565` 的实际 unattended 套件为 **13 passed / 2 failed**（`net-pair-unattended_e2e.log`），两种失败均 SOLO 保留。Unix socket 用例在长 TMPDIR 下于 bind 阶段触发 Darwin SUN_LEN；相同冻结测试程序在短专属 TMPDIR 下单项通过（`net-solo-artifact-short-control.log`），没有跳过非普通文件检查。另一个超时失败是下节修复的产品适配问题。

原 stdio 套件达到外层 300 秒限制，没有得到协议失败证据。同一冻结程序 SOLO 完成 **10/10 协议场景**，整次 318.807 秒（`net-solo-conformance-red-result.json`）；此前观察到测试进程消耗 CPU 并打开 candidate/aster，反复进行 debug SHA 校验。此结论推翻“协议卡死”的推断，但不构成每阶段耗时分解。新回归事先声明 420 秒外层套件预算和短 TMPDIR；产品四秒 deadline 未放宽。过晚执行的 native sample 发现原进程已经退出，没有取得栈样本，保留状态日志，不充当性能证据。

## Typed deadline cause through ACP and current paired regressions

实际 ACP 单项复现了 Core 到期后返回 `next goal slice: Session error: goal.deadline_expired`，却变成 -32603 的问题（`net-solo-deadline-red.log`）。Core preview 和 original worker 现在通过已有 Internal error 容器保留 `GoalControlError::Refused(DeadlineExpired)` 及 source chain；ACP 按该类型或宿主自己的 ExecutionDeadline 分类，而非匹配自由文本中的 deadline。早先可恢复工具错误不能掩盖最终到期原因。ACP 正常结束协议回合，同时如实记录 timed_out、deadline_exceeded 和非零 exitCode，未将超时记为任务成功。

包装上下文的单测先红后绿；新增三项测试覆盖 Core 无模型调用的 typed preview、ACP 上下文链和纯文本伪装。当前结果分类 **20 passed**、整个 unattended 单元组 **31 passed**（包含前者）、Core deadline **7 passed**、完整 Goal facade **72 passed**、串行会话生命周期 **85 passed**。日志以 `acp-goal-timeout-` 开头，均不累加成覆盖率。

两份 CLI 已从根 workspace 一起重建到私有 target，并冻结为 `aster-cli-pair-v1:478e8c562ed739c0bf5ad8020b606fd2ee854be1:aarch64-apple-darwin`。Aster SHA256 为 `611a5680c9a6c0c2546171be8545edbf35037e1edceb8fc08b1650c99b0482bf`，ACP 为 `ff05d8cbb46ef544956c8e41b512c39f60b2417f2e893c6fdcfb0d6f8177f052`。见 `timeout-pair-candidate-reference.json`。实际 compiler artifacts 证明 Core 采用默认 local-code 档，未启用 dynamic-workflow / advanced-harness（`timeout-pair-feature-profile.json`）。构建前后 5,667 个本地 Rust/manifest/build-control 文件一致；不扩大为全部传递、native、生成物和环境的可复现性证明。

新的冻结 pair/tester 实际运行：超时 SOLO **1 passed**，结果记录 durationMs=3747；完整 unattended **15 passed**（8 个产品流程、2 个 artifact、5 个 candidate 测试）；stdio **10 Rust tests passed，其中协议 10/10 logical cases**。分别见 `timeout-actual-*-result.json` 和 `timeout-conformance-conformance.md`。全部候选、manifest、实际 tester 和所记录源码前后保持一致。原始静止日志的 6 个 Goal 会话共 10 个 slice，全部采用 V3 并结算（`timeout-stdio-goal-basis.json`）。停滞对照在 4 个 slice、14 次本地模型调用后结束为 needs_input / failed，没有虚报 Goal 完成。

## Current H1 with actual process witness

`rust-scripted-858048a0-8a27-4ecf-a8fd-a7d323da0227` 的受控 H1 完成并通过外层审计。它使用上节登记的冻结 pair 和冻结实际 tester；独立 OS 观察确认 Driver PID=65264、父进程=63985、真正执行文件及 SHA 正确、cwd 位于专属 ignored workspace，排除了 build-id 探针。运行前后候选和源码一致，运行时匹配已资格化 assets 摘要 `db85f2c179d2dc383b29a26d2a6f4738f7d25f6d93387a13f6a43ade465cc902`。旧 c558 外层失败记录保留，未被覆盖或改判。

初始真实测试失败、模型可见退出码 1、复合命令拒绝后的精确合同引导、裸命令成功、bound verified/passed、ACP 清理后的独立 VM golden 验收均通过；保护文件未改动。8 个工具、7 次授权请求，工作阶段 29.415 秒，全测试 215.816 秒。9 次生成（6 主 + 3 辅助）均 HTTP 200，raw/ACP callSequence 1–9 的用量逐一相同，人工固定总量 18；relay 零拒绝、零传输失败且已关闭。原 ACP PID 和进程组在 join 后均不存在。19 项审计检查与原始输出见 `timeout-h1-independent-audit.json`、`timeout-h1-os-witness.json` 及 cohort 的 result/wire 文件。人工计数、单次 H1 和准备 VM 的时间均不代表真实模型计费、性能 A/B 或完整原始用例资格。

依赖边界 **6 passed**、静态配对交付 **10 passed**，scoped rustfmt/diff check 通过。完整 workspace formatter 仍检出其它任务的 Gateway 三文件、Studio 五文件差异（`timeout-workspace-fmt.log`），未修改这些文件。本轮未部署、提交、推送或新增付费模型调用。

整体目标继续 active。剩余边界和确切后续检查记录在 `remaining-liveness-boundaries.json`：宿主确认回调的取消、原始 checkpoint/store/export 的结算、重开时累计预算和绝对期限、持续活跃异步工作的 deadline 公平性，以及普通 ignored 子路径与持久观察恢复。它们目前是需验证的边界，不能用 pending 调用测试或这次 H1 代替。完整真实模型、TUI/GUI、故障矩阵和 soak 仍未完成。

## Confirmation callback cancellation and active-work deadline

本轮上一 goal turn 判为 progress：typed deadline 原因、冻结 pair 与实际 ACP/H1 资格都改变了权威状态。本轮继续检查仍未资格化的回调边界，没有把前一轮通过当成整个目标完成。

两项真实 SDK SOLO 分别复现：provider 在 request_confirmation 注册中挂起，以及取消时 confirm/cancel 回调挂起，都会让自动 Goal 到期后仍无法结算（`confirmation-registration-behavior-red.log`、`confirmation-cancellation-red.log`，各 1 failed）。测试会释放自己设置的阻塞后回收 owner；没有靠遗弃测试进程判通过。初次 `confirmation-registration-red.log` 只是测试误用了 Result 别名导致编译失败，不计行为 red。

Core 原始/恢复/子 Run 的 host-input 等待，以及共享 ToolConfirmationRuntime，现将注册、答复和事件等待整体置于同一个 Run 取消范围。已经取消的调用不注册，也不取消同 ID 的既有请求。取消只清理本次精确 tool id；共享取消 helper 最多等待 provider 一秒，超出宽限即丢弃该 callback future，报告 `confirmation.cancel_unresolved`。这不是许可超时自动批准/拒绝，也不证明宿主清理完成；外部 pending 可能仍在，需宿主核对。公开 Rust 方法签名、产品 ACL、依赖和权限边界不变，provider 文档明确 abandoned future 不能稍后授予许可。

部分修复曾错误地使用 AgentEvent::Error 表达清理未确认：record_event 会提前把 Run 变成 Failed，原始取消结算随后无法通过所有权证明，guardian 正确保留 lease。该批为 **1 passed / 1 failed**（`confirmation-callbacks-green.log`），失败 SOLO `confirmation-cancellation-partial-solo.log` 也保留；其诊断记录说明原始 fact 已结算、provider future 已 dropped，但 owner 未获合法退役。现在改用已有非终止 ConfirmationTimeout(cancel_unresolved)，没有放宽原始 writer、join 或退役证明。最终两项 SDK 控制通过，Run 为 Cancelled、工具未执行、future 在 retirement 前释放、无关 pending 未受影响；清理未确认时仍能观察到原 pending，且不会虚报 ConfirmationReceived。测试宿主明确核对该 pending 后，同一会话可继续。

ExecutorTools 的事件发送同样响应当前 Run token：满通道的尾部可取消，已排队前缀与 FIFO 保留，有空位的发送仍可观察。新增控制还覆盖共享确认协议的挂起注册/取消、事件拥塞、预先取消且同 ID 有既有确认。正常批准/拒绝、通道关闭、定时器不授予许可的旧六项协议测试继续通过。另有持续活跃模型/工具回合控制：多个各自有界的工具调用在截止前完成，绝对期限仍停止原始 Run；没有成功 End、没有接受 Goal step，结算一次且 retirement 后不再生成。它证明 cooperative async 活跃工作下的 timer，不是任意同步阻塞 Rust callback 的抢占证明。

本轮共新增 **8 个 Core 测试名**。最终 Goal facade **78 passed**、fact/receipt **190 passed**、共享确认协议 **6 passed**、串行会话生命周期 **85 passed**；各组重叠，不相加。命令、原始 red/partial/final、文件摘要与资格范围见 `confirmation-validation.json`。确认回调/事件组的单独 5/2/6 passed 是其中重叠控制，不额外计数。

两份 CLI 再次一起构建并冻结为 `aster-cli-pair-v1:0fc6d8cf21cb09f0e8cdcc3dc52d68e52c7f74b2:aarch64-apple-darwin`。Aster SHA 为 `f296c529e4fb6c618f5b1e6241c0d4d3672477c3ff8f186ee533e88396e6e45c`，ACP 为 `82ff025e9a4d0ff9006c08d832ac5e0bd4ae010eb5706e62443fa5a34f1a0d2a`。见 `confirmation-pair-candidate-reference.json` / `confirmation-pair-feature-profile.json`。实际 default Core 未启用 Flow 高级档；5,671 个已记录本地源码/manifest/build-control 文件前后相同，不能扩大为全部 native/传递环境冻结。

新冻结输入的实际 unattended **15 passed**；stdio **10 Rust tests passed，其中协议 10/10 logical cases**。原始静止记录仍为六个 V3 Goal session、十个 slice 全部结算，停滞对照为四个 slice、14 次本地模型调用后 needs_input / failed（`confirmation-stdio-goal-basis.json`）。候选、manifest、实际 tester 和已记录源码前后保持；没有拿旧 478e8c pair 代替本轮构建。

当前 H1 `rust-scripted-a0cf7439-6c08-43b4-82b1-820f95bdc230` 通过原始失败基线、精确命令修复、bound verified/passed、独立编译 VM golden 验收和外层 19 项审计。实际 Driver PID 85759 / 父 84269 的 executable、SHA、专属 ignored cwd 均经 OS 独立核对，join 后 PID/进程组不存在。运行时保持已资格化摘要；9 次生成（6 主 + 3 辅助）全部 HTTP 200，raw/ACP sequence 1–9 用量逐一匹配，人工固定总量 18；relay 零拒绝/传输失败且已关闭。8 个工具、7 次授权，工作阶段 29.112 秒、全测试 216.149 秒。见 `confirmation-h1-independent-audit.json` 和 cohort 原记录；不代表真实模型成本或性能改善。

scoped fmt/diff 通过，依赖边界 6 passed、静态 CLI 配对交付 10 passed。完整 formatter 仍只有其它任务的 Gateway 三文件、Studio 五文件差异。没有部署、提交、推送或新增付费模型调用。目标保持 active：checkpoint/store/export、恢复累计预算、其他 provider 策略回调、单个 Run 内重复无进展的提前退出，以及 ignored 子路径/持久恢复与长期故障资格仍待推进。当前重复结果反馈函数明确不阻塞，单 Run 是否会一直依赖最后期限须独立 SDK 复现；这只是需验证的源码边界，不能称已确认无限循环。

## Parent Run repeated-refusal containment

原始 SDK 行为 red（`refusal-cycle-sdk-red.log`）显示：一分钟可信截止时间、无限工具轮次下，一秒内模型调用32次、物理工具仅3次，Run仍未停止。已有重复工具保护确实拒绝了执行，却没有阻止下一次付费推理；因此不能把这些失败全部归为模型能力。

新增私有推理前检查复用原始 typed prepare/result 及完整性 fold，绑定当前 Run/thread/capability。相同有效调用在没有新原始信息或已知工作区变化时连续被拒绝，下一次模型调用前以 `run.tool_refusal_cycle` 失败，由原 owner 正常结算；无假成功、无强退 lease、无新增全局轮次上限。新用户输入、原始新信息和已观测变化可恢复机会，换 call ID、拒绝文案、压缩或外来 Run 不提供进展。SDK green为5次模型/3次物理调用，提前停止；三次必需重复检查和拒绝间获取新信息的正例通过。首次green编译曾因InvalidDigest无Display失败，原日志保留，不计行为red。

8项新Core具名测试、1项实际ACP测试。该阶段Core Goal81、Fact195、Ownership85通过，组间重叠。冻结配对c5f788（`refusal-pair-candidate-reference.json`）由根构建产生，5675项本地编译源码/清单/控制输入前后相同，默认local-code未启用Flow；非完整native/环境可复现证明。实际ACP unattended16项（9条产品流程+7辅助）、协议10项/10逻辑用例全部通过；交互式重复拒绝观察为5次模型、3成功+2拒绝receipt，完整套件prompt247ms，SOLO259ms。6个Goal会话10个slice使用V3且全部结算。

受控真实ACP+编译VM H1 cohort29b17332通过19项独立检查，包括精确失败/通过命令、原始exit code、黄金结果、保护文件、真实Driver PID/cwd/executable、原始HTTP与ACP usage逐条对应，以及退出后PID/进程组不存在。流程28165ms，整个tester207.564s；9次脚本生成合成18tokens，不代表真实模型成本。详见 `refusal-h1-independent-audit.json`。scoped格式/差异、依赖6项、配对交付10项通过；workspace formatter保留其它任务的差异。

**范围限制：目前只保护带原始admission的父Run。task/skill子Run有独立日志和不同所有权，尚需单独修复；无denial的成功工具空转、A↔B净零修改也未完成提前终止。** 此后新增子任务测试，c5f788保留为已资格化阶段，不再称为所有当前源码的构建。未提交、部署或发起新增付费模型调用；总体目标active。

## Owned child refusal cycles and pending policy queries

本轮先复核父Run阶段的权威结果，补齐 `refusal-validation.json`、受控H1的19项独立审计和6会话/10个V3 slice证据。随后的源码核对发现task/skill共用ChildRun具有独立日志、无父Run admission；父Run保护不能直接套用或伪造其身份。

真实ChildRun SDK在无限轮次下，一秒42次模型调用、3次物理执行后仍未停止（`child-refusal-sdk-duplicate-red.log`）。私有共享fold现仅在宿主明确创建、独占fenced的子日志模式下接受无admission原始receipt，旧的裸FactRun/缺失父RunID不会自动获得该权限；模型改变call ID或诊断文本不提供进展。green为5次模型/3次物理调用并如实失败，三次必需重复检查仍可完成；SDK task桥接共7次调用，子失败到达父任务且Goal未被完成。原始新信息/输入及外来admission隔离控制通过。最早fixture遗漏composed checker，46次模型/0次物理的拒绝记录也保留，修正fixture后才计重复执行red。

另外四项真实SDK分别复现确认要求、准备阶段复查、确认可用性和policy查询挂起。批次0/4后，每个不同失败均SOLO重现（`policy-callback-*-red-solo.log`）。这些纯查询现在由原Run取消token包围，availability/policy在工具自声明确认入口同样受控；取消只丢弃该查询future，既不执行工具也不制造许可。四项green都在可信deadline结算为Cancelled，future在退役前释放，模型只调用1次、物理工具0次。原有未知外部确认清理、owner join与lease退役证明保持不变；未添加工作流引擎、配置、依赖或公开Rust API。

本轮新增9项Core具名测试和1项实际ACP测试。完整Goal88、Fact197、会话所有权85、任务工具158（1项原有忽略）、子任务权限29、共享确认6通过；组间重叠。ACP fixture-only授权修正前后，Core与根构建控制输入完全一致，记录在 `child-policy-auth-core-regression-reuse.json`，未无故重跑相同Core组。

实际ACP的首个新增SOLO失败是测试客户端默认取消Task许可，原日志只有approved=false而无tool.prepared，不能算子任务执行或产品失败；`child-policy-actual-child-result.json`保留。只让该用例的私有客户端选择host提供的AllowOnce选项，其他用例继续默认取消。最终新配对 **ae4eb67e** 在根工作区同时构建CLI/ACP并冻结，5677项本地Rust/清单/控制输入前后及测试期间相同，Core仍为默认local-code、Flow未启用。真实子任务SOLO通过：5次子模型+2次父模型，3成功/2拒绝原始子receipt，父Task的原始failed结果包含明确拒绝原因，source.txt不变；prompt627ms（完整套件630ms）。独立receipt审计6项通过。

最终同一配对实际unattended **17项**（10产品流程+7辅助）、协议10项/10逻辑用例全部通过。6个Goal会话10个V3 slice均结算。受控ACP+真实编译VM H1 cohortd80d6a17的19项独立检查通过：精确失败/通过命令、原始exit code、黄金结果/保护文件、实际Driver PID/cwd/executable、逐条原始HTTP/ACP usage和退出后PID/进程组缺席。流程29387ms，整个tester212.977s；9次脚本生成合成18tokens，不能外推真实模型成本或提速。修正fixture前的52edfff8/H1证据同样保留为历史阶段。详细命令、counts、candidate/tester引用见 `child-policy-validation.json`。

scoped格式/差异、依赖6项、配对交付10项通过；全workspace formatter仍为其它任务的Gateway三文件与Studio五文件差异。本次故障定位在当前FactRun的推理/工具和取消边界，未证明历史LoopX内化就是根因。**仍未完成**：原始持久化/导出挂起、恢复累计预算/期限、无denial的成功空转和净零循环、完整Skill/子后台进展与确认晚答/复用ID、ignored子路径及长期故障资格。新逻辑没有保证不yield的同步宿主代码可被抢占；不强退未证明的owner。没有提交、部署或新增真实付费模型调用，总体目标继续active。

## Successful observed-state cycles and pending durable ownership

上一goal turn属于progress：子Run拒绝循环、四种查询挂起和ae4eb67e实际配对资格都有权威证据。本轮继续处理原始持久化边界，并独立复现了不含任何denial的成功写入空转。

**SDK行为red**：可信60秒期限、无限工具轮次下，3秒内模型调用24次、物理写入23次，始终A↔B且Run仍活跃（`net-state-loop-sdk-red.log`）。Effect层每次观察到修改就重置重复窗口，因而每次工具都成功也能持续计费。Core现在复用V3的原始工作区状态指纹，按当前Run/thread/capability或宿主独占子日志折叠；重复旧状态且没有新原始信息时，复用已有duplicate allowance，下一次生成前以 `run.tool_state_cycle` 失败。没有新增全局轮次上限、公开API、配置或工作流引擎；V1/V2/V3历史序列和哈希语义保持。快速候选检查按原始tool+有效参数计数，普通不同修改不进入完整状态fold。

**green与正例**：默认额度3时，8次模型/8次成功物理写入提前停止，denial为0；owned ChildRun相同。三次必需往返、12次相同参数但真实新状态、已有额度8下12次有界写入均可完成。正例使用独立宿主字节检查并绑定当前effect digest，没有放宽完成门。首次正例缺少bound verification、错误假定SDK builder的0表示disabled（原实现钳为1）、随后测试类型导入编译错误，以及achievement fixture多出的reason被闭集schema拒绝，所有失败和各类SOLO都保留；修正的是fixture，未调整完成或schema规则。

进一步SDK反例发现：外部进程在工具调用之间写入新before-state时，旧的闭环判断会误报（`state-cycle-external-sdk-red.log`）。现在循环推断还要求原始前后观察连续；不连续或同receipt内部端点冲突时不能宣称闭环。加入独立外部写入控制后，12次有界操作可完成；原有原始信息、新用户输入、外来绑定、harness-only路径、未知/不完整receipt控制均通过。**限制：一次连续性缺口会让本input保守退回可信deadline；后续重新建立连续窗口仍待实现。**

**原始持久化边界**：Store/Export挂起时，到期前后原始callback仍持有owner，后续调用应为精确SessionBusy；不能用丢弃future或强退lease伪造停止。此前到期诊断也被最终join挡住，两项批次red后分别SOLO重现。现在原始可信deadline先记录非终止BudgetThresholdHit(goal_deadline_unix_ms)，再继续等待真实结算；不是提前Error/End。Memory/File各执行Store+Export四种情形：等待中模型只调用1次、future未提前丢弃、Run未虚报terminal、deadline notice可经SDK run_events观察；明确释放测试宿主回调后实际写入返回、owner完成join、Run为Cancelled且仅1个settled slice。**这证明可观察的到期及真实所有权，不证明任意宿主持久化/导出有界返回，也未资格化ACP UI展示该pending notice。**

最终新增15项Core具名测试、1项实际ACP测试。当前Goal96、Fact204、Goal-control56、会话所有权85、admission65（1项原有忽略）、checkpoint准备13通过，组间重叠。最终Root构建同时冻结CLI/ACP **0cc82113**（`state-coherent-pair-candidate-reference.json`），5679项本地Rust/清单/构建控制输入在构建及测试期间一致；默认local-code未启用Flow。相对上一阶段有15项Code路径变化，另有共享工作树既存CLI scrollback文件变化纳入构建，不代表TUI功能资格；记录在 `state-coherent-scoped-source-change.json`。中途state-cycle初次构建未做实际资格即被外部输入修正取代，保留为未资格化历史输入。

真实注册ACP的成功写入闭环SOLO通过：8次原始model.turn、8个Succeeded原始write receipt、0拒绝、同一admission、连续before/after和仅2种after-state，原始审计8项通过，最终字节A；prompt2537ms（完整套件3128ms）。随后同一冻结pair的实际unattended **18项**（11产品流程+7辅助）、协议10项/10逻辑用例全过；6个Goal会话10个V3 slice均结算。全tester358.868s/363.22s包括大debug输入身份校验及独立fixture启动，不代表任务性能。

受控ACP+编译VM H1 cohortba9e3edf通过19项独立检查，包含精确失败/通过命令、原始exit code、独立黄金结果和保护文件、实际Driver PID/cwd/executable、逐条原始HTTP/ACP用量、最终PID/进程组缺席。流程33196ms、whole tester233.153s；9次脚本生成合成18tokens，不能外推真实模型成本或提速。详见 `state-cycle-validation.json` 与 `state-coherent-h1-independent-audit.json`。scoped格式/差异、依赖6项、配对交付10项通过；全workspace formatter仍有其它任务Gateway三文件、Studio五文件差异。

总体目标仍active：持久化取消/真实结算契约、恢复累计预算/期限、连续性重新锚定、不同拒绝参数/模型派生输出的循环、Skill/子后台/ignored子路径和长期故障资格尚未完成。本轮没有重判旧Terminal Bench具体样本的根因，也没有证明历史LoopX内化就是根因。没有提交、部署或新增真实付费模型调用。

## Re-anchored observations and persisted host-policy controls

本轮仍为progress，未完成整体架构目标。上节0cc82113为历史资格阶段；其中“一次连续性缺口后永久退回deadline”的限制已由本轮修复，旧记录保留。

**独立SDK red**：宿主仅在第二次模型调用时外部修改一次source.txt，之后工具继续A↔B。parent和owned child各在3秒内生成18次，仍未停止；可信deadline为60秒，不能把等待deadline算作及时制止。根因是MutationProgress把一次不连续永久保留为incoherent。现在原始连续性按最近一次观察判定；缺失、内部冲突或前后缺口清除旧重复额度和cycle，随后原始连续receipt重新建立窗口。已有状态历史不清空，V1/V2/V3序列及哈希语义不变。未新增公开API、配置、全局轮次上限或工作流。

**green及正例**：外部修改后的parent/child各9次模型/9次成功物理操作停止，无denial；旧无缺口案例仍8次。必要往返、真实新状态、持续外部输入及原始信息控制保留。真实注册ACP的新增SOLO也为9个原始model.turn、9个Succeeded write receipt、0拒绝、同一原始admission；原始before/after仅在第二次有缺口，随后7个连续receipt形成仅2状态的窗口，最终字节B。独立原始日志审计8项通过；prompt3597ms，完整套件中的同例2660ms。该时间只描述受控fixture，不证明真实任务提速。

**恢复契约**：新增四项公共SDK测试，分别覆盖max_slices、max_no_progress_slices、绝对deadline_unix_ms和automatic_enabled，Memory与重新打开的FileSessionStore各执行三个恢复阶段，共24项恢复观察。先收紧/禁用/等待真实原始期限过期，拒绝阶段均0次模型调用；仅显式可信宿主延长或重新启用可新增一次原始Run；再按累计值收紧时继续拒绝。累计原始admission和usage不重置，前三类信息工具流程的合成用量21→42，无进展纯文本流程2→4。这里证明正常save/close/resume下的策略与记账保留，不证明崩溃提交原子性、时钟回拨或真实provider计费幂等。

本轮新增10项Core具名测试、1项实际ACP测试。最终相关组为Goal102、Fact206、Goal-control58、会话所有权85、admission65（1原有忽略）、task158（1原有忽略）通过，组间重叠。Root同时构建并冻结CLI/ACP **15360782**（`reanchor-pair-candidate-reference.json`）；5680项已记录本地Rust/清单/构建控制输入在构建、实际运行和最终审计时一致，相对上一阶段恰有8项Code源码变化，默认local-code未启用Flow。范围不包含完整native/generated/registry/environment复现。

同一新pair的实际unattended **19项**（12产品流程+7辅助）、协议10项/10逻辑用例全过；原始日志证明6个Goal会话的10个V3 slice均已结算。两个完整tester分别367.539s/354.735s，包含大debug字节身份检查及独立fixture启动。受控ACP+编译VM H1 cohort **a940b208** 通过19项独立检查，流程31423ms、whole tester234.572s；实际Driver PID/cwd/executable、原始失败/通过命令及exit、黄金结果和保护文件、9条原始HTTP与ACP usage、最终进程和进程组清理全部一致。9次脚本生成合成18tokens，不是新真实模型成本证据。详见 `reanchor-validation.json`、`reanchor-h1-independent-audit.json` 和 `reanchor-stdio-goal-basis.json`。

scoped格式/差异、依赖6项、配对交付10项通过。全workspace formatter记录的是其它工作Gateway三文件、Studio六文件差异，未代替其它任务改写。仍待处理opaque持久化回调的有界调用方观察/取消及真实结算契约、授权迟到回复和ID复用、不同参数/模型派生输出循环与恢复故障边界；任意未返回的原始writer仍不能伪造terminal或释放lease。未提交、部署或新增真实付费模型调用，未重新断言历史Terminal Bench失败由LoopX引起。

## Provider preflight（不是 ACP E2E）

通过已有公共 Core client 只生成一个极短回答，不建立用户 Agent，不导入 Codex 账号或使用用户 Session。prism 返回 403，GLM 返回 429，DeepSeek Flash 返回有效 pong（adapter 用量 58 tokens；原始 usage 是否存在未由该探针证明）。详见 `provider-health-results.json`，不据此宣布模型能力上限或 harness 全绿。

仓库可信 ACL 包含显式 Codex 账号导入和外部 Skill 目录，因此后续任务子进程不会直接继承它。真实凭证仅用于测试父进程内存，ACP 子配置使用无秘密的本地 fixture 路由；任何新增 relay 必须有地址/预算/清理自证。

## Explicitly not claimed

130 项运行覆盖率、原 Rust 长程主旅程通过、完整 GUI/多平台/soak 资格、发布可靠性、ACP 行/分支覆盖率、完整传递源码冻结、全 workspace 已绿、生产优化已全部落地、部署或提交。旧真实上游原始 usage 未完整证明，相应原始 tokens 保持 unknown；人工脚本已观测数字不替真实模型背书。用户目标尚未完成；执行阶段继续推进。

## Paired 9.1.5 release checkpoint

The final isolated CLI0.1.3/ACP9.1.5 pair `3c601c086ffef3b3e86a13c0f71d19b49abc3dae` passed controlled actualACP20 (13 workflows plus7 auxiliary), protocol10 and H1 with19 independent checks. Core741 passed with2 existing ignored; all10 original admitted Goal slices settled (4 goals completed,2 intentionally stopped). See `../2026-10-24-v9.1.5/OUTCOME.md` and `final-qualification.json`. These are controlled-model/debug-pair results, not real-model performance or final multi-platform release bytes.

`STUDIO_READ_TOKEN` was saved and metadata verified; no credential value was written to files. Run40101228548 proved private checkout on macOS/Linux/Windows but Kylin failed on Git ownership. The common resolver now trusts only the declared checkout for its two read commands; one hermetic red/green test also rejects the wrong workspace and preserves global Git settings. The glibc probe explicitly selects Bash. Unified source411c2ca8f is building in run40102189209. Public release and credential narrowing/rotation remain unfinished; the broader architecture goal is not complete.
