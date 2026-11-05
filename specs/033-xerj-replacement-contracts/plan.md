# Implementation Plan: Xerj 完整替代契约

**Branch**: 当前 checkout，无新 branch／worktree | **Spec**: [spec.md](spec.md) | **Input**: 完整替代要求。

**Gate context**: 测试与契约批次；Root Product 实现和私有 Studio 仍按原 regime 交付。本批次不计划 control-plane 模块。

最新目标以 Core 机制和去重为准。对照协议用于发现行为和驱动测试，不要求新增兼容 server。MCP 清单必须归并到 `contracts/mechanisms.md` 的原 owner；共享实现满足多入口，薄 adapter 不拥有第二份机制或数据。

## Summary

先从已固定发布的机器 schema 生成保真清单，以明确的手写 owner 决策揭示缺口；再逐 surface 固定黑盒请求和比较规则，使用 032 的真实进程方法接通实际入口。每项初始 NOT_RUN，清单存在与功能通过分离。完整替代继续由总计划约束，不能缩成当前 MCP surface。

## Constitution Check

本表为设计检查；实现测试与实际入口资格另留证据。

| Principle | Status | Evidence in the plan |
| --- | --- | --- |
| I. Fail closed | PASS | digest、schema、owner 双向覆盖和状态闭集；未知项拒绝，缺证据不 PASS。 |
| II. Evidence retained | PASS | 固定发布原文、SHA、每次 attempt；T009。 |
| III. Hermetic tests | PASS | `tests/xerj_replacement/test_inventory.py`、actual adapters 与本地 endpoint。 |
| IV. Closed sets | PASS | fixture 状态／owner／presence 闭集；不猜默认映射。 |
| V. One workspace / APIs | PASS | 无新 Rust package、workspace／lockfile／SDK；后续 owner 不迁移。 |
| VI. Local schema changes | N-A | 仅测试合同数据，无产品存储 schema。 |
| VII. Shrink-only ratchets | PASS | 不扩大 allowlist，不改固定 grader／超时。 |
| VIII. Replaceable transports | PASS | 测试 adapter 归一化证据，不改生产 envelope。 |
| IX. One configuration language | PASS | 数据不授予 authority；产品只 ACL，不解析上游产品配置。 |
| X. Commands / tests | PASS | 原有 Python unittest + process fixture；新 Rust 行为另带 sibling tests。 |
| XI. Independent judgment | PASS | 记录上游 partial／缺陷，拒绝共同失败即通过。 |
| XII. Review | PASS | 固定 case 在候选实现前；T008 反例复核。 |
| XIII. One repository | PASS | 当前 checkout；Studio 源不加入父仓库；无多 agent 自动委派。 |
| XIV. Names are not guessed | PASS | 名称直接来自 schema；owner 手写精确键，未知拒绝。 |
| XV. Meaning is not matched | PASS | 意图由模型／typed 数据处理；不照搬上游 NL 规则 planner。 |
| XVI. Pickers | N-A | 无 UI 变更。 |
| XVII. Dialogs | N-A | 无 UI 变更。 |

## Project Structure

### Documentation

```text
specs/033-xerj-replacement-contracts/
├── spec.md
├── plan.md
├── tasks.md
└── contracts/
    ├── mcp-owners.json
    ├── mcp-inventory.json (generated)
    ├── query-owners.json (handwritten decisions, checked against syn AST)
    └── mechanisms.md
```

### Source Code

| Layer | Existing pattern | New files / change |
| --- | --- | --- |
| Inventory | `tests/retrieval_bench/qualification.py` 的有界 JSON／摘要验证 | `tests/xerj_replacement/inventory.py`：固定 schema + 手写决策 → generated inventory；`--check` 只读比较。 |
| Hermetic tests | `tests/retrieval_bench/test_qualification.py` 的临时目录与 corrupt-input cases | `tests/xerj_replacement/test_inventory.py`：真实固定源、漂移、字段／顺序、再生成与未知拒绝。 |
| Rust source contract | Core 已有 syn structural guards | `crates/code/core/tests/xerj_query_inventory.rs`：固定 QueryNode 的完整变体／有序字段与明确 owner 决策；仅结构核对，无运行时解析器。 |
| Actual process adapters | `tests/retrieval_bench/acp_engine.py`、ACP `tests/retrieval_qualification_stdio.rs` | 原 driver 扩展实际工具模式／协议 surface；不增加 SDK 或假产品命令。 |
| Product owners | Core retrieval／memory、Studio `agent_plane`／`local_documents` | 只审候选 owner 和缺口；实现分批留原 owner，不由清单生成代码。 |
| Evidence | `docs/evidence/code/2026-11-01-xerj-replacement/` | 固定 reference manifests、原文、schemas、清单审查／运行 logs、OUTCOME。 |

## Research

- 参考 `tool_specs_with(true)` 与发布的 `landing/docs/agents/schemas/mcp-tools.json`：22 项最大集合；3 项 Hub 工具受 signing-key 条件限制。保留完整原 schema，不手译／删参数。
- 032 已证明 `--engine` 可实际运行 ACP。继续使用该模式，不能把上游工具别名贴到只有 hybrid 的回执上。
- `aster-vec` 有 typed filters、FTS、ANN 等机制，但未证明所有 MCP／ES 字段可达；记为候选，不记通过。
- 上游 `xerj_plan` 声明规则型语义规划；宪法 XV 不允许复制其意图规则。保持能力目标，通过现有模型＋typed 校验实现，差异先形成合同。
- 上游反馈工具有默认外发。本批次只审 schema，不调用真实外发路径；未来 replay 限本地 endpoint，运行时沿既有权限。

## Verification Plan

```sh
python3 tests/xerj_replacement/inventory.py --check
python3 -m unittest discover -s tests/xerj_replacement -p 'test_*.py'
cargo test --locked -p aster-core --test xerj_query_inventory
```

- 数据来自冻结 schema，生成文件只由 generator 更新。测试不把 schema 漂移检查算作运行能力资格。
- 当加入实际 adapter 时先写坏协议／身份／占位结果反例，再跑真实目标正例；独立目录保留 attempts。
- 如改 Rust，按原 owner 跑 focused tests、Clippy all-targets no-deps、fmt，构建 CLI／ACP 配对。原生目标未跑就不 PASS。
- 不运行 live third-party services、付费模型、外发反馈或发布，不重做无关全 workspace campaign。
