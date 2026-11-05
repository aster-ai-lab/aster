# Feature Specification: Xerj 完整替代契约

**Feature Branch**: 当前 checkout；不新建 branch／worktree。

**Created**: 2026-11-01

**Status**: Implementing

**Current objective**: 用户最新目标是 Aster Core 机制完整替代、无重复实现且有效高效。公共协议是对照输入，不是另建兼容服务或复制 22 个工具的要求。验收必须把入口归并到机制和唯一 owner，并完成 [机制账本](contracts/mechanisms.md) 的收敛／补缺与实际验证。

**Input**: 用户要求完整替代 Xerj；整体要求见 [总计划](../../docs/xerj-replacement.md)。

**Gate context**: 资格测试与契约数据，承接 032 的真实 ACP driver 和固定质量集。这个有界批次固定公共接口分母与黑盒输入，不实现第二份数据库、不将清单生成当成功能完成。基础产品实现继续沿 032；新增机制按本批次发现的精确缺口进入原 owner 的后续有界实现。

## User Scenarios & Testing

### User Story 1 — 固定且完整的对照接口清单（Priority: P1）

维护者能从固定发布的实际 schema、CLI、HTTP／gRPC、查询 AST、extractor 和运维契约查到每一项需替代行为，不以 README 分组或 crate 名称代替分母。

**Why this priority**: 032 的 36 项只覆盖检索基础，不能证明全量替代。固定版 MCP schema 已提供精确的 22 项最大工具集合，包含受条件限制的 Hub 工具。

**Independent Test**: `tests/xerj_replacement/test_inventory.py` 校验引用 digest、源顺序、每个 schema 的完整 digest、owner 决策的双向覆盖、未知项拒绝和可再生成性；不需要运行参考 binary。

**Acceptance Scenarios**:

1. **Given** 固定 commit/schema，**When** 生成清单，**Then** 每项原始名称、顺序、required、完整 schema 指针／摘要和许可来源保真；原 schema 不被手改。
2. **Given** 一个上游项新增、缺少、重复或摘要不符，**When** 生成／检查，**Then** 拒绝；不能用前缀猜 owner 或默认映射成已支持。
3. **Given** 只完成 MCP 清单，**When** 汇总全量资格，**Then** 其它 surface 仍明确未审计，完整替代不能 PASS。

### User Story 2 — 从缺口到原 owner 的可执行资格（Priority: P1）

每个接口有实际 Aster 候选 owner、入口状态、独立正反例与所需 fixture。结构存在与运行通过是两个事实。

**Why this priority**: Aster 内部已有 vec、memory、Office、SAG、Core 和宿主机制。不能因缺一层协议而重复建立底层，也不能以底层已存在掩盖参数或行为缺口。

**Independent Test**: 固定的工具合同与实际 ACP／宿主／MCP／HTTP process adapters；被测结果来自真实实现，传输错误和远程副作用只用既有的受控边界注入。

**Acceptance Scenarios**:

1. **Given** 工具 schema 已登记但没有实际兼容入口，**When** 生成资格，**Then** 保持 NOT_RUN／FAIL，不因内部近似工具存在而 PASS。
2. **Given** 含义规划、资料／记忆图或 Corpus 操作，**When** 实现对应能力，**Then** 遵守原 ACL、模型／typed 意图、权限、scope 和资源 owner；拒绝复制上游与本仓库原则冲突的算法或默认副作用。
3. **Given** 外部反馈／远程服务操作，**When** 资格执行，**Then** 只访问显式的本地 hermetic endpoint；未获得发送指令时不向他人提交报告或 PR。

### User Story 3 — 双目标黑盒结果可解释（Priority: P1）

相同的冻结请求可送往参考实现和 Aster，按预先定义的确定性字段、动态字段、浮点容差、分页、错误与副作用回执比较。

**Why this priority**: 响应外形相同不足以证明语义对等；上游自己的未实现项或缺陷也不能给 Aster 加分。

**Independent Test**: 测试 adapter 的坏协议／身份／缺结果反例，固定 expected 控制结果，真实二进制／资料的正向 replay；未运行的参考或候选保持 NOT_RUN／BLOCKED。

**Acceptance Scenarios**:

1. **Given** 任一侧缺失、拒绝、超时或只返回占位值，**When** 比较，**Then** 保留各侧状态和原始回执，不能判成功。
2. **Given** 参考与候选均失败，**When** 汇总，**Then** 记录参考缺陷与候选缺口，不将共同失败判功能对等。
3. **Given** 完整分母全部适用正向与反向用例通过，**When** 汇总，**Then** 才允许完整替代声明；partial surface 的通过仍仅属于该 surface。

### Edge Cases

- 发布 schema 的 Hub 最大集合与无 signing-key 的实际工具列表不同。
- schema 与 source 不符、原始名称重复、required 指向不存在的 property。
- 同名近似工具不能提供 caller vector、混合子查询、图时态或输出预算。
- 上游规则型自然语言 planner 与宪法 XV 冲突：能力用模型／typed grounding 实现，不能照搬规则推意图。
- 上游反馈工具默认发送；本批次不执行任何真实外发。
- 来源、模型、版本、runtime profile 或 scope 变更；只读／恢复／取消／分页不完整。
- Windows／Linux／macOS 原生行为；编译成功不代表各平台运行成功。

## Requirements

### Functional Requirements

- **FR-001**: MUST 固定发布版本、commit、原始 schema／源码与 digest；拒绝移动目标版本或改源文件换通过。
- **FR-002**: MUST 从明确公共结构逐 surface 登记名称、参数、范围、条件、错误与副作用；拒绝用 regex／词表猜名字或行为。
- **FR-003**: MUST 维护 owner 决策与源清单双向覆盖，保留原始顺序和完整 schema 摘要；未知项、重复或缺项必须拒绝。
- **FR-004**: MUST 分开 declared、实际入口和 PASS／FAIL／BLOCKED／NOT_RUN；拒绝由 schema、注册或共同失败获得正向通过。
- **FR-005**: MUST 复用既有 Core／ACP／宿主 owner，并保持 ACL、意图和权限原则；拒绝第二份权威库、规则型语义路由或未经授权外发。
- **FR-006**: MUST 在候选运行前冻结黑盒输入与比较规则，真实返回进入 oracle；拒绝假 binary、候选驱动改金标或仅检查关键字。
- **FR-007**: MUST 汇总全部 surface 的独立分母、依赖、证据和未完成项；拒绝将本契约批次或 032 单独宣布为完整替代。
- **FR-008**: MUST 使用原测试工作流、根 workspace／私有 Studio 边界并记录 native 状态；拒绝新增 SDK、workspace、repo、lockfile、allowlist 扩大或以交叉编译充原生通过。
- **FR-009**: MUST 为每项机制明确唯一实现与权威 owner，追踪全部调用者、政策差异与公开 API；兼容入口只委托共享实现，拒绝在新／旧路径保留重复的投影、刷新、排名或存储机制。

### Key Entities

- **ReferenceSurface**：固定发布的一份公共 schema／路由／命令／类型集合；原文件是权威。
- **OwnerDecision**：手写的明确映射和缺口，不参与产品配置／授权；只缩小资格中的声明。
- **GeneratedInventory**：只从原 schema 与 OwnerDecision 生成，保存 JSON pointer 和完整对象摘要，不能手改。
- **ContractCase／Attempt**：冻结请求与确定性比较规则、各侧身份／状态、原始回执及副作用证据。

## House Checklist

- [ ] 新行为有小型 hermetic tests；T002–T008。
- [ ] 新 closed set 与未知值拒绝；T002/T006。
- [ ] 新 Rust 若有，无无依据 panic，sibling tests 与 owning Clippy；T008。
- [ ] Aster 依赖本地，公开 Rust API 与 workspace 保持；T008。
- [ ] 产品配置保持 ACL，本批次测试数据不授予权限；T004/T008。
- [ ] Evidence：`docs/evidence/code/2026-11-01-xerj-replacement/OUTCOME.md`；T009。

## Success Criteria

- **SC-001**: 固定版每个公共 surface 均有完整且可复核的清单及 source identity；未完成 surface 不隐去。
- **SC-002**: MCP 最大集合精确 22 项；required／字段／全部 schema 摘要、源顺序、Hub 条件保真，完整生成可重复，全部漂移反例拒绝。
- **SC-003**: 每项对应一个实际 owner 和可执行合同；真实 replay 逐项保留两侧状态，不用共同失败过门。
- **SC-004**: 每个已交付能力的正向、拒绝、生命周期、来源、预算与平台资格都有原始记录；所有未运行项阻止总体 PASS。
- **SC-005**: 机制去重账本有逐项实际证据，新增重复机制为零；必须收敛的旧机制已收敛并通过原有和新增测试，不能以“后续重构”记完成。

## Assumptions

- 参考 `v1.0.0-rc.100` / `682d7ce072c644239cfa13ca1a9ca9a0e45d794b`；main 增量独立。
- 不运行付费模型、不提交反馈或 PR、不安装／发布。远程副作用采用本地测试 endpoint。
- 032 与本批次均服务完整替代总目标；各阶段完成不缩小总范围。

## Explicitly Not Claimed

- 单独完成契约批次就实现完整替代。
- 源码声明即证明发布 binary 在当前 profile 可执行。
- 本批次引入第二份数据库、迁移现有 owner、修改模型策略或扩大权限。
