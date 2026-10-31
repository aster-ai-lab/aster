# T0.4.2 Linux 腿执行手册（硬件无关关键路径附录）

**依据**: owner 指示「跳过硬件限制」（2026-10-21）。上游 runbook
（`apps/cloud/docs/convergence/t0.4.2-oidc-federation-runbook.md`）
的 macOS/HVF 执行环境被替换为 Linux/KVM；本附录只定义环境映射与证据
落点，不改 runbook 步骤本身。

## 为什么成立

- krun 引导故障是 **HVF（macOS）特有**；KVM 上引导稳定（fn02 证据链
  的 Linux 段全部通过，仅卡在 macOS 段与手工栈的证书缺口）。
- runbook 2026-10-20 首执行的已验证修订全部可复用：五服务墙用 repo
  compose + `ensure-*` 脚本清障；成员登录需 `computer_id`；标识是成员
  手机号；验证面是 `/v1/api/org/*` 会话守卫转发 `/v1/introspect`。
- postgres digest（D-D）在常规 Linux 主机上 docker.io 可解析，或经
  任意镜像代理；不再是本地网络特例。

## 环境（一台 Linux/KVM 主机或自托管 CI runner）

| 项 | 要求 |
| --- | --- |
| OS/虚拟化 | Linux + KVM 可用（`/dev/kvm`）；裸金属更佳（直接满足 FN0 的 production-shaped host matrix 残留项） |
| 容器 | Docker 可用、docker.io 可达（或内网 mirror） |
| 检出 | 云端 checkout 私有子模块：自托管 runner 用机器侧 deploy key（绕开 `STUDIO_READ_TOKEN`），或仍用 token |
| 栈 | `apps/api` 的 compose + `ensure-*` 脚本拉起五服务（Postgres/Redis/Iggy/RustFS/config-center） |

## 执行映射（runbook 步骤 → Linux 腿）

1. **步骤 1（Cloud 半链）**：在 Linux 主机执行；krun/KVM 引导；
   失败日志由 T022 的 `diag/failed-boots/` 机制留存（若失败）。
2. **步骤 2-3（成员面）**：照 2026-10-20 修订直接复跑（已绿一次）。
3. **步骤 4-6**：OIDC 联邦端到端——前置仍是 **D-A 决策**（产品侧
   信任链 vs 测试联邦换公共 CA，材料在 `research.md`）。
4. **证据**：OUTCOME 落 `docs/evidence/convergence/<date>-t042-linux-leg/`，
   显式声明执行环境（KVM/Linux、内核版本、`/dev/kvm` 存在性），并把
   同一主机的规格记录为 FN0 host-matrix 保留证据的输入。

## FN0 双收

同一台 KVM/裸金属主机在完成 T0.4.2 后，按
`apps/cloud/docs/fn0-verified-residual.md` L189-198 的要求补一次
FN0.2 Fleet hosted-task 保留 run（GHA token 若仍缺，则先产出
主机本地保留包，token 就绪后补 GHA 形状）——「跳过硬件限制」把
FN0 的裸金属残留从等待项变成顺路项。

## 边界（Explicitly not claimed）

- macOS/HVF fork 修复不受本附录影响：并行轨（取证 → 修 fork → T021
  重 vendor）继续按 `tasks.md` T020/T021 走，不占关键路径。
- TEE 家族维持 GA-2/GA-3 关键路径之外（D-E 显式后置声明）；本附录
  不产生任何机密计算声明。
- 本附录是执行规划，不是验收记录；验收按宪法 II 落 OUTCOME。

## 裸进程变体（2026-10-21 补，krun 前提修正后）

krun 前提修正（见 OUTCOME 终段）后更强的一档：runbook 六步本质是身份
链验证，只需 **Cloud api 角色进程 + aster-auth + aster-api + 已迁移 PG**，
不内在需要 VM。先例：2026-10-18 导入验证曾将 api/auth/billing/config
四服务以裸进程对 live 中间件启动（docs/migration.md backend_import
verification）。本地 `aster-postgres` 容器已健康运行 23h+。

执行序（无 VM、无 docker digest 依赖）：
1. 裸进程拉起 Cloud api 角色（对 `aster-postgres`），完成 C0.3 OIDC
   issuer 配置（前置 1，经其管理面）。
2. aster-auth + aster-api 按 runbook 前置 2/3 起进程（10-20 已证 compose +
   ensure 可清五服务墙）。
3. SubjectLink 管理（前置 4，admin + identity:write）。
4. 步骤 1-6 照 runbook 执行；唯一剩余阻塞 = **D-A**（webpki-only 信任）。

边界：若裸进程 Cloud 角色在本地暴露出部署形态限制（如必须 VM 内凭据
拓扑），则回落 Linux 腿变体；两条变体都不依赖 macOS HVF 修复。
