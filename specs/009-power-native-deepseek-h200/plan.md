# Implementation Plan: Power 原生 DeepSeek V4.1 / 8×H200

**Branch**: 当前分支 | **Spec/Input**: [spec.md](spec.md)

**Gate context**: 2026-10-22 用户已要求按规划实施并交付 H200 原生部署的访问地址与临时 API Key。SGLang 提供对照，原 XQ 下载继续，不以代理路径替代原生里程碑。

## Summary

先固定执行身份与正确性，再让量化算子、权重与状态在 H200 上高效执行，接入真实八卡 TP/EP，随后优化批处理、prefill、KV 复用与 DSpark。Power 继续拥有通用资源、存储、执行和证据合同；DeepSeek 模型所有者拥有架构、tokenizer、状态和 token 循环。复用现有原生机制及受支持的 SM90 内核，所有性能晋升以匹配实验为准。

2026-10-23 用户扩充完整目标为八卡部署、访问地址/Key、充分利用服务器的推理加速优化及压力测试。X5 基本服务只是中间里程碑；最终完成必须同时关闭 FR-014/015、SC-007/008 及 T027，不因能返回文本就结束优化和压力工作。

## 分阶段路线

| 阶段 | 工程交付 | 进入下一阶段的证据 |
|---|---|---|
| P0 能力与基线 | 本地构建闭包、原生缺口表、模型/算子清单、正确性夹具、独占 GPU 实验合同 | 明确已有/缺失/未验证；冻结模型、误差和测量口径 |
| P1 原生正确执行 | 官方权重读取、模型控制流、量化 CUDA 算子、完整原生生成与服务接入 | 单算子/层数值通过；完整多卡模型的文本、工具、缓存关闭控制组和取消通过 |
| P2 权重常驻与八卡效率 | HBM 放置、必要的 Engram 分层、TP/EP collectives、grouped GEMM、图复用、EPLB | 真实八卡证据、内存峰值有界、原生输出正确、kernel/通信瓶颈可归因 |
| P3 连续批处理与预算 | ragged continuous batching、prefill 成本分块、CPU/GPU/通信重叠 | 同负载下吞吐/延迟曲线改善，过载、取消、队列与公平性通过 |
| P4 KV 与长上下文 | 精确前缀复用、按需缓存分层、条件式 SWA replay | 冷暖/分叉/隔离/失效均正确；长上下文收益包含数据搬运成本 |
| P5 DSpark | 目标验证、状态回滚、固定策略控制组、条件式 confidence 预算 | 真实 accepted/proposed、验证开销和质量通过；分别选低延迟/高吞吐 profile |
| P6 XQ 与正式验收 | XQ 原生格式分支、代表性质量和匹配性能、晋升及回滚 | 两种权重分别验收；单请求与并发 1000 token/s 分别给出实际结果 |

依赖主链：`P0 → P1 → P2 → P3 → P4/P5 → P6`。P4/P5 可在已有正确控制组上分别研究；晋升顺序仍逐项进行，不把多个未测机制一起打开。

### P0：先把“原生”定义清楚

- 盘点 `embedded-inference`、`embedded-cuda`、`picolm` 和实际服务器构建；执行记录包含实际内核调用，不能只用 feature 名或后端自报名称。
- 将 F32 静态图、模型专用低精度执行和 CPU 参考路径分开。保留既有 F32 API；不能为了 V4.1 改变 OCR 等调用者的精度合同。
- 盘点官方模型的 dense/expert/Engram/KV 字节与格式、tokenizer 和模块边界。单卡只做算子/单层或缩小架构夹具，完整模型按多卡容量执行。
- 固定官方与 XQ 两个身份；原 XQ 的 server-only 二进制与测速口径升级作为部署准备项，防止下载完成后用旧计数跑正式实验。
- 取得独占节点后采集 topology、实际空闲 HBM、主存/NVMe、工具链与 profiler；控制连接失败只记录观察失败，不自动重提作业或重启下载器。

### P1：最小正确原生模型

- 模型所有者实现编码、CED/Engram、稀疏检索/注意力、MoE、KV 更新、输出投影与生成。权重字段从精确检查点 schema 解析，未知版本拒绝。
- CPU 微型参考用于数值解释；真正的 FP8/MXFP4 GPU 路径通过进程内内核接口执行，避免将整个模型解成 F32 后再搬运。
- 保留精确路由、比例因子和量化块语义。先关闭 replay、DSpark 与缓存复用，建立可回滚控制组。
- 原生 Backend/服务必须真实增量流式、返回实际 token 计数，支持自动工具、错误和取消；未实现的约束明确拒绝。
- 使用同一权重的参考层输出/teacher-forced logits 对照，数值预算在测试候选前冻结；最终 prose 不要求跨执行器位级一致。
- 完整官方模型放不进单卡时，P1 可先实现模型语义安全的按层切分与显式 peer transfer，作为容量/正确性控制组；每个状态边界均需验证。该串行交接路径不声称 TP/EP 效率，P2 再增加 collectives，因此完整模型验收不循环依赖后续优化。

### P2：让八卡执行值得使用

- 默认先计算 HBM 常驻方案。八卡容量允许时，避免每 token 从 NVMe 重载层；Engram 的 HBM/DRAM 放置以 gather、内存和通信实测选择。
- 提供量化权重设备驻留与激活驻留的独立预算，计入 KV、草稿、graph、scratch、通信临时区与碎片；不得靠超卖才能通过。
- 从正确的固定 TP/EP 控制组扩展到 attention DP + EP 对照。实际 all-reduce/all-to-all 的语义、同步、错误与取消必须有实现；设备 mesh 声明不算 collectives。
- MoE 先做精确 expert batch 合并与 grouped GEMM，再测冗余专家/动态放置。副本更新使用版本化、请求安全边界，不改 top-k 与 gate 值。
- 根据 profile 优先解决低比特 matmul fallback、host-device 往返、launch 空泡和 collective 等待。有限 shape 的 CUDA Graph、融合和 overlap 逐项 A/B。
- 权重搬运和算子调用不离开 Power 的 admission、驻留与收据边界；采用外部 CUDA 库也必须绑定实际版本、SM90 dispatch 与许可证。

### P3：把更多请求合成有效计算

- 复用 `ExecutionBatchLifecycle`，增加模型拥有的 KV slot 与 ragged token 语义；新成员在下个 step 边界加入，取消只移除其状态。
- prefill 用本机拟合的 `cost(q,c)` 与 TTFT/内存预算选 chunk。曲线需有硬件/内核/模型/shape/TTL 绑定，拒绝 NaN、外推及过期测量。
- 同时优化 request 数与实际 KV 长度，避免长提示长期占满 decode。初始保留有界固定策略作为回退控制组。
- 调度 CPU、内核和预取/通信重叠必须从 trace 证明；不能把更多排队时间隐藏在“decode token/s”中。

### P4：只复用已证实的状态

- 引擎内实现 caller/model/tokenizer/template/精确 token 前缀隔离的 KV 索引；采集实际 reused token、eviction、epoch 与读写成本。
- 仅在多个独立 KV 所有者之间考虑亲和；一个八卡 EP 组不是八个可任选的完整副本。
- 缓存容量成为已测瓶颈后才实验 DRAM/NVMe offload；每次命中都包含搬运成本，不预设越多缓存越快。
- SWA replay 放在模型层，依据窗口与最大回退覆盖选择后缀；单列数值/质量变更身份。完整输入不在网关截断。

### P5：先验证 DSpark，再分配验证预算

- 官方 V4.1 的 DSpark 与 XQ 辅助 GGUF 各自绑定目标权重、tokenizer、图和布局；不套普通 EAGLE/MTP 参数，也不叠加不同草稿策略。
- 覆盖全接受、全拒绝、部分接受、采样、工具/语法、取消和 rank 一致性；目标验证成功之前不提交任何草稿 token。
- 先用支持的固定验证形状建立控制组。只有模型提供兼容 confidence、内核支持可变验证长度，且 H200 成本曲线有效时，比较自适应验证预算。
- 分别测交互和高并发：DSpark 的 step 开销可能使大 batch 退化，因此低延迟与高吞吐 profile 可以选择不同策略。

## Constitution Check

状态是规划是否遵循约束，不是实现是否通过。

| Principle | Status | Evidence in the plan |
|---|---|---|
| I. Fail closed | PASS | FR-001–012；格式、能力、放置、成本曲线与状态拒绝合同 |
| II. Evidence retained | PASS | T025；保留负结果、旧记录及实际执行身份 |
| III. Hermetic tests | PASS | T005–022 每项行为同任务带小型夹具；GPU 实验另外验收 |
| IV. Closed sets | PASS | 类型与 ACL 未知值拒绝，新增 enum 有 parse/as_str |
| V. One workspace / published APIs | PASS, implementation gated | T001 先验证现存 Power 边界与本地闭包；新增模型 crate 必须接根 workspace、publish=false；不新增独立 workspace/lock，不改既有 F32 或公开 Rust API 语义 |
| VI. Local schema changes | N-A | 无数据库迁移 |
| VII. Shrink-only ratchets | PASS | 保留依赖、能力与证据守卫，不放宽 allowlist |
| VIII. Replaceable transports | PASS | 模型状态与通用执行合同独立；GPU 库为进程内实现细节 |
| IX. One configuration language | PASS | 只在拥有者现有 ACL 增加闭集字段；JSON 仅保存实验/夹具数据 |
| X. Commands / tests | PASS | 测试放 sibling 文件；不改桌面命令 |
| XI–XII. Judgment / adversarial review | PASS | T023 先冻结门槛，包含状态污染、约束混合、rank 故障及性能退化 |
| XIII. One repository | PASS | 代码在当前仓库；不新增远程仓库或兄弟 checkout |
| XIV. Names are not guessed | PASS | 模型名、revision、架构字段来自检查点；新路径为明确提案 |
| XV. Meaning is not matched | PASS | 模型声明与 typed contracts；不按字符串猜算子语义 |
| XVI–XVII. UI rules | N-A | 无产品 UI 修改 |

如果 T001 发现无法在上述约束内接入，则保留具体阻碍并停止该实现切片；不能以存量独立 manifest 自动豁免根工作区规则。

## Project Structure

### Documentation

`spec.md`、`plan.md`、`research.md`、`data-model.md`、`contracts/native-execution.md`、`tasks.md`。

### Source Code

下列新路径先由 T001 核验所有权与根工作区接入，再逐项实施；不得引入第二个工作区或锁文件。

| Layer | Path | 计划职责 / 新文件 |
|---|---|---|
| 模型所有者 | `crates/deepseek/`（提议） | `src/model.rs`、`weights.rs`、`layers.rs`、`state.rs`、`generation.rs`、`cuda.rs`、`parallel.rs`、`speculative.rs`、`reference.rs` 及对应 sibling tests；根 workspace 成员，依赖本地 Power |
| 通用原生机制 | `crates/power/src/inference/` | 复用 residency、routing、execution_batch、accelerator；只补真正通用的 typed low-precision/collective/性能合同，测试同目录 |
| 原生服务接入 | `crates/power/src/backend/` 与 `src/config/` | 通过现有 BackendRegistry 可注入边界接入模型所有者；依赖保持单向，避免 Power ↔ 模型 crate 循环依赖 |
| 原生组合入口 | 模型所有者的 `src/bin/` 或现有服务 composition owner | Rust 入口组合 Power API/资源合同与模型 Backend；不新增 SDK、语言绑定或配置系统 |
| H200 工具 | `scripts/h200_native/`（提议） | 所有权化部署、构建/输入身份、匹配测速、profile、回滚；复用既有 H200 操作边界 |
| 原 XQ 准备 | `scripts/h200_xq/` | T004 同步已验证 Power 计数修复、精确测速/正常隐私模式与延迟启动器 pins；不重启下载任务 |
| 验收 | `docs/evidence/power/<实施日期>-native-deepseek-h200/` | 原始样本、校准、输出/状态散列、失败和 OUTCOME |

模型所有者到 Power 是依赖方向。若既有 Power 服务没有可用 composition port，先添加模型中立的注册入口；不能让通用 Power crate 反向依赖 DeepSeek crate。

## Research / Existing Patterns

- `src/inference/routing.rs`：精确 route union 与权重保持。
- `src/inference/residency/`、`weights/range_io.rs`：驻留、带界限的读取、预取与取消。
- `src/inference/execution_batch/`：批成员 admission、原子提交和中途退出。
- `src/inference/accelerator/mesh_execution.rs`：已声明设备/传输预算，不承担模型 parallel policy。
- `src/inference/shape_profile.rs`：有限 shape 选择、失效绑定及显式 fallback。
- `src/backend/mod.rs`：Backend trait 与注册；`backend/proxy_usage_tests.rs` 提供真实 token 统计边界测试先例。
- `scripts/h200_xq/service.py`：进程所有权与停止边界；`scripts/h200_official/batch_campaign.py`：异常后恢复。
- `scripts/power_serving_qualification.py`：有限客户端、用量、失败保留；原生 runner 必须同时核验实际内核身份。

详见 [research.md](research.md)。

## Verification Plan

1. **无 GPU 的工作**：本地依赖/工作区审计、微型格式与数学参考、状态/路由/调度合同、测速工具测试先行；不等待 XQ 下载完才开始。
2. **真实 GPU 正确性**：算子与单层先单卡，完整模型再按已验证八卡放置。逐阶段 profile 不等于完整模型验收。
3. **匹配对照**：在同一独占八卡、同一权重/输入 tokens/模板/采样/思考/缓存条件下，保存参考与原生控制组。每项优化至少三次重复并交替运行顺序，保留波动。
4. **工作负载**：单用户代码/中文/数学/工具，多用户并发 1/4/8/16/32；容量证明通过后再增加 64/128。短输入/长输出、长输入、重复前缀、分叉前缀与无缓存分别记录。
5. **测量**：单请求完整耗时与 steady decode、总输出 token/s、TTFT p50/p95/p99、ITL、队列、实际 KV 复用、HBM/DRAM/NVMe、collective、draft/verify 成本及取消后的真实 GPU 空闲。单调客户端时钟与原生计数；包含 reasoning token 时明确标注。
6. **有限实验**：起始每请求 300 秒、恢复 30 秒、每对照单元 900 秒；长上下文若需不同预算，运行前冻结且两臂一致。达到边界保存失败，不隐藏超时后重跑成成功。
7. **晋升预算**：校准后、候选运行前冻结数值/质量/内存和延迟门槛。持续收益需超过重复波动；同一 offered load 下 p95 普通请求延迟默认不劣于控制组 10%。高吞吐与低延迟 profile 分别判定，不能以增加并发冒充同一用户提速。
8. **真实故障与恢复**：过载、queued/pre-output/mid-output 取消、rank 故障、草稿拒绝、约束混合、缓存 epoch 更新；恢复到原生控制组并重新正确性验证。未经所有权确认不停止共享 SGLang/Xolotl。
9. **源码检查**：修改 Rust 后按 owning build boundary 执行 focused tests、`cargo fmt --all -- --check`、根 `tests/workspace_dependencies.py`、`tests/crate_drift_report.py` 与相关 CI；保留存量失败，不改守卫。实际命令、结果和范围分别保存在实施证据中，不能用局部通过改写全仓失败。

P0 是当前可立即执行的切片；后续阶段依赖其明确产物与独占 GPU。无固定工期或未经实测的倍率承诺。

## 用户入口与临时凭据交付

2026-10-24 用户明确的性能门槛是完整 XQ 模型的**单个请求生成速度达到
1000 token/s**。后续优先降低单请求 decode 的实际计算、同步、通信与
验证成本；融合、TP/EP、驻留与 DSpark 分别通过匹配实验。batch 和增加
副本仍用于独立吞吐/服务验收，不能关闭单请求速度门槛。任何微内核倍率
不得直接当作完整模型提速；未达标即保持目标未完成。

T026 在原生部署验收之后执行。入口可以是任务独享的 H200 内网服务，配合用户机器拥有的 SSH 转发；必须明确每个地址的可达范围，从实际客户端验证。不能用仅服务器 loopback 的 health 返回作为用户可用证明。

为该任务实例生成新的高熵 Key，设置并验证到期与撤销，复用现有鉴权所有者；如果现有静态 Key 合同不提供 expiry，需要先完成有测试的限时凭据实现。默认交付窗口拟定两小时，交付时列明实际到期时区/时刻；在完成验收后才开始该窗口。服务重启不得延长窗口。

Key 仅在私有服务器文件中保存，并在用户明确要求的最终交付中提供一次；源码、证据和日志只包含身份散列、期限与验证结果。禁止复用共享引擎、其他服务或原下载器的凭据。

## 2026-10-23 补充：H200 上优先部署 XQ

本节回应“规划 H200 服务器部署 XQ 三值量化的 DeepSeek V4.1 Flash 模型部署”。延续用户此前的原生要求，最终交付是 **Power → 进程内 aster-deepseek → SM90 CUDA → XQ 权重**。现有 **Power → HTTP → Xolotl** 作业提供同一 XQ 的参考结果，可以独立验收，但不能关闭原生部署任务。本节只更新规划和观察证据，没有新建、重提、修改或停止远程作业。

### 现状和模型身份

以下状态采集于 **2026-10-23 21:10 +08:00**，不是资源可用保证。

| 项目 | 已确认事实 | 尚未满足的门槛 |
|---|---|---|
| XQ V3 主权重 | 16 片，共 162,954,903,552 字节；原下载器逐片完成 SHA256，观察器核对大小、固定清单及完成标记 | 在获配节点拼接，重新计算完整 GGUF 的 SHA256；观察器没有独立重读全部 payload |
| 辅助权重 | 1017 Q2_K MTP/Vision sidecar 已下载，5,661,492,320 字节 | 只在 DSpark 实验时使用；不代表原生 DSpark 或视觉已支持 |
| 外部 XQ 作业 | `aster-power-xq-h200-final-6522e5af2fc3-31676386`，UID `6230a46e-8868-4e2c-a803-0321962f98a6`，仍 Inqueue | 尚无节点、整模生成、性能或用户入口验收 |
| 原生 CUDA | Linux Rust 测试二进制已构建；两个独立 H200 内核作业仍 Inqueue | 真实 H200 数值与 Rust adapter 运行；编译成功不等于 GPU 执行 |
| 原生 XQ 服务 | 当前 `DeepSeekModel` 只接受官方模型；原生权重加载绑定官方 safetensors | XQ GGUF/混合量化设备分支、生成循环、放置和服务 composition 均需补齐 |

观察依据：[权重检查](../../docs/evidence/power/2026-10-22-native-deepseek-h200/xq-completion-inspection-1792760970.json)、[三个自有作业的状态与资源](../../docs/evidence/power/2026-10-22-native-deepseek-h200/h200-owned-job-states-1792761029.json)。最新八卡调度事件列出 CPU、内存及节点选择限制，不能将所有排队原因概括成“缺 GPU”。

目标固定为 `IndrasMirror/DeepSeek-V4.1-Flash-1017-XQ-Uncensored`，revision `9d3c01d86709261e51d566b02e1dda75f40d009a`，V3 GGUF 完整 SHA256 为 `73df1934302f867601dd27f7fca59109d9b11674f50cc60526ec2a1cd4f3d389`。这是 PTQ1_0 / Q2_0 / Q3_K 混合专家量化，另含 Q8_0/F16/F32 张量；“三值量化”不表示整模统一 TQ1_0。该社区检查点带模型编辑，质量验收以同一 XQ 为准，不能套用官方权重的正确性结果。[固定版本模型卡](https://huggingface.co/IndrasMirror/DeepSeek-V4.1-Flash-1017-XQ-Uncensored/blob/9d3c01d86709261e51d566b02e1dda75f40d009a/README.md)

参考引擎固定 Xolotl `b3452666e07a6374d06bb8c2185dffa271e8f65f`、ggml/llama `3cf03257f219afbe7334045ff7c6a06ac68c627d`；不在部署中自动升级 revision 或替换成其他 GGUF。模型、rank、expert map、tokenizer、Engram 和 sidecar 分别保留身份。

### 资源和放置

| 资源 | 初始部署方案 | 上机必须验证 |
|---|---|---|
| GPU | 同节点独占 8×H200；SM90；逐卡保留当前 8 GiB VRAM margin 作为起始预算 | 型号、UUID、可见序号、实际可用 HBM、无其他 GPU 任务，以及 `nvidia-smi topo -m` / P2P；不把八卡 HBM 当成自动共享地址空间 |
| CPU / 主存 | 沿用已提交的 160 CPU / 1760 GiB，先记录实际峰值 | 这是当前作业的请求量，不是最低硬件需求；若考虑缩配，先核对资源池约束及实测工作集，再以新版本方案变更，不重复抢占提交 |
| NVMe | 当前 installer 要求真实 `/nvme` 且空闲 ≥500 GiB；规划建议留 ≥1 TiB 空闲 | 实际文件系统、空间、随机读和带宽；按 GGUF、两层 Engram、构建、临时文件及日志计算峰值；共享盘保留已验证分片 |
| Engram | 从固定官方 snapshot 提取第 1、14 层至两个独立目录，保持现有精确 SHA256 校验 | 索引、张量字节、加载语义与取消；原生路径优先比较 HBM/DRAM 常驻，NVMe 只作为有测量的分层方案 |
| 工具链 | Rust 1.97.1、根 workspace/lock；参考引擎 CUDA architecture=90 | 实际驱动和 CUDA ABI、动态库、内核身份；link-only stub 不能参与硬件资格验证 |

H200 标称每卡 141 GB HBM，八卡标称合计 1,128 GB；这是容量起点，不能替代逐卡权重、Engram、KV、通信和 scratch 峰值证明。[NVIDIA H200 规格](https://www.nvidia.com/en-us/data-center/h200/)

第一版原生放置使用显式逐层切分与真实跨卡状态交接，先证明容量及语义。Xolotl 当前请求边界 `5,10,15,20,25,30,35` 仅作为参考配置输入；其 `ds41_pipeline.cpp` 会核对 indexer-safe 边界并可能调整。必须保存实际生效边界、shadow/import 层、KV/indexer 交接与八卡参与证据，不能只匹配请求参数或一行 ready 日志。源码依据：[切分检查记录](../../docs/evidence/power/2026-10-22-native-deepseek-h200/xq-layer-split-related-source-inspection.json)。原生实现复用已验证的模型状态依赖，不能复制分界数字就假设正确。

### 八卡均衡与 KV 亲和的实际边界

当前 `split8` 配置是一个跨八卡的模型执行组：请求经 Power 有界队列进入同一组，按固定 stage 顺序执行。`service.py` 将其 `max_concurrent_requests` 设为 1，并拒绝 split8 的 parallel>1；这提供容量控制，不构成八卡动态负载均衡。按约五层切分只是初始放置，实际耗时还受注意力、Engram、专家、输出头和状态交接影响。阶段分界只有在数值/状态依赖正确且逐 stage trace/内存测量完成后才能调整，不在请求执行中为追求均衡随意移动层。

同一次请求的 prefill 与 decode 固定在同一执行组；各层 KV 由固定 stage 所有者持有，跨 stage 的共享 KV/indexer 依赖通过已声明的 shadow/import 状态交接处理。这个所有权约束与多轮缓存命中是不同门槛：初始 exact profile 的 `STRATA_SERVE_KEEP_CACHE=0`，没有承诺跨请求复用。keep-cache 候选只有在真实前缀匹配、分叉、驱逐及隔离验收后才能宣称有效亲和。21:36 +08:00 的精确 UID 观察仍为 Inqueue，见 [路由核对记录](../../docs/evidence/power/2026-10-22-native-deepseek-h200/xq-routing-observation-1792762595.json)；尚无八卡运行证据。

当后续部署多个完整执行组时，亲和单位是**执行组 + 缓存所有者/epoch**，再根据以下策略做请求路由；这些策略尚未接入本次原生部署：

1. 查找经过身份校验的缓存目录，校验 caller、模型/量化 revision、tokenizer、模板、有效 epoch 和精确 token 前缀；会话 ID 仅定位所有者，不能证明缓存可复用。
2. 在健康且满足 KV/内存准入的组中，比较“排队时间 + 未命中前缀的 prefill 成本 + decode 成本 + 必要状态搬运成本”。有有效长前缀的旧组优先获得成本优势，新会话按实际工作量分配；不能只比较 GPU 利用率或请求数。
3. 一次已准入生成固定在其组内完成。后续轮次若旧组过载，选择有界等待、明确拒绝，或在新组按相同输入重新 prefill；跨组 KV 迁移需完整状态协议、版本校验及真实搬运测量，未实现时不依赖迁移。
4. 组故障、缓存驱逐或 epoch 变化导致缓存失效时重新 prefill。保证的是输出语义和状态隔离；持续缓存命中不能在故障或过载下无条件保证。

### 实施顺序和退出条件

| 里程碑 | 工作及现有任务映射 | 通过条件 |
|---|---|---|
| X0 固定输入 / 恢复状态 | T004/T024：保留分片和旧失败记录；只观察已提交的精确 UID；校验 operator/source/tokenizer/rank/Engram 清单 | 文件身份匹配；重复观察不产生新作业，不重启已完成的下载器 |
| X1 建立 XQ 参考 | T004/T023：获配后构建 sm90 Xolotl、提取 Engram、拼接并校验 GGUF；关闭 DSpark、KV 降精度和缓存复用，建立参考控制组 | 完整权重 hash、实际切分、加载及真实生成通过；保存同一 tokenizer/template 的固定输入 IDs、logits/层输出及任务结果 |
| X2 补原生 XQ 格式 / 内核 | T022，并复用 T008 ABI：新增明确的 XQ 模型身份、GGUF 张量/专家 map；实现 PTQ1_0/Q2_0/Q3_K/Q8_0 及 F16/F32 精确布局，逐类资格验证 SM90 dispatch | CPU 小块解包与独立参考一致；截断/未知类型/错误 scale 拒绝；H200 小矩阵和单层通过预先冻结的误差预算；保留量化存储，不全模展开 F32 |
| X3 完整原生生成 | T009/T011：tokenizer/模板、40 层执行、Engram、MoE、KV、输出头、采样/停止；按真实逐卡预算常驻并显式交接状态 | 先短上下文单请求，再增至 32K；同一 XQ 的数学、中文、代码、工具、检索和取消/回滚通过；参考 HTTP 禁用时原生仍能生成 |
| X4 原生服务接入 | T010/T024：Backend composition、真实增量 SSE、准确 usage、有界 admission、取消/恢复；未知能力拒绝 | 从实际客户端访问 `/v1/models`、非流式与流式 chat、工具调用；过载有界、取消后真实 GPU 工作停止、drain/重启/原生控制组恢复通过 |
| X5 用户交付 | T023/T025/T026：冻结通过的配置及构建身份，完成限时鉴权、入口验证和证据 | 交付可达 Base URL、精确模型别名、新 Key、到期时刻、撤销方式和能力边界；匿名/错误/过期/撤销 Key 被拒绝，重启不延长期限 |
| X6 性能晋升 | T012–021：在 X3–X5 的正确控制组上分别实验 TP/EP、batch、KV、DSpark 和长上下文 | 每个候选先数值/质量后测速，失败回到已通过配置；报告单请求与总吞吐，分别判定原计划的 1000 token/s 目标 |

本次 XQ 优先级调整的执行顺序为：**共享原生执行能力 + T022 → X3–X5 最小交付 → 高级优化**。不要求先完成官方权重的所有优化、128K 上下文、DSpark 或 1000 token/s 才交付经过验收的基本 XQ 服务；官方模型和各优化任务继续独立保留。本节没有改变任务勾选或宣称 T008–T010/T022/T026 已完成。用户重新启动部署目标后，已在 `tasks.md` 同步具体依赖：先做不依赖 GPU 的 XQ 格式切片，再关闭相应 CUDA/整模/服务门槛。

### 质量、性能与接口验收

1. XQ 使用独立版本化预算，不能直接沿用官方检查点的 logits 基准。小块解码先做精确字节/数值参考，CUDA 算子及整模误差预算在候选运行前冻结；固定输入 token、模板、seed、思考模式和采样。现有四项数学/工具 smoke 及两个检索用例仅是冒烟检查，不能单独证明模型质量。
2. 初始承诺仅为通过验收的文本/工具能力和上下文上限。32K、128K 分阶段增加并分别证明 KV、长文检索与取消；sidecar 存在不增加视觉能力。DSpark、KV4、keep-cache 分别开关做匹配对照，未知/未支持能力明确拒绝。
3. 先测并发 1，再按实际容量增加 4/8/16/32；保留失败、超时和队列。每个配置至少三轮，分别报告端到端单请求 token/s、steady decode、并发总输出 token/s、TTFT p50/p95/p99、ITL、错误率与逐卡 HBM / 主存 / NVMe 峰值。reasoning token 单列计数口径。
4. 当前 `service.py` 对 split8 限制 parallel=1；这是现有适配器的能力边界。8×1、4×2 或新增 2×4 副本是后续候选，不是默认性能结论：单卡 141 GB 小于主 GGUF 的 162.95 GB，不能宣称八个单卡副本都能全量 HBM 常驻。副本缓存、路由和身份隔离需要独立验收。
5. 先采用既有有限实验合同：普通请求 300 秒、恢复 30 秒、每个对照单元 900 秒；长上下文若需调整，先冻结两臂一致的新预算。当前外部 `qualify.py` 的 1800 秒请求超时、installer 自动广泛 autotune 不能直接当作此有界原生 campaign；修改须有新 operator 版本、tests 和所有权检查，不覆盖已提交包。
6. 参考服务当前配置 Power 前端监听 `0.0.0.0:30002`、上游监听 `127.0.0.1:8150`；前端监听所有接口不证明集群外可达，尚无通过实际客户端验收的 URL。原生实例使用单独拥有的入口；可先通过用户侧 SSH 转发交付内网访问，若要求公网则另验收 TLS 入口。实际客户端连通及真实生成后才能报告“可访问”。
7. `service.py` 目前生成静态 Key；限时、到期和撤销还不是已实现能力。原生交付继续按 T026 实现并验证默认两小时窗口，在验收完成后开始计时。Key 仅保存在私有文件并向用户交付一次，证据、参数和日志中不写明文。

### 文件落点与约束复核

| 所有者 / 拟修改文件 | 复用的当前模式 | 所需验证 |
|---|---|---|
| `crates/deepseek/src/model.rs`、`config.rs`、`weights/xq.rs` 及 sibling tests | 现有闭集模型/ACL、`weights.rs` 有界读取与精确 checkpoint 绑定 | 未知 revision/format、坏块、路径及预算错误拒绝；官方身份不变 |
| `crates/deepseek/src/cuda/`、拟建 `generation.rs` / `parallel.rs` 及 sibling tests | 现有 `cuda/execution.rs`、`state/transaction.rs`、`model/sharing.rs` | XQ 每格式 CPU/GPU 数值、整模循环、跨卡状态及取消 |
| `crates/power/src/backend/mod.rs`、模型 composition、`server/auth.rs` | 现有注入边界、通用 admission 和鉴权所有者 | 禁止 HTTP/CPU fallback；真实流式/工具/usage；限时与撤销 |
| 拟建 `scripts/h200_native/`、现有 `scripts/h200_xq/` | 现有 deferred launcher 锁、精确 UID、manifest、失败保留及 benchmark 隐私恢复 | 新行为小型 hermetic tests；GPU 实验另保留收据；不对共享参考服务写入或执行实验 |

本节不改变既有 Constitution Check：所有代码仍在本仓库，Rust 接根 workspace 和本地 Aster 依赖，`publish=false`，产品配置只用 ACL。JSON manifest/实验结果仅是数据；不得照搬参考引擎的 JSON 配置作为新的 Power 产品配置。外部引擎源码/内核复用要保留许可证、精确版本和实际进程边界。规划文档检查链接、pins、任务映射和证据即可；实现时再执行 owning package tests、fmt 和根工作区守卫。

当前先决条件是已有 H200 作业获得资源，以及原生 XQ 必需实现通过资格验证。排队时间与完整原生研发时间分别记录，当前没有可靠的固定上线时间、H200 吞吐实测或可交付 URL/Key。

### 充分利用八卡性能与压力验收

2026-10-24 的目标已明确要求多个**完整模型副本**，不再只是可选优化。
T028/T029 必须同时保留容量证明、真实整模生成、组内状态所有权和跨组隔离证据。
当前新增闭集组宽 2/4/8；四个双卡 peer 小测试通过仅证明组件能力。
首先在一个双卡组完成完整 40 层生成，再启动四个互不重叠的双卡进程，
以私有 barrier 等待四个完整模型同时驻留，记录八张卡的 HBM 后同步释放生成。
各组必须报告自己的物理 UUID、完整模型身份、40 层执行统计及一致的固定输入输出。
四副本冒烟通过后仍需 Power Backend、请求路由、真实客户端及以下性能/压力门槛。

候选容量不能仅按主 GGUF 计算。主文件为 162954903552 字节，约 151.76 GiB；
两层 Engram 的 embedding 与 scale 合计约 188.83 GiB。若四副本各复制一份
完整 GPU Engram 表，仅这两项就约 1362 GiB，超过八卡约 1123.21 GiB 的
实际可见 HBM，不能准入这种布局。当前原生路径使用完整认证文件的有界
主机稀疏读取；这没有证明 HBM Engram 缓存或最佳性能。比较 2×4、4×2 时
必须分别测量主机 gather、GPU 驻留、通信与 batch 的成本；共享 GPU Engram
表需要另外实现明确的跨组设备映射、所有权和生命周期，不能用估算代替实现。

- 在独占同一节点上测量 kernel/CPU launch 空泡、逐 rank HBM 带宽/峰值、矩阵 shape/精度、collective/peer copy 和 Engram/主机 I/O；用真实执行 trace 选择下一瓶颈。八段串行切层仅提供容量控制组，必须继续比较已实现的 TP/EP、合适的副本组和 batch，不能将其启动成功当作服务器性能已充分利用。
- 晋升矩阵至少覆盖同一 XQ 的常驻/必要分层、串行控制组/实际可用并行组、并发 1/4/8/16/32、冷/暖/分叉前缀、关闭/开启已资格验证的 KV 和 DSpark。2×4、4×2、8×1 是需要各自证明容量和执行能力的候选；缺乏实现或容量时记录具体拒绝，不能伪造一个只改 admission 的并行结果。每一候选至少三次匹配重复，收益需超过运行波动；分别保存低延迟和高吞吐的最佳已测配置及剩余瓶颈。
- 在 T027 campaign 前冻结短/中/长输入（计划 2K/8K/32K，128K 仅在其能力通过后加入）、128/512/2048 输出长度、思考模式、请求混合、 offered load、准入上限、worker 数、队列/每请求/整轮停止预算和数值/内存/时延门槛。先逐级升压找出可持续区间，再为交互和吞吐配置分别运行至少 30 分钟持续负载；达到限制就停止并保留失败，不通过无界重试把失败变成成功。
- 压力结果逐请求保留输入身份、真实用量、TTFT/ITL/端到端耗时、成功/错误/超时/取消/过载拒绝，分别报告 accepted load 与 offered load。监测资源峰值和持续趋势；准入容量内要求无错误结果、跨请求状态串用、OOM 或未恢复故障，时延及内存预算按运行前冻结值判定。
- 演练 queued/pre-output/mid-output 取消、长短混合、工具、多轮/分叉/隔离、缓存失效、过载、任务自有 rank/worker 失败与恢复。测试结束后恢复晋升配置和正常隐私模式，从用户入口再做真实 stream/tool/鉴权验收。最终两小时 Key 窗口在优化、压力和恢复验收之后开始；压力工具使用自己的任务私有凭据，不能消耗用户的交付窗口。
- “极致”通过完整支持矩阵、可复现收益、硬件瓶颈证据及无收益/退化记录来验收，不宣称未经证明的绝对硬件上限。单用户速度与并发总吞吐各自保留真实结果和 1000 token/s 目标差距，不以其中一个覆盖另一个。
- 用户要求每轮完成立即汇报：优化前后数值、变化比例、正确性、采纳决定和计时口径。算子、端到端、原生 decode 与并发总吞吐分别报告；无收益或退化同样保留，不等最终部署完成才汇报。
