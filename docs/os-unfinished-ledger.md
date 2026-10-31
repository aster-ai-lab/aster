# Aster OS 未完成功能账本（aster 侧索引）

**快照日期**: 2026-10-21。本文只做**指针索引**——每一项的权威状态在
被指来源里；本文不复制内容，以免双记录漂移。状态词汇沿用上游
（Verified > Implemented > component-only > In progress > Planned）。

## 读法

- 上游门表：`apps/cloud/ROADMAP.md`（子模块 pin `59669e08`）
- 收敛计划：`apps/cloud/docs/convergence/plan.md` + 根 `aster-convergence-plan.html`
- 执行载体：`specs/005-os-convergence-execution/`（aster 侧）
- 证据链：`docs/evidence/development/2026-10-21-runtime-export-honesty/`

## 一、五大功能空洞（按体量）

| # | 空洞 | 权威指针 | aster 侧状态 |
| --- | --- | --- | --- |
| 1 | 推理数据面 I0/PW0：无 InferenceDeployment 聚合、worker 恒空、计费 tokenizer 未闭环 | `apps/cloud/docs/inference-plan.md` L1330-1763；`apps/cloud/docs/inference-service-module-plan.md` §4 | 无（上游 Planned） |
| 2 | 生产运维 GA-3：H0.3/0.4/0.5（PG 无认证故障切换）、OBS1-8、CD0 尾项、COMP/POL | `apps/cloud/ROADMAP.md` H0 行 L2178-2473；`platform-gap-analysis.md` §3.3-3.6 | 无（GA-3 于 2027-04 开启） |
| 3 | GA-2 验收链（当前第一优先）：W0.3 真供应商收尾 → HumanTask → 目录派发 → 匿名 Chatbot | `apps/cloud/docs/convergence/ga2-scope-and-codegen-pilot.md` L14-17；plan.md L100-108 | 等保留证据基建（D-B） |
| 4 | Phase 0 出口 T0.4.2 OIDC e2e：三解阻塞（D-A 私有 CA、D-D postgres digest、krun） | `apps/cloud/docs/convergence/t0.4.2-oidc-federation-runbook.md` L102-113 | **krun/D-D 已清**（2026-10-21 取证：根因非 krun，D-D 对 box 路径失效）；**仅剩 D-A**（见 §三；材料 `specs/005/research.md`） |
| 5 | Verified 晋升债：FN0（token+裸金属）、CELL0.5（Box#627，冻结）、G0（Box 日志契约）、W0.5/A0/MCP0 | `apps/cloud/docs/fn0-verified-residual.md` L189-198；`cell0-durable-cell-checklist.md` L19-21 | 无（等外部） |

## 二、Held / Planned lane（不占 WIP 名额）

`apps/cloud/ROADMAP.md` L356-367 收敛裁定：HELD = A0/U0/MCP0/FN0/WEB0/
K0/AUT0/S0；FROZEN P2 = EV0/AR0/I0-TrackB+/PW0.tee/CELL0。逐 lane 门表见
`apps/cloud/docs/development-plan.md` §3.1（L454-477）。

## 三、aster 侧自有轨道（krun，2026-10-21 起）

| 步 | 状态 | 指针 |
| --- | --- | --- |
| fork `aster-ai-lab/libkrun` + 插桩（11 早退 `KRUN_BOOT_ABORT`）+ main 合入 + 编译验证 | ✅ | fork main `02e1414`；证据 krun lane 段 |
| Box 失败引导保留日志 `<home>/diag/failed-boots/<box_id>/` | ✅ | `crates/box/runtime/src/vm/boot_diag.rs` |
| T0.4.2 重跑取证 → 修 fork → T021 重 vendor 切换 | 取证 ✅（2026-10-21：根因 = shim 硬拷贝缺 `~/.aster/lib`，非 krun；stock 链补全后 converged，D-D 对 box 路径失效）→ **仅剩 D-A**。T021 血统勘误 + 吸收分支 `3b1a018` 已建并编译验证（2026-10-21 晚，零缺失证明；剩 owner D-1/D-2 两决定，见 research.md） | `specs/005/tasks.md` T015/T021；证据 `docs/evidence/development/2026-10-21-krun-t042-rerun-forensics/` |

## 四、解阻钥匙（owner 动作，按杠杆排序）

1. **`STUDIO_READ_TOKEN`**（kense-labs/studio+api+cloud 只读）→ 远端三
   工作流全绿 + backend.yml 方案 a 首验。最小 scope 清单：
   `specs/005/research.md`。
2. **D-A**（T0.4.2 唯一剩余决策）**/ D-B**（W0.3 保留证据基建）→
   对比材料同上。（D-D 已不阻塞 T0.4.2：box 路径 pinned digest 引导
   实证成功。）
3. **上游 PR 权**（kense-labs/api + cloud）→ lane payload 三副本对账、
   aging 回填。PR 草稿同上。

## 五、本仓库侧已清零项（2026-10-21 会话）

CI 工程债（fmt 供给、use fixture+43 路径、gateway 目标修复）、
backend.yml 方案 a、runtime 导出诚实化、T022 保留日志、krun 转自有。
全量凭证：证据文件终验戳（HEAD 全库 exit 0 + 五包全量绿镜）。

## 六、减法候选预登记（2026-11 首次季度减法评审用，R1 并行轨）

按收敛程序 §7「空减法评审即失败」，预登记三项候选（均为已裁定路径的
执行尾巴，非新决策）：

1. **成员面 identity 第二写面物理删除**——T0.4.2 e2e 过线后执行（写
   路径冻结围栏已在；删除即销账，C2 收官）。
2. **iggy 镜像轨道降级**——DP-2 已裁 NATS 主线；Phase 2 T2.5 把 iggy
   降为兼容适配器时，成员面直连 iggy 轨道（IM、config fan-out 除外
   项已明示）随之收缩。
3. **`lane` redis job 后端实现删除**——test-only 围栏
   （`redisstore_consumer_surface_is_frozen`，15 文件）已绿；Phase 2
   删实现时连带围栏转纯回归。
