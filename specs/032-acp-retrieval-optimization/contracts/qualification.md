# Qualification contract

**Status**: PLANNED。这里定义验收输入、边界和 oracle，不记录未来执行为 PASS。

## 1. 能力与执行状态分开

- 能力来源：Core 原生、宿主提供、当前宿主未提供。沿用产品原有声明，不新增产品 registry。
- 执行状态：PASS、FAIL、NOT_RUN、BLOCKED。它们仅为测试记录的 closed set。
- 正向能力缺资源时记 BLOCKED，不能因为“正确拒绝”改记该功能可用。
- 对当前 profile 本来就不提供的能力，运行相应缺席／拒绝用例；不要求纯 ACP 具有 Studio 资料库。
- `source_review`／knowledge compile 等受限会话保留现有能力上限。

## 2. 三个 profile

| Profile | 固定前置 | 需证明 |
| --- | --- | --- |
| C0 | 无宿主服务，本地临时工作区，语义关闭 | grep／glob／BM25／hybrid 可用，宿主能力准确缺席。 |
| C1 | 同上，受信任 `workspace_retrieval` 开启 local_cpu | 真实 bundled provider、增量与取消；不开远程网络。 |
| H1 | 真实 Studio owner／路由，明确的 Wiki／memory／Office 等能力 | 实际 ACP-to-host 请求、原有资料／记忆／索引、回执和生命周期。 |

本地脚本 LLM 可以精确请求工具，证明 harness；不能据此宣称模型会自主选择正确工具。传输故障 stub 可用于反例；正向 PDF／OCR／SAG 资格使用实际 native 实现与合成资料。

## 3. 用例分母：36 项

同一用例可以由多个 owning test 和一个 process test 联合证明。计数按下列 ID，不能按测试函数数量替换。所有用例当前为 NOT_RUN（本 epic）。

| ID | 场景与关键 oracle | Profile／owner | 任务 |
| --- | --- | --- | --- |
| Q01 | BM25／hybrid 真正返回文件中的后段证据及准确行号 | C0／Core+ACP | T002、T007 |
| Q02 | semantic 关闭：工具定义不宣传，直接调用准确拒绝 | C0／ACP | T006、T007 |
| Q03 | semantic 开启：真实 bundled provider 按含义命中，排除无关文档 | C1／ACP | T002、T007 |
| Q04 | 宿主能力 absent／false／true：工具与 handler 声明一致 | C0/C1/H1／ACP+host | T006、T008 |
| Q05 | 权限 deny、受限会话、未经准入的 scope 不执行后端 | 全部／ACP+host | T008 |
| Q06 | 依赖失败时，hybrid 保留可用通道并报告实际 fallback | C1／Core+ACP | T007、T012 |
| Q07 | 单文件 edit 只重新处理变化内容，结果 revision 正确 | C1／Core | T011、T012 |
| Q08 | delete 后不返回已删文件作为当前证据 | C0/C1／Core | T011、T012 |
| Q09 | rename 后新路径正确、旧身份不重复出现 | C0/C1／Core | T011、T012 |
| Q10 | 同字节保存／重复查询：文档 embedding 增量为 0，来源复核仍执行 | C1／Core | T011、T012 |
| Q11 | .env、generated、超限／二进制文件沿原策略排除并体现 coverage | C0/C1／Core | T011、T012 |
| Q12 | 查询期间真实来源改变，不把旧代视作当前，必要时 partial／retry | C1／Core+ACP | T012 |
| Q13 | Wiki current 返回原有 publication、来源页和版本 | H1／host+ACP | T013、T020 |
| Q14 | Wiki all 跨 owner，3 秒预算和 continuation 保真，无跳项／重复归因 | H1／host+ACP | T013、T017、T020 |
| Q15 | memory current／all 分清 durable／session／working；关闭任务不隐式恢复 | H1／host+ACP | T018、T020 |
| Q16 | OOXML 持久文件读出真实段落／单元格／幻灯片位置和类型 | C0/H1／ACP+Office | T014、T020 |
| Q17 | PDF 文字层：页码、source digest、next_page、完整覆盖如实返回 | H1／host native | T015、T020 |
| Q18 | 扫描 PDF／图像：真实 OCR；缺 assets 时为 BLOCKED 正向资格 | H1／host native | T015、T020 |
| Q19 | SAG 原文片段检索可从 Agent 到达宿主、核对原文版本；Wiki 不能冒充原文命中 | H1／host+ACP | T016、T020 |
| Q20 | 宿主模型／处理器缺失、超时、分页未完：工具准确表达，不安装替代后端 | H1／ACP+host | T008、T015、T020 |
| Q21 | 记忆语义默认关闭：旧存储完整，零语义准备／调用 | C0/H1／memory owner | T021、T025 |
| Q22 | 显式开启：旧记录被有界回填，同义查询命中正确 ID／revision | C1/H1／memory owner | T022、T024、T025 |
| Q23 | 关闭重开／进程重启：派生索引可验证地复用或重建，记忆不丢 | C1/H1／memory owner | T023、T025 |
| Q24 | update／delete／prune 后召回仅包含当前有效记录 | C1/H1／memory owner | T022、T023、T025 |
| Q25 | model／dimension／来源版本不匹配：原投影不被冒认有效，不写零向量充数 | C1/H1／memory owner | T023、T025 |
| Q26 | 只读／锁占用／部分 embedding 失败：无源记录损失，状态准确、可重试 | C1/H1／memory owner | T023、T025 |
| Q27 | CPU 编码期间取消，挂起 prompt 返回 cancelled 与原生 worker 收尾分别有证据，晚结果不发布 | C1／embedding+ACP | T010、T012 |
| Q28 | 宿主断连／取消／超时：沿原 owner 释放，响应不被误投到别的 Run | H1／ACP+host | T008、T020 |
| Q29 | 查询中切换真实 library／workspace binding：旧根结果被拒绝或丢弃 | H1／host | T013、T016、T020 |
| Q30 | 两会话共享 bundled model，索引状态独立；关闭后 index records 清零且另一会话可用 | C1／ACP+Core | T009、T012 |
| Q31 | 指定 engine 必须实际执行；坏路径／坏协议失败；占位排序器不计产品资格 | harness／ACP tests | T002、T003 |
| Q32 | 控制 provider 与真实 provider 分栏；每个质量分数可由冻结排名／标注复算 | harness／ACP tests | T003、T026 |
| Q33 | 固定类别和 exact symbol／当前引用门不退化，重排策略不偷换查询或金标 | Core+ACP | T026、T027 |
| Q34 | 冷／热、1／4／8 并发、provider 调用和真实 RPC 开销分别计量 | 全部 | T004、T005、T028、T031 |
| Q35 | owner close 循环与 accounted bytes／RSS 报告分开；超额如实部分覆盖或拒绝 | Core+host | T019、T028、T031 |
| Q36 | 变更相关正反例在各原生目标有独立记录；cfg(unix) 跳过不能计 Windows 通过 | platform | T029、T030、T031、T032 |

## 4. 质量语料与 oracle

T003 在优化前冻结 `retrieval_qualification_v1.json`（拟放 ACP 的 `tests/fixtures/`），至少 60 条，六类各至少 10 条：exact symbol、行为含义、架构关系、中文／中英混合、长期记忆同义查询、资料引用／无匹配边界。分类来自编写者／标注者声明，产品不按词表猜类别。

每项含：query ID、资料 SHA-256、query、允许 profile／mode、真实来源 ID、金标范围或 memory ID／revision、grade 与负例、输出预算。标注有 provenance；优化开始后不得改标注争取通过。既有 mini corpus 和 grader 不改，新增资格集版本与其分开。

规范化必须在冻结时决定：返回 chunk、行范围、页或 memory ID 如何与金标匹配；重复命中如何计一次；无匹配如何判定。不得在看到候选结果后重解释范围获得加分。

报告 Recall@5／@10、MRR、NDCG@10，保留每类／每模式数据；无匹配 false positive、引用当前性和 exact symbol 正确性单列硬门。`chars/4` 只能标 `approx_tokens`；主要开销报告用实际输出字节、文件读取字节和 provider 次数。

新增质量目标在 T003/T005、任何优化前冻结，不能用已有 stand-in 分数作基线：

| 项目 | 拟定工程目标 |
| --- | --- |
| 明确的 exact symbol fixture | Top-1 正确率 100%；保留既有更严格的单例门。 |
| 有相关答案的查询 | 每类 Recall@10 ≥0.80；全体 NDCG@10 ≥0.75。资格分数只取下文事前固定的 profile／能力主路径；其它模式独立报告，不取逐查询最佳值。 |
| 对照回归 | 候选整体 NDCG@10／MRR／Recall@10 不低于相同真实基线；每类下降需保留 FAIL 和解释，不能用其它类别平均收益掩盖。 |
| 来源与范围 | 错 owner／revision、已删除来源、伪造原文引用的次数为 0。 |
| 无匹配负例 | 不伪造“已确认相关证据”；候选型搜索的非空列表和确定答案分开，按冻结规则报告 false positive，不能事后重解释工具含义加分。 |

无相关金标的查询不混入 Recall／NDCG 正例均值，其数量和 false-positive 分母单独保存。新增目标如未达到，保留失败与改进任务，不降低标准或挑选更简单的查询。新通道尚未接入时标 NOT_RUN，不虚构其基线为 0。

主验收路径在优化前固定：C0 工作区使用 `search(mode=hybrid)` 的 model-free 路径；C1 工作区使用同一 hybrid 加实际获准的 bundled semantic 通道；H1 的 Wiki、记忆、原文资料分别调用 `search_wiki`、`search_memory`、经资格验证的原文搜索入口。资料 SAG 初始策略固定为现有 Fast；Precise 独立消融。单独的 BM25／semantic／MMR 只列策略对照，不在看到金标后为每条 query 选择最高分模式。真正变更主路径需先冻结新策略，再对同一输入完整重跑。

T026 分为独立的工作区、宿主资料与记忆子资格。工作区质量在 T003/T007 后即可建立基线并随 T009–T012 重跑；宿主或记忆未完成只阻塞自己的子资格。全部三个子资格完成前，T026 整体仍未完成，不能缩减完整目标。

既有脚本的占位 ranker 只作 metric／harness 自检。真实执行路径优先复用 ACP stdio fixture，采用当前 protocol 请求／实际工具调用，结构化导出真实结果供 Python 指标函数读取。新增 adapter 只属于测试，不是 Core 的 Python SDK。`--engine` 不支持某种输入协议时必须明确失败，不增加一个假产品 CLI 来适配旧脚本描述。

## 5. 每次 attempt 的证据

记录：输入／标注／model asset digest，root 与 Studio submodule SHA 及 dirty source 摘要，binary digest、features、OS／arch／toolchain、profile、case、seed、测量边界、状态、实际调用／耗时／结果摘要。保存原始记录与独立汇总，不保存用户资料或凭证。

已有只返回文字的错误不允许以易变渲染文本驱动新增重试／路由逻辑。确需新错误区分时在原 owner 边界补 typed error／稳定 code，并带兼容测试；不重建通用 error framework。
