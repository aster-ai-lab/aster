# Architecture: 工作区产物与百科（优化目标）

**Status**: 设计优化稿（2026-11-03）。约束仍以 [spec.md](spec.md) / [contracts/workspace-wiki.md](contracts/workspace-wiki.md) 为准；本文收敛实现接缝与下一阶段切口，不另开引擎。

**原则**: 保留 **staging → 校验/审阅 → 整代发布**；不学 WeKnora 后台直写覆盖；不平行开 Map-Reduce 编译 epic。先收口 T009/T012 与产品面合同，再做无 LLM finalize 与同 scope 检索增强。

---

## 1. 三平面内容模型（替代「五标签平权」）

工作区 Wiki 详情不是五个对等产品，而是三层平面。标签只是入口，所有权与数据根不同。

| 平面 | 资源 | 根 / 权威 | 用户意图 |
| --- | --- | --- | --- |
| **证据 (Evidence)** | **产物** | `sourceRoot`；原生文档 / 工作区文件 owner | 看、改任务或项目真实文件 |
| **知识 (Knowledge)** | **百科** | `outputRoot`（`sourceRoot/wiki/workspace/<binding>/`）；OKF 发布账本 | 阅读/检索派生理解；触发编译维护 |
| **轨迹 (Trace)** | **分析**、**看板**、**记忆** | 会话/计划/记忆 owner；不写百科页 | 回放执行、跟踪交付、查长期记忆 |

### IA 规则（产品）

1. **知识面默认**：有已发布百科 → 默认「百科」；否则「产物」或带生成入口的空百科。不把「分析」当作 Wiki 首屏（分析是轨迹面）。
2. **任务详情**：只露 分析｜百科｜产物。记忆与看板仅项目（或合同明示的范围）；禁止在任务 Wiki 伪装项目级记忆。
3. **文案**：工作区「产物」≠ 个人「笔记」。共用编辑器壳，不共用「笔记」文案与空态。
4. **合同回写**：`spec` / `contracts` 须登记「分析」为轨迹面资源；原「产物／看板／百科／记忆」序改为平面优先，不强制旧 tab 序。

### 根隔离（硬不变量）

```text
sourceRoot  ──只读快照──►  staging/_sources/…
     │                         │
     │ 禁止                    ▼
     │                  staging/concepts/… ──finish/review──► outputRoot/concepts/
     │                         ▲
     └── egress 排除 wiki/workspace/**、密钥、会话定位文件、依赖目录
```

- 产物编辑改的是 `sourceRoot`；百科维护只改 staging / 发布树。
- 发布树不得再进入下一次源准入（防回灌）。
- 同范围复用要求：主体、canonical root、读取范围、egress、源代次一致。

---

## 2. 所有权与运行时（目标切分）

| 层 | 拥有 | 不拥有 |
| --- | --- | --- |
| **Studio native (agent-plane)** | 绑定与代次、`wiki-inputs`、maintenance HTTP、citation、启用列表、`sourceRevision`、归档/恢复对 Wiki 资格 | 模型调用、第二套发布账本 |
| **Core** | 有界发现、source-egress、snapshot/coverage、hybrid / WikiIndex 查询原语、aster-vec 会话索引 | 任意 root、跨实例索引服务 |
| **OKF** | job / staging / history / conflict / 整代 publish；workspace adapter | 宿主绑定解析、UI 状态 |
| **ACP** | `knowledgeCompile` Run（含 `workspaceRevision` 每工具复核）、`search_wiki` / `search_memory`、workspace search 扩展 | 自行 begin/finish 绕过宿主 |
| **Renderer** | 目录/详情展示、草稿、维护**适配器**（连上 native/OKF 命令）、编译会话 UI | 自决发布、自扫磁盘、全局第二队列权威 |

### 当前接缝缺陷（须在优化中消化）

1. **维护编排偏 renderer**：`KnowledgeMaintenanceController` 按 `outputRoot` 建实例，进程内全局串行 `runningController`；native 已有 `.wiki-maintenance.lock`。目标：串行与预算以 **OKF/native 锁** 为准；renderer 队列只做 UI 背压，不充当跨窗口权威。
2. **增量靠 10s 轮询启用列表**：`KnowledgeMaintenanceRuntime` 分页拉 `/wiki-maintenance` 比对 `sourceRevision`，只 `refresh`，不等于 T009「过期 → 去抖 → 排队更新」。目标：native `workspace_watch` → 绑定级 `sourceRevision` 推送或短路径失效 → ledger 标页过期 → 预算内 `begin`（用户启用范围内）。
3. **个人库有文件监视失效；工作区 Wiki 未对称**：personal 对 `wiki/local` 源变更 debounce refresh；workspace 应复用同一「源变更 → 计划刷新/过期」模式，而不是仅靠轮询。
4. **分析面未进合同**：实现已默认打开「分析」，021 正文未定义 → 验收与 IA 漂移。按上节平面模型回写即可，不另建知识引擎。

---

## 3. 知识编译流水线（保留形态 + 增量切口）

### 3.1 稳态（已成立，不改信任模型）

```text
用户/启用范围触发
  → host 解析 binding + 准入快照（wiki-inputs / begin 包）
  → OKF begin_workspace_*（冻结 _sources，建 staging）
  → ACP 编译会话（只写 concepts 候选；无 Shell/网络/改源）
  → finish（manifest/引用/代次）→ 可选 review
  → 整代激活；旧代可读；WikiIndex 只见当前 concepts
```

冲突继续用既有码：`knowledge.page_changed`、`workspace_source_changed`、`candidate_changed` 等。不新建冲突子系统。

### 3.2 增量（T009 目标形态 — 非 Map-Reduce）

```text
watch(sourceRoot, egress)
  → debounce
  → 计算受影响页 / 未知则标概览过期
  → 若范围已启用且预算允许：入队单活动 job（同 outputRoot 互斥）
  → begin 时只带变更准入集（或全量快照但跳过无变化 identity）
  → 模型更新候选 → finalize（见 3.3）→ 整代替换或拒绝陈旧候选
```

- **不做**：按 chunk 的 WeKnora 式 Pass0..N 并行写页、自动覆盖正式页、独立 Redis/Asynq Wiki worker。
- **可借鉴**：去抖合并、dead-letter/明确重试、无 LLM 的链接/索引收尾。

### 3.3 Finalize（可选增强，P 级）

在模型 `finish` 之后增加 **确定性收尾**（无 LLM）：相对链接存在性、index/log 一致性、死链降级标记。失败则拒绝本代激活或留下 issue，不静默发布坏图。

### 3.4 与检索（T012）

- 普通会话：异步 aster-vec + 持久 FTS；冷启动可 catalog 回退。
- 编译 / sourceReview：跳过环境向量预热与个人资源装配。
- `search_wiki`：仅已发布 concepts；跨任务默认当前实例已存绑定，`scope=current` 收窄。

---

## 4. 桌面产品架构与 UX（对齐 DESIGN + Arc 动效）

权威视觉/动效合同：根 `DESIGN.md`（Dialogs / Motion and emotional feedback）与 [Arc motion](https://uiarc.dev/r/skills/arc/motion.md)。行为参考 Arc，石墨色板与叠层几何仍属 Aster。

```text
工作侧栏 · 对象行悬停/焦点 Wiki 快捷入口（无分组「更多」目录页，见 DESIGN）
  → 内容区叠层 KnowledgePage(scope)；ActivityBar 保留；后台 inert
        Evidence: 产物
        Knowledge: 百科（默认：已发布时）
        Trace: 分析｜记忆｜看板（项目；任务隐藏后两者）
  → Escape/关闭：原地退出，焦点回触发钮；不隐式 cancel job
```

| 交互 | 动效角色（DESIGN / Arc） |
| --- | --- |
| 打开/关闭 Wiki 叠层 | Dialog 原地：入 160ms opacity+scale 0.98→1；出 200ms 反向；scrim 同相；禁止底滑/侧滑宽 |
| 资源 Segmented tabs | Shared highlight `spring.morph` ~420ms、bounce≤0.16；`aria-selected` 立即更新 |
| 树行首次进入 | 200ms / 4px rise，至多前 12 行 16ms stagger；过滤不重放 |
| 编译/更新等待 | 单一 live 信号 + 稳定 Stop；未知进度不定百分比；完成确认一次后静默 |
| 看板卡片 → 产物 | 同叠层切 tab；卡片可用 snappy press 0.97（非弹层锚点） |
| reduced-motion | 去掉位移/缩放/脉冲/stagger；终态与控件立刻可用 |

可交互线框：[`wireframes/wiki-ux-arc.html`](wireframes/wiki-ux-arc.html)。

**默认 tab 优化（相对当前实现「分析默认」）**：有已发布百科 → 百科；否则产物空态 + 一主键编译。分析留在轨迹面，打开不发起生成。需同步回写 `DESIGN.md` 资源默认句。

看板卡片 → 同弹层切到该任务产物根；头部可切已访问目录；独立动作开原任务。

---

## 5. 优化优先级（替换空泛 WeKnora 对标 epic）

| 序 | 主题 | 架构动作 | 对应 |
| --- | --- | --- | --- |
| O1 | 合同与 IA | 三平面 + 默认 tab；回写 spec/contracts；任务/项目 tab 差保持 | FR-010 / 产品漂移 |
| O2 | 增量权威上移 | watch → 过期 → 去抖队列 → 单 job；弱化 10s 轮询为兜底 | T009 |
| O3 | 维护锁单一权威 | 以 OKF/native lock 为串行；renderer 仅适配 | FR-005/012 |
| O4 | 向量门禁 | 收口 T012 证据与勾选；不新增索引服务 | T012 / FR-015 |
| O5 | 冲突可观测 | 编译中编辑策略 + 冲突码进任务编译 UI | 现有 OKF 码 |
| O6 | Finalize | 无 LLM 链接/索引收尾 | 可选 |
| O7 | 同 scope 检索 | hybrid/wiki 质量；非多租户中台 | FR-007 |

**明确不做（本优化窗）**：多租户知识中台、DataSource 同步、FAQ、Neo4j GraphRAG、取消人审直写 vault、恢复独立 `wiki-question`、任务跨项目拖拽。

---

## 6. 验证焦点（架构级）

- 产物保存不触发百科回灌；百科发布不改源摘要。
- 改绑 / 归档后迟到 `finish` 与 tool 调用拒绝。
- 无变化重开：零新 begin；有变化：过期可见且至多一个活动 job。
- 分析 tab 不发起百科生成；目录浏览零模型。
- personal 重要更新审阅不因 workspace 直发路径被放宽。

证据目录仍为 `docs/evidence/desktop/<date>-workspace-wiki*/OUTCOME.md`。
