# Feature Specification: ACP 检索闭环与性能优化

**Feature Branch**: 当前 checkout；未请求新分支或 worktree。

**Created**: 2026-11-01

**Status**: Implementing；按用户授权实施与端到端验证，逐任务记录证据。

**Program scope**: 用户进一步要求完整替代 Xerj。本 spec 保留为检索基础阶段；这里的 36 项验收与排除项只界定本 slice，不界定总目标。完整替代由 [总计划](../../docs/xerj-replacement.md) 逐项跟踪，不能完成本 spec 就宣称整体替代完成。

**Input**: 对照 Xerj 思考 Aster 的检索、索引、记忆机制；用户要求这些机制通过 ACP 可用、有效、高效，并明确部分能力应由宿主完成；现要求详细优化计划。

**Gate context**: Product regime。覆盖 Core、ACP、共享检索库与私有 Studio 宿主的检索调用闭环。承接 011-workspace-retrieval、012-retrieval-benchmark、013-minishlab-absorption、018-local-memory-studio，保留其接口与已有验收口径。本计划的新增价值是跨边界可达性、真实质量测量和端到端开销，而非重建检索系统。当前依据是 [2026-11-01 检查记录](../../docs/evidence/code/2026-11-01-acp-retrieval-qualification/OUTCOME.md)。

## User Scenarios & Testing

### User Story 1 — Agent 能使用当前宿主声明的检索能力（Priority: P1）

Agent 通过 ACP 使用本地工作区搜索、宿主 Wiki／记忆搜索及获准的文档服务。数据目录、跨工作区范围、后台入库与索引生命周期继续由其原有 owner 管理。

**Why this priority**: 工具可见且能够到达真实实现，是相关性和性能优化的前提。宿主能力缺席、被拒绝和索引尚未完成必须准确表达。

**Independent Test**: `aster-acp` 实际 stdio 进程 + 受控 ACP 客户端／本地脚本模型 + 真实检索实现；Studio owning tests 验证实际宿主路由。传输故障可以注入，检索和来源验证不能用复制的业务逻辑代替。

**Acceptance Scenarios**:

1. **Given** 纯 ACP 工作区，**When** 执行获准的 BM25／hybrid，**Then** 返回当前来源片段；未开启的 semantic 不被宣传为可用。
2. **Given** 受信任配置开启本地语义后端，**When** Agent 按含义检索，**Then** 真实 bundled provider 参与召回，返回正确文件、范围和版本。
3. **Given** Studio 显式声明 Wiki／记忆／文档能力，**When** 工具调用宿主，**Then** 由宿主解析 profile、根目录和 owner，ACP 保留引用、覆盖率及 continuation。
4. **Given** 宿主能力缺席、owner 失联或范围拒绝，**When** 请求进入，**Then** 工具列表和调用结果准确说明不可用／拒绝；不得创建替代数据库或扩大检索范围。
5. **Given** 宿主已有资料原文 SAG 搜索，**When** Agent 执行资料检索旅程，**Then** 先验证既有桥接；只有旅程确实不可达时才添加最小只读桥，计算与资料生命周期仍留宿主。

### User Story 2 — 用户快速开始工作，重复查询复用已完成的工作（Priority: P1）

用户能区分会话可交互、词法索引就绪和语义覆盖完成。热路径复用模型与可复用的既有索引；编辑、删除、取消后不返回过期内容作为当前证据。

**Why this priority**: 已测到完整 ACP 冷路径约 5–7 秒，但无法归因于单个阶段；继续凭局部微基准优化会选错目标。

**Independent Test**: `aster-acp` 阶段耗时和 provider／文件读取计数；Core manifest、persistent index 与 semantic lifecycle owning tests；同一发布构建和固定工作负载的冷／热对照。

**Acceptance Scenarios**:

1. **Given** 首个、同进程第二个及重启后的会话，**When** 建立并检索，**Then** 分开记录初始化、模型载入、目录发现、索引、查询和协议开销。
2. **Given** 同一内容重复查询或同字节保存，**When** 请求执行，**Then** 不重复载入模型、不重新生成未变化文档的 embedding；必要的来源复核单独计数。
3. **Given** 文件被修改、删除或改名，**When** 索引发布新代，**Then** 后续结果引用当前内容；覆盖未完成时明确报告 partial／pending。
4. **Given** 原生计算或宿主查询正在执行，**When** 用户取消／关闭会话，**Then** 前台及时响应，旧结果不再发布；原生 worker 的实际收尾与取消确认分别记录。

### User Story 3 — 本地长期记忆能按含义召回，并保持原有事实与生命周期（Priority: P2）

显式启用本地记忆语义召回的宿主，可以找到词面不同的相关历史记录。旧记录、更新和删除均参与当前投影；不开启时保持现有词法路径。

**Why this priority**: ACP 当前 `LazyFileMemoryStore` 的底层语义 provider 为 None；单独调用 setter 也只影响后续 put，无法覆盖已有记录和重启。Core 同时已有另一套 typed durable semantic／refresh API，需要先做兼容对照，避免重复建设。

**Independent Test**: owning-package 临时存储 fixture + 真实本地 embedding；旧记录回填、重启、更新、删除、只读 owner、失败恢复；ACP 与宿主 memory 查询分别验收，不把其中一个视为另一个通过。

**Acceptance Scenarios**:

1. **Given** 无语义配置的存量记忆，**When** 打开 ACP，**Then** 原有记忆和词法检索不变，不自动改写记录或提升 Candidate／Superseded 等状态。
2. **Given** 显式获准的本地语义记忆策略及已有记录，**When** 投影完成后执行同义查询，**Then** 命中正确 ID／revision，并说明实际参与的通道。
3. **Given** 修改、删除、prune 或重启，**When** 再次召回，**Then** 旧版本／删除记录不被当作当前记忆，投影失效不会损坏权威记录。
4. **Given** 模型身份变化、只读存储或 embedding 失败，**When** 初始化／查询，**Then** 不伪造有效向量或 Ready 状态；按现有策略明确降级词法或拒绝语义操作。

### User Story 4 — 优化结论来自真实实现和固定验收（Priority: P1）

维护者能重现质量、时延、资源和可达性结果，知道哪些是控制夹具、哪些是实际 provider、哪些经过真实 ACP／宿主边界。

**Why this priority**: 当前 `run_quality.py` 接收 `--engine` 但未执行该程序；部分 Rust 基准使用 deterministic／orthogonal provider。它们不能充当真实语义质量成绩。

**Independent Test**: 质量 harness 的 subprocess 调用负对照、引擎身份校验、固定标注与标准指标自测；真实 ACP binary 的同一查询对照；原始结果与汇总可逐项复算。

**Acceptance Scenarios**:

1. **Given** 指定 `--engine`，**When** 运行质量评估，**Then** 实际执行该目标；不可执行／协议不匹配时失败，不得只把路径写入报告。
2. **Given** 固定语料、查询、标注、模型和预算，**When** 比较 BM25／hybrid／semantic／rerank，**Then** 不改 grader 争取通过，按模式与类别分别报告 NDCG、Recall、MRR 和输出体积。
3. **Given** 默认源码基准与发布性能资格报告，**When** 汇总，**Then** 分清 debug／release、局部调用／完整链路、accounted bytes／RSS 和样本最大值／p95。

### Edge Cases

- 宿主已声明能力但断开；声明与实际 handler 不一致；模型仍持有旧工具定义。
- 来源在查询、宿主读取或结果组装期间发生真实修改；索引落后一代。
- 空语料、超限文件、二进制文件、秘密／generated 路径、损坏持久化索引。
- Wiki 部分工作区正在构建；32 个候选与 3 秒预算触顶；continuation 重复或跳项。
- OCR 模型／native companion 缺失；扫描 PDF 与已有文字层 PDF；页级覆盖不完整。
- 同一记忆目录已有活跃 owner；只读打开；回填中断；模型／分块身份变化。
- 中文、英文、混合语言、空格和非 ASCII 路径；Windows 原生路径与进程行为。
- 同时 1／4／8 个查询、两会话复用模型、会话关闭后迟到结果。

## Requirements

### Functional Requirements

- **FR-001**: MUST 按宿主／Core／ACP 的实际职责登记能力、原有 owner 和调用入口；拒绝以 UI 命令存在或 crate 存在代替 Agent 可达证明。
- **FR-002**: MUST 保留现有会话权限、profile／workspace 来源绑定和显式能力声明；拒绝由提示文字、任意模型路径或缺席的宿主授权访问。
- **FR-003**: MUST 使评估指定的 engine 真正执行，并标记 provider 和边界；缺目标、缺样本、协议错误或控制排序器替代产品时拒绝资格结论。
- **FR-004**: MUST 分阶段度量启动与查询，保留 source／binary／submodule 身份；拒绝把局部耗时、代理 token 估值或向量计数当完整时延、实际 token 或 RSS。
- **FR-005**: MUST 返回可验证的来源、版本、范围和 coverage／continuation；拒绝陈旧、跨范围或伪造归因，未完成覆盖不能报告“没有匹配”。
- **FR-006**: MUST 复用进程内 bundled model、既有持久词法索引和增量机制；拒绝为热查询重复载入模型或重嵌入未变化文档。来源复核不能因优化而移除。
- **FR-007**: MUST 保留前台取消、deadline 与真实 worker owner 的分离；拒绝在取消后发布旧结果，不将丢弃 `spawn_blocking` handle 宣称为计算已停止。
- **FR-008**: MUST 通过原有宿主执行资料入库、OCR、跨工作区查询与 SAG 资源操作；缺少 Agent 桥接时只补对应操作，拒绝复制宿主数据库或把编排强迁到 ACP。
- **FR-009**: MUST 为本地记忆语义召回复用现有实现并覆盖存量记录、更新、删除、prune、重启；拒绝仅 setter 接线、零向量占位或第二份权威记忆库。
- **FR-010**: MUST 显式控制记忆语义启用，保持默认兼容；workspace 语义开启不能自动授予另一种记忆政策，未获准的远程 embedding／rerank 不得启动。
- **FR-011**: MUST 冻结工作负载、标注、限额与 grader，再实施和对照；失败必须保留，拒绝调整输入、超时或阈值换取通过。
- **FR-012**: MUST 用固定质量／性能证据决定是否推广 rerank，或另开 ANN／跨会话向量缓存任务；拒绝把存在 HNSW crate、精确扫描或会话重建本身视为优化依据。
- **FR-013**: MUST 对变更执行 owning tests、支持 features 的 Clippy、格式与工作区 guard，并构建 CLI／ACP 配对；拒绝以 macOS 或交叉编译代替原生 Windows／Linux 验证。
- **FR-014**: MUST 按阶段记录通过、失败、未运行、宿主缺席和环境阻塞；拒绝把本计划或旧测试成绩标作新功能已完成。
- **FR-015**: MUST 按最新目标收敛同类机制，复用已有 embedding、索引、刷新与排序 owner；公开 API 可用薄 adapter 保持兼容，拒绝为旧／新记忆路径再建类似机制或第二份权威存储。

### Key Entities

- **能力条目**：现有能力和接口的规划映射，owner 为对应组件；不是新增产品能力注册中心。
- **来源身份／publication**：由工作区、记忆或宿主资料 owner 维护；ACP 传递其身份，不另行创造来源事实。
- **检索结果**：命中、来源范围、revision、通道与覆盖状态；沿用各已有 DTO。
- **记忆语义投影**：权威记录的可重建派生物；owner 随当前记忆命名空间，不改变记录激活政策。
- **Qualification attempt**：固定输入与源／二进制身份的原始运行事实，属于测试证据。

## House Checklist

- [ ] 每个行为变更带 owning-package hermetic test：见 T002、T004、T006–T028。
- [ ] 新 closed set 具有 parse／as_str 与未知值拒绝：见 T006、T016、T021。
- [ ] 新／变更生产 Rust 不引入无依据 panic：见 T029。
- [ ] Aster 依赖保持本地、公开 API 与根 workspace：见 T001、T021、T029。
- [ ] 如确需产品配置，只扩展受信任 ACL：见 T021；未证明需要则不添加配置。
- [ ] 记录 evidence：见 T032；目标 `docs/evidence/code/<execution-date>-acp-retrieval-optimization/OUTCOME.md`，规划不代替证据。

## Success Criteria

### Measurable Outcomes

- **SC-001**: [验收矩阵](contracts/qualification.md) 的 36 个用例均有明确结果；每个已宣传的能力，其对应正向用例必须 PASS，不能由负向拒绝用例替代。
- **SC-002**: 新固定质量集至少 60 条查询，六类各至少 10 条；既有 fixture／grader 保持不变；真实 engine／provider 独立计分，达到 qualification 中的每类 Recall@10、整体 NDCG、exact symbol 与来源正确性目标。
- **SC-003**: 发布构建满足 [性能目标](contracts/performance.md)；既有更严格 gate 保持。未通过的档位不得宣称已优化完成。
- **SC-004**: 同内容保存、编辑／删除和同进程第二会话的计数证明复用与增量行为；取消后的旧 publication 数为 0。
- **SC-005**: 记忆语义查询在旧记录、重启、修改／删除场景命中当前 revision；关闭或失败路径保持原有词法可用性和记录完整性。
- **SC-006**: 根／私有子模块的改动与证据分开，native 平台状态、既有失败及未覆盖范围均有原始记录。

## Assumptions

- 已授权按规划开发和端到端验证；不运行付费模型或发布，不放宽固定 grader／输入／超时。
- 受信任的现有本地 embedding 是第一选择；不安装第二个服务、不隐式下载模型。
- 各宿主可提供不同能力。宿主缺席时的合理降级不算功能缺陷；已声明但不可达才是缺口。
- 新目标先在固定发布构建和指定性能机上冻结；真实模型自主选工具的测试若无固定版本／预算，明确不运行。
- 无需新的用户事实即可开始 P0–P2；记忆接口兼容与可选资料搜索桥在相应技术决策任务中收敛。

## Explicitly Not Claimed

- Xerj 全部数据库／聚合／ES API／集群／CorpusHub 功能对等，或复制其宣传性能数字。
- 重建宿主资料库、把 OCR／入库作业搬到 ACP、统一所有存储或改变租户身份。
- 默认启用远程向量或重排；自动迁移／合并／删除用户记忆；新建第二份权威存储。
- 未经测量就上线 ANN、跨会话语义向量缓存、分布式索引或新依赖框架。
- 新 GUI 设计、通用 SQL／聚合工具、新 MCP／HTTP 服务或 Node／Python／Go SDK。
- 此规划阶段的产品性能或兼容性已验收；历史 20 项回归只属于此前 slice。
