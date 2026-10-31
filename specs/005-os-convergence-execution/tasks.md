---
description: "Task list for specs/005-os-convergence-execution"
---

# Tasks: Aster OS 技术收敛执行（W0–W3）

**Input**: `specs/005-os-convergence-execution/{spec.md, plan.md, research.md}`

**Prerequisites**: plan.md（含 Constitution Check，已通过）、spec.md（含
US-6 证据修正）

**House rule**: 每个改行为的任务在同任务内携带其 hermetic 测试；无测试
即未完成。

## Format: `[ID] [P?] [Story] Description`

- **[P]**: 可并行（不同文件、无共享状态）
- **[Story]**: 归属用户故事（US1..US8）

## Path Conventions

- 数据面：`apps/runtime/`
- 共享 crate：`crates/<name>/`
- CI：`.github/workflows/`
- 证据：`docs/evidence/<domain>/<date-slug>/OUTCOME.md`

## Phase A: Behavior and tests（已完成 2026-10-21）

- [x] T001 [US7] usage/batch 与 analytics 的命名拒绝（501 +
      `usage_export.not_wired` / `analytics.not_wired`），usage 视图移除
      未接线的 lifecycle 广告；hermetic 测试
      `apps/runtime/src/http/router/usage_export_tests.rs`（3 用例）✅
- [x] T002 [P] [US1] `backend.yml` fmt job 补 `rustup component add
      rustfmt`（`.github/workflows/backend.yml`）✅
- [x] T003 [US6] `apps/runtime/src/lib.rs` 文档注释对齐包名；spec 记录
      证据修正 ✅
- [x] T004 [P] [US1] `crates/use` 测试编译修复：7 个上游根级 fixture 恢复
      （`.github/workflows/*`、`Cargo.lock`、`install.sh/ps1`）+ 33 处陈旧
      嵌套 include 重指向拍平布局（15 文件）；验证 =
      `cargo check --locked -p aster-use --all-targets`（exit 0）+
      `cargo fmt -p aster-use -- --check` ✅
- [x] T004b [P] [US1] `aster-gateway` 已提交债务修复：`benches/routing.rs`
      补 `parking: None`（对齐 `src/config/router.rs:95` 的 Default）；
      `examples/protocol_benchmark_load.rs:138` `Vec<u8>`→`Bytes`；
      验证 = `cargo check -p aster-gateway --benches --examples` 零错误 +
      包级 fmt 干净 ✅
- [x] T005 [US3] 决策材料与外联草稿 `specs/005-.../research.md`（D-A/
      D-B/D-D 对比、D-F issue 草稿、US-5 上游 PR 草稿）✅
- [x] T006 [US7/US1] 证据 `docs/evidence/development/
      2026-10-21-runtime-export-honesty/OUTCOME.md`（含 Phase B 诊断与
      backend.yml 结构性发现）✅

## Phase B: Wiring / 落库（部分阻塞）

- [x] T007 [US1] 本切片改动已逐文件落库并推送（5 笔提交，工作树仅余并
      发会话在途编辑，未捎带；其落库由该会话自行完成）✅
- [x] T008 [US1] 全库 `cargo check --locked --workspace --all-targets`
      → **exit 0 / 0 错误**（29m15s）；fmt 门 HEAD 绿（唯一残余 diff 属并
      发会话未提交文件）；依赖守卫 6/6 OK——终值已写入 OUTCOME.md ✅
- [x] T009 [US1] backend.yml `hermetic`/`campaigns` 死配置修复——owner 选
      方案：新增 `.github/actions/checkout-os-backend`（镜像 checkout-studio：
      fail-closed token、gitlink pin、不持久化凭据）；hermetic 矩阵带
      `submodule`/`test_args`、按真值门控检出；campaigns 三命令
      `--manifest-path` 作用域；rust-cache 补 `workspaces`。**2026-10-22 修正**：
      aster-api 移出 hermetic 矩阵——其 tobot/git_store PG 证明对不可达服务
      panic 而非跳过（实证见 research.md），hermetic 无服务容器首跑必红；
      api 覆盖只在 campaigns 活栈串行腿。本地证明：YAML
      解析、五包在各自 manifest 下可解析、gitlink 可定位；真实 CI 执行未
      声明（token 范围不足时 fail-closed 即设计行为）✅
- [x] T010 [US1] 工作区 fmt 门：**HEAD 提交态已绿**（早间 8 红中 7 个仅
      红在并发会话未提交树；studio `plugin_selection.rs` 的红亦属其当时
      脏字节，HEAD 本身干净——详见 OUTCOME.md 终值段）✅

## Phase C: Phase 0 收口（阻塞：owner 决策 + 上游 PR）

- [ ] T011 [US3] D-A ADR 落 kense-labs/cloud `docs/decisions/`（材料已
      备于 research.md）——**阻塞 owner**
- [ ] T012 [US3] D-B token/runner 决策与配置——**阻塞 owner**
- [x] T013 [US3] D-D postgres digest 方案——**由 box 路径取代**（2026-10-21
      实证：compose 引导消费的正是 pinned digest `742f…2193`，Docker
      fallback 对 T0.4.2 失去必要性；镜像镜像问题独立存在但不再占执行
      位）✅
- [x] T014 [US3] D-F krun issue 外发——**由 Phase C+ fork 轨替代**
      （T020 已落：fork + 插桩 + main 合入 + 编译验证）；D-F 草稿文本留
      research.md 备 owner 后续外发，不再占执行位 ✅
- [ ] T015 [US4] T0.4.2 runbook 首执行 + OUTCOME 证据——取证腿已落
      （2026-10-21：「krun 引导链失败」根因 = 10-19 硬拷贝 shim 缺
      `~/.aster/lib` 的 rpath 库，dyld 加载失败被误诊为 VM boot 失败；六格
      实验矩阵定案，补全 `~/.aster/lib` 后 stock 3.3.0 链对 compose.acl
      两服务 converged。D-D 对 box 路径失效——compose 用的正是 pinned
      digest。**步骤 1 已执行并证明**（建链 201→解析 200→负控 404，
      human principal + HTTPS issuer 两条 runbook 修订；前置 1 环境半边
      跑通，含 run_cloud.sh 三处 dev 工具缺口）。**修订文本已落 runbook**
      （2026-10-21 晚：cloud `cda0231` 修订落库，Status 行改为步骤 1–3 已证
      /4–6 阻 D-A，10-19 recon 结论标记被取代）。**剩余唯一阻塞 = D-A**
      （步骤 4–6 的私有 CA 准入决策）。证据：
      `docs/evidence/development/2026-10-21-krun-t042-rerun-forensics/`、
      `docs/evidence/development/2026-10-21-t042-step1-subject-link/`）
- [ ] T016 [P] [US4] G0/P0/WEB0 aging 日期回填——**材料完备性已查（2026-10-22）：
      阻塞是数据，不只是通道**。本地 kense-labs/cloud 克隆仅 13 笔提交（根
      =`0eb6f63` 2026-10-19「Import the Aster Cloud control plane」，此前 0 笔），
      G0/P0（ROADMAP L327/328，无子行）与 WEB0.1–4（L3322–3325）全行零日
      期、零 evidence 链接，本地 docs/evidence 亦无该三 lane 的 retained 目
      录——完成日期只存在于上游 aster-ai-lab/Cloud 前导入史或 owner 记忆。回填
      日期喂「两季度无 retained 证据即降级」的治理钟，猜值即腐蚀月度评审决
      策（宪法 fail-closed 禁）。通道侧照旧：日期由 owner/上游史供给后，
      本地编辑 + 排队 PR + 指针提升（与 T017 同队列）
- [x] T017 [US5] lane payload 三副本对账——**已落三仓 main（2026-10-22）**：
      修复 = `Command` 增默认 `payload()`（空对象，不破坏既有实现者）+
      `JsonCommand` 覆写真值 + `enqueue` 在 move 进 wrapper 前捕获并落盘
      （替换 `json!({})`）；保真测试 = CapturingStorage 回环，断言落盘
      字节与提交 payload 相等；三副本（根/`apps/api`/`apps/cloud`）同一
      diff，各 402+6 全绿、fmt 干净；drift 集保持 15 不生长、`aster-lane`
      零漂移。落地顺序照草稿：api `7091f80` → cloud `2ff9eed` → aster
      根副本 + 双 gitlink 同一提交 `8a8f99881`，均已推送 ✅

## Phase C+: krun fork 修复轨（2026-10-21 新增，替代 T014 的外发路径）

- [x] T020 [US4] fork `aster-ai-lab/libkrun` → **aster-ai-lab/libkrun**；分支
      `boot-diagnostics`（`02e1414`）：`krun_start_enter` 全部 11 处早退
      经 `boot_abort(stage, code)` 输出 `KRUN_BOOT_ABORT` 行 + 4 个
      `KRUN_BOOT_STAGE` 标记（含原两处静默退出）；纯插桩、控制流不变、
      rustfmt 干净；编译验证边界已在提交信息声明（cargo 需 make 先产
      init 产物）✅
- [ ] T021 [US4] 重 vendor——**血统勘误 + 吸收探针已执行（2026-10-21
      晚）**：浅克隆边界伪影推翻"分叉血统"旧判——unshallow 后
      **f7c69184 就是 fork main 的祖先**（fork 全量增量仅 3 文件：
      virtiofs-trunc + 插桩 + sysroot），TSI 产品补丁在 vendor 树与
      f7c69184 逐字一致，"重 vendor 丢产品补丁"风险不存在。真实缺口方向
      反转：vendored 树约 91 文件的前向演化（vmm snapshot 实装、
      KRUN_SNAPSHOT_SOCK 触发、macOS passthrough 测试、devices/vmm 广面）
      从未进任何 git 血统。**吸收分支已建并验证**：fork
      `absorb/t021-vendor-evolution`@`3b1a018`（90 文件逐字 + lib.rs 三向
      合并唯一冲突解为并留；`NET=1 BLK=1 make` exit 0；`diff -rq` 零缺失，
      差异仅 3 个 fork 超集项）。**D-1 已落**（2026-10-21 晚：吸收分支
      --no-ff 合入 fork main `2fe72be`，main 成唯一超集血统；2026-10-22
      fork main 已推 `aster-ai-lab/libkrun`（`02e1414..2fe72be`））。归档机械
      已测量并写入 SOURCE-PROVENANCE（tar 为 f7c69184 的 16 条目过滤
      pax，配方不在库；tar 兼任 Windows corresponding-source，仅 macOS/
      Linux 重 vendor 时应保持冻结）。**剩余 owner 单决定 D-2**：产品
      vendor 树是否含 boot 插桩（行为面）。之后走确定性 `git archive`
      再生成 + `LIBKRUN_SOURCE_ARCHIVE_SHA256` + SOURCE-PROVENANCE 同步。
      详见 research.md §T021 前置推翻
- [x] T022 [US4] Box 保留日志策略：启动失败时先把 `box_dir/logs` 拷至
      `<home>/diag/failed-boots/<box_id>/`（按 box_id 单份、重试整体替换、
      best-effort 不影响失败路径），拆毁语义不变；`vm::boot_diag` 三用例
      hermetic 绿（拷贝/替换/no-op），`vm_backend.rs` 冷启动失败分支接线 ✅

## Phase C++: AppRelease 加固轨（2026-10-21 发现）

- [x] T023 [US4] `/app-releases` 鉴权核查——**发现已勘误（2026-10-21 晚）**：
      实挂载在 `/v1/api/admin/app-releases`（API_PREFIX），在拦截器前缀门
      之内；真实洞 = 前缀门只查令牌形状，控制器层零会话验证（任意
      `aster_us_` 形状令牌可写）。发现、勘误与上游 PR 材料入 research.md ✅
- [x] T024 [US4] AppRelease 上游 PR（kense-labs/api）——**加固已落上游 main**
      （2026-10-22：本地实现+证明后经 owner 指令直推——api
      `aa0e7ae..079e746`；写面全 `operator()` 验证 + SQLSTATE 23505 命名
      409（连带修复重复上传 500 存量缺陷）+ 双层准入与全链 e2e 2/2 绿于活
      栈；整栈随全量套件 818+17+3 绿验证；读面公开化未代决——仍是 owner
      独立决策）✅

## Phase F: aster-api 缺口收敛（2026-10-21 完成度核查后分类）

8 缺口 → 3 类：**设计关闭**（SMS 本地适配器不建——成员面按 D4/P3 只读投影
走 Cloud SMS 联邦；About 占位文案待 owner 提供联系方式后一并换）；**上游
排队**（发现 P4.1、PG 队列 flow/repeat[共享副本三份对账]、继承测试债、
Redis 启动依赖[Phase 2 T2.5]、AppRelease 治理[T024]）；**本仓可执行**：

- [x] T029 [US7] 计量环生产者侧：`apps/runtime` 经 aster-event/NATS 发布
      `events.runtime.usage`（尽力语义、断线降级、`ASTER_RUNTIME_NATS_URL`
      未设即拒绝闭合）；批端点三态契约（501 not_wired / 200 真实条数 /
      503 publish_failed）；六契约用例 + 存量 2 全绿（7/7）✅ 原
      `aster.gateway.usage-batch.v1` 批次（usage store 记录→批→POST api 摄取
      端），替换今日的 501 拒绝为真实导出；先设计步（事件源=UsageStore
      生命周期记录、摄取端契约核对 api usage 上下文），再实现+契约测试
      （200 仅真实 records 的既有契约保持）

- [x] T030 [US4] api 测试留白补齐（notes/oem/config_center 三 context 结构
      全、库内零测）——**已落上游 main**（2026-10-22：`f14f605` 随
      `aa0e7ae..079e746` 直推）——notes 5（model_service 模式脚本仓）+ oem 14（域不变式 +
      new/bind 准入与生命周期脚本目录）+ config_center 3（wire e2e：进程内
      脚本化 aster-config 服务 + 真 HttpConfigClient + 真控制器；死服务目录
      200+命名错误、remote 404→404/409→409 映射、删幂等）；合计 22，全
      进程内零外部依赖；fmt 债随队清偿（api `079e746`，crate fmt 门全绿）；
      整栈经全量套件（CI 原话串行调用）818+17+3 绿/0 败 ✅

- [ ] T031 [US4] studio 接线 ToBot 记忆适配器（755 行适配器的
      recall_note/retain_exchange 挂进回合循环；后端 10 端点已交付）——
      **上游排队**（kense-labs/studio，与 api PR 同队列）

## Phase E: CI/CD 自替代冲刺（2026-10-21 CD0 评估修正后新增）

CD0 机器已 90% 在库（10-02/03 落地：九编译器/晋升治理/韧性/SDK/真 E2E）；
「替代 GHA」收束为四缺口，且互相成环——自托管 runner 同时解 CI token、
保留管道与 FN0 的 GHA-retain 依赖。

- [ ] T025 [US1] 自托管 runner + 保留管道：backend.yml 增 self-hosted
      label 变体（机器侧 deploy key 检出私有子模块，摆脱 STUDIO_READ_TOKEN）
      ——**工作流预置件已置库**（2026-10-21：`os-retained-run.yml` 独立
      workflow 承载 label 变体，见 T018 注；runner 注册前零自动影响）
      ——**阻塞：一台 runner 机器**（与 T0.4.2 Linux 腿同一台即可）
- [x] T026 [US8] 月度评审提案：修订 FN0 residual 的「production-shaped
      GHA retain」为「self-hosted retained run 等价可接受」——自认证闭环
      （草稿已落 research.md §T026：等价判据三条 + 红线不变项 + 回退态；
      裁定权在 2026-11 月度评审，上游文本修订随 T016/T017 队列）✅
- [ ] T027 [US1] Box 构建日志契约 issue 外发（G0/CD0.8-logs 的上游前置，
      与 krun/lane PR 同队列；阻塞外发授权）
- [ ] T028 [US1] CD0.8 webhook/event 触发集成 + P0 收尾（上游 cloud 工
      作，排 Phase 2 T2.1.1 之前置化评估）

## Phase D: GA-2 aster 侧支撑（阻塞 D-B + 上游进展）

- [ ] T018 [US8] 保留绿 run 管道（backend.yml 或独立 workflow；私有子
      模块不可读时命名失败，禁止跳过检出）——**管道已置库**（2026-10-21：
      `.github/workflows/os-retained-run.yml`，仅 `workflow_dispatch`、
      `runs-on: [self-hosted, aster-os]`、机器 deploy-key 检出
      （gitlink 160000 → pinned sha → 命名失败不跳过）、复刻 backend.yml
      验证集（1.97.1/fmt/--all-targets/服务无关四包集/依赖守卫——2026-10-22
      与 backend.yml 同步移除 aster-api：该管道无任何服务，tobot/git_store
      PG 证明必 panic；见 research.md）+ 90 天产物保留；
      本地证明 = YAML 解析 + gitlink 逻辑对 HEAD 实跑（api `aa0e7ae`/
      cloud `4f65537` 均可解析）。原边界（cloud pin 上游不可达则首跑命名
      失败）已消解：2026-10-22 cloud main 直推 `c5deb07..cda0231`、api
      main 直推 `aa0e7ae..079e746`，两枚 pin 均公开可解析。**仍阻塞 T012**（runner 机器到位才有真实执行；不
      代声明绿）
- [ ] T019 [US8] GA-2 各环上游落地后的指针提升 + 证据托管（不代上游声明
      Verified）

## Final Verification Gate

- [x] 拥有包聚焦测试通过：`cargo test --locked -p aster-gateway-runtime`
      → 5 passed / 0 failed；`cargo check -p aster-use --all-targets` →
      exit 0
- [x] 本切片 Rust 编辑的 `cargo fmt -p <pkg> -- --check` 干净
      （aster-gateway-runtime、aster-use；全库门见 T010）
- [x] 无新增 crates.io `aster-*` 依赖（恢复的 fixture 是数据文件；
      `tests/workspace_dependencies.py` 不受影响）
- [x] 未新增原生 `<select>`（无 UI 改动）
- [x] 未新增产品配置（无配置面；D-A 产物将来必须 ACL——spec FR-012）
- [x] 全库 `--all-targets` 检查转绿（T008 ✅ exit 0 / 0 错误）
