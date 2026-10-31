# Implementation Plan: Default Aster tenant

Use the existing Aster tenant pipeline and shared ACP. Copy only canonical Work brand assets; do not copy its renderer, product alias table, runtime or user state. Keep `toyou` and its current bundle/profile identity. Aster gets `dev.aster.studio.aster` and a fresh profile namespace.

## Constitution check

| Principle | Result | Implementation constraint |
| --- | --- | --- |
| I/IV fail closed, closed sets | PASS | Explicit tenant identity; unknown/mismatched identities refuse |
| II/III/XII evidence, tests, review | PASS | Owning RED/green, two-tenant build/GUI and independent adversarial review |
| V/VI workspace, schemas | PASS | One root workspace; existing stores remain owned; no implicit migration |
| VII/XVI/XVII UI ratchets | PASS | Existing controls/primitives; no native select or allowlist expansion |
| VIII/IX configuration | PASS | No new provider/policy config format; tenant branding remains definition data |
| XIII/XIV/XV repository, names, meaning | PASS | Same repository, explicit product contract, typed behavior |

## Ownership and caller closure

- Brand/UI owner: tenant assets and loader/stamper, renderer brand surfaces and theme, explicit tenant capability callers and tests.
- Native owner: tenant/profile/bundle/WebView/lock/runtime discovery, all store callers and sibling isolation tests; model-admission investigation.
- Delivery owner: root entry points, Tauri selected build identity, development launcher, package sealer and Kylin pipeline/tests.
- Root: bounded spec, native updater identity/feed refusal, integration builds, real CUA, governance/provenance and completion audit.

Tests use newly created synthetic profiles and model/update services. HOME is not replaced. Legacy Work state is never opened. No changes to live release metadata or API/control-plane persistence are required for an unpublished Aster feed.

## Analyze result

No unresolved requirement/ownership conflict. User clarification resolves the migration question. The original shared bundle ID and profile collision are confirmed defects and addressed by FR-004/005. Aster's publication is not inferred from the existing `toyou` feed. Spec 003 remains a dependency; its unchecked interactive requirements stay incomplete until observed.

## 用户补充范围

本地 `aster` 构建完全移除登录、用户身份展示及成员/组织查询；`toyou` 保持原有认证。Office 必需代码先迁入现有 `packages/office`，内核脚本改为自包含；移除旧应用的 workspace、构建与 CI 依赖后再移除 `apps/work`。保留许可和来源记录，不复制凭据或迁移用户数据。

## 脱离源码目录的交付闭包

内置技能的精确文件清单作为共享构建契约，供两个打包器及原生校验复用。
开发模式可读取仓库资源；安装包从自己的资源目录读取，缺失时明确拒绝，
不退回构建机绝对路径。技能引用、脚本和许可证均随完整产品封装及校验。
真实启动验收还覆盖空本地列表和页脚，避免隐藏的组织目录查询或通知入口残留。
