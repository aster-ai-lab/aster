# Aster Core 机制完整替代 Xerj

当前目标是“实现 aster core 中各项机制对 Xerj 完全替代，不出现重复和类似的机制实现，确保功能有效且高效”。验收单位是机制及其权威 owner：复用已满足的实现，收敛重复实现，真实缺口才补到原 owner。032 是检索基础阶段，不能作为总目标的完成证明。

上游 CLI／MCP／HTTP／gRPC 是发现行为、构造测试输入和比较结果的对照入口。Core 通过原 Rust API 与现有 ACP／宿主路径提供机制；不因为某个上游入口存在就新增一套兼容运行时、数据库或服务。资料、OCR、跨工作区资源继续由宿主拥有。

## 对照身份与完成含义

- 第一版对照发布：`v1.0.0-rc.100`，commit `682d7ce072c644239cfa13ca1a9ca9a0e45d794b`，2026-10-27 发布。
- 同日 main 观察点：`da474292400f46dac2f5a48e5b07c7e84dd69221`。main 增量单列，不能悄悄移动发布验收分母。
- 上游出处、原文和 SHA-256 保存在 [对照证据](evidence/code/2026-11-01-xerj-replacement/)。这些是只读研究资料，没有把上游工程或其 Cargo workspace 加入 Aster。
- “完整替代”要求固定发布实际提供的各项 Core 机制都有对应实现和黑盒证据，包括成功、错误、取消、持久化、恢复、规模和平台行为。支持某个库或显示工具名不算通过；同一机制不能由两个权威 owner 分别实现。
- 上游 roadmap 中的未实现功能和已知缺陷必须与已交付能力分开。记录兼容差异；不以复现数据丢失、越权或崩溃作为兼容要求，也不把对照自身的未实现功能宣传为已提供。
- Aster 的职责约定继续有效：Core 提供可嵌入机制，ACP 提供获准 Agent 调用，宿主管理资料、OCR、跨工作区范围及持久资源。协议兼容层调用同一 owner，不建立第二份权威数据。

## 能力分母：先逐接口展开，再实现

下表是机制分组，不是完整分母或完成成绩。每组须从固定版本的实际结构、公开行为与测试展开，映射到 Core／共享库／宿主的原 owner，标明复用、收敛、补缺或仅为 adapter。未核查的条目保持未审计，不能靠字符串搜索推定功能已实现。

| 分组 | 必须覆盖的行为 | Aster 复用方向／职责 | 当前结论 |
| --- | --- | --- | --- |
| 自动入库 | 文件类型、数据集发现、schema、增量／watch、恢复、删除、资源预算、错误记录 | Core manifest + 宿主现有 ingest／scan／watch | 部分基础存在；全部格式与行为待逐项审计 |
| 数据格式 | 源码、文本、日志、JSON／JSONL、CSV、SQLite／SQL dump、HTML／XML／YAML、邮件、Notebook、电子书、Office／旧 Office、PDF 等固定版实际 extractor | 原 Office／OCR／宿主 processor；按原契约处理二进制来源 | 不以现有 OOXML／PDF 支持冒称全格式对等 |
| 检索与代码理解 | BM25、向量、hybrid、过滤、排序、分页、精确符号、定义、批量查询、片段与引用 | `aster-vec`、Core retrieval、现有语言／结构化能力；经 ACP 可达 | 032 已开始真实 engine 资格；仅 hybrid 主路径有新成绩 |
| 查询语言 | 固定版的 typed Query DSL、query-string、SQL、脚本及日期／空值／数组语义 | 先审 `aster-vec` 的过滤与 typed query；缺口在原 owner 补齐 | ES DSL／SQL 对等未证明 |
| 聚合分析 | 固定版实际提供的 metric、bucket、pipeline 聚合、日志分析与关联能力 | 原数据 owner 执行；ACP 只传获准 typed 请求 | `group_by` 不能代替完整聚合语义；待审计 |
| Agent 记忆与知识 | 命名空间、存量语义召回、CRUD、生命周期、跨会话、图关系／Second Brain／decide 等已交付行为 | Core memory + 宿主 Wiki／SAG；沿原 activation／retention | 032 记忆投影尚未完成；其它行为待审计 |
| Corpus 与参考资料 | pack 构建、来源适配、去重、身份、签名验证、目录／锁定／同步、Hub 检索及许可证 | 宿主资料 owner；复用本地签名／归档／下载组件前先核对契约 | 当前没有完整 Hub／pack 兼容证明 |
| 调用与批量编排 | 各对照入口体现的字段语义、顺序、每项错误、取消、分页和预算 | 原生 Rust／ACP／宿主调用同一机制；adapter 只转换契约 | 需证明实际可达；不复制兼容执行引擎 |
| 索引／存储生命周期 | schema、文档 CRUD／bulk、版本、持久化、WAL／恢复、压缩、保留、维护 | 优先 `aster-vec` 与现有存储 owner；保持原 Rust API | 不以搜索成功推定持久性和恢复完成 |
| 备份与迁移 | snapshot／restore、reindex、版本升级、可验证迁移与失败恢复 | owner 管理源与派生状态；用户原资料不被覆盖 | 尚无完整迁移资格 |
| 远程数据与共享 | 已交付对象存储／watch、sharing、远程 embedding／rerank／judge／WAL tap 等边界 | 宿主已有授权与 egress 政策；明确 opt-in | 本地默认不隐式触发远程行为；资格待建立 |
| 集群 | 固定版实际提供的路由、Raft metadata、分片、副本写入／确认、日志／快照恢复 | 先检查既有共享组件与部署 owner；不能把会话当分片 | 未实现或未证明；上游自身 partial 项另列 |
| 管理与可观测性 | 固定版 console／管理 API、权限、统计、健康、进度、用量、调试与资源控制 | 原宿主管理入口、tracing 与 metrics；保留租户隔离 | 现有 tracing 不能代替完整运维行为 |
| 交付与跨平台 | Windows／Linux／macOS、CPU／ABI、完整 companions、离线运行、升级／卸载 | 原 paired CLI／ACP 和 Studio 发布工作流 | 当前仅 macOS 开发构建／focused 验证；不是平台资格 |

## 实施顺序与退出门

1. **固定机制分母与去重账本。** 展开 query／extractor／存储／记忆／图／编排／资源操作，把对照入口映射到机制及原 owner；记录 actual path、重复实现、行为差异和验收 case。留存原文与许可。清单未完整前，不给“已替代百分比”。
2. **完成检索基础阶段 032。** 真实 engine、质量集、阶段观测、完整调用链、宿主资料、记忆及性能。保持其冻结输入和门限。新增总目标不允许重写已有成绩。
3. **先收敛，再补缺。** 追踪重复候选的全部调用者及政策差异，保留一个机制实现与权威数据 owner；公开 API 以薄 adapter 委托同一实现。保持源数据、权限、activation 和 retention；不为旧／新路径复制投影、刷新或排序算法。
4. **完成真实功能资格。** 同一冻结输入通过对照入口与 Aster 原生 Rust／ACP／宿主路径测同一机制。事先明确顺序、浮点容差、动态身份、分页和错误语义。新增实现同时交付调用者与 hermetic tests，不以额外 server 或 SDK 代替 Core 实现。
5. **Corpus、运维和部署。** 完成签名／损坏包、断线／断电恢复、迁移、远程 source 及集群的实际行为。需要多个进程时用既有测试 runner；不能用 mock 节点宣称集群通过。
6. **统一最终资格。** 同一最终 source／binary、同一机制分母，复跑功能、实际调用、相关性、吞吐、尾延迟、资源、恢复和原生平台，并复查调用图与去重账本。所有适用机制 PASS 且没有重复机制，才可宣称完整替代；FAIL／BLOCKED／NOT_RUN 均保留并阻止该总声明。

这些是分批交付顺序，不是将总目标缩成第一批。每个实现批次沿仓库 spec → plan → tasks → analyze → implement 工作流建立有界 epic，写明接口和 hermetic 测试。涉及同一 owner 的写入串行；当前 checkout、无新增 worktree、无自动多 agent 委派。

## 性能与兼容口径

- 032 的 60-query 合成集和 36-case matrix 继续作为基础资格，不代表此总计划的全部分母。格式／协议／聚合／集群／迁移必须有对应独立测试。
- 性能与对照使用相同数据、查询、模型策略、硬件和预算。控制 embedder、真实本地模型、远程组件分栏；不能把对照的局部微基准当 Aster 整体目标或以少索引数据换速度。
- 原有测试、评分器、门限和超时不放宽。新的兼容断言先冻结再实现；对照本身失败时记录参考缺陷，不能直接把 Aster 同样失败记为通过。
- Aster 运行时不依赖已安装的 Xerj、Xerj sidecar 或其数据库。参考 binary 仅能用于独立资格测试；不能转调它来声称实现替代。
- 配置仍由 `aster-acl` 解析 ACL；数据迁移不解析或引入上游的产品配置格式。保留公开 Rust API、local dependencies、workspace／子模块及 tenant 边界。
- 尚未审计完整工作量，原 14–23 人日只属于 032。此文不承诺全量替代能在该时间内完成。

## 当前证据与未完成项

[032 执行记录](evidence/code/2026-11-01-acp-retrieval-optimization/OUTCOME.md) 保存真实 engine、冻结输入、原始排名和测试结果。当前基础资格仍在实施；机制去重、完整聚合、Corpus／pack、集群和迁移资格尚未完成，不能由检索测试替代。

[033 替代契约](../specs/033-xerj-replacement-contracts/spec.md) 将对照入口归并到机制、原 owner 和双目标资格。固定的 22 项 MCP schema 是对照数据；不要求 Core 重建 22 个平行工具或一套 Xerj 运行时。
