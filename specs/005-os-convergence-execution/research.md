# specs/005 决策材料与外联草稿

供 owner 拍板 D-A/D-B/D-D 与外发 D-F 时直接取用。证据出处标注于各节；
事实核验日期 2026-10-21。

## D-A 私有 CA 准入策略（卡 T0.4.2 runbook 步骤 1）

背景（`apps/cloud/docs/convergence/t0.4.2-oidc-federation-runbook.md`
L102-113）：生产 OIDC 客户端仅信任 webpki 根；测试联邦的 IdP 证书走私有
CA，客户端拒绝。

| 方案 | 内容 | 安全边界 | 成本 |
| --- | --- | --- | --- |
| a. 产品侧信任链可配置 | OIDC 客户端增加受管 CA 源（如 `x-trust-anchors` 引用 Secret），仅联邦配置显式启用 | 私有根进入生产信任面；必须限定「仅 federation IdP 端点」使用，且 CA 指纹入审计 | 控制面代码改动 + 安全评审；一次投入，后续联邦场景复用 |
| b. 测试联邦换公共 CA | 测试 IdP 用 Let's Encrypt 等公共可信证书（需测试域名可公网解析） | 生产信任面不变；但测试拓扑暴露公网、且不能验证私有 CA 联邦路径 | 运维侧为主；每次环境重建受公共 CA 有效期/速率约束 |

建议：若 D-A 的目的是**验证产品在私有 CA 联邦下的行为**（Phase 0 的
本意），只有方案 a 能覆盖；若仅为打通 runbook，b 更快。ADR 落
kense-labs/cloud `docs/decisions/`，需 owner 明示「私有根的允许范围」。

## D-B 保留运行凭据（卡 FN0 保留 run 与 GA-2 绿 run）

背景（`apps/cloud/docs/fn0-verified-residual.md` L189-198）：FN0.2 GHA
retain run 36190947596 失败于私有 `kense-labs` 子模块默认 GITHUB_TOKEN
不可读；分支 `fn0-verified-gha` 已推待触发。

| 方案 | 内容 | 适用 |
| --- | --- | --- |
| a. GHA 环境 + `ASTER_LABS_READ_TOKEN` | org secret，fine-grained PAT，contents:read 限 `kense-labs/*` | 最小改动；secret 轮换入月度运维 |
| b. 自托管 runner | 常驻机器，天然可配 SSH key 读私有仓 + 后续裸金属/KVM 矩阵（FN0 Verified 的另一残留项） | 一次投入大，但同时解 FN0 宿主矩阵与 W2 保留管道 |

建议：若 D-E（SEV/硬件）倾向采购，b 一并解；否则 a 先行、b 挂 GA-3 前
置。

## D-D pinned postgres digest 不可解析（卡 runbook 步骤 4–6）

背景（runbook L80-83）：pin 的 postgres 镜像 digest 在测试环境无法拉取。
方案：① 内网镜像代理重写 digest；② 重新 pin 到可解析 digest 并记
lockfile 依据。倾向 ②——digest 漂移本身就是 runbook 应发现的缺陷，重
pin 即是修复动作的一部分。

## D-F krun 引导链失败 — Box 仓库 issue 草稿（外发文本）

> 标题：aster-ai-lab/libkrun fork boot chain: VM exits before the guest exec
> server is ready (macOS/HVF, T0.4.2 stack)
>
> 正文要点：定性——这不是上游 libkrun 报障，是**自家 fork** 的引导链
> （`aster-ai-lab/libkrun`，安装版 3.3.0 的运行时可溯源至 fork commit
> `dc5519f`/`2692169b`，见 `aster-libkrun-sys` SOURCE-PROVENANCE.md）。
> 环境（macOS/Darwin，pin 版本）；症状（"VM process exited before the
> guest exec server became ready"，且失败路径清理了 shim/console 日志，
> 无留存诊断）；**第一诉求：失败路径保留 shim/console 日志**；第二诉
> 求：排查 fork 的 macOS 增量（VirtioFS 目录项快照、合成 FUSE resume
> cookie、guest-init 握手）——正是引导早期 guest 侧就绪路径的地基；
> 最小复现引用 runbook 步骤 1–3；可对照树内 vendored 源
> （`crates/box/deps/libkrun-sys/vendor/libkrun`）诊断；关联：收敛计划
> Phase 0 唯一出口项的外部前置（D-F）。
> 附：同栈已知干扰——旧 `aster-os-node-agent.service` crash-loop（50,974
> 次重启，203/EXEC）需在每次重跑前禁用（fn02 证据 L166-177）。

## lane payload 三副本对账 — 上游 PR 描述草稿（US-5）

> 标题：fix(queue): persist StoredCommand.payload instead of `{}`
>
> 正文要点：`queue.rs:284` 以 `json!({})` 落盘 payload，重放丢失命令内
> 容；三副本（本仓 `crates/lane`、`apps/api/crates/lane`、
> `apps/cloud/crates/lane`）同线同缺陷；本 PR 为其中之一，其余两处以相
> 同 diff 落地；测试：构造带 payload 命令 → 持久化 → 重放 → payload 字
> 节相等；对账后 `tests/crate_drift_report.py` 漂移集不生长。
>
> 落地顺序：kense-labs/api PR → kense-labs/cloud PR → aster 根副本 +
> 两个子模块指针提升（同一提交，gitlink-only）。
>
> **已落地（2026-10-22，通道改直推）**：api `7091f80` → cloud
> `2ff9eed` → aster `8a8f99881`；修复含 trait 默认 `payload()` 方案
> （原草稿未定法）；三副本各 402+6 绿，drift 保持 15 不生长。

## STUDIO_READ_TOKEN 最小权限配置清单（远端 CI 单点解阻，2026-10-21 实证）

远端 backend/Rust 工作流全部停在 `checkout-studio` 的 fail-closed 守卫
（run 39540474980 等，见 OUTCOME.md 远端判定段）。一个 secret 同时服务
studio 与方案 a 的 api/cloud 检出。最小配置：

| 项 | 值 |
| --- | --- |
| 类型 | GitHub fine-grained PAT（或同 scope 的 org bot token） |
| 仓库访问 | 仅 `kense-labs/studio`、`kense-labs/api`、`kense-labs/cloud`（Contents: **Read-only**；Metadata 随附） |
| 明确不授 | Workflows、Secrets、Admin、写入任何仓库 |
| 存放 | aster-ai-lab/aster → Settings → Secrets and variables → Actions → `STUDIO_READ_TOKEN` |
| 验证 | push 任意提交或 `gh workflow run`，观察 fmt/check 过 checkout 段 |
| 轮换 | 建议 90 天；到期症状即本次（基线 10-20 通过、10-21 拒绝） |

若组织策略不允许 PAT 读私仓：备选是 GitHub App + 安装 token 的 checkout
action 变体，成本更高，需另立 ADR。

## 决策期限（specs/005 US-3）

D-A/D-B：2026-11-07；D-D：2026-11-15。到期未决 → 依赖故事 fail closed
（FR-005）。

## 宪法 XIII 修正草案（供 /speckit.constitution 拍板；analyze 发现 #1）

行为已先行：aster-ai-lab/libkrun fork（用户 2026-10-21 明示授权）。按
修正纪律「行为先行、文本随后」，建议在 XIII 的 apps/studio 授权句后补：

> The user-authorized dependency fork boundary is
> `aster-ai-lab/libkrun` (created from aster-ai-lab/libkrun at owner
> direction, 2026-10-21). It is consumed through the vendored
> aster-libkrun-sys tree; the parent tracks source provenance, never a
> parallel Aster checkout of libkrun source inside this repository's
> crates.

## AppRelease 加固 — 上游 PR 草稿（kense-labs/api；2026-10-21 发现；当晚前提勘误+本地已实现）

> **前提勘误（2026-10-21 晚，首 e2e 挂载时发现）**：初判"不在任何受守卫前缀"
> 错了一半——`API_PREFIX = /v1/api/admin`，app-release 实际挂在
> `/v1/api/admin/app-releases`，**在** SessionPlaneGuardInterceptor 的前缀门
> 之内（含读路径）。真实洞更精确：拦截器只校验令牌**前缀形状**
> （`aster_us_`），控制器层零会话验证——任意 `aster_us_` 形状（无效/吊销/外来）
> 令牌即可写。初判的"/current/file 直供下载站"同样修正：读也在前缀门后，
> 是否公开化是独立产品决定，未代决。

原始证据（保留供对照）：`/v1/api/app-releases`（实为 admin 前缀下）的
upload/publish/withdraw/delete 控制器层零鉴权（对比 model_hub 的
`sessions.operator()` 全验证模式）。

**本地已实现（api `58410bb`，排队上游 PR）**：
1. 写面全验证：四个写操作前置 `AssetSessions::operator()`
   （model_hub 模式，`ProofAssetSessions` 注入）；读路径保持现挂载形态。
2. 唯一性：`(channel, platform, version)` 唯一索引**库内本已存在**
   （migration 031 表级 + 063 partial published 索引）——真实缺陷是
   `insert_err` 用词表猜冲突而 orm 错误文本从不含那些词（重复上传一直
   500 而非命名 409）；已改为类型化链上匹配 **SQLSTATE 23505**（宪法
   禁词表猜测的直接应用）。
3. e2e（org_hub live 变体模式：真控制器 + 活 PG + 活 RustFS + 一次性
   迁移库 + 脚本会话）：双层准入拒绝（前缀门：无头/成员令牌；服务层：
   `aster_us_` 形状未知令牌）+ 上传→发布→current→流式文件字节回环 +
   重复三元组命名 409 + 撤回后 denied≡missing + 全量删除清理。
   2/2 绿（本地活栈实测）。已知边界（已清偿 2026-10-22）：`plugin/
   packaging/` 两文件的 HEAD 既有 fmt 债（`55a64b3` 引入）——api `079e746`
   纯格式化清偿，crate fmt 门全绿，随队 PR。

**剩余**：仅推送权（PR 内容已从草稿变为 ready-to-open 的实现 + 证明）。

## T029 计量环生产者 — 设计笔记（2026-10-21）

消费者侧实况（今日核验）：
- Cloud I0.2c 用量账本摄取**已存在**（control-plane `modules/inference/
  application/usage_batch_campaign.rs` + `usage_ledger.rs`，campaign 已测）。
- Kense 原始 runtime 设计是**事件式**：尽力发布 `events.runtime.*`，api
  队列 worker 消费 `usage-ingest`，durable 投影（ui merge-evidence L439）。
  Rust 重写丢了队列腿（runtime `vault_apply` 的「NATS not wired」同证）。

两案：
- **A（HTTP 批）**：runtime POST `aster.gateway.usage-batch.v1` 到 Cloud I0.2c
  摄取面。小改（runtime 已有 http client + 能力钥），但绕开事件主干。
- **B（事件式，DP-2 对齐）**：runtime 接 NATS 发布 `events.runtime.usage`
  （DP-2 已裁 NATS 主线；api worker 消费模式为原始设计）。大改：runtime
  增队列依赖与 at-least-once 语义。

建议：**B 为目标态、A 不做**（避免建成又拆的临时 HTTP 腿）；实现切片 =
runtime 增 NATS 出站 publisher（尽力、断线不阻塞请求路径，宪法 I 降级语
义）→ api 侧 `usage-ingest` worker（若上游未备则该半环归上游）→ 契约测
试保持「200/发布仅真实 records」。体量：中（一天级切片）。

## T026 月度评审提案 — FN0 保留 run 等价修订（2026-10-21）

背景：FN0 residual「Still required for Verified」首行把 production-shaped
FN0.2 Fleet retain 钉死在 GitHub Actions `ubuntu-24.04` workflow
（`fn0-fleet-hosted-task-live.yml`），唯一解阻钥匙是 org secret
`ASTER_LABS_READ_TOKEN`（kense-labs 三私仓只读），至今未批。同时 Phase E
自托管 runner（specs/005 T025）以机器侧 deploy key 检出私有子模块，同机
可跑同一 CI 入口 `run_fn02_fleet_live_ci.sh` 并由保留管道（T018）留存。

提案（供 2026-11 月度评审裁定；裁定前不改上游文本）：

> 修订 `apps/cloud/docs/fn0-verified-residual.md`「Still required for
> Verified」首行：接受「self-hosted retained run 等价可接受」——
> production-shaped retain 的判据从「GitHub Actions `ubuntu-24.04`
> workflow 保留」放宽为「GHA 或自托管 Linux runner 保留」，自托管路径
> 须同时满足：
> 1. 干净 Linux 主机（非 Orb/开发 VM），os-pinned gitlink 检出私有子
>    模块，fail-fast 守卫与现 workflow 同标准（token 缺失即命名失败，
>    禁止跳过检出）；
> 2. 同一 CI 入口（`run_fn02_fleet_live_ci.sh`）与同一保留产物
>    （OUTCOME + run 记录 + 摘要 digest）；
> 3. 保留 run 经月度评审记录采纳（自认证闭环：机器自持，独立性由检出
>    pin + 评审记录补偿）。载体二选一：kense-labs/cloud workflow 的
>    self-hosted 变体（上游 PR），或 aster backend.yml self-hosted 腿
>    驱动同一入口。
> 不变项：Verified 晋升仍需完整真实供应商 failure/recovery/cleanup/
> release 证据；Darwin probe 与未 prepare 的 Orb 依旧禁止入账（原文
> 红线全保留）。

依赖与顺序：T025（runner 机器，与 T0.4.2 Linux 腿同机）→ 首个自托管保
留 run（T018 管道）→ 本提案经 2026-11 评审记录采纳 → 上游 residual 文
本随下次 kense-labs/cloud PR 一并修订（与 T016/T017 同队列）。

自评（fail-closed 视角）：等价性主张的实质是「production-shaped = 主机
形态 + 保留能力」，非「GitHub 品牌」。若评审不采纳，回退态 = 等
`ASTER_LABS_READ_TOKEN`，FN0 维持现状，不产生新增债。

## T015 取证修订 — krun 前提更正（2026-10-21，证据见 development/
2026-10-21-krun-t042-rerun-forensics/OUTCOME.md）

六格实验矩阵定案：10-19 的「krun 引导链内部失败」是 **dyld 加载失败**——
硬拷贝至 `~/.aster/bin` 的 shim 其 rpath（`@executable_path/../lib` →
`~/.aster/lib`）无 libkrun 可载，立即退出被 CLI 报成 VM boot 失败。补全
`~/.aster/lib`（拷安装件 `lib/` 的 `libkrun*.dylib`）后 **stock 3.3.0 链
converged**；repo 构建（vendored 或 fork 插桩库）全程绿，插桩未触发。

对决策板的修订：
- **D-D**：对 T0.4.2 的 box 路径失效（compose 引导用的正是 pinned
  digest `742f…2193`；digest 不可解析是 docker.io pull 特有）。Docker
  fallback 的镜像镜像问题独立存在，但 T0.4.2 不再依赖它。
- **D-A**：成为 T0.4.2 步骤 4–6 的**唯一剩余决策**（Cloud 侧
  webpki-only trust 对本地 aster-auth issuer 的私有 CA 准入）。
- **D-F/上游 Box issue 素材补充**：`find_shim` 会选中脱离安装上下文的
  硬拷贝 shim，其 dyld 失败被归类为 VM boot 失败——上游修复方向：错误
  分类区分「shim 未加载」与「VM 退出」，或 find_shim 校验 rpath 可解析。
  stock/repo 两链 config 同形（`:copy` 目录卷），排除配置面因素。

fork 侧：`ad556a3`（sysroot gcc-runtime 链接修复，macOS 交叉 `make` 冷
启动自证）——fork 首个非插桩修复已合入 main。

## T0.4.2 步骤 1 执行修订 — D-A 材料补充（2026-10-21，证据见
development/2026-10-21-t042-step1-subject-link/OUTCOME.md）

步骤 1（SubjectLink 建链/解析/负控）已在真实 Cloud api 上执行并证明。
对 D-A 材料的两点补充：

- issuer 标识符在**域层**已强制有界规范 HTTPS URL（loopback HTTP 422），
  即 D-A 的 a/b 两案之外还有一个既成事实：本地 aster-auth 对 Cloud 的
  issuer 标识符必须是 `https://…`。D-A 若选 a（受管信任锚），issuer
  标识符与信任链一起落配置；选 b（公共 CA），需可公网解析的测试域名。
- SubjectLink 的 principalId 须为活跃 human principal（`POST memberships
  {principalKind:"human"}` 直建）——runbook 步骤 1 的前置修订，随上游
  runbook 文本更新进 PR 队列。

附带：run_cloud.sh 的 box 路径三处 dev 工具缺口（S3 凭据未导出、
`host.orb.internal` 端点 host-run 不解析、node-control 端口无避让）已
在证据中记录，随 lane/AppRelease PR 同队列上报。

## T021 前置锐化 — fork 与 vendor 血统分叉（2026-10-21 核查）

> **[已推翻 2026-10-21 晚]** 本节"非同支血统"结论是浅克隆边界的伪影：
> fork 克隆的 `.git/shallow` 边界恰在 `6fdc25f`，遮蔽了父边。补全历史后
> merge-base(main, f7c69184) = f7c69184 本身。勘误与吸收探针见下节。

`vendor/libkrun`（归档 SHA `b92b7d…`，源工具链提交 `f7c69184`）与
aster-ai-lab/libkrun main（`02e1414`+`ad556a3`）**不是同支血统**：
96 处树差异，vendor 独有 SNAPSHOT_PHASE_B.md / VSOCK_RESTORE_DIAGNOSIS.md
及产品补丁集（TSI 反向代理 #13、端口发夹 #14、-EPERM #12、allowlist
host bind、VirtioFS 快照、FUSE resume cookie……，SOURCE-PROVENANCE 详载）；
fork 是 aster-ai-lab 主线 + 插桩 + sysroot 修复。直接重 vendor = 回归产品网络
行为（build.rs 的 TSI 警告即此因）。

**结论**：T021 真实前置 = fork 先吸收 f7c69184 补丁集（约 3 个 PR 量级，
cherry-pick 或 rebase 到 fork main + macOS 冷编验证），这是 owner 可见的
实质工程项。sysroot 修复（`ad556a3`）不足以触发重 vendor——它只是让
fork 可在本机从源构建。

## T021 前置推翻与吸收探针（2026-10-21 晚；supersedes 上节结论）

**血统勘误**：fork 克隆是浅克隆（shallow 边界 = `6fdc25f`），此前
merge-base 失败是边界伪影。`git fetch upstream --unshallow` 后：
**merge-base(main, f7c69184) = f7c69184 本身**——f7c69184 就是 fork main
的祖先。fork main = f7c69184 + virtiofs-trunc 修复（`6fdc25f`）+ 插桩
（`02e1414`）+ sysroot 修复（`ad556a3`），全量增量仅 3 文件。TSI 产品补丁
（#12/#13/#14 所涉 9 文件）在 vendor 树与 f7c69184 **逐字一致**——
"重 vendor 丢产品网络补丁"风险不存在。

**真实缺口（方向反转）**：aster vendored 树相对 f7c69184 有 **94 处差异**
（87 内容差异 + 4 vendor 独有 + 3 f7 独有），即约 91 文件的前向演化
（`src/vmm/src/snapshot.rs` 快照实装、`KRUN_SNAPSHOT_SOCK` 触发服务、
macOS `passthrough_tests.rs`、`SNAPSHOT_PHASE_B.md`/`VSOCK_RESTORE_DIAGNOSIS.md`
工作文档、devices 26 / vmm 18 / utils 7 广面改动）**从未进入任何 git
血统**——只活在 aster 仓的 vendor 树里。吸收方向 = 把这 91 文件提取进
fork，而非从上游拉补丁。

**吸收探针已执行并编译验证**（fork 分支 `absorb/t021-vendor-evolution`
@ `3b1a018`，本地未推）：
1. 90 文件自 vendored 树逐字拷入；`src/libkrun/src/lib.rs` 真三向合并
   （base=f7c69184；ours=插桩；theirs=快照触发/关机路径），唯一冲突
   （stage 标记 vs 快照块同点插入）解为两者并留。
2. `NET=1 BLK=1 make` **exit 0**（增量，含快照触发块 linux/x86_64 cfg
   与 macOS aarch64 cfg 放宽）。
3. 无损失证明：`diff -rq` vendored 树 vs 分支树——**零缺失**；差异仅
   3 个 fork 超集项（Makefile sysroot 修复、lib.rs 插桩、make 收敛后的
   Cargo.lock +26/−14）+ 构建产物。合计 +1949/−3355（vendor 演化含删减，
   内容逐字保真）。

**T021 剩余（owner 决定 + 已测量机械）**：
- D-1 ✅（2026-10-21 晚）：`absorb/t021-vendor-evolution` --no-ff 合入
  fork main `2fe72be`——依据 T020 轨道既有完成判据（"fork + 修 + main
  合入 + 编译验证"），分支不触产品树、可逆，且把 91 个仅存于 vendor
  目录的文件收入血统保护。
- D-2（唯一剩余 owner 决定）：重 vendor 的产品树是否含 boot 插桩
  （`KRUN_BOOT_ABORT` stderr 行是行为面变化；不含则从剥离插桩的提交
  生成）。
- 归档机械（已测量，防下次猜测）：`vendor/libkrun-source.tar`
  （`b92b7d00…`）pax 全局头标注生成提交 `f7c69184`，但为 **16 顶层
  条目的过滤产物**（435/545 文件；排除 `.github`/`docs`/`examples`/
  `tests`/`krun-sys`/Windows 例子与 prebuilt 等）；裸 `git archive`
  （git 2.50.1 Apple Git-155）不复现排除集亦不复现摘要；树内
  `.gitattributes` 无对应 `export-ignore`——配方不在库。tar 兼任
  Windows 运行时 corresponding-source（`dc5519f`/`2692169b` 血统），
  vendored 演化不改其内容；**仅 macOS/Linux 重 vendor 应保持 tar 冻结**，
  若需再生 tar 须先在 SOURCE-PROVENANCE 记录排除配方与生成 git。
  全部已写入 SOURCE-PROVENANCE.md「Ownership transition」。

## hermetic 矩阵服务依赖修正（2026-10-22，T009/T018 后续）

背景：全量 api 套件（CI 原话 `cargo test --locked --manifest-path
apps/api/Cargo.toml -p aster-api -- --test-threads=1`，活 PG/RustFS）
818+17+3 全绿、0 失败——排队三提交（58410bb/f14f605/079e746）整栈无
回归。并行跑出现 1 例假红：tobot_memory 两 PG 测试共用可丢弃库
`aster_tobot_memory_proof`，其注释自书「必须 `--test-threads=1`」，而
backend.yml campaigns 正是串行调用——调用方式错误，非缺陷。

由此暴露 hermetic 矩阵（无服务容器）的存量地雷：api 三处 PG 证明对
不可达服务是 panic 不是跳过——tobot `postgres_tests.rs:67`（双测实证：
死端点 → "could not reset disposable database … error connecting to
server"）、git_store `postgres_catalog_tests.rs:44`
（`.expect("live PostgreSQL")`，同实证）；cloud control-plane 同病
（`panic!("ASTER_CLOUD_TEST_POSTGRES_URL is required")` 门 + 后续连接失
败）。对照 mail `postgres_tests.rs:64` 是响亮跳过（"not a pass"）。
workflow 顶层 `env:` 使变量在 hermetic 在而 PG 死 → 首次真实执行必红。
node-agent/sdk/contracts 零 PG 引用，干净。

修正（aster 仓）：backend.yml hermetic 矩阵移除 aster-api（
aster-cloud-control-plane 本就不在其列），os-retained-run.yml 同步移除
`run_tests api`（该管道无任何服务，步骤更名「service-free set」）。
api 的测试覆盖只在 campaigns 活栈串行腿。本地证明：双 YAML 解析、
hermetic 13 包清单断言、campaigns 三串行命令原样、run 块 `bash -n`。
边界：自托管 retained 管道现不含任何服务背书套件——runner 侧若要补，
需 campaigns 形态（真 PG/S3 + 串行，tobot 另有单线程约束），属 T012
runner 键。
